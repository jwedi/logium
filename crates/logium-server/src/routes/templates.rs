use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Json, Router};
use axum::routing::get;
use serde::{Deserialize, Serialize};

use crate::AppState;
use super::ApiResult;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/projects/{project_id}/templates",
            get(list).post(create),
        )
        .route(
            "/api/projects/{project_id}/templates/{id}",
            get(get_one).put(update).delete(remove),
        )
}

#[derive(Deserialize)]
struct CreateTemplate {
    name: String,
    timestamp_template_id: u64,
    line_delimiter: String,
    content_regex: Option<String>,
}

#[derive(Serialize)]
struct TemplateResponse {
    id: u64,
    name: String,
    timestamp_template_id: u64,
    line_delimiter: String,
    content_regex: Option<String>,
}

impl From<logium_core::model::SourceTemplate> for TemplateResponse {
    fn from(t: logium_core::model::SourceTemplate) -> Self {
        Self {
            id: t.id,
            name: t.name,
            timestamp_template_id: t.timestamp_template_id,
            line_delimiter: t.line_delimiter,
            content_regex: t.content_regex,
        }
    }
}

async fn list(
    State(state): State<AppState>,
    Path(project_id): Path<i64>,
) -> ApiResult<Json<Vec<TemplateResponse>>> {
    let templates = state.db.list_templates(project_id).await?;
    Ok(Json(templates.into_iter().map(Into::into).collect()))
}

async fn create(
    State(state): State<AppState>,
    Path(project_id): Path<i64>,
    Json(body): Json<CreateTemplate>,
) -> ApiResult<(StatusCode, Json<TemplateResponse>)> {
    let t = state
        .db
        .create_template(
            project_id,
            &body.name,
            body.timestamp_template_id as i64,
            &body.line_delimiter,
            body.content_regex.as_deref(),
        )
        .await?;
    Ok((StatusCode::CREATED, Json(t.into())))
}

async fn get_one(
    State(state): State<AppState>,
    Path((project_id, id)): Path<(i64, i64)>,
) -> ApiResult<Json<TemplateResponse>> {
    let t = state.db.get_template(project_id, id).await?;
    Ok(Json(t.into()))
}

async fn update(
    State(state): State<AppState>,
    Path((project_id, id)): Path<(i64, i64)>,
    Json(body): Json<CreateTemplate>,
) -> ApiResult<Json<TemplateResponse>> {
    let t = state
        .db
        .update_template(
            project_id,
            id,
            &body.name,
            body.timestamp_template_id as i64,
            &body.line_delimiter,
            body.content_regex.as_deref(),
        )
        .await?;
    Ok(Json(t.into()))
}

async fn remove(
    State(state): State<AppState>,
    Path((project_id, id)): Path<(i64, i64)>,
) -> ApiResult<StatusCode> {
    state.db.delete_template(project_id, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
