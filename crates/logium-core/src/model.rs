use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

/// Value types for state and predicates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StateValue {
    String(String),
    Integer(i64),
    Float(f64),
    Bool(bool),
}

impl fmt::Display for StateValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StateValue::String(s) => write!(f, "{s}"),
            StateValue::Integer(i) => write!(f, "{i}"),
            StateValue::Float(v) => write!(f, "{v}"),
            StateValue::Bool(b) => write!(f, "{b}"),
        }
    }
}

impl PartialEq for StateValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (StateValue::String(a), StateValue::String(b)) => a == b,
            (StateValue::Integer(a), StateValue::Integer(b)) => a == b,
            (StateValue::Float(a), StateValue::Float(b)) => a == b,
            (StateValue::Bool(a), StateValue::Bool(b)) => a == b,
            // Cross-type: attempt numeric comparison
            (StateValue::Integer(a), StateValue::Float(b)) => (*a as f64) == *b,
            (StateValue::Float(a), StateValue::Integer(b)) => *a == (*b as f64),
            _ => false,
        }
    }
}

impl PartialOrd for StateValue {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (StateValue::String(a), StateValue::String(b)) => a.partial_cmp(b),
            (StateValue::Integer(a), StateValue::Integer(b)) => a.partial_cmp(b),
            (StateValue::Float(a), StateValue::Float(b)) => a.partial_cmp(b),
            (StateValue::Bool(a), StateValue::Bool(b)) => a.partial_cmp(b),
            (StateValue::Integer(a), StateValue::Float(b)) => (*a as f64).partial_cmp(b),
            (StateValue::Float(a), StateValue::Integer(b)) => a.partial_cmp(&(*b as f64)),
            _ => None,
        }
    }
}

/// A state value paired with the timestamp of the log line that last set it.
/// TODO: Add `line_number: Option<u64>` for source-line provenance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedValue {
    pub value: StateValue,
    pub set_at: NaiveDateTime,
}

/// Timestamp template - describes how to parse timestamps from log lines.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimestampTemplate {
    pub id: u64,
    pub name: String,
    pub format: String,
    pub extraction_regex: Option<String>,
    pub default_year: Option<i32>,
}

/// Source template - describes how to read a log source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceTemplate {
    pub id: u64,
    pub name: String,
    pub timestamp_template_id: u64,
    pub line_delimiter: String,
    pub content_regex: Option<String>,
    pub continuation_regex: Option<String>,
    pub json_timestamp_field: Option<String>,
}

/// Source - an actual log file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub id: u64,
    pub name: String,
    pub template_id: u64,
    pub file_path: String,
}

/// A parsed log line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogLine {
    pub timestamp: NaiveDateTime,
    pub source_id: u64,
    pub raw: Arc<str>,
    pub content: Arc<str>,
}

/// Match modes for log rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MatchMode {
    Any,
    All,
}

/// Match rule - regex to test against log line content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchRule {
    pub id: u64,
    pub pattern: String,
}

/// Extraction types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExtractionType {
    Parsed,
    Static,
    Clear,
}

/// Extraction modes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExtractionMode {
    Replace,
    Accumulate,
}

/// Extraction rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionRule {
    pub id: u64,
    pub extraction_type: ExtractionType,
    pub state_key: String,
    pub pattern: Option<String>,
    pub static_value: Option<String>,
    pub mode: ExtractionMode,
}

/// A log rule combining match rules and extraction rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogRule {
    pub id: u64,
    pub name: String,
    pub match_mode: MatchMode,
    pub match_rules: Vec<MatchRule>,
    pub extraction_rules: Vec<ExtractionRule>,
}

/// A ruleset binding rules to a template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ruleset {
    pub id: u64,
    pub name: String,
    pub template_id: u64,
    pub rule_ids: Vec<u64>,
}

/// Predicate operators.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operator {
    Eq,
    Neq,
    Gt,
    Lt,
    Gte,
    Lte,
    Contains,
    Exists,
}

/// Operand - can be literal value or reference to another source's state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operand {
    Literal(StateValue),
    StateRef {
        source_name: String,
        state_key: String,
    },
}

/// A single predicate in a pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternPredicate {
    pub source_name: String,
    pub state_key: String,
    pub operator: Operator,
    pub operand: Operand,
}

/// A pattern consisting of an ordered sequence of predicates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pattern {
    pub id: u64,
    pub name: String,
    pub predicates: Vec<PatternPredicate>,
}

/// Result of a rule match on a specific log line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleMatch {
    pub rule_id: u64,
    pub source_id: u64,
    pub log_line: LogLine,
    pub extracted_state: HashMap<String, StateValue>,
}

/// Result of a pattern matching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternMatch {
    pub pattern_id: u64,
    pub timestamp: NaiveDateTime,
    pub state_snapshot: HashMap<String, HashMap<String, TrackedValue>>,
}

/// A state change event emitted when a mutation modifies per-source state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateChange {
    pub timestamp: NaiveDateTime,
    pub source_id: u64,
    pub source_name: String,
    pub state_key: String,
    pub old_value: Option<StateValue>,
    pub new_value: Option<StateValue>,
    pub rule_id: u64,
}

/// Combined analysis result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub rule_matches: Vec<RuleMatch>,
    pub pattern_matches: Vec<PatternMatch>,
    pub state_changes: Vec<StateChange>,
}

/// A cluster of log lines sharing the same structural template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogCluster {
    pub template: String,
    pub count: u64,
    pub source_ids: Vec<u64>,
    pub sample_lines: Vec<String>,
}

/// Result of clustering all log lines.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterResult {
    pub clusters: Vec<LogCluster>,
    pub total_lines: u64,
}
