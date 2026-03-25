// =============================================================================
// COINjecture Load & Stress Testing Harness
// =============================================================================
//
// Usage:
//   load-test tx-flood --rpc http://127.0.0.1:9933 --tps 100 --duration 60
//   load-test mempool-flood --rpc http://127.0.0.1:9933 --count 10000
//   load-test rpc-blast --rpc http://127.0.0.1:9933 --concurrency 50 --duration 30
//   load-test stability --rpc http://127.0.0.1:9933 --duration 3600 --tps 10
//   load-test network-stress --target 127.0.0.1:707 --peers 100
//   load-test large-block --rpc http://127.0.0.1:9933
//   load-test recovery --rpc http://127.0.0.1:9933

use clap::{Parser, Subcommand};
use tracing_subscriber::filter::EnvFilter;

mod tx_generator;
mod mempool_flood;
mod rpc_load;
mod stability;
mod network_stress;
mod large_block;
mod monitor;
mod results;

use results::TestResults;

#[derive(Parser)]
#[command(name = "load-test", about = "COINjecture load and stress testing harness")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Output results as JSON
    #[arg(long, global = true)]
    json: bool,

    /// Output file for results (defaults to stdout)
    #[arg(long, global = true)]
    output: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Flood the node with transactions at configurable TPS
    TxFlood {
        /// RPC endpoint
        #[arg(long, default_value = "http://127.0.0.1:9933")]
        rpc: String,

        /// Target transactions per second
        #[arg(long, default_value_t = 50)]
        tps: u64,

        /// Test duration in seconds
        #[arg(long, default_value_t = 60)]
        duration: u64,

        /// Number of signing keys to cycle through
        #[arg(long, default_value_t = 10)]
        keys: usize,

        /// Tx amount per transfer (in base units)
        #[arg(long, default_value_t = 1)]
        amount: u64,
    },

    /// Flood the mempool beyond its capacity
    MempoolFlood {
        /// RPC endpoint
        #[arg(long, default_value = "http://127.0.0.1:9933")]
        rpc: String,

        /// Number of transactions to submit
        #[arg(long, default_value_t = 5000)]
        count: u64,

        /// Submission concurrency
        #[arg(long, default_value_t = 20)]
        concurrency: usize,
    },

    /// Blast all RPC endpoints concurrently
    RpcBlast {
        /// RPC endpoint
        #[arg(long, default_value = "http://127.0.0.1:9933")]
        rpc: String,

        /// Concurrent request count
        #[arg(long, default_value_t = 50)]
        concurrency: usize,

        /// Duration in seconds
        #[arg(long, default_value_t = 30)]
        duration: u64,
    },

    /// Long-running stability test under moderate load
    Stability {
        /// RPC endpoint
        #[arg(long, default_value = "http://127.0.0.1:9933")]
        rpc: String,

        /// Duration in seconds (default: 1 hour)
        #[arg(long, default_value_t = 3600)]
        duration: u64,

        /// TPS during stable phase
        #[arg(long, default_value_t = 10)]
        tps: u64,

        /// Memory sample interval in seconds
        #[arg(long, default_value_t = 60)]
        sample_interval: u64,
    },

    /// Connect many simulated peers to stress the network layer
    NetworkStress {
        /// P2P target address
        #[arg(long, default_value = "127.0.0.1:707")]
        target: String,

        /// Number of simulated peers
        #[arg(long, default_value_t = 50)]
        peers: usize,

        /// Duration in seconds
        #[arg(long, default_value_t = 30)]
        duration: u64,
    },

    /// Mine a block with the maximum number of transactions
    LargeBlock {
        /// RPC endpoint
        #[arg(long, default_value = "http://127.0.0.1:9933")]
        rpc: String,

        /// Number of transactions to pack
        #[arg(long, default_value_t = 1000)]
        tx_count: u64,
    },

    /// Test node recovery after a crash
    Recovery {
        /// RPC endpoint
        #[arg(long, default_value = "http://127.0.0.1:9933")]
        rpc: String,

        /// Seconds between crash and restart check
        #[arg(long, default_value_t = 5)]
        restart_wait: u64,

        /// Restart command (e.g., "systemctl start coinject-node")
        #[arg(long)]
        restart_cmd: Option<String>,
    },
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("load_test=info".parse().unwrap()))
        .init();

    let cli = Cli::parse();

    let results: TestResults = match cli.command {
        Commands::TxFlood { rpc, tps, duration, keys, amount } => {
            tx_generator::run_tx_flood(&rpc, tps, duration, keys, amount).await
        }
        Commands::MempoolFlood { rpc, count, concurrency } => {
            mempool_flood::run_mempool_flood(&rpc, count, concurrency).await
        }
        Commands::RpcBlast { rpc, concurrency, duration } => {
            rpc_load::run_rpc_blast(&rpc, concurrency, duration).await
        }
        Commands::Stability { rpc, duration, tps, sample_interval } => {
            stability::run_stability_test(&rpc, duration, tps, sample_interval).await
        }
        Commands::NetworkStress { target, peers, duration } => {
            network_stress::run_network_stress(&target, peers, duration).await
        }
        Commands::LargeBlock { rpc, tx_count } => {
            large_block::run_large_block_test(&rpc, tx_count).await
        }
        Commands::Recovery { rpc, restart_wait, restart_cmd } => {
            stability::run_recovery_test(&rpc, restart_wait, restart_cmd).await
        }
    };

    if cli.json {
        let json = serde_json::to_string_pretty(&results).expect("results serialization failed");
        if let Some(path) = cli.output {
            std::fs::write(&path, &json).expect("failed to write output file");
            tracing::info!("results written to {}", path);
        } else {
            println!("{}", json);
        }
    } else {
        results.print_summary();
        if let Some(path) = cli.output {
            let json = serde_json::to_string_pretty(&results).expect("results serialization failed");
            std::fs::write(&path, json).expect("failed to write output file");
            tracing::info!("results written to {}", path);
        }
    }

    if results.passed {
        std::process::exit(0);
    } else {
        std::process::exit(1);
    }
}
