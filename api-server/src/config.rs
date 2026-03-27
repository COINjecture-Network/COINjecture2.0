/// Typed configuration loaded from environment variables.
///
/// Required: `SUPABASE_URL`, `SUPABASE_ANON_KEY`, `SUPABASE_JWT_SECRET`
/// Optional (with defaults): everything else — see [`Config::from_env`].
#[derive(Clone, Debug)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub supabase_url: String,
    pub supabase_anon_key: String,
    pub supabase_jwt_secret: String,
    pub supabase_service_role_key: Option<String>,
    pub jwt_expiry_seconds: u64,
    pub rate_limit_rps: u32,
    pub network: String,
    pub node_rpc_url: Option<String>,
}

impl Config {
    /// Read configuration from environment variables (with `dotenvy` already loaded).
    pub fn from_env() -> Result<Self, String> {
        Ok(Config {
            host: std::env::var("COINJECTURE_API_HOST")
                .unwrap_or_else(|_| "0.0.0.0".into()),
            port: std::env::var("COINJECTURE_API_PORT")
                .unwrap_or_else(|_| "3030".into())
                .parse()
                .map_err(|e| format!("Invalid COINJECTURE_API_PORT: {e}"))?,
            supabase_url: std::env::var("SUPABASE_URL")
                .map_err(|_| "SUPABASE_URL is required")?,
            supabase_anon_key: std::env::var("SUPABASE_ANON_KEY")
                .map_err(|_| "SUPABASE_ANON_KEY is required")?,
            supabase_jwt_secret: std::env::var("SUPABASE_JWT_SECRET")
                .map_err(|_| "SUPABASE_JWT_SECRET is required")?,
            supabase_service_role_key: std::env::var("SUPABASE_SERVICE_ROLE_KEY").ok(),
            jwt_expiry_seconds: std::env::var("JWT_EXPIRY_SECONDS")
                .unwrap_or_else(|_| "86400".into())
                .parse()
                .map_err(|e| format!("Invalid JWT_EXPIRY_SECONDS: {e}"))?,
            rate_limit_rps: std::env::var("RATE_LIMIT_RPS")
                .unwrap_or_else(|_| "100".into())
                .parse()
                .map_err(|e| format!("Invalid RATE_LIMIT_RPS: {e}"))?,
            network: std::env::var("COINJECTURE_NETWORK")
                .unwrap_or_else(|_| "testnet".into()),
            node_rpc_url: std::env::var("NODE_RPC_URL").ok(),
        })
    }

    /// Returns `true` when all required fields are non-empty.
    pub fn is_valid(&self) -> bool {
        !self.supabase_url.is_empty()
            && !self.supabase_anon_key.is_empty()
            && !self.supabase_jwt_secret.is_empty()
    }
}
