use std::collections::HashMap;

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use crate::model::{
    AnalysisResult, LogRule, Pattern, PatternMatch, RuleMatch, Source, StateChange, StateValue,
    TrackedValue,
};

// ---- Export options ----

/// Controls which sections to include in JSON export.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportOptions {
    pub rule_matches: bool,
    pub pattern_matches: bool,
    pub state_changes: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            rule_matches: true,
            pattern_matches: true,
            state_changes: true,
        }
    }
}

/// Selects which single section to export as CSV.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsvSection {
    RuleMatches,
    PatternMatches,
    StateChanges,
}

// ---- Enriched export types (private) ----

#[derive(Serialize)]
struct ExportResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    rule_matches: Option<Vec<ExportRuleMatch>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pattern_matches: Option<Vec<ExportPatternMatch>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state_changes: Option<Vec<ExportStateChange>>,
}

#[derive(Serialize)]
struct ExportRuleMatch {
    timestamp: NaiveDateTime,
    rule_name: String,
    source_name: String,
    content: String,
    extracted_state: HashMap<String, StateValue>,
}

#[derive(Serialize)]
struct ExportPatternMatch {
    timestamp: NaiveDateTime,
    pattern_name: String,
    state_snapshot: HashMap<String, HashMap<String, TrackedValue>>,
}

#[derive(Serialize)]
struct ExportStateChange {
    timestamp: NaiveDateTime,
    source_name: String,
    rule_name: String,
    state_key: String,
    old_value: Option<StateValue>,
    new_value: Option<StateValue>,
}

// ---- Lookup helpers ----

struct Lookups<'a> {
    rules: HashMap<u64, &'a str>,
    sources: HashMap<u64, &'a str>,
    patterns: HashMap<u64, &'a str>,
}

fn build_lookups<'a>(
    rules: &'a [LogRule],
    sources: &'a [Source],
    patterns: &'a [Pattern],
) -> Lookups<'a> {
    Lookups {
        rules: rules.iter().map(|r| (r.id, r.name.as_str())).collect(),
        sources: sources.iter().map(|s| (s.id, s.name.as_str())).collect(),
        patterns: patterns.iter().map(|p| (p.id, p.name.as_str())).collect(),
    }
}

fn lookup_name(map: &HashMap<u64, &str>, id: u64) -> String {
    match map.get(&id) {
        Some(name) => name.to_string(),
        None => format!("unknown({id})"),
    }
}

// ---- Enrichment ----

fn enrich_rule_match(rm: &RuleMatch, lookups: &Lookups) -> ExportRuleMatch {
    ExportRuleMatch {
        timestamp: rm.log_line.timestamp,
        rule_name: lookup_name(&lookups.rules, rm.rule_id),
        source_name: lookup_name(&lookups.sources, rm.source_id),
        content: rm.log_line.content.to_string(),
        extracted_state: rm.extracted_state.clone(),
    }
}

fn enrich_pattern_match(pm: &PatternMatch, lookups: &Lookups) -> ExportPatternMatch {
    ExportPatternMatch {
        timestamp: pm.timestamp,
        pattern_name: lookup_name(&lookups.patterns, pm.pattern_id),
        state_snapshot: pm.state_snapshot.clone(),
    }
}

fn enrich_state_change(sc: &StateChange, lookups: &Lookups) -> ExportStateChange {
    ExportStateChange {
        timestamp: sc.timestamp,
        source_name: lookup_name(&lookups.sources, sc.source_id),
        rule_name: lookup_name(&lookups.rules, sc.rule_id),
        state_key: sc.state_key.clone(),
        old_value: sc.old_value.clone(),
        new_value: sc.new_value.clone(),
    }
}

