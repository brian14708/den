use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "web/out"]
struct Assets;

const CACHE_CONTROL_IMMUTABLE: &str = "public, max-age=31536000, immutable";
const CACHE_CONTROL_DEFAULT: &str = "public, max-age=1800";

fn cache_control_for_path(path: &str) -> &'static str {
    if path.starts_with("_next/static/") {
        CACHE_CONTROL_IMMUTABLE
    } else {
        CACHE_CONTROL_DEFAULT
    }
}

pub async fn handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = path.trim_end_matches('/');

    // Exact file match (JS, CSS, images, etc.)
    if let Some(file) = Assets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        let cache_control = cache_control_for_path(path);
        return (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, mime.as_ref()),
                (header::CACHE_CONTROL, cache_control),
            ],
            file.data,
        )
            .into_response();
    }

    // Try .html extension (e.g. /settings -> settings.html)
    if let Some(file) = Assets::get(&format!("{path}.html")) {
        return (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "text/html; charset=utf-8"),
                (header::CACHE_CONTROL, CACHE_CONTROL_DEFAULT),
            ],
            file.data,
        )
            .into_response();
    }

    // Try index.html in subdirectory (e.g. /foo/ -> foo/index.html)
    if !path.is_empty()
        && let Some(file) = Assets::get(&format!("{path}/index.html"))
    {
        return (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "text/html; charset=utf-8"),
                (header::CACHE_CONTROL, CACHE_CONTROL_DEFAULT),
            ],
            file.data,
        )
            .into_response();
    }

    // SPA fallback — serve index.html for client-side routing
    match Assets::get("index.html") {
        Some(file) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "text/html; charset=utf-8"),
                (header::CACHE_CONTROL, CACHE_CONTROL_DEFAULT),
            ],
            file.data,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
