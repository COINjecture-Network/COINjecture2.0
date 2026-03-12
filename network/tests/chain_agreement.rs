// =============================================================================
// Chain Agreement Integration Test — The Consensus Proof
// =============================================================================
//
// This test proves that COINjecture achieves consensus:
//   assert_eq!(node_a.block_hash_at(height), node_b.block_hash_at(height))
//
// Flow:
//   1. 3 nodes form a mesh network (coordinators NOT started yet)
//   2. Wait for full mesh connectivity
//   3. Start coordinators simultaneously — now all nodes share the same peer set
//   4. Each epoch: auto-solve → commit → seal → winner builds Block
//   5. Block broadcast via mesh; all nodes store it
//   6. Assert all nodes agree on block hashes at each height

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, RwLock};
use tokio::time;

use coinject_consensus::{
    CoordinatorCommand, CoordinatorConfig, CoordinatorEvent, EpochCoordinator, SolutionCommit,
};
use coinject_core::{Hash, ProblemType, Solution};
use coinject_network::mesh::bridge::{BridgeCommand, BridgeEvent, BridgeState, run_bridge};
use coinject_network::mesh::config::NetworkConfig;
use coinject_network::NetworkService;

/// In-memory chain store: height → block hash.
type ChainStore = Arc<RwLock<HashMap<u64, Hash>>>;

/// A test node with full block production and storage wiring.
struct ChainNode {
    coord_cmd_tx: mpsc::UnboundedSender<CoordinatorCommand>,
    node_id: [u8; 32],
    events: Arc<RwLock<Vec<CoordinatorEvent>>>,
    chain: ChainStore,
    mesh_service: Option<NetworkService>,
}

impl ChainNode {
    /// Phase 1: Start the mesh network and bridge, but NOT the coordinator.
    /// Returns a pre-node that can be upgraded to a full ChainNode once peers are connected.
    async fn start_mesh(
        listen_addr: &str,
        seeds: Vec<std::net::SocketAddr>,
    ) -> PreNode {
        let listen: std::net::SocketAddr = listen_addr.parse().unwrap();
        let data_dir = tempfile::tempdir().unwrap();

        let mesh_config = NetworkConfig {
            listen_addr: listen,
            seed_nodes: seeds,
            data_dir: data_dir.into_path(),
            ..Default::default()
        };

        let (mesh_service, mesh_event_rx) = NetworkService::start(mesh_config)
            .await
            .expect("mesh start failed");

        let mesh_cmd_tx = mesh_service.command_sender();
        let mesh_node_id = *mesh_service.local_id();
        let node_id: [u8; 32] = mesh_node_id.0;

        // Bridge channels
        let (bridge_cmd_tx, bridge_cmd_rx) = mpsc::unbounded_channel::<BridgeCommand>();
        let (bridge_event_tx, bridge_event_rx) = mpsc::unbounded_channel::<BridgeEvent>();

        let bridge_state = Arc::new(RwLock::new(BridgeState {
            best_height: 0,
            best_hash: Hash::from_bytes([0; 32]),
            epoch: 0,
        }));

        // Spawn bridge
        let bridge_state_clone = Arc::clone(&bridge_state);
        tokio::spawn(async move {
            run_bridge(bridge_cmd_rx, bridge_event_tx, mesh_cmd_tx, mesh_event_rx, bridge_state_clone).await;
        });

        PreNode {
            mesh_service,
            node_id,
            bridge_cmd_tx,
            bridge_event_rx,
        }
    }
}

/// Pre-node: mesh + bridge running, coordinator not yet started.
struct PreNode {
    mesh_service: NetworkService,
    node_id: [u8; 32],
    bridge_cmd_tx: mpsc::UnboundedSender<BridgeCommand>,
    bridge_event_rx: mpsc::UnboundedReceiver<BridgeEvent>,
}

