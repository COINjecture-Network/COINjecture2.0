//! Shared JWT types and helpers used by SIWB auth, email auth, and middleware.

use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use crate::errors::ApiError;

/// JWT claims for all COINjecture auth tokens.
///
/// Both wallet-only and email-only users share the same claim schema.
/// Fields that don't apply to the auth method are `None`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub wallet_address: Option<String>,
    pub email: Option<String>,
    pub network: String,
    pub iat: i64,
    pub exp: i64,
    pub iss: String,
    pub aud: String,
}

/// Issue a signed JWT.
pub fn issue_token(
    secret: &str,
    sub: &str,
    wallet_address: Option<&str>,
    email: Option<&str>,
    network: &str,
    expiry_seconds: u64,
) -> Result<String, ApiError> {
    let now = Utc::now();
    let exp = now + chrono::Duration::seconds(expiry_seconds as i64);

    let claims = Claims {
        sub: sub.to_string(),
        wallet_address: wallet_address.map(String::from),
        email: email.map(String::from),
        network: network.to_string(),
        iat: now.timestamp(),
        exp: exp.timestamp(),
        iss: "coinjecture-api".into(),
        aud: "coinjecture-app".into(),
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| ApiError::Internal(format!("JWT encoding failed: {e}")))
}

/// Decode and validate a JWT, returning the claims.
pub fn decode_token(secret: &str, token: &str) -> Result<Claims, ApiError> {
    let mut validation = Validation::default();
    validation.set_issuer(&["coinjecture-api"]);
    validation.set_audience(&["coinjecture-app"]);

    decode::<Claims>(token, &DecodingKey::from_secret(secret.as_bytes()), &validation)
        .map(|data| data.claims)
        .map_err(|_| ApiError::Unauthorized("Invalid or expired token".into()))
}
