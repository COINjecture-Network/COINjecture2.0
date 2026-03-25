// =============================================================================
// COINjecture Custom P2P — Testnet Integration Tests
// =============================================================================
// Phase 3: Real multi-node integration tests with actual TCP connections
//
// Test 3.1 — Two Node Connect and Sync
// Test 3.2 — Block Propagation
// Test 3.3 — Peer Reconnection
// Test 3.4 — Router Fanout Math (η = 1/√2)

use coinject_core::{
    Address, Block, BlockHeader, CoinbaseTransaction, Commitment, Hash, SolutionReveal,
};
use coinject_network::cpp::{
    config::{NodeType, ETA},
    router::{EquilibriumRouter, PeerInfo},
    CppConfig, CppNetwork, NetworkCommand, NetworkEvent,
};
use std::net::SocketAddr;
use tokio::time::Duration;

// =============================================================================
// Test Helpers
// =============================================================================

/// Create a CppConfig for a test node on a given port
fn test_config(port: u16) -> CppConfig {
    CppConfig {
        p2p_listen: format!("127.0.0.1:{}", port),
        ws_listen: "127.0.0.1:0".to_string(),
        bootnodes: vec![],
        max_peers: 10,
        enable_websocket: false,
        node_type: NodeType::Full,
        ..CppConfig::default()
    }
}

/// Create a test block at the given height
fn create_test_block(height: u64, prev_hash: Hash) -> Block {
    let header = BlockHeader {
        version: 1,
        height,
        prev_hash,
        timestamp: (height * 600) as i64,
        transactions_root: Hash::ZERO,
        solutions_root: Hash::ZERO,
        commitment: Commitment {
            hash: Hash::ZERO,
            problem_hash: Hash::ZERO,
        },
        work_score: 100.0,
        miner: Address::from_bytes([0u8; 32]),
        nonce: height,
        solve_time_us: 0,
        verify_time_us: 0,
        time_asymmetry_ratio: 0.0,
        solution_quality: 0.0,
        complexity_weight: 0.0,
        energy_estimate_joules: 0.0,
    };

    Block {
        header: header.clone(),
        coinbase: CoinbaseTransaction::new(Address::from_bytes([0u8; 32]), 0, height),
        transactions: vec![],
        solution_reveal: SolutionReveal {
            commitment: Commitment {
                hash: Hash::ZERO,
                problem_hash: Hash::ZERO,
            },
            problem: coinject_core::ProblemType::Custom {
                problem_id: Hash::ZERO,
                data: vec![],
            },
            solution: coinject_core::Solution::Custom(vec![]),
        },
    }
}

/// Create a PeerInfo for router testing
fn router_test_peer(id: u8, height: u64, quality: f64) -> PeerInfo {
    PeerInfo {
        id: [id; 32],
        best_height: height,
        node_type: NodeType::Full.as_u8(),
        quality,
        last_seen: 0,
        flock_phase: id % 8,
        flock_epoch: 0,
        velocity: 0.0,
    }
}

/// Wait for a specific event variant, ignoring others, with a timeout
async fn wait_for_event(
    event_rx: &mut tokio::sync::mpsc::UnboundedReceiver<NetworkEvent>,
    timeout_secs: u64,
    predicate: impl Fn(&NetworkEvent) -> bool,
) -> Option<NetworkEvent> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);
    loop {
        match tokio::time::timeout_at(deadline, event_rx.recv()).await {
            Ok(Some(event)) => {
                if predicate(&event) {
                    return Some(event);
                }
                // Otherwise keep waiting (skip StatusUpdate, etc.)
            }
            _ => return None, // Timeout or channel closed
        }
    }
}

// =============================================================================
// Test 3.1 — Two Node Connect and Sync
// =============================================================================
// Spawns two CppNetwork instances on localhost, connects B→A via
// ConnectBootnode, and asserts PeerConnected events fire on both sides.

