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
    let rpc = state
        .node_rpc
        .as_ref()
        .ok_or_else(|| ApiError::ServiceUnavailable("Node RPC not configured".into()))?;

    let (status_u16, bytes) = rpc
        .forward_jsonrpc_body(body)
        .await
        .map_err(|e| ApiError::ServiceUnavailable(e.to_string()))?;

    let status = StatusCode::from_u16(status_u16).unwrap_or(StatusCode::BAD_GATEWAY);

    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
        .body(Body::from(bytes))
        .map_err(|e| ApiError::Internal(e.to_string()))
}
