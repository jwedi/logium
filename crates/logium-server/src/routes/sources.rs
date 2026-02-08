use axum::extract::{Multipart, Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;

use super::{ApiError, ApiResult};
use crate::AppState;
use crate::db::DbError;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/projects/{project_id}/sources", get(list).post(create))
        .route(
            "/api/projects/{project_id}/sources/{id}",
            get(get_one).delete(remove),
        )
        .route(
            "/api/projects/{project_id}/sources/{id}/upload",
            post(upload),
        )
}

#[derive(Deserialize)]
struct CreateSource {
    template_id: i64,
    name: String,
    file_path: String,
}

async fn list(
    State(state): State<AppState>,
    Path(project_id): Path<i64>,
) -> ApiResult<Json<serde_json::Value>> {
    let sources = state.db.list_sources(project_id).await?;
    Ok(Json(serde_json::to_value(sources).unwrap()))
}

async fn create(
    State(state): State<AppState>,
    Path(project_id): Path<i64>,
    Json(body): Json<CreateSource>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    let source = state
        .db
        .create_source(project_id, body.template_id, &body.name, &body.file_path)
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::to_value(source).unwrap()),
    ))
}

async fn get_one(
    State(state): State<AppState>,
    Path((project_id, id)): Path<(i64, i64)>,
) -> ApiResult<Json<serde_json::Value>> {
    let source = state.db.get_source(project_id, id).await?;
    Ok(Json(serde_json::to_value(source).unwrap()))
}

async fn remove(
    State(state): State<AppState>,
    Path((project_id, id)): Path<(i64, i64)>,
) -> ApiResult<StatusCode> {
    state.db.delete_source(project_id, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn upload(
    State(state): State<AppState>,
    Path((project_id, id)): Path<(i64, i64)>,
    mut multipart: Multipart,
) -> ApiResult<Json<serde_json::Value>> {
    // Verify source exists
    let _source = state.db.get_source(project_id, id).await?;

    if let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::from(DbError::InvalidData(format!("multipart error: {e}"))))?
    {
        let file_name = field.file_name().unwrap_or("upload.log").to_string();
        let data = field
            .bytes()
            .await
            .map_err(|e| ApiError::from(DbError::InvalidData(format!("read error: {e}"))))?;

        // Save to uploads directory
        let upload_path = state.uploads_dir.join(format!("{}_{}", id, file_name));
        tokio::fs::write(&upload_path, &data)
            .await
            .map_err(|e| ApiError::from(DbError::InvalidData(format!("write error: {e}"))))?;

        // Update the source's file_path
        let path_str = upload_path.to_string_lossy().to_string();
        state
            .db
            .update_source_file_path(project_id, id, &path_str)
            .await?;

        let source = state.db.get_source(project_id, id).await?;
        Ok(Json(serde_json::to_value(source).unwrap()))
    } else {
        Err(ApiError::from(DbError::InvalidData(
            "no file field in multipart upload".to_string(),
        )))
    }
}
