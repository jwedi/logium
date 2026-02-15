-- Initial schema: all tables with current (latest) column set.
-- Uses CREATE TABLE IF NOT EXISTS so it's safe to run on existing databases.

CREATE TABLE IF NOT EXISTS projects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS timestamp_templates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    format TEXT NOT NULL,
    extraction_regex TEXT,
    default_year INTEGER
);

CREATE TABLE IF NOT EXISTS source_templates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    timestamp_template_id INTEGER NOT NULL REFERENCES timestamp_templates(id),
    line_delimiter TEXT NOT NULL,
    content_regex TEXT,
    continuation_regex TEXT,
    json_timestamp_field TEXT,
    file_name_regex TEXT,
    log_content_regex TEXT
);

CREATE TABLE IF NOT EXISTS sources (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    template_id INTEGER NOT NULL REFERENCES source_templates(id),
    name TEXT NOT NULL,
    file_path TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS rules (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    match_mode TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS match_rules (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    rule_id INTEGER NOT NULL REFERENCES rules(id) ON DELETE CASCADE,
    pattern TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS extraction_rules (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    rule_id INTEGER NOT NULL REFERENCES rules(id) ON DELETE CASCADE,
    extraction_type TEXT NOT NULL,
    state_key TEXT NOT NULL,
    pattern TEXT,
    static_value TEXT,
    mode TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS rulesets (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    template_id INTEGER NOT NULL REFERENCES source_templates(id),
    name TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS ruleset_rules (
    ruleset_id INTEGER NOT NULL REFERENCES rulesets(id) ON DELETE CASCADE,
    rule_id INTEGER NOT NULL REFERENCES rules(id) ON DELETE CASCADE,
    PRIMARY KEY (ruleset_id, rule_id)
);

CREATE TABLE IF NOT EXISTS patterns (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS pattern_predicates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    pattern_id INTEGER NOT NULL REFERENCES patterns(id) ON DELETE CASCADE,
    order_index INTEGER NOT NULL,
    source_name TEXT NOT NULL,
    state_key TEXT NOT NULL,
    operator TEXT NOT NULL,
    operand_type TEXT NOT NULL,
    operand_value TEXT NOT NULL
);
