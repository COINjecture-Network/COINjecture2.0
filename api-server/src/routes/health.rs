use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde_json::{json, Value};

use crate::AppState;

/// `GET /health` — always returns 200 with version and network info.
pub async fn health(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "network": state.config.network,
    }))
}

/// `GET /ready` — returns 200 if config is valid, 503 otherwise.
pub async fn ready(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    if state.config.is_valid() {
        Ok(Json(json!({ "status": "ready" })))
    } else {
        Err(StatusCode::SERVICE_UNAVAILABLE)
    }
}
