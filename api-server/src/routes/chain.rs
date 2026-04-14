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
    /// Mirrors node `chain_getInfo` when the API can reach the node.
    #[serde(skip_serializing_if = "Option::is_none")]
    chain_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    best_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    genesis_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    total_work: Option<u64>,
    /// Header PoW: leading hex zero nibbles (from node when mining enabled).
    #[serde(skip_serializing_if = "Option::is_none")]
    header_pow_difficulty: Option<u64>,
    /// NP adjuster problem size for next block (from node when mining enabled).
    #[serde(skip_serializing_if = "Option::is_none")]
    np_problem_size: Option<u64>,
    /// Σ truncated `work_score` on the best chain to tip (`u128` decimal string).
    #[serde(skip_serializing_if = "Option::is_none")]
    best_cumulative_work: Option<String>,
    /// Σ coinbase rewards on the best chain to tip (`u128` decimal string).
    #[serde(skip_serializing_if = "Option::is_none")]
    total_minted_rewards: Option<String>,
}

/// `GET /chain/latest-block` — returns cached latest block from the poller.
pub async fn latest_block(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let cached = state.broadcaster.latest_block.read().await;
    match cached.as_ref() {
        Some(block) => Ok(Json(serde_json::to_value(block).unwrap_or(json!({})))),
        None => {
            // Try direct RPC call if no cached block
            if let Some(ref rpc) = state.node_rpc {
                let result = rpc
                    .get_latest_block()
                    .await
                    .map_err(|e| ApiError::ServiceUnavailable(format!("Node unreachable: {e}")))?;
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
    let mut chain_id = None;
    let mut best_hash = None;
    let mut genesis_hash = None;
    let mut total_work = None;
    let mut header_pow_difficulty = None;
    let mut np_problem_size = None;
    let mut best_cumulative_work = None;
    let mut total_minted_rewards = None;
    let (height, syncing, peer_count) = if let Some(ref rpc) = state.node_rpc {
        // Serve from cache when available and fresh (5-second TTL).
        let info = if let Some(cached) = state.broadcaster.get_cached_chain_info().await {
            cached
        } else {
            match rpc.get_chain_info().await {
                Ok(fresh) => {
                    state.broadcaster.set_cached_chain_info(fresh.clone()).await;
                    fresh
                }
                Err(_) => {
                    return Json(ChainInfoResponse {
                        network: state.config.network.clone(),
                        height: None,
                        syncing: false,
                        peer_count: None,
                        version: env!("CARGO_PKG_VERSION").to_string(),
                        chain_id: None,
                        best_hash: None,
                        genesis_hash: None,
                        total_work: None,
                        header_pow_difficulty: None,
                        np_problem_size: None,
                        best_cumulative_work: None,
                        total_minted_rewards: None,
                    });
                }
            }
        };
        chain_id = info["chain_id"].as_str().map(str::to_owned);
        best_hash = info["best_hash"].as_str().map(str::to_owned);
        genesis_hash = info["genesis_hash"].as_str().map(str::to_owned);
        total_work = info["total_work"].as_u64();
        header_pow_difficulty = info["header_pow_difficulty"].as_u64();
        np_problem_size = info["np_problem_size"].as_u64();
        best_cumulative_work = info["best_cumulative_work"]
            .as_str()
            .map(str::to_owned)
            .or_else(|| info["best_cumulative_work"].as_u64().map(|n| n.to_string()));
        total_minted_rewards = info["total_minted_rewards"]
            .as_str()
            .map(str::to_owned)
            .or_else(|| info["total_minted_rewards"].as_u64().map(|n| n.to_string()));
        (
            info["best_height"].as_u64(),
            info["is_syncing"].as_bool().unwrap_or(false),
            info["peer_count"].as_u64(),
        )
    } else {
        (None, false, None)
    };

    Json(ChainInfoResponse {
        network: state.config.network.clone(),
        height,
        syncing,
        peer_count,
        version: env!("CARGO_PKG_VERSION").to_string(),
        chain_id,
        best_hash,
        genesis_hash,
        total_work,
        header_pow_difficulty,
        np_problem_size,
        best_cumulative_work,
        total_minted_rewards,
    })
}
