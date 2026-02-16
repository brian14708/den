use axum::body::Body;
use axum::extract::State;
use axum::http::Request;
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use url::form_urlencoded;

use crate::origin::{origin_host, request_fallback_scheme, request_origin};
use crate::state::AppState;

fn path_matches(path: &str, route: &str) -> bool {
    path == route
        || path
            .strip_prefix(route)
            .is_some_and(|rest| rest.starts_with('/'))
}

fn canonical_auth_path(path: &str) -> bool {
    path_matches(path, "/login") || path_matches(path, "/setup")
}

pub async fn enforce_canonical_auth_origin(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let path = request.uri().path().to_string();
    let is_login_path = path_matches(&path, "/login");
    if !canonical_auth_path(&path) {
        return next.run(request).await;
    }

    let fallback_scheme = request_fallback_scheme(request.headers(), &state.rp_origin);
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
