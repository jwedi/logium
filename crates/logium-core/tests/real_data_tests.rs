use logium_core::engine::{LogLineIterator, analyze};
use logium_core::model::*;

use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn fixture_path(format: &str, file: &str) -> String {
    fixtures_dir()
        .join(format)
        .join(file)
        .to_str()
        .unwrap()
        .to_string()
}

fn make_ts_template(
    id: u64,
    name: &str,
    format: &str,
    extraction_regex: Option<&str>,
    default_year: Option<i32>,
) -> TimestampTemplate {
    TimestampTemplate {
        id,
        name: name.into(),
        format: format.into(),
        extraction_regex: extraction_regex.map(|s| s.into()),
        default_year,
    }
}

fn make_source_template(
    id: u64,
    name: &str,
    ts_template_id: u64,
    content_regex: Option<&str>,
) -> SourceTemplate {
    SourceTemplate {
        id,
        name: name.into(),
        timestamp_template_id: ts_template_id,
        line_delimiter: "\n".into(),
        content_regex: content_regex.map(|s| s.into()),
    }
}

fn make_source(id: u64, name: &str, path: &str, template_id: u64) -> Source {
    Source {
        id,
        name: name.into(),
        template_id,
        file_path: path.into(),
    }
}

// ---------------------------------------------------------------------------
// Per-format parsing tests
// ---------------------------------------------------------------------------

#[test]
fn test_zookeeper_parsing() {
    let ts = make_ts_template(1, "zk_ts", "%Y-%m-%d %H:%M:%S", None, None);
    let tmpl = make_source_template(1, "zk", 1, None);
    let src = make_source(1, "zk_full", &fixture_path("zookeeper", "full.log"), 1);

    let iter = LogLineIterator::new(&src, &tmpl, &ts).unwrap();
    let mut count = 0;
    for result in iter.take(100) {
        let line = result.expect("line should parse");
        assert!(
            line.timestamp.and_utc().year() == 2015,
            "expected year 2015, got {}",
            line.timestamp.and_utc().year()
        );
        count += 1;
    }
    assert_eq!(count, 100);
}

#[test]
fn test_nginx_parsing() {
    let ts = make_ts_template(
        1,
        "nginx_ts",
        "%d/%b/%Y:%H:%M:%S",
        Some(r"\[(\d{2}/\w{3}/\d{4}:\d{2}:\d{2}:\d{2})"),
        None,
    );
    let tmpl = make_source_template(1, "nginx", 1, None);
    let src = make_source(1, "nginx_full", &fixture_path("nginx", "full.log"), 1);

    let iter = LogLineIterator::new(&src, &tmpl, &ts).unwrap();
    let mut count = 0;
    for result in iter.take(100) {
        let line = result.expect("line should parse");
        assert!(
            line.timestamp.and_utc().year() == 2015,
            "expected year 2015, got {} for line: {}",
            line.timestamp.and_utc().year(),
            line.raw
        );
        count += 1;
    }
    assert_eq!(count, 100);
}

#[test]
fn test_syslog_parsing() {
    let ts = make_ts_template(1, "syslog_ts", "%b %d %H:%M:%S", None, Some(2005));
    let tmpl = make_source_template(1, "syslog", 1, None);
    let src = make_source(1, "syslog_full", &fixture_path("syslog", "full.log"), 1);

    let iter = LogLineIterator::new(&src, &tmpl, &ts).unwrap();
    let mut count = 0;
    for result in iter.take(100) {
        let line = result.expect("line should parse");
        assert_eq!(
            line.timestamp.and_utc().year(),
            2005,
            "expected year 2005, got {} for line: {}",
            line.timestamp.and_utc().year(),
            line.raw
        );
        count += 1;
    }
    assert_eq!(count, 100);
}

// ---------------------------------------------------------------------------
// Cross-source analysis tests
// ---------------------------------------------------------------------------

