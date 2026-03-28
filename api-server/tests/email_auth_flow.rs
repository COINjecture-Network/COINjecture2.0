//! Integration tests for email auth endpoints and admin stats.
//!
//! These test validation logic and graceful degradation (no real Supabase).

use axum::body::Body;
use axum::http::{Request, StatusCode};
use coinjecture_api_server::{
    build_router,
    config::Config,
    jwt::issue_token,
    middleware::rate_limit::create_rate_limiter,
    nonce_store::NonceStore,
    AppState,
};
use http_body_util::BodyExt;
use serde_json::Value;
use std::sync::{Arc, OnceLock};
use tower::ServiceExt;

// ── Helpers ─────────────────────────────────────────────────────────────────

static TEST_METRICS: OnceLock<metrics_exporter_prometheus::PrometheusHandle> = OnceLock::new();

fn test_metrics() -> metrics_exporter_prometheus::PrometheusHandle {
    TEST_METRICS
        .get_or_init(|| {
            metrics_exporter_prometheus::PrometheusBuilder::new()
                .install_recorder()
                .unwrap()
        })
        .clone()
}

fn test_config() -> Config {
    Config {
        host: "127.0.0.1".into(),
        port: 0,
        supabase_url: String::new(),
        supabase_anon_key: "test-anon-key".into(),
        supabase_jwt_secret: "test-jwt-secret-must-be-long-enough-32".into(),
        supabase_service_role_key: None,
        jwt_expiry_seconds: 3600,
        rate_limit_rps: 1000,
        network: "testnet".into(),
        node_rpc_url: None,
        indexer_enabled: false,
        indexer_poll_interval_secs: 5,
        indexer_confirmations: 6,
    }
}

fn test_app() -> axum::Router {
    let config = test_config();
    let state = AppState {
        nonce_store: Arc::new(NonceStore::new(10_000)),
        rate_limiter: create_rate_limiter(config.rate_limit_rps),
        metrics_handle: test_metrics(),
        supabase: None,
        node_rpc: None,
        broadcaster: std::sync::Arc::new(coinjecture_api_server::sse::EventBroadcaster::new(16)),
        engine: None,
        config,
    };
    build_router(state)
}

async fn body_json(body: Body) -> Value {
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

fn json_request(method: &str, uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_email_signup_validation() {
    let app = test_app();

    // Invalid email
    let resp = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/auth/email/signup",
            serde_json::json!({ "email": "not-an-email", "password": "12345678" }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // Short password
    let resp = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/auth/email/signup",
            serde_json::json!({ "email": "user@example.com", "password": "short" }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // Valid inputs but no Supabase → 503
    let resp = app
        .oneshot(json_request(
            "POST",
            "/auth/email/signup",
            serde_json::json!({ "email": "user@example.com", "password": "validpass123" }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn test_email_signin_validation() {
    let app = test_app();

    // Missing fields
    let resp = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/auth/email/signin",
            serde_json::json!({ "email": "", "password": "" }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // Valid inputs but no Supabase → 503
    let resp = app
        .oneshot(json_request(
            "POST",
            "/auth/email/signin",
            serde_json::json!({ "email": "user@example.com", "password": "password123" }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn test_bind_wallet_requires_auth() {
    let app = test_app();

    // No Authorization header → 401
    let resp = app
        .oneshot(json_request(
            "POST",
            "/auth/email/bind-wallet",
            serde_json::json!({
                "wallet_address": "a".repeat(64),
                "signature": "b".repeat(128),
                "message": "test"
            }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_bind_wallet_validates_address() {
    let app = test_app();
    let config = test_config();

    // Issue a valid test JWT
    let token = issue_token(
        &config.supabase_jwt_secret,
        "test-user-id",
        None,
        Some("test@example.com"),
        "testnet",
        3600,
    )
    .unwrap();

    // Invalid wallet address (too short) → 400
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/email/bind-wallet")
                .header("content-type", "application/json")
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::from(
                    serde_json::json!({
                        "wallet_address": "tooshort",
                        "signature": "a".repeat(128),
                        "message": "test"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_admin_stats_returns_mock_without_supabase() {
    let app = test_app();

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/admin/stats")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp.into_body()).await;
    // Without Supabase, returns mock data with warning
    assert!(body["_warning"].is_string());
    assert_eq!(body["total_users"], 0);
}

#[tokio::test]
async fn test_admin_peers_returns_503_without_node() {
    let app = test_app();

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/admin/peers")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn test_add_peer_validation() {
    let app = test_app();

    // Invalid address format → 400
    let resp = app
        .oneshot(json_request(
            "POST",
            "/admin/peers/add",
            serde_json::json!({ "address": "not-valid", "connect_now": false }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_sse_blocks_returns_event_stream() {
    let app = test_app();

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/events/blocks")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(ct.contains("text/event-stream"));
}

#[tokio::test]
async fn test_marketplace_pairs_returns_503_without_supabase() {
    let app = test_app();

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/marketplace/pairs")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn test_marketplace_order_requires_auth() {
    let app = test_app();

    let resp = app
        .oneshot(json_request(
            "POST",
            "/marketplace/orders",
            serde_json::json!({
                "pair_id": "test",
                "side": "buy",
                "type": "limit",
                "price": "1.50",
                "quantity": "100"
            }),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_chain_info_endpoint() {
    let app = test_app();

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/chain/info")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp.into_body()).await;
    assert_eq!(body["network"], "testnet");
}

#[tokio::test]
async fn test_chain_latest_block_503_without_node() {
    let app = test_app();

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/chain/latest-block")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn test_magic_link_validation() {
    let app = test_app();

    // Invalid email
    let resp = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/auth/email/magic-link",
            serde_json::json!({ "email": "invalid" }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}