impl PreNode {
    /// Phase 2: Start the coordinator and wire everything up.
    /// Call this only after all mesh peers are connected.
    fn start_coordinator(self, coord_config: CoordinatorConfig) -> ChainNode {
        let PreNode { mesh_service, node_id, bridge_cmd_tx, mut bridge_event_rx } = self;

        // Coordinator channels
        let (coord_cmd_tx, coord_cmd_rx) = mpsc::unbounded_channel::<CoordinatorCommand>();
        let (coord_event_tx, mut coord_event_rx) = mpsc::unbounded_channel::<CoordinatorEvent>();

        let (coordinator, _shared_state) = EpochCoordinator::new(
            node_id,
            coord_config,
            0,
            Hash::from_bytes([0; 32]),
        );

        // Spawn coordinator
        tokio::spawn(async move {
            coordinator.run(coord_cmd_rx, coord_event_tx).await;
        });

        // Chain store
        let chain: ChainStore = Arc::new(RwLock::new(HashMap::new()));

        // Collected events
        let events = Arc::new(RwLock::new(Vec::<CoordinatorEvent>::new()));

        // ── Coordinator event handler ──
        let events_clone = Arc::clone(&events);
        let bridge_cmd_for_coord = bridge_cmd_tx.clone();
        let coord_cmd_for_mine = coord_cmd_tx.clone();
        let chain_for_events = Arc::clone(&chain);
        let coord_node_id = node_id;

        tokio::spawn(async move {
            while let Some(event) = coord_event_rx.recv().await {
                match &event {
                    CoordinatorEvent::BroadcastSalt { epoch, salt } => {
                        let _ = bridge_cmd_for_coord.send(BridgeCommand::BroadcastConsensusSalt {
                            epoch: *epoch,
                            salt: *salt,
                        });
                    }
                    CoordinatorEvent::BroadcastCommit { epoch, solution_hash, work_score } => {
                        let _ = bridge_cmd_for_coord.send(BridgeCommand::BroadcastCommit {
                            epoch: *epoch,
                            solution_hash: *solution_hash,
                            node_id: coord_node_id,
                            work_score: *work_score,
                            signature: Vec::new(),
                        });
                    }
                    CoordinatorEvent::MinePhaseStarted { epoch, .. } => {
                        // Auto-solve: submit a valid solution immediately
                        let epoch = *epoch;
                        let mut hash = [0u8; 32];
                        hash[..8].copy_from_slice(&epoch.to_le_bytes());
                        hash[8..16].copy_from_slice(&coord_node_id[..8]);
                        // Deterministic work score unique to this node
                        let work_score = 100.0 + (coord_node_id[0] as f64);

                        let _ = coord_cmd_for_mine.send(CoordinatorCommand::LocalSolutionReady {
                            epoch,
                            solution_hash: hash,
                            work_score,
                            problem: ProblemType::SubsetSum {
                                numbers: vec![1, 2, 3, 4, 5],
                                target: 9,
                            },
                            solution: Solution::SubsetSum(vec![3, 4]), // 4+5=9
                            solve_time: Duration::from_millis(50),
                        });
                    }
                    CoordinatorEvent::BlockProduced { block, epoch } => {
                        let block_hash = block.header.hash();
                        let height = block.header.height;
                        tracing::info!(
                            epoch, height,
                            hash = hex::encode(&block_hash.as_bytes()[..4]),
                            node = hex::encode(&coord_node_id[..4]),
                            "BlockProduced: storing locally + broadcasting"
                        );

                        // Store the block we produced
                        chain_for_events.write().await.insert(height, block_hash);

                        // Broadcast to peers
                        let _ = bridge_cmd_for_coord.send(BridgeCommand::BroadcastBlock {
                            block: block.clone(),
                        });
                    }
                    _ => {}
                }
                events_clone.write().await.push(event);
            }
        });

        // ── Bridge event handler ──
        let coord_cmd_for_bridge = coord_cmd_tx.clone();
        let chain_for_bridge = Arc::clone(&chain);

        tokio::spawn(async move {
            while let Some(event) = bridge_event_rx.recv().await {
                match event {
                    BridgeEvent::PeerConnected { peer_id, .. } => {
                        let _ = coord_cmd_for_bridge.send(CoordinatorCommand::PeerJoined {
                            node_id: peer_id.0,
                        });
                    }
                    BridgeEvent::PeerDisconnected { peer_id, .. } => {
                        let _ = coord_cmd_for_bridge.send(CoordinatorCommand::PeerLeft {
                            node_id: peer_id.0,
                        });
                    }
                    BridgeEvent::ConsensusSaltReceived { epoch, salt, from } => {
                        let _ = coord_cmd_for_bridge.send(CoordinatorCommand::SaltReceived {
                            epoch,
                            salt,
                            from: from.0,
                        });
                    }
                    BridgeEvent::ConsensusCommitReceived { epoch, commits, .. } => {
                        for commit in commits {
                            let _ = coord_cmd_for_bridge.send(CoordinatorCommand::CommitReceived {
                                epoch,
                                commit: SolutionCommit {
                                    node_id: commit.node_id.0,
                                    solution_hash: commit.solution_hash,
                                    work_score: commit.work_score,
                                    signature: commit.signature,
                                },
                            });
                        }
                    }
                    BridgeEvent::BlockReceived { block, peer_id } => {
                        let height = block.header.height;
                        let block_hash = block.header.hash();
                        tracing::info!(
                            height,
                            hash = hex::encode(&block_hash.as_bytes()[..4]),
                            from = hex::encode(&peer_id.0[..4]),
                            "BlockReceived: storing in chain"
                        );
                        chain_for_bridge.write().await.insert(height, block_hash);

                        // Update coordinator's chain tip
                        let _ = coord_cmd_for_bridge.send(CoordinatorCommand::ChainTipUpdated {
                            height,
                            hash: block_hash,
                        });
                    }
                    _ => {}
                }
            }
        });

        ChainNode {
            coord_cmd_tx,
            node_id,
            events,
            chain,
            mesh_service: Some(mesh_service),
        }
    }
}

