use axum::extract::State;
use axum::Json;
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::crypto::verify_ed25519_signature;
use crate::errors::ApiError;
use crate::jwt::issue_token;
use crate::middleware::jwt_auth::AuthenticatedUser;
use crate::AppState;

// ── Request types ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SignupRequest {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct SigninRequest {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct MagicLinkRequest {
    pub email: String,
}

#[derive(Deserialize)]
pub struct VerifyMagicLinkRequest {
    pub token: String,
    #[serde(rename = "type")]
    pub token_type: Option<String>,
}

#[derive(Deserialize)]
pub struct BindWalletRequest {
    pub wallet_address: String,
    pub signature: String,
    pub message: String,
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn is_valid_email(email: &str) -> bool {
    let parts: Vec<&str> = email.split('@').collect();
    parts.len() == 2
        && !parts[0].is_empty()
        && parts[1].contains('.')
        && !parts[1].starts_with('.')
        && !parts[1].ends_with('.')
}

fn require_supabase(
    state: &AppState,
) -> Result<&std::sync::Arc<crate::supabase::SupabaseClient>, ApiError> {
    state
        .supabase
        .as_ref()
        .ok_or_else(|| ApiError::ServiceUnavailable("Email auth requires Supabase configuration".into()))
}

// ── Handlers ────────────────────────────────────────────────────────────────

/// `POST /auth/email/signup` — Register with email + password.
pub async fn signup(
    State(state): State<AppState>,
    Json(req): Json<SignupRequest>,
) -> Result<Json<Value>, ApiError> {
    // Validate BEFORE checking Supabase
    if !is_valid_email(&req.email) {
        return Err(ApiError::BadRequest("Invalid email format".into()));
    }
    if req.password.len() < 8 {
        return Err(ApiError::BadRequest(
            "Password must be at least 8 characters".into(),
        ));
    }

    let supabase = require_supabase(&state)?;

    let result = supabase.email_signup(&req.email, &req.password).await.map_err(|e| {
        match e {
            crate::supabase::SupabaseError::UserAlreadyExists(_) => {
                ApiError::BadRequest("An account with this email already exists".into())
            }
            _ => ApiError::Internal(format!("Signup failed: {e}")),
        }
    })?;

    if result.needs_confirmation {
        return Ok(Json(json!({
            "message": "Check your email for confirmation",
            "user_id": result.user_id,
        })));
    }

    let token = issue_token(
        &state.config.supabase_jwt_secret,
        &result.user_id,
        None,
        Some(&req.email),
        &state.config.network,
        state.config.jwt_expiry_seconds,
    )?;

    Ok(Json(json!({
        "message": "Account created",
        "token": token,
        "user": {
            "id": result.user_id,
            "email": req.email,
            "wallet_address": null,
        },
    })))
}

/// `POST /auth/email/signin` — Sign in with email + password.
pub async fn signin(
    State(state): State<AppState>,
    Json(req): Json<SigninRequest>,
) -> Result<Json<Value>, ApiError> {
    if req.email.is_empty() || req.password.is_empty() {
        return Err(ApiError::BadRequest("Email and password are required".into()));
    }

    let supabase = require_supabase(&state)?;

    let result = supabase.email_signin(&req.email, &req.password).await.map_err(|e| {
        ApiError::Unauthorized(format!("Sign-in failed: {e}"))
    })?;

    // Look up wallet binding for this user
    let wallet_address = match supabase.find_wallets_for_user(&result.user_id).await {
        Ok(bindings) => bindings
            .iter()
            .find(|b| b.is_primary)
            .or(bindings.first())
            .map(|b| b.wallet_address.clone()),
        Err(e) => {
            tracing::warn!("Failed to look up wallet bindings: {e}");
            None
        }
    };

    let token = issue_token(
        &state.config.supabase_jwt_secret,
        &result.user_id,
        wallet_address.as_deref(),
        Some(&req.email),
        &state.config.network,
        state.config.jwt_expiry_seconds,
    )?;

    Ok(Json(json!({
        "token": token,
        "user": {
            "id": result.user_id,
            "email": req.email,
            "wallet_address": wallet_address,
        },
    })))
}

/// `POST /auth/email/magic-link` — Request a passwordless magic link.
pub async fn request_magic_link(
    State(state): State<AppState>,
    Json(req): Json<MagicLinkRequest>,
) -> Result<Json<Value>, ApiError> {
    if !is_valid_email(&req.email) {
        return Err(ApiError::BadRequest("Invalid email format".into()));
    }

    let supabase = require_supabase(&state)?;

    // Always return success to avoid leaking whether the email exists
    let _ = supabase.request_magic_link(&req.email).await;

    Ok(Json(json!({
        "message": "If an account exists, a sign-in link has been sent to your email",
    })))
}

/// `POST /auth/email/verify-magic-link` — Exchange a magic-link token for a session.
pub async fn verify_magic_link(
    State(state): State<AppState>,
    Json(req): Json<VerifyMagicLinkRequest>,
) -> Result<Json<Value>, ApiError> {
    let supabase = require_supabase(&state)?;

    let result = supabase
        .verify_magic_link(&req.token)
        .await
        .map_err(|e| ApiError::Unauthorized(format!("Verification failed: {e}")))?;

    let wallet_address = match supabase.find_wallets_for_user(&result.user_id).await {
        Ok(bindings) => bindings
            .iter()
            .find(|b| b.is_primary)
            .or(bindings.first())
            .map(|b| b.wallet_address.clone()),
        Err(_) => None,
    };

    let token = issue_token(
        &state.config.supabase_jwt_secret,
        &result.user_id,
        wallet_address.as_deref(),
        result.email.as_deref(),
        &state.config.network,
        state.config.jwt_expiry_seconds,
    )?;

    Ok(Json(json!({
        "token": token,
        "user": {
            "id": result.user_id,
            "email": result.email,
            "wallet_address": wallet_address,
        },
    })))
}

/// `POST /auth/email/bind-wallet` — Bind a wallet to an email account (requires auth).
pub async fn bind_wallet(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(req): Json<BindWalletRequest>,
) -> Result<Json<Value>, ApiError> {
    // Validate wallet address BEFORE checking Supabase
    if req.wallet_address.len() != 64 {
        return Err(ApiError::BadRequest(
            "wallet_address must be 64 hex characters".into(),
        ));
    }
    if hex::decode(&req.wallet_address).is_err() {
        return Err(ApiError::BadRequest("wallet_address must be valid hex".into()));
    }

    let supabase = require_supabase(&state)?;

    // Verify the bind message contains this user and wallet
    if !req.message.contains(&req.wallet_address) || !req.message.contains(&auth_user.user_id) {
        return Err(ApiError::BadRequest(
            "Bind message must contain your wallet address and user ID".into(),
        ));
    }

    // Check message timestamp is recent (within 5 minutes)
    let ts_prefix = "Timestamp: ";
    if let Some(ts_line) = req.message.lines().find(|l| l.starts_with(ts_prefix)) {
        let ts_str = ts_line.strip_prefix(ts_prefix).unwrap_or("");
        if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(ts_str) {
            let age = Utc::now().signed_duration_since(ts.with_timezone(&Utc));
            if age.num_seconds().unsigned_abs() > 300 {
                return Err(ApiError::Unauthorized("Bind message expired".into()));
            }
        }
    }

    // Verify Ed25519 signature
    let pubkey_bytes =
        hex::decode(&req.wallet_address).map_err(|_| ApiError::BadRequest("Invalid hex".into()))?;
    let sig_bytes =
        hex::decode(&req.signature).map_err(|_| ApiError::BadRequest("Invalid signature hex".into()))?;

    let valid =
        verify_ed25519_signature(&pubkey_bytes, req.message.as_bytes(), &sig_bytes)
            .map_err(|_| ApiError::Unauthorized("Signature verification failed".into()))?;

    if !valid {
        return Err(ApiError::Unauthorized("Invalid signature".into()));
    }

    // Check wallet isn't already bound to another user
    if let Ok(Some(existing)) = supabase.find_user_by_wallet(&req.wallet_address).await {
        if existing.id != auth_user.user_id {
            return Err(ApiError::BadRequest(
                "This wallet is already bound to another account".into(),
            ));
        }
    }

    // Bind the wallet
    let chain_id = format!("coinjecture:{}", state.config.network);
    supabase
        .bind_wallet(
            &auth_user.user_id,
            &req.wallet_address,
            &chain_id,
            &req.signature,
            true,
        )
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to bind wallet: {e}")))?;

    // Issue new JWT with wallet_address included
    let token = issue_token(
        &state.config.supabase_jwt_secret,
        &auth_user.user_id,
        Some(&req.wallet_address),
        auth_user.email.as_deref(),
        &state.config.network,
        state.config.jwt_expiry_seconds,
    )?;

    Ok(Json(json!({
        "token": token,
        "wallet_address": req.wallet_address,
    })))
}
