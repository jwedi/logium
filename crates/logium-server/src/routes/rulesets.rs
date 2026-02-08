use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Json, Router};
use axum::routing::get;
use serde::Deserialize;

use crate::AppState;
use super::ApiResult;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/projects/{project_id}/rulesets",
            get(list).post(create),
        )
        .route(
            "/api/projects/{project_id}/rulesets/{id}",
            get(get_one).put(update).delete(remove),
        )
}

#[derive(Deserialize)]
struct CreateRuleset {
    name: String,
    template_id: i64,
    rule_ids: Vec<i64>,
}

async fn list(
    State(state): State<AppState>,
    Path(project_id): Path<i64>,
) -> ApiResult<Json<serde_json::Value>> {
    let rulesets = state.db.list_rulesets(project_id).await?;
    Ok(Json(serde_json::to_value(rulesets).unwrap()))
}

async fn create(
    State(state): State<AppState>,
    Path(project_id): Path<i64>,
    Json(body): Json<CreateRuleset>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    let rs = state
        .db
        .create_ruleset(project_id, &body.name, body.template_id, &body.rule_ids)
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::to_value(rs).unwrap()),
    ))
}

async fn get_one(
    State(state): State<AppState>,
    Path((project_id, id)): Path<(i64, i64)>,
) -> ApiResult<Json<serde_json::Value>> {
    let rs = state.db.get_ruleset(project_id, id).await?;
    Ok(Json(serde_json::to_value(rs).unwrap()))
}

async fn update(
    State(state): State<AppState>,
    Path((project_id, id)): Path<(i64, i64)>,
    Json(body): Json<CreateRuleset>,
) -> ApiResult<Json<serde_json::Value>> {
    let rs = state
        .db
        .update_ruleset(project_id, id, &body.name, body.template_id, &body.rule_ids)
        .await?;
    Ok(Json(serde_json::to_value(rs).unwrap()))
}

async fn remove(
    State(state): State<AppState>,
    Path((project_id, id)): Path<(i64, i64)>,
) -> ApiResult<StatusCode> {
    state.db.delete_ruleset(project_id, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
