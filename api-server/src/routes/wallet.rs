use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::Value;

use crate::errors::ApiError;
use crate::AppState;

#[derive(Deserialize)]
pub struct TxQuery {
    pub address: String,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 {
    20
}

pub async fn get_transactions(
    State(state): State<AppState>,
    Query(params): Query<TxQuery>,
) -> Result<Json<Value>, ApiError> {
    let supabase = state
        .supabase
        .as_ref()
        .ok_or_else(|| ApiError::ServiceUnavailable("Supabase not configured".into()))?;
    let limit = params.limit.min(100);
    let txs = supabase
        .get_wallet_transactions(&params.address, limit)
        .await
        .map_err(|e| ApiError::ServiceUnavailable(format!("DB error: {e}")))?;
    Ok(Json(txs))
}
