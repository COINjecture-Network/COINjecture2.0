use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use tracing::warn;

use crate::errors::ApiError;
use crate::wallet_activity;
use crate::{AppState, WalletScanCacheEntry};

#[derive(Deserialize)]
pub struct TxQuery {
    pub address: String,
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}

fn default_limit() -> u32 {
    100
}

const MAX_PAGE: u32 = 500;
const MAX_OFFSET: u32 = 50_000;
const SCAN_CACHE_TTL: Duration = Duration::from_secs(180);

fn paginate_rows(rows: &Value, offset: usize, limit: usize) -> Value {
    let Some(arr) = rows.as_array() else {
        return json!([]);
    };
    Value::Array(arr.iter().skip(offset).take(limit).cloned().collect())
}

async fn cached_full_chain_activity(state: &AppState, addr: &str) -> Result<Value, String> {
    let node_rpc = state
        .node_rpc
        .as_ref()
        .ok_or_else(|| "NODE_RPC_URL not configured".to_string())?;
    let tip = node_rpc
        .get_chain_info()
        .await
        .map_err(|e| format!("chain info: {e}"))?["best_height"]
        .as_u64()
        .unwrap_or(0);

    {
        let cache = state
            .wallet_scan_cache
            .lock()
            .expect("wallet scan cache lock");
        if let Some(entry) = cache.get(addr) {
            if entry.chain_tip == tip && entry.fetched_at.elapsed() < SCAN_CACHE_TTL {
                return Ok(entry.merged.clone());
            }
        }
    }

    let merged = wallet_activity::scan_wallet_activity_from_chain(
        node_rpc,
        addr,
        usize::MAX,
        0,
        0, // entire chain
    )
    .await?;

    {
        let mut cache = state
            .wallet_scan_cache
            .lock()
            .expect("wallet scan cache lock");
        cache.insert(
            addr.to_string(),
            WalletScanCacheEntry {
                chain_tip: tip,
                merged: merged.clone(),
                fetched_at: Instant::now(),
            },
        );
    }

    Ok(merged)
}

pub async fn get_transactions(
    State(state): State<AppState>,
    Query(params): Query<TxQuery>,
) -> Result<Json<Value>, ApiError> {
    if state.supabase.is_none() && state.node_rpc.is_none() {
        return Err(ApiError::ServiceUnavailable(
            "Wallet history requires Supabase or NODE_RPC_URL".into(),
        ));
    }

    let addr = wallet_activity::normalize_hex_address(&params.address).map_err(|e| {
        ApiError::BadRequest(format!(
            "Invalid wallet address (expected 64 hex chars / 32-byte pubkey): {e}"
        ))
    })?;

    let limit = params.limit.clamp(1, MAX_PAGE) as usize;
    let offset = params.offset.min(MAX_OFFSET) as usize;
    let db_cap = (offset + limit).min(5000).max(limit) as u32;

    let mut supabase_degraded = false;

    let (signed, incoming, mined, marketplace) = if let Some(supabase) = state.supabase.as_ref() {
        let signed = match supabase.get_wallet_transactions(&addr, db_cap).await {
            Ok(v) => v,
            Err(e) => {
                supabase_degraded = true;
                warn!(error = %e, "signed tx query failed; falling back to chain scan if needed");
                json!([])
            }
        };

        let incoming = match supabase.get_wallet_incoming_transfers(&addr, db_cap).await {
            Ok(v) => v,
            Err(e) => {
                supabase_degraded = true;
                warn!(error = %e, "incoming transfer query failed; continuing without inbound rows");
                json!([])
            }
        };

        let mined = match supabase.get_wallet_mined_blocks(&addr, db_cap).await {
            Ok(v) => v,
            Err(e) => {
                supabase_degraded = true;
                warn!(error = %e, "mined blocks query failed; continuing without mining rows");
                json!([])
            }
        };

        let marketplace = match supabase.get_wallet_marketplace_events(&addr, db_cap).await {
            Ok(v) => v,
            Err(e) => {
                supabase_degraded = true;
                warn!(error = %e, "marketplace events query failed; continuing without bounty rows");
                json!([])
            }
        };

        (signed, incoming, mined, marketplace)
    } else {
        supabase_degraded = true;
        (json!([]), json!([]), json!([]), json!([]))
    };

    if !supabase_degraded {
        let merged = wallet_activity::merge_wallet_activity(
            &addr,
            limit,
            offset,
            signed,
            incoming,
            mined,
            marketplace,
        );
        if merged.as_array().is_some_and(|rows| !rows.is_empty()) || state.node_rpc.is_none() {
            return Ok(Json(merged));
        }
        // Index tables empty (indexer catching up) — fall through to full chain scan.
    }

    // Supabase unavailable, empty, or catching up — full-chain scan (cached) for complete history.
    match cached_full_chain_activity(&state, &addr).await {
        Ok(full_chain) => Ok(Json(paginate_rows(&full_chain, offset, limit))),
        Err(e) => {
            warn!(error = %e, address = %addr, "full chain wallet scan failed");
            Err(ApiError::ServiceUnavailable(format!(
                "Wallet history unavailable (Supabase indexer down and chain scan failed: {e})"
            )))
        }
    }
}
