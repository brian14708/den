mod api;
mod auth;
mod frontend;
mod state;

use std::sync::Arc;

use state::AppState;
use tower_http::compression::CompressionLayer;
use url::Url;
use webauthn_rs::prelude::*;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:den.db?mode=rwc".into());
    let db = sqlx::SqlitePool::connect(&db_url).await.unwrap();
    sqlx::migrate!().run(&db).await.unwrap();
    tracing::info!("database ready");

    // Initialize WebAuthn
    let rp_id = std::env::var("RP_ID").unwrap_or_else(|_| "localhost".into());
    let rp_origin = std::env::var("RP_ORIGIN").unwrap_or_else(|_| "http://localhost:3000".into());
    let secure_cookies = rp_origin.starts_with("https://");
    let rp_origin_url = Url::parse(&rp_origin).expect("invalid RP_ORIGIN");

    let webauthn = WebauthnBuilder::new(&rp_id, &rp_origin_url)
        .expect("failed to create WebauthnBuilder")
        .rp_name("den")
        .build()
        .expect("failed to build Webauthn");

    // Initialize JWT signing key
    let jwt_secret = init_jwt_secret(&db).await;

    let state = AppState {
        db,
        webauthn: Arc::new(webauthn),
        jwt_secret: Arc::new(jwt_secret),
        secure_cookies,
    };

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

async fn init_jwt_secret(db: &sqlx::SqlitePool) -> Vec<u8> {
    let existing: Option<Vec<u8>> =
        sqlx::query_scalar("SELECT secret FROM signing_key WHERE id = 1")
            .fetch_optional(db)
            .await
            .unwrap();

    match existing {
        Some(secret) => {
            tracing::info!("loaded existing JWT signing key");
            secret
        }
        None => {
            use rand::RngCore;
            let mut secret = vec![0u8; 64];
            rand::rng().fill_bytes(&mut secret);

            sqlx::query("INSERT INTO signing_key (id, secret) VALUES (1, ?)")
                .bind(&secret)
                .execute(db)
                .await
                .unwrap();
            tracing::info!("generated new JWT signing key");
            secret
        }
    }
}
