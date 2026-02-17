use std::path::{Component, Path, PathBuf};

use axum::http::{HeaderValue, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};

const CACHE_CONTROL_IMMUTABLE: &str = "public, max-age=31536000, immutable";
const ENV_WEB_OUT_DIR: &str = "DEN_WEB_OUT_DIR";

fn cache_control_for_path(path: &str) -> Option<&'static str> {
    if path.starts_with("_next/static/") {
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

async fn read_file(root: &Path, rel: &str) -> Option<Vec<u8>> {
    if rel.is_empty() || !is_safe_rel_path(rel) {
        return None;
    }
    let path = root.join(rel);
    tokio::fs::read(path).await.ok()
}

fn response_for_bytes(
    content_type: &str,
    cache_control: Option<&'static str>,
    bytes: Vec<u8>,
) -> Response {
    let mut res = (
        StatusCode::OK,
        [(header::CONTENT_TYPE, content_type)],
        bytes,
    )
        .into_response();

    if let Some(cache_control) = cache_control {
        res.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static(cache_control),
        );
    }

    res
}

pub async fn handler(uri: Uri) -> Response {
    let Some(root) = resolve_web_out_dir() else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let path = uri.path().trim_start_matches('/');
    let path = path.trim_end_matches('/');

    if !path.is_empty()
        && let Some(bytes) = read_file(&root, path).await
    {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return response_for_bytes(mime.as_ref(), cache_control_for_path(path), bytes);
    }

    if !path.is_empty()
        && let Some(bytes) = read_file(&root, &format!("{path}.html")).await
    {
        return response_for_bytes("text/html; charset=utf-8", None, bytes);
    }

    if !path.is_empty()
        && let Some(bytes) = read_file(&root, &format!("{path}/index.html")).await
    {
        return response_for_bytes("text/html; charset=utf-8", None, bytes);
    }

    match read_file(&root, "index.html").await {
        Some(bytes) => response_for_bytes("text/html; charset=utf-8", None, bytes),
        None => StatusCode::NOT_FOUND.into_response(),
    }
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
}