#[test]
fn test_zookeeper_cross_source() {
    let ts = make_ts_template(1, "zk_ts", "%Y-%m-%d %H:%M:%S", None, None);
    let tmpl = make_source_template(1, "zk", 1, None);
    let src_a = make_source(1, "source_a", &fixture_path("zookeeper", "source_a.log"), 1);
    let src_b = make_source(2, "source_b", &fixture_path("zookeeper", "source_b.log"), 1);

    let warn_rule = LogRule {
        id: 1,
        name: "detect_warn".into(),
        match_mode: MatchMode::Any,
        match_rules: vec![MatchRule {
            id: 1,
            pattern: r"WARN".into(),
        }],
        extraction_rules: vec![ExtractionRule {
            id: 1,
            extraction_type: ExtractionType::Static,
            state_key: "level".into(),
            pattern: None,
            static_value: Some("warn".into()),
            mode: ExtractionMode::Replace,
        }],
    };

    let connection_rule = LogRule {
        id: 2,
        name: "detect_connection".into(),
        match_mode: MatchMode::Any,
        match_rules: vec![MatchRule {
            id: 2,
            pattern: r"Connection broken|connection request".into(),
        }],
        extraction_rules: vec![ExtractionRule {
            id: 2,
            extraction_type: ExtractionType::Static,
            state_key: "connection_event".into(),
            pattern: None,
            static_value: Some("true".into()),
            mode: ExtractionMode::Replace,
        }],
    };

    let ruleset = Ruleset {
        id: 1,
        name: "zk_rules".into(),
        template_id: 1,
        rule_ids: vec![1, 2],
    };

    let pattern = Pattern {
        id: 1,
        name: "warn_and_connection".into(),
        predicates: vec![
            PatternPredicate {
                source_name: "source_a".into(),
                state_key: "level".into(),
                operator: Operator::Eq,
                operand: Operand::Literal(StateValue::String("warn".into())),
            },
            PatternPredicate {
                source_name: "source_b".into(),
                state_key: "connection_event".into(),
                operator: Operator::Eq,
                operand: Operand::Literal(StateValue::String("true".into())),
            },
        ],
    };

    let result = analyze(
        &[src_a, src_b],
        &[tmpl],
        &[ts],
        &[warn_rule, connection_rule],
        &[ruleset],
        &[pattern],
    )
    .unwrap();

    assert!(
        !result.rule_matches.is_empty(),
        "expected rule matches from zookeeper logs"
    );
    // Verify rule matches come from both sources
    let source_ids: std::collections::HashSet<u64> =
        result.rule_matches.iter().map(|m| m.source_id).collect();
    assert!(source_ids.contains(&1), "expected matches from source_a");
    assert!(source_ids.contains(&2), "expected matches from source_b");
}

