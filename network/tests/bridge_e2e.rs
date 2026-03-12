// =============================================================================
// Bridge End-to-End Smoke Test
// =============================================================================
//
// Proves: Block created on Node A → mesh gossip → Node B bridge → deserialized
//         correctly as BridgeEvent::BlockReceived with matching data.
//
// This tests the full mesh + bridge pipeline without requiring the node crate's
// validator or chain storage (which live in the binary-only node crate).
//
// Run: cargo test -p coinject-network --test bridge_e2e -- --nocapture

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, RwLock};
use tokio::time::timeout;

use coinject_core::{
    Address, Block, BlockHeader, CoinbaseTransaction, Commitment, Ed25519Signature,
    Hash, ProblemType, PublicKey, Solution, SolutionReveal,
};
use coinject_network::mesh::bridge::{self, BridgeCommand, BridgeEvent, BridgeState};
use coinject_network::mesh::config::NetworkConfig;
use coinject_network::mesh::{NetworkEvent, NetworkService};

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Create a mesh config on a fixed port with optional seeds.
fn test_config(port: u16, seeds: Vec<std::net::SocketAddr>) -> NetworkConfig {
    NetworkConfig {
        listen_addr: format!("127.0.0.1:{}", port).parse().unwrap(),
        seed_nodes: seeds,
        data_dir: PathBuf::from(format!(
            "{}/coinject-bridge-e2e-{}",
            std::env::temp_dir().display(),
            port
        )),
        heartbeat_interval: Duration::from_secs(3),
        max_missed_heartbeats: 3,
        reconnect_base_delay: Duration::from_millis(200),
        reconnect_max_delay: Duration::from_secs(2),
        default_ttl: 10,
        dedup_cache_capacity: 10_000,
        dedup_cache_ttl: Duration::from_secs(60),
        max_message_size: 16 * 1024 * 1024,
        max_messages_per_second_per_peer: 1000,
        peer_exchange_interval: Duration::from_secs(5),
        handshake_timeout: Duration::from_secs(5),
        connect_timeout: Duration::from_secs(3),
    }
}

/// Create a deterministic genesis block (same as node crate's create_genesis_block).
fn create_genesis_block() -> Block {
    let genesis_address = Address::from_bytes([0x01; 32]);

    let problem = ProblemType::SubsetSum {
        numbers: vec![1, 2, 3, 4, 5],
        target: 9,
    };
    let solution = Solution::SubsetSum(vec![1, 2, 3]); // indices: 2+3+4 = 9
    let epoch_salt = Hash::new(b"coinject-genesis-epoch");
    let commitment = Commitment::create(&problem, &solution, &epoch_salt);

    let header = BlockHeader {
        version: 1,
        height: 0,
        prev_hash: Hash::ZERO,
        timestamp: 1735689600,
        transactions_root: Hash::ZERO,
        solutions_root: Hash::new(&bincode::serialize(&solution).unwrap_or_default()),
        commitment: commitment.clone(),
        work_score: 1.0,
        miner: genesis_address,
        nonce: 0,
        solve_time_us: 1,
        verify_time_us: 1,
        time_asymmetry_ratio: 1.0,
        solution_quality: 1.0,
        complexity_weight: 1.0,
        energy_estimate_joules: 0.001,
    };

    let coinbase = CoinbaseTransaction::new(genesis_address, 0, 0);
    let solution_reveal = SolutionReveal {
        problem,
        solution,
        commitment,
    };

    Block {
        header,
        coinbase,
        transactions: vec![],
        solution_reveal,
    }
}

/// Create a test block at height 1, building on the genesis block.
fn create_test_block(genesis: &Block) -> Block {
    let miner = Address::from_bytes([0x42; 32]);
    let genesis_hash = genesis.header.hash();

    // Use genesis hash as epoch_salt (mirrors real validation: epoch_salt = prev_hash)
    let problem = ProblemType::SubsetSum {
        numbers: vec![10, 20, 30, 40, 50],
        target: 60,
    };
    let solution = Solution::SubsetSum(vec![0, 1, 2]); // 10+20+30 = 60
    let commitment = Commitment::create(&problem, &solution, &genesis_hash);

    let header = BlockHeader {
        version: 1,
        height: 1,
        prev_hash: genesis_hash,
        timestamp: 1735689660, // 60 seconds after genesis
        transactions_root: Hash::ZERO, // No transactions
        solutions_root: Hash::new(&bincode::serialize(&solution).unwrap_or_default()),
        commitment: commitment.clone(),
        work_score: 1.0,
        miner,
        nonce: 0,
        solve_time_us: 1000,
        verify_time_us: 10,
        time_asymmetry_ratio: 100.0,
        solution_quality: 0.95,
        complexity_weight: 2.0,
        energy_estimate_joules: 0.5,
    };

    let coinbase = CoinbaseTransaction::new(miner, 1000, 1);
    let solution_reveal = SolutionReveal {
        problem,
        solution,
        commitment,
    };

    Block {
        header,
        coinbase,
        transactions: vec![],
        solution_reveal,
    }
}

