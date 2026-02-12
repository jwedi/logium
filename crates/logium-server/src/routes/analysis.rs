use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use super::{ApiError, ApiResult};
use crate::AppState;
use crate::db::DbError;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/projects/{project_id}/analyze", post(analyze))
        .route("/api/projects/{project_id}/analyze/ws", get(analyze_ws))
        .route(
            "/api/projects/{project_id}/detect-template",
            post(detect_template),
        )
        .route(
            "/api/projects/{project_id}/suggest-rule",
            post(suggest_rule),
        )
}

async fn analyze(
    State(state): State<AppState>,
    Path(project_id): Path<i64>,
) -> ApiResult<Json<serde_json::Value>> {
    let data = state.db.load_project_data(project_id).await?;

    let result = tokio::task::spawn_blocking(move || {
        logium_core::engine::analyze(
            &data.sources,
            &data.templates,
            &data.timestamp_templates,
            &data.rules,
            &data.rulesets,
            &data.patterns,
        )
    })
    .await
    .map_err(|e| ApiError::from(DbError::InvalidData(format!("task join error: {e}"))))?
    .map_err(|e| ApiError::from(DbError::InvalidData(format!("analysis error: {e}"))))?;

    Ok(Json(serde_json::to_value(result).unwrap()))
}

async fn analyze_ws(
    State(state): State<AppState>,
    Path(project_id): Path<i64>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(move |socket| handle_analysis_ws(socket, state, project_id))
}

async fn handle_analysis_ws(mut socket: WebSocket, state: AppState, project_id: i64) {
    let data = match state.db.load_project_data(project_id).await {
        Ok(d) => d,
        Err(e) => {
            let err_event = logium_core::engine::AnalysisEvent::Error {
                message: format!("failed to load project data: {e}"),
            };
            let _ = socket
                .send(Message::Text(
                    serde_json::to_string(&err_event).unwrap().into(),
                ))
                .await;
            return;
        }
    };

    // std::sync::mpsc channel for the blocking engine -> bridge task
    let (std_tx, std_rx) = std::sync::mpsc::channel();
    // tokio::sync::mpsc channel for bridge task -> async WS loop
    let (tok_tx, mut tok_rx) =
        tokio::sync::mpsc::channel::<logium_core::engine::AnalysisEvent>(256);

    // Spawn the blocking engine
    tokio::task::spawn_blocking(move || {
        let _ = logium_core::engine::analyze_streaming(
            &data.sources,
            &data.templates,
            &data.timestamp_templates,
            &data.rules,
            &data.rulesets,
            &data.patterns,
            std_tx,
        );
    });

    // Bridge std channel -> tokio channel
    tokio::task::spawn_blocking(move || {
        for event in std_rx {
            if tok_tx.blocking_send(event).is_err() {
                break;
            }
        }
    });

    // Async loop: read from tokio channel, send to WS
    while let Some(event) = tok_rx.recv().await {
        let json = serde_json::to_string(&event).unwrap();
        if socket.send(Message::Text(json.into())).await.is_err() {
            break; // client disconnected
        }
    }
}

#[derive(Deserialize)]
struct DetectTemplateRequest {
    sample: String,
}

#[derive(Serialize)]
struct DetectTemplateResponse {
    timestamp_format: Option<String>,
    line_delimiter: String,
    content_regex: Option<String>,
    json_timestamp_field: Option<String>,
    confidence: f64,
}

const TIMESTAMP_FORMATS: &[&str] = &[
    "%Y-%m-%d %H:%M:%S",
    "%Y-%m-%dT%H:%M:%S",
    "%Y-%m-%dT%H:%M:%S%.f",
    "%Y-%m-%d %H:%M:%S%.f",
    "%d/%b/%Y:%H:%M:%S",
    "%b %d %H:%M:%S",
    "%Y/%m/%d %H:%M:%S",
];

async fn detect_template(
    State(_state): State<AppState>,
    Path(_project_id): Path<i64>,
    Json(body): Json<DetectTemplateRequest>,
) -> ApiResult<Json<DetectTemplateResponse>> {
    let lines: Vec<&str> = body.sample.lines().take(20).collect();
    if lines.is_empty() {
        return Err(ApiError::from(DbError::InvalidData(
            "empty sample".to_string(),
        )));
    }

    // Check if majority of lines are JSON
    let json_count = lines
        .iter()
        .filter(|l| l.trim_start().starts_with('{'))
        .count();
    if json_count > lines.len() / 2 {
        // Try to detect JSON timestamp field
        let candidate_fields = ["timestamp", "ts", "@timestamp", "time", "datetime"];
        for line in &lines {
            if let Ok(serde_json::Value::Object(map)) = serde_json::from_str(line) {
                for field in &candidate_fields {
                    if let Some(serde_json::Value::String(val)) = map.get(*field) {
                        // Try parsing against known formats
                        for fmt in TIMESTAMP_FORMATS {
                            if try_parse_timestamp(val, fmt) {
                                return Ok(Json(DetectTemplateResponse {
                                    timestamp_format: Some(fmt.to_string()),
                                    line_delimiter: "\n".to_string(),
                                    content_regex: None,
                                    json_timestamp_field: Some(field.to_string()),
                                    confidence: json_count as f64 / lines.len() as f64,
                                }));
                            }
                        }
                    }
                }
            }
        }
    }

    let mut best_format: Option<&str> = None;
    let mut best_score: usize = 0;

    for fmt in TIMESTAMP_FORMATS {
        let mut matched = 0usize;
        for line in &lines {
            if try_parse_timestamp(line, fmt) {
                matched += 1;
            }
        }
        if matched > best_score {
            best_score = matched;
            best_format = Some(fmt);
        }
    }

    let total = lines.len();
    let confidence = best_score as f64 / total as f64;

    let content_regex = best_format.and_then(|fmt| {
        let prefix_len = estimate_timestamp_len(fmt);
        if prefix_len > 0 {
            Some(format!(r"^.{{{prefix_len}}}\s*(.+)$"))
        } else {
            None
        }
    });

    Ok(Json(DetectTemplateResponse {
        timestamp_format: best_format.map(|s| s.to_string()),
        line_delimiter: "\n".to_string(),
        content_regex,
        json_timestamp_field: None,
        confidence,
    }))
}

