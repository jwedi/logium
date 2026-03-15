use std::collections::HashMap;
use std::str::FromStr;

use chrono::Datelike;
use sqlx::Row;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};

use logium_core::model::*;

use crate::routes::import_export::{ImportResult, ProjectExport};

#[derive(Debug)]
pub enum DbError {
    Sqlx(sqlx::Error),
    Migrate(sqlx::migrate::MigrateError),
    NotFound,
    InvalidData(String),
}

impl From<sqlx::Error> for DbError {
    fn from(e: sqlx::Error) -> Self {
        DbError::Sqlx(e)
    }
}

impl From<sqlx::migrate::MigrateError> for DbError {
    fn from(e: sqlx::migrate::MigrateError) -> Self {
        DbError::Migrate(e)
    }
}

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbError::Sqlx(e) => write!(f, "database error: {e}"),
            DbError::Migrate(e) => write!(f, "migration error: {e}"),
            DbError::NotFound => write!(f, "not found"),
            DbError::InvalidData(s) => write!(f, "invalid data: {s}"),
        }
    }
}

impl std::error::Error for DbError {}

#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn new(url: &str) -> Result<Self, DbError> {
        let options = SqliteConnectOptions::from_str(url)?
            .pragma("foreign_keys", "ON")
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;
        let db = Self { pool };
        db.legacy_fixup().await?;
        sqlx::migrate!("./migrations").run(&db.pool).await?;
        Ok(db)
    }

    /// Handle pre-sqlx-migration databases. Runs before `sqlx::migrate!()`.
    async fn legacy_fixup(&self) -> Result<(), DbError> {
        // If _sqlx_migrations exists, we've already bootstrapped — nothing to do.
        let has_sqlx_table = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='_sqlx_migrations'",
        )
        .fetch_one(&self.pool)
        .await?;
        if has_sqlx_table > 0 {
            return Ok(());
        }

        // If projects table doesn't exist, this is a fresh DB — nothing to do.
        let has_projects = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='projects'",
        )
        .fetch_one(&self.pool)
        .await?;
        if has_projects == 0 {
            return Ok(());
        }

        // Legacy database: fix schema before sqlx migrations run.
        self.legacy_fix_timestamp_template().await?;
        self.legacy_add_column_if_missing("source_templates", "continuation_regex", "TEXT")
            .await?;
        self.legacy_add_column_if_missing("source_templates", "json_timestamp_field", "TEXT")
            .await?;
        self.legacy_add_column_if_missing("source_templates", "file_name_regex", "TEXT")
            .await?;
        self.legacy_add_column_if_missing("source_templates", "log_content_regex", "TEXT")
            .await?;

        Ok(())
    }

    /// Fix legacy databases that have `timestamp_format` in `source_templates`.
    /// Handles both V1 (only old column) and partially-migrated (both columns) states.
    async fn legacy_fix_timestamp_template(&self) -> Result<(), DbError> {
        // Check if source_templates exists at all.
        let has_table = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='source_templates'",
        )
        .fetch_one(&self.pool)
        .await?;
        if has_table == 0 {
            return Ok(());
        }

        let cols = sqlx::query("PRAGMA table_info(source_templates)")
            .fetch_all(&self.pool)
            .await?;
        let has_old = cols
            .iter()
            .any(|r| sqlx::Row::get::<String, _>(r, "name") == "timestamp_format");
        let has_new = cols
            .iter()
            .any(|r| sqlx::Row::get::<String, _>(r, "name") == "timestamp_template_id");

        if !has_old {
            // No old column — nothing to fix.
            return Ok(());
        }

        // Ensure timestamp_templates table exists.
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS timestamp_templates (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                name TEXT NOT NULL,
                format TEXT NOT NULL,
                extraction_regex TEXT,
                default_year INTEGER
            )",
        )
        .execute(&self.pool)
        .await?;

        // Seed default timestamp templates for every project that has none.
        let project_ids = sqlx::query_scalar::<_, i64>("SELECT id FROM projects")
            .fetch_all(&self.pool)
            .await?;
        for pid in &project_ids {
            let count = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM timestamp_templates WHERE project_id = ?",
            )
            .bind(pid)
            .fetch_one(&self.pool)
            .await?;
            if count == 0 {
                self.seed_default_timestamp_templates(*pid).await?;
            }
        }

        // Add the new column if missing (nullable for now so we can populate it).
        if !has_new {
            sqlx::query(
                "ALTER TABLE source_templates ADD COLUMN timestamp_template_id INTEGER REFERENCES timestamp_templates(id)",
            )
            .execute(&self.pool)
            .await?;
        }

        // Populate any NULL timestamp_template_id values from old format strings.
        let rows = sqlx::query(
            "SELECT id, project_id, timestamp_format FROM source_templates WHERE timestamp_template_id IS NULL",
        )
        .fetch_all(&self.pool)
        .await?;

        for row in &rows {
            let st_id: i64 = sqlx::Row::get(row, "id");
            let project_id: i64 = sqlx::Row::get(row, "project_id");
            let ts_format: String = sqlx::Row::get(row, "timestamp_format");

            let tt_id = sqlx::query_scalar::<_, i64>(
                "SELECT id FROM timestamp_templates WHERE project_id = ? AND format = ? LIMIT 1",
            )
            .bind(project_id)
            .bind(&ts_format)
            .fetch_optional(&self.pool)
            .await?;

            let tt_id = match tt_id {
                Some(id) => id,
                None => {
                    sqlx::query_scalar::<_, i64>(
                        "INSERT INTO timestamp_templates (project_id, name, format) VALUES (?, ?, ?) RETURNING id",
                    )
                    .bind(project_id)
                    .bind(format!("Migrated: {ts_format}"))
                    .bind(&ts_format)
                    .fetch_one(&self.pool)
                    .await?
                }
            };

            sqlx::query("UPDATE source_templates SET timestamp_template_id = ? WHERE id = ?")
                .bind(tt_id)
                .bind(st_id)
                .execute(&self.pool)
                .await?;
        }

        // Drop the old column so its NOT NULL constraint no longer blocks inserts.
        sqlx::query("ALTER TABLE source_templates DROP COLUMN timestamp_format")
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Add a column to an existing table if it doesn't already exist.
    async fn legacy_add_column_if_missing(
        &self,
        table: &str,
        column: &str,
        col_type: &str,
    ) -> Result<(), DbError> {
        let has_table = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?",
        )
        .bind(table)
        .fetch_one(&self.pool)
        .await?;
        if has_table == 0 {
            return Ok(());
        }

        let rows = sqlx::query(&format!("PRAGMA table_info({table})"))
            .fetch_all(&self.pool)
            .await?;
        let exists = rows
            .iter()
            .any(|r| sqlx::Row::get::<String, _>(r, "name") == column);
        if !exists {
            sqlx::query(&format!(
                "ALTER TABLE {table} ADD COLUMN {column} {col_type}"
            ))
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Projects
    // -----------------------------------------------------------------------

    pub async fn list_projects(&self) -> Result<Vec<ProjectRow>, DbError> {
        let rows = sqlx::query_as::<_, ProjectRow>(
            "SELECT id, name, created_at FROM projects ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_project(&self, id: i64) -> Result<ProjectRow, DbError> {
        sqlx::query_as::<_, ProjectRow>("SELECT id, name, created_at FROM projects WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(DbError::NotFound)
    }

    pub async fn create_project(&self, name: &str) -> Result<ProjectRow, DbError> {
        let created_at = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO projects (name, created_at) VALUES (?, ?) RETURNING id",
        )
        .bind(name)
        .bind(&created_at)
        .fetch_one(&self.pool)
        .await?;
        self.seed_default_timestamp_templates(id).await?;
        Ok(ProjectRow {
            id,
            name: name.to_string(),
            created_at,
        })
    }

    async fn seed_default_timestamp_templates(&self, project_id: i64) -> Result<(), DbError> {
        let current_year = chrono::Utc::now().year();
        let defaults: &[(&str, &str, Option<&str>, Option<i32>)] = &[
            ("ISO 8601", "%Y-%m-%dT%H:%M:%S", None, None),
            ("ISO 8601 (millis)", "%Y-%m-%dT%H:%M:%S%.f", None, None),
            ("Standard Datetime", "%Y-%m-%d %H:%M:%S", None, None),
            (
                "Standard Datetime (millis)",
                "%Y-%m-%d %H:%M:%S%.f",
                None,
                None,
            ),
            (
                "Apache/Nginx",
                "%d/%b/%Y:%H:%M:%S",
                Some(r"\[(\d{2}/\w{3}/\d{4}:\d{2}:\d{2}:\d{2})"),
                None,
            ),
            (
                "Syslog (RFC 3164)",
                "%b %d %H:%M:%S",
                None,
                Some(current_year),
            ),
        ];
        for (name, format, regex, year) in defaults {
            self.create_timestamp_template(project_id, name, format, *regex, *year)
                .await?;
        }
        Ok(())
    }

    pub async fn update_project(&self, id: i64, name: &str) -> Result<ProjectRow, DbError> {
        let result = sqlx::query("UPDATE projects SET name = ? WHERE id = ?")
            .bind(name)
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }
        self.get_project(id).await
    }

    pub async fn delete_project(&self, id: i64) -> Result<(), DbError> {
        let result = sqlx::query("DELETE FROM projects WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Source Templates
    // -----------------------------------------------------------------------

    // -----------------------------------------------------------------------
    // Timestamp Templates
    // -----------------------------------------------------------------------

    pub async fn list_timestamp_templates(
        &self,
        project_id: i64,
    ) -> Result<Vec<TimestampTemplate>, DbError> {
        let rows = sqlx::query(
            "SELECT id, name, format, extraction_regex, default_year
             FROM timestamp_templates WHERE project_id = ? ORDER BY id",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(row_to_timestamp_template).collect())
    }

    pub async fn get_timestamp_template(
        &self,
        project_id: i64,
        id: i64,
    ) -> Result<TimestampTemplate, DbError> {
        let row = sqlx::query(
            "SELECT id, name, format, extraction_regex, default_year
             FROM timestamp_templates WHERE id = ? AND project_id = ?",
        )
        .bind(id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(DbError::NotFound)?;

        Ok(row_to_timestamp_template(&row))
    }

    pub async fn create_timestamp_template(
        &self,
        project_id: i64,
        name: &str,
        format: &str,
        extraction_regex: Option<&str>,
        default_year: Option<i32>,
    ) -> Result<TimestampTemplate, DbError> {
        let id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO timestamp_templates (project_id, name, format, extraction_regex, default_year)
             VALUES (?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(project_id)
        .bind(name)
        .bind(format)
        .bind(extraction_regex)
        .bind(default_year)
        .fetch_one(&self.pool)
        .await?;

        Ok(TimestampTemplate {
            id: id as u64,
            name: name.to_string(),
            format: format.to_string(),
            extraction_regex: extraction_regex.map(|s| s.to_string()),
            default_year,
        })
    }

    pub async fn update_timestamp_template(
        &self,
        project_id: i64,
        id: i64,
        name: &str,
        format: &str,
        extraction_regex: Option<&str>,
        default_year: Option<i32>,
    ) -> Result<TimestampTemplate, DbError> {
        let result = sqlx::query(
            "UPDATE timestamp_templates SET name = ?, format = ?, extraction_regex = ?, default_year = ?
             WHERE id = ? AND project_id = ?",
        )
        .bind(name)
        .bind(format)
        .bind(extraction_regex)
        .bind(default_year)
        .bind(id)
        .bind(project_id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }
        self.get_timestamp_template(project_id, id).await
    }

    pub async fn delete_timestamp_template(&self, project_id: i64, id: i64) -> Result<(), DbError> {
        let result = sqlx::query("DELETE FROM timestamp_templates WHERE id = ? AND project_id = ?")
            .bind(id)
            .bind(project_id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Source Templates
    // -----------------------------------------------------------------------

    pub async fn list_templates(&self, project_id: i64) -> Result<Vec<SourceTemplate>, DbError> {
        let rows = sqlx::query(
            "SELECT id, name, timestamp_template_id, line_delimiter, content_regex, continuation_regex, json_timestamp_field, file_name_regex, log_content_regex
             FROM source_templates WHERE project_id = ? ORDER BY id",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(row_to_template).collect())
    }

    pub async fn get_template(&self, project_id: i64, id: i64) -> Result<SourceTemplate, DbError> {
        let row = sqlx::query(
            "SELECT id, name, timestamp_template_id, line_delimiter, content_regex, continuation_regex, json_timestamp_field, file_name_regex, log_content_regex
             FROM source_templates WHERE id = ? AND project_id = ?",
        )
        .bind(id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(DbError::NotFound)?;

        Ok(row_to_template(&row))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_template(
        &self,
        project_id: i64,
        name: &str,
        timestamp_template_id: i64,
        line_delimiter: &str,
        content_regex: Option<&str>,
        continuation_regex: Option<&str>,
        json_timestamp_field: Option<&str>,
        file_name_regex: Option<&str>,
        log_content_regex: Option<&str>,
    ) -> Result<SourceTemplate, DbError> {
        let id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO source_templates (project_id, name, timestamp_template_id, line_delimiter, content_regex, continuation_regex, json_timestamp_field, file_name_regex, log_content_regex)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(project_id)
        .bind(name)
        .bind(timestamp_template_id)
        .bind(line_delimiter)
        .bind(content_regex)
        .bind(continuation_regex)
        .bind(json_timestamp_field)
        .bind(file_name_regex)
        .bind(log_content_regex)
        .fetch_one(&self.pool)
        .await?;

        // Auto-create default ruleset for the new template
        let ruleset_name = format!("Default — {}", name);
        sqlx::query("INSERT INTO rulesets (project_id, template_id, name) VALUES (?, ?, ?)")
            .bind(project_id)
            .bind(id)
            .bind(&ruleset_name)
            .execute(&self.pool)
            .await?;

        Ok(SourceTemplate {
            id: id as u64,
            name: name.to_string(),
            timestamp_template_id: timestamp_template_id as u64,
            line_delimiter: line_delimiter.to_string(),
            content_regex: content_regex.map(|s| s.to_string()),
            continuation_regex: continuation_regex.map(|s| s.to_string()),
            json_timestamp_field: json_timestamp_field.map(|s| s.to_string()),
            file_name_regex: file_name_regex.map(|s| s.to_string()),
            log_content_regex: log_content_regex.map(|s| s.to_string()),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update_template(
        &self,
        project_id: i64,
        id: i64,
        name: &str,
        timestamp_template_id: i64,
        line_delimiter: &str,
        content_regex: Option<&str>,
        continuation_regex: Option<&str>,
        json_timestamp_field: Option<&str>,
        file_name_regex: Option<&str>,
        log_content_regex: Option<&str>,
    ) -> Result<SourceTemplate, DbError> {
        let result = sqlx::query(
            "UPDATE source_templates SET name = ?, timestamp_template_id = ?, line_delimiter = ?, content_regex = ?, continuation_regex = ?, json_timestamp_field = ?, file_name_regex = ?, log_content_regex = ?
             WHERE id = ? AND project_id = ?",
        )
        .bind(name)
        .bind(timestamp_template_id)
        .bind(line_delimiter)
        .bind(content_regex)
        .bind(continuation_regex)
        .bind(json_timestamp_field)
        .bind(file_name_regex)
        .bind(log_content_regex)
        .bind(id)
        .bind(project_id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }
        self.get_template(project_id, id).await
    }

    pub async fn delete_template(&self, project_id: i64, id: i64) -> Result<(), DbError> {
        // Delete associated rulesets first (FK on template_id has no CASCADE)
        sqlx::query("DELETE FROM rulesets WHERE template_id = ? AND project_id = ?")
            .bind(id)
            .bind(project_id)
            .execute(&self.pool)
            .await?;

        let result = sqlx::query("DELETE FROM source_templates WHERE id = ? AND project_id = ?")
            .bind(id)
            .bind(project_id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Sources
    // -----------------------------------------------------------------------

    pub async fn list_sources(&self, project_id: i64) -> Result<Vec<Source>, DbError> {
        let rows = sqlx::query(
            "SELECT id, name, template_id, file_path, color FROM sources WHERE project_id = ? ORDER BY id",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(row_to_source).collect())
    }

    pub async fn get_source(&self, project_id: i64, id: i64) -> Result<Source, DbError> {
        let row = sqlx::query(
            "SELECT id, name, template_id, file_path, color FROM sources WHERE id = ? AND project_id = ?",
        )
        .bind(id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(DbError::NotFound)?;

        Ok(row_to_source(&row))
    }

    pub async fn create_source(
        &self,
        project_id: i64,
        template_id: i64,
        name: &str,
        file_path: &str,
        color: &str,
    ) -> Result<Source, DbError> {
        let id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO sources (project_id, template_id, name, file_path, color)
             VALUES (?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(project_id)
        .bind(template_id)
        .bind(name)
        .bind(file_path)
        .bind(color)
        .fetch_one(&self.pool)
        .await?;

        Ok(Source {
            id: id as u64,
            name: name.to_string(),
            template_id: template_id as u64,
            file_path: file_path.to_string(),
            color: color.to_string(),
        })
    }

    pub async fn update_source_color(
        &self,
        project_id: i64,
        id: i64,
        color: &str,
    ) -> Result<(), DbError> {
        let result = sqlx::query("UPDATE sources SET color = ? WHERE id = ? AND project_id = ?")
            .bind(color)
            .bind(id)
            .bind(project_id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }
        Ok(())
    }

    pub async fn update_source_file_path(
        &self,
        project_id: i64,
        id: i64,
        file_path: &str,
    ) -> Result<(), DbError> {
        let result =
            sqlx::query("UPDATE sources SET file_path = ? WHERE id = ? AND project_id = ?")
                .bind(file_path)
                .bind(id)
                .bind(project_id)
                .execute(&self.pool)
                .await?;
        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }
        Ok(())
    }

    pub async fn delete_source(&self, project_id: i64, id: i64) -> Result<(), DbError> {
        let result = sqlx::query("DELETE FROM sources WHERE id = ? AND project_id = ?")
            .bind(id)
            .bind(project_id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Rules (with match_rules and extraction_rules)
    // -----------------------------------------------------------------------

    pub async fn list_rules(&self, project_id: i64) -> Result<Vec<LogRule>, DbError> {
        let rule_rows = sqlx::query(
            "SELECT id, name, match_mode, event_text FROM rules WHERE project_id = ? ORDER BY id",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;

        let mut rules = Vec::with_capacity(rule_rows.len());
        for row in &rule_rows {
            let rule_id: i64 = row.get("id");
            rules.push(self.build_log_rule(row, rule_id).await?);
        }
        Ok(rules)
    }

    pub async fn get_rule(&self, project_id: i64, id: i64) -> Result<LogRule, DbError> {
        let row = sqlx::query(
            "SELECT id, name, match_mode, event_text FROM rules WHERE id = ? AND project_id = ?",
        )
        .bind(id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(DbError::NotFound)?;

        self.build_log_rule(&row, id).await
    }

    async fn build_log_rule(
        &self,
        row: &sqlx::sqlite::SqliteRow,
        rule_id: i64,
    ) -> Result<LogRule, DbError> {
        let name: String = row.get("name");
        let match_mode_str: String = row.get("match_mode");
        let match_mode = parse_match_mode(&match_mode_str)?;
        let event_text: Option<String> = row.get("event_text");

        let match_rows =
            sqlx::query("SELECT id, pattern FROM match_rules WHERE rule_id = ? ORDER BY id")
                .bind(rule_id)
                .fetch_all(&self.pool)
                .await?;

        let match_rules: Vec<MatchRule> = match_rows
            .iter()
            .map(|r| MatchRule {
                id: r.get::<i64, _>("id") as u64,
                pattern: r.get("pattern"),
            })
            .collect();

        let ext_rows = sqlx::query(
            "SELECT id, extraction_type, state_key, pattern, static_value, mode, event_text_only
             FROM extraction_rules WHERE rule_id = ? ORDER BY id",
        )
        .bind(rule_id)
        .fetch_all(&self.pool)
        .await?;

        let extraction_rules: Result<Vec<ExtractionRule>, DbError> = ext_rows
            .iter()
            .map(|r| {
                let ext_type_str: String = r.get("extraction_type");
                let mode_str: String = r.get("mode");
                Ok(ExtractionRule {
                    id: r.get::<i64, _>("id") as u64,
                    extraction_type: parse_extraction_type(&ext_type_str)?,
                    state_key: r.get("state_key"),
                    pattern: r.get("pattern"),
                    static_value: r.get("static_value"),
                    mode: parse_extraction_mode(&mode_str)?,
                    event_text_only: r.get::<i64, _>("event_text_only") != 0,
                })
            })
            .collect();

        Ok(LogRule {
            id: rule_id as u64,
            name,
            match_mode,
            match_rules,
            extraction_rules: extraction_rules?,
            event_text,
        })
    }

    pub async fn create_rule(
        &self,
        project_id: i64,
        name: &str,
        match_mode: &MatchMode,
        match_rules: &[CreateMatchRule],
        extraction_rules: &[CreateExtractionRule],
        event_text: Option<&str>,
    ) -> Result<LogRule, DbError> {
        let mode_str = match_mode_to_str(match_mode);
        let rule_id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO rules (project_id, name, match_mode, event_text) VALUES (?, ?, ?, ?) RETURNING id",
        )
        .bind(project_id)
        .bind(name)
        .bind(mode_str)
        .bind(event_text)
        .fetch_one(&self.pool)
        .await?;

        let mut built_match_rules = Vec::with_capacity(match_rules.len());
        for mr in match_rules {
            let id = sqlx::query_scalar::<_, i64>(
                "INSERT INTO match_rules (rule_id, pattern) VALUES (?, ?) RETURNING id",
            )
            .bind(rule_id)
            .bind(&mr.pattern)
            .fetch_one(&self.pool)
            .await?;
            built_match_rules.push(MatchRule {
                id: id as u64,
                pattern: mr.pattern.clone(),
            });
        }

        let mut built_ext_rules = Vec::with_capacity(extraction_rules.len());
        for er in extraction_rules {
            let ext_type_str = extraction_type_to_str(&er.extraction_type);
            let mode_str = extraction_mode_to_str(&er.mode);
            let id = sqlx::query_scalar::<_, i64>(
                "INSERT INTO extraction_rules (rule_id, extraction_type, state_key, pattern, static_value, mode, event_text_only)
                 VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
            )
            .bind(rule_id)
            .bind(ext_type_str)
            .bind(&er.state_key)
            .bind(er.pattern.as_deref())
            .bind(er.static_value.as_deref())
            .bind(mode_str)
            .bind(er.event_text_only as i64)
            .fetch_one(&self.pool)
            .await?;
            built_ext_rules.push(ExtractionRule {
                id: id as u64,
                extraction_type: er.extraction_type.clone(),
                state_key: er.state_key.clone(),
                pattern: er.pattern.clone(),
                static_value: er.static_value.clone(),
                mode: er.mode.clone(),
                event_text_only: er.event_text_only,
            });
        }

        Ok(LogRule {
            id: rule_id as u64,
            name: name.to_string(),
            match_mode: match_mode.clone(),
            match_rules: built_match_rules,
            extraction_rules: built_ext_rules,
            event_text: event_text.map(|s| s.to_string()),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update_rule(
        &self,
        project_id: i64,
        id: i64,
        name: &str,
        match_mode: &MatchMode,
        match_rules: &[CreateMatchRule],
        extraction_rules: &[CreateExtractionRule],
        event_text: Option<&str>,
    ) -> Result<LogRule, DbError> {
        let mode_str = match_mode_to_str(match_mode);
        let result = sqlx::query(
            "UPDATE rules SET name = ?, match_mode = ?, event_text = ? WHERE id = ? AND project_id = ?",
        )
        .bind(name)
        .bind(mode_str)
        .bind(event_text)
        .bind(id)
        .bind(project_id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        // Delete old sub-rules and re-create
        sqlx::query("DELETE FROM match_rules WHERE rule_id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM extraction_rules WHERE rule_id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        let mut built_match_rules = Vec::with_capacity(match_rules.len());
        for mr in match_rules {
            let mr_id = sqlx::query_scalar::<_, i64>(
                "INSERT INTO match_rules (rule_id, pattern) VALUES (?, ?) RETURNING id",
            )
            .bind(id)
            .bind(&mr.pattern)
            .fetch_one(&self.pool)
            .await?;
            built_match_rules.push(MatchRule {
                id: mr_id as u64,
                pattern: mr.pattern.clone(),
            });
        }

        let mut built_ext_rules = Vec::with_capacity(extraction_rules.len());
        for er in extraction_rules {
            let ext_type_str = extraction_type_to_str(&er.extraction_type);
            let mode_str = extraction_mode_to_str(&er.mode);
            let er_id = sqlx::query_scalar::<_, i64>(
                "INSERT INTO extraction_rules (rule_id, extraction_type, state_key, pattern, static_value, mode, event_text_only)
                 VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
            )
            .bind(id)
            .bind(ext_type_str)
            .bind(&er.state_key)
            .bind(er.pattern.as_deref())
            .bind(er.static_value.as_deref())
            .bind(mode_str)
            .bind(er.event_text_only as i64)
            .fetch_one(&self.pool)
            .await?;
            built_ext_rules.push(ExtractionRule {
                id: er_id as u64,
                extraction_type: er.extraction_type.clone(),
                state_key: er.state_key.clone(),
                pattern: er.pattern.clone(),
                static_value: er.static_value.clone(),
                mode: er.mode.clone(),
                event_text_only: er.event_text_only,
            });
        }

        Ok(LogRule {
            id: id as u64,
            name: name.to_string(),
            match_mode: match_mode.clone(),
            match_rules: built_match_rules,
            extraction_rules: built_ext_rules,
            event_text: event_text.map(|s| s.to_string()),
        })
    }

    pub async fn delete_rule(&self, project_id: i64, id: i64) -> Result<(), DbError> {
        let result = sqlx::query("DELETE FROM rules WHERE id = ? AND project_id = ?")
            .bind(id)
            .bind(project_id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Rulesets
    // -----------------------------------------------------------------------

    pub async fn list_rulesets(&self, project_id: i64) -> Result<Vec<Ruleset>, DbError> {
        let rows = sqlx::query(
            "SELECT id, name, template_id FROM rulesets WHERE project_id = ? ORDER BY id",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;

        let mut rulesets = Vec::with_capacity(rows.len());
        for row in &rows {
            let id: i64 = row.get("id");
            let rule_ids = self.get_ruleset_rule_ids(id).await?;
            rulesets.push(Ruleset {
                id: id as u64,
                name: row.get("name"),
                template_id: row.get::<i64, _>("template_id") as u64,
                rule_ids,
            });
        }
        Ok(rulesets)
    }

    pub async fn get_ruleset(&self, project_id: i64, id: i64) -> Result<Ruleset, DbError> {
        let row = sqlx::query(
            "SELECT id, name, template_id FROM rulesets WHERE id = ? AND project_id = ?",
        )
        .bind(id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(DbError::NotFound)?;

        let rule_ids = self.get_ruleset_rule_ids(id).await?;
        Ok(Ruleset {
            id: id as u64,
            name: row.get("name"),
            template_id: row.get::<i64, _>("template_id") as u64,
            rule_ids,
        })
    }

    async fn get_ruleset_rule_ids(&self, ruleset_id: i64) -> Result<Vec<u64>, DbError> {
        let rows =
            sqlx::query("SELECT rule_id FROM ruleset_rules WHERE ruleset_id = ? ORDER BY rule_id")
                .bind(ruleset_id)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows
            .iter()
            .map(|r| r.get::<i64, _>("rule_id") as u64)
            .collect())
    }

    pub async fn create_ruleset(
        &self,
        project_id: i64,
        name: &str,
        template_id: i64,
        rule_ids: &[i64],
    ) -> Result<Ruleset, DbError> {
        let id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO rulesets (project_id, template_id, name) VALUES (?, ?, ?) RETURNING id",
        )
        .bind(project_id)
        .bind(template_id)
        .bind(name)
        .fetch_one(&self.pool)
        .await?;

        for rule_id in rule_ids {
            sqlx::query("INSERT INTO ruleset_rules (ruleset_id, rule_id) VALUES (?, ?)")
                .bind(id)
                .bind(rule_id)
                .execute(&self.pool)
                .await?;
        }

        Ok(Ruleset {
            id: id as u64,
            name: name.to_string(),
            template_id: template_id as u64,
            rule_ids: rule_ids.iter().map(|&i| i as u64).collect(),
        })
    }

    pub async fn update_ruleset(
        &self,
        project_id: i64,
        id: i64,
        name: &str,
        template_id: i64,
        rule_ids: &[i64],
    ) -> Result<Ruleset, DbError> {
        let result = sqlx::query(
            "UPDATE rulesets SET name = ?, template_id = ? WHERE id = ? AND project_id = ?",
        )
        .bind(name)
        .bind(template_id)
        .bind(id)
        .bind(project_id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        sqlx::query("DELETE FROM ruleset_rules WHERE ruleset_id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        for rule_id in rule_ids {
            sqlx::query("INSERT INTO ruleset_rules (ruleset_id, rule_id) VALUES (?, ?)")
                .bind(id)
                .bind(rule_id)
                .execute(&self.pool)
                .await?;
        }

        Ok(Ruleset {
            id: id as u64,
            name: name.to_string(),
            template_id: template_id as u64,
            rule_ids: rule_ids.iter().map(|&i| i as u64).collect(),
        })
    }

    pub async fn delete_ruleset(&self, project_id: i64, id: i64) -> Result<(), DbError> {
        let result = sqlx::query("DELETE FROM rulesets WHERE id = ? AND project_id = ?")
            .bind(id)
            .bind(project_id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Patterns (with predicates)
    // -----------------------------------------------------------------------

    pub async fn list_patterns(&self, project_id: i64) -> Result<Vec<Pattern>, DbError> {
        let rows = sqlx::query("SELECT id, name FROM patterns WHERE project_id = ? ORDER BY id")
            .bind(project_id)
            .fetch_all(&self.pool)
            .await?;

        let mut patterns = Vec::with_capacity(rows.len());
        for row in &rows {
            let id: i64 = row.get("id");
            let predicates = self.get_predicates(id).await?;
            patterns.push(Pattern {
                id: id as u64,
                name: row.get("name"),
                predicates,
            });
        }
        Ok(patterns)
    }

    pub async fn get_pattern(&self, project_id: i64, id: i64) -> Result<Pattern, DbError> {
        let row = sqlx::query("SELECT id, name FROM patterns WHERE id = ? AND project_id = ?")
            .bind(id)
            .bind(project_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(DbError::NotFound)?;

        let predicates = self.get_predicates(id).await?;
        Ok(Pattern {
            id: id as u64,
            name: row.get("name"),
            predicates,
        })
    }

    async fn get_predicates(&self, pattern_id: i64) -> Result<Vec<PatternPredicate>, DbError> {
        let rows = sqlx::query(
            "SELECT ruleset_name, state_key, operator, operand_type, operand_value
             FROM pattern_predicates WHERE pattern_id = ? ORDER BY order_index",
        )
        .bind(pattern_id)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(row_to_predicate).collect()
    }

    pub async fn create_pattern(
        &self,
        project_id: i64,
        name: &str,
        predicates: &[CreatePredicate],
    ) -> Result<Pattern, DbError> {
        let id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO patterns (project_id, name) VALUES (?, ?) RETURNING id",
        )
        .bind(project_id)
        .bind(name)
        .fetch_one(&self.pool)
        .await?;

        let built = self.insert_predicates(id, predicates).await?;
        Ok(Pattern {
            id: id as u64,
            name: name.to_string(),
            predicates: built,
        })
    }

    pub async fn update_pattern(
        &self,
        project_id: i64,
        id: i64,
        name: &str,
        predicates: &[CreatePredicate],
    ) -> Result<Pattern, DbError> {
        let result = sqlx::query("UPDATE patterns SET name = ? WHERE id = ? AND project_id = ?")
            .bind(name)
            .bind(id)
            .bind(project_id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        sqlx::query("DELETE FROM pattern_predicates WHERE pattern_id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        let built = self.insert_predicates(id, predicates).await?;
        Ok(Pattern {
            id: id as u64,
            name: name.to_string(),
            predicates: built,
        })
    }

    async fn insert_predicates(
        &self,
        pattern_id: i64,
        predicates: &[CreatePredicate],
    ) -> Result<Vec<PatternPredicate>, DbError> {
        let mut built = Vec::with_capacity(predicates.len());
        for (idx, p) in predicates.iter().enumerate() {
            let (operand_type, operand_value) = serialize_operand(&p.operand);
            sqlx::query(
                "INSERT INTO pattern_predicates (pattern_id, order_index, ruleset_name, state_key, operator, operand_type, operand_value)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(pattern_id)
            .bind(idx as i64)
            .bind(&p.ruleset_name)
            .bind(&p.state_key)
            .bind(operator_to_str(&p.operator))
            .bind(operand_type)
            .bind(operand_value)
            .execute(&self.pool)
            .await?;

            built.push(PatternPredicate {
                ruleset_name: p.ruleset_name.clone(),
                state_key: p.state_key.clone(),
                operator: p.operator.clone(),
                operand: p.operand.clone(),
            });
        }
        Ok(built)
    }

    pub async fn delete_pattern(&self, project_id: i64, id: i64) -> Result<(), DbError> {
        let result = sqlx::query("DELETE FROM patterns WHERE id = ? AND project_id = ?")
            .bind(id)
            .bind(project_id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Import project configuration
    // -----------------------------------------------------------------------

    pub async fn import_project_config(
        &self,
        project_id: i64,
        data: &ProjectExport,
    ) -> Result<ImportResult, DbError> {
        let mut tt_id_map: HashMap<u64, u64> = HashMap::new();
        let mut st_id_map: HashMap<u64, u64> = HashMap::new();
        let mut rule_id_map: HashMap<u64, u64> = HashMap::new();

        // 1. TimestampTemplates (no FK deps)
        for tt in &data.timestamp_templates {
            let new_tt = self
                .create_timestamp_template(
                    project_id,
                    &tt.name,
                    &tt.format,
                    tt.extraction_regex.as_deref(),
                    tt.default_year,
                )
                .await?;
            tt_id_map.insert(tt.id, new_tt.id);
        }

        // 2. SourceTemplates (FK: timestamp_template_id)
        for st in &data.source_templates {
            let new_tt_id = tt_id_map.get(&st.timestamp_template_id).ok_or_else(|| {
                DbError::InvalidData(format!(
                    "source template '{}' references unknown timestamp_template_id {}",
                    st.name, st.timestamp_template_id
                ))
            })?;
            let new_st = self
                .create_template(
                    project_id,
                    &st.name,
                    *new_tt_id as i64,
                    &st.line_delimiter,
                    st.content_regex.as_deref(),
                    st.continuation_regex.as_deref(),
                    st.json_timestamp_field.as_deref(),
                    st.file_name_regex.as_deref(),
                    st.log_content_regex.as_deref(),
                )
                .await?;
            st_id_map.insert(st.id, new_st.id);
        }

        // 3. Rules (match_rules/extraction_rules are sub-entities)
        for rule in &data.rules {
            let create_match_rules: Vec<CreateMatchRule> = rule
                .match_rules
                .iter()
                .map(|mr| CreateMatchRule {
                    pattern: mr.pattern.clone(),
                })
                .collect();
            let create_ext_rules: Vec<CreateExtractionRule> = rule
                .extraction_rules
                .iter()
                .map(|er| CreateExtractionRule {
                    extraction_type: er.extraction_type.clone(),
                    state_key: er.state_key.clone(),
                    pattern: er.pattern.clone(),
                    static_value: er.static_value.clone(),
                    mode: er.mode.clone(),
                    event_text_only: er.event_text_only,
                })
                .collect();
            let new_rule = self
                .create_rule(
                    project_id,
                    &rule.name,
                    &rule.match_mode,
                    &create_match_rules,
                    &create_ext_rules,
                    rule.event_text.as_deref(),
                )
                .await?;
            rule_id_map.insert(rule.id, new_rule.id);
        }

        // 4. Rulesets (FK: template_id, rule_ids)
        for rs in &data.rulesets {
            let new_st_id = st_id_map.get(&rs.template_id).ok_or_else(|| {
                DbError::InvalidData(format!(
                    "ruleset '{}' references unknown template_id {}",
                    rs.name, rs.template_id
                ))
            })?;
            let remapped_rule_ids: Vec<i64> = rs
                .rule_ids
                .iter()
                .map(|old_id| {
                    rule_id_map
                        .get(old_id)
                        .map(|&new_id| new_id as i64)
                        .ok_or_else(|| {
                            DbError::InvalidData(format!(
                                "ruleset '{}' references unknown rule_id {}",
                                rs.name, old_id
                            ))
                        })
                })
                .collect::<Result<_, _>>()?;
            self.create_ruleset(project_id, &rs.name, *new_st_id as i64, &remapped_rule_ids)
                .await?;
        }

        // 5. Patterns (predicates use ruleset_name strings, no ID FKs)
        for pattern in &data.patterns {
            let create_predicates: Vec<CreatePredicate> = pattern
                .predicates
                .iter()
                .map(|p| CreatePredicate {
                    ruleset_name: p.ruleset_name.clone(),
                    state_key: p.state_key.clone(),
                    operator: p.operator.clone(),
                    operand: p.operand.clone(),
                })
                .collect();
            self.create_pattern(project_id, &pattern.name, &create_predicates)
                .await?;
        }

        Ok(ImportResult {
            timestamp_templates: data.timestamp_templates.len(),
            source_templates: data.source_templates.len(),
            rules: data.rules.len(),
            rulesets: data.rulesets.len(),
            patterns: data.patterns.len(),
        })
    }

    // -----------------------------------------------------------------------
    // Clone project
    // -----------------------------------------------------------------------

    pub async fn clone_project(
        &self,
        source_id: i64,
        new_name: &str,
    ) -> Result<ProjectRow, DbError> {
        let created_at = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let new_project_id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO projects (name, created_at) VALUES (?, ?) RETURNING id",
        )
        .bind(new_name)
        .bind(&created_at)
        .fetch_one(&self.pool)
        .await?;
        self.seed_default_timestamp_templates(new_project_id)
            .await?;

        let data = self.load_project_data(source_id).await?;

        let mut tt_id_map: HashMap<u64, u64> = HashMap::new();
        let mut st_id_map: HashMap<u64, u64> = HashMap::new();
        let mut rule_id_map: HashMap<u64, u64> = HashMap::new();

        // 1. Timestamp templates
        for tt in &data.timestamp_templates {
            let new_tt = self
                .create_timestamp_template(
                    new_project_id,
                    &tt.name,
                    &tt.format,
                    tt.extraction_regex.as_deref(),
                    tt.default_year,
                )
                .await?;
            tt_id_map.insert(tt.id, new_tt.id);
        }

        // 2. Source templates (remap timestamp_template_id)
        for st in &data.templates {
            let new_tt_id = tt_id_map.get(&st.timestamp_template_id).ok_or_else(|| {
                DbError::InvalidData(format!(
                    "source template '{}' references unknown tt_id {}",
                    st.name, st.timestamp_template_id
                ))
            })?;
            let new_st = self
                .create_template(
                    new_project_id,
                    &st.name,
                    *new_tt_id as i64,
                    &st.line_delimiter,
                    st.content_regex.as_deref(),
                    st.continuation_regex.as_deref(),
                    st.json_timestamp_field.as_deref(),
                    st.file_name_regex.as_deref(),
                    st.log_content_regex.as_deref(),
                )
                .await?;
            st_id_map.insert(st.id, new_st.id);
        }

        // 3. Rules (with nested match/extraction rules)
        for rule in &data.rules {
            let create_match_rules: Vec<CreateMatchRule> = rule
                .match_rules
                .iter()
                .map(|mr| CreateMatchRule {
                    pattern: mr.pattern.clone(),
                })
                .collect();
            let create_ext_rules: Vec<CreateExtractionRule> = rule
                .extraction_rules
                .iter()
                .map(|er| CreateExtractionRule {
                    extraction_type: er.extraction_type.clone(),
                    state_key: er.state_key.clone(),
                    pattern: er.pattern.clone(),
                    static_value: er.static_value.clone(),
                    mode: er.mode.clone(),
                    event_text_only: er.event_text_only,
                })
                .collect();
            let new_rule = self
                .create_rule(
                    new_project_id,
                    &rule.name,
                    &rule.match_mode,
                    &create_match_rules,
                    &create_ext_rules,
                    rule.event_text.as_deref(),
                )
                .await?;
            rule_id_map.insert(rule.id, new_rule.id);
        }

        // 4. Rulesets (remap template_id and rule_ids)
        for rs in &data.rulesets {
            let new_st_id = st_id_map.get(&rs.template_id).ok_or_else(|| {
                DbError::InvalidData(format!(
                    "ruleset '{}' references unknown template_id {}",
                    rs.name, rs.template_id
                ))
            })?;
            let remapped_rule_ids: Vec<i64> = rs
                .rule_ids
                .iter()
                .map(|old_id| {
                    rule_id_map
                        .get(old_id)
                        .map(|&new_id| new_id as i64)
                        .ok_or_else(|| {
                            DbError::InvalidData(format!(
                                "ruleset '{}' references unknown rule_id {}",
                                rs.name, old_id
                            ))
                        })
                })
                .collect::<Result<_, _>>()?;
            self.create_ruleset(
                new_project_id,
                &rs.name,
                *new_st_id as i64,
                &remapped_rule_ids,
            )
            .await?;
        }

        // 5. Patterns (predicates reference ruleset names as strings — no ID remapping needed)
        for pattern in &data.patterns {
            let create_predicates: Vec<CreatePredicate> = pattern
                .predicates
                .iter()
                .map(|p| CreatePredicate {
                    ruleset_name: p.ruleset_name.clone(),
                    state_key: p.state_key.clone(),
                    operator: p.operator.clone(),
                    operand: p.operand.clone(),
                })
                .collect();
            self.create_pattern(new_project_id, &pattern.name, &create_predicates)
                .await?;
        }

        // 6. Sources (remap template_id; copy file_path and color verbatim)
        for source in &data.sources {
            let new_template_id = st_id_map.get(&source.template_id).ok_or_else(|| {
                DbError::InvalidData(format!(
                    "source '{}' references unknown template_id {}",
                    source.name, source.template_id
                ))
            })?;
            self.create_source(
                new_project_id,
                *new_template_id as i64,
                &source.name,
                &source.file_path,
                &source.color,
            )
            .await?;
        }

        Ok(ProjectRow {
            id: new_project_id,
            name: new_name.to_string(),
            created_at,
        })
    }

    // -----------------------------------------------------------------------
    // Load all project data (for analysis)
    // -----------------------------------------------------------------------

    pub async fn load_project_data(&self, project_id: i64) -> Result<ProjectData, DbError> {
        let timestamp_templates = self.list_timestamp_templates(project_id).await?;
        let templates = self.list_templates(project_id).await?;
        let sources = self.list_sources(project_id).await?;
        let rules = self.list_rules(project_id).await?;
        let rulesets = self.list_rulesets(project_id).await?;
        let patterns = self.list_patterns(project_id).await?;
        Ok(ProjectData {
            timestamp_templates,
            templates,
            sources,
            rules,
            rulesets,
            patterns,
        })
    }
}

// ---------------------------------------------------------------------------
// Row types and conversion helpers
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct ProjectRow {
    pub id: i64,
    pub name: String,
    pub created_at: String,
}

pub struct ProjectData {
    pub timestamp_templates: Vec<TimestampTemplate>,
    pub templates: Vec<SourceTemplate>,
    pub sources: Vec<Source>,
    pub rules: Vec<LogRule>,
    pub rulesets: Vec<Ruleset>,
    pub patterns: Vec<Pattern>,
}

/// Input type for creating match rules (no id yet).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CreateMatchRule {
    pub pattern: String,
}

/// Input type for creating extraction rules (no id yet).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CreateExtractionRule {
    pub extraction_type: ExtractionType,
    pub state_key: String,
    pub pattern: Option<String>,
    pub static_value: Option<String>,
    pub mode: ExtractionMode,
    #[serde(default)]
    pub event_text_only: bool,
}

/// Input type for creating pattern predicates (no id yet).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CreatePredicate {
    pub ruleset_name: String,
    pub state_key: String,
    pub operator: Operator,
    pub operand: Operand,
}

fn row_to_timestamp_template(row: &sqlx::sqlite::SqliteRow) -> TimestampTemplate {
    TimestampTemplate {
        id: row.get::<i64, _>("id") as u64,
        name: row.get("name"),
        format: row.get("format"),
        extraction_regex: row.get("extraction_regex"),
        default_year: row.get("default_year"),
    }
}

fn row_to_template(row: &sqlx::sqlite::SqliteRow) -> SourceTemplate {
    SourceTemplate {
        id: row.get::<i64, _>("id") as u64,
        name: row.get("name"),
        timestamp_template_id: row.get::<i64, _>("timestamp_template_id") as u64,
        line_delimiter: row.get("line_delimiter"),
        content_regex: row.get("content_regex"),
        continuation_regex: row.get("continuation_regex"),
        json_timestamp_field: row.get("json_timestamp_field"),
        file_name_regex: row.get("file_name_regex"),
        log_content_regex: row.get("log_content_regex"),
    }
}

fn row_to_source(row: &sqlx::sqlite::SqliteRow) -> Source {
    Source {
        id: row.get::<i64, _>("id") as u64,
        name: row.get("name"),
        template_id: row.get::<i64, _>("template_id") as u64,
        file_path: row.get("file_path"),
        color: row.get("color"),
    }
}

fn row_to_predicate(row: &sqlx::sqlite::SqliteRow) -> Result<PatternPredicate, DbError> {
    let ruleset_name: String = row.get("ruleset_name");
    let state_key: String = row.get("state_key");
    let operator_str: String = row.get("operator");
    let operand_type: String = row.get("operand_type");
    let operand_value: String = row.get("operand_value");

    let operator = parse_operator(&operator_str)?;
    let operand = deserialize_operand(&operand_type, &operand_value)?;

    Ok(PatternPredicate {
        ruleset_name,
        state_key,
        operator,
        operand,
    })
}

// ---------------------------------------------------------------------------
// Enum serialization helpers
// ---------------------------------------------------------------------------

fn parse_match_mode(s: &str) -> Result<MatchMode, DbError> {
    match s {
        "any" => Ok(MatchMode::Any),
        "all" => Ok(MatchMode::All),
        _ => Err(DbError::InvalidData(format!("unknown match_mode: {s}"))),
    }
}

fn match_mode_to_str(m: &MatchMode) -> &'static str {
    match m {
        MatchMode::Any => "any",
        MatchMode::All => "all",
    }
}

fn parse_extraction_type(s: &str) -> Result<ExtractionType, DbError> {
    match s {
        "parsed" => Ok(ExtractionType::Parsed),
        "static" => Ok(ExtractionType::Static),
        "clear" => Ok(ExtractionType::Clear),
        _ => Err(DbError::InvalidData(format!(
            "unknown extraction_type: {s}"
        ))),
    }
}

fn extraction_type_to_str(t: &ExtractionType) -> &'static str {
    match t {
        ExtractionType::Parsed => "parsed",
        ExtractionType::Static => "static",
        ExtractionType::Clear => "clear",
    }
}

fn parse_extraction_mode(s: &str) -> Result<ExtractionMode, DbError> {
    match s {
        "replace" => Ok(ExtractionMode::Replace),
        "accumulate" => Ok(ExtractionMode::Accumulate),
        _ => Err(DbError::InvalidData(format!(
            "unknown extraction_mode: {s}"
        ))),
    }
}

fn extraction_mode_to_str(m: &ExtractionMode) -> &'static str {
    match m {
        ExtractionMode::Replace => "replace",
        ExtractionMode::Accumulate => "accumulate",
    }
}

fn parse_operator(s: &str) -> Result<Operator, DbError> {
    match s {
        "eq" => Ok(Operator::Eq),
        "neq" => Ok(Operator::Neq),
        "gt" => Ok(Operator::Gt),
        "lt" => Ok(Operator::Lt),
        "gte" => Ok(Operator::Gte),
        "lte" => Ok(Operator::Lte),
        "contains" => Ok(Operator::Contains),
        "exists" => Ok(Operator::Exists),
        _ => Err(DbError::InvalidData(format!("unknown operator: {s}"))),
    }
}

fn operator_to_str(o: &Operator) -> &'static str {
    match o {
        Operator::Eq => "eq",
        Operator::Neq => "neq",
        Operator::Gt => "gt",
        Operator::Lt => "lt",
        Operator::Gte => "gte",
        Operator::Lte => "lte",
        Operator::Contains => "contains",
        Operator::Exists => "exists",
    }
}

fn serialize_operand(operand: &Operand) -> (&'static str, String) {
    match operand {
        Operand::Literal(val) => {
            let json = serde_json::to_string(val).unwrap_or_default();
            ("literal", json)
        }
        Operand::StateRef {
            ruleset_name,
            state_key,
        } => {
            let json = serde_json::json!({
                "ruleset_name": ruleset_name,
                "state_key": state_key,
            })
            .to_string();
            ("state_ref", json)
        }
    }
}

fn deserialize_operand(operand_type: &str, operand_value: &str) -> Result<Operand, DbError> {
    match operand_type {
        "literal" => {
            let val: StateValue = serde_json::from_str(operand_value)
                .map_err(|e| DbError::InvalidData(format!("invalid literal operand JSON: {e}")))?;
            Ok(Operand::Literal(val))
        }
        "state_ref" => {
            let obj: serde_json::Value = serde_json::from_str(operand_value).map_err(|e| {
                DbError::InvalidData(format!("invalid state_ref operand JSON: {e}"))
            })?;
            // Support both old "source_name" key (legacy) and new "ruleset_name" key.
            let ruleset_name = obj
                .get("ruleset_name")
                .or_else(|| obj.get("source_name"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| DbError::InvalidData("missing ruleset_name in state_ref".into()))?
                .to_string();
            let state_key = obj["state_key"]
                .as_str()
                .ok_or_else(|| DbError::InvalidData("missing state_key in state_ref".into()))?
                .to_string();
            Ok(Operand::StateRef {
                ruleset_name,
                state_key,
            })
        }
        _ => Err(DbError::InvalidData(format!(
            "unknown operand_type: {operand_type}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_db() -> Database {
        Database::new("sqlite::memory:").await.unwrap()
    }

    #[tokio::test]
    async fn test_project_crud() {
        let db = test_db().await;

        // Create
        let p = db.create_project("Test Project").await.unwrap();
        assert_eq!(p.name, "Test Project");
        assert!(p.id > 0);

        // List
        let projects = db.list_projects().await.unwrap();
        assert_eq!(projects.len(), 1);

        // Get
        let fetched = db.get_project(p.id).await.unwrap();
        assert_eq!(fetched.name, "Test Project");

        // Update
        let updated = db.update_project(p.id, "Updated").await.unwrap();
        assert_eq!(updated.name, "Updated");

        // Delete
        db.delete_project(p.id).await.unwrap();
        assert!(db.get_project(p.id).await.is_err());
    }

    #[tokio::test]
    async fn test_timestamp_template_crud() {
        let db = test_db().await;
        let p = db.create_project("P1").await.unwrap();

        let tt = db
            .create_timestamp_template(p.id, "default_ts", "%Y-%m-%d %H:%M:%S", None, None)
            .await
            .unwrap();
        assert_eq!(tt.name, "default_ts");
        assert_eq!(tt.format, "%Y-%m-%d %H:%M:%S");
        assert!(tt.extraction_regex.is_none());
        assert!(tt.default_year.is_none());

        let tts = db.list_timestamp_templates(p.id).await.unwrap();
        // 6 seeded + 1 manually created
        assert_eq!(tts.len(), 7);

        let fetched = db.get_timestamp_template(p.id, tt.id as i64).await.unwrap();
        assert_eq!(fetched.format, "%Y-%m-%d %H:%M:%S");

        let updated = db
            .update_timestamp_template(
                p.id,
                tt.id as i64,
                "updated_ts",
                "%d/%b/%Y:%H:%M:%S",
                Some(r"\[(\d{2}/\w{3}/\d{4}:\d{2}:\d{2}:\d{2})"),
                None,
            )
            .await
            .unwrap();
        assert_eq!(updated.name, "updated_ts");
        assert!(updated.extraction_regex.is_some());

        db.delete_timestamp_template(p.id, tt.id as i64)
            .await
            .unwrap();
        assert!(db.get_timestamp_template(p.id, tt.id as i64).await.is_err());
    }

    #[tokio::test]
    async fn test_template_crud() {
        let db = test_db().await;
        let p = db.create_project("P1").await.unwrap();

        let tt = db
            .create_timestamp_template(p.id, "ts", "%Y-%m-%d %H:%M:%S", None, None)
            .await
            .unwrap();

        let t = db
            .create_template(
                p.id,
                "default",
                tt.id as i64,
                "\n",
                Some(r"^\d{4}.+$"),
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();
        assert_eq!(t.name, "default");
        assert_eq!(t.content_regex, Some(r"^\d{4}.+$".to_string()));
        assert_eq!(t.timestamp_template_id, tt.id);

        // Verify auto-created default ruleset
        let rulesets = db.list_rulesets(p.id).await.unwrap();
        assert_eq!(rulesets.len(), 1);
        assert_eq!(rulesets[0].name, "Default — default");
        assert_eq!(rulesets[0].template_id, t.id);

        let templates = db.list_templates(p.id).await.unwrap();
        assert_eq!(templates.len(), 1);

        let fetched = db.get_template(p.id, t.id as i64).await.unwrap();
        assert_eq!(fetched.timestamp_template_id, tt.id);

        let updated = db
            .update_template(
                p.id,
                t.id as i64,
                "updated",
                tt.id as i64,
                "\r\n",
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();
        assert_eq!(updated.name, "updated");
        assert!(updated.content_regex.is_none());

        db.delete_template(p.id, t.id as i64).await.unwrap();
        assert!(db.get_template(p.id, t.id as i64).await.is_err());
    }

    #[tokio::test]
    async fn test_source_crud() {
        let db = test_db().await;
        let p = db.create_project("P1").await.unwrap();
        let tt = db
            .create_timestamp_template(p.id, "ts", "%Y-%m-%d %H:%M:%S", None, None)
            .await
            .unwrap();
        let t = db
            .create_template(
                p.id,
                "tmpl",
                tt.id as i64,
                "\n",
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();

        let s = db
            .create_source(
                p.id,
                t.id as i64,
                "server.log",
                "/var/log/server.log",
                "#60a5fa",
            )
            .await
            .unwrap();
        assert_eq!(s.name, "server.log");
        assert_eq!(s.color, "#60a5fa");

        let sources = db.list_sources(p.id).await.unwrap();
        assert_eq!(sources.len(), 1);

        db.delete_source(p.id, s.id as i64).await.unwrap();
        assert!(db.list_sources(p.id).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_rule_crud() {
        let db = test_db().await;
        let p = db.create_project("P1").await.unwrap();

        let rule = db
            .create_rule(
                p.id,
                "error_rule",
                &MatchMode::Any,
                &[CreateMatchRule {
                    pattern: r"ERROR".to_string(),
                }],
                &[CreateExtractionRule {
                    extraction_type: ExtractionType::Static,
                    state_key: "status".to_string(),
                    pattern: None,
                    static_value: Some("error".to_string()),
                    mode: ExtractionMode::Replace,
                    event_text_only: false,
                }],
                None,
            )
            .await
            .unwrap();
        assert_eq!(rule.name, "error_rule");
        assert_eq!(rule.match_rules.len(), 1);
        assert_eq!(rule.extraction_rules.len(), 1);

        let fetched = db.get_rule(p.id, rule.id as i64).await.unwrap();
        assert_eq!(fetched.match_rules[0].pattern, "ERROR");

        let updated = db
            .update_rule(
                p.id,
                rule.id as i64,
                "updated_rule",
                &MatchMode::All,
                &[
                    CreateMatchRule {
                        pattern: "WARN".to_string(),
                    },
                    CreateMatchRule {
                        pattern: "CRITICAL".to_string(),
                    },
                ],
                &[],
                None,
            )
            .await
            .unwrap();
        assert_eq!(updated.name, "updated_rule");
        assert_eq!(updated.match_rules.len(), 2);
        assert!(updated.extraction_rules.is_empty());

        db.delete_rule(p.id, rule.id as i64).await.unwrap();
        assert!(db.get_rule(p.id, rule.id as i64).await.is_err());
    }

    #[tokio::test]
    async fn test_ruleset_crud() {
        let db = test_db().await;
        let p = db.create_project("P1").await.unwrap();
        let tt = db
            .create_timestamp_template(p.id, "ts", "%Y", None, None)
            .await
            .unwrap();
        let t = db
            .create_template(
                p.id,
                "tmpl",
                tt.id as i64,
                "\n",
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();
        let r1 = db
            .create_rule(p.id, "r1", &MatchMode::Any, &[], &[], None)
            .await
            .unwrap();
        let r2 = db
            .create_rule(p.id, "r2", &MatchMode::Any, &[], &[], None)
            .await
            .unwrap();

        let rs = db
            .create_ruleset(p.id, "ruleset1", t.id as i64, &[r1.id as i64, r2.id as i64])
            .await
            .unwrap();
        assert_eq!(rs.rule_ids.len(), 2);

        let fetched = db.get_ruleset(p.id, rs.id as i64).await.unwrap();
        assert_eq!(fetched.name, "ruleset1");

        db.delete_ruleset(p.id, rs.id as i64).await.unwrap();
        assert!(db.get_ruleset(p.id, rs.id as i64).await.is_err());
    }

    #[tokio::test]
    async fn test_pattern_crud() {
        let db = test_db().await;
        let p = db.create_project("P1").await.unwrap();

        let pattern = db
            .create_pattern(
                p.id,
                "test_pattern",
                &[
                    CreatePredicate {
                        ruleset_name: "server_rs".to_string(),
                        state_key: "status".to_string(),
                        operator: Operator::Eq,
                        operand: Operand::Literal(StateValue::String("running".to_string())),
                    },
                    CreatePredicate {
                        ruleset_name: "server_rs".to_string(),
                        state_key: "count".to_string(),
                        operator: Operator::Gt,
                        operand: Operand::Literal(StateValue::Integer(10)),
                    },
                ],
            )
            .await
            .unwrap();
        assert_eq!(pattern.predicates.len(), 2);
        assert_eq!(pattern.predicates[0].ruleset_name, "server_rs");

        // Verify operand round-trip
        if let Operand::Literal(StateValue::String(ref s)) = pattern.predicates[0].operand {
            assert_eq!(s, "running");
        } else {
            panic!("expected literal string operand");
        }

        let fetched = db.get_pattern(p.id, pattern.id as i64).await.unwrap();
        assert_eq!(fetched.predicates.len(), 2);

        db.delete_pattern(p.id, pattern.id as i64).await.unwrap();
        assert!(db.get_pattern(p.id, pattern.id as i64).await.is_err());
    }

    #[tokio::test]
    async fn test_project_seeds_default_timestamp_templates() {
        let db = test_db().await;
        let p = db.create_project("Seeded").await.unwrap();
        let tts = db.list_timestamp_templates(p.id).await.unwrap();
        assert_eq!(tts.len(), 6);
        let names: Vec<&str> = tts.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"ISO 8601"));
        assert!(names.contains(&"Syslog (RFC 3164)"));
        let syslog = tts.iter().find(|t| t.name == "Syslog (RFC 3164)").unwrap();
        assert!(syslog.default_year.is_some());
    }

    #[tokio::test]
    async fn test_pattern_with_state_ref_operand() {
        let db = test_db().await;
        let p = db.create_project("P1").await.unwrap();

        let pattern = db
            .create_pattern(
                p.id,
                "cross_source",
                &[CreatePredicate {
                    ruleset_name: "server_rs".to_string(),
                    state_key: "region".to_string(),
                    operator: Operator::Eq,
                    operand: Operand::StateRef {
                        ruleset_name: "client_rs".to_string(),
                        state_key: "region".to_string(),
                    },
                }],
            )
            .await
            .unwrap();

        let fetched = db.get_pattern(p.id, pattern.id as i64).await.unwrap();
        if let Operand::StateRef {
            ref ruleset_name,
            ref state_key,
        } = fetched.predicates[0].operand
        {
            assert_eq!(ruleset_name, "client_rs");
            assert_eq!(state_key, "region");
        } else {
            panic!("expected state_ref operand");
        }
    }

    #[tokio::test]
    async fn test_load_project_data() {
        let db = test_db().await;
        let p = db.create_project("P1").await.unwrap();
        let tt = db
            .create_timestamp_template(p.id, "ts", "%Y", None, None)
            .await
            .unwrap();
        db.create_template(
            p.id,
            "tmpl",
            tt.id as i64,
            "\n",
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
        db.create_rule(p.id, "r1", &MatchMode::Any, &[], &[], None)
            .await
            .unwrap();

        let data = db.load_project_data(p.id).await.unwrap();
        // 6 seeded + 1 manually created
        assert_eq!(data.timestamp_templates.len(), 7);
        assert_eq!(data.templates.len(), 1);
        assert_eq!(data.rules.len(), 1);
        assert!(data.sources.is_empty());
        assert_eq!(data.rulesets.len(), 1);
        assert!(data.patterns.is_empty());
    }

    #[tokio::test]
    async fn test_import_project_config() {
        use crate::routes::import_export::ProjectExport;

        let db = test_db().await;

        // Create source project with full config
        let src = db.create_project("Source").await.unwrap();
        let tt = db
            .create_timestamp_template(
                src.id,
                "custom_ts",
                "%Y-%m-%d",
                Some(r"\[(.+?)\]"),
                Some(2025),
            )
            .await
            .unwrap();
        let st = db
            .create_template(
                src.id,
                "server_log",
                tt.id as i64,
                "\n",
                Some(r"^\d+"),
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();
        let rule = db
            .create_rule(
                src.id,
                "error_rule",
                &MatchMode::Any,
                &[CreateMatchRule {
                    pattern: "ERROR".to_string(),
                }],
                &[CreateExtractionRule {
                    extraction_type: ExtractionType::Static,
                    state_key: "status".to_string(),
                    pattern: None,
                    static_value: Some("error".to_string()),
                    mode: ExtractionMode::Replace,
                    event_text_only: false,
                }],
                None,
            )
            .await
            .unwrap();
        db.create_ruleset(src.id, "main_rules", st.id as i64, &[rule.id as i64])
            .await
            .unwrap();
        db.create_pattern(
            src.id,
            "failure_pattern",
            &[CreatePredicate {
                ruleset_name: "main_rules".to_string(),
                state_key: "status".to_string(),
                operator: Operator::Eq,
                operand: Operand::Literal(StateValue::String("error".to_string())),
            }],
        )
        .await
        .unwrap();

        // Export source project data
        let data = db.load_project_data(src.id).await.unwrap();
        let export = ProjectExport {
            version: 1,
            timestamp_templates: data.timestamp_templates,
            source_templates: data.templates,
            rules: data.rules,
            rulesets: data.rulesets,
            patterns: data.patterns,
        };

        // Import into a fresh target project
        let target = db.create_project("Target").await.unwrap();
        let result = db.import_project_config(target.id, &export).await.unwrap();

        // Verify counts (source project has 6 seeded + 1 custom TT)
        assert_eq!(result.timestamp_templates, 7);
        assert_eq!(result.source_templates, 1);
        assert_eq!(result.rules, 1);
        assert_eq!(result.rulesets, 2);
        assert_eq!(result.patterns, 1);

        // Verify entities exist in target project
        let target_data = db.load_project_data(target.id).await.unwrap();
        // 6 seeded (from create_project) + 7 imported
        assert_eq!(target_data.timestamp_templates.len(), 13);
        assert_eq!(target_data.templates.len(), 1);
        assert_eq!(target_data.rules.len(), 1);
        // 1 auto-created default (from create_template) + 2 imported
        assert_eq!(target_data.rulesets.len(), 3);
        assert_eq!(target_data.patterns.len(), 1);

        // Find the imported "main_rules" ruleset (not the auto-created defaults)
        let imported_rs = target_data
            .rulesets
            .iter()
            .find(|rs| rs.name == "main_rules")
            .expect("should find imported main_rules ruleset");
        let imported_st = &target_data.templates[0];
        assert_eq!(imported_rs.template_id, imported_st.id);

        // Verify ruleset's rule_ids point to the new rule
        let imported_rule = &target_data.rules[0];
        assert_eq!(imported_rs.rule_ids, vec![imported_rule.id]);

        // Verify imported template's timestamp_template_id points to a valid TT in target
        let imported_tt = target_data
            .timestamp_templates
            .iter()
            .find(|t| t.id == imported_st.timestamp_template_id)
            .expect("imported source template should reference a valid timestamp template");
        assert_eq!(imported_tt.name, "custom_ts");
        assert_eq!(imported_tt.extraction_regex.as_deref(), Some(r"\[(.+?)\]"));
        assert_eq!(imported_tt.default_year, Some(2025));
    }

    #[tokio::test]
    async fn test_import_empty_config() {
        use crate::routes::import_export::ProjectExport;

        let db = test_db().await;
        let p = db.create_project("P1").await.unwrap();

        let export = ProjectExport {
            version: 1,
            timestamp_templates: vec![],
            source_templates: vec![],
            rules: vec![],
            rulesets: vec![],
            patterns: vec![],
        };

        let result = db.import_project_config(p.id, &export).await.unwrap();
        assert_eq!(result.timestamp_templates, 0);
        assert_eq!(result.source_templates, 0);
        assert_eq!(result.rules, 0);
        assert_eq!(result.rulesets, 0);
        assert_eq!(result.patterns, 0);
    }

    #[tokio::test]
    async fn test_clone_project() {
        let db = test_db().await;

        // Build source project with full set of entities
        let src = db.create_project("Source").await.unwrap();
        let tt = db
            .create_timestamp_template(
                src.id,
                "custom_ts",
                "%Y-%m-%d",
                Some(r"\[(.+?)\]"),
                Some(2025),
            )
            .await
            .unwrap();
        let st = db
            .create_template(
                src.id,
                "server_log",
                tt.id as i64,
                "\n",
                Some(r"^\d+"),
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();
        let rule = db
            .create_rule(
                src.id,
                "error_rule",
                &MatchMode::Any,
                &[CreateMatchRule {
                    pattern: "ERROR".to_string(),
                }],
                &[CreateExtractionRule {
                    extraction_type: ExtractionType::Static,
                    state_key: "status".to_string(),
                    pattern: None,
                    static_value: Some("error".to_string()),
                    mode: ExtractionMode::Replace,
                    event_text_only: false,
                }],
                None,
            )
            .await
            .unwrap();
        db.create_ruleset(src.id, "main_rules", st.id as i64, &[rule.id as i64])
            .await
            .unwrap();
        db.create_pattern(
            src.id,
            "failure_pattern",
            &[CreatePredicate {
                ruleset_name: "main_rules".to_string(),
                state_key: "status".to_string(),
                operator: Operator::Eq,
                operand: Operand::Literal(StateValue::String("error".to_string())),
            }],
        )
        .await
        .unwrap();
        db.create_source(
            src.id,
            st.id as i64,
            "server.log",
            "/var/log/server.log",
            "#60a5fa",
        )
        .await
        .unwrap();

        // Clone the project
        let clone = db.clone_project(src.id, "Cloned").await.unwrap();
        assert_eq!(clone.name, "Cloned");
        assert_ne!(clone.id, src.id);

        // Load both projects' data and compare entity counts
        let src_data = db.load_project_data(src.id).await.unwrap();
        let clone_data = db.load_project_data(clone.id).await.unwrap();

        // Source has 6 seeded + 1 custom TT.
        // Clone gets 6 seeded (from create_project) + 7 copied = 13 TTs total.
        assert_eq!(src_data.timestamp_templates.len(), 7);
        assert_eq!(clone_data.timestamp_templates.len(), 13);

        assert_eq!(src_data.templates.len(), clone_data.templates.len());
        assert_eq!(src_data.rules.len(), clone_data.rules.len());
        // Source has 2 rulesets: 1 auto-created default + 1 "main_rules".
        // Clone gets 1 auto-created default (from create_template) + 2 copied = 3 total.
        assert_eq!(src_data.rulesets.len(), 2);
        assert_eq!(clone_data.rulesets.len(), 3);
        assert_eq!(src_data.patterns.len(), clone_data.patterns.len());
        assert_eq!(src_data.sources.len(), clone_data.sources.len());

        // Verify file_path and color are preserved in sources
        let clone_source = &clone_data.sources[0];
        assert_eq!(clone_source.file_path, "/var/log/server.log");
        assert_eq!(clone_source.color, "#60a5fa");

        // Verify IDs are different (no shared references)
        assert_ne!(clone_data.sources[0].id, src_data.sources[0].id);
        assert_ne!(clone_data.templates[0].id, src_data.templates[0].id);
        assert_ne!(clone_data.rules[0].id, src_data.rules[0].id);

        // Verify cloned template points to a valid TT within the cloned project
        let cloned_st = &clone_data.templates[0];
        let cloned_tt = clone_data
            .timestamp_templates
            .iter()
            .find(|t| t.id == cloned_st.timestamp_template_id)
            .expect("cloned source template should reference a valid TT in cloned project");
        assert_eq!(cloned_tt.name, "custom_ts");
        assert_eq!(cloned_tt.extraction_regex.as_deref(), Some(r"\[(.+?)\]"));
        assert_eq!(cloned_tt.default_year, Some(2025));
    }

    #[tokio::test]
    async fn test_template_auto_selection_fields() {
        let db = test_db().await;
        let p = db.create_project("P1").await.unwrap();
        let tt = db
            .create_timestamp_template(p.id, "ts", "%Y-%m-%d %H:%M:%S", None, None)
            .await
            .unwrap();

        // Create with auto-selection regexes
        let t = db
            .create_template(
                p.id,
                "nginx",
                tt.id as i64,
                "\n",
                None,
                None,
                None,
                Some(r"nginx.*\.log$"),
                Some(r"^\d+\.\d+\.\d+\.\d+ -"),
            )
            .await
            .unwrap();
        assert_eq!(t.file_name_regex.as_deref(), Some(r"nginx.*\.log$"));
        assert_eq!(
            t.log_content_regex.as_deref(),
            Some(r"^\d+\.\d+\.\d+\.\d+ -")
        );

        // Verify round-trip via get
        let fetched = db.get_template(p.id, t.id as i64).await.unwrap();
        assert_eq!(fetched.file_name_regex.as_deref(), Some(r"nginx.*\.log$"));
        assert_eq!(
            fetched.log_content_regex.as_deref(),
            Some(r"^\d+\.\d+\.\d+\.\d+ -")
        );

        // Verify round-trip via list
        let all = db.list_templates(p.id).await.unwrap();
        let listed = all.iter().find(|t| t.name == "nginx").unwrap();
        assert_eq!(listed.file_name_regex.as_deref(), Some(r"nginx.*\.log$"));

        // Update: clear the fields
        let updated = db
            .update_template(
                p.id,
                t.id as i64,
                "nginx",
                tt.id as i64,
                "\n",
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();
        assert!(updated.file_name_regex.is_none());
        assert!(updated.log_content_regex.is_none());
    }

    #[tokio::test]
    async fn test_legacy_fixup_v1_database() {
        // Simulate a V1 database with `timestamp_format` column instead of
        // `timestamp_template_id`.
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let url = format!("sqlite:{}?mode=rwc", tmp.path().display());

        // Create V1 schema directly.
        let opts = SqliteConnectOptions::from_str(&url)
            .unwrap()
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .unwrap();

        sqlx::query(
            "CREATE TABLE projects (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "CREATE TABLE source_templates (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                name TEXT NOT NULL,
                timestamp_format TEXT NOT NULL,
                line_delimiter TEXT NOT NULL,
                content_regex TEXT
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Insert V1 data.
        sqlx::query(
            "INSERT INTO projects (name, created_at) VALUES ('P1', '2024-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO source_templates (project_id, name, timestamp_format, line_delimiter)
             VALUES (1, 'syslog', '%b %d %H:%M:%S', '\n')",
        )
        .execute(&pool)
        .await
        .unwrap();

        pool.close().await;

        // Open via Database::new() — should migrate successfully.
        let db = Database::new(&url).await.unwrap();

        // Verify: old column is gone, source template has timestamp_template_id.
        let templates = db.list_templates(1).await.unwrap();
        assert_eq!(templates.len(), 1);
        assert!(templates[0].timestamp_template_id > 0);

        // Verify: can create new source templates (would fail if timestamp_format NOT NULL remained).
        let tts = db.list_timestamp_templates(1).await.unwrap();
        assert!(!tts.is_empty());
        let tt_id = tts[0].id as i64;
        let new_st = db
            .create_template(1, "new_tmpl", tt_id, "\n", None, None, None, None, None)
            .await
            .unwrap();
        assert_eq!(new_st.name, "new_tmpl");
    }

    #[tokio::test]
    async fn test_legacy_fixup_partially_migrated() {
        // Simulate partially-migrated state: both `timestamp_format` and
        // `timestamp_template_id` exist, but `timestamp_template_id` is NULL.
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let url = format!("sqlite:{}?mode=rwc", tmp.path().display());

        let opts = SqliteConnectOptions::from_str(&url)
            .unwrap()
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .unwrap();

        sqlx::query(
            "CREATE TABLE projects (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "CREATE TABLE timestamp_templates (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                name TEXT NOT NULL,
                format TEXT NOT NULL,
                extraction_regex TEXT,
                default_year INTEGER
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Partially migrated: has BOTH columns, timestamp_template_id is nullable.
        sqlx::query(
            "CREATE TABLE source_templates (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                name TEXT NOT NULL,
                timestamp_format TEXT NOT NULL,
                timestamp_template_id INTEGER REFERENCES timestamp_templates(id),
                line_delimiter TEXT NOT NULL,
                content_regex TEXT
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO projects (name, created_at) VALUES ('P1', '2024-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO source_templates (project_id, name, timestamp_format, line_delimiter)
             VALUES (1, 'syslog', '%b %d %H:%M:%S', '\n')",
        )
        .execute(&pool)
        .await
        .unwrap();

        pool.close().await;

        // Open via Database::new() — should fix the partially-migrated state.
        let db = Database::new(&url).await.unwrap();

        // Verify: timestamp_template_id is populated.
        let templates = db.list_templates(1).await.unwrap();
        assert_eq!(templates.len(), 1);
        assert!(templates[0].timestamp_template_id > 0);

        // Verify: timestamp_format column is gone (can insert without it).
        let tts = db.list_timestamp_templates(1).await.unwrap();
        let tt_id = tts[0].id as i64;
        db.create_template(1, "new", tt_id, "\n", None, None, None, None, None)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_idempotent_restart() {
        // Open a DB, create data, close, re-open — verify data survives.
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let url = format!("sqlite:{}?mode=rwc", tmp.path().display());

        let db = Database::new(&url).await.unwrap();
        let p = db.create_project("Persistent").await.unwrap();
        let tt = db
            .create_timestamp_template(p.id, "ts", "%Y-%m-%d", None, None)
            .await
            .unwrap();
        db.create_template(
            p.id,
            "tmpl",
            tt.id as i64,
            "\n",
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
        // Drop the pool to release all connections.
        drop(db);

        // Re-open — should not fail and data should survive.
        let db2 = Database::new(&url).await.unwrap();
        let projects = db2.list_projects().await.unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "Persistent");

        let templates = db2.list_templates(p.id).await.unwrap();
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].name, "tmpl");
    }
}