/// Wait for PeerConnected event on a mesh event receiver.
async fn wait_for_mesh_peer(
    rx: &mut mpsc::UnboundedReceiver<NetworkEvent>,
    timeout_dur: Duration,
) -> bool {
    let deadline = tokio::time::Instant::now() + timeout_dur;
    loop {
        let remaining = deadline - tokio::time::Instant::now();
        match timeout(remaining, rx.recv()).await {
            Ok(Some(NetworkEvent::PeerConnected(_))) => return true,
            Ok(Some(_)) => continue,
            _ => return false,
        }
    }
}

/// Wait for a BridgeEvent::BlockReceived on the bridge event channel.
async fn wait_for_bridge_block(
    rx: &mut mpsc::UnboundedReceiver<BridgeEvent>,
    timeout_dur: Duration,
) -> Option<(Block, coinject_network::mesh::identity::NodeId)> {
    let deadline = tokio::time::Instant::now() + timeout_dur;
    loop {
        let remaining = deadline - tokio::time::Instant::now();
        match timeout(remaining, rx.recv()).await {
            Ok(Some(BridgeEvent::BlockReceived { block, peer_id })) => {
                return Some((block, peer_id));
            }
            Ok(Some(other)) => {
                tracing::debug!("bridge event (not block): {:?}", other);
                continue;
            }
            Ok(None) => return None,
            Err(_) => return None, // Timeout
        }
    }
}

