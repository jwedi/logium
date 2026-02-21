use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use super::ApiResult;
use crate::AppState;

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
        .route(
            "/api/projects/{project_id}/timestamp-templates/test-format",
            post(test_format),
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

#[derive(Deserialize)]
struct TestFormatRequest {
    format: String,
    sample: String,
    extraction_regex: Option<String>,
    default_year: Option<i32>,
}

async fn test_format(
    Path(_project_id): Path<i64>,
    Json(body): Json<TestFormatRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    match logium_core::engine::test_timestamp_format(
        &body.sample,
        &body.format,
        body.extraction_regex.as_deref(),
        body.default_year,
    ) {
        Ok(ts) => Ok(Json(
            serde_json::json!({ "parsed_timestamp": ts.to_string() }),
        )),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e })),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_request_deserialization() {
        let json = r#"{
            "format": "%Y-%m-%dT%H:%M:%S",
            "sample": "2024-01-15T10:30:45 some log",
            "extraction_regex": null,
            "default_year": null
        }"#;
        let req: TestFormatRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.format, "%Y-%m-%dT%H:%M:%S");
        assert_eq!(req.sample, "2024-01-15T10:30:45 some log");
        assert!(req.extraction_regex.is_none());
        assert!(req.default_year.is_none());
    }

    #[test]
    fn test_format_request_with_options() {
        let json = r#"{
            "format": "%d/%b/%Y:%H:%M:%S",
            "sample": "192.168.1.1 - - [29/Jul/2015:17:41:44 +0000]",
            "extraction_regex": "\\[(\\d{2}/\\w{3}/\\d{4}:\\d{2}:\\d{2}:\\d{2})",
            "default_year": 2025
        }"#;
        let req: TestFormatRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.format, "%d/%b/%Y:%H:%M:%S");
        assert!(req.extraction_regex.is_some());
        assert_eq!(req.default_year, Some(2025));
    }
}
