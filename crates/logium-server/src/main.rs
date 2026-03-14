use std::path::PathBuf;

use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::EnvFilter;

mod db;
mod routes;

// Embedded UI assets — only active in release builds.
// In debug builds, assets are read from disk at runtime (ui/dist/) as before.
#[cfg(not(debug_assertions))]
mod embedded {
    use axum::{
        body::Body,
        http::{StatusCode, Uri, header},
        response::{IntoResponse, Response},
    };
    use rust_embed::RustEmbed;

    #[derive(RustEmbed)]
    #[folder = "../../ui/dist/"]
    struct Assets;

    pub async fn handler(uri: Uri) -> Response {
        let path = uri.path().trim_start_matches('/');
        let path = if path.is_empty() { "index.html" } else { path };

        match Assets::get(path) {
            Some(content) => {
                let mime = mime_guess::from_path(path)
                    .first_or_octet_stream()
                    .to_string();
                ([(header::CONTENT_TYPE, mime)], Body::from(content.data)).into_response()
            }
            None => {
                // SPA fallback: any unmatched path returns index.html
                match Assets::get("index.html") {
                    Some(index) => (
                        StatusCode::OK,
                        [(header::CONTENT_TYPE, "text/html")],
                        Body::from(index.data),
                    )
                        .into_response(),
                    None => StatusCode::NOT_FOUND.into_response(),
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub db: db::Database,
    pub uploads_dir: PathBuf,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let db_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:logium.db?mode=rwc".to_string());
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let uploads_dir = std::env::var("UPLOADS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("uploads"));

    // Ensure uploads directory exists
    tokio::fs::create_dir_all(&uploads_dir)
        .await
        .expect("failed to create uploads directory");

    let database = db::Database::new(&db_url)
        .await
        .expect("failed to initialize database");

    let state = AppState {
        db: database,
        uploads_dir,
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let mut app = Router::new()
        .merge(routes::projects::router())
        .merge(routes::timestamp_templates::router())
        .merge(routes::templates::router())
        .merge(routes::sources::router())
        .merge(routes::rules::router())
        .merge(routes::rulesets::router())
        .merge(routes::patterns::router())
        .merge(routes::import_export::router())
        .merge(routes::analysis::router())
        .merge(routes::clustering::router())
        .layer(cors)
        .with_state(state);

    // Release: serve embedded assets; Debug: serve from disk (existing behavior)
    #[cfg(not(debug_assertions))]
    {
        app = app.fallback(embedded::handler);
    }
    #[cfg(debug_assertions)]
    {
        use tower_http::services::{ServeDir, ServeFile};
        let static_dir = PathBuf::from("ui/dist");
        if static_dir.exists() {
            app = app.fallback_service(
                ServeDir::new(&static_dir).fallback(ServeFile::new(static_dir.join("index.html"))),
            );
        }
    }

    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind");
    tracing::info!("Logium server listening on http://localhost:{port}");

    // Auto-open the browser (release builds only; skipped on headless/server)
    #[cfg(not(debug_assertions))]
    {
        let url = format!("http://localhost:{port}");
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            if let Err(e) = open::that(&url) {
                tracing::warn!("Could not open browser automatically: {e}");
            }
        });
    }

    axum::serve(listener, app).await.unwrap();
}