// ─── Main E2E Test ──────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_block_propagates_through_mesh_bridge() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                tracing_subscriber::EnvFilter::new(
                    "bridge_e2e=debug,coinject_network::mesh=info",
                )
            }),
        )
        .try_init();

    let test_start = Instant::now();

    // ── Step 1: Create genesis block ────────────────────────────────────
    let genesis = create_genesis_block();
    let genesis_hash = genesis.header.hash();
    println!();
    println!("  ╔═══════════════════════════════════════════════╗");
    println!("  ║   Mesh Bridge E2E — Block Propagation Test    ║");
    println!("  ╚═══════════════════════════════════════════════╝");
    println!();
    println!("  Genesis hash: {}", genesis_hash);

    // ── Step 2: Start mesh nodes ────────────────────────────────────────
    let port_a = 19400 + (rand::random::<u16>() % 500);
    let port_b = port_a + 1;
    let seed_a: std::net::SocketAddr = format!("127.0.0.1:{}", port_a).parse().unwrap();

    let cfg_a = test_config(port_a, vec![]);
    let cfg_b = test_config(port_b, vec![seed_a]);

    let (svc_a, mesh_rx_a) = NetworkService::start(cfg_a).await.unwrap();
    let (svc_b, mesh_rx_b) = NetworkService::start(cfg_b).await.unwrap();

    let id_a = *svc_a.local_id();
    let id_b = *svc_b.local_id();

    println!("  Node A: {} (seed, port {})", id_a.short(), port_a);
    println!("  Node B: {} (port {})", id_b.short(), port_b);

    // ── Step 3: Create bridge for Node A (sending side) ────────────────
    let (bridge_cmd_tx_a, bridge_cmd_rx_a) = mpsc::unbounded_channel::<BridgeCommand>();
    let (bridge_event_tx_a, _bridge_event_rx_a) = mpsc::unbounded_channel::<BridgeEvent>();
    let mesh_cmd_tx_a = svc_a.command_sender();

    let bridge_state_a = Arc::new(RwLock::new(BridgeState {
        best_height: 0,
        best_hash: genesis_hash,
        epoch: 0,
    }));

    tokio::spawn(bridge::run_bridge(
        bridge_cmd_rx_a,
        bridge_event_tx_a,
        mesh_cmd_tx_a,
        mesh_rx_a, // Node A's mesh events feed into its bridge (not used much here)
        bridge_state_a,
    ));

    // ── Step 4: Create bridge for Node B (receiving side) ──────────────
    let (bridge_cmd_tx_b, bridge_cmd_rx_b) = mpsc::unbounded_channel::<BridgeCommand>();
    let (bridge_event_tx_b, mut bridge_event_rx_b) = mpsc::unbounded_channel::<BridgeEvent>();
    let mesh_cmd_tx_b = svc_b.command_sender();

    let bridge_state_b = Arc::new(RwLock::new(BridgeState {
        best_height: 0,
        best_hash: genesis_hash,
        epoch: 0,
    }));

    tokio::spawn(bridge::run_bridge(
        bridge_cmd_rx_b,
        bridge_event_tx_b,
        mesh_cmd_tx_b,
        mesh_rx_b,
        bridge_state_b,
    ));

    // ── Step 5: Wait for mesh connection ────────────────────────────────
    // Since we gave mesh_rx_a to the bridge, we need to wait differently.
    // The bridge emits PeerConnected events. Wait for Node B's bridge to
    // see a peer, which means the mesh formed.
    println!();
    println!("  Waiting for mesh connection...");

    let connected = wait_for_bridge_peer(&mut bridge_event_rx_b, Duration::from_secs(10)).await;
    assert!(connected, "FAIL: Nodes failed to discover each other within 10s");

    // Small delay to let Node A's mesh also register the inbound connection
    // (Node B connects outbound to A, but A's inbound registration is async)
    tokio::time::sleep(Duration::from_millis(500)).await;

    let mesh_time = test_start.elapsed();
    println!("  Mesh connected: A ↔ B ({:.1}s)", mesh_time.as_secs_f64());

    // ── Step 6: Create and broadcast test block ────────────────────────
    let test_block = create_test_block(&genesis);
    let test_block_hash = test_block.header.hash();
    let test_block_height = test_block.header.height;

    println!();
    println!("  Node A: created test block height={} hash={}", test_block_height, test_block_hash);
    println!("  Node A: broadcasting block via mesh bridge...");

    let broadcast_start = Instant::now();

    bridge_cmd_tx_a
        .send(BridgeCommand::BroadcastBlock {
            block: test_block.clone(),
        })
        .expect("Failed to send BroadcastBlock command");

    // ── Step 7: Wait for Node B's bridge to receive the block ──────────
    let result = wait_for_bridge_block(&mut bridge_event_rx_b, Duration::from_secs(10)).await;

    let propagation_time = broadcast_start.elapsed();

    // ── Step 8: Verify ─────────────────────────────────────────────────
    println!();
    match result {
        Some((received_block, from_peer)) => {
            let received_hash = received_block.header.hash();
            let received_height = received_block.header.height;

            println!("  Node B: block received from mesh (peer {})", from_peer.short());
            println!("  Node B: height={} hash={}", received_height, received_hash);

            // Check hash match
            let hash_match = received_hash == test_block_hash;
            // Check height match
            let height_match = received_height == test_block_height;
            // Check prev_hash match
            let prev_hash_match = received_block.header.prev_hash == test_block.header.prev_hash;
            // Check miner match
            let miner_match = received_block.header.miner == test_block.header.miner;
            // Check coinbase match
            let coinbase_match = received_block.coinbase.reward == test_block.coinbase.reward
                && received_block.coinbase.height == test_block.coinbase.height;
            // Check solution verifies
            let solution_valid = received_block
                .solution_reveal
                .solution
                .verify(&received_block.solution_reveal.problem);

            println!();
            println!("  ─────────────────────────────────────────────────");

            if hash_match && height_match && prev_hash_match && miner_match && coinbase_match && solution_valid {
                println!(
                    "  ✓ PASS — Block propagated A→B in {}ms",
                    propagation_time.as_millis()
                );
                println!("    Node A: {} → Block {} (height {})", id_a.short(), test_block_hash, test_block_height);
                println!("    Node B: {} → Received {} (height {})", id_b.short(), received_hash, received_height);
                println!("    Hashes match: ✓");
                println!("    Solution valid: ✓");
                println!("    Prev hash correct: ✓");
                println!("    Coinbase correct: ✓");
            } else {
                println!("  ✗ FAIL — Block data mismatch");
                if !hash_match {
                    println!("    Hash: EXPECTED {} GOT {}", test_block_hash, received_hash);
                }
                if !height_match {
                    println!("    Height: EXPECTED {} GOT {}", test_block_height, received_height);
                }
                if !prev_hash_match {
                    println!("    Prev hash mismatch");
                }
                if !miner_match {
                    println!("    Miner mismatch");
                }
                if !coinbase_match {
                    println!("    Coinbase mismatch");
                }
                if !solution_valid {
                    println!("    Solution INVALID");
                }
                panic!("Block data mismatch — see details above");
            }
            println!("  ─────────────────────────────────────────────────");
            println!();
        }
        None => {
            println!("  ─────────────────────────────────────────────────");
            println!("  ✗ FAIL — Node B did not receive block within 10s");
            println!("  ─────────────────────────────────────────────────");
            println!();
            panic!("Block did not propagate from A to B");
        }
    }

    // ── Cleanup ────────────────────────────────────────────────────────
    drop(bridge_cmd_tx_a);
    drop(bridge_cmd_tx_b);
    svc_a.shutdown().await.unwrap();
    svc_b.shutdown().await.unwrap();

    let total_time = test_start.elapsed();
    println!("  Total test time: {:.1}s", total_time.as_secs_f64());
    println!();
}

