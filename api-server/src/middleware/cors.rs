use axum::http::{HeaderValue, Method, request::Parts as RequestParts};
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
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::OPTIONS,
            Method::HEAD,
        ])
        .allow_headers(Any)
        .allow_credentials(false)
}

fn is_allowed_origin(origin: &HeaderValue, _parts: &RequestParts) -> bool {
    let Ok(origin) = origin.to_str() else {
        return false;
    };

    matches!(
        origin,
        "https://coinjecture.com" | "https://www.coinjecture.com"
    ) || is_local_dev_origin(origin)
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