fn build_export(
    result: &AnalysisResult,
    lookups: &Lookups,
    options: &ExportOptions,
) -> ExportResult {
    ExportResult {
        rule_matches: if options.rule_matches {
            Some(
                result
                    .rule_matches
                    .iter()
                    .map(|rm| enrich_rule_match(rm, lookups))
                    .collect(),
            )
        } else {
            None
        },
        pattern_matches: if options.pattern_matches {
            Some(
                result
                    .pattern_matches
                    .iter()
                    .map(|pm| enrich_pattern_match(pm, lookups))
                    .collect(),
            )
        } else {
            None
        },
        state_changes: if options.state_changes {
            Some(
                result
                    .state_changes
                    .iter()
                    .map(|sc| enrich_state_change(sc, lookups))
                    .collect(),
            )
        } else {
            None
        },
    }
}

// ---- Public API ----

/// Export analysis results as pretty-printed JSON. Multiple sections can be
/// included in a single file via `ExportOptions`.
pub fn to_json(
    result: &AnalysisResult,
    rules: &[LogRule],
    sources: &[Source],
    patterns: &[Pattern],
    options: &ExportOptions,
) -> String {
    let lookups = build_lookups(rules, sources, patterns);
    let export = build_export(result, &lookups, options);
    serde_json::to_string_pretty(&export).unwrap()
}

/// Export a single section of analysis results as CSV. Each section has its
/// own column schema, so they must be separate files for spreadsheet apps.
///
/// Output includes a UTF-8 BOM, CRLF line endings, and all-quoted fields
/// for macOS Numbers / Excel compatibility.
pub fn to_csv(
    result: &AnalysisResult,
    rules: &[LogRule],
    sources: &[Source],
    patterns: &[Pattern],
    section: CsvSection,
) -> String {
    let lookups = build_lookups(rules, sources, patterns);
    let mut wtr = csv::WriterBuilder::new()
        .quote_style(csv::QuoteStyle::Always)
        .terminator(csv::Terminator::CRLF)
        .from_writer(vec![]);

    match section {
        CsvSection::RuleMatches => {
            write_rule_matches_csv(&mut wtr, &result.rule_matches, &lookups);
        }
        CsvSection::PatternMatches => {
            write_pattern_matches_csv(&mut wtr, &result.pattern_matches, &lookups);
        }
        CsvSection::StateChanges => {
            write_state_changes_csv(&mut wtr, &result.state_changes, &lookups);
        }
    }

    let data = wtr.into_inner().unwrap();
    // UTF-8 BOM required for macOS Numbers / Excel to recognize CSV encoding
    let mut out = String::from("\u{FEFF}");
    out.push_str(&String::from_utf8(data).unwrap());
    out
}

// ---- CSV helpers ----

fn fmt_ts(ts: NaiveDateTime) -> String {
    ts.format("%Y-%m-%d %H:%M:%S").to_string()
}

fn fmt_state_value(sv: &Option<StateValue>) -> String {
    match sv {
        None => String::new(),
        Some(v) => v.to_string(),
    }
}

fn write_rule_matches_csv(
    wtr: &mut csv::Writer<Vec<u8>>,
    matches: &[RuleMatch],
    lookups: &Lookups,
) {
    wtr.write_record([
        "timestamp",
        "rule_name",
        "source_name",
        "content",
        "extracted_state",
    ])
    .unwrap();
    for rm in matches {
        let enriched = enrich_rule_match(rm, lookups);
        let state_json = serde_json::to_string(&enriched.extracted_state).unwrap();
        wtr.write_record([
            &fmt_ts(enriched.timestamp),
            &enriched.rule_name,
            &enriched.source_name,
            &enriched.content,
            &state_json,
        ])
        .unwrap();
    }
}

fn write_pattern_matches_csv(
    wtr: &mut csv::Writer<Vec<u8>>,
    matches: &[PatternMatch],
    lookups: &Lookups,
) {
    wtr.write_record(["timestamp", "pattern_name", "state_snapshot"])
        .unwrap();
    for pm in matches {
        let enriched = enrich_pattern_match(pm, lookups);
        let snapshot_json = serde_json::to_string(&enriched.state_snapshot).unwrap();
        wtr.write_record([
            &fmt_ts(enriched.timestamp),
            &enriched.pattern_name,
            &snapshot_json,
        ])
        .unwrap();
    }
}

