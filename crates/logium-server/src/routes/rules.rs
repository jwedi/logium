use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use logium_core::model::MatchMode;

use super::ApiResult;
use crate::AppState;
use crate::db::{CreateExtractionRule, CreateMatchRule};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/projects/{project_id}/rules", get(list).post(create))
        .route(
            "/api/projects/{project_id}/rules/{id}",
            get(get_one).put(update).delete(remove),
        )
}

#[derive(Deserialize)]
struct CreateRule {
    name: String,
    match_mode: MatchMode,
    match_rules: Vec<CreateMatchRule>,
    extraction_rules: Vec<CreateExtractionRule>,
}

async fn list(
    State(state): State<AppState>,
    Path(project_id): Path<i64>,
) -> ApiResult<Json<serde_json::Value>> {
    let rules = state.db.list_rules(project_id).await?;
    Ok(Json(serde_json::to_value(rules).unwrap()))
}

async fn create(
    State(state): State<AppState>,
    Path(project_id): Path<i64>,
    Json(body): Json<CreateRule>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    let rule = state
        .db
        .create_rule(
            project_id,
            &body.name,
            &body.match_mode,
            &body.match_rules,
            &body.extraction_rules,
        )
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::to_value(rule).unwrap()),
    ))
}

async fn get_one(
    State(state): State<AppState>,
    Path((project_id, id)): Path<(i64, i64)>,
) -> ApiResult<Json<serde_json::Value>> {
    let rule = state.db.get_rule(project_id, id).await?;
    Ok(Json(serde_json::to_value(rule).unwrap()))
}

async fn update(
    State(state): State<AppState>,
    Path((project_id, id)): Path<(i64, i64)>,
    Json(body): Json<CreateRule>,
) -> ApiResult<Json<serde_json::Value>> {
    let rule = state
        .db
        .update_rule(
            project_id,
            id,
            &body.name,
            &body.match_mode,
            &body.match_rules,
            &body.extraction_rules,
        )
        .await?;
    Ok(Json(serde_json::to_value(rule).unwrap()))
}

async fn remove(
    State(state): State<AppState>,
    Path((project_id, id)): Path<(i64, i64)>,
) -> ApiResult<StatusCode> {
    state.db.delete_rule(project_id, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
