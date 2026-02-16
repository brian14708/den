use axum::http::{StatusCode, Uri, header};
use axum::response::{Html, IntoResponse, Response};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "web/out"]
struct Assets;

pub async fn handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    // Exact file match (JS, CSS, images, etc.)
    if let Some(file) = Assets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, mime.as_ref())],
            file.data,
        )
            .into_response();
    }

    // Try .html extension
    if let Some(file) = Assets::get(&format!("{path}.html")) {
        return Html(file.data).into_response();
    }

    // SPA fallback — serve index.html for client-side routing
    match Assets::get("index.html") {
        Some(file) => Html(file.data).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
