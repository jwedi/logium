use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncBufReadExt;

use logium_core::model::Source;

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
            post(upload).layer(DefaultBodyLimit::max(500 * 1024 * 1024)),
        )
        .route(
            "/api/projects/{project_id}/sources/{id}/content",
            get(content),
        )
        .route(
            "/api/projects/{project_id}/sources/{id}/raw-lines",
            get(raw_lines),
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
) -> ApiResult<Json<Vec<Source>>> {
    let sources = state.db.list_sources(project_id).await?;
    Ok(Json(sources))
}

async fn create(
    State(state): State<AppState>,
    Path(project_id): Path<i64>,
    Json(body): Json<CreateSource>,
) -> ApiResult<(StatusCode, Json<Source>)> {
    let source = state
        .db
        .create_source(project_id, body.template_id, &body.name, &body.file_path)
        .await?;
    Ok((StatusCode::CREATED, Json(source)))
}

async fn get_one(
    State(state): State<AppState>,
    Path((project_id, id)): Path<(i64, i64)>,
) -> ApiResult<Json<Source>> {
    let source = state.db.get_source(project_id, id).await?;
    Ok(Json(source))
}

async fn remove(
    State(state): State<AppState>,
    Path((project_id, id)): Path<(i64, i64)>,
) -> ApiResult<StatusCode> {
    state.db.delete_source(project_id, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn content(
    State(state): State<AppState>,
    Path((project_id, id)): Path<(i64, i64)>,
) -> ApiResult<String> {
    let source = state.db.get_source(project_id, id).await?;
    let path = &source.file_path;
    if path.is_empty() {
        return Err(ApiError::from(DbError::InvalidData(
            "no file uploaded for this source".to_string(),
        )));
    }
    let content = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| ApiError::from(DbError::InvalidData(format!("read error: {e}"))))?;
    Ok(content)
}

async fn upload(
    State(state): State<AppState>,
    Path((project_id, id)): Path<(i64, i64)>,
    mut multipart: Multipart,
) -> ApiResult<Json<Source>> {
    tracing::debug!(project_id, source_id = id, "upload request received");

    // Verify source exists
    let _source = state.db.get_source(project_id, id).await?;

    let field = multipart
        .next_field()
        .await
        .map_err(|e| {
            tracing::warn!(project_id, source_id = id, error = %e, "multipart parse error (file may exceed size limit)");
            ApiError::from(DbError::InvalidData(format!("multipart error: {e}")))
        })?
        .ok_or_else(|| {
            ApiError::from(DbError::InvalidData(
                "no file field in multipart upload".to_string(),
            ))
        })?;

    let file_name = field.file_name().unwrap_or("upload.log").to_string();
    let content_type = field.content_type().unwrap_or("unknown").to_string();
    tracing::debug!(project_id, source_id = id, %file_name, %content_type, "reading upload field");

    let data = field
        .bytes()
        .await
        .map_err(|e| {
            tracing::warn!(project_id, source_id = id, %file_name, error = %e, "failed to read upload bytes");
            ApiError::from(DbError::InvalidData(format!("read error: {e}")))
        })?;

    // Save to uploads directory
    let upload_path = state.uploads_dir.join(format!("{}_{}", id, file_name));
    tokio::fs::write(&upload_path, &data)
        .await
        .map_err(|e| ApiError::from(DbError::InvalidData(format!("write error: {e}"))))?;

    tracing::info!(project_id, source_id = id, %file_name, bytes = data.len(), "upload received successfully");

    // Update the source's file_path
    let path_str = upload_path.to_string_lossy().to_string();
    state
        .db
        .update_source_file_path(project_id, id, &path_str)
        .await?;

    let source = state.db.get_source(project_id, id).await?;
    Ok(Json(source))
}

#[derive(Deserialize)]
struct RawLinesQuery {
    offset: Option<usize>,
    limit: Option<usize>,
}

#[derive(Serialize)]
struct RawLinesResponse {
    lines: Vec<String>,
    total_lines: usize,
}

async fn raw_lines(
    State(state): State<AppState>,
    Path((project_id, id)): Path<(i64, i64)>,
    Query(query): Query<RawLinesQuery>,
) -> ApiResult<Json<RawLinesResponse>> {
    let source = state.db.get_source(project_id, id).await?;
    let path = &source.file_path;
    if path.is_empty() {
        return Err(ApiError::from(DbError::InvalidData(
            "no file uploaded for this source".to_string(),
        )));
    }

    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(200).min(1000);

    let file = tokio::fs::File::open(path)
        .await
        .map_err(|e| ApiError::from(DbError::InvalidData(format!("read error: {e}"))))?;

    let reader = tokio::io::BufReader::new(file);
    let mut line_stream = reader.lines();
    let mut total_lines = 0usize;
    let mut lines = Vec::new();

    while let Some(line) = line_stream
        .next_line()
        .await
        .map_err(|e| ApiError::from(DbError::InvalidData(format!("read error: {e}"))))?
    {
        if total_lines >= offset && lines.len() < limit {
            lines.push(line);
        }
        total_lines += 1;
    }

    Ok(Json(RawLinesResponse { lines, total_lines }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Extract the inner Json value from ApiResult, panicking with a message on error.
    fn unwrap_api<T>(result: ApiResult<Json<T>>) -> T {
        match result {
            Ok(Json(v)) => v,
            Err(_) => panic!("API call returned an error"),
        }
    }

    #[tokio::test]
    async fn test_raw_lines_pagination() {
        // Create a temp file with known content
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        for i in 0..10 {
            writeln!(tmp, "line {i}").unwrap();
        }
        let path = tmp.path().to_string_lossy().to_string();

        // Set up DB + source
        let db = crate::db::Database::new("sqlite::memory:").await.unwrap();
        let project = db.create_project("test").await.unwrap();

        let ts_tmpl = db
            .create_timestamp_template(project.id, "ts", "%Y-%m-%d %H:%M:%S", None, None)
            .await
            .unwrap();

        let tmpl = db
            .create_template(
                project.id,
                "tmpl",
                ts_tmpl.id as i64,
                "\\n",
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();

        let source = db
            .create_source(project.id, tmpl.id as i64, "test-src", &path)
            .await
            .unwrap();

        let state = AppState {
            db,
            uploads_dir: std::path::PathBuf::from("/tmp"),
        };

        // Test: fetch all lines (default)
        let resp = unwrap_api(
            raw_lines(
                State(state.clone()),
                Path((project.id, source.id as i64)),
                Query(RawLinesQuery {
                    offset: None,
                    limit: None,
                }),
            )
            .await,
        );

        assert_eq!(resp.total_lines, 10);
        assert_eq!(resp.lines.len(), 10);
        assert_eq!(resp.lines[0], "line 0");
        assert_eq!(resp.lines[9], "line 9");

        // Test: offset + limit
        let resp = unwrap_api(
            raw_lines(
                State(state.clone()),
                Path((project.id, source.id as i64)),
                Query(RawLinesQuery {
                    offset: Some(3),
                    limit: Some(2),
                }),
            )
            .await,
        );

        assert_eq!(resp.total_lines, 10);
        assert_eq!(resp.lines.len(), 2);
        assert_eq!(resp.lines[0], "line 3");
        assert_eq!(resp.lines[1], "line 4");

        // Test: offset past end
        let resp = unwrap_api(
            raw_lines(
                State(state),
                Path((project.id, source.id as i64)),
                Query(RawLinesQuery {
                    offset: Some(100),
                    limit: Some(10),
                }),
            )
            .await,
        );

        assert_eq!(resp.total_lines, 10);
        assert_eq!(resp.lines.len(), 0);
    }
}
