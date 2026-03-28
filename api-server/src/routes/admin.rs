use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};

use crate::errors::ApiError;
use crate::AppState;

/// `GET /admin/stats` — Basic user signup and wallet analytics.
///
/// Returns mock data when Supabase is not configured.
pub async fn stats(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let supabase = match &state.supabase {
        Some(s) => s,
        None => {
            return Ok(Json(json!({
                "_warning": "Supabase not configured — returning mock data",
                "total_users": 0,
                "wallet_only_users": 0,
                "email_only_users": 0,
                "email_with_wallet_users": 0,
                "signups_last_24h": 0,
                "signups_last_7d": 0,
                "total_wallet_bindings": 0,
                "active_wallets": 0,
            })));
        }
    };

    match supabase.get_user_stats().await {
        Ok(stats) => Ok(Json(stats)),
        Err(e) => {
            tracing::warn!("Failed to fetch user stats: {e}");
            Ok(Json(json!({
                "_error": format!("Stats unavailable: {e}"),
                "total_users": 0,
                "wallet_only_users": 0,
                "email_only_users": 0,
                "email_with_wallet_users": 0,
                "signups_last_24h": 0,
                "signups_last_7d": 0,
                "total_wallet_bindings": 0,
                "active_wallets": 0,
            })))
        }
    }
}