#[tokio::test]
async fn test_two_node_connect_and_sync() {
    let genesis = Hash::ZERO;

    // Node A — listens on fixed test port
    let config_a = test_config(17071);
    let peer_id_a = [1u8; 32];
    let (network_a, _cmd_a, mut events_a) = CppNetwork::new(config_a, peer_id_a, genesis);

    // Node B — listens on different port
    let config_b = test_config(17072);
    let peer_id_b = [2u8; 32];
    let (network_b, cmd_b, mut events_b) = CppNetwork::new(config_b, peer_id_b, genesis);

    // Spawn both network event loops
    let _handle_a = tokio::spawn(network_a.start());
    let _handle_b = tokio::spawn(network_b.start());

    // Give listeners time to bind
    tokio::time::sleep(Duration::from_millis(100)).await;

    // B connects to A
    let addr_a: SocketAddr = "127.0.0.1:17071".parse().unwrap();
    cmd_b
        .send(NetworkCommand::ConnectBootnode { addr: addr_a })
        .unwrap();

    // B should receive PeerConnected
    let event_b = wait_for_event(&mut events_b, 5, |e| {
        matches!(e, NetworkEvent::PeerConnected { .. })
    })
    .await;
    assert!(
        event_b.is_some(),
        "Node B did not receive PeerConnected within 5s"
    );

    if let Some(NetworkEvent::PeerConnected {
        peer_id, node_type, ..
    }) = &event_b
    {
        assert_eq!(*peer_id, peer_id_a, "B should see A's peer ID");
        assert_eq!(*node_type, NodeType::Full);
    }

    // A should also receive PeerConnected (from inbound connection)
    let event_a = wait_for_event(&mut events_a, 5, |e| {
        matches!(e, NetworkEvent::PeerConnected { .. })
    })
    .await;
    assert!(
        event_a.is_some(),
        "Node A did not receive PeerConnected within 5s"
    );

    if let Some(NetworkEvent::PeerConnected { peer_id, .. }) = &event_a {
        assert_eq!(*peer_id, peer_id_b, "A should see B's peer ID");
    }
}

// =============================================================================
// Test 3.2 — Block Propagation
// =============================================================================
// After connecting two nodes, broadcasts a block from A and verifies
// B receives it as a BlockReceived event.

#[tokio::test]
async fn test_block_propagation() {
    let genesis = Hash::ZERO;

    // Node A
    let config_a = test_config(17073);
    let peer_id_a = [3u8; 32];
    let (network_a, cmd_a, mut events_a) = CppNetwork::new(config_a, peer_id_a, genesis);

    // Node B
    let config_b = test_config(17074);
    let peer_id_b = [4u8; 32];
    let (network_b, cmd_b, mut events_b) = CppNetwork::new(config_b, peer_id_b, genesis);

    let _handle_a = tokio::spawn(network_a.start());
    let _handle_b = tokio::spawn(network_b.start());

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Connect B → A
    let addr_a: SocketAddr = "127.0.0.1:17073".parse().unwrap();
    cmd_b
        .send(NetworkCommand::ConnectBootnode { addr: addr_a })
        .unwrap();

    // Wait for connection to establish on both sides
    let connected_b = wait_for_event(&mut events_b, 5, |e| {
        matches!(e, NetworkEvent::PeerConnected { .. })
    })
    .await;
    assert!(connected_b.is_some(), "B did not connect to A");

    let connected_a = wait_for_event(&mut events_a, 5, |e| {
        matches!(e, NetworkEvent::PeerConnected { .. })
    })
    .await;
    assert!(connected_a.is_some(), "A did not see B's connection");

    // A broadcasts a block
    let block = create_test_block(1, genesis);
    let block_height = block.header.height;
    cmd_a
        .send(NetworkCommand::BroadcastBlock { block })
        .unwrap();

    // B should receive the block
    let block_event = wait_for_event(&mut events_b, 5, |e| {
        matches!(e, NetworkEvent::BlockReceived { .. })
    })
    .await;
    assert!(
        block_event.is_some(),
        "Node B did not receive broadcasted block within 5s"
    );

    if let Some(NetworkEvent::BlockReceived { block, peer_id }) = block_event {
        assert_eq!(block.header.height, block_height, "Block height mismatch");
        assert_eq!(peer_id, peer_id_a, "Block should come from A");
    }
}

// =============================================================================
// Test 3.3 — Peer Reconnection
// =============================================================================
// Connects two nodes, disconnects, then reconnects and verifies the
// second handshake succeeds.