fn write_state_changes_csv(
    wtr: &mut csv::Writer<Vec<u8>>,
    changes: &[StateChange],
    lookups: &Lookups,
) {
    wtr.write_record([
        "timestamp",
        "source_name",
        "rule_name",
        "state_key",
        "old_value",
        "new_value",
    ])
    .unwrap();
    for sc in changes {
        let enriched = enrich_state_change(sc, lookups);
        wtr.write_record([
            &fmt_ts(enriched.timestamp),
            &enriched.source_name,
            &enriched.rule_name,
            &enriched.state_key,
            &fmt_state_value(&enriched.old_value),
            &fmt_state_value(&enriched.new_value),
        ])
        .unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;
    use std::collections::HashMap;
    use std::sync::Arc;

    fn test_rules() -> Vec<LogRule> {
        vec![LogRule {
            id: 1,
            name: "Timeout".to_string(),
            match_mode: MatchMode::Any,
            match_rules: vec![],
            extraction_rules: vec![],
        }]
    }

    fn test_sources() -> Vec<Source> {
        vec![Source {
            id: 10,
            name: "app.log".to_string(),
            template_id: 1,
            file_path: "/var/log/app.log".to_string(),
        }]
    }

    fn test_patterns() -> Vec<Pattern> {
        vec![Pattern {
            id: 100,
            name: "Service Down".to_string(),
            predicates: vec![],
        }]
    }

    fn test_ts() -> NaiveDateTime {
        NaiveDateTime::parse_from_str("2024-01-15 10:30:00", "%Y-%m-%d %H:%M:%S").unwrap()
    }

    fn test_result() -> AnalysisResult {
        let mut extracted = HashMap::new();
        extracted.insert(
            "status".to_string(),
            StateValue::String("error".to_string()),
        );

        let mut snapshot = HashMap::new();
        let mut inner = HashMap::new();
        inner.insert(
            "key".to_string(),
            TrackedValue {
                value: StateValue::String("val".to_string()),
                set_at: test_ts(),
            },
        );
        snapshot.insert("app.log".to_string(), inner);

        AnalysisResult {
            rule_matches: vec![RuleMatch {
                rule_id: 1,
                source_id: 10,
                log_line: LogLine {
                    timestamp: test_ts(),
                    source_id: 10,
                    raw: Arc::from("ERROR broke"),
                    content: Arc::from("broke"),
                },
                extracted_state: extracted,
            }],
            pattern_matches: vec![PatternMatch {
                pattern_id: 100,
                timestamp: test_ts(),
                state_snapshot: snapshot,
            }],
            state_changes: vec![StateChange {
                timestamp: test_ts(),
                source_id: 10,
                source_name: "app.log".to_string(),
                state_key: "status".to_string(),
                old_value: None,
                new_value: Some(StateValue::String("error".to_string())),
                rule_id: 1,
            }],
        }
    }

    /// Strip the UTF-8 BOM from the beginning of CSV output for easier test assertions.
    fn strip_bom(s: &str) -> &str {
        s.strip_prefix('\u{FEFF}').unwrap_or(s)
    }

    #[test]
    fn test_csv_starts_with_bom() {
        let result = test_result();
        let csv = to_csv(
            &result,
            &test_rules(),
            &test_sources(),
            &test_patterns(),
            CsvSection::RuleMatches,
        );
        assert!(
            csv.starts_with('\u{FEFF}'),
            "CSV must start with UTF-8 BOM for macOS Numbers compatibility"
        );
    }

    #[test]
    fn test_csv_uses_crlf_and_all_quoted() {
        let result = test_result();
        let csv = to_csv(
            &result,
            &test_rules(),
            &test_sources(),
            &test_patterns(),
            CsvSection::RuleMatches,
        );
        let csv = strip_bom(&csv);
        assert!(csv.contains("\r\n"), "must use CRLF line endings");
        for line in csv.lines() {
            if !line.is_empty() {
                assert!(line.starts_with('"'), "all fields must be quoted: {line}");
            }
        }
    }

    #[test]
    fn test_to_json_has_names() {
        let result = test_result();
        let json = to_json(
            &result,
            &test_rules(),
            &test_sources(),
            &test_patterns(),
            &ExportOptions::default(),
        );
        assert!(json.contains("Timeout"), "should contain rule name");
        assert!(json.contains("app.log"), "should contain source name");
        assert!(json.contains("Service Down"), "should contain pattern name");
        assert!(!json.contains("rule_id"));
        assert!(!json.contains("source_id"));
        assert!(!json.contains("pattern_id"));
    }

    #[test]
    fn test_csv_rule_matches_header_and_data() {
        let result = test_result();
        let csv = to_csv(
            &result,
            &test_rules(),
            &test_sources(),
            &test_patterns(),
            CsvSection::RuleMatches,
        );
        let csv = strip_bom(&csv);
        let lines: Vec<&str> = csv.lines().collect();
        // header + 1 data row
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("timestamp"));
        assert!(lines[0].contains("rule_name"));
        assert!(lines[1].contains("Timeout"));
        assert!(lines[1].contains("app.log"));
    }

    #[test]
    fn test_csv_pattern_matches_header_and_data() {
        let result = test_result();
        let csv = to_csv(
            &result,
            &test_rules(),
            &test_sources(),
            &test_patterns(),
            CsvSection::PatternMatches,
        );
        let csv = strip_bom(&csv);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("pattern_name"));
        assert!(lines[1].contains("Service Down"));
    }

    #[test]
    fn test_csv_state_changes_none_value() {
        let result = test_result();
        let csv_out = to_csv(
            &result,
            &test_rules(),
            &test_sources(),
            &test_patterns(),
            CsvSection::StateChanges,
        );
        let csv_out = strip_bom(&csv_out);
        let lines: Vec<&str> = csv_out.lines().collect();
        assert_eq!(lines.len(), 2);
        // Parse the data row
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_reader(lines[1].as_bytes());
        let record = rdr.records().next().unwrap().unwrap();
        assert_eq!(&record[4], "", "old_value None should be empty");
        assert_eq!(&record[5], "error", "new_value should be 'error'");
    }

    #[test]
    fn test_csv_empty_section() {
        let result = AnalysisResult {
            rule_matches: vec![],
            pattern_matches: vec![],
            state_changes: vec![],
        };
        let csv = to_csv(
            &result,
            &test_rules(),
            &test_sources(),
            &test_patterns(),
            CsvSection::RuleMatches,
        );
        let csv = strip_bom(&csv);
        // Just the header row
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("timestamp"));
    }

    #[test]
    fn test_export_options_excludes_json_sections() {
        let result = test_result();
        let options = ExportOptions {
            rule_matches: true,
            pattern_matches: false,
            state_changes: true,
        };
        let json = to_json(
            &result,
            &test_rules(),
            &test_sources(),
            &test_patterns(),
            &options,
        );
        assert!(json.contains("rule_matches"));
        assert!(!json.contains("pattern_matches"));
        assert!(json.contains("state_changes"));
    }

    #[test]
    fn test_csv_round_trips_through_reader() {
        let mut result = test_result();
        result.rule_matches[0].log_line.content =
            Arc::from("connection from 1.2.3.4, status=\"failed\"");

        let csv_out = to_csv(
            &result,
            &test_rules(),
            &test_sources(),
            &test_patterns(),
            CsvSection::RuleMatches,
        );
        let csv_out = strip_bom(&csv_out);

        let mut rdr = csv::ReaderBuilder::new().from_reader(csv_out.as_bytes());
        let record = rdr.records().next().unwrap().unwrap();
        assert_eq!(&record[0], "2024-01-15 10:30:00");
        assert_eq!(&record[1], "Timeout");
        assert_eq!(&record[2], "app.log");
        assert_eq!(&record[3], "connection from 1.2.3.4, status=\"failed\"");
    }
}
