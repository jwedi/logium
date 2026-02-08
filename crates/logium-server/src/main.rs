use std::path::PathBuf;

use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

mod db;
mod routes;

#[derive(Clone)]
pub struct AppState {
    pub db: db::Database,
    pub uploads_dir: PathBuf,
}

#[tokio::main]
async fn main() {
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:logium.db?mode=rwc".to_string());
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
        .merge(routes::analysis::router())
        .layer(cors)
        .with_state(state);

    // Serve static files from ui/dist if it exists
    let static_dir = PathBuf::from("../ui/dist");
    if static_dir.exists() {
        app = app.fallback_service(ServeDir::new(static_dir));
    }

    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind");
    println!("Logium server listening on http://localhost:{port}");
    axum::serve(listener, app).await.unwrap();
}