#[test]
fn test_nginx_cross_source() {
    let ts = make_ts_template(
        1,
        "nginx_ts",
        "%d/%b/%Y:%H:%M:%S",
        Some(r"\[(\d{2}/\w{3}/\d{4}:\d{2}:\d{2}:\d{2})"),
        None,
    );
    let tmpl = make_source_template(1, "nginx", 1, None);
    let src_a = make_source(1, "source_a", &fixture_path("nginx", "source_a.log"), 1);
    let src_b = make_source(2, "source_b", &fixture_path("nginx", "source_b.log"), 1);

    let status_rule = LogRule {
        id: 1,
        name: "extract_status".into(),
        match_mode: MatchMode::Any,
        match_rules: vec![MatchRule {
            id: 1,
            pattern: r#"HTTP/1\.\d"\s+\d+"#.into(),
        }],
        extraction_rules: vec![ExtractionRule {
            id: 1,
            extraction_type: ExtractionType::Parsed,
            state_key: "status".into(),
            pattern: Some(r#"HTTP/1\.\d"\s+(?P<status>\d+)"#.into()),
            static_value: None,
            mode: ExtractionMode::Replace,
        }],
    };

    let ruleset = Ruleset {
        id: 1,
        name: "nginx_rules".into(),
        template_id: 1,
        rule_ids: vec![1],
    };

    let pattern = Pattern {
        id: 1,
        name: "both_404".into(),
        predicates: vec![
            PatternPredicate {
                source_name: "source_a".into(),
                state_key: "status".into(),
                operator: Operator::Eq,
                operand: Operand::Literal(StateValue::Integer(404)),
            },
            PatternPredicate {
                source_name: "source_b".into(),
                state_key: "status".into(),
                operator: Operator::Eq,
                operand: Operand::Literal(StateValue::Integer(404)),
            },
        ],
    };

    let result = analyze(
        &[src_a, src_b],
        &[tmpl],
        &[ts],
        &[status_rule],
        &[ruleset],
        &[pattern],
    )
    .unwrap();

    assert!(
        !result.rule_matches.is_empty(),
        "expected rule matches from nginx logs"
    );
    assert!(
        !result.pattern_matches.is_empty(),
        "expected pattern matches (404s in both halves)"
    );
    // Verify matches come from both sources
    let source_ids: std::collections::HashSet<u64> =
        result.rule_matches.iter().map(|m| m.source_id).collect();
    assert!(source_ids.contains(&1), "expected matches from source_a");
    assert!(source_ids.contains(&2), "expected matches from source_b");
}

#[test]
fn test_syslog_cross_source() {
    let ts = make_ts_template(1, "syslog_ts", "%b %d %H:%M:%S", None, Some(2005));
    let tmpl = make_source_template(1, "syslog", 1, None);
    let src_a = make_source(1, "source_a", &fixture_path("syslog", "source_a.log"), 1);
    let src_b = make_source(2, "source_b", &fixture_path("syslog", "source_b.log"), 1);

    let auth_rule = LogRule {
        id: 1,
        name: "detect_auth_failure".into(),
        match_mode: MatchMode::Any,
        match_rules: vec![MatchRule {
            id: 1,
            pattern: r"authentication failure".into(),
        }],
        extraction_rules: vec![ExtractionRule {
            id: 1,
            extraction_type: ExtractionType::Static,
            state_key: "auth_failed".into(),
            pattern: None,
            static_value: Some("true".into()),
            mode: ExtractionMode::Replace,
        }],
    };

    let rhost_rule = LogRule {
        id: 2,
        name: "extract_rhost".into(),
        match_mode: MatchMode::Any,
        match_rules: vec![MatchRule {
            id: 2,
            pattern: r"rhost=\S+".into(),
        }],
        extraction_rules: vec![ExtractionRule {
            id: 2,
            extraction_type: ExtractionType::Parsed,
            state_key: "rhost".into(),
            pattern: Some(r"rhost=(?P<rhost>\S+)".into()),
            static_value: None,
            mode: ExtractionMode::Replace,
        }],
    };

    let ruleset = Ruleset {
        id: 1,
        name: "syslog_rules".into(),
        template_id: 1,
        rule_ids: vec![1, 2],
    };

    let pattern = Pattern {
        id: 1,
        name: "both_auth_fail".into(),
        predicates: vec![
            PatternPredicate {
                source_name: "source_a".into(),
                state_key: "auth_failed".into(),
                operator: Operator::Eq,
                operand: Operand::Literal(StateValue::String("true".into())),
            },
            PatternPredicate {
                source_name: "source_b".into(),
                state_key: "auth_failed".into(),
                operator: Operator::Eq,
                operand: Operand::Literal(StateValue::String("true".into())),
            },
        ],
    };

    let result = analyze(
        &[src_a, src_b],
        &[tmpl],
        &[ts],
        &[auth_rule, rhost_rule],
        &[ruleset],
        &[pattern],
    )
    .unwrap();

    assert!(
        !result.rule_matches.is_empty(),
        "expected rule matches from syslog"
    );
    assert!(
        !result.pattern_matches.is_empty(),
        "expected pattern matches (auth failures across both sources)"
    );
}

// ---------------------------------------------------------------------------
// Template reuse test
// ---------------------------------------------------------------------------

#[test]
fn test_timestamp_template_reuse() {
    // One TimestampTemplate shared by two SourceTemplates with different content_regex
    let ts = make_ts_template(1, "shared_ts", "%Y-%m-%d %H:%M:%S", None, None);
    let tmpl_a = SourceTemplate {
        id: 1,
        name: "tmpl_a".into(),
        timestamp_template_id: 1,
        line_delimiter: "\n".into(),
        content_regex: Some(r"^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2},\d+ - (.+)$".into()),
    };
    let tmpl_b = SourceTemplate {
        id: 2,
        name: "tmpl_b".into(),
        timestamp_template_id: 1,
        line_delimiter: "\n".into(),
        content_regex: None,
    };

    let src_a = make_source(1, "source_a", &fixture_path("zookeeper", "source_a.log"), 1);
    let src_b = make_source(2, "source_b", &fixture_path("zookeeper", "source_b.log"), 2);

    let rule = LogRule {
        id: 1,
        name: "detect_info".into(),
        match_mode: MatchMode::Any,
        match_rules: vec![MatchRule {
            id: 1,
            pattern: r"INFO".into(),
        }],
        extraction_rules: vec![ExtractionRule {
            id: 1,
            extraction_type: ExtractionType::Static,
            state_key: "seen".into(),
            pattern: None,
            static_value: Some("true".into()),
            mode: ExtractionMode::Replace,
        }],
    };

    let rs_a = Ruleset {
        id: 1,
        name: "rs_a".into(),
        template_id: 1,
        rule_ids: vec![1],
    };
    let rs_b = Ruleset {
        id: 2,
        name: "rs_b".into(),
        template_id: 2,
        rule_ids: vec![1],
    };

    let pattern = Pattern {
        id: 1,
        name: "both_seen".into(),
        predicates: vec![
            PatternPredicate {
                source_name: "source_a".into(),
                state_key: "seen".into(),
                operator: Operator::Eq,
                operand: Operand::Literal(StateValue::String("true".into())),
            },
            PatternPredicate {
                source_name: "source_b".into(),
                state_key: "seen".into(),
                operator: Operator::Eq,
                operand: Operand::Literal(StateValue::String("true".into())),
            },
        ],
    };

    let result = analyze(
        &[src_a, src_b],
        &[tmpl_a, tmpl_b],
        &[ts],
        &[rule],
        &[rs_a, rs_b],
        &[pattern],
    )
    .unwrap();

    // Both sources should parse correctly and produce matches
    let source_ids: std::collections::HashSet<u64> =
        result.rule_matches.iter().map(|m| m.source_id).collect();
    assert!(source_ids.contains(&1), "expected matches from source_a");
    assert!(source_ids.contains(&2), "expected matches from source_b");
    assert!(
        !result.pattern_matches.is_empty(),
        "expected pattern matches with shared timestamp template"
    );
}

// ---------------------------------------------------------------------------
// Cross-source state reference test
// ---------------------------------------------------------------------------

#[test]
fn test_cross_source_state_ref() {
    let ts = make_ts_template(1, "zk_ts", "%Y-%m-%d %H:%M:%S", None, None);
    let tmpl = make_source_template(1, "zk", 1, None);
    let src_a = make_source(1, "source_a", &fixture_path("zookeeper", "source_a.log"), 1);
    let src_b = make_source(2, "source_b", &fixture_path("zookeeper", "source_b.log"), 1);

    // Extract log level from both sources
    let level_rule = LogRule {
        id: 1,
        name: "extract_level".into(),
        match_mode: MatchMode::Any,
        match_rules: vec![MatchRule {
            id: 1,
            pattern: r"(INFO|WARN|ERROR)".into(),
        }],
        extraction_rules: vec![ExtractionRule {
            id: 1,
            extraction_type: ExtractionType::Parsed,
            state_key: "level".into(),
            pattern: Some(r"(?P<level>INFO|WARN|ERROR)".into()),
            static_value: None,
            mode: ExtractionMode::Replace,
        }],
    };

    let ruleset = Ruleset {
        id: 1,
        name: "level_rules".into(),
        template_id: 1,
        rule_ids: vec![1],
    };

    // Pattern: source_a.level == StateRef(source_b.level) — both at same level
    let pattern = Pattern {
        id: 1,
        name: "same_level".into(),
        predicates: vec![PatternPredicate {
            source_name: "source_a".into(),
            state_key: "level".into(),
            operator: Operator::Eq,
            operand: Operand::StateRef {
                source_name: "source_b".into(),
                state_key: "level".into(),
            },
        }],
    };

    let result = analyze(
        &[src_a, src_b],
        &[tmpl],
        &[ts],
        &[level_rule],
        &[ruleset],
        &[pattern],
    )
    .unwrap();

    assert!(
        !result.rule_matches.is_empty(),
        "expected rule matches for level extraction"
    );
    assert!(
        !result.pattern_matches.is_empty(),
        "expected pattern matches when both sources have same level"
    );
}

// Needed for .year() calls
use chrono::Datelike;
