use criterion::{Criterion, criterion_group, criterion_main};
use logium_core::engine::analyze;
use logium_core::model::*;
use std::path::PathBuf;

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

fn bench_nginx_pipeline(c: &mut Criterion) {
    let ts = TimestampTemplate {
        id: 1,
        name: "nginx_ts".into(),
        format: "%d/%b/%Y:%H:%M:%S".into(),
        extraction_regex: Some(r"\[(\d{2}/\w{3}/\d{4}:\d{2}:\d{2}:\d{2})".into()),
        default_year: None,
    };
    let tmpl = SourceTemplate {
        id: 1,
        name: "nginx".into(),
        timestamp_template_id: 1,
        line_delimiter: "\n".into(),
        content_regex: None,
        continuation_regex: None,
        json_timestamp_field: None,
    };
    let src_a = Source {
        id: 1,
        name: "source_a".into(),
        template_id: 1,
        file_path: fixture_path("nginx", "source_a.log"),
    };
    let src_b = Source {
        id: 2,
        name: "source_b".into(),
        template_id: 1,
        file_path: fixture_path("nginx", "source_b.log"),
    };
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
    let method_rule = LogRule {
        id: 2,
        name: "detect_method".into(),
        match_mode: MatchMode::Any,
        match_rules: vec![MatchRule {
            id: 2,
            pattern: r"GET|POST|PUT".into(),
        }],
        extraction_rules: vec![ExtractionRule {
            id: 2,
            extraction_type: ExtractionType::Static,
            state_key: "method_seen".into(),
            pattern: None,
            static_value: Some("true".into()),
            mode: ExtractionMode::Replace,
        }],
    };
    let ruleset = Ruleset {
        id: 1,
        name: "nginx_rules".into(),
        template_id: 1,
        rule_ids: vec![1, 2],
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

    c.bench_function("nginx_cross_source_1k_each", |b| {
        b.iter(|| {
            analyze(
                &[src_a.clone(), src_b.clone()],
                std::slice::from_ref(&tmpl),
                std::slice::from_ref(&ts),
                &[status_rule.clone(), method_rule.clone()],
                std::slice::from_ref(&ruleset),
                std::slice::from_ref(&pattern),
            )
            .unwrap()
        });
    });
}

fn bench_nginx_large(c: &mut Criterion) {
    let ts = TimestampTemplate {
        id: 1,
        name: "nginx_ts".into(),
        format: "%d/%b/%Y:%H:%M:%S".into(),
        extraction_regex: Some(r"\[(\d{2}/\w{3}/\d{4}:\d{2}:\d{2}:\d{2})".into()),
        default_year: None,
    };
    let tmpl = SourceTemplate {
        id: 1,
        name: "nginx".into(),
        timestamp_template_id: 1,
        line_delimiter: "\n".into(),
        content_regex: None,
        continuation_regex: None,
        json_timestamp_field: None,
    };
    let src = Source {
        id: 1,
        name: "nginx_full".into(),
        template_id: 1,
        file_path: fixture_path("nginx", "full_large.log"),
    };
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
        name: "detect_404".into(),
        predicates: vec![PatternPredicate {
            source_name: "nginx_full".into(),
            state_key: "status".into(),
            operator: Operator::Eq,
            operand: Operand::Literal(StateValue::Integer(404)),
        }],
    };

    c.bench_function("nginx_large_51k_lines", |b| {
        b.iter(|| {
            analyze(
                std::slice::from_ref(&src),
                std::slice::from_ref(&tmpl),
                std::slice::from_ref(&ts),
                std::slice::from_ref(&status_rule),
                std::slice::from_ref(&ruleset),
                std::slice::from_ref(&pattern),
            )
            .unwrap()
        });
    });
}

criterion_group!(benches, bench_nginx_pipeline, bench_nginx_large);
criterion_main!(benches);
