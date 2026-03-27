//! Lightweight Supabase client for user management via REST + Auth Admin API.
//!
//! Gracefully degrades when Supabase is not configured — the API server
//! continues to issue JWTs with `sub = wallet_address` as a fallback.

use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use std::fmt;

pub struct SupabaseClient {
    http: Client,
    url: String,
    anon_key: String,
    service_key: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SupabaseUser {
    pub id: String,
    pub wallet_address: String,
    pub email: Option<String>,
    pub created_at: Option<String>,
}

/// Result from Supabase email signup/signin.
#[derive(Debug)]
pub struct AuthResult {
    pub user_id: String,
    pub email: Option<String>,
    pub access_token: Option<String>,
    pub needs_confirmation: bool,
}

/// A row from the `wallet_bindings` table.
#[derive(Debug, Clone, Deserialize)]
pub struct WalletBinding {
    pub id: String,
    pub user_id: String,
    pub wallet_address: String,
    pub chain_id: String,
    pub is_primary: bool,
    pub bound_at: String,
}

#[derive(Debug)]
pub enum SupabaseError {
    RequestFailed(String),
    UserAlreadyExists(String),
    NotFound,
    Unauthorized,
    Unknown(String),
}

impl fmt::Display for SupabaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequestFailed(msg) => write!(f, "Request failed: {msg}"),
            Self::UserAlreadyExists(id) => write!(f, "User already exists: {id}"),
            Self::NotFound => write!(f, "Not found"),
            Self::Unauthorized => write!(f, "Unauthorized (missing service role key)"),
            Self::Unknown(msg) => write!(f, "Unknown error: {msg}"),
        }
    }
}

impl SupabaseClient {
    pub fn new(url: &str, anon_key: &str, service_key: Option<&str>) -> Self {
        Self {
            http: Client::new(),
            url: url.trim_end_matches('/').to_string(),
            anon_key: anon_key.to_string(),
            service_key: service_key.map(String::from),
        }
    }

    // ── Wallet-based user management ────────────────────────────────────────

    /// Create a user via the Supabase Auth admin API.
    ///
    /// Synthetic email: `{first_8_hex}@coinjecture.beans`
    pub async fn create_wallet_user(
        &self,
        wallet_address: &str,
    ) -> Result<SupabaseUser, SupabaseError> {
        let service_key = self
            .service_key
            .as_deref()
            .ok_or(SupabaseError::Unauthorized)?;

        let short_addr = &wallet_address[..wallet_address.len().min(8)];
        let email = format!("{short_addr}@coinjecture.beans");

        let body = serde_json::json!({
            "email": email,
            "email_confirm": true,
            "app_metadata": {
                "wallet_address": wallet_address,
                "provider": "siwb"
            },
            "user_metadata": {
                "wallet_address": wallet_address
            }
        });

        let resp = self
            .http
            .post(format!("{}/auth/v1/admin/users", self.url))
            .header("Authorization", format!("Bearer {service_key}"))
            .header("apikey", &self.anon_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| SupabaseError::RequestFailed(e.to_string()))?;

        if resp.status() == reqwest::StatusCode::CONFLICT {
            return match self.find_user_by_wallet(wallet_address).await? {
                Some(user) => Err(SupabaseError::UserAlreadyExists(user.id)),
                None => Err(SupabaseError::Unknown("Conflict but user not found".into())),
            };
        }

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(SupabaseError::RequestFailed(format!("{status}: {text}")));
        }

        #[derive(Deserialize)]
        struct AuthUser {
            id: String,
            email: Option<String>,
            created_at: Option<String>,
        }

        let auth_user: AuthUser = resp
            .json()
            .await
            .map_err(|e| SupabaseError::Unknown(e.to_string()))?;

