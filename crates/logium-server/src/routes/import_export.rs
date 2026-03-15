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

// --- Preview types ---

#[derive(Debug, Serialize)]
pub struct ImportItemStatus {
    pub export_id: u64,
    pub name: String,
    pub existing_id: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct ImportPreview {
    pub timestamp_templates: Vec<ImportItemStatus>,
    pub source_templates: Vec<ImportItemStatus>,
    pub rules: Vec<ImportItemStatus>,
    pub rulesets: Vec<ImportItemStatus>,
    pub patterns: Vec<ImportItemStatus>,
}

// --- Selective import types ---

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImportAction {
    Add,
    Overwrite,
    Skip,
}

#[derive(Debug, Deserialize)]
pub struct EntitySelection {
    pub export_id: u64,
    pub action: ImportAction,
    pub existing_id: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct ImportSelections {
    pub timestamp_templates: Vec<EntitySelection>,
    pub source_templates: Vec<EntitySelection>,
    pub rules: Vec<EntitySelection>,
    pub rulesets: Vec<EntitySelection>,
    pub patterns: Vec<EntitySelection>,
}

#[derive(Debug, Deserialize)]
pub struct SelectiveImportRequest {
    pub export: ProjectExport,
    pub selections: ImportSelections,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/projects/{project_id}/export", get(export))
        .route(
            "/api/projects/{project_id}/import",
            axum::routing::post(import),
        )
        .route(
            "/api/projects/{project_id}/import/preview",
            axum::routing::post(preview),
        )
        .route(
            "/api/projects/{project_id}/import/selective",
            axum::routing::post(import_selective),
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

    let result = state.db.import_project_config(project_id, &body).await?;

    Ok((StatusCode::OK, Json(result)))
}

async fn preview(
    State(state): State<AppState>,
    Path(project_id): Path<i64>,
    Json(body): Json<ProjectExport>,
) -> ApiResult<Json<ImportPreview>> {
    if body.version != 1 {
        return Err(DbError::InvalidData(format!("unsupported version {}", body.version)).into());
    }
    state.db.get_project(project_id).await?;
    Ok(Json(state.db.get_import_preview(project_id, &body).await?))
}

async fn import_selective(
    State(state): State<AppState>,
    Path(project_id): Path<i64>,
    Json(body): Json<SelectiveImportRequest>,
) -> ApiResult<(StatusCode, Json<ImportResult>)> {
    if body.export.version != 1 {
        return Err(
            DbError::InvalidData(format!("unsupported version {}", body.export.version)).into(),
        );
    }
    state.db.get_project(project_id).await?;
    let result = state
        .db
        .selective_import_project_config(project_id, &body.export, &body.selections)
        .await?;
    Ok((StatusCode::OK, Json(result)))
}
