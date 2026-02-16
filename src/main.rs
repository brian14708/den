mod api;
mod auth;
mod frontend;
mod state;

use std::collections::HashSet;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, Request, header};
use axum::middleware::{Next, from_fn_with_state};
use axum::response::{IntoResponse, Redirect, Response};
use state::AppState;
use tower_http::compression::CompressionLayer;
use url::{Url, form_urlencoded};
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
    let rp_origin = rp_origin_url.origin().ascii_serialization();
    let allowed_hosts = load_allowed_hosts(&rp_origin);

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
        rp_origin,
        allowed_hosts: Arc::new(allowed_hosts),
    };

    let app = axum::Router::new()
        .nest("/api", api::router(state.clone()))
        .fallback(frontend::handler)
        .layer(from_fn_with_state(
            state.clone(),
            enforce_canonical_auth_origin,
        ))
        .layer(CompressionLayer::new())
        .with_state(state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".into());
    let addr = format!("[::]:{port}");
    tracing::info!("listening on {addr}");

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

fn path_matches(path: &str, route: &str) -> bool {
    path == route
        || path
            .strip_prefix(route)
            .is_some_and(|rest| rest.starts_with('/'))
}

fn canonical_auth_path(path: &str) -> bool {
    path_matches(path, "/login") || path_matches(path, "/setup")
}

fn request_origin(headers: &HeaderMap, fallback_scheme: &str) -> Option<String> {
    let proto = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(fallback_scheme);

    let host = headers
        .get("x-forwarded-host")
        .or_else(|| headers.get(header::HOST))
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(str::trim)
        .filter(|v| !v.is_empty())?;

    Some(format!("{proto}://{host}"))
}

fn normalize_origin(origin: &str) -> Option<String> {
    let parsed = Url::parse(origin).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    parsed.host_str()?;
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return None;
    }
    Some(parsed.origin().ascii_serialization())
}

fn host_with_port(url: &Url) -> Option<String> {
    let host = url.host_str()?.to_ascii_lowercase();
    Some(match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host,
    })
}

fn origin_host(origin: &str) -> Option<String> {
    let origin = normalize_origin(origin)?;
    let parsed = Url::parse(&origin).ok()?;
    host_with_port(&parsed)
}

fn normalize_host(candidate: &str) -> Option<String> {
    let candidate = candidate.trim();
    if candidate.is_empty() {
        return None;
    }
    if candidate.contains("://") {
        return origin_host(candidate);
    }
    if candidate.contains('/') || candidate.contains('?') || candidate.contains('#') {
        return None;
    }

    let parsed = Url::parse(&format!("http://{candidate}")).ok()?;
    host_with_port(&parsed)
}

fn load_allowed_hosts(rp_origin: &str) -> HashSet<String> {
    let mut hosts = HashSet::new();
    if let Some(host) = origin_host(rp_origin) {
        hosts.insert(host);
    }
    let configured = std::env::var("ALLOWED_HOSTS").unwrap_or_default();
    for candidate in configured
        .split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        if let Some(normalized) = normalize_host(candidate) {
            hosts.insert(normalized);
        } else {
            tracing::warn!(host = candidate, "ignoring invalid allowed host");
        }
    }
    hosts
}

async fn enforce_canonical_auth_origin(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let path = request.uri().path().to_string();
    let is_login_path = path_matches(&path, "/login");
    if !canonical_auth_path(&path) {
        return next.run(request).await;
    }

    let fallback_scheme = if state.rp_origin.starts_with("https://") {
        "https"
    } else {
        "http"
    };
    let Some(origin) = request_origin(request.headers(), fallback_scheme) else {
        return next.run(request).await;
    };
    if origin.eq_ignore_ascii_case(&state.rp_origin) {
        return next.run(request).await;
    }

    let mut serializer = form_urlencoded::Serializer::new(String::new());
    let mut has_redirect_origin = false;
    let mut has_redirect_path = false;
    if let Some(query) = request.uri().query() {
        for (key, value) in form_urlencoded::parse(query.as_bytes()) {
            if key == "redirect_origin" {
                if is_login_path {
                    has_redirect_origin = true;
                }
                continue;
            }
            if key == "redirect_path" {
                if !is_login_path {
                    continue;
                }
                has_redirect_path = true;
            }
            serializer.append_pair(&key, &value);
        }
    }
    let origin_host = origin_host(&origin);
    if is_login_path
        && origin_host
            .as_ref()
            .is_some_and(|host| state.allowed_hosts.contains(host))
    {
        serializer.append_pair("redirect_origin", &origin);
        has_redirect_origin = true;
    }
    if is_login_path && has_redirect_origin && !has_redirect_path {
        serializer.append_pair("redirect_path", "/");
    }
    let query = serializer.finish();

    let mut redirect_url = format!("{}{}", state.rp_origin, path);
    if !query.is_empty() {
        redirect_url.push('?');
        redirect_url.push_str(&query);
    }

    Redirect::temporary(&redirect_url).into_response()
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn request_origin_uses_fallback_scheme_when_proto_missing() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("example.com"));

        let origin = request_origin(&headers, "https");
        assert_eq!(origin.as_deref(), Some("https://example.com"));
    }

    #[test]
    fn request_origin_prefers_forwarded_proto_and_host() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-proto", HeaderValue::from_static("https, http"));
        headers.insert(
            "x-forwarded-host",
            HeaderValue::from_static("proxy.example"),
        );
        headers.insert(header::HOST, HeaderValue::from_static("ignored.example"));

        let origin = request_origin(&headers, "http");
        assert_eq!(origin.as_deref(), Some("https://proxy.example"));
    }
}
