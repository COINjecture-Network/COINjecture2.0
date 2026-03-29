use axum::http::{HeaderValue, Method};
use tower_http::cors::{Any, CorsLayer};

/// CORS for browser calls from the static site + local dev.
///
/// `allow_credentials(false)` — auth uses `Authorization` / local storage, not cross-site cookies.
/// That avoids Safari/WebKit edge cases with `Access-Control-Allow-Credentials` on JSON-RPC POST.
/// `allow_headers(Any)` — preflight sometimes includes extra headers (e.g. `Accept`); denying them
/// surfaces as "access control checks" / "network connection was lost" in WebKit.
pub fn cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin([
            "http://localhost:3000".parse::<HeaderValue>().unwrap(),
            "http://localhost:8080".parse::<HeaderValue>().unwrap(),
            "http://127.0.0.1:8080".parse::<HeaderValue>().unwrap(),
            "https://coinjecture.com".parse::<HeaderValue>().unwrap(),
            "https://www.coinjecture.com".parse::<HeaderValue>().unwrap(),
        ])
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::OPTIONS,
            Method::HEAD,
        ])
        .allow_headers(Any)
        .allow_credentials(false)
}
