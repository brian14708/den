use std::collections::HashSet;

use axum::http::{HeaderMap, header};
use url::Url;

pub fn request_origin(headers: &HeaderMap, fallback_scheme: &str) -> Option<String> {
    let proto = header_value_first(headers, "x-forwarded-proto").unwrap_or(fallback_scheme);
    request_host(headers).map(|host| format!("{proto}://{host}"))
}

pub fn request_host(headers: &HeaderMap) -> Option<String> {
    header_value_first(headers, "x-forwarded-host")
        .or_else(|| header_value_first(headers, header::HOST))
        .map(str::to_owned)
}

fn header_value_first(
    headers: &HeaderMap,
    name: impl axum::http::header::AsHeaderName,
) -> Option<&str> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(str::trim)
        .filter(|v| !v.is_empty())
}

pub fn normalize_origin(origin: &str) -> Option<String> {
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

pub fn origin_host(origin: &str) -> Option<String> {
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

pub fn request_fallback_scheme(headers: &HeaderMap, rp_origin: &str) -> &'static str {
    let rp_fallback = if rp_origin.starts_with("https://") {
        "https"
    } else {
        "http"
    };
    let Some(rp_host) = origin_host(rp_origin) else {
        return rp_fallback;
    };
    let Some(request_host) = request_host(headers).and_then(|host| normalize_host(&host)) else {
        return rp_fallback;
    };
    if request_host.eq_ignore_ascii_case(&rp_host) {
        rp_fallback
    } else {
        "http"
    }
}

pub fn load_allowed_hosts(rp_origin: &str, configured_hosts: &[String]) -> HashSet<String> {
    let mut hosts = HashSet::new();
    if let Some(host) = origin_host(rp_origin) {
        hosts.insert(host);
    }
    for candidate in configured_hosts {
        if let Some(normalized) = normalize_host(candidate) {
            hosts.insert(normalized);
        } else {
            tracing::warn!(host = candidate, "ignoring invalid allowed host");
        }
    }
    hosts
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

    #[test]
    fn request_fallback_scheme_uses_rp_scheme_for_canonical_host() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("lab.014708.xyz"));
        assert_eq!(
            request_fallback_scheme(&headers, "https://lab.014708.xyz"),
            "https"
        );
    }

    #[test]
    fn request_fallback_scheme_defaults_non_canonical_hosts_to_http() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("fujin:3000"));
        assert_eq!(
            request_fallback_scheme(&headers, "https://lab.014708.xyz"),
            "http"
        );
    }
}