fn try_parse_timestamp(line: &str, fmt: &str) -> bool {
    let min_len = fmt.len().min(line.len());
    for end in (min_len..=line.len()).rev() {
        if !line.is_char_boundary(end) {
            continue;
        }
        if chrono::NaiveDateTime::parse_from_str(&line[..end], fmt).is_ok() {
            return true;
        }
    }
    false
}

fn estimate_timestamp_len(fmt: &str) -> usize {
    let mut len = 0;
    let mut chars = fmt.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            if let Some(&spec) = chars.peek() {
                chars.next();
                len += match spec {
                    'Y' => 4,
                    'm' | 'd' | 'H' | 'M' | 'S' => 2,
                    'b' => 3,
                    'f' => 3,
                    _ => 2,
                };
            }
        } else {
            len += 1;
        }
    }
    len
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct SuggestRuleRequest {
    text: String,
    #[serde(default)]
    context_lines: Vec<String>,
}

#[derive(Serialize)]
struct SuggestRuleResponse {
    pattern: String,
    capture_groups: Vec<String>,
}

async fn suggest_rule(
    State(_state): State<AppState>,
    Path(_project_id): Path<i64>,
    Json(body): Json<SuggestRuleRequest>,
) -> ApiResult<Json<SuggestRuleResponse>> {
    let (pattern, groups) = build_suggested_pattern(&body.text);
    Ok(Json(SuggestRuleResponse {
        pattern,
        capture_groups: groups,
    }))
}

fn build_suggested_pattern(text: &str) -> (String, Vec<String>) {
    let mut pattern = String::new();
    let mut groups = Vec::new();
    let mut group_idx = 0;

    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_ascii_digit() {
            // Consume the full number
            while let Some(&next) = chars.peek() {
                if next.is_ascii_digit() || next == '.' {
                    chars.next();
                } else {
                    break;
                }
            }
            let group_name = format!("num_{group_idx}");
            pattern.push_str(&format!("(?P<{group_name}>\\d[\\d.]*)"));
            groups.push(group_name);
            group_idx += 1;
        } else if c == '"' {
            // Quoted string
            while let Some(&next) = chars.peek() {
                chars.next();
                if next == '"' {
                    break;
                }
            }
            let group_name = format!("str_{group_idx}");
            pattern.push_str(&format!("\"(?P<{group_name}>[^\"]*)\""));
            groups.push(group_name);
            group_idx += 1;
        } else {
            if is_regex_special(c) {
                pattern.push('\\');
            }
            pattern.push(c);
        }
    }

    (pattern, groups)
}

fn is_regex_special(c: char) -> bool {
    matches!(
        c,
        '.' | '^' | '$' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '\\'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_suggested_pattern_numbers() {
        let (pat, groups) = build_suggested_pattern("error code 42 at line 100");
        assert!(pat.contains("(?P<num_0>\\d[\\d.]*)"));
        assert!(pat.contains("(?P<num_1>\\d[\\d.]*)"));
        assert_eq!(groups.len(), 2);
    }

    #[test]
    fn test_build_suggested_pattern_quoted_string() {
        let (pat, groups) = build_suggested_pattern(r#"message "hello world" received"#);
        assert!(pat.contains(r#""(?P<str_0>[^"]*)"#));
        assert_eq!(groups.len(), 1);
    }

    #[test]
    fn test_build_suggested_pattern_regex_escaping() {
        let (pat, _) = build_suggested_pattern("file.log");
        assert!(pat.contains(r"\."));
    }

    #[test]
    fn test_estimate_timestamp_len() {
        assert_eq!(estimate_timestamp_len("%Y-%m-%d %H:%M:%S"), 19);
    }

    #[test]
    fn test_try_parse_timestamp() {
        assert!(try_parse_timestamp(
            "2024-01-15 10:30:45 some content",
            "%Y-%m-%d %H:%M:%S"
        ));
        assert!(!try_parse_timestamp("not a timestamp", "%Y-%m-%d %H:%M:%S"));
    }
}
