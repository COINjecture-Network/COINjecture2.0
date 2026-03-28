//! Integration tests for the full SIWB auth flow.
//!
//! Each test spins up an in-process axum router (no TCP listener) and
//! exercises the challenge → sign → verify → /auth/me lifecycle.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use coinjecture_api_server::{
    build_router,
    config::Config,
    middleware::rate_limit::create_rate_limiter,
    nonce_store::NonceStore,
    AppState,
};
use ed25519_dalek::{Signer, SigningKey};
use http_body_util::BodyExt;
use serde_json::Value;
use std::sync::{Arc, OnceLock};
use tower::ServiceExt;

// ── Helpers ─────────────────────────────────────────────────────────────────

/// The Prometheus recorder can only be installed once per process.
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
async fn test_full_siwb_flow() {
    let app = test_app();

    // 1. Generate an Ed25519 keypair
    let signing_key = SigningKey::generate(&mut rand::rngs::OsRng);
    let pubkey_hex = hex::encode(signing_key.verifying_key().as_bytes());

    // 2. Request challenge
    let resp = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/auth/challenge",
            serde_json::json!({ "wallet_address": pubkey_hex }),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let challenge = body_json(resp.into_body()).await;
    let message = challenge["message"].as_str().unwrap();
    let nonce = challenge["nonce"].as_str().unwrap();
    assert!(!nonce.is_empty());
    assert!(message.contains(&pubkey_hex));
    assert!(message.contains("coinjecture.com"));

    // 3. Sign the message
    let signature = signing_key.sign(message.as_bytes());
    let sig_hex = hex::encode(signature.to_bytes());

    // 4. Verify
    let resp = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/auth/verify",
            serde_json::json!({
                "wallet_address": pubkey_hex,
                "signature": sig_hex,
                "message": message,
            }),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let verify_resp = body_json(resp.into_body()).await;
    let token = verify_resp["token"].as_str().unwrap();
    assert!(!token.is_empty());
    assert_eq!(verify_resp["user"]["wallet_address"], pubkey_hex);

    // 5. Use the JWT to GET /auth/me
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/auth/me")
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let me = body_json(resp.into_body()).await;
    assert_eq!(me["wallet_address"], pubkey_hex);
    assert_eq!(me["network"], "testnet");
}

#[tokio::test]
async fn test_invalid_signature_rejected() {
    let app = test_app();

    let signing_key = SigningKey::generate(&mut rand::rngs::OsRng);
    let pubkey_hex = hex::encode(signing_key.verifying_key().as_bytes());

    // Get challenge
    let resp = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/auth/challenge",
            serde_json::json!({ "wallet_address": pubkey_hex }),
        ))
        .await
        .unwrap();
    let challenge = body_json(resp.into_body()).await;
    let message = challenge["message"].as_str().unwrap();

    // Sign with a DIFFERENT key
    let wrong_key = SigningKey::generate(&mut rand::rngs::OsRng);
    let bad_sig = hex::encode(wrong_key.sign(message.as_bytes()).to_bytes());

    let resp = app
        .oneshot(json_request(
            "POST",
            "/auth/verify",
            serde_json::json!({
                "wallet_address": pubkey_hex,
                "signature": bad_sig,
                "message": message,
            }),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_nonce_replay_prevention() {
    let app = test_app();

    let signing_key = SigningKey::generate(&mut rand::rngs::OsRng);
    let pubkey_hex = hex::encode(signing_key.verifying_key().as_bytes());

    // Get challenge
    let resp = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/auth/challenge",
            serde_json::json!({ "wallet_address": pubkey_hex }),
        ))
        .await
        .unwrap();
    let challenge = body_json(resp.into_body()).await;
    let message = challenge["message"].as_str().unwrap();
    let sig_hex = hex::encode(signing_key.sign(message.as_bytes()).to_bytes());

    let verify_body = serde_json::json!({
        "wallet_address": pubkey_hex,
        "signature": sig_hex,
        "message": message,
    });

    // First verify succeeds
    let resp = app
        .clone()
        .oneshot(json_request("POST", "/auth/verify", verify_body.clone()))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Second verify with same nonce fails (replay prevention)
    let resp = app
        .oneshot(json_request("POST", "/auth/verify", verify_body))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_invalid_wallet_address_format() {
    let app = test_app();

    // Too short
    let resp = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/auth/challenge",
            serde_json::json!({ "wallet_address": "deadbeef" }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // Not hex
    let resp = app
        .oneshot(json_request(
            "POST",
            "/auth/challenge",
            serde_json::json!({ "wallet_address": "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz" }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_auth_me_with_invalid_token() {
    let app = test_app();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/auth/me")
                .header("Authorization", "Bearer garbage.token.here")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // Missing header entirely
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/auth/me")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_health_endpoint() {
    let app = test_app();

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp.into_body()).await;
    assert_eq!(body["status"], "ok");
    assert_eq!(body["network"], "testnet");
}
