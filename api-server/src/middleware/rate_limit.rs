use axum::extract::State;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use governor::Quota;
use governor::RateLimiter;
use std::net::IpAddr;
use std::num::NonZeroU32;
use std::sync::Arc;

/// Per-IP keyed rate limiter backed by DashMap.
pub type KeyedRateLimiter = Arc<
    RateLimiter<
        IpAddr,
        governor::state::keyed::DashMapStateStore<IpAddr>,
        governor::clock::DefaultClock,
    >,
>;

/// Create a governor rate limiter at `rps` requests per second per IP.
pub fn create_rate_limiter(rps: u32) -> KeyedRateLimiter {
    let quota = Quota::per_second(NonZeroU32::new(rps).expect("rate limit must be > 0"));
    Arc::new(RateLimiter::dashmap(quota))
}

/// Axum middleware — checks the per-IP rate limiter and returns 429 if exceeded.
pub async fn rate_limit_middleware(
    State(limiter): State<KeyedRateLimiter>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    let ip = extract_client_ip(&request);

    if limiter.check_key(&ip).is_err() {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [("Retry-After", "1")],
            "Too Many Requests",
        )
            .into_response();
    }

    next.run(request).await
}

fn extract_client_ip(request: &axum::extract::Request) -> IpAddr {
    // Try X-Forwarded-For first (reverse proxy / load balancer)
    if let Some(forwarded) = request.headers().get("x-forwarded-for") {
        if let Ok(val) = forwarded.to_str() {
            if let Some(ip_str) = val.split(',').next() {
                if let Ok(ip) = ip_str.trim().parse() {
                    return ip;
                }
            }
        }
    }
    // Fallback — loopback (behind a proxy we always expect X-Forwarded-For)
    IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
}