impl ChainNode {
    /// Get the block hash at a given height.
    async fn block_hash_at(&self, height: u64) -> Option<Hash> {
        self.chain.read().await.get(&height).copied()
    }

    /// Get all stored heights, sorted.
    async fn stored_heights(&self) -> Vec<u64> {
        let mut heights: Vec<u64> = self.chain.read().await.keys().copied().collect();
        heights.sort();
        heights
    }

    /// Count BlockProduced events.
    async fn blocks_produced(&self) -> usize {
        self.events.read().await.iter()
            .filter(|e| matches!(e, CoordinatorEvent::BlockProduced { .. }))
            .count()
    }

    /// Count EpochSealed events.
    async fn epochs_sealed(&self) -> usize {
        self.events.read().await.iter()
            .filter(|e| matches!(e, CoordinatorEvent::EpochSealed { .. }))
            .count()
    }

    async fn shutdown(mut self) {
        if let Some(svc) = self.mesh_service.take() {
            let _ = svc.shutdown().await;
        }
    }
}

fn short_id(id: &[u8; 32]) -> String {
    hex::encode(&id[..4])
}

// =============================================================================
// THE CONSENSUS PROOF TEST
// =============================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn test_three_node_chain_agreement() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("chain_agreement=info,coinject_consensus=info,coinject_network::mesh=info")
        .try_init();

    let port_a = 19400;
    let port_b = 19401;
    let port_c = 19402;
    let addr_a: std::net::SocketAddr = format!("127.0.0.1:{}", port_a).parse().unwrap();
    let addr_b: std::net::SocketAddr = format!("127.0.0.1:{}", port_b).parse().unwrap();

    // ── Phase 1: Start mesh networks (no coordinators yet) ──
    // Full mesh: A←B, A←C, B←C so every pair is directly connected.
    // This ensures commit broadcasts reach all nodes (mesh doesn't gossip-relay).
    let pre_a = ChainNode::start_mesh(
        &format!("127.0.0.1:{}", port_a),
        vec![],
    ).await;

    let pre_b = ChainNode::start_mesh(
        &format!("127.0.0.1:{}", port_b),
        vec![addr_a],
    ).await;

    let pre_c = ChainNode::start_mesh(
        &format!("127.0.0.1:{}", port_c),
        vec![addr_a, addr_b],
    ).await;

    println!("Node A: {}", short_id(&pre_a.node_id));
    println!("Node B: {}", short_id(&pre_b.node_id));
    println!("Node C: {}", short_id(&pre_c.node_id));

    // Wait for mesh connections to fully form (3 nodes = 2 connections each)
    println!("Waiting for mesh connections...");
    tokio::time::sleep(Duration::from_secs(3)).await;

    // ── Phase 2: Start coordinators simultaneously ──
    // Now all nodes have the same peer set, so leader election will be consistent
    let config = CoordinatorConfig {
        salt_duration: Duration::from_millis(500),
        mine_duration: Duration::from_secs(2),
        commit_duration: Duration::from_secs(1),
        seal_duration: Duration::from_millis(500),
        stall_timeout: Duration::from_secs(10),
        quorum_threshold: 0.5,  // 2 of 3 is enough
        max_consecutive_stalls: 5,
        failover_depth: 3,
    };

    println!("Starting coordinators...");
    let node_a = pre_a.start_coordinator(config.clone());
    let node_b = pre_b.start_coordinator(config.clone());
    let node_c = pre_c.start_coordinator(config.clone());

    // Wait for at least 2 blocks to be produced
    let target_blocks = 2u64;
    let deadline = time::Instant::now() + Duration::from_secs(30);

    loop {
        let a_heights = node_a.stored_heights().await;
        let b_heights = node_b.stored_heights().await;
        let c_heights = node_c.stored_heights().await;

        let max_height = a_heights.last().copied().unwrap_or(0)
            .max(b_heights.last().copied().unwrap_or(0))
            .max(c_heights.last().copied().unwrap_or(0));

        if max_height >= target_blocks {
            println!("\nChain heights reached target ({}):", target_blocks);
            println!("  A: {:?}", a_heights);
            println!("  B: {:?}", b_heights);
            println!("  C: {:?}", c_heights);
            break;
        }

        if time::Instant::now() > deadline {
            let a_sealed = node_a.epochs_sealed().await;
            let b_sealed = node_b.epochs_sealed().await;
            let c_sealed = node_c.epochs_sealed().await;
            let a_produced = node_a.blocks_produced().await;
            let b_produced = node_b.blocks_produced().await;
            let c_produced = node_c.blocks_produced().await;

            println!("\nTimeout! Chain state:");
            println!("  A heights: {:?}, sealed: {}, produced: {}", a_heights, a_sealed, a_produced);
            println!("  B heights: {:?}, sealed: {}, produced: {}", b_heights, b_sealed, b_produced);
            println!("  C heights: {:?}, sealed: {}, produced: {}", c_heights, c_sealed, c_produced);

            assert!(
                a_heights.len().max(b_heights.len()).max(c_heights.len()) >= 1,
                "Should have produced at least 1 block"
            );
            break;
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    // ══════════════════════════════════════════════════════════════════════
    // THE CONSENSUS PROOF: All nodes agree on block hashes at each height
    // ══════════════════════════════════════════════════════════════════════

    // Give time for block propagation to complete
    tokio::time::sleep(Duration::from_secs(2)).await;

    let a_heights = node_a.stored_heights().await;
    let b_heights = node_b.stored_heights().await;
    let c_heights = node_c.stored_heights().await;

    // Find the common heights all nodes have
    let mut common_heights: Vec<u64> = a_heights.iter()
        .filter(|h| b_heights.contains(h) && c_heights.contains(h))
        .copied()
        .collect();
    common_heights.sort();

    println!("\n═══════════════════════════════════════");
    println!("  CONSENSUS PROOF — Chain Agreement");
    println!("═══════════════════════════════════════");

    assert!(
        !common_heights.is_empty(),
        "All 3 nodes should have at least 1 common block height.\n  A: {:?}\n  B: {:?}\n  C: {:?}",
        a_heights, b_heights, c_heights
    );

    for height in &common_heights {
        let hash_a = node_a.block_hash_at(*height).await.unwrap();
        let hash_b = node_b.block_hash_at(*height).await.unwrap();
        let hash_c = node_c.block_hash_at(*height).await.unwrap();

        let short_a = hex::encode(&hash_a.as_bytes()[..8]);
        let short_b = hex::encode(&hash_b.as_bytes()[..8]);
        let short_c = hex::encode(&hash_c.as_bytes()[..8]);

        println!(
            "  Height {}: A={} B={} C={} {}",
            height, short_a, short_b, short_c,
            if hash_a == hash_b && hash_b == hash_c { "AGREE" } else { "DISAGREE" }
        );

        assert_eq!(
            hash_a, hash_b,
            "Node A and B disagree at height {}!\n  A: {}\n  B: {}",
            height, short_a, short_b
        );
        assert_eq!(
            hash_b, hash_c,
            "Node B and C disagree at height {}!\n  B: {}\n  C: {}",
            height, short_b, short_c
        );
    }

    println!("═══════════════════════════════════════");
    println!("  {} heights verified — CONSENSUS PROVED", common_heights.len());
    println!("═══════════════════════════════════════\n");

    // Cleanup
    node_a.shutdown().await;
    node_b.shutdown().await;
    node_c.shutdown().await;
}
