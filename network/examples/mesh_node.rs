// =============================================================================
// Mesh Network Node — Standalone Demo
// =============================================================================
//
// Run multiple instances to form a P2P mesh:
//
//   # Terminal 1 — seed node
//   RUST_LOG=info cargo run --example mesh_node -p coinject-network -- --listen 0.0.0.0:9000
//
//   # Terminal 2
//   RUST_LOG=info cargo run --example mesh_node -p coinject-network -- --listen 0.0.0.0:9001 --seed 127.0.0.1:9000
//
//   # Terminal 3
//   RUST_LOG=info cargo run --example mesh_node -p coinject-network -- --listen 0.0.0.0:9002 --seed 127.0.0.1:9000

use coinject_network::mesh::config::NetworkConfig;
use coinject_network::mesh::protocol::Payload;
use coinject_network::mesh::{NetworkCommand, NetworkEvent, NetworkService};
use std::net::SocketAddr;
use std::path::PathBuf;
use tokio::signal;

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    // Parse CLI args
    let args: Vec<String> = std::env::args().collect();
    let mut listen_addr: SocketAddr = "0.0.0.0:9000".parse()?;
    let mut seeds: Vec<SocketAddr> = Vec::new();
    let mut data_dir = PathBuf::from("./data/mesh");

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--listen" => {
                i += 1;
                listen_addr = args[i].parse()?;
            }
            "--seed" => {
                i += 1;
                seeds.push(args[i].parse()?);
            }
            "--data-dir" => {
                i += 1;
                data_dir = PathBuf::from(&args[i]);
            }
            _ => {
                eprintln!("Unknown arg: {}", args[i]);
                eprintln!("Usage: mesh_node --listen ADDR [--seed ADDR]... [--data-dir PATH]");
                std::process::exit(1);
            }
        }
        i += 1;
    }

    let port = listen_addr.port();
    let data_dir = data_dir.join(format!("node-{}", port));

    let config = NetworkConfig {
        listen_addr,
        seed_nodes: seeds.clone(),
        data_dir,
        ..Default::default()
    };

    println!();
    println!("  ╔═══════════════════════════════════════════════╗");
    println!("  ║         COINjecture Mesh Network Node         ║");
    println!("  ╚═══════════════════════════════════════════════╝");
    println!();

    let (service, mut event_rx) = NetworkService::start(config).await?;
    let node_id = *service.local_id();

    println!("  Node ID:  {}", node_id);
    println!("  Listen:   {}", listen_addr);
    if seeds.is_empty() {
        println!("  Seeds:    (none — this is a seed node)");
    } else {
        for s in &seeds {
            println!("  Seed:     {}", s);
        }
    }
    println!();
    println!("  Commands: b(roadcast), p(eers), q(uit)");
    println!();

    // Get a command sender for the stdin task
    let cmd_tx = service.command_sender();

    // Spawn event handler
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                NetworkEvent::PeerConnected(peer) => {
                    tracing::info!("PEER CONNECTED: {}", peer.short());
                }
                NetworkEvent::PeerDisconnected(peer) => {
                    tracing::info!("PEER DISCONNECTED: {}", peer.short());
                }
                NetworkEvent::MessageReceived { from, payload, .. } => {
                    tracing::info!("MESSAGE from {}: {}", from.short(), payload);
                }
                NetworkEvent::PeerList(peers) => {
                    tracing::info!("PEERS: {} total", peers.len());
                    for (id, addr, state) in peers {
                        tracing::info!("  {} @ {} [{}]", id.short(), addr, state);
                    }
                }
            }
        }
    });

    // Spawn stdin reader for interactive commands
    tokio::spawn(async move {
        let stdin = std::io::stdin();
        let mut line = String::new();
        loop {
            line.clear();
            if stdin.read_line(&mut line).is_err() {
                break;
            }
            let trimmed = line.trim();
            match trimmed {
                "b" | "broadcast" => {
                    let payload = Payload::ConsensusSalt {
                        epoch: 0,
                        salt: {
                            let mut s = [0u8; 32];
                            rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut s);
                            s
                        },
                    };
                    tracing::info!("Broadcasting test ConsensusSalt");
                    let _ = cmd_tx.send(NetworkCommand::Broadcast(payload));
                }
                "p" | "peers" => {
                    let _ = cmd_tx.send(NetworkCommand::GetPeers);
                }
                "q" | "quit" => {
                    let _ = cmd_tx.send(NetworkCommand::Shutdown);
                    break;
                }
                _ => {
                    if !trimmed.is_empty() {
                        println!("Commands: b(roadcast), p(eers), q(uit)");
                    }
                }
            }
        }
    });

    // Wait for Ctrl+C
    signal::ctrl_c().await?;
    println!();
    tracing::info!("Shutting down...");
    service.shutdown().await?;
    tracing::info!("Goodbye!");

    Ok(())
}
