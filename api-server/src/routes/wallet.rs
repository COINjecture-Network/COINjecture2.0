use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::warn;

use crate::errors::ApiError;
use crate::wallet_activity;
use crate::AppState;

#[derive(Deserialize)]
pub struct TxQuery {
    pub address: String,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 {
    40
}

pub async fn get_transactions(
    State(state): State<AppState>,
    Query(params): Query<TxQuery>,
) -> Result<Json<Value>, ApiError> {
    let supabase = state
        .supabase
        .as_ref()
        .ok_or_else(|| ApiError::ServiceUnavailable("Supabase not configured".into()))?;

    let addr = wallet_activity::normalize_hex_address(&params.address).map_err(|e| {
        ApiError::BadRequest(format!(
            "Invalid wallet address (expected 64 hex chars / 32-byte pubkey): {e}"
        ))
    })?;

    let limit = (params.limit.min(100)) as usize;
    let cap = (limit.saturating_mul(4)).clamp(limit, 100) as u32;

    let signed = supabase
        .get_wallet_transactions(&addr, cap)
        .await
        .map_err(|e| ApiError::ServiceUnavailable(format!("DB error (signed txs): {e}")))?;

    let incoming = match supabase.get_wallet_incoming_transfers(&addr, cap).await {
        Ok(v) => v,
        Err(e) => {
            warn!(error = %e, "incoming transfer query failed; continuing without inbound rows");
            json!([])
        }
    };

    let mined = match supabase.get_wallet_mined_blocks(&addr, cap).await {
        Ok(v) => v,
        Err(e) => {
            warn!(error = %e, "mined blocks query failed; continuing without mining rows");
            json!([])
        }
    };

    let marketplace = match supabase.get_wallet_marketplace_events(&addr, cap).await {
        Ok(v) => v,
        Err(e) => {
            warn!(error = %e, "marketplace events query failed; continuing without bounty rows");
            json!([])
        }
    };

    let merged = wallet_activity::merge_wallet_activity(
        &addr,
        limit,
        signed,
        incoming,
        mined,
        marketplace,
    );
    Ok(Json(merged))
}
