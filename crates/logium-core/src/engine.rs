use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::sync::Arc;

use chrono::NaiveDateTime;
use rayon::prelude::*;
use regex::{Regex, RegexSet};
use serde::{Deserialize, Serialize};

use crate::model::*;

// ---------------------------------------------------------------------------
// Time-range filtering
// ---------------------------------------------------------------------------

/// Optional start/end bounds for time-range filtering.
/// Lines before `start` are skipped; lines after `end` cause an early break
/// (the merged stream is chronological).
#[derive(Debug, Clone, Default)]
pub struct TimeRange {
    pub start: Option<NaiveDateTime>,
    pub end: Option<NaiveDateTime>,
}

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors that can occur during analysis.
#[derive(Debug)]
pub enum AnalysisError {
    InvalidRegex(String),
    InvalidTimestampFormat(String),
    FileNotFound(String),
    ParseError(String),
}

impl std::fmt::Display for AnalysisError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnalysisError::InvalidRegex(s) => write!(f, "invalid regex: {s}"),
            AnalysisError::InvalidTimestampFormat(s) => write!(f, "invalid timestamp format: {s}"),
            AnalysisError::FileNotFound(s) => write!(f, "file not found: {s}"),
            AnalysisError::ParseError(s) => write!(f, "parse error: {s}"),
        }
    }
}

impl std::error::Error for AnalysisError {}

// ---------------------------------------------------------------------------
// Streaming analysis events
// ---------------------------------------------------------------------------

/// Events emitted during streaming analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
#[serde(rename_all = "snake_case")]
pub enum AnalysisEvent {
    RuleMatch(RuleMatch),
    PatternMatch(PatternMatch),
    StateChange(StateChange),
    Progress {
        lines_processed: u64,
    },
    Complete {
        total_lines: u64,
        total_rule_matches: u64,
        total_pattern_matches: u64,
        total_state_changes: u64,
    },
    Error {
        message: String,
    },
}

// ---------------------------------------------------------------------------
// Log line parser (streaming iterator)
// ---------------------------------------------------------------------------

/// Lazily yields `LogLine` items from a source file.
pub struct LogLineIterator {
    reader: BufReader<File>,
    source_id: u64,
    timestamp_format: String,
    extraction_regex: Option<Regex>,
    default_year: Option<i32>,
    content_regex: Option<Regex>,
    continuation_regex: Option<Regex>,
    json_timestamp_field: Option<String>,
    pending_line: Option<String>,
    buf: String,
}

impl LogLineIterator {
    pub fn new(
        source: &Source,
        template: &SourceTemplate,
        ts_template: &TimestampTemplate,
    ) -> Result<Self, AnalysisError> {
        let file = File::open(&source.file_path)
            .map_err(|_| AnalysisError::FileNotFound(source.file_path.clone()))?;
        let content_regex = match &template.content_regex {
            Some(pat) => {
                let re = Regex::new(pat).map_err(|e| AnalysisError::InvalidRegex(e.to_string()))?;
                Some(re)
            }
            None => None,
        };
        let extraction_regex = match &ts_template.extraction_regex {
            Some(pat) => {
                let re = Regex::new(pat).map_err(|e| AnalysisError::InvalidRegex(e.to_string()))?;
                Some(re)
            }
            None => None,
        };
        let continuation_regex = match &template.continuation_regex {
            Some(pat) => {
                let re = Regex::new(pat).map_err(|e| AnalysisError::InvalidRegex(e.to_string()))?;
                Some(re)
            }
            None => None,
        };
        Ok(Self {
            reader: BufReader::with_capacity(64 * 1024, file),
            source_id: source.id,
            timestamp_format: ts_template.format.clone(),
            extraction_regex,
            default_year: ts_template.default_year,
            content_regex,
            continuation_regex,
            json_timestamp_field: template.json_timestamp_field.clone(),
            pending_line: None,
            buf: String::new(),
        })
    }
}

impl Iterator for LogLineIterator {
    type Item = Result<LogLine, AnalysisError>;

    fn next(&mut self) -> Option<Self::Item> {
        // Get the head line: either from pending_line or by reading from the reader.
        let head_line = if let Some(pending) = self.pending_line.take() {
            pending
        } else {
            self.buf.clear();
            match self.reader.read_line(&mut self.buf) {
                Ok(0) => return None,
                Ok(_) => self
                    .buf
                    .trim_end_matches('\n')
                    .trim_end_matches('\r')
                    .to_string(),
                Err(e) => return Some(Err(AnalysisError::ParseError(e.to_string()))),
            }
        };

        // If continuation_regex is set, merge continuation lines.
        let merged_raw = if let Some(cont_re) = &self.continuation_regex {
            let mut merged = head_line;
            loop {
                self.buf.clear();
                match self.reader.read_line(&mut self.buf) {
                    Ok(0) => break, // EOF
                    Ok(_) => {
                        let line = self
                            .buf
                            .trim_end_matches('\n')
                            .trim_end_matches('\r')
                            .to_string();
                        if cont_re.is_match(&line) {
                            merged.push('\n');
                            merged.push_str(&line);
                        } else {
                            self.pending_line = Some(line);
                            break;
                        }
                    }
                    Err(e) => return Some(Err(AnalysisError::ParseError(e.to_string()))),
                }
            }
            merged
        } else {
            head_line
        };

        // JSON mode: parse line as JSON, extract timestamp from configured field
        if let Some(ref field_name) = self.json_timestamp_field {
            let json_val: serde_json::Value = match serde_json::from_str(&merged_raw) {
                Ok(v) => v,
                Err(e) => {
                    return Some(Err(AnalysisError::ParseError(format!(
                        "failed to parse JSON: {e}"
                    ))));
                }
            };

            let ts_str = match json_val.get(field_name).and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => {
                    return Some(Err(AnalysisError::ParseError(format!(
                        "JSON field '{}' not found or not a string",
                        field_name
                    ))));
                }
            };

            let timestamp = NaiveDateTime::parse_from_str(&ts_str, &self.timestamp_format)
                .or_else(|_| parse_timestamp_prefix(&ts_str, &self.timestamp_format))
                .or_else(|e| {
                    if let Some(year) = self.default_year {
                        let augmented_input = format!("{year} {ts_str}");
                        let augmented_fmt = format!("%Y {}", self.timestamp_format);
                        NaiveDateTime::parse_from_str(&augmented_input, &augmented_fmt)
                            .or_else(|_| parse_timestamp_prefix(&augmented_input, &augmented_fmt))
                    } else {
                        Err(e)
                    }
                });

            return match timestamp {
                Ok(ts) => {
                    let raw: Arc<str> = Arc::from(merged_raw);
                    Some(Ok(LogLine {
                        timestamp: ts,
                        source_id: self.source_id,
                        content: Arc::clone(&raw),
                        raw,
                        cached_json: Some(json_val),
                    }))
                }
                Err(e) => Some(Err(AnalysisError::InvalidTimestampFormat(format!(
                    "failed to parse timestamp from '{}' with format '{}': {}",
                    ts_str, self.timestamp_format, e
                )))),
            };
        }

        // For timestamp and content_regex, use only the first physical line.
        let first_line = merged_raw
            .split_once('\n')
            .map_or(merged_raw.as_str(), |(first, _)| first);

        let content_override: Option<String> = if let Some(re) = &self.content_regex {
            if let Some(caps) = re.captures(first_line) {
                let head_content = caps
                    .get(1)
                    .map_or(first_line.to_string(), |m| m.as_str().to_string());
                // Append continuation lines to content
                Some(if let Some((_first, rest)) = merged_raw.split_once('\n') {
                    format!("{head_content}\n{rest}")
                } else {
                    head_content
                })
            } else {
                None
            }
        } else {
            None
        };

        // Extract timestamp substring: use extraction_regex if set, otherwise first line
        let ts_input = if let Some(re) = &self.extraction_regex {
            if let Some(caps) = re.captures(first_line) {
                caps.get(1)
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_else(|| first_line.to_string())
            } else {
                first_line.to_string()
            }
        } else {
            first_line.to_string()
        };

        let timestamp = NaiveDateTime::parse_from_str(&ts_input, &self.timestamp_format)
            .or_else(|_| parse_timestamp_prefix(&ts_input, &self.timestamp_format))
            .or_else(|e| {
                if let Some(year) = self.default_year {
                    let augmented_input = format!("{year} {ts_input}");
                    let augmented_fmt = format!("%Y {}", self.timestamp_format);
                    NaiveDateTime::parse_from_str(&augmented_input, &augmented_fmt)
                        .or_else(|_| parse_timestamp_prefix(&augmented_input, &augmented_fmt))
                } else {
                    Err(e)
                }
            });

        match timestamp {
            Ok(ts) => {
                let raw: Arc<str> = Arc::from(merged_raw);
                let content = match content_override {
                    Some(s) => Arc::from(s),
                    None => Arc::clone(&raw),
                };
                Some(Ok(LogLine {
                    timestamp: ts,
                    source_id: self.source_id,
                    raw,
                    content,
                    cached_json: None,
                }))
            }
            Err(e) => Some(Err(AnalysisError::InvalidTimestampFormat(format!(
                "failed to parse timestamp from '{}' with format '{}': {}",
                first_line, self.timestamp_format, e
            )))),
        }
    }
}

/// Estimate the (min, max) output length of a chrono format string.
/// Used to narrow the search window in `parse_timestamp_prefix`.
fn estimate_timestamp_len(fmt: &str) -> (usize, usize) {
    let bytes = fmt.as_bytes();
    let mut min_len = 0usize;
    let mut max_len = 0usize;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 1 < bytes.len() {
            i += 1;
            match bytes[i] {
                // Width-prefixed subsecond: %3f, %6f, %9f
                b'3' | b'6' | b'9' if i + 1 < bytes.len() && bytes[i + 1] == b'f' => {
                    let w = (bytes[i] - b'0') as usize;
                    min_len += w;
                    max_len += w;
                    i += 1;
                }
                // Timezone offset with colon: %:z → "+00:00" (6 chars)
                b':' if i + 1 < bytes.len() && bytes[i + 1] == b'z' => {
                    min_len += 6;
                    max_len += 6;
                    i += 1;
                }
                b'Y' => {
                    min_len += 4;
                    max_len += 4;
                }
                b'C' | b'y' | b'm' | b'd' | b'e' | b'H' | b'I' | b'M' | b'S' => {
                    min_len += 2;
                    max_len += 2;
                }
                b'b' | b'h' | b'a' | b'j' => {
                    min_len += 3;
                    max_len += 3;
                }
                b'B' | b'A' => {
                    min_len += 3;
                    max_len += 9;
                }
                b'p' | b'P' => {
                    min_len += 2;
                    max_len += 2;
                }
                b'z' => {
                    min_len += 5;
                    max_len += 5;
                } // "+0000"
                b'Z' => {
                    min_len += 3;
                    max_len += 5;
                } // timezone abbreviation
                b'u' | b'w' => {
                    min_len += 1;
                    max_len += 1;
                }
                b'f' => {
                    min_len += 1;
                    max_len += 9;
                } // nanoseconds, variable
                b'%' => {
                    min_len += 1;
                    max_len += 1;
                } // literal %
                _ => {
                    min_len += 1;
                    max_len += 6;
                } // unknown specifier
            }
        } else {
            min_len += 1;
            max_len += 1;
        }
        i += 1;
    }
    (min_len, max_len)
}

/// Parse a timestamp from the beginning of a line by trying progressively
/// shorter prefixes until chrono can parse it without "trailing input" errors.
///
/// Estimates the expected timestamp length from the format string to try a
/// narrow window first (typically 1-5 attempts), falling back to a full scan
/// only for exotic format strings where the estimate is wrong.
fn parse_timestamp_prefix(line: &str, fmt: &str) -> Result<NaiveDateTime, chrono::ParseError> {
    let (min_ts, max_ts) = estimate_timestamp_len(fmt);

    // Narrow window: [min_ts - 1, max_ts + 1], clamped to valid range.
    // Covers the expected timestamp length with a small margin for edge cases.
    let lo = min_ts.saturating_sub(1).max(1).min(line.len());
    let hi = (max_ts + 1).min(line.len());

    let mut last_err = None;
    for end in (lo..=hi).rev() {
        if !line.is_char_boundary(end) {
            continue;
        }
        match NaiveDateTime::parse_from_str(&line[..end], fmt) {
            Ok(ts) => return Ok(ts),
            Err(e) => {
                if last_err.is_none() {
                    last_err = Some(e);
                }
            }
        }
    }

    // Fallback: full scan for exotic formats where the estimate was wrong.
    let min_len = fmt.len().min(line.len());
    for end in (min_len..lo).rev() {
        if !line.is_char_boundary(end) {
            continue;
        }
        match NaiveDateTime::parse_from_str(&line[..end], fmt) {
            Ok(ts) => return Ok(ts),
            Err(e) => {
                if last_err.is_none() {
                    last_err = Some(e);
                }
            }
        }
    }
    for end in ((hi + 1)..=line.len()).rev() {
        if !line.is_char_boundary(end) {
            continue;
        }
        match NaiveDateTime::parse_from_str(&line[..end], fmt) {
            Ok(ts) => return Ok(ts),
            Err(e) => {
                if last_err.is_none() {
                    last_err = Some(e);
                }
            }
        }
    }

    Err(last_err.unwrap_or_else(|| NaiveDateTime::parse_from_str(line, fmt).unwrap_err()))
}