/// Wait for a BridgeEvent::PeerConnected.
async fn wait_for_bridge_peer(
    rx: &mut mpsc::UnboundedReceiver<BridgeEvent>,
    timeout_dur: Duration,
) -> bool {
    let deadline = tokio::time::Instant::now() + timeout_dur;
    loop {
        let remaining = deadline - tokio::time::Instant::now();
        match timeout(remaining, rx.recv()).await {
            Ok(Some(BridgeEvent::PeerConnected { peer_id, .. })) => {
                tracing::info!("bridge: peer connected {}", peer_id.short());
                return true;
            }
            Ok(Some(other)) => {
                tracing::debug!("bridge event while waiting for peer: {:?}", other);
                continue;
            }
            Ok(None) => return false,
            Err(_) => return false,
        }
    }
}

// ─── Transaction Propagation Test ───────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_transaction_propagates_through_mesh_bridge() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("info")
        .try_init();

    let port_a = 19500 + (rand::random::<u16>() % 500);
    let port_b = port_a + 1;
    let seed_a: std::net::SocketAddr = format!("127.0.0.1:{}", port_a).parse().unwrap();

    let cfg_a = test_config(port_a, vec![]);
    let cfg_b = test_config(port_b, vec![seed_a]);

    let (svc_a, mesh_rx_a) = NetworkService::start(cfg_a).await.unwrap();
    let (svc_b, mesh_rx_b) = NetworkService::start(cfg_b).await.unwrap();

    let genesis = create_genesis_block();
    let genesis_hash = genesis.header.hash();

    // Bridge A
    let (bridge_cmd_tx_a, bridge_cmd_rx_a) = mpsc::unbounded_channel();
    let (bridge_event_tx_a, _) = mpsc::unbounded_channel();
    let bridge_state_a = Arc::new(RwLock::new(BridgeState {
        best_height: 0,
        best_hash: genesis_hash,
        epoch: 0,
    }));
    tokio::spawn(bridge::run_bridge(
        bridge_cmd_rx_a,
        bridge_event_tx_a,
        svc_a.command_sender(),
        mesh_rx_a,
        bridge_state_a,
    ));

    // Bridge B
    let (_bridge_cmd_tx_b, bridge_cmd_rx_b) = mpsc::unbounded_channel();
    let (bridge_event_tx_b, mut bridge_event_rx_b) = mpsc::unbounded_channel();
    let bridge_state_b = Arc::new(RwLock::new(BridgeState {
        best_height: 0,
        best_hash: genesis_hash,
        epoch: 0,
    }));
    tokio::spawn(bridge::run_bridge(
        bridge_cmd_rx_b,
        bridge_event_tx_b,
        svc_b.command_sender(),
        mesh_rx_b,
        bridge_state_b,
    ));

    // Wait for mesh connection
    assert!(
        wait_for_bridge_peer(&mut bridge_event_rx_b, Duration::from_secs(10)).await,
        "Nodes failed to connect"
    );
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Create a dummy transaction (TransferTransaction)
    use coinject_core::Transaction;
    let test_tx = Transaction::Transfer(coinject_core::TransferTransaction {
        from: Address::from_bytes([0xAA; 32]),
        to: Address::from_bytes([0xBB; 32]),
        amount: 500,
        fee: 1,
        nonce: 1,
        public_key: PublicKey::from_bytes([0xCC; 32]),
        signature: Ed25519Signature::from_bytes([0xDD; 64]),
    });
    let tx_hash = test_tx.hash();

    // Broadcast via bridge
    bridge_cmd_tx_a
        .send(BridgeCommand::BroadcastTransaction {
            transaction: test_tx.clone(),
        })
        .unwrap();

    // Wait for transaction on Node B
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let mut received_tx = false;
    loop {
        let remaining = deadline - tokio::time::Instant::now();
        match timeout(remaining, bridge_event_rx_b.recv()).await {
            Ok(Some(BridgeEvent::TransactionReceived { transaction, .. })) => {
                assert_eq!(transaction.hash(), tx_hash, "Transaction hash mismatch");
                received_tx = true;
                break;
            }
            Ok(Some(_)) => continue,
            _ => break,
        }
    }

    assert!(received_tx, "FAIL: Transaction did not propagate A→B");
    println!("  ✓ Transaction propagated A→B (hash {})", tx_hash);

    drop(bridge_cmd_tx_a);
    drop(_bridge_cmd_tx_b);
    svc_a.shutdown().await.unwrap();
    svc_b.shutdown().await.unwrap();
}
