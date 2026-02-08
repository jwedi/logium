use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::http::header;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use logium_core::model::*;

use super::ApiResult;
use crate::AppState;
use crate::db::DbError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectExport {
    pub version: u32,
    pub timestamp_templates: Vec<TimestampTemplate>,
    pub source_templates: Vec<SourceTemplate>,
    pub rules: Vec<LogRule>,
    pub rulesets: Vec<Ruleset>,
    pub patterns: Vec<Pattern>,
}

#[derive(Debug, Serialize)]
pub struct ImportResult {
    pub timestamp_templates: usize,
    pub source_templates: usize,
    pub rules: usize,
    pub rulesets: usize,
    pub patterns: usize,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/projects/{project_id}/export", get(export))
        .route(
            "/api/projects/{project_id}/import",
            axum::routing::post(import),
        )
}

async fn export(
    State(state): State<AppState>,
    Path(project_id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    // Verify project exists
    state.db.get_project(project_id).await?;

    let data = state.db.load_project_data(project_id).await?;
    let export = ProjectExport {
        version: 1,
        timestamp_templates: data.timestamp_templates,
        source_templates: data.templates,
        rules: data.rules,
        rulesets: data.rulesets,
        patterns: data.patterns,
    };

    let json = serde_json::to_string_pretty(&export)
        .map_err(|e| DbError::InvalidData(format!("serialization error: {e}")))?;

    Ok((
        [(
            header::CONTENT_DISPOSITION,
            "attachment; filename=\"project-export.logium.json\"",
        )],
        [(header::CONTENT_TYPE, "application/json")],
        json,
    ))
}

async fn import(
    State(state): State<AppState>,
    Path(project_id): Path<i64>,
    Json(body): Json<ProjectExport>,
) -> ApiResult<(StatusCode, Json<ImportResult>)> {
    if body.version != 1 {
        return Err(DbError::InvalidData(format!(
            "unsupported export version: {}, expected 1",
            body.version
        ))
        .into());
    }

    // Verify project exists
    state.db.get_project(project_id).await?;

    let result = state
        .db
        .import_project_config(project_id, &body)
        .await?;

    Ok((StatusCode::OK, Json(result)))
}
