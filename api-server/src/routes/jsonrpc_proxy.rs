//! Browser-safe JSON-RPC tunnel: `POST /node-rpc` → node's HTTP JSON-RPC (same as `NODE_RPC_URL`).
//! Avoids shipping `http://localhost:9933` in the static bundle for production sites.

use axum::{
    body::Body,
    extract::State,
    http::{header::CONTENT_TYPE, HeaderValue, StatusCode},
    response::Response,
    Json,
};
use bytes::Bytes;
use serde_json::json;

use crate::errors::ApiError;
use crate::AppState;

/// Opening `/node-rpc` in a tab sends GET; JSON-RPC must use POST. Avoids a confusing 404.
pub async fn get_hint() -> Json<serde_json::Value> {
    Json(json!({
        "message": "POST a JSON-RPC 2.0 body (Content-Type: application/json). Example method: chain_getInfo.",
        "methods": ["POST"]
    }))
}

pub async fn proxy(State(state): State<AppState>, body: Bytes) -> Result<Response, ApiError> {
    let rpc = state.node_rpc.as_ref().ok_or_else(|| {
        tracing::warn!("/node-rpc: NODE_RPC_URL not set — cannot forward to chain node");
        ApiError::ServiceUnavailable("Node RPC not configured (set NODE_RPC_URL for the API process)".into())
    })?;

    let body_len = body.len();
    const MAX_RETRIES: u32 = 3;
    const RETRY_DELAY_MS: u64 = 1000;

    // Retry up to 3 times with 1s delay — the node may briefly refuse connections
    // while mining a block (CPU-intensive PoW solving). Worst case: ~3s before 503.
    let mut last_err = String::new();
    for attempt in 0..=MAX_RETRIES {
        if attempt > 0 {
            tracing::debug!(attempt, body_len, "/node-rpc retrying in {RETRY_DELAY_MS}ms");
            tokio::time::sleep(std::time::Duration::from_millis(RETRY_DELAY_MS)).await;
        }
        match rpc.forward_jsonrpc_body(body.clone()).await {
            Ok(r) => {
                if attempt > 0 {
                    tracing::info!(attempt, body_len, "/node-rpc succeeded after retry");
                }
                let status = StatusCode::from_u16(r.0).unwrap_or(StatusCode::BAD_GATEWAY);
                return Response::builder()
                    .status(status)
                    .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
                    .body(Body::from(r.1))
                    .map_err(|e| ApiError::Internal(e.to_string()));
            }
            Err(e) => {
                last_err = e.to_string();
            }
        }
    }

    tracing::warn!(body_len, retries = MAX_RETRIES, "/node-rpc all attempts failed: {last_err}");
    Err(ApiError::ServiceUnavailable(format!("Node RPC forward failed after {MAX_RETRIES} retries: {last_err}")))
}
