mod api;
mod auth;
mod config;
mod frontend;
mod middleware;
mod origin;
mod state;

use std::path::Path;
use std::sync::Arc;

use axum::middleware::from_fn_with_state;
use config::{AppConfig, load_app_config};
use state::AppState;
use tower_http::compression::CompressionLayer;
use tracing_subscriber::EnvFilter;
use url::Url;
use webauthn_rs::prelude::*;

const DEFAULT_RUST_LOG: &str = "info";

#[tokio::main]
async fn main() {
    let AppConfig {
        port,
        rust_log,
        rp_id,
        rp_origin,
        allowed_hosts: configured_allowed_hosts,
        database_path,
    } = load_app_config();

    let env_filter = EnvFilter::try_new(&rust_log).unwrap_or_else(|_| {
        eprintln!("invalid rust_log value in config, falling back to '{DEFAULT_RUST_LOG}'");
        EnvFilter::new(DEFAULT_RUST_LOG)
    });
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    let db_dir = database_path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(db_dir).unwrap_or_else(|error| {
        panic!(
            "failed to create data directory at {}: {error}",
            db_dir.display()
        )
    });

    let db_url = sqlite_url_for_path(&database_path);
    let db = sqlx::SqlitePool::connect(&db_url).await.unwrap();
    sqlx::migrate!().run(&db).await.unwrap();
    tracing::info!("database ready");

    let secure_cookies = rp_origin.starts_with("https://");
    let rp_origin_url = Url::parse(&rp_origin).expect("invalid rp_origin in config");
    let rp_origin = rp_origin_url.origin().ascii_serialization();
    let allowed_hosts = origin::load_allowed_hosts(&rp_origin, &configured_allowed_hosts);

    let webauthn = WebauthnBuilder::new(&rp_id, &rp_origin_url)
        .expect("failed to create WebauthnBuilder")
        .rp_name("den")
        .build()
        .expect("failed to build Webauthn");

    let jwt_secret = init_jwt_secret(&db).await;

    let state = AppState {
        db,
        webauthn: Arc::new(webauthn),
        jwt_secret: Arc::new(jwt_secret),
        secure_cookies,
        rp_origin,
        allowed_hosts: Arc::new(allowed_hosts),
    };

    let app = axum::Router::new()
        .nest("/api", api::router())
        .fallback(frontend::handler)
        .layer(from_fn_with_state(
            state.clone(),
            middleware::enforce_canonical_auth_origin,
        ))
        .layer(CompressionLayer::new())
        .with_state(state);

    let addr = format!("[::]:{port}");
    tracing::info!("listening on {addr}");

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

fn sqlite_url_for_path(database_path: &Path) -> String {
    format!("sqlite:{}?mode=rwc", database_path.display())
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
            use rand::Rng;
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