#[tokio::test]
async fn test_peer_reconnection() {
    let genesis = Hash::ZERO;

    // Node A
    let config_a = test_config(17075);
    let peer_id_a = [5u8; 32];
    let (network_a, _cmd_a, mut events_a) = CppNetwork::new(config_a, peer_id_a, genesis);

    // Node B
    let config_b = test_config(17076);
    let peer_id_b = [6u8; 32];
    let (network_b, cmd_b, mut events_b) = CppNetwork::new(config_b, peer_id_b, genesis);

    let _handle_a = tokio::spawn(network_a.start());
    let _handle_b = tokio::spawn(network_b.start());

    tokio::time::sleep(Duration::from_millis(100)).await;

    // First connection: B → A
    let addr_a: SocketAddr = "127.0.0.1:17075".parse().unwrap();
    cmd_b
        .send(NetworkCommand::ConnectBootnode { addr: addr_a })
        .unwrap();

    // Wait for connection
    let connected = wait_for_event(&mut events_b, 5, |e| {
        matches!(e, NetworkEvent::PeerConnected { .. })
    })
    .await;
    assert!(connected.is_some(), "Initial connection failed");

    // Also drain A's PeerConnected
    let _ = wait_for_event(&mut events_a, 5, |e| {
        matches!(e, NetworkEvent::PeerConnected { .. })
    })
    .await;

    // Disconnect from B's side (B controls the reconnect, so it must
    // clear its own peer map immediately — A-side disconnect would require
    // waiting for B's read timeout to detect the TCP close)
    cmd_b
        .send(NetworkCommand::DisconnectPeer {
            peer_id: peer_id_a,
            reason: "test reconnection".to_string(),
        })
        .unwrap();

    // Wait for B's disconnect event (immediate since B initiated)
    let _disconnected_b = wait_for_event(&mut events_b, 5, |e| {
        matches!(e, NetworkEvent::PeerDisconnected { .. })
    })
    .await;

    // Drain any disconnect event on A too
    let _disconnected_a = wait_for_event(&mut events_a, 3, |e| {
        matches!(e, NetworkEvent::PeerDisconnected { .. })
    })
    .await;

    // Give TCP connections time to fully close
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Reconnect: B → A again
    cmd_b
        .send(NetworkCommand::ConnectBootnode { addr: addr_a })
        .unwrap();

    // Should get PeerConnected again
    let reconnected = wait_for_event(&mut events_b, 10, |e| {
        matches!(e, NetworkEvent::PeerConnected { .. })
    })
    .await;
    assert!(
        reconnected.is_some(),
        "Reconnection failed — B did not receive PeerConnected"
    );
}

// =============================================================================
// Test 3.4 — Router Fanout Math
// =============================================================================
// Pure unit tests verifying the equilibrium constant η = 1/√2 governs
// broadcast fanout as fanout = ⌈√n × η⌉