        Ok(SupabaseUser {
            id: auth_user.id,
            wallet_address: wallet_address.to_string(),
            email: auth_user.email,
            created_at: auth_user.created_at,
        })
    }

    /// Look up a user by wallet address via the `wallet_bindings` PostgREST table.
    pub async fn find_user_by_wallet(
        &self,
        wallet_address: &str,
    ) -> Result<Option<SupabaseUser>, SupabaseError> {
        let resp = self
            .http
            .get(format!(
                "{}/rest/v1/wallet_bindings?wallet_address=eq.{}&select=user_id",
                self.url, wallet_address
            ))
            .header("Authorization", format!("Bearer {}", self.anon_key))
            .header("apikey", &self.anon_key)
            .send()
            .await
            .map_err(|e| SupabaseError::RequestFailed(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(SupabaseError::RequestFailed(format!(
                "Status: {}",
                resp.status()
            )));
        }

        #[derive(Deserialize)]
        struct Binding {
            user_id: String,
        }

        let bindings: Vec<Binding> = resp
            .json()
            .await
            .map_err(|e| SupabaseError::Unknown(e.to_string()))?;

        Ok(bindings.first().map(|b| SupabaseUser {
            id: b.user_id.clone(),
            wallet_address: wallet_address.to_string(),
            email: None,
            created_at: None,
        }))
    }

    /// Bind a wallet address to an existing Supabase user.
    pub async fn bind_wallet(
        &self,
        user_id: &str,
        wallet_address: &str,
        chain_id: &str,
        siwb_signature: &str,
        is_primary: bool,
    ) -> Result<(), SupabaseError> {
        let service_key = self
            .service_key
            .as_deref()
            .ok_or(SupabaseError::Unauthorized)?;

        let body = serde_json::json!({
            "user_id": user_id,
            "wallet_address": wallet_address,
            "chain_id": chain_id,
            "siwb_signature": siwb_signature,
            "is_primary": is_primary
        });

        let resp = self
            .http
            .post(format!("{}/rest/v1/wallet_bindings", self.url))
            .header("Authorization", format!("Bearer {service_key}"))
            .header("apikey", &self.anon_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| SupabaseError::RequestFailed(e.to_string()))?;

        if resp.status().is_success() || resp.status() == reqwest::StatusCode::CREATED {
            Ok(())
        } else {
            let text = resp.text().await.unwrap_or_default();
            Err(SupabaseError::RequestFailed(text))
        }
    }

    // ── Email-based auth ────────────────────────────────────────────────────

    /// Sign up with email + password via Supabase Auth.
    pub async fn email_signup(
        &self,
        email: &str,
        password: &str,
    ) -> Result<AuthResult, SupabaseError> {
        let body = serde_json::json!({
            "email": email,
            "password": password,
            "data": {
                "signup_source": "email",
                "signed_up_at": chrono::Utc::now().to_rfc3339()
            }
        });

        let resp = self
            .http
            .post(format!("{}/auth/v1/signup", self.url))
            .header("apikey", &self.anon_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| SupabaseError::RequestFailed(e.to_string()))?;

        if resp.status() == reqwest::StatusCode::UNPROCESSABLE_ENTITY {
            return Err(SupabaseError::UserAlreadyExists(email.to_string()));
        }

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(SupabaseError::RequestFailed(text));
        }

        let data: Value = resp
            .json()
            .await
            .map_err(|e| SupabaseError::Unknown(e.to_string()))?;

        let access_token = data["access_token"].as_str().map(String::from);
        let needs_confirmation = access_token.is_none();

        let user_id = data["user"]["id"]
            .as_str()
            .or_else(|| data["id"].as_str())
            .unwrap_or("")
            .to_string();

        Ok(AuthResult {
            user_id,
            email: Some(email.to_string()),
            access_token,
            needs_confirmation,
        })
    }

    /// Sign in with email + password.
    pub async fn email_signin(
        &self,
        email: &str,
        password: &str,
    ) -> Result<AuthResult, SupabaseError> {
        let body = serde_json::json!({
            "email": email,
            "password": password
        });

        let resp = self
            .http
            .post(format!("{}/auth/v1/token?grant_type=password", self.url))
            .header("apikey", &self.anon_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| SupabaseError::RequestFailed(e.to_string()))?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(SupabaseError::RequestFailed(text));
        }

        let data: Value = resp
            .json()
            .await
            .map_err(|e| SupabaseError::Unknown(e.to_string()))?;

        let user_id = data["user"]["id"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let user_email = data["user"]["email"].as_str().map(String::from);

        Ok(AuthResult {
            user_id,
            email: user_email,
            access_token: data["access_token"].as_str().map(String::from),
            needs_confirmation: false,
        })
    }

    /// Request a passwordless magic link email.
    pub async fn request_magic_link(&self, email: &str) -> Result<(), SupabaseError> {
        let body = serde_json::json!({ "email": email });

        let resp = self
            .http
            .post(format!("{}/auth/v1/magiclink", self.url))
            .header("apikey", &self.anon_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| SupabaseError::RequestFailed(e.to_string()))?;

        if resp.status().is_success() {
            Ok(())
        } else {
            let text = resp.text().await.unwrap_or_default();
            Err(SupabaseError::RequestFailed(text))
        }
    }

    /// Exchange a magic-link token for a session.
    pub async fn verify_magic_link(&self, token: &str) -> Result<AuthResult, SupabaseError> {
        let body = serde_json::json!({
            "token": token,
            "type": "magiclink"
        });

        let resp = self
            .http
            .post(format!("{}/auth/v1/verify", self.url))
            .header("apikey", &self.anon_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| SupabaseError::RequestFailed(e.to_string()))?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(SupabaseError::RequestFailed(text));
        }

        let data: Value = resp
            .json()
            .await
            .map_err(|e| SupabaseError::Unknown(e.to_string()))?;

        let user_id = data["user"]["id"]
            .as_str()
            .unwrap_or("")
            .to_string();

        Ok(AuthResult {
            user_id,
            email: data["user"]["email"].as_str().map(String::from),
            access_token: data["access_token"].as_str().map(String::from),
            needs_confirmation: false,
        })
    }

    /// Find all wallet bindings for a specific user ID.
    pub async fn find_wallets_for_user(
        &self,
        user_id: &str,
    ) -> Result<Vec<WalletBinding>, SupabaseError> {
        let resp = self
            .http
            .get(format!(
                "{}/rest/v1/wallet_bindings?user_id=eq.{}&select=*",
                self.url, user_id
            ))
            .header("Authorization", format!("Bearer {}", self.anon_key))
            .header("apikey", &self.anon_key)
            .send()
            .await
            .map_err(|e| SupabaseError::RequestFailed(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(SupabaseError::RequestFailed(format!(
                "Status: {}",
                resp.status()
            )));
        }

        resp.json()
            .await
            .map_err(|e| SupabaseError::Unknown(e.to_string()))
    }

    // ── Admin ───────────────────────────────────────────────────────────────

    /// Call the `get_user_stats()` RPC function via PostgREST.
    pub async fn get_user_stats(&self) -> Result<Value, SupabaseError> {
        let key = self
            .service_key
            .as_deref()
            .unwrap_or(&self.anon_key);

        let resp = self
            .http
            .post(format!("{}/rest/v1/rpc/get_user_stats", self.url))
            .header("Authorization", format!("Bearer {key}"))
            .header("apikey", &self.anon_key)
            .header("Content-Type", "application/json")
            .body("{}")
            .send()
            .await
            .map_err(|e| SupabaseError::RequestFailed(e.to_string()))?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(SupabaseError::RequestFailed(text));
        }

        resp.json()
            .await
            .map_err(|e| SupabaseError::Unknown(e.to_string()))
    }

    // ── Marketplace queries ─────────────────────────────────────────────────

    /// Generic PostgREST GET query.
    async fn postgrest_get(&self, path: &str) -> Result<Value, SupabaseError> {
        let resp = self
            .http
            .get(format!("{}/rest/v1/{path}", self.url))
            .header("Authorization", format!("Bearer {}", self.anon_key))
            .header("apikey", &self.anon_key)
            .send()
            .await
            .map_err(|e| SupabaseError::RequestFailed(e.to_string()))?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(SupabaseError::RequestFailed(text));
        }

        resp.json()
            .await
            .map_err(|e| SupabaseError::Unknown(e.to_string()))
    }

    /// Generic PostgREST INSERT (returns the inserted row).
    pub async fn insert_row(&self, table: &str, body: Value) -> Result<Value, SupabaseError> {
        let key = self.service_key.as_deref().unwrap_or(&self.anon_key);

        let resp = self
            .http
            .post(format!("{}/rest/v1/{table}", self.url))
            .header("Authorization", format!("Bearer {key}"))
            .header("apikey", &self.anon_key)
            .header("Prefer", "return=representation")
            .json(&body)
            .send()
            .await
            .map_err(|e| SupabaseError::RequestFailed(e.to_string()))?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(SupabaseError::RequestFailed(text));
        }

        resp.json()
            .await
            .map_err(|e| SupabaseError::Unknown(e.to_string()))
    }

    /// Get active trading pairs.
    pub async fn get_trading_pairs(&self) -> Result<Value, SupabaseError> {
        self.postgrest_get("trading_pairs?is_active=eq.true&select=*")
            .await
    }

    /// Get open orders for an order book view.
    pub async fn get_order_book(&self, pair_id: &str) -> Result<Value, SupabaseError> {
        self.postgrest_get(&format!(
            "orders?pair_id=eq.{pair_id}&status=eq.open&select=side,price,quantity&order=price.asc"
        ))
        .await
    }

    /// Cancel an order (set status to cancelled).
    pub async fn cancel_order(&self, order_id: &str, user_id: &str) -> Result<(), SupabaseError> {
        let key = self.service_key.as_deref().unwrap_or(&self.anon_key);
        let resp = self
            .http
            .patch(format!(
                "{}/rest/v1/orders?id=eq.{order_id}&user_id=eq.{user_id}",
                self.url
            ))
            .header("Authorization", format!("Bearer {key}"))
            .header("apikey", &self.anon_key)
            .json(&serde_json::json!({ "status": "cancelled" }))
            .send()
            .await
            .map_err(|e| SupabaseError::RequestFailed(e.to_string()))?;

        if resp.status().is_success() {
            Ok(())
        } else {
            let text = resp.text().await.unwrap_or_default();
            Err(SupabaseError::RequestFailed(text))
        }
    }

    /// Get recent trades for a pair.
    pub async fn get_recent_trades(
        &self,
        pair_id: &str,
        limit: usize,
    ) -> Result<Value, SupabaseError> {
        self.postgrest_get(&format!(
            "trades?pair_id=eq.{pair_id}&order=executed_at.desc&limit={limit}&select=price,quantity,executed_at,buyer_wallet,seller_wallet"
        ))
        .await
    }

    /// Get open PoUW tasks with optional class filter.
    pub async fn get_open_tasks(
        &self,
        class_filter: Option<&str>,
    ) -> Result<Value, SupabaseError> {
        let filter = match class_filter {
            Some(c) => format!("&problem_class=eq.{c}"),
            None => String::new(),
        };
        self.postgrest_get(&format!(
            "pouw_tasks?status=eq.open{filter}&order=created_at.desc&select=*"
        ))
        .await
    }
}
