use axum::extract::{Path, Query, State};
use axum::routing::post;
use axum::{Json, Router};

use super::analysis::TimeRangeQuery;
use super::{ApiError, ApiResult};
use crate::AppState;
use crate::db::DbError;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/projects/{project_id}/cluster", post(cluster))
}

async fn cluster(
    State(state): State<AppState>,
    Path(project_id): Path<i64>,
    Query(query): Query<TimeRangeQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let time_range = query
        .to_time_range()
        .map_err(|e| ApiError::from(DbError::InvalidData(e)))?;

    let data = state.db.load_project_data(project_id).await?;

    let result = tokio::task::spawn_blocking(move || {
        logium_core::engine::cluster_logs(
            &data.sources,
            &data.templates,
            &data.timestamp_templates,
            &time_range,
        )
    })
    .await
    .map_err(|e| ApiError::from(DbError::InvalidData(format!("task join error: {e}"))))?
    .map_err(|e| ApiError::from(DbError::InvalidData(format!("clustering error: {e}"))))?;

    Ok(Json(serde_json::to_value(result).unwrap()))
}

#[cfg(test)]
mod tests {
    use crate::db::Database;

    #[tokio::test]
    async fn test_cluster_endpoint() {
        let db = Database::new("sqlite::memory:").await.unwrap();
        let project = db.create_project("ClusterTest").await.unwrap();

        // Create timestamp template + source template
        let tt = db
            .create_timestamp_template(project.id, "ts", "%Y-%m-%d %H:%M:%S", None, None)
            .await
            .unwrap();
        let tmpl = db
            .create_template(
                project.id,
                "tmpl",
                tt.id as i64,
                "\n",
                Some(r"^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2} (.+)$"),
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();

        // Create a temp log file
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("test.log");
        std::fs::write(
            &log_path,
            "2024-01-01 00:00:01 ERROR timeout 100\n\
             2024-01-01 00:00:02 ERROR timeout 200\n\
             2024-01-01 00:00:03 INFO started ok\n",
        )
        .unwrap();

        // Create source pointing to the temp file
        db.create_source(
            project.id,
            tmpl.id as i64,
            "test",
            log_path.to_str().unwrap(),
        )
        .await
        .unwrap();

        // Load project data and cluster
        let data = db.load_project_data(project.id).await.unwrap();
        let result = logium_core::engine::cluster_logs(
            &data.sources,
            &data.templates,
            &data.timestamp_templates,
            &logium_core::engine::TimeRange::default(),
        )
        .unwrap();

        assert_eq!(result.total_lines, 3);
        assert_eq!(result.clusters.len(), 1); // singleton "INFO started ok" filtered out
        assert_eq!(result.clusters[0].count, 2);
        assert!(!result.clusters[0].sample_lines.is_empty());
    }
}
