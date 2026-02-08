use chrono::Datelike;
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use sqlx::Row;

use logium_core::model::*;

#[derive(Debug)]
pub enum DbError {
    Sqlx(sqlx::Error),
    NotFound,
    InvalidData(String),
}

impl From<sqlx::Error> for DbError {
    fn from(e: sqlx::Error) -> Self {
        DbError::Sqlx(e)
    }
}

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbError::Sqlx(e) => write!(f, "database error: {e}"),
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
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(url)
            .await?;
        let db = Self { pool };
        db.run_migrations().await?;
        Ok(db)
    }

    async fn run_migrations(&self) -> Result<(), DbError> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS projects (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
        )
        .execute(&self.pool)
        .await?;

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

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS source_templates (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                name TEXT NOT NULL,
                timestamp_template_id INTEGER NOT NULL REFERENCES timestamp_templates(id),
                line_delimiter TEXT NOT NULL,
                content_regex TEXT
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS sources (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                template_id INTEGER NOT NULL REFERENCES source_templates(id),
                name TEXT NOT NULL,
                file_path TEXT NOT NULL
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS rules (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                name TEXT NOT NULL,
                match_mode TEXT NOT NULL
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS match_rules (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                rule_id INTEGER NOT NULL REFERENCES rules(id) ON DELETE CASCADE,
                pattern TEXT NOT NULL
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS extraction_rules (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                rule_id INTEGER NOT NULL REFERENCES rules(id) ON DELETE CASCADE,
                extraction_type TEXT NOT NULL,
                state_key TEXT NOT NULL,
                pattern TEXT,
                static_value TEXT,
                mode TEXT NOT NULL
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS rulesets (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                template_id INTEGER NOT NULL REFERENCES source_templates(id),
                name TEXT NOT NULL
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS ruleset_rules (
                ruleset_id INTEGER NOT NULL REFERENCES rulesets(id) ON DELETE CASCADE,
                rule_id INTEGER NOT NULL REFERENCES rules(id) ON DELETE CASCADE,
                PRIMARY KEY (ruleset_id, rule_id)
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS patterns (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                name TEXT NOT NULL
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS pattern_predicates (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                pattern_id INTEGER NOT NULL REFERENCES patterns(id) ON DELETE CASCADE,
                order_index INTEGER NOT NULL,
                source_name TEXT NOT NULL,
                state_key TEXT NOT NULL,
                operator TEXT NOT NULL,
                operand_type TEXT NOT NULL,
                operand_value TEXT NOT NULL
            )",
        )
        .execute(&self.pool)
        .await?;

        // Enable foreign keys
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&self.pool)
            .await?;

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
        sqlx::query_as::<_, ProjectRow>(
            "SELECT id, name, created_at FROM projects WHERE id = ?",
        )
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
            ("Standard Datetime (millis)", "%Y-%m-%d %H:%M:%S%.f", None, None),
            (
                "Apache/Nginx",
                "%d/%b/%Y:%H:%M:%S",
                Some(r"\[(\d{2}/\w{3}/\d{4}:\d{2}:\d{2}:\d{2})"),
                None,
            ),
            ("Syslog (RFC 3164)", "%b %d %H:%M:%S", None, Some(current_year)),
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

    pub async fn list_timestamp_templates(&self, project_id: i64) -> Result<Vec<TimestampTemplate>, DbError> {
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
        let result = sqlx::query(
            "DELETE FROM timestamp_templates WHERE id = ? AND project_id = ?",
        )
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
            "SELECT id, name, timestamp_template_id, line_delimiter, content_regex
             FROM source_templates WHERE project_id = ? ORDER BY id",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(row_to_template).collect())
    }

    pub async fn get_template(
        &self,
        project_id: i64,
        id: i64,
    ) -> Result<SourceTemplate, DbError> {
        let row = sqlx::query(
            "SELECT id, name, timestamp_template_id, line_delimiter, content_regex
             FROM source_templates WHERE id = ? AND project_id = ?",
        )
        .bind(id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(DbError::NotFound)?;

        Ok(row_to_template(&row))
    }

    pub async fn create_template(
        &self,
        project_id: i64,
        name: &str,
        timestamp_template_id: i64,
        line_delimiter: &str,
        content_regex: Option<&str>,
    ) -> Result<SourceTemplate, DbError> {
        let id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO source_templates (project_id, name, timestamp_template_id, line_delimiter, content_regex)
             VALUES (?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(project_id)
        .bind(name)
        .bind(timestamp_template_id)
        .bind(line_delimiter)
        .bind(content_regex)
        .fetch_one(&self.pool)
        .await?;

        Ok(SourceTemplate {
            id: id as u64,
            name: name.to_string(),
            timestamp_template_id: timestamp_template_id as u64,
            line_delimiter: line_delimiter.to_string(),
            content_regex: content_regex.map(|s| s.to_string()),
        })
    }

    pub async fn update_template(
        &self,
        project_id: i64,
        id: i64,
        name: &str,
        timestamp_template_id: i64,
        line_delimiter: &str,
        content_regex: Option<&str>,
    ) -> Result<SourceTemplate, DbError> {
        let result = sqlx::query(
            "UPDATE source_templates SET name = ?, timestamp_template_id = ?, line_delimiter = ?, content_regex = ?
             WHERE id = ? AND project_id = ?",
        )
        .bind(name)
        .bind(timestamp_template_id)
        .bind(line_delimiter)
        .bind(content_regex)
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
        let result = sqlx::query(
            "DELETE FROM source_templates WHERE id = ? AND project_id = ?",
        )
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
            "SELECT id, name, template_id, file_path FROM sources WHERE project_id = ? ORDER BY id",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(row_to_source).collect())
    }

    pub async fn get_source(&self, project_id: i64, id: i64) -> Result<Source, DbError> {
        let row = sqlx::query(
            "SELECT id, name, template_id, file_path FROM sources WHERE id = ? AND project_id = ?",
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
    ) -> Result<Source, DbError> {
        let id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO sources (project_id, template_id, name, file_path)
             VALUES (?, ?, ?, ?) RETURNING id",
        )
        .bind(project_id)
        .bind(template_id)
        .bind(name)
        .bind(file_path)
        .fetch_one(&self.pool)
        .await?;

        Ok(Source {
            id: id as u64,
            name: name.to_string(),
            template_id: template_id as u64,
            file_path: file_path.to_string(),
        })
    }

    pub async fn update_source_file_path(
        &self,
        project_id: i64,
        id: i64,
        file_path: &str,
    ) -> Result<(), DbError> {
        let result = sqlx::query(
            "UPDATE sources SET file_path = ? WHERE id = ? AND project_id = ?",
        )
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
            "SELECT id, name, match_mode FROM rules WHERE project_id = ? ORDER BY id",
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
            "SELECT id, name, match_mode FROM rules WHERE id = ? AND project_id = ?",
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

        let match_rows = sqlx::query(
            "SELECT id, pattern FROM match_rules WHERE rule_id = ? ORDER BY id",
        )
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
            "SELECT id, extraction_type, state_key, pattern, static_value, mode
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
                })
            })
            .collect();

        Ok(LogRule {
            id: rule_id as u64,
            name,
            match_mode,
            match_rules,
            extraction_rules: extraction_rules?,
        })
    }

    pub async fn create_rule(
        &self,
        project_id: i64,
        name: &str,
        match_mode: &MatchMode,
        match_rules: &[CreateMatchRule],
        extraction_rules: &[CreateExtractionRule],
    ) -> Result<LogRule, DbError> {
        let mode_str = match_mode_to_str(match_mode);
        let rule_id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO rules (project_id, name, match_mode) VALUES (?, ?, ?) RETURNING id",
        )
        .bind(project_id)
        .bind(name)
        .bind(mode_str)
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
                "INSERT INTO extraction_rules (rule_id, extraction_type, state_key, pattern, static_value, mode)
                 VALUES (?, ?, ?, ?, ?, ?) RETURNING id",
            )
            .bind(rule_id)
            .bind(ext_type_str)
            .bind(&er.state_key)
            .bind(er.pattern.as_deref())
            .bind(er.static_value.as_deref())
            .bind(mode_str)
            .fetch_one(&self.pool)
            .await?;
            built_ext_rules.push(ExtractionRule {
                id: id as u64,
                extraction_type: er.extraction_type.clone(),
                state_key: er.state_key.clone(),
                pattern: er.pattern.clone(),
                static_value: er.static_value.clone(),
                mode: er.mode.clone(),
            });
        }

        Ok(LogRule {
            id: rule_id as u64,
            name: name.to_string(),
            match_mode: match_mode.clone(),
            match_rules: built_match_rules,
            extraction_rules: built_ext_rules,
        })
    }

    pub async fn update_rule(
        &self,
        project_id: i64,
        id: i64,
        name: &str,
        match_mode: &MatchMode,
        match_rules: &[CreateMatchRule],
        extraction_rules: &[CreateExtractionRule],
    ) -> Result<LogRule, DbError> {
        let mode_str = match_mode_to_str(match_mode);
        let result = sqlx::query(
            "UPDATE rules SET name = ?, match_mode = ? WHERE id = ? AND project_id = ?",
        )
        .bind(name)
        .bind(mode_str)
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
                "INSERT INTO extraction_rules (rule_id, extraction_type, state_key, pattern, static_value, mode)
                 VALUES (?, ?, ?, ?, ?, ?) RETURNING id",
            )
            .bind(id)
            .bind(ext_type_str)
            .bind(&er.state_key)
            .bind(er.pattern.as_deref())
            .bind(er.static_value.as_deref())
            .bind(mode_str)
            .fetch_one(&self.pool)
            .await?;
            built_ext_rules.push(ExtractionRule {
                id: er_id as u64,
                extraction_type: er.extraction_type.clone(),
                state_key: er.state_key.clone(),
                pattern: er.pattern.clone(),
                static_value: er.static_value.clone(),
                mode: er.mode.clone(),
            });
        }

        Ok(LogRule {
            id: id as u64,
            name: name.to_string(),
            match_mode: match_mode.clone(),
            match_rules: built_match_rules,
            extraction_rules: built_ext_rules,
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
        let rows = sqlx::query(
            "SELECT rule_id FROM ruleset_rules WHERE ruleset_id = ? ORDER BY rule_id",
        )
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
        let rows = sqlx::query(
            "SELECT id, name FROM patterns WHERE project_id = ? ORDER BY id",
        )
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
        let row = sqlx::query(
            "SELECT id, name FROM patterns WHERE id = ? AND project_id = ?",
        )
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
            "SELECT source_name, state_key, operator, operand_type, operand_value
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
                "INSERT INTO pattern_predicates (pattern_id, order_index, source_name, state_key, operator, operand_type, operand_value)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(pattern_id)
            .bind(idx as i64)
            .bind(&p.source_name)
            .bind(&p.state_key)
            .bind(operator_to_str(&p.operator))
            .bind(operand_type)
            .bind(operand_value)
            .execute(&self.pool)
            .await?;

            built.push(PatternPredicate {
                source_name: p.source_name.clone(),
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
    // Load all project data (for analysis)
    // -----------------------------------------------------------------------

    pub async fn load_project_data(
        &self,
        project_id: i64,
    ) -> Result<ProjectData, DbError> {
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
}

/// Input type for creating pattern predicates (no id yet).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CreatePredicate {
    pub source_name: String,
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
    }
}

fn row_to_source(row: &sqlx::sqlite::SqliteRow) -> Source {
    Source {
        id: row.get::<i64, _>("id") as u64,
        name: row.get("name"),
        template_id: row.get::<i64, _>("template_id") as u64,
        file_path: row.get("file_path"),
    }
}

fn row_to_predicate(row: &sqlx::sqlite::SqliteRow) -> Result<PatternPredicate, DbError> {
    let source_name: String = row.get("source_name");
    let state_key: String = row.get("state_key");
    let operator_str: String = row.get("operator");
    let operand_type: String = row.get("operand_type");
    let operand_value: String = row.get("operand_value");

    let operator = parse_operator(&operator_str)?;
    let operand = deserialize_operand(&operand_type, &operand_value)?;

    Ok(PatternPredicate {
        source_name,
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
            source_name,
            state_key,
        } => {
            let json = serde_json::json!({
                "source_name": source_name,
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
            let val: StateValue = serde_json::from_str(operand_value).map_err(|e| {
                DbError::InvalidData(format!("invalid literal operand JSON: {e}"))
            })?;
            Ok(Operand::Literal(val))
        }
        "state_ref" => {
            let obj: serde_json::Value =
                serde_json::from_str(operand_value).map_err(|e| {
                    DbError::InvalidData(format!("invalid state_ref operand JSON: {e}"))
                })?;
            let source_name = obj["source_name"]
                .as_str()
                .ok_or_else(|| DbError::InvalidData("missing source_name in state_ref".into()))?
                .to_string();
            let state_key = obj["state_key"]
                .as_str()
                .ok_or_else(|| DbError::InvalidData("missing state_key in state_ref".into()))?
                .to_string();
            Ok(Operand::StateRef {
                source_name,
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
            .create_timestamp_template(
                p.id,
                "default_ts",
                "%Y-%m-%d %H:%M:%S",
                None,
                None,
            )
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

        db.delete_timestamp_template(p.id, tt.id as i64).await.unwrap();
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
            )
            .await
            .unwrap();
        assert_eq!(t.name, "default");
        assert_eq!(t.content_regex, Some(r"^\d{4}.+$".to_string()));
        assert_eq!(t.timestamp_template_id, tt.id);

        let templates = db.list_templates(p.id).await.unwrap();
        assert_eq!(templates.len(), 1);

        let fetched = db.get_template(p.id, t.id as i64).await.unwrap();
        assert_eq!(fetched.timestamp_template_id, tt.id);

        let updated = db
            .update_template(p.id, t.id as i64, "updated", tt.id as i64, "\r\n", None)
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
            .create_template(p.id, "tmpl", tt.id as i64, "\n", None)
            .await
            .unwrap();

        let s = db
            .create_source(p.id, t.id as i64, "server.log", "/var/log/server.log")
            .await
            .unwrap();
        assert_eq!(s.name, "server.log");

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
                }],
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
            .create_template(p.id, "tmpl", tt.id as i64, "\n", None)
            .await
            .unwrap();
        let r1 = db
            .create_rule(p.id, "r1", &MatchMode::Any, &[], &[])
            .await
            .unwrap();
        let r2 = db
            .create_rule(p.id, "r2", &MatchMode::Any, &[], &[])
            .await
            .unwrap();

        let rs = db
            .create_ruleset(
                p.id,
                "ruleset1",
                t.id as i64,
                &[r1.id as i64, r2.id as i64],
            )
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
                        source_name: "server".to_string(),
                        state_key: "status".to_string(),
                        operator: Operator::Eq,
                        operand: Operand::Literal(StateValue::String("running".to_string())),
                    },
                    CreatePredicate {
                        source_name: "server".to_string(),
                        state_key: "count".to_string(),
                        operator: Operator::Gt,
                        operand: Operand::Literal(StateValue::Integer(10)),
                    },
                ],
            )
            .await
            .unwrap();
        assert_eq!(pattern.predicates.len(), 2);
        assert_eq!(pattern.predicates[0].source_name, "server");

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
                    source_name: "server".to_string(),
                    state_key: "region".to_string(),
                    operator: Operator::Eq,
                    operand: Operand::StateRef {
                        source_name: "client".to_string(),
                        state_key: "region".to_string(),
                    },
                }],
            )
            .await
            .unwrap();

        let fetched = db.get_pattern(p.id, pattern.id as i64).await.unwrap();
        if let Operand::StateRef {
            ref source_name,
            ref state_key,
        } = fetched.predicates[0].operand
        {
            assert_eq!(source_name, "client");
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
        db.create_template(p.id, "tmpl", tt.id as i64, "\n", None)
            .await
            .unwrap();
        db.create_rule(p.id, "r1", &MatchMode::Any, &[], &[])
            .await
            .unwrap();

        let data = db.load_project_data(p.id).await.unwrap();
        // 6 seeded + 1 manually created
        assert_eq!(data.timestamp_templates.len(), 7);
        assert_eq!(data.templates.len(), 1);
        assert_eq!(data.rules.len(), 1);
        assert!(data.sources.is_empty());
        assert!(data.rulesets.is_empty());
        assert!(data.patterns.is_empty());
    }
}
