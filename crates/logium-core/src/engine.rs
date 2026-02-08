use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::fs::File;
use std::io::{BufRead, BufReader};

use chrono::NaiveDateTime;
use regex::{Regex, RegexSet};
use serde::{Deserialize, Serialize};

use crate::model::*;

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
    Progress { lines_processed: u64 },
    Complete { total_lines: u64, total_rule_matches: u64, total_pattern_matches: u64 },
    Error { message: String },
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
    buf: String,
}

impl LogLineIterator {
    pub fn new(
        source: &Source,
        template: &SourceTemplate,
        ts_template: &TimestampTemplate,
    ) -> Result<Self, AnalysisError> {
        let file = File::open(&source.file_path).map_err(|_| {
            AnalysisError::FileNotFound(source.file_path.clone())
        })?;
        let content_regex = match &template.content_regex {
            Some(pat) => {
                let re = Regex::new(pat)
                    .map_err(|e| AnalysisError::InvalidRegex(e.to_string()))?;
                Some(re)
            }
            None => None,
        };
        let extraction_regex = match &ts_template.extraction_regex {
            Some(pat) => {
                let re = Regex::new(pat)
                    .map_err(|e| AnalysisError::InvalidRegex(e.to_string()))?;
                Some(re)
            }
            None => None,
        };
        Ok(Self {
            reader: BufReader::new(file),
            source_id: source.id,
            timestamp_format: ts_template.format.clone(),
            extraction_regex,
            default_year: ts_template.default_year,
            content_regex,
            buf: String::new(),
        })
    }
}

impl Iterator for LogLineIterator {
    type Item = Result<LogLine, AnalysisError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.buf.clear();
        match self.reader.read_line(&mut self.buf) {
            Ok(0) => None,
            Ok(_) => {
                let raw = self.buf.trim_end_matches('\n').trim_end_matches('\r').to_string();
                let content = if let Some(re) = &self.content_regex {
                    if let Some(caps) = re.captures(&raw) {
                        caps.get(1).map_or(raw.clone(), |m| m.as_str().to_string())
                    } else {
                        raw.clone()
                    }
                } else {
                    raw.clone()
                };

                // Extract timestamp substring: use extraction_regex if set, otherwise raw line
                let ts_input = if let Some(re) = &self.extraction_regex {
                    if let Some(caps) = re.captures(&raw) {
                        caps.get(1)
                            .map(|m| m.as_str().to_string())
                            .unwrap_or_else(|| raw.clone())
                    } else {
                        raw.clone()
                    }
                } else {
                    raw.clone()
                };

                let timestamp = NaiveDateTime::parse_from_str(&ts_input, &self.timestamp_format)
                    .or_else(|_| parse_timestamp_prefix(&ts_input, &self.timestamp_format))
                    .or_else(|e| {
                        // For yearless formats, prepend default_year and try again
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
                    Ok(ts) => Some(Ok(LogLine {
                        timestamp: ts,
                        source_id: self.source_id,
                        raw,
                        content,
                    })),
                    Err(e) => Some(Err(AnalysisError::InvalidTimestampFormat(format!(
                        "failed to parse timestamp from '{}' with format '{}': {}",
                        raw, self.timestamp_format, e
                    )))),
                }
            }
            Err(e) => Some(Err(AnalysisError::ParseError(e.to_string()))),
        }
    }
}