// ---------------------------------------------------------------------------
// Pre-processed line (Phase 1 output)
// ---------------------------------------------------------------------------

/// A log line with pre-computed rule evaluation results from parallel Phase 1.
struct ProcessedLine {
    line: LogLine,
    rule_matches: Vec<(u64, HashMap<String, StateValue>)>, // (rule_id, extractions)
    json_fields: Option<HashMap<String, StateValue>>,
}

// ---------------------------------------------------------------------------
// K-way merge (min-heap)
// ---------------------------------------------------------------------------

struct HeapItem {
    line: LogLine,
    source_idx: usize,
}

impl PartialEq for HeapItem {
    fn eq(&self, other: &Self) -> bool {
        self.line.timestamp == other.line.timestamp
    }
}

impl Eq for HeapItem {}

// Reversed ordering for min-heap behavior with BinaryHeap (which is a max-heap).
impl PartialOrd for HeapItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HeapItem {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .line
            .timestamp
            .cmp(&self.line.timestamp)
            .then_with(|| other.source_idx.cmp(&self.source_idx))
    }
}

/// Merges multiple `LogLineIterator` streams in chronological order.
pub struct MergedLogStream {
    heap: BinaryHeap<HeapItem>,
    iterators: Vec<LogLineIterator>,
}

impl MergedLogStream {
    pub fn new(mut iterators: Vec<LogLineIterator>) -> Result<Self, AnalysisError> {
        let mut heap = BinaryHeap::with_capacity(iterators.len());
        for (idx, iter) in iterators.iter_mut().enumerate() {
            if let Some(result) = iter.next() {
                let line = result?;
                heap.push(HeapItem {
                    line,
                    source_idx: idx,
                });
            }
        }
        Ok(Self { heap, iterators })
    }
}

impl Iterator for MergedLogStream {
    type Item = Result<LogLine, AnalysisError>;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.heap.pop()?;
        // Refill from the same source
        if let Some(result) = self.iterators[item.source_idx].next() {
            match result {
                Ok(line) => {
                    self.heap.push(HeapItem {
                        line,
                        source_idx: item.source_idx,
                    });
                }
                Err(e) => return Some(Err(e)),
            }
        }
        Some(Ok(item.line))
    }
}

// ---------------------------------------------------------------------------
// K-way merge for ProcessedLine (Phase 2)
// ---------------------------------------------------------------------------

struct ProcessedHeapItem {
    processed: ProcessedLine,
    source_idx: usize,
}

impl PartialEq for ProcessedHeapItem {
    fn eq(&self, other: &Self) -> bool {
        self.processed.line.timestamp == other.processed.line.timestamp
    }
}

impl Eq for ProcessedHeapItem {}

impl PartialOrd for ProcessedHeapItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ProcessedHeapItem {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .processed
            .line
            .timestamp
            .cmp(&self.processed.line.timestamp)
            .then_with(|| other.source_idx.cmp(&self.source_idx))
    }
}

/// Merges multiple pre-processed source Vecs in chronological order.
struct ProcessedLineMerger {
    heap: BinaryHeap<ProcessedHeapItem>,
    iters: Vec<std::vec::IntoIter<ProcessedLine>>,
}

impl ProcessedLineMerger {
    fn new(sources: Vec<Vec<ProcessedLine>>) -> Self {
        let mut iters: Vec<std::vec::IntoIter<ProcessedLine>> =
            sources.into_iter().map(|v| v.into_iter()).collect();
        let mut heap = BinaryHeap::with_capacity(iters.len());
        for (idx, iter) in iters.iter_mut().enumerate() {
            if let Some(processed) = iter.next() {
                heap.push(ProcessedHeapItem {
                    processed,
                    source_idx: idx,
                });
            }
        }
        Self { heap, iters }
    }
}

impl Iterator for ProcessedLineMerger {
    type Item = ProcessedLine;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.heap.pop()?;
        // Refill from the same source
        if let Some(next) = self.iters[item.source_idx].next() {
            self.heap.push(ProcessedHeapItem {
                processed: next,
                source_idx: item.source_idx,
            });
        }
        Some(item.processed)
    }
}

// ---------------------------------------------------------------------------
// Compiled rules
// ---------------------------------------------------------------------------

/// Pre-compiled regex data for a single LogRule.
pub struct CompiledRule {
    pub rule_id: u64,
    pub match_set: RegexSet,
    pub match_count: usize,
    pub match_mode: MatchMode,
    pub extraction_regexes: Vec<(usize, Regex)>, // (extraction_rule_index, compiled regex)
}

fn compile_rules(rules: &[LogRule]) -> Result<Vec<CompiledRule>, AnalysisError> {
    let mut compiled = Vec::with_capacity(rules.len());
    for rule in rules {
        let patterns: Vec<&str> = rule
            .match_rules
            .iter()
            .map(|m| m.pattern.as_str())
            .collect();
        let match_set =
            RegexSet::new(&patterns).map_err(|e| AnalysisError::InvalidRegex(e.to_string()))?;
        let match_count = rule.match_rules.len();

        let mut extraction_regexes = Vec::new();
        for (idx, ext) in rule.extraction_rules.iter().enumerate() {
            if let ExtractionType::Parsed = ext.extraction_type
                && let Some(pat) = &ext.pattern
            {
                let re = Regex::new(pat).map_err(|e| AnalysisError::InvalidRegex(e.to_string()))?;
                extraction_regexes.push((idx, re));
            }
        }

        compiled.push(CompiledRule {
            rule_id: rule.id,
            match_set,
            match_count,
            match_mode: rule.match_mode.clone(),
            extraction_regexes,
        });
    }
    Ok(compiled)
}

// ---------------------------------------------------------------------------
// Rule engine
// ---------------------------------------------------------------------------

/// Evaluates a single rule against a log line. Returns extracted state if matched.
pub fn evaluate_rule(
    rule: &LogRule,
    line: &LogLine,
    compiled: &CompiledRule,
) -> Option<HashMap<String, StateValue>> {
    let matches: Vec<usize> = compiled
        .match_set
        .matches(&line.content)
        .into_iter()
        .collect();

    let matched = match compiled.match_mode {
        MatchMode::Any => !matches.is_empty(),
        MatchMode::All => matches.len() == compiled.match_count,
    };

    if !matched {
        return None;
    }

    let mut extracted = HashMap::new();

    for ext in &rule.extraction_rules {
        match ext.extraction_type {
            ExtractionType::Static => {
                if let Some(val) = &ext.static_value {
                    extracted.insert(ext.state_key.clone(), StateValue::String(val.clone()));
                }
            }
            ExtractionType::Clear => {
                // Sentinel: we handle Clear during state application.
                // We don't insert anything here, the state manager will handle it.
            }
            ExtractionType::Parsed => {
                // Find the compiled regex for this extraction rule
                if let Some((_, re)) = compiled
                    .extraction_regexes
                    .iter()
                    .find(|(idx, _)| rule.extraction_rules[*idx].id == ext.id)
                    && let Some(caps) = re.captures(&line.content)
                    && let Some(m) = caps.name(&ext.state_key)
                {
                    let val_str = m.as_str();
                    // Try to parse as integer, then float, otherwise string
                    let value = if let Ok(i) = val_str.parse::<i64>() {
                        StateValue::Integer(i)
                    } else if let Ok(f) = val_str.parse::<f64>() {
                        StateValue::Float(f)
                    } else if val_str == "true" || val_str == "false" {
                        StateValue::Bool(val_str == "true")
                    } else {
                        StateValue::String(val_str.to_string())
                    };
                    extracted.insert(ext.state_key.clone(), value);
                }
            }
        }
    }

    Some(extracted)
}

/// Read all lines from a source (sequential I/O), then evaluate rules in parallel.
/// Returns a Vec of ProcessedLine in chronological order.
fn process_source(
    source: &Source,
    template: &SourceTemplate,
    ts_template: &TimestampTemplate,
    rule_ids: &[u64],
    rule_map: &HashMap<u64, &LogRule>,
    compiled_map: &HashMap<u64, &CompiledRule>,
) -> Result<Vec<ProcessedLine>, AnalysisError> {
    // Step 1: sequential I/O — read all lines
    let lines: Vec<LogLine> =
        LogLineIterator::new(source, template, ts_template)?.collect::<Result<Vec<_>, _>>()?;

    let is_json = template.json_timestamp_field.is_some();

    // Step 2: parallel rule evaluation (rayon)
    let processed: Vec<ProcessedLine> = lines
        .into_par_iter()
        .map(|mut line| {
            let mut rule_matches = Vec::new();
            for rule_id in rule_ids {
                if let (Some(rule), Some(compiled)) =
                    (rule_map.get(rule_id), compiled_map.get(rule_id))
                    && let Some(extracted) = evaluate_rule(rule, &line, compiled)
                {
                    rule_matches.push((*rule_id, extracted));
                }
            }
            let json_fields = if is_json {
                if let Some(serde_json::Value::Object(map)) = line.cached_json.take() {
                    let mut fields = HashMap::new();
                    for (key, value) in &map {
                        if let Some(sv) = json_value_to_state_value(value) {
                            fields.insert(key.clone(), sv);
                        }
                    }
                    Some(fields)
                } else {
                    None
                }
            } else {
                None
            };
            ProcessedLine {
                line,
                rule_matches,
                json_fields,
            }
        })
        .collect();

    Ok(processed)
}

// ---------------------------------------------------------------------------
// State manager
// ---------------------------------------------------------------------------

/// Manages per-source state.
pub struct StateManager {
    pub per_source_state: HashMap<u64, Arc<HashMap<String, TrackedValue>>>,
    pub source_names: HashMap<u64, String>,
    name_to_id: HashMap<String, u64>,
}

impl StateManager {
    pub fn new(sources: &[Source]) -> Self {
        let mut source_names = HashMap::new();
        let mut name_to_id = HashMap::new();
        for src in sources {
            source_names.insert(src.id, src.name.clone());
            name_to_id.insert(src.name.clone(), src.id);
        }
        Self {
            per_source_state: HashMap::new(),
            source_names,
            name_to_id,
        }
    }

    /// Apply extractions to a source's state, respecting extraction rules for mode/type.
    /// Returns a list of (key, old_value, new_value) for each actual change.
    pub fn apply_mutations(
        &mut self,
        source_id: u64,
        extractions: &HashMap<String, StateValue>,
        rules: &[ExtractionRule],
        timestamp: NaiveDateTime,
    ) -> Vec<(String, Option<StateValue>, Option<StateValue>)> {
        let state = Arc::make_mut(self.per_source_state.entry(source_id).or_default());
        let mut changes = Vec::new();

        for rule in rules {
            match rule.extraction_type {
                ExtractionType::Clear => {
                    let old = state.remove(&rule.state_key).map(|t| t.value);
                    if old.is_some() {
                        changes.push((rule.state_key.clone(), old, None));
                    }
                }
                ExtractionType::Static => {
                    if let Some(val) = &rule.static_value {
                        let new_val = StateValue::String(val.clone());
                        let old = state.get(&rule.state_key).map(|t| t.value.clone());
                        match rule.mode {
                            ExtractionMode::Replace => {
                                state.insert(
                                    rule.state_key.clone(),
                                    TrackedValue {
                                        value: new_val,
                                        set_at: timestamp,
                                    },
                                );
                            }
                            ExtractionMode::Accumulate => {
                                accumulate(state, &rule.state_key, new_val, timestamp);
                            }
                        }
                        let new = state.get(&rule.state_key).map(|t| t.value.clone());
                        if old != new {
                            changes.push((rule.state_key.clone(), old, new));
                        }
                    }
                }
                ExtractionType::Parsed => {
                    if let Some(val) = extractions.get(&rule.state_key) {
                        let old = state.get(&rule.state_key).map(|t| t.value.clone());
                        match rule.mode {
                            ExtractionMode::Replace => {
                                state.insert(
                                    rule.state_key.clone(),
                                    TrackedValue {
                                        value: val.clone(),
                                        set_at: timestamp,
                                    },
                                );
                            }
                            ExtractionMode::Accumulate => {
                                accumulate(state, &rule.state_key, val.clone(), timestamp);
                            }
                        }
                        let new = state.get(&rule.state_key).map(|t| t.value.clone());
                        if old != new {
                            changes.push((rule.state_key.clone(), old, new));
                        }
                    }
                }
            }
        }

        changes
    }

    /// Resolve the value of a source's state key by source name.
    pub fn get_state_by_name(&self, source_name: &str, key: &str) -> Option<&StateValue> {
        let id = self.name_to_id.get(source_name)?;
        self.per_source_state.get(id)?.get(key).map(|t| &t.value)
    }

    /// Snapshot all state, keyed by source name.
    pub fn snapshot(&self) -> HashMap<String, Arc<HashMap<String, TrackedValue>>> {
        let mut snap = HashMap::new();
        for (id, state) in &self.per_source_state {
            if let Some(name) = self.source_names.get(id) {
                snap.insert(name.clone(), Arc::clone(state));
            }
        }
        snap
    }
}

