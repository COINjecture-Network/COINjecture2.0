//! Reusable JWT extractor for protected routes.

use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;

use crate::errors::ApiError;
use crate::jwt::decode_token;
use crate::AppState;

/// Extractor that validates the `Authorization: Bearer <token>` header and
/// provides the authenticated user's identity to the handler.
///
/// Usage: `async fn handler(user: AuthenticatedUser) -> impl IntoResponse { ... }`
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: String,
    pub wallet_address: Option<String>,
    pub email: Option<String>,
    pub network: String,
}

impl FromRequestParts<AppState> for AuthenticatedUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let bearer = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or_else(|| {
                ApiError::Unauthorized("Missing or invalid Authorization header".into())
            })?;

        let claims = decode_token(&state.config.supabase_jwt_secret, bearer)?;

        Ok(AuthenticatedUser {
            user_id: claims.sub,
            wallet_address: claims.wallet_address,
            email: claims.email,
            network: claims.network,
        })
    }
}
