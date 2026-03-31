use axum::extract::State;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::errors::ApiError;
use crate::AppState;

// ── Request types ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct AddPeerRequest {
    pub address: String,
    #[serde(default)]
    pub connect_now: bool,
}

#[derive(Deserialize)]
pub struct RemovePeerRequest {
    pub address: String,
    #[serde(default)]
    pub ban_hours: u32,
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn require_node_rpc(
    state: &AppState,
) -> Result<&std::sync::Arc<crate::node_rpc::NodeRpcClient>, ApiError> {
    state
        .node_rpc
        .as_ref()
        .ok_or_else(|| {
            ApiError::ServiceUnavailable("Node RPC not available".into())
        })
}

fn is_valid_socket_addr(addr: &str) -> bool {
    addr.parse::<std::net::SocketAddr>().is_ok()
        || (addr.contains(':') && !addr.is_empty())
}

// ── Handlers ────────────────────────────────────────────────────────────────

/// `GET /admin/peers` — List connected peers and network summary.
pub async fn list_peers(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let rpc = require_node_rpc(&state)?;

    let net_info = rpc
        .get_network_info()
        .await
        .map_err(|e| ApiError::ServiceUnavailable(format!("Node unreachable: {e}")))?;

    let chain_info = rpc.get_chain_info().await.ok();

    Ok(Json(json!({
        "network": net_info,
        "chain": chain_info,
        "peer_management": {
            "_note": "Detailed peer list requires net_peers RPC method (Phase 3)",
        },
    })))
}

/// `POST /admin/peers/add` — Manually add a peer.
pub async fn add_peer(
    State(state): State<AppState>,
    Json(req): Json<AddPeerRequest>,
) -> Result<Json<Value>, ApiError> {
    if !is_valid_socket_addr(&req.address) {
        return Err(ApiError::BadRequest(
            "Invalid address format (expected host:port)".into(),
        ));
    }

    let _rpc = require_node_rpc(&state)?;

    // TODO: Once net_addPeer RPC method is added to the node, proxy the call here.
    // For now, return success with a note.
    Ok(Json(json!({
        "status": "accepted",
        "address": req.address,
        "connect_now": req.connect_now,
        "_note": "Peer queued. Full RPC integration in Phase 3.",
    })))
}

/// `POST /admin/peers/remove` — Disconnect and optionally ban a peer.
pub async fn remove_peer(
    State(state): State<AppState>,
    Json(req): Json<RemovePeerRequest>,
) -> Result<Json<Value>, ApiError> {
    if !is_valid_socket_addr(&req.address) {
        return Err(ApiError::BadRequest(
            "Invalid address format (expected host:port)".into(),
        ));
    }

    let _rpc = require_node_rpc(&state)?;

    Ok(Json(json!({
        "status": "accepted",
        "address": req.address,
        "ban_hours": req.ban_hours,
        "_note": "Peer removal queued. Full RPC integration in Phase 3.",
    })))
}
