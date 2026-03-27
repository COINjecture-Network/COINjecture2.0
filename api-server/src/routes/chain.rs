use axum::extract::State;
use axum::Json;
use serde::Serialize;
use serde_json::{json, Value};

use crate::errors::ApiError;
use crate::AppState;

#[derive(Serialize)]
pub struct ChainInfoResponse {
    network: String,
    height: Option<u64>,
    syncing: bool,
    peer_count: Option<u64>,
    version: String,
}

/// `GET /chain/latest-block` — returns cached latest block from the poller.
pub async fn latest_block(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let cached = state.broadcaster.latest_block.read().await;
    match cached.as_ref() {
        Some(block) => Ok(Json(serde_json::to_value(block).unwrap_or(json!({})))),
        None => {
            // Try direct RPC call if no cached block
            if let Some(ref rpc) = state.node_rpc {
                let result = rpc.get_latest_block().await.map_err(|e| {
                    ApiError::ServiceUnavailable(format!("Node unreachable: {e}"))
                })?;
                Ok(Json(result))
            } else {
                Err(ApiError::ServiceUnavailable(
                    "No block data available (node not connected)".into(),
                ))
            }
        }
    }
}

/// `GET /chain/info` — returns chain status (always returns at minimum the network name).
pub async fn chain_info(State(state): State<AppState>) -> Json<ChainInfoResponse> {
    let (height, syncing, peer_count) = if let Some(ref rpc) = state.node_rpc {
        match rpc.get_chain_info().await {
            Ok(info) => (
                info["best_height"].as_u64(),
                info["is_syncing"].as_bool().unwrap_or(false),
                info["peer_count"].as_u64(),
            ),
            Err(_) => (None, false, None),
        }
    } else {
        (None, false, None)
    };

    Json(ChainInfoResponse {
        network: state.config.network.clone(),
        height,
        syncing,
        peer_count,
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}
