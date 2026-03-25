// COINjecture Node
// Network B - NP-hard Consensus Blockchain
//
// Supports 6 Specialized Node Types with Dynamic Behavioral Classification:
// - Light: Header-only sync, minimal storage (mobile-friendly)
// - Full: Complete validation, standard storage (default)
// - Archive: Complete history, 2TB+ storage
// - Validator: Block production, high validation speed
// - Bounty: NP-problem solving focused
// - Oracle: External data feeds
//
// CRITICAL: Nodes are classified EMPIRICALLY based on behavior, NOT self-declaration

mod chain;
mod sync_optimizer;
#[cfg(feature = "adzdb")]
mod chain_adzdb;
mod config;
mod faucet;
mod genesis;
mod keystore;
mod light_client;
mod light_sync;
mod metrics;
mod metrics_integration;
mod metrics_server;
pub mod mobile_sdk;
pub mod node_manager;
pub mod node_types;
mod peer_consensus;
mod service;
mod validator;

use config::NodeConfig;
use service::CoinjectNode;
use std::time::Instant;
use tokio::signal;
use tracing::{error, info};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialise tracing.  Returns a guard that must be held for the process lifetime
/// (dropping it flushes the non-blocking file writer).
///
/// Format is controlled by the `LOG_FORMAT` environment variable:
///   LOG_FORMAT=json   → newline-delimited JSON (production default)
///   LOG_FORMAT=pretty → human-readable pretty-print (default)
///
/// Log file output is controlled by `LOG_DIR` (optional, daily rotation):
///   LOG_DIR=/var/log/coinject
///
/// Log level is controlled by `RUST_LOG` (default: info).
fn init_logging() -> Option<WorkerGuard> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let log_format = std::env::var("LOG_FORMAT")
        .unwrap_or_else(|_| "pretty".to_string());

    let use_json = log_format == "json";

    match std::env::var("LOG_DIR") {
        Ok(log_dir) => {
            // File + console, both in the same format
            let file_appender = tracing_appender::rolling::daily(log_dir, "coinject.log");
            let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
            if use_json {
                tracing_subscriber::registry()
                    .with(filter)
                    .with(fmt::layer().json())
                    .with(fmt::layer().json().with_writer(non_blocking))
                    .init();
            } else {
                tracing_subscriber::registry()
                    .with(filter)
                    .with(fmt::layer().with_target(false))
                    .with(fmt::layer().with_writer(non_blocking))
                    .init();
            }
            Some(guard)
        }
        Err(_) => {
            // Console only
            if use_json {
                tracing_subscriber::registry()
                    .with(filter)
                    .with(fmt::layer().json())
                    .init();
            } else {
                tracing_subscriber::registry()
                    .with(filter)
                    .with(fmt::layer().with_target(false))
                    .init();
            }
            None
        }
    }
}

// Multi-threaded runtime for CPP protocol TCP connections
// Worker threads handle concurrent peer I/O and mining tasks
#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialise logging (guard must live for the whole process)
    let _log_guard = init_logging();

    // Parse configuration
    let config = NodeConfig::parse_args();

    // Display terminal banner (intentional stdout — not a log event)
    print_banner(&config);

    // Startup: log sanitized config (no secrets)
    let node_start = Instant::now();
    info!(
        version = env!("CARGO_PKG_VERSION"),
        node_type = %config.node_type,
        chain_id = %config.chain_id,
        rpc_addr = %config.rpc_addr,
        cpp_p2p_addr = %config.cpp_p2p_addr,
        cpp_ws_addr = %config.cpp_ws_addr,
        metrics_addr = %config.metrics_addr,
        data_dir = %config.data_dir.display(),
        mining = config.mine,
        dev_mode = config.dev,
        bootnode_count = config.bootnodes.len(),
        max_peers = config.max_peers,
        difficulty = config.difficulty,
        block_time_s = config.block_time,
        hf_sync = config.hf_dataset_name.is_some(),
        "node starting"
    );

    // Initialize Prometheus metrics
    metrics::init();

    // Start metrics server
    let metrics_addr = config.metrics_socket_addr()?;
    tokio::spawn(async move {
        if let Err(e) = metrics_server::start_metrics_server(metrics_addr).await {
            error!(error = %e, "metrics server failed");
        }
    });

    // Create and start node
    let mut node = CoinjectNode::new(config).await?;
    node.start().await?;

    // Wait for shutdown signal (Ctrl+C or SIGTERM)
    match signal::ctrl_c().await {
        Ok(()) => {
            info!("shutdown signal received");
            node.shutdown();
        }
        Err(err) => {
            error!(error = %err, "failed to listen for shutdown signal");
        }
    }

    // Wait for graceful shutdown
    node.wait_for_shutdown().await;

    info!(
        uptime_s = node_start.elapsed().as_secs(),
        "node stopped"
    );

    Ok(())
}

