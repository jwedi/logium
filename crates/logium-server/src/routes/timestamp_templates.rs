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
            "/api/projects/{project_id}/timestamp-templates",
            get(list).post(create),
        )
        .route(
            "/api/projects/{project_id}/timestamp-templates/{id}",
            get(get_one).put(update).delete(remove),
        )
}

#[derive(Deserialize)]
struct CreateTimestampTemplate {
    name: String,
    format: String,
    extraction_regex: Option<String>,
    default_year: Option<i32>,
}

#[derive(Serialize)]
struct TimestampTemplateResponse {
    id: u64,
    name: String,
    format: String,
    extraction_regex: Option<String>,
    default_year: Option<i32>,
}

impl From<logium_core::model::TimestampTemplate> for TimestampTemplateResponse {
    fn from(t: logium_core::model::TimestampTemplate) -> Self {
        Self {
            id: t.id,
            name: t.name,
            format: t.format,
            extraction_regex: t.extraction_regex,
            default_year: t.default_year,
        }
    }
}

async fn list(
    State(state): State<AppState>,
    Path(project_id): Path<i64>,
) -> ApiResult<Json<Vec<TimestampTemplateResponse>>> {
    let templates = state.db.list_timestamp_templates(project_id).await?;
    Ok(Json(templates.into_iter().map(Into::into).collect()))
}

async fn create(
    State(state): State<AppState>,
    Path(project_id): Path<i64>,
    Json(body): Json<CreateTimestampTemplate>,
) -> ApiResult<(StatusCode, Json<TimestampTemplateResponse>)> {
    let t = state
        .db
        .create_timestamp_template(
            project_id,
            &body.name,
            &body.format,
            body.extraction_regex.as_deref(),
            body.default_year,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(t.into())))
}

async fn get_one(
    State(state): State<AppState>,
    Path((project_id, id)): Path<(i64, i64)>,
) -> ApiResult<Json<TimestampTemplateResponse>> {
    let t = state.db.get_timestamp_template(project_id, id).await?;
    Ok(Json(t.into()))
}

async fn update(
    State(state): State<AppState>,
    Path((project_id, id)): Path<(i64, i64)>,
    Json(body): Json<CreateTimestampTemplate>,
) -> ApiResult<Json<TimestampTemplateResponse>> {
    let t = state
        .db
        .update_timestamp_template(
            project_id,
            id,
            &body.name,
            &body.format,
            body.extraction_regex.as_deref(),
            body.default_year,
        )
        .await?;
    Ok(Json(t.into()))
}

async fn remove(
    State(state): State<AppState>,
    Path((project_id, id)): Path<(i64, i64)>,
) -> ApiResult<StatusCode> {
    state.db.delete_timestamp_template(project_id, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
