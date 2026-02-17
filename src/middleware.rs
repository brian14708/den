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

    let mut q = form_urlencoded::Serializer::new(String::new());
    let (mut has_origin, mut has_path) = (false, false);
    if let Some(query) = request.uri().query() {
        for (k, v) in form_urlencoded::parse(query.as_bytes()) {
            match k.as_ref() {
                "redirect_origin" if is_login_path => {
                    has_origin = true;
                    continue;
                }
                "redirect_origin" => continue,
                "redirect_path" if !is_login_path => continue,
                "redirect_path" => has_path = true,
                _ => {}
            }
            q.append_pair(&k, &v);
        }
    }
    if is_login_path && origin_host(&origin).is_some_and(|h| state.allowed_hosts.contains(&h)) {
        q.append_pair("redirect_origin", &origin);
        has_origin = true;
    }
    if is_login_path && has_origin && !has_path {
        q.append_pair("redirect_path", "/");
    }
    let query = q.finish();

    let query = if query.is_empty() {
        String::new()
    } else {
        format!("?{query}")
    };
    Redirect::temporary(&format!("{}{path}{query}", state.rp_origin)).into_response()
}