fn print_banner(config: &NodeConfig) {
    println!(r#"
    ╔═══════════════════════════════════════════════════════════════╗
    ║                                                               ║
    ║         ██████╗ ██████╗ ██╗███╗   ██╗     ██╗███████╗ ██████╗████████╗██╗   ██╗██████╗ ███████╗    ║
    ║        ██╔════╝██╔═══██╗██║████╗  ██║     ██║██╔════╝██╔════╝╚══██╔══╝██║   ██║██╔══██╗██╔════╝    ║
    ║        ██║     ██║   ██║██║██╔██╗ ██║     ██║█████╗  ██║        ██║   ██║   ██║██████╔╝█████╗      ║
    ║        ██║     ██║   ██║██║██║╚██╗██║██   ██║██╔══╝  ██║        ██║   ██║   ██║██╔══██╗██╔══╝      ║
    ║        ╚██████╗╚██████╔╝██║██║ ╚████║╚█████╔╝███████╗╚██████╗   ██║   ╚██████╔╝██║  ██║███████╗    ║
    ║         ╚═════╝ ╚═════╝ ╚═╝╚═╝  ╚═══╝ ╚════╝ ╚══════╝ ╚═════╝   ╚═╝    ╚═════╝ ╚═╝  ╚═╝╚══════╝    ║
    ║                                                               ║
    ║                    Network B - NP-Hard Consensus              ║
    ║                    η = 1/√2 Tokenomics Engine                ║
    ║                                                               ║
    ╚═══════════════════════════════════════════════════════════════╝
    "#);
    println!("    Version: {}", env!("CARGO_PKG_VERSION"));
    println!("    Repository: {}", env!("CARGO_PKG_REPOSITORY"));
    println!();
    
    // Display node type information
    let target_type = config.target_node_type();
    let (icon, mode_name) = match target_type {
        node_types::NodeType::Light => ("📱", "LIGHT"),
        node_types::NodeType::Full => ("💻", "FULL"),
        node_types::NodeType::Archive => ("🗄️", "ARCHIVE"),
        node_types::NodeType::Validator => ("⚡", "VALIDATOR"),
        node_types::NodeType::Bounty => ("🎯", "BOUNTY"),
        node_types::NodeType::Oracle => ("🔮", "ORACLE"),
    };
    
    println!("    ┌─────────────────────────────────────────────────────────────┐");
    println!("    │ {} Node Type: {:<10} │ Reward Multiplier: {:.3}x       │", 
             icon, mode_name, target_type.reward_multiplier());
    println!("    │ {} │", target_type.description());
    println!("    │                                                             │");
    println!("    │ ℹ️  Actual classification determined by BEHAVIOR, not config │");
    println!("    │    (storage ratio, validation speed, solve rate, uptime)   │");
    println!("    └─────────────────────────────────────────────────────────────┘");
    println!();
    
    // Display hardware requirements
    let hw = target_type.hardware_requirements();
    println!("    Hardware Requirements for {} node:", mode_name);
    println!("    • RAM: {} GB minimum", hw.min_ram_gb);
    println!("    • Storage: {} GB minimum", hw.min_storage_gb);
    println!("    • Bandwidth: {} Mbps minimum", hw.min_bandwidth_mbps);
    println!("    • CPU Cores: {} minimum", hw.min_cpu_cores);
    println!();
    
    // Display stake requirement
    let stake = target_type.min_stake();
    if stake > 0 {
        println!("    💰 Minimum Stake: {} tokens", stake / 1_000_000);
    } else {
        println!("    💰 No stake required for Light nodes");
    }
    println!();
}
