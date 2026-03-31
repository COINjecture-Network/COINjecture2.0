use axum::extract::State;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use serde::Serialize;
use uuid::Uuid;

use crate::crypto::verify_ed25519_signature;
use crate::errors::ApiError;
use crate::jwt::{decode_token, issue_token};
use crate::nonce_store::NonceEntry;
use crate::siwb::SiwbMessage;
use crate::AppState;

// ── Request / response types ────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ChallengeRequest {
    pub wallet_address: String,
}

#[derive(Deserialize)]
pub struct VerifyRequest {
    pub wallet_address: String,
    pub signature: String,
    pub message: String,
}

#[derive(Serialize)]
pub struct MeResponse {
    /// Supabase user id (JWT `sub`) — stable for email- and wallet-backed sessions.
    sub: String,
    wallet_address: Option<String>,
    email: Option<String>,
    network: String,
    issued_at: Option<String>,
    expires_at: Option<String>,
}

// ── Handlers ────────────────────────────────────────────────────────────────

/// `POST /auth/challenge` — generate a SIWB challenge message and nonce.
pub async fn challenge(
    State(state): State<AppState>,
    Json(req): Json<ChallengeRequest>,
) -> Result<Json<Value>, ApiError> {
    // Validate: 64 hex chars = 32-byte Ed25519 public key
    if req.wallet_address.len() != 64 {
        return Err(ApiError::BadRequest(
            "wallet_address must be 64 hex characters (32 bytes)".into(),
        ));
    }
    if hex::decode(&req.wallet_address).is_err() {
        return Err(ApiError::BadRequest(
            "wallet_address must be valid hex".into(),
        ));
    }

    let nonce = Uuid::new_v4().to_string();
    let siwb = SiwbMessage::new(&req.wallet_address, &nonce, &state.config.network);
    let message = siwb.to_message_string();

    let entry = NonceEntry {
        wallet_address: req.wallet_address,
        message: message.clone(),
        created_at: siwb.issued_at,
        expires_at: siwb.expiration_time,
    };

    state
        .nonce_store
        .insert(nonce.clone(), entry)
        .map_err(|e| ApiError::Internal(format!("Nonce store error: {e}")))?;

    Ok(Json(json!({
        "message": message,
        "nonce": nonce,
    })))
}

/// `POST /auth/verify` — verify an Ed25519-signed SIWB message and issue a JWT.
pub async fn verify(
    State(state): State<AppState>,
    Json(req): Json<VerifyRequest>,
) -> Result<Json<Value>, ApiError> {
    if req.wallet_address.is_empty() || req.signature.is_empty() || req.message.is_empty() {
        return Err(ApiError::BadRequest("All fields are required".into()));
    }

    // Generic error to avoid leaking which step failed
    let auth_err = || ApiError::Unauthorized("Invalid signature or expired challenge".into());

    // 1. Parse the SIWB message to extract the nonce
    let siwb = SiwbMessage::from_message_string(&req.message).map_err(|_| auth_err())?;

    // 2. Validate & consume the nonce (one-time use = replay prevention)
    state
        .nonce_store
        .validate_and_remove(&siwb.nonce, &req.wallet_address, &req.message)
        .map_err(|_| auth_err())?;

    // 3. Decode hex → raw bytes
    let pubkey_bytes = hex::decode(&req.wallet_address).map_err(|_| auth_err())?;
    let sig_bytes = hex::decode(&req.signature).map_err(|_| auth_err())?;

    // 4. Verify Ed25519 signature
    let valid = verify_ed25519_signature(&pubkey_bytes, req.message.as_bytes(), &sig_bytes)
        .map_err(|_| auth_err())?;

    if !valid {
        return Err(auth_err());
    }

    // 5. Supabase user lookup / creation (graceful degradation)
    let user_id = resolve_user(&state, &req.wallet_address, &req.signature).await;

    // 6. Issue JWT
    let token = issue_token(
        &state.config.supabase_jwt_secret,
        &user_id,
        Some(&req.wallet_address),
        None,
        &state.config.network,
        state.config.jwt_expiry_seconds,
    )?;

    Ok(Json(json!({
        "token": token,
        "user": {
            "id": user_id,
            "wallet_address": req.wallet_address,
        },
    })))
}

/// `GET /auth/me` — validate JWT and return session info.
pub async fn me(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<MeResponse>, ApiError> {
    let bearer = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| ApiError::Unauthorized("Missing or invalid Authorization header".into()))?;

    let c = decode_token(&state.config.supabase_jwt_secret, bearer)?;

    Ok(Json(MeResponse {
        sub: c.sub,
        wallet_address: c.wallet_address,
        email: c.email,
        network: c.network,
        issued_at: chrono::DateTime::from_timestamp(c.iat, 0).map(|dt| dt.to_rfc3339()),
        expires_at: chrono::DateTime::from_timestamp(c.exp, 0).map(|dt| dt.to_rfc3339()),
    }))
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Try to resolve a Supabase user ID; fall back to wallet address if Supabase
/// is unconfigured or unreachable.
async fn resolve_user(state: &AppState, wallet_address: &str, signature: &str) -> String {
    let supabase = match state.supabase {
        Some(ref s) => s,
        None => return wallet_address.to_string(),
    };

    match supabase.find_user_by_wallet(wallet_address).await {
        Ok(Some(user)) => return user.id,
        Ok(None) => {}
        Err(e) => {
            tracing::warn!("Supabase lookup failed, using wallet address: {e}");
            return wallet_address.to_string();
        }
    }

    match supabase.create_wallet_user(wallet_address).await {
        Ok(user) => {
            let chain_id = format!("coinjecture:{}", state.config.network);
            if let Err(e) = supabase
                .bind_wallet(&user.id, wallet_address, &chain_id, signature, true)
                .await
            {
                tracing::warn!("Failed to bind wallet after creation: {e}");
            }
            user.id
        }
        Err(crate::supabase::SupabaseError::UserAlreadyExists(id)) => id,
        Err(e) => {
            tracing::warn!("Supabase user creation failed, using wallet address: {e}");
            wallet_address.to_string()
        }
    }
}
