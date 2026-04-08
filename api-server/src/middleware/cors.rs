use axum::http::{request::Parts as RequestParts, HeaderValue, Method};
use tower_http::cors::{AllowOrigin, Any, CorsLayer};

/// CORS for browser calls from the static site + local dev.
///
/// `allow_credentials(false)` — auth uses `Authorization` / local storage, not cross-site cookies.
/// That avoids Safari/WebKit edge cases with `Access-Control-Allow-Credentials` on JSON-RPC POST.
/// `allow_headers(Any)` — preflight sometimes includes extra headers (e.g. `Accept`); denying them
/// surfaces as "access control checks" / "network connection was lost" in WebKit.
pub fn cors_layer() -> CorsLayer {
    CorsLayer::new()
        // Accept any localhost dev port while keeping production origins explicit.
        .allow_origin(AllowOrigin::predicate(is_allowed_origin))
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS, Method::HEAD])
        .allow_headers(Any)
        .allow_credentials(false)
        // Fewer preflights during bursts (e.g. metrics + wallet + node-rpc).
        .max_age(std::time::Duration::from_secs(600))
}

fn is_allowed_origin(origin: &HeaderValue, _parts: &RequestParts) -> bool {
    let Ok(origin) = origin.to_str() else {
        return false;
    };

    is_production_coinjecture_origin(origin) || is_local_dev_origin(origin)
}

/// Match production site origins case-insensitively (host) and tolerate a trailing `/` on the
/// serialized `Origin` header. Exact string matching misses `https://COINjecture.com` and
/// `https://coinjecture.com/`, which yield HTTP 200 preflights **without** `Access-Control-Allow-Origin`.
fn is_production_coinjecture_origin(origin: &str) -> bool {
    let origin = origin.trim_end_matches('/');

    let Some((scheme, after_scheme)) = origin.split_once("://") else {
        return false;
    };
    if !scheme.eq_ignore_ascii_case("https") {
        return false;
    }

    let hostport = after_scheme
        .split(['/', '?'])
        .next()
        .unwrap_or("")
        .trim();

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
