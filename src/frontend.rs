use std::path::{Component, Path, PathBuf};
use std::task::{Context, Poll};

use axum::body::Body;
use axum::http::{HeaderValue, Method, Request, StatusCode, header};
use axum::response::{IntoResponse, Response};
use tower::Service;
use tower::ServiceExt;
use tower_http::services::{ServeDir, ServeFile};

const CACHE_CONTROL_IMMUTABLE: &str = "public, max-age=31536000, immutable";
const ENV_WEB_OUT_DIR: &str = "DEN_WEB_OUT_DIR";

fn cache_control_for_path(path: &str) -> Option<&'static str> {
    if path.starts_with("_next/") {
        Some(CACHE_CONTROL_IMMUTABLE)
    } else {
        None
    }
}

fn is_safe_rel_path(path: &str) -> bool {
    Path::new(path)
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
}

fn resolve_web_out_dir() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os(ENV_WEB_OUT_DIR) {
        let path = PathBuf::from(path);
        if path.is_dir() {
            return Some(path);
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(bin_dir) = exe.parent() {
            let path = bin_dir.join("../share/den/web/out");
            if path.is_dir() {
                return Some(path);
            }
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        let path = cwd.join("web/out");
        if path.is_dir() {
            return Some(path);
        }
    }

    None
}

fn maybe_apply_cache_header(path: &str, response: &mut Response) {
    let Some(cache_control) = cache_control_for_path(path) else {
        return;
    };
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(cache_control),
    );
}

async fn handle_request(request: Request<Body>) -> Response {
    let Some(root) = resolve_web_out_dir() else {
        return StatusCode::NOT_FOUND.into_response();
    };

    if request.method() != Method::GET && request.method() != Method::HEAD {
        return StatusCode::NOT_FOUND.into_response();
    }

    let rel_path = request.uri().path().trim_start_matches('/').to_string();

    if !rel_path.is_empty() && !is_safe_rel_path(&rel_path) {
        return StatusCode::NOT_FOUND.into_response();
    }

    let dir = ServeDir::new(&root)
        .append_index_html_on_directories(true)
        .not_found_service(ServeFile::new(root.join("_not-found/index.html")));
    let mut res = dir.oneshot(request).await.unwrap().map(Body::new);

    if res.status() != StatusCode::NOT_FOUND {
        maybe_apply_cache_header(&rel_path, &mut res);
    }

    res
}

#[derive(Clone, Copy, Default)]
pub struct FrontendService;

impl Service<Request<Body>> for FrontendService {
    type Response = Response;
    type Error = std::convert::Infallible;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        Box::pin(async move { Ok(handle_request(request).await) })
    }
}

pub fn service() -> FrontendService {
    FrontendService
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_static_assets_are_immutable() {
        assert_eq!(
            cache_control_for_path("_next/static/chunks/app.js"),
            Some(CACHE_CONTROL_IMMUTABLE)
        );
    }

    #[test]
    fn non_next_assets_do_not_set_cache_control() {
        assert_eq!(cache_control_for_path("index.html"), None);
    }

    #[test]
    fn path_traversal_is_rejected() {
        assert!(!is_safe_rel_path("../secrets.txt"));
        assert!(!is_safe_rel_path(".."));
        assert!(!is_safe_rel_path("./index.html"));
        assert!(!is_safe_rel_path("/index.html"));
        assert!(!is_safe_rel_path("a/../../b"));
        assert!(is_safe_rel_path("_next/static/chunks/app.js"));
        assert!(is_safe_rel_path("settings.html"));
    }

    // Not testing `ServeDir` behavior here; we keep unit tests focused on path/cache helpers.
}
