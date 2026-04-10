use axum::http::{request::Parts as RequestParts, HeaderValue, Method};
use std::sync::Arc;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};

/// CORS for browser calls from the static site + local dev.
///
/// `allow_credentials(false)` — auth uses `Authorization` / local storage, not cross-site cookies.
/// That avoids Safari/WebKit edge cases with `Access-Control-Allow-Credentials` on JSON-RPC POST.
/// `allow_headers(Any)` — preflight sometimes includes extra headers (e.g. `Accept`); denying them
/// surfaces as "access control checks" / "network connection was lost" in WebKit.
pub fn cors_layer(extra_origins: Vec<String>) -> CorsLayer {
    let extra = Arc::new(extra_origins);
    CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(move |origin: &HeaderValue, parts: &RequestParts| {
            is_allowed_origin(origin, parts, &extra)
        }))
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS, Method::HEAD])
        .allow_headers(Any)
        .allow_credentials(false)
        // Fewer preflights during bursts (e.g. metrics + wallet + node-rpc).
        .max_age(std::time::Duration::from_secs(600))
}

fn is_allowed_origin(
    origin: &HeaderValue,
    _parts: &RequestParts,
    extra: &[String],
) -> bool {
    let Ok(origin) = origin.to_str() else {
        return false;
    };

    if is_production_coinjecture_origin(origin) || is_local_dev_origin(origin) {
        return true;
    }

    extra
        .iter()
        .any(|allowed| origins_equal_ignore_case(origin, allowed))
}

fn origins_equal_ignore_case(a: &str, b: &str) -> bool {
    normalize_origin(a) == normalize_origin(b)
}

fn normalize_origin(s: &str) -> String {
    s.trim().trim_end_matches('/').to_ascii_lowercase()
}

/// Match production site origins case-insensitively (host) and tolerate a trailing `/` on the
/// serialized `Origin` header. Exact string matching misses `https://COINjecture.com` and
/// `https://coinjecture.com/`, which yield HTTP 200 preflights **without** `Access-Control-Allow-Origin`.
///
/// Allows **http** and **https** for apex + www so users who hit the site over HTTP (before a
/// redirect) can still call the API from the browser.
fn is_production_coinjecture_origin(origin: &str) -> bool {
    let origin = origin.trim_end_matches('/');

    let Some((scheme, after_scheme)) = origin.split_once("://") else {
        return false;
    };
    if !scheme.eq_ignore_ascii_case("https") && !scheme.eq_ignore_ascii_case("http") {
        return false;
    }

    let hostport = after_scheme.split(['/', '?']).next().unwrap_or("").trim();

    if hostport.is_empty() {
        return false;
    }

    let hostport_lower = hostport.to_ascii_lowercase();
    matches!(
        hostport_lower.as_str(),
        "coinjecture.com"
            | "www.coinjecture.com"
            | "coinjecture.com:443"
            | "www.coinjecture.com:443"
            | "coinjecture.com:80"
            | "www.coinjecture.com:80"
    )
}

fn is_local_dev_origin(origin: &str) -> bool {
    let Some((scheme, rest)) = origin.split_once("://") else {
        return false;
    };

    if scheme != "http" {
        return false;
    }

    let host_port = rest.split('/').next().unwrap_or(rest);

    if host_port == "localhost" || host_port == "127.0.0.1" || host_port == "[::1]" {
        return true;
    }

    host_port.starts_with("localhost:")
        || host_port.starts_with("127.0.0.1:")
        || host_port.starts_with("[::1]:")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_allows_http_apex() {
        assert!(is_production_coinjecture_origin("http://coinjecture.com"));
        assert!(is_production_coinjecture_origin("http://COINjecture.com/"));
    }

    #[test]
    fn production_allows_https_www() {
        assert!(is_production_coinjecture_origin("https://www.coinjecture.com"));
    }

    #[test]
    fn extra_list_matches_normalized() {
        let extra = vec!["https://D123.cloudfront.NET".to_string()];
        assert!(extra
            .iter()
            .any(|allowed| origins_equal_ignore_case("https://d123.cloudfront.net/", allowed)));
    }
}
