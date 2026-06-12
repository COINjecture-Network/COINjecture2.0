pub mod config;
pub mod crypto;
pub mod errors;
pub mod indexer;
pub mod jwt;
pub mod matching;
pub mod metrics;
pub mod middleware;
pub mod node_poller;
pub mod node_rpc;
pub mod nonce_store;
pub mod routes;
pub mod siwb;
pub mod sse;
pub mod supabase;
pub mod wallet_activity;

use config::Config;
use matching::engine::EngineHandle;
use metrics_exporter_prometheus::PrometheusHandle;
use middleware::rate_limit::KeyedRateLimiter;
use node_rpc::NodeRpcClient;
use nonce_store::NonceStore;
use serde_json::Value;
use sse::EventBroadcaster;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use supabase::SupabaseClient;

/// Cached full-chain wallet activity (used when Supabase indexer is unavailable).
#[derive(Clone)]
pub struct WalletScanCacheEntry {
    pub chain_tip: u64,
    pub merged: Value,
    pub fetched_at: Instant,
}

pub type WalletScanCache = Arc<Mutex<HashMap<String, WalletScanCacheEntry>>>;

/// Shared application state threaded through all handlers via axum's `State`.
#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub nonce_store: Arc<NonceStore>,
    pub rate_limiter: KeyedRateLimiter,
    pub metrics_handle: PrometheusHandle,
    pub supabase: Option<Arc<SupabaseClient>>,
    pub node_rpc: Option<Arc<NodeRpcClient>>,
    pub broadcaster: Arc<EventBroadcaster>,
    pub engine: Option<EngineHandle>,
    pub wallet_scan_cache: WalletScanCache,
}

/// Build the full axum router.
pub fn build_router(state: AppState) -> axum::Router {
    routes::build_routes(state)
}
