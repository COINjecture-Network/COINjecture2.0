// COINjecture Node
// Network B - NP-hard Consensus Blockchain
#![recursion_limit = "512"]
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
#[cfg(feature = "adzdb")]
mod chain_adzdb;
mod sync_canonical;
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
mod sync_optimizer;
mod validator;

use config::NodeConfig;
use service::CoinjectNode;
use tokio::signal;
use tracing_subscriber::EnvFilter;

/// Install a panic hook that logs the panic location and backtrace via `tracing`
/// before allowing the default behaviour (abort in release, unwind in debug).
/// This ensures panics are always visible in structured logs rather than only
/// on stderr, and gives operators a chance to correlate crashes with metrics.
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let location = info.location().map_or_else(
            || "unknown location".to_string(),
            |l| format!("{}:{}:{}", l.file(), l.line(), l.column()),
        );

        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "<non-string panic payload>".to_string()
        };

        // Log at ERROR level so the panic is captured by any tracing subscriber
        // (file sink, Loki, etc.) before the process exits.
        tracing::error!(
            target: "coinject::panic",
            location = %location,
            message = %payload,
            "NODE PANIC — initiating graceful shutdown"
        );

        // Flush logs before exiting — best-effort, ignore flush errors.
        // The default panic handler will print to stderr and then abort/unwind.
    }));
}

// Multi-threaded runtime for CPP protocol TCP connections
// Worker threads handle concurrent peer I/O and mining tasks
#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Install panic hook first so any subsequent panic is logged.
    install_panic_hook();

    // Initialize logging
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    // Parse configuration first (needed for banner)
    let config = NodeConfig::parse_args();

    // Display banner with node type info
    print_banner(&config);

    // Log active network mode
    tracing::info!("Network: CPP protocol on {}", config.cpp_p2p_addr);

    // Initialize Prometheus metrics
    metrics::init();

    // Start metrics server
    let metrics_addr = config.metrics_socket_addr()?;
    tokio::spawn(async move {
        if let Err(e) = metrics_server::start_metrics_server(metrics_addr).await {
            tracing::error!("Metrics server error: {}", e);
        }
    });

    // Create and start node
    let mut node = CoinjectNode::new(config).await?;
    node.start().await?;

    // Wait for shutdown signal (SIGINT / Ctrl-C, or SIGTERM from the OS / container runtime)
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm =
            signal(SignalKind::terminate()).expect("failed to register SIGTERM handler");

        tokio::select! {
            result = signal::ctrl_c() => {
                match result {
                    Ok(()) => {
                        println!();
                        tracing::info!("Received SIGINT (Ctrl-C) — shutting down gracefully");
                    }
                    Err(err) => {
                        tracing::error!("Unable to listen for SIGINT: {}", err);
                    }
                }
            }
            _ = sigterm.recv() => {
                tracing::info!("Received SIGTERM — shutting down gracefully");
            }
        }
        node.shutdown();
    }

    #[cfg(not(unix))]
    match signal::ctrl_c().await {
        Ok(()) => {
            println!();
            tracing::info!("Received shutdown signal (Ctrl-C)");
            node.shutdown();
        }
        Err(err) => {
            tracing::error!("Unable to listen for shutdown signal: {}", err);
            node.shutdown();
        }
    }

    // Wait for graceful shutdown
    node.wait_for_shutdown().await;

    println!("👋 COINjecture Node stopped");
    println!();

    Ok(())
}

fn print_banner(config: &NodeConfig) {
    println!(
        r#"
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
    "#
    );
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
    println!(
        "    │ {} Node Type: {:<10} │ Reward Multiplier: {:.3}x       │",
        icon,
        mode_name,
        target_type.reward_multiplier()
    );
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