#[test]
fn test_router_fanout_formula() {
    // η = 1/√2 ≈ 0.7071
    assert!(
        (ETA - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-6,
        "ETA should equal 1/√2"
    );

    // Test fanout = ⌈√n × η⌉ for various peer counts
    // Note: Due to floating-point precision, we verify against the same
    // computation the router uses internally rather than hardcoded values.
    for n in [1, 2, 4, 9, 16, 25, 36, 49, 64, 100] {
        let mut router = EquilibriumRouter::new();
        for i in 0..n {
            router.add_peer(router_test_peer(i as u8, 100, 1.0));
        }

        let selected = router.select_broadcast_peers();
        let computed_fanout = ((n as f64).sqrt() * ETA).ceil() as usize;

        assert_eq!(
            selected.len(),
            computed_fanout,
            "n={}: fanout should be ceil(sqrt({}) * η) = {}, got {}",
            n,
            n,
            computed_fanout,
            selected.len()
        );

        // Sanity: fanout should always be >= 1 and <= n
        assert!(selected.len() >= 1, "n={}: fanout must be >= 1", n);
        assert!(selected.len() <= n, "n={}: fanout must be <= n", n);

        // Verify all selected peers are valid
        for peer_id in &selected {
            assert!(
                router.get_peer(peer_id).is_some(),
                "n={}: selected peer should exist in router",
                n
            );
        }
    }
}

#[test]
fn test_router_quality_decay_uses_eta() {
    let mut router = EquilibriumRouter::new();
    let id = [1u8; 32];
    router.add_peer(router_test_peer(1, 100, 1.0));

    // Failure: quality *= (1 - η)
    router.update_peer_quality(&id, false);
    let q = router.get_peer(&id).unwrap().quality;
    let expected = 1.0 * (1.0 - ETA); // ≈ 0.2929
    assert!(
        (q - expected).abs() < 0.001,
        "After failure: expected quality ≈ {:.4}, got {:.4}",
        expected,
        q
    );

    // Success: quality += 0.1
    router.update_peer_quality(&id, true);
    let q2 = router.get_peer(&id).unwrap().quality;
    assert!(
        (q2 - (expected + 0.1)).abs() < 0.001,
        "After success: expected quality ≈ {:.4}, got {:.4}",
        expected + 0.1,
        q2
    );
}

#[test]
fn test_router_sync_peer_selects_closest() {
    let mut router = EquilibriumRouter::new();

    // Peers at heights 50, 100, 150, 200
    router.add_peer(router_test_peer(1, 50, 1.0));
    router.add_peer(router_test_peer(2, 100, 1.0));
    router.add_peer(router_test_peer(3, 150, 1.0));
    router.add_peer(router_test_peer(4, 200, 1.0));

    // Need height 120 → closest above is peer 3 at 150
    let selected = router.select_sync_peer(120).unwrap();
    assert_eq!(
        selected, [3; 32],
        "Should select peer at height 150 (closest >= 120)"
    );

    // Need height 200 → only peer 4 qualifies
    let selected = router.select_sync_peer(200).unwrap();
    assert_eq!(selected, [4; 32], "Should select peer at height 200");

    // Need height 201 → nobody qualifies
    let selected = router.select_sync_peer(201);
    assert!(selected.is_none(), "No peer at height >= 201");
}

#[test]
fn test_router_chunk_size_adaptive() {
    let router = EquilibriumRouter::new();

    // chunk = base × (1 + Δh × η / 10)
    // Small delta: 20 × (1 + 10 × 0.7071 / 10) = 20 × 1.7071 ≈ 34.14 → 35
    let chunk = router.calculate_chunk_size(10, 20, 100);
    let expected = (20.0 * (1.0 + 10.0 * ETA / 10.0)).ceil() as u64;
    assert_eq!(
        chunk, expected,
        "Adaptive chunk for delta=10: expected {}, got {}",
        expected, chunk
    );

    // Large delta: capped at max_chunk
    let chunk = router.calculate_chunk_size(1000, 20, 100);
    assert_eq!(chunk, 100, "Large delta should cap at max_chunk=100");

    // Zero delta: base chunk
    let chunk = router.calculate_chunk_size(0, 20, 100);
    assert_eq!(chunk, 20, "Zero delta should return base chunk");
}

#[test]
fn test_router_flock_broadcast_uses_reynolds_rules() {
    let mut router = EquilibriumRouter::new();

    // Create 16 peers: 12 at height 100, 4 "divergent" at height 500
    for i in 0..12u8 {
        router.add_peer(router_test_peer(i, 100, 1.0));
    }
    for i in 12..16u8 {
        router.add_peer(router_test_peer(i, 500, 1.0));
    }

    // Our height is 100 (aligned with majority)
    let selected = router.select_broadcast_peers_flock(100, 0);

    // Should prefer peers near our height (separation rule penalizes divergent peers)
    let near_peers: Vec<_> = selected
        .iter()
        .filter(|id| {
            let peer = router.get_peer(id).unwrap();
            peer.best_height <= 200
        })
        .collect();

    let far_peers: Vec<_> = selected
        .iter()
        .filter(|id| {
            let peer = router.get_peer(id).unwrap();
            peer.best_height > 200
        })
        .collect();

    // Near peers should dominate the selection
    assert!(
        near_peers.len() >= far_peers.len(),
        "Separation rule: near peers ({}) should outnumber far peers ({})",
        near_peers.len(),
        far_peers.len()
    );
}
