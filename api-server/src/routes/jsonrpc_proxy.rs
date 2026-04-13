//! Browser-safe JSON-RPC tunnel: `POST /node-rpc` → node's HTTP JSON-RPC (same as `NODE_RPC_URL`).
//! Avoids shipping `http://localhost:9933` in the static bundle for production sites.
//!
//! Forwards `X-Forwarded-For` to the node so per-IP RPC rate limits apply per browser, not per
//! API container (otherwise every user shares one bucket and hits 429 quickly).

use axum::{
    body::Body,
    extract::ConnectInfo,
    extract::State,
    http::{header::CONTENT_TYPE, HeaderMap, HeaderValue, StatusCode},
    response::Response,
    Json,
};
use bytes::Bytes;
use serde_json::json;
use std::net::SocketAddr;

use crate::errors::ApiError;
use crate::AppState;

fn client_ip_for_upstream(headers: &HeaderMap, peer: &SocketAddr) -> String {
    if let Some(val) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        if let Some(first) = val.split(',').next() {
            let t = first.trim();
            if !t.is_empty() {
                return t.to_string();
            }
        }
    }
    if let Some(val) = headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
        let t = val.trim();
        if !t.is_empty() {
            return t.to_string();
        }
    }
    peer.ip().to_string()
}

/// Opening `/node-rpc` in a tab sends GET; JSON-RPC must use POST. Avoids a confusing 404.
pub async fn get_hint() -> Json<serde_json::Value> {
    Json(json!({
        "message": "POST a JSON-RPC 2.0 body (Content-Type: application/json). Example method: chain_getInfo.",
        "methods": ["POST"]
    }))
}

pub async fn proxy(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ApiError> {
    let rpc = state.node_rpc.as_ref().ok_or_else(|| {
        tracing::warn!("/node-rpc: NODE_RPC_URL not set — cannot forward to chain node");
        ApiError::ServiceUnavailable(
            "Node RPC not configured (set NODE_RPC_URL for the API process)".into(),
        )
    })?;
    let body_len = body.len();
    let xff = client_ip_for_upstream(&headers, &peer);

    let (status_u16, bytes) = rpc
        .forward_jsonrpc_body(body, Some(xff.as_str()))
        .await
        .map_err(|e| {
        tracing::warn!(error = %e, body_len, "/node-rpc forward to node failed");
        ApiError::ServiceUnavailable(format!("Node RPC forward failed: {e}"))
    })?;

    let status = StatusCode::from_u16(status_u16).unwrap_or(StatusCode::BAD_GATEWAY);

    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
        .body(Body::from(bytes))
        .map_err(|e| ApiError::Internal(e.to_string()))
}