/// Accumulate a value into existing state.
fn accumulate(
    state: &mut HashMap<String, TrackedValue>,
    key: &str,
    new_val: StateValue,
    timestamp: NaiveDateTime,
) {
    if let Some(existing) = state.get(key) {
        let merged = match (&existing.value, &new_val) {
            (StateValue::String(a), StateValue::String(b)) => {
                StateValue::String(format!("{a},{b}"))
            }
            (StateValue::Integer(a), StateValue::Integer(b)) => StateValue::Integer(a + b),
            (StateValue::Float(a), StateValue::Float(b)) => StateValue::Float(a + b),
            (StateValue::Integer(a), StateValue::Float(b)) => StateValue::Float(*a as f64 + b),
            (StateValue::Float(a), StateValue::Integer(b)) => StateValue::Float(a + *b as f64),
            _ => new_val,
        };
        state.insert(
            key.to_string(),
            TrackedValue {
                value: merged,
                set_at: timestamp,
            },
        );
    } else {
        state.insert(
            key.to_string(),
            TrackedValue {
                value: new_val,
                set_at: timestamp,
            },
        );
    }
}

// ---------------------------------------------------------------------------
// Pattern evaluator
// ---------------------------------------------------------------------------

/// Evaluates ordered-predicate patterns against the current state.
pub struct PatternEvaluator {
    /// Current progress index per pattern (index into predicates).
    progress: Vec<usize>,
}

impl PatternEvaluator {
    pub fn new(patterns: &[Pattern]) -> Self {
        Self {
            progress: vec![0; patterns.len()],
        }
    }

    /// Evaluate all patterns against the current state. Returns any new matches.
    pub fn evaluate_patterns(
        &mut self,
        patterns: &[Pattern],
        state: &StateManager,
    ) -> Vec<PatternMatch> {
        let mut matches = Vec::new();

        for (i, pattern) in patterns.iter().enumerate() {
            if pattern.predicates.is_empty() {
                continue;
            }

            let progress = self.progress[i];

            // Check if the current predicate (at progress index) is satisfied
            let current_pred = &pattern.predicates[progress];
            if evaluate_predicate(current_pred, state) {
                // Verify all previous predicates still hold
                let mut all_previous_hold = true;
                for prev_idx in 0..progress {
                    if !evaluate_predicate(&pattern.predicates[prev_idx], state) {
                        all_previous_hold = false;
                        break;
                    }
                }

                if !all_previous_hold {
                    // Previous predicate no longer holds, reset progress
                    self.progress[i] = 0;
                } else {
                    // Advance progress
                    self.progress[i] = progress + 1;

                    // Check if all predicates are satisfied
                    if self.progress[i] == pattern.predicates.len() {
                        matches.push(PatternMatch {
                            pattern_id: pattern.id,
                            timestamp: chrono::Utc::now().naive_utc(),
                            state_snapshot: state.snapshot(),
                        });
                        // Reset for potential re-firing
                        self.progress[i] = 0;
                    }
                }
            }
        }

        matches
    }
}

/// Evaluate a single predicate against the current state.
fn evaluate_predicate(pred: &PatternPredicate, state: &StateManager) -> bool {
    let current_val = state.get_state_by_name(&pred.source_name, &pred.state_key);

    // Resolve the operand
    let operand_val: Option<StateValue> = match &pred.operand {
        Operand::Literal(v) => Some(v.clone()),
        Operand::StateRef {
            source_name,
            state_key,
        } => state.get_state_by_name(source_name, state_key).cloned(),
    };

    match pred.operator {
        Operator::Exists => current_val.is_some(),
        Operator::Eq => match (current_val, &operand_val) {
            (Some(a), Some(b)) => a == b,
            _ => false,
        },
        Operator::Neq => match (current_val, &operand_val) {
            (Some(a), Some(b)) => a != b,
            _ => false,
        },
        Operator::Gt => match (current_val, &operand_val) {
            (Some(a), Some(b)) => a.partial_cmp(b) == Some(Ordering::Greater),
            _ => false,
        },
        Operator::Lt => match (current_val, &operand_val) {
            (Some(a), Some(b)) => a.partial_cmp(b) == Some(Ordering::Less),
            _ => false,
        },
        Operator::Gte => match (current_val, &operand_val) {
            (Some(a), Some(b)) => {
                matches!(a.partial_cmp(b), Some(Ordering::Greater | Ordering::Equal))
            }
            _ => false,
        },
        Operator::Lte => match (current_val, &operand_val) {
            (Some(a), Some(b)) => {
                matches!(a.partial_cmp(b), Some(Ordering::Less | Ordering::Equal))
            }
            _ => false,
        },
        Operator::Contains => match (current_val, &operand_val) {
            (Some(StateValue::String(a)), Some(StateValue::String(b))) => a.contains(b.as_str()),
            _ => false,
        },
    }
}

// ---------------------------------------------------------------------------
// JSON field extraction helper
// ---------------------------------------------------------------------------

fn json_value_to_state_value(v: &serde_json::Value) -> Option<StateValue> {
    match v {
        serde_json::Value::String(s) => Some(StateValue::String(s.clone())),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Some(StateValue::Integer(i))
            } else {
                n.as_f64().map(StateValue::Float)
            }
        }
        serde_json::Value::Bool(b) => Some(StateValue::Bool(*b)),
        serde_json::Value::Null => None,
        other => Some(StateValue::String(other.to_string())),
    }
}

// ---------------------------------------------------------------------------
// Main analysis function
// ---------------------------------------------------------------------------

/// Run the full analysis pipeline.
pub fn analyze(
    sources: &[Source],
    templates: &[SourceTemplate],
    timestamp_templates: &[TimestampTemplate],
    rules: &[LogRule],
    rulesets: &[Ruleset],
    patterns: &[Pattern],
    time_range: &TimeRange,
) -> Result<AnalysisResult, AnalysisError> {
    // Build template lookup
    let template_map: HashMap<u64, &SourceTemplate> = templates.iter().map(|t| (t.id, t)).collect();

    // Build timestamp template lookup
    let ts_template_map: HashMap<u64, &TimestampTemplate> =
        timestamp_templates.iter().map(|t| (t.id, t)).collect();

    // Build rule lookup
    let rule_map: HashMap<u64, &LogRule> = rules.iter().map(|r| (r.id, r)).collect();

    // Compile all rules
    let compiled_rules = compile_rules(rules)?;
    let compiled_map: HashMap<u64, &CompiledRule> =
        compiled_rules.iter().map(|c| (c.rule_id, c)).collect();

    // Build ruleset-by-template lookup: template_id -> list of rule_ids
    let mut template_rule_ids: HashMap<u64, Vec<u64>> = HashMap::new();
    for rs in rulesets {
        template_rule_ids
            .entry(rs.template_id)
            .or_default()
            .extend(rs.rule_ids.iter());
    }

    // --- Phase 1: parallel per-source processing (rayon) ---
    let processed_sources: Vec<Vec<ProcessedLine>> = sources
        .par_iter()
        .map(|source| {
            let template = template_map.get(&source.template_id).ok_or_else(|| {
                AnalysisError::ParseError(format!(
                    "no template found for template_id {}",
                    source.template_id
                ))
            })?;
            let ts_template = ts_template_map
                .get(&template.timestamp_template_id)
                .ok_or_else(|| {
                    AnalysisError::ParseError(format!(
                        "no timestamp template found for timestamp_template_id {}",
                        template.timestamp_template_id
                    ))
                })?;
            let rule_ids = template_rule_ids
                .get(&source.template_id)
                .map(|v| v.as_slice())
                .unwrap_or(&[]);
            process_source(
                source,
                template,
                ts_template,
                rule_ids,
                &rule_map,
                &compiled_map,
            )
        })
        .collect::<Result<_, _>>()?;

    // --- Phase 2: sequential merge + state mutations + pattern evaluation ---
    let merger = ProcessedLineMerger::new(processed_sources);

    let mut state_manager = StateManager::new(sources);
    let mut pattern_eval = PatternEvaluator::new(patterns);

    let mut all_rule_matches = Vec::new();
    let mut all_pattern_matches = Vec::new();
    let mut all_state_changes = Vec::new();

    for processed in merger {
        let line = &processed.line;

        // Time-range filtering (stream is chronological)
        if let Some(start) = time_range.start
            && line.timestamp < start
        {
            continue;
        }
        if let Some(end) = time_range.end
            && line.timestamp > end
        {
            break;
        }

        // Apply pre-computed JSON fields as state
        if let Some(json_fields) = &processed.json_fields {
            let source_name = state_manager
                .source_names
                .get(&line.source_id)
                .cloned()
                .unwrap_or_default();
            let state = Arc::make_mut(
                state_manager
                    .per_source_state
                    .entry(line.source_id)
                    .or_default(),
            );
            for (key, sv) in json_fields {
                let old = state.get(key).map(|t| t.value.clone());
                let new = Some(sv.clone());
                state.insert(
                    key.clone(),
                    TrackedValue {
                        value: sv.clone(),
                        set_at: line.timestamp,
                    },
                );
                if old != new {
                    all_state_changes.push(StateChange {
                        timestamp: line.timestamp,
                        source_id: line.source_id,
                        source_name: source_name.clone(),
                        state_key: key.clone(),
                        old_value: old,
                        new_value: new,
                        rule_id: 0,
                    });
                }
            }
        }

        // Apply pre-computed rule matches
        let source_name = state_manager
            .source_names
            .get(&line.source_id)
            .cloned()
            .unwrap_or_default();

        for (rule_id, extracted) in &processed.rule_matches {
            if let Some(rule) = rule_map.get(rule_id) {
                let changes = state_manager.apply_mutations(
                    line.source_id,
                    extracted,
                    &rule.extraction_rules,
                    line.timestamp,
                );

                for (key, old, new) in changes {
                    all_state_changes.push(StateChange {
                        timestamp: line.timestamp,
                        source_id: line.source_id,
                        source_name: source_name.clone(),
                        state_key: key,
                        old_value: old,
                        new_value: new,
                        rule_id: *rule_id,
                    });
                }

                all_rule_matches.push(RuleMatch {
                    rule_id: *rule_id,
                    source_id: line.source_id,
                    log_line: line.clone(),
                    extracted_state: extracted.clone(),
                });
            }
        }

        // Evaluate patterns after each line
        let pmatches = pattern_eval.evaluate_patterns(patterns, &state_manager);
        for mut pm in pmatches {
            pm.timestamp = line.timestamp;
            all_pattern_matches.push(pm);
        }
    }

    Ok(AnalysisResult {
        rule_matches: all_rule_matches,
        pattern_matches: all_pattern_matches,
        state_changes: all_state_changes,
    })
}

