pub mod admin;
pub mod auth;
pub mod chain;
pub mod email_auth;
pub mod events;
pub mod health;
pub mod jsonrpc_proxy;
pub mod marketplace;
pub mod peers;

use axum::extract::DefaultBodyLimit;
use axum::http::StatusCode;
use axum::routing::{delete, get, post};
use axum::Router;
use std::time::Duration;
use tower_http::compression::CompressionLayer;
use tower_http::timeout::TimeoutLayer;

use crate::middleware::{cors, rate_limit};
use crate::AppState;

/// Assemble all routes with middleware layers.
///
/// SSE routes are separated from API routes so the 30 s timeout doesn't
/// kill long-lived event streams.
pub fn build_routes(state: AppState) -> Router {
    // ── API routes (with timeout + rate limiter) ────────────────────────
    let api_routes = Router::new()
        // Health
        .route("/health", get(health::health))
        .route("/ready", get(health::ready))
        // Wallet auth (SIWB)
        .route("/auth/challenge", post(auth::challenge))
        .route("/auth/verify", post(auth::verify))
        .route("/auth/me", get(auth::me))
        // Email auth
        .route("/auth/email/signup", post(email_auth::signup))
        .route("/auth/email/signin", post(email_auth::signin))
        .route(
            "/auth/email/magic-link",
            post(email_auth::request_magic_link),
        )
        .route(
            "/auth/email/verify-magic-link",
            post(email_auth::verify_magic_link),
        )
        .route("/auth/email/bind-wallet", post(email_auth::bind_wallet))
        // Chain data
        .route("/chain/latest-block", get(chain::latest_block))
        .route("/chain/info", get(chain::chain_info))
        .route(
            "/node-rpc",
            get(jsonrpc_proxy::get_hint).post(jsonrpc_proxy::proxy),
        )
        // Marketplace
        .route(
            "/marketplace/pairs",
            get(marketplace::get_pairs),
        )
        .route(
            "/marketplace/orders",
            get(marketplace::get_orders).post(marketplace::place_order),
        )
        .route(
            "/marketplace/orders/{id}",
            delete(marketplace::cancel_order),
        )
        .route("/marketplace/trades", get(marketplace::get_trades))
        .route(
            "/marketplace/tasks",
            get(marketplace::get_tasks).post(marketplace::create_task),
        )
        .route("/marketplace/engine/stats", get(marketplace::engine_stats))
        // Admin
        .route("/admin/stats", get(admin::stats))
        .route("/admin/peers", get(peers::list_peers))
        .route("/admin/peers/add", post(peers::add_peer))
        .route("/admin/peers/remove", post(peers::remove_peer))
        // Metrics
        .route("/metrics", get(crate::metrics::metrics_handler))
        // Rate limiter (before with_state)
        .layer(axum::middleware::from_fn_with_state(
            state.rate_limiter.clone(),
            rate_limit::rate_limit_middleware,
        ))
        .with_state(state.clone())
        // Timeout (only API routes, NOT SSE). `/node-rpc` may run `chain_submitBlock` (large/slow).
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(300),
        ));

    // ── SSE routes (NO timeout — long-lived connections) ────────────────
    let sse_routes = Router::new()
        .route("/events/blocks", get(events::block_stream))
        .route("/events/mempool", get(events::mempool_stream))
        .route("/events/marketplace", get(events::marketplace_stream))
        .with_state(state);

    // ── Merge + shared middleware ───────────────────────────────────────
    api_routes
        .merge(sse_routes)
        // Default is ~2 MiB; solver-lab `chain_submitBlock` JSON exceeds that.
        .layer(DefaultBodyLimit::max(64 * 1024 * 1024))
        .layer(axum::middleware::from_fn(
            crate::metrics::metrics_middleware,
        ))
        .layer(crate::middleware::tracing::tracing_layer())
        .layer(cors::cors_layer())
        .layer(CompressionLayer::new())
}
