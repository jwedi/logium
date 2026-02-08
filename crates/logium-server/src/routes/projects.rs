use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Json, Router};
use axum::routing::get;
use serde::Deserialize;

use crate::AppState;
use super::ApiResult;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/projects", get(list).post(create))
        .route(
            "/api/projects/{id}",
            get(get_one).put(update).delete(remove),
        )
}

#[derive(Deserialize)]
struct CreateProject {
    name: String,
}

async fn list(State(state): State<AppState>) -> ApiResult<Json<serde_json::Value>> {
    let projects = state.db.list_projects().await?;
    Ok(Json(serde_json::to_value(projects).unwrap()))
}

async fn create(
    State(state): State<AppState>,
    Json(body): Json<CreateProject>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    let project = state.db.create_project(&body.name).await?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::to_value(project).unwrap()),
    ))
}

async fn get_one(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<Json<serde_json::Value>> {
    let project = state.db.get_project(id).await?;
    Ok(Json(serde_json::to_value(project).unwrap()))
}

async fn update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<CreateProject>,
) -> ApiResult<Json<serde_json::Value>> {
    let project = state.db.update_project(id, &body.name).await?;
    Ok(Json(serde_json::to_value(project).unwrap()))
}

async fn remove(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<StatusCode> {
    state.db.delete_project(id).await?;
    Ok(StatusCode::NO_CONTENT)
}