/// Run the analysis pipeline, streaming events through a channel.
///
/// Mirrors `analyze()` but sends each match as it occurs rather than collecting.
/// Returns early if the receiver is dropped (client disconnected).
#[allow(clippy::too_many_arguments)]
pub fn analyze_streaming(
    sources: &[Source],
    templates: &[SourceTemplate],
    timestamp_templates: &[TimestampTemplate],
    rules: &[LogRule],
    rulesets: &[Ruleset],
    patterns: &[Pattern],
    tx: std::sync::mpsc::Sender<AnalysisEvent>,
    time_range: &TimeRange,
) -> Result<(), AnalysisError> {
    // Build template lookup
    let template_map: HashMap<u64, &SourceTemplate> = templates.iter().map(|t| (t.id, t)).collect();
    let ts_template_map: HashMap<u64, &TimestampTemplate> =
        timestamp_templates.iter().map(|t| (t.id, t)).collect();
    let rule_map: HashMap<u64, &LogRule> = rules.iter().map(|r| (r.id, r)).collect();

    let compiled_rules = compile_rules(rules)?;
    let compiled_map: HashMap<u64, &CompiledRule> =
        compiled_rules.iter().map(|c| (c.rule_id, c)).collect();

    let mut template_rule_ids: HashMap<u64, Vec<u64>> = HashMap::new();
    for rs in rulesets {
        template_rule_ids
            .entry(rs.template_id)
            .or_default()
            .extend(rs.rule_ids.iter());
    }

    // --- Phase 1: parallel per-source processing (rayon) ---
    let processed_sources: Vec<Vec<ProcessedLine>> = sources
        .par_iter()
        .map(|source| {
            let template = template_map.get(&source.template_id).ok_or_else(|| {
                AnalysisError::ParseError(format!(
                    "no template found for template_id {}",
                    source.template_id
                ))
            })?;
            let ts_template = ts_template_map
                .get(&template.timestamp_template_id)
                .ok_or_else(|| {
                    AnalysisError::ParseError(format!(
                        "no timestamp template found for timestamp_template_id {}",
                        template.timestamp_template_id
                    ))
                })?;
            let rule_ids = template_rule_ids
                .get(&source.template_id)
                .map(|v| v.as_slice())
                .unwrap_or(&[]);
            process_source(
                source,
                template,
                ts_template,
                rule_ids,
                &rule_map,
                &compiled_map,
            )
        })
        .collect::<Result<_, _>>()?;

    // --- Phase 2: sequential merge + state mutations + pattern evaluation ---
    let merger = ProcessedLineMerger::new(processed_sources);

    let mut state_manager = StateManager::new(sources);
    let mut pattern_eval = PatternEvaluator::new(patterns);

    let mut lines_processed: u64 = 0;
    let mut total_rule_matches: u64 = 0;
    let mut total_pattern_matches: u64 = 0;
    let mut total_state_changes: u64 = 0;

    for processed in merger {
        let line = &processed.line;

        // Time-range filtering (stream is chronological)
        if let Some(start) = time_range.start
            && line.timestamp < start
        {
            continue;
        }
        if let Some(end) = time_range.end
            && line.timestamp > end
        {
            break;
        }

        lines_processed += 1;

        // Apply pre-computed JSON fields as state
        if let Some(json_fields) = &processed.json_fields {
            let source_name = state_manager
                .source_names
                .get(&line.source_id)
                .cloned()
                .unwrap_or_default();
            let state = Arc::make_mut(
                state_manager
                    .per_source_state
                    .entry(line.source_id)
                    .or_default(),
            );
            for (key, sv) in json_fields {
                let old = state.get(key).map(|t| t.value.clone());
                let new = Some(sv.clone());
                state.insert(
                    key.clone(),
                    TrackedValue {
                        value: sv.clone(),
                        set_at: line.timestamp,
                    },
                );
                if old != new {
                    total_state_changes += 1;
                    if tx
                        .send(AnalysisEvent::StateChange(StateChange {
                            timestamp: line.timestamp,
                            source_id: line.source_id,
                            source_name: source_name.clone(),
                            state_key: key.clone(),
                            old_value: old,
                            new_value: new,
                            rule_id: 0,
                        }))
                        .is_err()
                    {
                        return Ok(());
                    }
                }
            }
        }

        // Apply pre-computed rule matches
        let source_name = state_manager
            .source_names
            .get(&line.source_id)
            .cloned()
            .unwrap_or_default();

        for (rule_id, extracted) in &processed.rule_matches {
            if let Some(rule) = rule_map.get(rule_id) {
                let changes = state_manager.apply_mutations(
                    line.source_id,
                    extracted,
                    &rule.extraction_rules,
                    line.timestamp,
                );

                for (key, old, new) in changes {
                    total_state_changes += 1;
                    if tx
                        .send(AnalysisEvent::StateChange(StateChange {
                            timestamp: line.timestamp,
                            source_id: line.source_id,
                            source_name: source_name.clone(),
                            state_key: key,
                            old_value: old,
                            new_value: new,
                            rule_id: *rule_id,
                        }))
                        .is_err()
                    {
                        return Ok(());
                    }
                }

                let rm = RuleMatch {
                    rule_id: *rule_id,
                    source_id: line.source_id,
                    log_line: line.clone(),
                    extracted_state: extracted.clone(),
                };
                total_rule_matches += 1;
                if tx.send(AnalysisEvent::RuleMatch(rm)).is_err() {
                    return Ok(()); // receiver dropped
                }
            }
        }

        let pmatches = pattern_eval.evaluate_patterns(patterns, &state_manager);
        for mut pm in pmatches {
            pm.timestamp = line.timestamp;
            total_pattern_matches += 1;
            if tx.send(AnalysisEvent::PatternMatch(pm)).is_err() {
                return Ok(());
            }
        }

        if lines_processed.is_multiple_of(500)
            && tx
                .send(AnalysisEvent::Progress { lines_processed })
                .is_err()
        {
            return Ok(());
        }
    }

    let _ = tx.send(AnalysisEvent::Complete {
        total_lines: lines_processed,
        total_rule_matches,
        total_pattern_matches,
        total_state_changes,
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// Log clustering (Drain-inspired tokenization)
// ---------------------------------------------------------------------------

use regex::Regex as Re;
use std::sync::LazyLock;

static VARIABLE_PATTERNS: LazyLock<Vec<Re>> = LazyLock::new(|| {
    vec![
        // UUID: 8-4-4-4-12 hex
        Re::new(r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$")
            .unwrap(),
        // ISO timestamp: 2024-01-15T10:30:00 (with optional fractional seconds and timezone)
        Re::new(r"^\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}").unwrap(),
        // IP:port or IP address
        Re::new(r"^\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}(:\d+)?$").unwrap(),
        // Hex string (8+ chars)
        Re::new(r"^0x[0-9a-fA-F]+$|^[0-9a-fA-F]{8,}$").unwrap(),
        // Unix path
        Re::new(r"^/[^\s]+/[^\s]+$").unwrap(),
        // Quoted string (double or single)
        Re::new(r#"^"[^"]*"$|^'[^']*'$"#).unwrap(),
        // Time-only format: HH:MM:SS with optional fractional seconds (e.g., 04:03:33, 17:41:44,747)
        Re::new(r"^\d{1,2}:\d{2}:\d{2}([,.]\d+)?$").unwrap(),
        // Decimal number (e.g., 3.14)
        Re::new(r"^\d+\.\d+$").unwrap(),
        // Plain integer
        Re::new(r"^\d+$").unwrap(),
        // Contains any digit → likely variable (Drain heuristic catch-all)
        Re::new(r"\d").unwrap(),
    ]
});

/// Tokenize a log line by replacing variable tokens with `<*>`.
fn tokenize(line: &str) -> String {
    line.split_whitespace()
        .map(|token| {
            for re in VARIABLE_PATTERNS.iter() {
                if re.is_match(token) {
                    return "<*>";
                }
            }
            token
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Cluster log lines by structural template.
pub fn cluster_logs(
    sources: &[Source],
    templates: &[SourceTemplate],
    timestamp_templates: &[TimestampTemplate],
    time_range: &TimeRange,
) -> Result<ClusterResult, AnalysisError> {
    let template_map: HashMap<u64, &SourceTemplate> = templates.iter().map(|t| (t.id, t)).collect();
    let ts_template_map: HashMap<u64, &TimestampTemplate> =
        timestamp_templates.iter().map(|t| (t.id, t)).collect();

    let mut iterators = Vec::new();
    for source in sources {
        let template = template_map.get(&source.template_id).ok_or_else(|| {
            AnalysisError::ParseError(format!(
                "no template found for template_id {}",
                source.template_id
            ))
        })?;
        let ts_template = ts_template_map
            .get(&template.timestamp_template_id)
            .ok_or_else(|| {
                AnalysisError::ParseError(format!(
                    "no timestamp template found for timestamp_template_id {}",
                    template.timestamp_template_id
                ))
            })?;
        iterators.push(LogLineIterator::new(source, template, ts_template)?);
    }

    let stream = MergedLogStream::new(iterators)?;

    struct ClusterEntry {
        count: u64,
        source_ids: std::collections::HashSet<u64>,
        sample_lines: Vec<String>,
    }

    let mut clusters: HashMap<String, ClusterEntry> = HashMap::new();
    let mut total_lines: u64 = 0;

    for result in stream {
        let line = result?;

        if let Some(start) = time_range.start
            && line.timestamp < start
        {
            continue;
        }
        if let Some(end) = time_range.end
            && line.timestamp > end
        {
            break;
        }

        total_lines += 1;
        let signature = tokenize(&line.content);
        let entry = clusters.entry(signature).or_insert_with(|| ClusterEntry {
            count: 0,
            source_ids: std::collections::HashSet::new(),
            sample_lines: Vec::new(),
        });
        entry.count += 1;
        entry.source_ids.insert(line.source_id);
        if entry.sample_lines.len() < 3 {
            entry.sample_lines.push(line.raw.to_string());
        }
    }

    let mut result_clusters: Vec<LogCluster> = clusters
        .into_iter()
        .filter(|(_, entry)| entry.count > 1)
        .map(|(template, entry)| {
            let mut source_ids: Vec<u64> = entry.source_ids.into_iter().collect();
            source_ids.sort();
            LogCluster {
                template,
                count: entry.count,
                source_ids,
                sample_lines: entry.sample_lines,
            }
        })
        .collect();
    result_clusters.sort_by(|a, b| b.count.cmp(&a.count));

    Ok(ClusterResult {
        clusters: result_clusters,
        total_lines,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    // Helper to build a simple compiled rule from a LogRule
    fn compile_one(rule: &LogRule) -> CompiledRule {
        compile_rules(std::slice::from_ref(rule)).unwrap().remove(0)
    }

    fn test_ts() -> NaiveDateTime {
        NaiveDateTime::parse_from_str("2024-01-01 00:00:00", "%Y-%m-%d %H:%M:%S").unwrap()
    }

    fn make_log_line(content: &str) -> LogLine {
        LogLine {
            timestamp: NaiveDateTime::parse_from_str("2024-01-01 00:00:00", "%Y-%m-%d %H:%M:%S")
                .unwrap(),
            source_id: 1,
            raw: Arc::from(content),
            content: Arc::from(content),
            cached_json: None,
        }
    }

    // -----------------------------------------------------------------------
    // Rule matching tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_single_match_rule_any() {
        let rule = LogRule {
            id: 1,
            name: "test".into(),
            match_mode: MatchMode::Any,
            match_rules: vec![MatchRule {
                id: 1,
                pattern: r"ERROR".into(),
            }],
            extraction_rules: vec![],
        };
        let compiled = compile_one(&rule);
        let line = make_log_line("2024-01-01 ERROR something broke");
        assert!(evaluate_rule(&rule, &line, &compiled).is_some());
    }

    #[test]
    fn test_single_match_rule_no_match() {
        let rule = LogRule {
            id: 1,
            name: "test".into(),
            match_mode: MatchMode::Any,
            match_rules: vec![MatchRule {
                id: 1,
                pattern: r"ERROR".into(),
            }],
            extraction_rules: vec![],
        };
        let compiled = compile_one(&rule);
        let line = make_log_line("2024-01-01 INFO all good");
        assert!(evaluate_rule(&rule, &line, &compiled).is_none());
    }

    #[test]
    fn test_multiple_match_rules_any_mode() {
        let rule = LogRule {
            id: 1,
            name: "test".into(),
            match_mode: MatchMode::Any,
            match_rules: vec![
                MatchRule {
                    id: 1,
                    pattern: r"ERROR".into(),
                },
                MatchRule {
                    id: 2,
                    pattern: r"WARN".into(),
                },
            ],
            extraction_rules: vec![],
        };
        let compiled = compile_one(&rule);

        // Matches ERROR
        let line = make_log_line("ERROR happened");
        assert!(evaluate_rule(&rule, &line, &compiled).is_some());

        // Matches WARN
        let line2 = make_log_line("WARN something");
        assert!(evaluate_rule(&rule, &line2, &compiled).is_some());

        // Matches neither
        let line3 = make_log_line("INFO ok");
        assert!(evaluate_rule(&rule, &line3, &compiled).is_none());
    }

    #[test]
    fn test_multiple_match_rules_all_mode() {
        let rule = LogRule {
            id: 1,
            name: "test".into(),
            match_mode: MatchMode::All,
            match_rules: vec![
                MatchRule {
                    id: 1,
                    pattern: r"server".into(),
                },
                MatchRule {
                    id: 2,
                    pattern: r"error".into(),
                },
            ],
            extraction_rules: vec![],
        };
        let compiled = compile_one(&rule);

        // Both match
        let line = make_log_line("server error occurred");
        assert!(evaluate_rule(&rule, &line, &compiled).is_some());

        // Only one matches
        let line2 = make_log_line("server started fine");
        assert!(evaluate_rule(&rule, &line2, &compiled).is_none());
    }

    // -----------------------------------------------------------------------
    // State mutation tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_replace_mode() {
        let sources = vec![Source {
            id: 1,
            name: "src1".into(),
            template_id: 1,
            file_path: "".into(),
        }];
        let mut sm = StateManager::new(&sources);
        Arc::make_mut(sm.per_source_state.entry(1).or_default()).insert(
            "key".into(),
            TrackedValue {
                value: StateValue::String("old".into()),
                set_at: test_ts(),
            },
        );

        let extractions: HashMap<String, StateValue> = HashMap::new();
        let rules = vec![ExtractionRule {
            id: 1,
            extraction_type: ExtractionType::Static,
            state_key: "key".into(),
            pattern: None,
            static_value: Some("new".into()),
            mode: ExtractionMode::Replace,
        }];
        sm.apply_mutations(1, &extractions, &rules, test_ts());

        assert_eq!(
            sm.per_source_state[&1]["key"].value,
            StateValue::String("new".into())
        );
    }

    #[test]
    fn test_accumulate_mode_strings() {
        let sources = vec![Source {
            id: 1,
            name: "src1".into(),
            template_id: 1,
            file_path: "".into(),
        }];
        let mut sm = StateManager::new(&sources);
        Arc::make_mut(sm.per_source_state.entry(1).or_default()).insert(
            "tags".into(),
            TrackedValue {
                value: StateValue::String("a".into()),
                set_at: test_ts(),
            },
        );

        let extractions: HashMap<String, StateValue> = HashMap::new();
        let rules = vec![ExtractionRule {
            id: 1,
            extraction_type: ExtractionType::Static,
            state_key: "tags".into(),
            pattern: None,
            static_value: Some("b".into()),
            mode: ExtractionMode::Accumulate,
        }];
        sm.apply_mutations(1, &extractions, &rules, test_ts());

        assert_eq!(
            sm.per_source_state[&1]["tags"].value,
            StateValue::String("a,b".into())
        );
    }

    #[test]
    fn test_accumulate_mode_integers() {
        let sources = vec![Source {
            id: 1,
            name: "src1".into(),
            template_id: 1,
            file_path: "".into(),
        }];
        let mut sm = StateManager::new(&sources);
        Arc::make_mut(sm.per_source_state.entry(1).or_default()).insert(
            "count".into(),
            TrackedValue {
                value: StateValue::Integer(10),
                set_at: test_ts(),
            },
        );

        let mut extractions: HashMap<String, StateValue> = HashMap::new();
        extractions.insert("count".into(), StateValue::Integer(5));

        let rules = vec![ExtractionRule {
            id: 1,
            extraction_type: ExtractionType::Parsed,
            state_key: "count".into(),
            pattern: Some(r"(?P<count>\d+)".into()),
            static_value: None,
            mode: ExtractionMode::Accumulate,
        }];
        sm.apply_mutations(1, &extractions, &rules, test_ts());

        assert_eq!(
            sm.per_source_state[&1]["count"].value,
            StateValue::Integer(15)
        );
    }

    #[test]
    fn test_clear_type() {
        let sources = vec![Source {
            id: 1,
            name: "src1".into(),
            template_id: 1,
            file_path: "".into(),
        }];
        let mut sm = StateManager::new(&sources);
        Arc::make_mut(sm.per_source_state.entry(1).or_default()).insert(
            "key".into(),
            TrackedValue {
                value: StateValue::String("val".into()),
                set_at: test_ts(),
            },
        );

        let extractions: HashMap<String, StateValue> = HashMap::new();
        let rules = vec![ExtractionRule {
            id: 1,
            extraction_type: ExtractionType::Clear,
            state_key: "key".into(),
            pattern: None,
            static_value: None,
            mode: ExtractionMode::Replace,
        }];
        sm.apply_mutations(1, &extractions, &rules, test_ts());

        assert!(!sm.per_source_state[&1].contains_key("key"));
    }

    #[test]
    fn test_parsed_extraction_with_capture_groups() {
        let rule = LogRule {
            id: 1,
            name: "parse_count".into(),
            match_mode: MatchMode::Any,
            match_rules: vec![MatchRule {
                id: 1,
                pattern: r"players: \d+".into(),
            }],
            extraction_rules: vec![ExtractionRule {
                id: 1,
                extraction_type: ExtractionType::Parsed,
                state_key: "player_count".into(),
                pattern: Some(r"players: (?P<player_count>\d+)".into()),
                static_value: None,
                mode: ExtractionMode::Replace,
            }],
        };
        let compiled = compile_one(&rule);
        let line = make_log_line("server players: 42 online");
        let extracted = evaluate_rule(&rule, &line, &compiled).unwrap();
        assert_eq!(extracted["player_count"], StateValue::Integer(42));
    }

    #[test]
    fn test_static_value_assignment() {
        let rule = LogRule {
            id: 1,
            name: "tag_error".into(),
            match_mode: MatchMode::Any,
            match_rules: vec![MatchRule {
                id: 1,
                pattern: r"ERROR".into(),
            }],
            extraction_rules: vec![ExtractionRule {
                id: 1,
                extraction_type: ExtractionType::Static,
                state_key: "status".into(),
                pattern: None,
                static_value: Some("error_detected".into()),
                mode: ExtractionMode::Replace,
            }],
        };
        let compiled = compile_one(&rule);
        let line = make_log_line("ERROR something");
        let extracted = evaluate_rule(&rule, &line, &compiled).unwrap();
        // Static extraction puts the value into the extracted map
        assert_eq!(
            extracted["status"],
            StateValue::String("error_detected".into())
        );
    }

    // -----------------------------------------------------------------------
    // Pattern evaluation tests
    // -----------------------------------------------------------------------

    fn make_sources() -> Vec<Source> {
        vec![
            Source {
                id: 1,
                name: "server".into(),
                template_id: 1,
                file_path: "".into(),
            },
            Source {
                id: 2,
                name: "client".into(),
                template_id: 1,
                file_path: "".into(),
            },
        ]
    }

    #[test]
    fn test_simple_two_predicate_pattern() {
        let sources = make_sources();
        let mut sm = StateManager::new(&sources);

        let pattern = Pattern {
            id: 1,
            name: "test_pattern".into(),
            predicates: vec![
                PatternPredicate {
                    source_name: "server".into(),
                    state_key: "status".into(),
                    operator: Operator::Eq,
                    operand: Operand::Literal(StateValue::String("running".into())),
                },
                PatternPredicate {
                    source_name: "server".into(),
                    state_key: "players".into(),
                    operator: Operator::Gt,
                    operand: Operand::Literal(StateValue::Integer(0)),
                },
            ],
        };
        let patterns = vec![pattern];
        let mut eval = PatternEvaluator::new(&patterns);

        // Pred 1 not yet satisfied
        let matches = eval.evaluate_patterns(&patterns, &sm);
        assert!(matches.is_empty());

        // Set status = running -> pred 1 satisfied
        Arc::make_mut(sm.per_source_state.entry(1).or_default()).insert(
            "status".into(),
            TrackedValue {
                value: StateValue::String("running".into()),
                set_at: test_ts(),
            },
        );
        let matches = eval.evaluate_patterns(&patterns, &sm);
        assert!(matches.is_empty()); // only 1 of 2 done

        // Set players = 5 -> pred 2 satisfied
        Arc::make_mut(sm.per_source_state.entry(1).or_default()).insert(
            "players".into(),
            TrackedValue {
                value: StateValue::Integer(5),
                set_at: test_ts(),
            },
        );
        let matches = eval.evaluate_patterns(&patterns, &sm);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].pattern_id, 1);
    }

    #[test]
    fn test_predicate_invalidation_resets_progress() {
        let sources = make_sources();
        let mut sm = StateManager::new(&sources);

        let pattern = Pattern {
            id: 1,
            name: "test".into(),
            predicates: vec![
                PatternPredicate {
                    source_name: "server".into(),
                    state_key: "status".into(),
                    operator: Operator::Eq,
                    operand: Operand::Literal(StateValue::String("running".into())),
                },
                PatternPredicate {
                    source_name: "server".into(),
                    state_key: "count".into(),
                    operator: Operator::Gt,
                    operand: Operand::Literal(StateValue::Integer(10)),
                },
            ],
        };
        let patterns = vec![pattern];
        let mut eval = PatternEvaluator::new(&patterns);

        // Satisfy pred 1
        Arc::make_mut(sm.per_source_state.entry(1).or_default()).insert(
            "status".into(),
            TrackedValue {
                value: StateValue::String("running".into()),
                set_at: test_ts(),
            },
        );
        eval.evaluate_patterns(&patterns, &sm);
        assert_eq!(eval.progress[0], 1);

        // Now invalidate pred 1 (change status away from "running") and try pred 2
        Arc::make_mut(sm.per_source_state.get_mut(&1).unwrap()).insert(
            "status".into(),
            TrackedValue {
                value: StateValue::String("stopped".into()),
                set_at: test_ts(),
            },
        );
        Arc::make_mut(sm.per_source_state.get_mut(&1).unwrap()).insert(
            "count".into(),
            TrackedValue {
                value: StateValue::Integer(20),
                set_at: test_ts(),
            },
        );
        let matches = eval.evaluate_patterns(&patterns, &sm);
        assert!(matches.is_empty());
        // Progress should be reset to 0
        assert_eq!(eval.progress[0], 0);
    }

    #[test]
    fn test_pattern_refire_after_match() {
        let sources = make_sources();
        let mut sm = StateManager::new(&sources);

        let pattern = Pattern {
            id: 1,
            name: "test".into(),
            predicates: vec![PatternPredicate {
                source_name: "server".into(),
                state_key: "flag".into(),
                operator: Operator::Eq,
                operand: Operand::Literal(StateValue::Bool(true)),
            }],
        };
        let patterns = vec![pattern];
        let mut eval = PatternEvaluator::new(&patterns);

        // Set flag=true -> should match
        Arc::make_mut(sm.per_source_state.entry(1).or_default()).insert(
            "flag".into(),
            TrackedValue {
                value: StateValue::Bool(true),
                set_at: test_ts(),
            },
        );
        let matches = eval.evaluate_patterns(&patterns, &sm);
        assert_eq!(matches.len(), 1);

        // Progress should be reset after match, so it can fire again
        assert_eq!(eval.progress[0], 0);

        // Should fire again immediately since flag is still true
        let matches = eval.evaluate_patterns(&patterns, &sm);
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn test_cross_source_state_reference() {
        let sources = make_sources();
        let mut sm = StateManager::new(&sources);

        let pattern = Pattern {
            id: 1,
            name: "cross_source".into(),
            predicates: vec![PatternPredicate {
                source_name: "server".into(),
                state_key: "region".into(),
                operator: Operator::Eq,
                operand: Operand::StateRef {
                    source_name: "client".into(),
                    state_key: "region".into(),
                },
            }],
        };
        let patterns = vec![pattern];
        let mut eval = PatternEvaluator::new(&patterns);

        // Different regions -> no match
        Arc::make_mut(sm.per_source_state.entry(1).or_default()).insert(
            "region".into(),
            TrackedValue {
                value: StateValue::String("us-east".into()),
                set_at: test_ts(),
            },
        );
        Arc::make_mut(sm.per_source_state.entry(2).or_default()).insert(
            "region".into(),
            TrackedValue {
                value: StateValue::String("eu-west".into()),
                set_at: test_ts(),
            },
        );
        let matches = eval.evaluate_patterns(&patterns, &sm);
        assert!(matches.is_empty());

        // Same regions -> match
        Arc::make_mut(sm.per_source_state.get_mut(&2).unwrap()).insert(
            "region".into(),
            TrackedValue {
                value: StateValue::String("us-east".into()),
                set_at: test_ts(),
            },
        );
        let matches = eval.evaluate_patterns(&patterns, &sm);
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn test_all_operators() {
        let sources = make_sources();
        let mut sm = StateManager::new(&sources);
        Arc::make_mut(sm.per_source_state.entry(1).or_default()).insert(
            "val".into(),
            TrackedValue {
                value: StateValue::Integer(10),
                set_at: test_ts(),
            },
        );
        Arc::make_mut(sm.per_source_state.entry(1).or_default()).insert(
            "name".into(),
            TrackedValue {
                value: StateValue::String("hello world".into()),
                set_at: test_ts(),
            },
        );

        // Eq
        assert!(evaluate_predicate(
            &PatternPredicate {
                source_name: "server".into(),
                state_key: "val".into(),
                operator: Operator::Eq,
                operand: Operand::Literal(StateValue::Integer(10)),
            },
            &sm,
        ));

        // Neq
        assert!(evaluate_predicate(
            &PatternPredicate {
                source_name: "server".into(),
                state_key: "val".into(),
                operator: Operator::Neq,
                operand: Operand::Literal(StateValue::Integer(5)),
            },
            &sm,
        ));

        // Gt
        assert!(evaluate_predicate(
            &PatternPredicate {
                source_name: "server".into(),
                state_key: "val".into(),
                operator: Operator::Gt,
                operand: Operand::Literal(StateValue::Integer(5)),
            },
            &sm,
        ));

        // Lt
        assert!(evaluate_predicate(
            &PatternPredicate {
                source_name: "server".into(),
                state_key: "val".into(),
                operator: Operator::Lt,
                operand: Operand::Literal(StateValue::Integer(20)),
            },
            &sm,
        ));

        // Gte (equal case)
        assert!(evaluate_predicate(
            &PatternPredicate {
                source_name: "server".into(),
                state_key: "val".into(),
                operator: Operator::Gte,
                operand: Operand::Literal(StateValue::Integer(10)),
            },
            &sm,
        ));

        // Lte (equal case)
        assert!(evaluate_predicate(
            &PatternPredicate {
                source_name: "server".into(),
                state_key: "val".into(),
                operator: Operator::Lte,
                operand: Operand::Literal(StateValue::Integer(10)),
            },
            &sm,
        ));

        // Contains
        assert!(evaluate_predicate(
            &PatternPredicate {
                source_name: "server".into(),
                state_key: "name".into(),
                operator: Operator::Contains,
                operand: Operand::Literal(StateValue::String("world".into())),
            },
            &sm,
        ));

        // Exists
        assert!(evaluate_predicate(
            &PatternPredicate {
                source_name: "server".into(),
                state_key: "val".into(),
                operator: Operator::Exists,
                operand: Operand::Literal(StateValue::Bool(false)),
            },
            &sm,
        ));

        // Exists - false case
        assert!(!evaluate_predicate(
            &PatternPredicate {
                source_name: "server".into(),
                state_key: "nonexistent".into(),
                operator: Operator::Exists,
                operand: Operand::Literal(StateValue::Bool(false)),
            },
            &sm,
        ));
    }

    // -----------------------------------------------------------------------
    // K-way merge test
    // -----------------------------------------------------------------------

    fn make_ts_template() -> TimestampTemplate {
        TimestampTemplate {
            id: 1,
            name: "default".into(),
            format: "%Y-%m-%d %H:%M:%S".into(),
            extraction_regex: None,
            default_year: None,
        }
    }

    fn make_template() -> SourceTemplate {
        SourceTemplate {
            id: 1,
            name: "test".into(),
            timestamp_template_id: 1,
            line_delimiter: "\n".into(),
            content_regex: None,
            continuation_regex: None,
            json_timestamp_field: None,
        }
    }

    #[test]
    fn test_kway_merge_three_sources() {
        // Create 3 temp files with interleaved timestamps
        let mut f1 = NamedTempFile::new().unwrap();
        let mut f2 = NamedTempFile::new().unwrap();
        let mut f3 = NamedTempFile::new().unwrap();

        writeln!(f1, "2024-01-01 00:00:01 source1 line1").unwrap();
        writeln!(f1, "2024-01-01 00:00:04 source1 line2").unwrap();
        writeln!(f1, "2024-01-01 00:00:07 source1 line3").unwrap();

        writeln!(f2, "2024-01-01 00:00:02 source2 line1").unwrap();
        writeln!(f2, "2024-01-01 00:00:05 source2 line2").unwrap();

        writeln!(f3, "2024-01-01 00:00:03 source3 line1").unwrap();
        writeln!(f3, "2024-01-01 00:00:06 source3 line2").unwrap();

        let template = make_template();
        let ts_template = make_ts_template();

        let sources = [
            Source {
                id: 1,
                name: "s1".into(),
                template_id: 1,
                file_path: f1.path().to_str().unwrap().into(),
            },
            Source {
                id: 2,
                name: "s2".into(),
                template_id: 1,
                file_path: f2.path().to_str().unwrap().into(),
            },
            Source {
                id: 3,
                name: "s3".into(),
                template_id: 1,
                file_path: f3.path().to_str().unwrap().into(),
            },
        ];

        let iters: Vec<LogLineIterator> = sources
            .iter()
            .map(|s| LogLineIterator::new(s, &template, &ts_template).unwrap())
            .collect();

        let stream = MergedLogStream::new(iters).unwrap();
        let lines: Vec<LogLine> = stream.map(|r| r.unwrap()).collect();

        assert_eq!(lines.len(), 7);
        // Verify chronological order
        for i in 1..lines.len() {
            assert!(lines[i].timestamp >= lines[i - 1].timestamp);
        }
        // Verify interleaving
        let source_ids: Vec<u64> = lines.iter().map(|l| l.source_id).collect();
        assert_eq!(source_ids, vec![1, 2, 3, 1, 2, 3, 1]);
    }

    // -----------------------------------------------------------------------
    // Integration test: server + client log scenario
    // -----------------------------------------------------------------------

    #[test]
    fn test_integration_server_client_scenario() {
        // Create temp log files
        let mut server_log = NamedTempFile::new().unwrap();
        let mut client_log = NamedTempFile::new().unwrap();

        // Server log: has player count and region info
        writeln!(
            server_log,
            "2024-01-01 00:00:01 [INFO] Server started in region us-east"
        )
        .unwrap();
        writeln!(server_log, "2024-01-01 00:00:03 [INFO] Players online: 42").unwrap();
        writeln!(server_log, "2024-01-01 00:00:05 [INFO] Players online: 100").unwrap();

        // Client log: has region info
        writeln!(
            client_log,
            "2024-01-01 00:00:02 [INFO] Client connecting to region us-east"
        )
        .unwrap();
        writeln!(
            client_log,
            "2024-01-01 00:00:04 [INFO] Client connected, status active"
        )
        .unwrap();

        let ts_template = make_ts_template();

        let template = SourceTemplate {
            id: 1,
            name: "default".into(),
            timestamp_template_id: 1,
            line_delimiter: "\n".into(),
            content_regex: Some(r"^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2} (.+)$".into()),
            continuation_regex: None,
            json_timestamp_field: None,
        };

        let sources = vec![
            Source {
                id: 1,
                name: "server".into(),
                template_id: 1,
                file_path: server_log.path().to_str().unwrap().into(),
            },
            Source {
                id: 2,
                name: "client".into(),
                template_id: 1,
                file_path: client_log.path().to_str().unwrap().into(),
            },
        ];

        // Rule: extract region from server
        let server_region_rule = LogRule {
            id: 1,
            name: "server_region".into(),
            match_mode: MatchMode::Any,
            match_rules: vec![MatchRule {
                id: 1,
                pattern: r"region \w+".into(),
            }],
            extraction_rules: vec![ExtractionRule {
                id: 1,
                extraction_type: ExtractionType::Parsed,
                state_key: "region".into(),
                pattern: Some(r"region (?P<region>\S+)".into()),
                static_value: None,
                mode: ExtractionMode::Replace,
            }],
        };

        // Rule: extract player count
        let player_count_rule = LogRule {
            id: 2,
            name: "player_count".into(),
            match_mode: MatchMode::Any,
            match_rules: vec![MatchRule {
                id: 2,
                pattern: r"Players online: \d+".into(),
            }],
            extraction_rules: vec![ExtractionRule {
                id: 2,
                extraction_type: ExtractionType::Parsed,
                state_key: "player_count".into(),
                pattern: Some(r"Players online: (?P<player_count>\d+)".into()),
                static_value: None,
                mode: ExtractionMode::Replace,
            }],
        };

        // Rule: extract client region
        let client_region_rule = LogRule {
            id: 3,
            name: "client_region".into(),
            match_mode: MatchMode::Any,
            match_rules: vec![MatchRule {
                id: 3,
                pattern: r"connecting to region".into(),
            }],
            extraction_rules: vec![ExtractionRule {
                id: 3,
                extraction_type: ExtractionType::Parsed,
                state_key: "region".into(),
                pattern: Some(r"region (?P<region>\S+)".into()),
                static_value: None,
                mode: ExtractionMode::Replace,
            }],
        };

        let rules = vec![server_region_rule, player_count_rule, client_region_rule];

        let rulesets = vec![Ruleset {
            id: 1,
            name: "server_rules".into(),
            template_id: 1,
            rule_ids: vec![1, 2, 3],
        }];

        // Pattern: detect when server and client are in same region AND player count > 50
        let pattern = Pattern {
            id: 1,
            name: "cross_source_detect".into(),
            predicates: vec![
                PatternPredicate {
                    source_name: "server".into(),
                    state_key: "region".into(),
                    operator: Operator::Eq,
                    operand: Operand::StateRef {
                        source_name: "client".into(),
                        state_key: "region".into(),
                    },
                },
                PatternPredicate {
                    source_name: "server".into(),
                    state_key: "player_count".into(),
                    operator: Operator::Gt,
                    operand: Operand::Literal(StateValue::Integer(50)),
                },
            ],
        };

        let result = analyze(
            &sources,
            &[template],
            &[ts_template],
            &rules,
            &rulesets,
            &[pattern],
            &TimeRange::default(),
        )
        .unwrap();

        // Should have rule matches for region and player count lines
        assert!(
            result.rule_matches.len() >= 3,
            "expected at least 3 rule matches, got {}",
            result.rule_matches.len()
        );

        // Pattern should match when player_count = 100 > 50 and regions match
        assert_eq!(
            result.pattern_matches.len(),
            1,
            "expected 1 pattern match, got {}",
            result.pattern_matches.len()
        );
        assert_eq!(result.pattern_matches[0].pattern_id, 1);

        // Verify state snapshot has both sources
        let snap = &result.pattern_matches[0].state_snapshot;
        assert!(snap.contains_key("server"));
        assert!(snap.contains_key("client"));
        assert_eq!(
            snap["server"]["region"].value,
            StateValue::String("us-east".into())
        );
    }

    // -----------------------------------------------------------------------
    // Streaming analysis test
    // -----------------------------------------------------------------------

    #[test]
    fn test_analyze_streaming_sends_events() {
        // Reuse the same setup as the integration test
        let mut server_log = NamedTempFile::new().unwrap();
        let mut client_log = NamedTempFile::new().unwrap();

        writeln!(
            server_log,
            "2024-01-01 00:00:01 [INFO] Server started in region us-east"
        )
        .unwrap();
        writeln!(server_log, "2024-01-01 00:00:03 [INFO] Players online: 42").unwrap();
        writeln!(server_log, "2024-01-01 00:00:05 [INFO] Players online: 100").unwrap();

        writeln!(
            client_log,
            "2024-01-01 00:00:02 [INFO] Client connecting to region us-east"
        )
        .unwrap();
        writeln!(
            client_log,
            "2024-01-01 00:00:04 [INFO] Client connected, status active"
        )
        .unwrap();

        let ts_template = make_ts_template();
        let template = SourceTemplate {
            id: 1,
            name: "default".into(),
            timestamp_template_id: 1,
            line_delimiter: "\n".into(),
            content_regex: Some(r"^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2} (.+)$".into()),
            continuation_regex: None,
            json_timestamp_field: None,
        };

        let sources = vec![
            Source {
                id: 1,
                name: "server".into(),
                template_id: 1,
                file_path: server_log.path().to_str().unwrap().into(),
            },
            Source {
                id: 2,
                name: "client".into(),
                template_id: 1,
                file_path: client_log.path().to_str().unwrap().into(),
            },
        ];

        let rules = vec![
            LogRule {
                id: 1,
                name: "server_region".into(),
                match_mode: MatchMode::Any,
                match_rules: vec![MatchRule {
                    id: 1,
                    pattern: r"region \w+".into(),
                }],
                extraction_rules: vec![ExtractionRule {
                    id: 1,
                    extraction_type: ExtractionType::Parsed,
                    state_key: "region".into(),
                    pattern: Some(r"region (?P<region>\S+)".into()),
                    static_value: None,
                    mode: ExtractionMode::Replace,
                }],
            },
            LogRule {
                id: 2,
                name: "player_count".into(),
                match_mode: MatchMode::Any,
                match_rules: vec![MatchRule {
                    id: 2,
                    pattern: r"Players online: \d+".into(),
                }],
                extraction_rules: vec![ExtractionRule {
                    id: 2,
                    extraction_type: ExtractionType::Parsed,
                    state_key: "player_count".into(),
                    pattern: Some(r"Players online: (?P<player_count>\d+)".into()),
                    static_value: None,
                    mode: ExtractionMode::Replace,
                }],
            },
            LogRule {
                id: 3,
                name: "client_region".into(),
                match_mode: MatchMode::Any,
                match_rules: vec![MatchRule {
                    id: 3,
                    pattern: r"connecting to region".into(),
                }],
                extraction_rules: vec![ExtractionRule {
                    id: 3,
                    extraction_type: ExtractionType::Parsed,
                    state_key: "region".into(),
                    pattern: Some(r"region (?P<region>\S+)".into()),
                    static_value: None,
                    mode: ExtractionMode::Replace,
                }],
            },
        ];

        let rulesets = vec![Ruleset {
            id: 1,
            name: "server_rules".into(),
            template_id: 1,
            rule_ids: vec![1, 2, 3],
        }];

        let pattern = Pattern {
            id: 1,
            name: "cross_source_detect".into(),
            predicates: vec![
                PatternPredicate {
                    source_name: "server".into(),
                    state_key: "region".into(),
                    operator: Operator::Eq,
                    operand: Operand::StateRef {
                        source_name: "client".into(),
                        state_key: "region".into(),
                    },
                },
                PatternPredicate {
                    source_name: "server".into(),
                    state_key: "player_count".into(),
                    operator: Operator::Gt,
                    operand: Operand::Literal(StateValue::Integer(50)),
                },
            ],
        };

        // Run streaming analysis
        let (tx, rx) = std::sync::mpsc::channel();
        analyze_streaming(
            &sources,
            std::slice::from_ref(&template),
            std::slice::from_ref(&ts_template),
            &rules,
            &rulesets,
            std::slice::from_ref(&pattern),
            tx,
            &TimeRange::default(),
        )
        .unwrap();

        // Also run synchronous analysis for comparison
        let sync_result = analyze(
            &sources,
            std::slice::from_ref(&template),
            std::slice::from_ref(&ts_template),
            &rules,
            &rulesets,
            std::slice::from_ref(&pattern),
            &TimeRange::default(),
        )
        .unwrap();

        // Collect streaming events
        let events: Vec<AnalysisEvent> = rx.iter().collect();

        let stream_rule_matches: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, AnalysisEvent::RuleMatch(_)))
            .collect();
        let stream_pattern_matches: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, AnalysisEvent::PatternMatch(_)))
            .collect();
        let complete_events: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, AnalysisEvent::Complete { .. }))
            .collect();

        // Same number of matches as synchronous
        assert_eq!(stream_rule_matches.len(), sync_result.rule_matches.len());
        assert_eq!(
            stream_pattern_matches.len(),
            sync_result.pattern_matches.len()
        );

        // Exactly one Complete event
        assert_eq!(complete_events.len(), 1);
        if let AnalysisEvent::Complete {
            total_lines,
            total_rule_matches,
            total_pattern_matches,
            total_state_changes,
        } = &complete_events[0]
        {
            assert_eq!(*total_lines, 5);
            assert_eq!(*total_rule_matches, sync_result.rule_matches.len() as u64);
            assert_eq!(
                *total_pattern_matches,
                sync_result.pattern_matches.len() as u64
            );
            assert_eq!(*total_state_changes, sync_result.state_changes.len() as u64);
        } else {
            panic!("expected Complete event");
        }
    }

    // -----------------------------------------------------------------------
    // State change tracking tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_apply_mutations_returns_changes_on_replace() {
        let sources = vec![Source {
            id: 1,
            name: "src1".into(),
            template_id: 1,
            file_path: "".into(),
        }];
        let mut sm = StateManager::new(&sources);
        Arc::make_mut(sm.per_source_state.entry(1).or_default()).insert(
            "key".into(),
            TrackedValue {
                value: StateValue::String("old".into()),
                set_at: test_ts(),
            },
        );

        let extractions: HashMap<String, StateValue> = HashMap::new();
        let rules = vec![ExtractionRule {
            id: 1,
            extraction_type: ExtractionType::Static,
            state_key: "key".into(),
            pattern: None,
            static_value: Some("new".into()),
            mode: ExtractionMode::Replace,
        }];
        let changes = sm.apply_mutations(1, &extractions, &rules, test_ts());

        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].0, "key");
        assert_eq!(changes[0].1, Some(StateValue::String("old".into())));
        assert_eq!(changes[0].2, Some(StateValue::String("new".into())));
    }

    #[test]
    fn test_apply_mutations_returns_changes_on_clear() {
        let sources = vec![Source {
            id: 1,
            name: "src1".into(),
            template_id: 1,
            file_path: "".into(),
        }];
        let mut sm = StateManager::new(&sources);
        Arc::make_mut(sm.per_source_state.entry(1).or_default()).insert(
            "key".into(),
            TrackedValue {
                value: StateValue::String("val".into()),
                set_at: test_ts(),
            },
        );

        let extractions: HashMap<String, StateValue> = HashMap::new();
        let rules = vec![ExtractionRule {
            id: 1,
            extraction_type: ExtractionType::Clear,
            state_key: "key".into(),
            pattern: None,
            static_value: None,
            mode: ExtractionMode::Replace,
        }];
        let changes = sm.apply_mutations(1, &extractions, &rules, test_ts());

        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].0, "key");
        assert_eq!(changes[0].1, Some(StateValue::String("val".into())));
        assert_eq!(changes[0].2, None);
    }

    #[test]
    fn test_apply_mutations_returns_changes_on_first_set() {
        let sources = vec![Source {
            id: 1,
            name: "src1".into(),
            template_id: 1,
            file_path: "".into(),
        }];
        let mut sm = StateManager::new(&sources);

        let extractions: HashMap<String, StateValue> = HashMap::new();
        let rules = vec![ExtractionRule {
            id: 1,
            extraction_type: ExtractionType::Static,
            state_key: "key".into(),
            pattern: None,
            static_value: Some("val".into()),
            mode: ExtractionMode::Replace,
        }];
        let changes = sm.apply_mutations(1, &extractions, &rules, test_ts());

        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].0, "key");
        assert_eq!(changes[0].1, None);
        assert_eq!(changes[0].2, Some(StateValue::String("val".into())));
    }

    #[test]
    fn test_apply_mutations_skips_noop() {
        let sources = vec![Source {
            id: 1,
            name: "src1".into(),
            template_id: 1,
            file_path: "".into(),
        }];
        let mut sm = StateManager::new(&sources);
        Arc::make_mut(sm.per_source_state.entry(1).or_default()).insert(
            "key".into(),
            TrackedValue {
                value: StateValue::String("same".into()),
                set_at: test_ts(),
            },
        );

        let extractions: HashMap<String, StateValue> = HashMap::new();
        let rules = vec![ExtractionRule {
            id: 1,
            extraction_type: ExtractionType::Static,
            state_key: "key".into(),
            pattern: None,
            static_value: Some("same".into()),
            mode: ExtractionMode::Replace,
        }];
        let changes = sm.apply_mutations(1, &extractions, &rules, test_ts());

        assert!(changes.is_empty());
    }

    #[test]
    fn test_streaming_emits_state_change_events() {
        let mut server_log = NamedTempFile::new().unwrap();
        writeln!(
            server_log,
            "2024-01-01 00:00:01 [INFO] Server started in region us-east"
        )
        .unwrap();
        writeln!(server_log, "2024-01-01 00:00:03 [INFO] Players online: 42").unwrap();

        let ts_template = make_ts_template();
        let template = SourceTemplate {
            id: 1,
            name: "default".into(),
            timestamp_template_id: 1,
            line_delimiter: "\n".into(),
            content_regex: Some(r"^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2} (.+)$".into()),
            continuation_regex: None,
            json_timestamp_field: None,
        };

        let sources = vec![Source {
            id: 1,
            name: "server".into(),
            template_id: 1,
            file_path: server_log.path().to_str().unwrap().into(),
        }];

        let rules = vec![
            LogRule {
                id: 1,
                name: "server_region".into(),
                match_mode: MatchMode::Any,
                match_rules: vec![MatchRule {
                    id: 1,
                    pattern: r"region \w+".into(),
                }],
                extraction_rules: vec![ExtractionRule {
                    id: 1,
                    extraction_type: ExtractionType::Parsed,
                    state_key: "region".into(),
                    pattern: Some(r"region (?P<region>\S+)".into()),
                    static_value: None,
                    mode: ExtractionMode::Replace,
                }],
            },
            LogRule {
                id: 2,
                name: "player_count".into(),
                match_mode: MatchMode::Any,
                match_rules: vec![MatchRule {
                    id: 2,
                    pattern: r"Players online: \d+".into(),
                }],
                extraction_rules: vec![ExtractionRule {
                    id: 2,
                    extraction_type: ExtractionType::Parsed,
                    state_key: "player_count".into(),
                    pattern: Some(r"Players online: (?P<player_count>\d+)".into()),
                    static_value: None,
                    mode: ExtractionMode::Replace,
                }],
            },
        ];

        let rulesets = vec![Ruleset {
            id: 1,
            name: "server_rules".into(),
            template_id: 1,
            rule_ids: vec![1, 2],
        }];

        let (tx, rx) = std::sync::mpsc::channel();
        analyze_streaming(
            &sources,
            std::slice::from_ref(&template),
            std::slice::from_ref(&ts_template),
            &rules,
            &rulesets,
            &[],
            tx,
            &TimeRange::default(),
        )
        .unwrap();

        let events: Vec<AnalysisEvent> = rx.iter().collect();

        let state_changes: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, AnalysisEvent::StateChange(_)))
            .collect();

        // region set + player_count set = 2 state changes
        assert_eq!(state_changes.len(), 2);

        // Verify first state change is region
        if let AnalysisEvent::StateChange(sc) = &state_changes[0] {
            assert_eq!(sc.state_key, "region");
            assert_eq!(sc.source_name, "server");
            assert!(sc.old_value.is_none());
            assert_eq!(sc.new_value, Some(StateValue::String("us-east".into())));
        }
    }

    // -----------------------------------------------------------------------
    // Multi-line continuation tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_multiline_continuation() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "2024-01-15 10:00:01 INFO Server started").unwrap();
        writeln!(f, "2024-01-15 10:00:05 ERROR NullPointerException").unwrap();
        writeln!(f, "  at com.example.Handler.process(Handler.java:42)").unwrap();
        writeln!(f, "  at com.example.Server.handle(Server.java:128)").unwrap();
        writeln!(f, "2024-01-15 10:00:06 WARN Pool low: 3 remaining").unwrap();

        let ts_template = make_ts_template();
        let template = SourceTemplate {
            id: 1,
            name: "test".into(),
            timestamp_template_id: 1,
            line_delimiter: "\n".into(),
            content_regex: None,
            continuation_regex: Some(r"^\s".to_string()),
            json_timestamp_field: None,
        };
        let source = Source {
            id: 1,
            name: "test".into(),
            template_id: 1,
            file_path: f.path().to_str().unwrap().into(),
        };

        let iter = LogLineIterator::new(&source, &template, &ts_template).unwrap();
        let lines: Vec<LogLine> = iter.map(|r| r.unwrap()).collect();

        // Should be 3 logical entries, not 5 physical lines
        assert_eq!(
            lines.len(),
            3,
            "expected 3 logical entries, got {}",
            lines.len()
        );

        // First entry: single line
        assert_eq!(&*lines[0].raw, "2024-01-15 10:00:01 INFO Server started");
        assert!(!lines[0].raw.contains('\n'));

        // Second entry: merged with continuation lines
        assert!(
            lines[1].raw.contains('\n'),
            "multi-line entry should contain newlines"
        );
        assert!(lines[1].raw.contains("NullPointerException"));
        assert!(lines[1].raw.contains("at com.example.Handler.process"));
        assert!(lines[1].raw.contains("at com.example.Server.handle"));

        // Third entry: single line
        assert_eq!(
            &*lines[2].raw,
            "2024-01-15 10:00:06 WARN Pool low: 3 remaining"
        );

        // Timestamps should be from head lines only
        assert_eq!(
            lines[0].timestamp,
            NaiveDateTime::parse_from_str("2024-01-15 10:00:01", "%Y-%m-%d %H:%M:%S").unwrap()
        );
        assert_eq!(
            lines[1].timestamp,
            NaiveDateTime::parse_from_str("2024-01-15 10:00:05", "%Y-%m-%d %H:%M:%S").unwrap()
        );
        assert_eq!(
            lines[2].timestamp,
            NaiveDateTime::parse_from_str("2024-01-15 10:00:06", "%Y-%m-%d %H:%M:%S").unwrap()
        );
    }

    #[test]
    fn test_multiline_continuation_none_is_passthrough() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "2024-01-15 10:00:01 INFO Server started").unwrap();
        writeln!(f, "2024-01-15 10:00:05 ERROR NullPointerException").unwrap();
        writeln!(f, "  at com.example.Handler.process(Handler.java:42)").unwrap();

        let ts_template = make_ts_template();
        // No continuation_regex — should treat each line independently
        let template = make_template();
        let source = Source {
            id: 1,
            name: "test".into(),
            template_id: 1,
            file_path: f.path().to_str().unwrap().into(),
        };

        let iter = LogLineIterator::new(&source, &template, &ts_template).unwrap();
        let results: Vec<_> = iter.collect();

        // First two lines parse fine; third line ("  at ...") will fail timestamp parsing
        assert_eq!(
            results.len(),
            3,
            "without continuation_regex, each physical line is separate"
        );
        assert!(results[0].is_ok());
        assert!(results[1].is_ok());
        // The continuation line will fail timestamp parsing (expected behavior)
        assert!(results[2].is_err());
    }

    // -----------------------------------------------------------------------
    // JSON Lines tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_json_line_parsing() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, r#"{{"timestamp":"2024-01-15 10:00:01","level":"INFO","message":"Server started","port":8080}}"#).unwrap();
        writeln!(f, r#"{{"timestamp":"2024-01-15 10:00:02","level":"ERROR","message":"Connection failed","retries":3}}"#).unwrap();
        writeln!(f, r#"{{"timestamp":"2024-01-15 10:00:03","level":"WARN","message":"High memory","usage_pct":85.5}}"#).unwrap();

        let ts_template = make_ts_template();
        let template = SourceTemplate {
            id: 1,
            name: "json_test".into(),
            timestamp_template_id: 1,
            line_delimiter: "\n".into(),
            content_regex: None,
            continuation_regex: None,
            json_timestamp_field: Some("timestamp".into()),
        };
        let source = Source {
            id: 1,
            name: "test".into(),
            template_id: 1,
            file_path: f.path().to_str().unwrap().into(),
        };

        let iter = LogLineIterator::new(&source, &template, &ts_template).unwrap();
        let lines: Vec<LogLine> = iter.map(|r| r.unwrap()).collect();

        assert_eq!(lines.len(), 3);
        assert_eq!(
            lines[0].timestamp,
            NaiveDateTime::parse_from_str("2024-01-15 10:00:01", "%Y-%m-%d %H:%M:%S").unwrap()
        );
        assert_eq!(
            lines[1].timestamp,
            NaiveDateTime::parse_from_str("2024-01-15 10:00:02", "%Y-%m-%d %H:%M:%S").unwrap()
        );
        // Content should be the raw JSON string
        assert!(lines[0].content.contains("Server started"));
        assert!(lines[0].content.starts_with('{'));
    }

    #[test]
    fn test_json_line_invalid_json() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "this is not json").unwrap();

        let ts_template = make_ts_template();
        let template = SourceTemplate {
            id: 1,
            name: "json_test".into(),
            timestamp_template_id: 1,
            line_delimiter: "\n".into(),
            content_regex: None,
            continuation_regex: None,
            json_timestamp_field: Some("timestamp".into()),
        };
        let source = Source {
            id: 1,
            name: "test".into(),
            template_id: 1,
            file_path: f.path().to_str().unwrap().into(),
        };

        let iter = LogLineIterator::new(&source, &template, &ts_template).unwrap();
        let results: Vec<_> = iter.collect();
        assert_eq!(results.len(), 1);
        assert!(results[0].is_err());
    }

    #[test]
    fn test_json_auto_extraction() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, r#"{{"timestamp":"2024-01-15 10:00:01","level":"INFO","message":"Server started","port":8080}}"#).unwrap();
        writeln!(f, r#"{{"timestamp":"2024-01-15 10:00:02","level":"ERROR","message":"Connection failed","retries":3}}"#).unwrap();
        writeln!(f, r#"{{"timestamp":"2024-01-15 10:00:03","level":"WARN","message":"High memory","usage_pct":85.5}}"#).unwrap();
        writeln!(f, r#"{{"timestamp":"2024-01-15 10:00:04","level":"INFO","message":"Done","success":true}}"#).unwrap();

        let ts_template = make_ts_template();
        let template = SourceTemplate {
            id: 1,
            name: "json_test".into(),
            timestamp_template_id: 1,
            line_delimiter: "\n".into(),
            content_regex: None,
            continuation_regex: None,
            json_timestamp_field: Some("timestamp".into()),
        };
        let source = Source {
            id: 1,
            name: "json_src".into(),
            template_id: 1,
            file_path: f.path().to_str().unwrap().into(),
        };

        let result = analyze(
            &[source],
            &[template],
            &[ts_template],
            &[],
            &[],
            &[],
            &TimeRange::default(),
        )
        .unwrap();

        // Should have state changes for all JSON fields across all lines
        assert!(
            !result.state_changes.is_empty(),
            "expected state changes from JSON auto-extraction"
        );

        // Check specific field types by looking at state changes
        let port_changes: Vec<_> = result
            .state_changes
            .iter()
            .filter(|sc| sc.state_key == "port")
            .collect();
        assert!(!port_changes.is_empty(), "expected port state change");
        assert_eq!(port_changes[0].new_value, Some(StateValue::Integer(8080)));

        let usage_changes: Vec<_> = result
            .state_changes
            .iter()
            .filter(|sc| sc.state_key == "usage_pct")
            .collect();
        assert!(!usage_changes.is_empty(), "expected usage_pct state change");
        assert_eq!(usage_changes[0].new_value, Some(StateValue::Float(85.5)));

        let success_changes: Vec<_> = result
            .state_changes
            .iter()
            .filter(|sc| sc.state_key == "success")
            .collect();
        assert!(!success_changes.is_empty(), "expected success state change");
        assert_eq!(success_changes[0].new_value, Some(StateValue::Bool(true)));

        // rule_id should be 0 for JSON auto-extracted state changes
        for sc in &result.state_changes {
            assert_eq!(
                sc.rule_id, 0,
                "JSON auto-extracted state changes should have rule_id=0"
            );
        }
    }

    // -------------------------------------------------------------------
    // Time-range filtering tests
    // -------------------------------------------------------------------

    fn make_time_range_test_data() -> (
        NamedTempFile,
        Source,
        SourceTemplate,
        TimestampTemplate,
        Vec<LogRule>,
        Vec<Ruleset>,
    ) {
        let mut f = NamedTempFile::new().unwrap();
        // Lines at 00:01 through 00:05
        for min in 1..=5 {
            writeln!(f, "2024-01-01 00:{min:02}:00 event_{min}").unwrap();
        }
        f.flush().unwrap();

        let ts_template = TimestampTemplate {
            id: 1,
            name: "ts".into(),
            format: "%Y-%m-%d %H:%M:%S".into(),
            extraction_regex: None,
            default_year: None,
        };
        let template = SourceTemplate {
            id: 1,
            name: "tmpl".into(),
            timestamp_template_id: 1,
            line_delimiter: "\n".into(),
            content_regex: Some(r"^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2} (.+)$".into()),
            continuation_regex: None,
            json_timestamp_field: None,
        };
        let source = Source {
            id: 1,
            name: "src".into(),
            template_id: 1,
            file_path: f.path().to_str().unwrap().into(),
        };
        let rules = vec![LogRule {
            id: 1,
            name: "match_event".into(),
            match_mode: MatchMode::Any,
            match_rules: vec![MatchRule {
                id: 1,
                pattern: r"event_\d+".into(),
            }],
            extraction_rules: vec![],
        }];
        let rulesets = vec![Ruleset {
            id: 1,
            name: "rs".into(),
            template_id: 1,
            rule_ids: vec![1],
        }];
        (f, source, template, ts_template, rules, rulesets)
    }

    #[test]
    fn test_time_range_filtering() {
        let (_f, source, template, ts_template, rules, rulesets) = make_time_range_test_data();
        let time_range = TimeRange {
            start: Some(
                NaiveDateTime::parse_from_str("2024-01-01 00:02:00", "%Y-%m-%d %H:%M:%S").unwrap(),
            ),
            end: Some(
                NaiveDateTime::parse_from_str("2024-01-01 00:04:00", "%Y-%m-%d %H:%M:%S").unwrap(),
            ),
        };
        let result = analyze(
            &[source],
            &[template],
            &[ts_template],
            &rules,
            &rulesets,
            &[],
            &time_range,
        )
        .unwrap();
        assert_eq!(
            result.rule_matches.len(),
            3,
            "expected matches for events 2, 3, 4"
        );
    }

    #[test]
    fn test_time_range_start_only() {
        let (_f, source, template, ts_template, rules, rulesets) = make_time_range_test_data();
        let time_range = TimeRange {
            start: Some(
                NaiveDateTime::parse_from_str("2024-01-01 00:04:00", "%Y-%m-%d %H:%M:%S").unwrap(),
            ),
            end: None,
        };
        let result = analyze(
            &[source],
            &[template],
            &[ts_template],
            &rules,
            &rulesets,
            &[],
            &time_range,
        )
        .unwrap();
        assert_eq!(
            result.rule_matches.len(),
            2,
            "expected matches for events 4, 5"
        );
    }

    #[test]
    fn test_time_range_end_only() {
        let (_f, source, template, ts_template, rules, rulesets) = make_time_range_test_data();
        let time_range = TimeRange {
            start: None,
            end: Some(
                NaiveDateTime::parse_from_str("2024-01-01 00:02:00", "%Y-%m-%d %H:%M:%S").unwrap(),
            ),
        };
        let result = analyze(
            &[source],
            &[template],
            &[ts_template],
            &rules,
            &rulesets,
            &[],
            &time_range,
        )
        .unwrap();
        assert_eq!(
            result.rule_matches.len(),
            2,
            "expected matches for events 1, 2"
        );
    }

    #[test]
    fn test_time_range_default() {
        let (_f, source, template, ts_template, rules, rulesets) = make_time_range_test_data();
        let result = analyze(
            &[source],
            &[template],
            &[ts_template],
            &rules,
            &rulesets,
            &[],
            &TimeRange::default(),
        )
        .unwrap();
        assert_eq!(
            result.rule_matches.len(),
            5,
            "default TimeRange should return all matches"
        );
    }

    // -----------------------------------------------------------------------
    // Tokenizer tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_tokenize_numbers() {
        assert_eq!(tokenize("ERROR 42 failed"), "ERROR <*> failed");
    }

    #[test]
    fn test_tokenize_ips() {
        assert_eq!(tokenize("connect 10.0.0.1:8080"), "connect <*>");
    }

    #[test]
    fn test_tokenize_preserves_keywords() {
        assert_eq!(tokenize("ERROR timeout"), "ERROR timeout");
    }

    #[test]
    fn test_tokenize_uuids() {
        assert_eq!(
            tokenize("req a1b2c3d4-e5f6-7890-abcd-ef1234567890 done"),
            "req <*> done"
        );
    }

    #[test]
    fn test_tokenize_paths() {
        assert_eq!(tokenize("open /var/log/app.log failed"), "open <*> failed");
    }

    #[test]
    fn test_tokenize_timestamps() {
        assert_eq!(tokenize("at 2024-01-15T10:30:00 event"), "at <*> event");
    }

    #[test]
    fn test_tokenize_quoted_strings() {
        assert_eq!(tokenize(r#"msg "hello_world" end"#), "msg <*> end");
    }

    #[test]
    fn test_cluster_logs_basic() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "2024-01-01 00:00:01 ERROR timeout after 100 ms").unwrap();
        writeln!(f, "2024-01-01 00:00:02 ERROR timeout after 200 ms").unwrap();
        writeln!(f, "2024-01-01 00:00:03 INFO started successfully").unwrap();
        writeln!(f, "2024-01-01 00:00:04 ERROR timeout after 300 ms").unwrap();
        f.flush().unwrap();

        let ts_template = TimestampTemplate {
            id: 1,
            name: "ts".into(),
            format: "%Y-%m-%d %H:%M:%S".into(),
            extraction_regex: None,
            default_year: None,
        };
        let template = SourceTemplate {
            id: 1,
            name: "tmpl".into(),
            timestamp_template_id: 1,
            line_delimiter: "\n".into(),
            content_regex: Some(r"^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2} (.+)$".into()),
            continuation_regex: None,
            json_timestamp_field: None,
        };
        let source = Source {
            id: 1,
            name: "test".into(),
            template_id: 1,
            file_path: f.path().to_str().unwrap().to_string(),
        };

        let result = cluster_logs(
            &[source],
            &[template],
            &[ts_template],
            &TimeRange::default(),
        )
        .unwrap();

        assert_eq!(result.total_lines, 4);
        assert_eq!(result.clusters.len(), 1); // singleton "INFO started successfully" filtered out
        assert_eq!(result.clusters[0].count, 3);
        assert!(result.clusters[0].template.contains("<*>"));
        assert!(result.clusters[0].sample_lines.len() <= 3);
        assert_eq!(result.clusters[0].source_ids, vec![1]);
    }

    #[test]
    fn test_tokenize_time_only() {
        assert_eq!(tokenize("at 04:03:33 event"), "at <*> event");
    }

    #[test]
    fn test_tokenize_time_with_millis() {
        assert_eq!(tokenize("at 17:41:44,747 event"), "at <*> event");
    }

    #[test]
    fn test_tokenize_embedded_digits() {
        // Drain catch-all: tokens with embedded digits are variable
        assert_eq!(
            tokenize("su(pam_unix)[27953]: session opened"),
            "<*> session opened"
        );
    }

    #[test]
    fn test_tokenize_syslog_line() {
        assert_eq!(
            tokenize(
                "authentication failure; logname= uid=0 euid=0 tty=NODEVssh ruser= rhost=218.188.2.4"
            ),
            "authentication failure; logname= <*> <*> tty=NODEVssh ruser= <*>"
        );
    }

    #[test]
    fn test_cluster_excludes_singletons() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "2024-01-01 00:00:01 ERROR timeout 100").unwrap();
        writeln!(f, "2024-01-01 00:00:02 ERROR timeout 200").unwrap();
        writeln!(f, "2024-01-01 00:00:03 ERROR timeout 300").unwrap();
        writeln!(f, "2024-01-01 00:00:04 UNIQUE never repeated xyz").unwrap();
        f.flush().unwrap();

        let ts_template = TimestampTemplate {
            id: 1,
            name: "ts".into(),
            format: "%Y-%m-%d %H:%M:%S".into(),
            extraction_regex: None,
            default_year: None,
        };
        let template = SourceTemplate {
            id: 1,
            name: "tmpl".into(),
            timestamp_template_id: 1,
            line_delimiter: "\n".into(),
            content_regex: Some(r"^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2} (.+)$".into()),
            continuation_regex: None,
            json_timestamp_field: None,
        };
        let source = Source {
            id: 1,
            name: "test".into(),
            template_id: 1,
            file_path: f.path().to_str().unwrap().to_string(),
        };

        let result = cluster_logs(
            &[source],
            &[template],
            &[ts_template],
            &TimeRange::default(),
        )
        .unwrap();

        assert_eq!(result.total_lines, 4);
        assert_eq!(result.clusters.len(), 1); // singleton filtered out
        assert_eq!(result.clusters[0].count, 3);
    }

    #[test]
    fn test_estimate_timestamp_len_iso() {
        // %Y-%m-%d %H:%M:%S → "2015-07-29 17:41:44" = 19 chars
        let (min, max) = estimate_timestamp_len("%Y-%m-%d %H:%M:%S");
        assert_eq!(min, 19);
        assert_eq!(max, 19);
    }

    #[test]
    fn test_estimate_timestamp_len_nginx() {
        // %d/%b/%Y:%H:%M:%S → "17/May/2015:08:05:32" = 20 chars
        let (min, max) = estimate_timestamp_len("%d/%b/%Y:%H:%M:%S");
        assert_eq!(min, 20);
        assert_eq!(max, 20);
    }

    #[test]
    fn test_estimate_timestamp_len_syslog() {
        // %b %d %H:%M:%S → "Jan  3 04:03:33" = 15 chars
        let (min, max) = estimate_timestamp_len("%b %d %H:%M:%S");
        assert_eq!(min, 15);
        assert_eq!(max, 15);
    }

    #[test]
    fn test_estimate_timestamp_len_with_subsecond() {
        // %Y-%m-%dT%H:%M:%S.%3f → "2015-07-29T17:41:44.747" = 23 chars
        let (min, max) = estimate_timestamp_len("%Y-%m-%dT%H:%M:%S.%3f");
        assert_eq!(min, 23);
        assert_eq!(max, 23);
    }

    #[test]
    fn test_estimate_timestamp_len_with_timezone() {
        // %Y-%m-%d %H:%M:%S %z → "2015-07-29 17:41:44 +0000" = 25 chars
        let (min, max) = estimate_timestamp_len("%Y-%m-%d %H:%M:%S %z");
        assert_eq!(min, 25);
        assert_eq!(max, 25);
    }

    #[test]
    fn test_estimate_timestamp_len_yearless_augmented() {
        // Augmented: %Y %b %d %H:%M:%S → "2005 Jan  3 04:03:33" = 20 chars
        let (min, max) = estimate_timestamp_len("%Y %b %d %H:%M:%S");
        assert_eq!(min, 20);
        assert_eq!(max, 20);
    }

    #[test]
    fn test_parse_timestamp_prefix_zookeeper() {
        let line = "2015-07-29 17:41:44,747 - INFO  [QuorumPeer]";
        let ts = parse_timestamp_prefix(line, "%Y-%m-%d %H:%M:%S").unwrap();
        assert_eq!(
            ts,
            NaiveDateTime::parse_from_str("2015-07-29 17:41:44", "%Y-%m-%d %H:%M:%S").unwrap()
        );
    }

    #[test]
    fn test_parse_timestamp_prefix_syslog_augmented() {
        let line = "2005 Jan  3 04:03:33 combo sshd[5765]: pam_unix";
        let ts = parse_timestamp_prefix(line, "%Y %b %d %H:%M:%S").unwrap();
        assert_eq!(
            ts,
            NaiveDateTime::parse_from_str("2005-01-03 04:03:33", "%Y-%m-%d %H:%M:%S").unwrap()
        );
    }
}
