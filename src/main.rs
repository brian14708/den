mod api;
mod frontend;
mod state;

use state::AppState;
use tower_http::compression::CompressionLayer;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:den.db?mode=rwc".into());
    let db = sqlx::SqlitePool::connect(&db_url).await.unwrap();
    sqlx::migrate!().run(&db).await.unwrap();
    tracing::info!("database ready");

    let state = AppState { db };

    let app = axum::Router::new()
        .nest("/api", api::router())
        .fallback(frontend::handler)
        .layer(CompressionLayer::new())
        .with_state(state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".into());
    let addr = format!("[::]:{port}");
    tracing::info!("listening on {addr}");

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
