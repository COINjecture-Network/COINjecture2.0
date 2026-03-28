use coinjecture_api_server::{
    build_router,
    config::Config,
    matching::{engine::{EngineHandle, MatchingEngine}, outbox::TradeOutbox},
    metrics::init_metrics,
    middleware::rate_limit::create_rate_limiter,
    node_poller::NodePoller,
    node_rpc::NodeRpcClient,
    nonce_store::NonceStore,
    sse::EventBroadcaster,
    supabase::SupabaseClient,
    AppState,
};
use std::sync::Arc;
use std::time::Duration;
use tracing_subscriber::{fmt, EnvFilter};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    // ── Tracing ─────────────────────────────────────────────────────────────
    let rust_log = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into());
    if rust_log.contains("json") {
        fmt()
            .with_env_filter(EnvFilter::new(&rust_log))
            .json()
            .init();
    } else {
        fmt()
            .with_env_filter(EnvFilter::new(&rust_log))
            .pretty()
            .init();
    }

    // ── Config ──────────────────────────────────────────────────────────────
    let config = Config::from_env().expect("Failed to load configuration");

    // ── Prometheus metrics ──────────────────────────────────────────────────
    let metrics_handle = init_metrics();

    // ── Nonce store + background cleanup ────────────────────────────────────
    let nonce_store = Arc::new(NonceStore::new(10_000));
    {
        let cleanup = nonce_store.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                let removed = cleanup.cleanup_expired();
                if removed > 0 {
                    tracing::debug!(removed, "Cleaned up expired nonces");
                }
            }
        });
    }

    // ── Optional Supabase client ────────────────────────────────────────────
    let supabase = if !config.supabase_url.is_empty()
        && config.supabase_url != "https://your-project.supabase.co"
    {
        tracing::info!("Supabase configured: {}", config.supabase_url);
        Some(Arc::new(SupabaseClient::new(
            &config.supabase_url,
            &config.supabase_anon_key,
            config.supabase_service_role_key.as_deref(),
        )))
    } else {
        tracing::warn!("Supabase not configured — running in standalone mode");
        None
    };

    // ── Optional Node RPC client ───────────────────────────────────────────
    let node_rpc = config.node_rpc_url.as_deref().map(|url| {
        tracing::info!("Node RPC configured: {url}");
        Arc::new(NodeRpcClient::new(url))
    });
    if node_rpc.is_none() {
        tracing::info!("Node RPC not configured — peer/chain endpoints will return 503");
    }

    // ── SSE event broadcaster ──────────────────────────────────────────────
    let broadcaster = Arc::new(EventBroadcaster::new(256));

    // Start node poller if RPC URL is configured
    if let Some(ref rpc) = node_rpc {
        let poller = NodePoller::new(rpc.clone(), broadcaster.clone(), Duration::from_secs(2));
        tokio::spawn(async move {
            poller.run().await;
        });
    }

    // ── Matching engine + trade outbox ─────────────────────────────────────
    let known_pairs = vec!["BEANS/USDC".into(), "BEANS/ETH".into()];
    let (engine, engine_tx, trade_rx) = MatchingEngine::new(known_pairs);
    let engine_handle = EngineHandle::new(engine_tx);
    tokio::spawn(engine.run());

    let outbox = TradeOutbox::new(trade_rx, supabase.clone(), broadcaster.clone());
    tokio::spawn(outbox.run());

    // ── Build state & router ────────────────────────────────────────────────
    let state = AppState {
        config: config.clone(),
        nonce_store,
        rate_limiter: create_rate_limiter(config.rate_limit_rps),
        metrics_handle,
        supabase,
        node_rpc,
        broadcaster,
        engine: Some(engine_handle),
    };
    let app = build_router(state);

    // ── Startup banner ──────────────────────────────────────────────────────
    let version = env!("CARGO_PKG_VERSION");
    println!(
        r#"
   ██████╗ ██████╗ ██╗███╗   ██╗     ██╗███████╗ ██████╗████████╗██╗   ██╗██████╗ ███████╗
  ██╔════╝██╔═══██╗██║████╗  ██║     ██║██╔════╝██╔════╝╚══██╔══╝██║   ██║██╔══██╗██╔════╝
  ██║     ██║   ██║██║██╔██╗ ██║     ██║█████╗  ██║        ██║   ██║   ██║██████╔╝█████╗
  ██║     ██║   ██║██║██║╚██╗██║██   ██║██╔══╝  ██║        ██║   ██║   ██║██╔══██╗██╔══╝
  ╚██████╗╚██████╔╝██║██║ ╚████║╚█████╔╝███████╗╚██████╗   ██║   ╚██████╔╝██║  ██║███████╗
   ╚═════╝ ╚═════╝ ╚═╝╚═╝  ╚═══╝ ╚════╝ ╚══════╝ ╚═════╝   ╚═╝    ╚═════╝ ╚═╝  ╚═╝╚══════╝

  COINjecture API v{version} | {network} | port {port}
"#,
        version = version,
        network = config.network,
        port = config.port,
    );

    // ── Serve ───────────────────────────────────────────────────────────────
    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("Failed to bind to {addr}: {e}"));

    tracing::info!("Listening on {addr}");
    axum::serve(listener, app).await.expect("server error");
}