/// Parse a timestamp from the beginning of a line by trying progressively
/// shorter prefixes until chrono can parse it without "trailing input" errors.
fn parse_timestamp_prefix(
    line: &str,
    fmt: &str,
) -> Result<NaiveDateTime, chrono::ParseError> {
    // Try substrings from the full line down to a minimum length.
    // This handles the common case where the timestamp is at the start of the line
    // and is followed by arbitrary content.
    let mut last_err = NaiveDateTime::parse_from_str(line, fmt).unwrap_err();
    let min_len = fmt.len().min(line.len());
    for end in (min_len..=line.len()).rev() {
        // Only try at character boundaries
        if !line.is_char_boundary(end) {
            continue;
        }
        match NaiveDateTime::parse_from_str(&line[..end], fmt) {
            Ok(ts) => return Ok(ts),
            Err(e) => last_err = e,
        }
    }
    Err(last_err)
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
        let patterns: Vec<&str> = rule.match_rules.iter().map(|m| m.pattern.as_str()).collect();
        let match_set = RegexSet::new(&patterns)
            .map_err(|e| AnalysisError::InvalidRegex(e.to_string()))?;
        let match_count = rule.match_rules.len();

        let mut extraction_regexes = Vec::new();
        for (idx, ext) in rule.extraction_rules.iter().enumerate() {
            if let ExtractionType::Parsed = ext.extraction_type {
                if let Some(pat) = &ext.pattern {
                    let re = Regex::new(pat)
                        .map_err(|e| AnalysisError::InvalidRegex(e.to_string()))?;
                    extraction_regexes.push((idx, re));
                }
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
    let matches: Vec<usize> = compiled.match_set.matches(&line.content).into_iter().collect();

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
                {
                    if let Some(caps) = re.captures(&line.content) {
                        if let Some(m) = caps.name(&ext.state_key) {
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
        }
    }

    Some(extracted)
}

// ---------------------------------------------------------------------------
// State manager
// ---------------------------------------------------------------------------

/// Manages per-source state.
pub struct StateManager {
    pub per_source_state: HashMap<u64, HashMap<String, StateValue>>,
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
    pub fn apply_mutations(
        &mut self,
        source_id: u64,
        extractions: &HashMap<String, StateValue>,
        rules: &[ExtractionRule],
    ) {
        let state = self.per_source_state.entry(source_id).or_default();

        for rule in rules {
            match rule.extraction_type {
                ExtractionType::Clear => {
                    state.remove(&rule.state_key);
                }
                ExtractionType::Static => {
                    if let Some(val) = &rule.static_value {
                        let new_val = StateValue::String(val.clone());
                        match rule.mode {
                            ExtractionMode::Replace => {
                                state.insert(rule.state_key.clone(), new_val);
                            }
                            ExtractionMode::Accumulate => {
                                accumulate(state, &rule.state_key, new_val);
                            }
                        }
                    }
                }
                ExtractionType::Parsed => {
                    if let Some(val) = extractions.get(&rule.state_key) {
                        match rule.mode {
                            ExtractionMode::Replace => {
                                state.insert(rule.state_key.clone(), val.clone());
                            }
                            ExtractionMode::Accumulate => {
                                accumulate(state, &rule.state_key, val.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    /// Resolve the value of a source's state key by source name.
    pub fn get_state_by_name(&self, source_name: &str, key: &str) -> Option<&StateValue> {
        let id = self.name_to_id.get(source_name)?;
        self.per_source_state.get(id)?.get(key)
    }

    /// Snapshot all state, keyed by source name.
    pub fn snapshot(&self) -> HashMap<String, HashMap<String, StateValue>> {
        let mut snap = HashMap::new();
        for (id, state) in &self.per_source_state {
            if let Some(name) = self.source_names.get(id) {
                snap.insert(name.clone(), state.clone());
            }
        }
        snap
    }
}

/// Accumulate a value into existing state.
fn accumulate(state: &mut HashMap<String, StateValue>, key: &str, new_val: StateValue) {
    if let Some(existing) = state.get(key) {
        let merged = match (existing, &new_val) {
            (StateValue::String(a), StateValue::String(b)) => {
                StateValue::String(format!("{a},{b}"))
            }
            (StateValue::Integer(a), StateValue::Integer(b)) => StateValue::Integer(a + b),
            (StateValue::Float(a), StateValue::Float(b)) => StateValue::Float(a + b),
            (StateValue::Integer(a), StateValue::Float(b)) => StateValue::Float(*a as f64 + b),
            (StateValue::Float(a), StateValue::Integer(b)) => StateValue::Float(a + *b as f64),
            _ => new_val,
        };
        state.insert(key.to_string(), merged);
    } else {
        state.insert(key.to_string(), new_val);
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
        Operator::Eq => {
            match (current_val, &operand_val) {
                (Some(a), Some(b)) => a == b,
                _ => false,
            }
        }
        Operator::Neq => {
            match (current_val, &operand_val) {
                (Some(a), Some(b)) => a != b,
                _ => false,
            }
        }
        Operator::Gt => {
            match (current_val, &operand_val) {
                (Some(a), Some(b)) => a.partial_cmp(b) == Some(Ordering::Greater),
                _ => false,
            }
        }
        Operator::Lt => {
            match (current_val, &operand_val) {
                (Some(a), Some(b)) => a.partial_cmp(b) == Some(Ordering::Less),
                _ => false,
            }
        }
        Operator::Gte => {
            match (current_val, &operand_val) {
                (Some(a), Some(b)) => matches!(
                    a.partial_cmp(b),
                    Some(Ordering::Greater | Ordering::Equal)
                ),
                _ => false,
            }
        }
        Operator::Lte => {
            match (current_val, &operand_val) {
                (Some(a), Some(b)) => matches!(
                    a.partial_cmp(b),
                    Some(Ordering::Less | Ordering::Equal)
                ),
                _ => false,
            }
        }
        Operator::Contains => {
            match (current_val, &operand_val) {
                (Some(StateValue::String(a)), Some(StateValue::String(b))) => a.contains(b.as_str()),
                _ => false,
            }
        }
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
) -> Result<AnalysisResult, AnalysisError> {
    // Build template lookup
    let template_map: HashMap<u64, &SourceTemplate> =
        templates.iter().map(|t| (t.id, t)).collect();

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

    // Build source -> template_id lookup
    let source_template: HashMap<u64, u64> = sources.iter().map(|s| (s.id, s.template_id)).collect();

    // Create log line iterators
    let mut iterators = Vec::new();
    for source in sources {
        let template = template_map
            .get(&source.template_id)
            .ok_or_else(|| AnalysisError::ParseError(format!(
                "no template found for template_id {}",
                source.template_id
            )))?;
        let ts_template = ts_template_map
            .get(&template.timestamp_template_id)
            .ok_or_else(|| AnalysisError::ParseError(format!(
                "no timestamp template found for timestamp_template_id {}",
                template.timestamp_template_id
            )))?;
        iterators.push(LogLineIterator::new(source, template, ts_template)?);
    }

    // K-way merge
    let stream = MergedLogStream::new(iterators)?;

    // State and pattern evaluator
    let mut state_manager = StateManager::new(sources);
    let mut pattern_eval = PatternEvaluator::new(patterns);

    let mut all_rule_matches = Vec::new();
    let mut all_pattern_matches = Vec::new();

    for result in stream {
        let line = result?;
        let tmpl_id = source_template
            .get(&line.source_id)
            .copied()
            .unwrap_or(0);

        // Find applicable rule ids
        if let Some(rule_ids) = template_rule_ids.get(&tmpl_id) {
            for rule_id in rule_ids {
                if let (Some(rule), Some(compiled)) =
                    (rule_map.get(rule_id), compiled_map.get(rule_id))
                {
                    if let Some(extracted) = evaluate_rule(rule, &line, compiled) {
                        // Apply state mutations
                        state_manager.apply_mutations(
                            line.source_id,
                            &extracted,
                            &rule.extraction_rules,
                        );

                        all_rule_matches.push(RuleMatch {
                            rule_id: *rule_id,
                            source_id: line.source_id,
                            log_line: line.clone(),
                            extracted_state: extracted,
                        });
                    }
                }
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
    })
}

/// Run the analysis pipeline, streaming events through a channel.
///
/// Mirrors `analyze()` but sends each match as it occurs rather than collecting.
/// Returns early if the receiver is dropped (client disconnected).
pub fn analyze_streaming(
    sources: &[Source],
    templates: &[SourceTemplate],
    timestamp_templates: &[TimestampTemplate],
    rules: &[LogRule],
    rulesets: &[Ruleset],
    patterns: &[Pattern],
    tx: std::sync::mpsc::Sender<AnalysisEvent>,
) -> Result<(), AnalysisError> {
    // Build template lookup
    let template_map: HashMap<u64, &SourceTemplate> =
        templates.iter().map(|t| (t.id, t)).collect();
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

    let source_template: HashMap<u64, u64> = sources.iter().map(|s| (s.id, s.template_id)).collect();

    let mut iterators = Vec::new();
    for source in sources {
        let template = template_map
            .get(&source.template_id)
            .ok_or_else(|| AnalysisError::ParseError(format!(
                "no template found for template_id {}",
                source.template_id
            )))?;
        let ts_template = ts_template_map
            .get(&template.timestamp_template_id)
            .ok_or_else(|| AnalysisError::ParseError(format!(
                "no timestamp template found for timestamp_template_id {}",
                template.timestamp_template_id
            )))?;
        iterators.push(LogLineIterator::new(source, template, ts_template)?);
    }

    let stream = MergedLogStream::new(iterators)?;

    let mut state_manager = StateManager::new(sources);
    let mut pattern_eval = PatternEvaluator::new(patterns);

    let mut lines_processed: u64 = 0;
    let mut total_rule_matches: u64 = 0;
    let mut total_pattern_matches: u64 = 0;

    for result in stream {
        let line = match result {
            Ok(l) => l,
            Err(e) => {
                let _ = tx.send(AnalysisEvent::Error { message: e.to_string() });
                return Err(e);
            }
        };

        lines_processed += 1;

        let tmpl_id = source_template
            .get(&line.source_id)
            .copied()
            .unwrap_or(0);

        if let Some(rule_ids) = template_rule_ids.get(&tmpl_id) {
            for rule_id in rule_ids {
                if let (Some(rule), Some(compiled)) =
                    (rule_map.get(rule_id), compiled_map.get(rule_id))
                {
                    if let Some(extracted) = evaluate_rule(rule, &line, compiled) {
                        state_manager.apply_mutations(
                            line.source_id,
                            &extracted,
                            &rule.extraction_rules,
                        );

                        let rm = RuleMatch {
                            rule_id: *rule_id,
                            source_id: line.source_id,
                            log_line: line.clone(),
                            extracted_state: extracted,
                        };
                        total_rule_matches += 1;
                        if tx.send(AnalysisEvent::RuleMatch(rm)).is_err() {
                            return Ok(()); // receiver dropped
                        }
                    }
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

        if lines_processed % 500 == 0 {
            if tx.send(AnalysisEvent::Progress { lines_processed }).is_err() {
                return Ok(());
            }
        }
    }

    let _ = tx.send(AnalysisEvent::Complete {
        total_lines: lines_processed,
        total_rule_matches,
        total_pattern_matches,
    });

    Ok(())
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

    fn make_log_line(content: &str) -> LogLine {
        LogLine {
            timestamp: NaiveDateTime::parse_from_str(
                "2024-01-01 00:00:00",
                "%Y-%m-%d %H:%M:%S",
            )
            .unwrap(),
            source_id: 1,
            raw: content.to_string(),
            content: content.to_string(),
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
                MatchRule { id: 1, pattern: r"ERROR".into() },
                MatchRule { id: 2, pattern: r"WARN".into() },
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
                MatchRule { id: 1, pattern: r"server".into() },
                MatchRule { id: 2, pattern: r"error".into() },
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
        sm.per_source_state
            .entry(1)
            .or_default()
            .insert("key".into(), StateValue::String("old".into()));

        let extractions: HashMap<String, StateValue> = HashMap::new();
        let rules = vec![ExtractionRule {
            id: 1,
            extraction_type: ExtractionType::Static,
            state_key: "key".into(),
            pattern: None,
            static_value: Some("new".into()),
            mode: ExtractionMode::Replace,
        }];
        sm.apply_mutations(1, &extractions, &rules);

        assert_eq!(
            sm.per_source_state[&1]["key"],
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
        sm.per_source_state
            .entry(1)
            .or_default()
            .insert("tags".into(), StateValue::String("a".into()));

        let extractions: HashMap<String, StateValue> = HashMap::new();
        let rules = vec![ExtractionRule {
            id: 1,
            extraction_type: ExtractionType::Static,
            state_key: "tags".into(),
            pattern: None,
            static_value: Some("b".into()),
            mode: ExtractionMode::Accumulate,
        }];
        sm.apply_mutations(1, &extractions, &rules);

        assert_eq!(
            sm.per_source_state[&1]["tags"],
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
        sm.per_source_state
            .entry(1)
            .or_default()
            .insert("count".into(), StateValue::Integer(10));

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
        sm.apply_mutations(1, &extractions, &rules);

        assert_eq!(sm.per_source_state[&1]["count"], StateValue::Integer(15));
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
        sm.per_source_state
            .entry(1)
            .or_default()
            .insert("key".into(), StateValue::String("val".into()));

        let extractions: HashMap<String, StateValue> = HashMap::new();
        let rules = vec![ExtractionRule {
            id: 1,
            extraction_type: ExtractionType::Clear,
            state_key: "key".into(),
            pattern: None,
            static_value: None,
            mode: ExtractionMode::Replace,
        }];
        sm.apply_mutations(1, &extractions, &rules);

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
            Source { id: 1, name: "server".into(), template_id: 1, file_path: "".into() },
            Source { id: 2, name: "client".into(), template_id: 1, file_path: "".into() },
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
        sm.per_source_state
            .entry(1)
            .or_default()
            .insert("status".into(), StateValue::String("running".into()));
        let matches = eval.evaluate_patterns(&patterns, &sm);
        assert!(matches.is_empty()); // only 1 of 2 done

        // Set players = 5 -> pred 2 satisfied
        sm.per_source_state
            .entry(1)
            .or_default()
            .insert("players".into(), StateValue::Integer(5));
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
        sm.per_source_state
            .entry(1)
            .or_default()
            .insert("status".into(), StateValue::String("running".into()));
        eval.evaluate_patterns(&patterns, &sm);
        assert_eq!(eval.progress[0], 1);

        // Now invalidate pred 1 (change status away from "running") and try pred 2
        sm.per_source_state
            .get_mut(&1)
            .unwrap()
            .insert("status".into(), StateValue::String("stopped".into()));
        sm.per_source_state
            .get_mut(&1)
            .unwrap()
            .insert("count".into(), StateValue::Integer(20));
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
        sm.per_source_state
            .entry(1)
            .or_default()
            .insert("flag".into(), StateValue::Bool(true));
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
        sm.per_source_state
            .entry(1)
            .or_default()
            .insert("region".into(), StateValue::String("us-east".into()));
        sm.per_source_state
            .entry(2)
            .or_default()
            .insert("region".into(), StateValue::String("eu-west".into()));
        let matches = eval.evaluate_patterns(&patterns, &sm);
        assert!(matches.is_empty());

        // Same regions -> match
        sm.per_source_state
            .get_mut(&2)
            .unwrap()
            .insert("region".into(), StateValue::String("us-east".into()));
        let matches = eval.evaluate_patterns(&patterns, &sm);
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn test_all_operators() {
        let sources = make_sources();
        let mut sm = StateManager::new(&sources);
        sm.per_source_state.entry(1).or_default().insert(
            "val".into(),
            StateValue::Integer(10),
        );
        sm.per_source_state.entry(1).or_default().insert(
            "name".into(),
            StateValue::String("hello world".into()),
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

        let sources = vec![
            Source { id: 1, name: "s1".into(), template_id: 1, file_path: f1.path().to_str().unwrap().into() },
            Source { id: 2, name: "s2".into(), template_id: 1, file_path: f2.path().to_str().unwrap().into() },
            Source { id: 3, name: "s3".into(), template_id: 1, file_path: f3.path().to_str().unwrap().into() },
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
        writeln!(server_log, "2024-01-01 00:00:01 [INFO] Server started in region us-east").unwrap();
        writeln!(server_log, "2024-01-01 00:00:03 [INFO] Players online: 42").unwrap();
        writeln!(server_log, "2024-01-01 00:00:05 [INFO] Players online: 100").unwrap();

        // Client log: has region info
        writeln!(client_log, "2024-01-01 00:00:02 [INFO] Client connecting to region us-east").unwrap();
        writeln!(client_log, "2024-01-01 00:00:04 [INFO] Client connected, status active").unwrap();

        let ts_template = make_ts_template();

        let template = SourceTemplate {
            id: 1,
            name: "default".into(),
            timestamp_template_id: 1,
            line_delimiter: "\n".into(),
            content_regex: Some(r"^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2} (.+)$".into()),
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

        let rulesets = vec![
            Ruleset {
                id: 1,
                name: "server_rules".into(),
                template_id: 1,
                rule_ids: vec![1, 2, 3],
            },
        ];

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
            snap["server"]["region"],
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

        writeln!(server_log, "2024-01-01 00:00:01 [INFO] Server started in region us-east").unwrap();
        writeln!(server_log, "2024-01-01 00:00:03 [INFO] Players online: 42").unwrap();
        writeln!(server_log, "2024-01-01 00:00:05 [INFO] Players online: 100").unwrap();

        writeln!(client_log, "2024-01-01 00:00:02 [INFO] Client connecting to region us-east").unwrap();
        writeln!(client_log, "2024-01-01 00:00:04 [INFO] Client connected, status active").unwrap();

        let ts_template = make_ts_template();
        let template = SourceTemplate {
            id: 1,
            name: "default".into(),
            timestamp_template_id: 1,
            line_delimiter: "\n".into(),
            content_regex: Some(r"^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2} (.+)$".into()),
        };

        let sources = vec![
            Source {
                id: 1, name: "server".into(), template_id: 1,
                file_path: server_log.path().to_str().unwrap().into(),
            },
            Source {
                id: 2, name: "client".into(), template_id: 1,
                file_path: client_log.path().to_str().unwrap().into(),
            },
        ];

        let rules = vec![
            LogRule {
                id: 1, name: "server_region".into(),
                match_mode: MatchMode::Any,
                match_rules: vec![MatchRule { id: 1, pattern: r"region \w+".into() }],
                extraction_rules: vec![ExtractionRule {
                    id: 1, extraction_type: ExtractionType::Parsed,
                    state_key: "region".into(),
                    pattern: Some(r"region (?P<region>\S+)".into()),
                    static_value: None, mode: ExtractionMode::Replace,
                }],
            },
            LogRule {
                id: 2, name: "player_count".into(),
                match_mode: MatchMode::Any,
                match_rules: vec![MatchRule { id: 2, pattern: r"Players online: \d+".into() }],
                extraction_rules: vec![ExtractionRule {
                    id: 2, extraction_type: ExtractionType::Parsed,
                    state_key: "player_count".into(),
                    pattern: Some(r"Players online: (?P<player_count>\d+)".into()),
                    static_value: None, mode: ExtractionMode::Replace,
                }],
            },
            LogRule {
                id: 3, name: "client_region".into(),
                match_mode: MatchMode::Any,
                match_rules: vec![MatchRule { id: 3, pattern: r"connecting to region".into() }],
                extraction_rules: vec![ExtractionRule {
                    id: 3, extraction_type: ExtractionType::Parsed,
                    state_key: "region".into(),
                    pattern: Some(r"region (?P<region>\S+)".into()),
                    static_value: None, mode: ExtractionMode::Replace,
                }],
            },
        ];

        let rulesets = vec![Ruleset {
            id: 1, name: "server_rules".into(), template_id: 1, rule_ids: vec![1, 2, 3],
        }];

        let pattern = Pattern {
            id: 1, name: "cross_source_detect".into(),
            predicates: vec![
                PatternPredicate {
                    source_name: "server".into(), state_key: "region".into(),
                    operator: Operator::Eq,
                    operand: Operand::StateRef { source_name: "client".into(), state_key: "region".into() },
                },
                PatternPredicate {
                    source_name: "server".into(), state_key: "player_count".into(),
                    operator: Operator::Gt,
                    operand: Operand::Literal(StateValue::Integer(50)),
                },
            ],
        };

        // Run streaming analysis
        let (tx, rx) = std::sync::mpsc::channel();
        analyze_streaming(
            &sources, &[template.clone()], &[ts_template.clone()],
            &rules, &rulesets, &[pattern.clone()], tx,
        ).unwrap();

        // Also run synchronous analysis for comparison
        let sync_result = analyze(
            &sources, &[template], &[ts_template],
            &rules, &rulesets, &[pattern],
        ).unwrap();

        // Collect streaming events
        let events: Vec<AnalysisEvent> = rx.iter().collect();

        let stream_rule_matches: Vec<_> = events.iter().filter(|e| matches!(e, AnalysisEvent::RuleMatch(_))).collect();
        let stream_pattern_matches: Vec<_> = events.iter().filter(|e| matches!(e, AnalysisEvent::PatternMatch(_))).collect();
        let complete_events: Vec<_> = events.iter().filter(|e| matches!(e, AnalysisEvent::Complete { .. })).collect();

        // Same number of matches as synchronous
        assert_eq!(stream_rule_matches.len(), sync_result.rule_matches.len());
        assert_eq!(stream_pattern_matches.len(), sync_result.pattern_matches.len());

        // Exactly one Complete event
        assert_eq!(complete_events.len(), 1);
        if let AnalysisEvent::Complete { total_lines, total_rule_matches, total_pattern_matches } = &complete_events[0] {
            assert_eq!(*total_lines, 5);
            assert_eq!(*total_rule_matches, sync_result.rule_matches.len() as u64);
            assert_eq!(*total_pattern_matches, sync_result.pattern_matches.len() as u64);
        } else {
            panic!("expected Complete event");
        }
    }
}
