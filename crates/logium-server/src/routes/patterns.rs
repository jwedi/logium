use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use super::ApiResult;
use crate::AppState;
use crate::db::CreatePredicate;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/projects/{project_id}/patterns",
            get(list).post(create),
        )
        .route(
            "/api/projects/{project_id}/patterns/{id}",
            get(get_one).put(update).delete(remove),
        )
}

#[derive(Deserialize)]
struct CreatePattern {
    name: String,
    predicates: Vec<CreatePredicate>,
}

async fn list(
    State(state): State<AppState>,
    Path(project_id): Path<i64>,
) -> ApiResult<Json<serde_json::Value>> {
    let patterns = state.db.list_patterns(project_id).await?;
    Ok(Json(serde_json::to_value(patterns).unwrap()))
}

async fn create(
    State(state): State<AppState>,
    Path(project_id): Path<i64>,
    Json(body): Json<CreatePattern>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    let p = state
        .db
        .create_pattern(project_id, &body.name, &body.predicates)
        .await?;
    Ok((StatusCode::CREATED, Json(serde_json::to_value(p).unwrap())))
}

async fn get_one(
    State(state): State<AppState>,
    Path((project_id, id)): Path<(i64, i64)>,
) -> ApiResult<Json<serde_json::Value>> {
    let p = state.db.get_pattern(project_id, id).await?;
    Ok(Json(serde_json::to_value(p).unwrap()))
}

async fn update(
    State(state): State<AppState>,
    Path((project_id, id)): Path<(i64, i64)>,
    Json(body): Json<CreatePattern>,
) -> ApiResult<Json<serde_json::Value>> {
    let p = state
        .db
        .update_pattern(project_id, id, &body.name, &body.predicates)
        .await?;
    Ok(Json(serde_json::to_value(p).unwrap()))
}

async fn remove(
    State(state): State<AppState>,
    Path((project_id, id)): Path<(i64, i64)>,
) -> ApiResult<StatusCode> {
    state.db.delete_pattern(project_id, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
