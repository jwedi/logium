pub mod analysis;
pub mod import_export;
pub mod patterns;
pub mod projects;
pub mod rules;
pub mod rulesets;
pub mod sources;
pub mod templates;
pub mod timestamp_templates;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::db::DbError;

/// Shared error type for route handlers, converts DbError into HTTP responses.
pub struct ApiError(DbError);

impl From<DbError> for ApiError {
    fn from(e: DbError) -> Self {
        ApiError(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match &self.0 {
            DbError::NotFound => (StatusCode::NOT_FOUND, "not found".to_string()),
            DbError::InvalidData(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            DbError::Sqlx(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("database error: {e}"),
            ),
        };
        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}

/// Convenience type alias for route handler results.
pub type ApiResult<T> = Result<T, ApiError>;
