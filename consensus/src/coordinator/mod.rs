// =============================================================================
// Epoch Coordinator
// =============================================================================
//
// Orchestrates the multi-node consensus cycle:
//   Salt → Mine → Commit → Seal → (next epoch)
//
// The coordinator runs as a single async task, driven by a tokio::select! loop
// that listens for:
//   - Phase timer expirations (drives the state machine forward)
//   - Incoming consensus messages from the mesh bridge
//   - Commands from the local mining pipeline
//
// It communicates with the rest of the system via channels:
//   - CoordinatorEvent: events emitted to the node service
//   - CoordinatorCommand: commands received from the node service / bridge

pub mod config;
pub mod leader;
pub mod epoch;
pub mod commit;

pub use config::CoordinatorConfig;
pub use leader::NodeId;
pub use epoch::{EpochPhase, EpochState};
pub use commit::{CommitCollector, SolutionCommit};

use std::collections::BTreeSet;
use std::sync::Arc;

use coinject_core::{Address, Block, Hash, ProblemType, Solution};
use tokio::sync::{mpsc, RwLock};

// ─── Events (coordinator → node service) ─────────────────────────────────────

/// Events emitted by the coordinator to the node service / bridge.
#[derive(Debug, Clone)]
pub enum CoordinatorEvent {
    /// A new epoch has started. The node should prepare to mine.
    EpochStarted {
        epoch: u64,
        salt: [u8; 32],
        leader: NodeId,
    },

    /// The Mine phase has begun. The node should start solving.
    MinePhaseStarted {
        epoch: u64,
        salt: [u8; 32],
    },

    /// The Commit phase has begun. The node should broadcast its commitment.
    CommitPhaseStarted {
        epoch: u64,
    },

    /// A winner has been selected for this epoch.
    EpochSealed {
        epoch: u64,
        winner: NodeId,
        work_score: f64,
        commit_count: usize,
    },

    /// The epoch stalled (leader didn't produce, or quorum wasn't reached).
    EpochStalled {
        epoch: u64,
        phase: EpochPhase,
        reason: String,
    },

    /// Broadcast a salt message to mesh peers (leader duty).
    BroadcastSalt {
        epoch: u64,
        salt: [u8; 32],
    },

    /// Broadcast our solution commitment to mesh peers.
    BroadcastCommit {
        epoch: u64,
        solution_hash: [u8; 32],
        work_score: f64,
    },

    /// A block was produced by this node (we won the epoch).
    BlockProduced {
        block: Block,
        epoch: u64,
    },
}

// ─── Commands (node service / bridge → coordinator) ──────────────────────────

/// Commands sent to the coordinator from the node service or bridge.
#[derive(Debug, Clone)]
pub enum CoordinatorCommand {
    /// A salt was received from the mesh (from the leader).
    SaltReceived {
        epoch: u64,
        salt: [u8; 32],
        from: NodeId,
    },

    /// Our local mining completed — submit our solution commitment.
    LocalSolutionReady {
        epoch: u64,
        solution_hash: [u8; 32],
        work_score: f64,
        problem: ProblemType,
        solution: Solution,
        solve_time: std::time::Duration,
    },

    /// A peer's solution commitment was received from the mesh.
    CommitReceived {
        epoch: u64,
        commit: SolutionCommit,
    },

    /// A new peer joined the mesh (update peer set).
    PeerJoined {
        node_id: NodeId,
    },

    /// A peer left the mesh.
    PeerLeft {
        node_id: NodeId,
    },

    /// Update the chain tip (for salt derivation).
    ChainTipUpdated {
        height: u64,
        hash: Hash,
    },
}

// ─── Coordinator Shared State ────────────────────────────────────────────────

/// Shared read-only state that external components can query.
#[derive(Debug)]
pub struct CoordinatorState {
    pub epoch: u64,
    pub phase: EpochPhase,
    pub leader: Option<NodeId>,
    pub commit_count: usize,
    pub peer_count: usize,
}

// ─── Local Solution Cache ───────────────────────────────────────────────────

/// Cached solution from our local mining for block production.
struct LocalSolution {
    problem: ProblemType,
    solution: Solution,
    work_score: f64,
    solve_time: std::time::Duration,
}

// ─── Coordinator ─────────────────────────────────────────────────────────────

/// The main epoch coordinator.
pub struct EpochCoordinator {
    /// Our node's identity.
    our_id: NodeId,
    /// Configuration.
    config: CoordinatorConfig,
    /// Known peer set (BTreeSet for deterministic ordering).
    peers: BTreeSet<NodeId>,
    /// Current epoch state.
    epoch_state: EpochState,
    /// Commit collector for the current epoch.
    collector: CommitCollector,
    /// Current chain tip.
    chain_height: u64,
    chain_hash: Hash,
    /// Our miner address for block production.
    miner_address: Address,
    /// Cached local solution for the current epoch (if we solved it).
    local_solution: Option<LocalSolution>,
    /// Consecutive stall counter.
    consecutive_stalls: u32,
    /// Shared state for external queries.
    shared_state: Arc<RwLock<CoordinatorState>>,
}

impl EpochCoordinator {
    /// Create a new coordinator.
    pub fn new(
        our_id: NodeId,
        config: CoordinatorConfig,
        chain_height: u64,
        chain_hash: Hash,
    ) -> (Self, Arc<RwLock<CoordinatorState>>) {
        Self::with_miner_address(our_id, config, chain_height, chain_hash, Address::from_bytes(our_id))
    }

    /// Create a new coordinator with an explicit miner address for block production.
    pub fn with_miner_address(
        our_id: NodeId,
        config: CoordinatorConfig,
        chain_height: u64,
        chain_hash: Hash,
        miner_address: Address,
    ) -> (Self, Arc<RwLock<CoordinatorState>>) {
        let shared_state = Arc::new(RwLock::new(CoordinatorState {
            epoch: 0,
            phase: EpochPhase::Salt,
            leader: None,
            commit_count: 0,
            peer_count: 0,
        }));

        let coordinator = Self {
            our_id,
            config,
            peers: BTreeSet::new(),
            epoch_state: EpochState::new(0),
            collector: CommitCollector::new(0),
            chain_height,
            chain_hash,
            miner_address,
            local_solution: None,
            consecutive_stalls: 0,
            shared_state: shared_state.clone(),
        };

        (coordinator, shared_state)
    }

    /// Run the coordinator event loop.
    ///
    /// This drives the Salt→Mine→Commit→Seal state machine, listening for
    /// commands from the bridge/node service and emitting events.
    pub async fn run(
        mut self,
        mut cmd_rx: mpsc::UnboundedReceiver<CoordinatorCommand>,
        event_tx: mpsc::UnboundedSender<CoordinatorEvent>,
    ) {
        // Add ourselves to the peer set
        self.peers.insert(self.our_id);

        // Start first epoch
        self.start_epoch(1, &event_tx).await;

        loop {
            // Calculate time until current phase expires
            let phase_remaining = self.phase_time_remaining();

            tokio::select! {
                // ── Phase timer ─────────────────────────────────────────
                _ = tokio::time::sleep(phase_remaining) => {
                    self.handle_phase_expiry(&event_tx).await;
                }

                // ── Incoming commands ───────────────────────────────────
                cmd = cmd_rx.recv() => {
                    match cmd {
                        Some(command) => {
                            self.handle_command(command, &event_tx).await;
                        }
                        None => {
                            tracing::info!("coordinator: command channel closed, shutting down");
                            break;
                        }
                    }
                }
            }

            // Update shared state
            self.update_shared_state().await;
        }

        tracing::info!("epoch coordinator exited");
    }

    /// Calculate remaining time in the current phase.
    fn phase_time_remaining(&self) -> std::time::Duration {
        let phase_duration = self.epoch_state.phase.duration(&self.config);
        let elapsed = self.epoch_state.phase_elapsed();
        phase_duration.saturating_sub(elapsed)
    }

    /// Start a new epoch.
    async fn start_epoch(
        &mut self,
        epoch: u64,
        event_tx: &mpsc::UnboundedSender<CoordinatorEvent>,
    ) {
        self.epoch_state = EpochState::new(epoch);
        self.collector = CommitCollector::new(epoch);
        self.local_solution = None;
        self.consecutive_stalls = 0;

        // Derive salt from chain tip: salt = H(epoch || chain_hash)
        let salt = self.derive_salt(epoch);
        self.epoch_state.set_salt(salt);

        // Elect leader
        let leader = leader::elect_leader(epoch, &self.chain_hash, &self.peers);

        tracing::info!(
            epoch = epoch,
            phase = "Salt",
            leader = ?leader.map(|l| hex::encode(&l[..4])),
            peers = self.peers.len(),
            "epoch started"
        );

        if let Some(leader_id) = leader {
            let _ = event_tx.send(CoordinatorEvent::EpochStarted {
                epoch,
                salt,
                leader: leader_id,
            });

            // If we are the leader, broadcast the salt
            if leader_id == self.our_id {
                tracing::info!(epoch, "we are the leader, broadcasting salt");
                let _ = event_tx.send(CoordinatorEvent::BroadcastSalt { epoch, salt });
            }
        }
    }

    /// Handle phase timer expiry.
    async fn handle_phase_expiry(
        &mut self,
        event_tx: &mpsc::UnboundedSender<CoordinatorEvent>,
    ) {
        let current_phase = self.epoch_state.phase;
        let epoch = self.epoch_state.epoch;

        // Hard deadline: force epoch termination if the epoch has run too long
        // regardless of which phase it's in. This prevents Byzantine leaders
        // from blocking consensus indefinitely via crafted timeouts.
        if self.epoch_state.has_exceeded_hard_deadline(&self.config) {
            tracing::error!(
                epoch,
                phase = %current_phase,
                elapsed_ms = self.epoch_state.epoch_elapsed().as_millis(),
                "epoch exceeded hard deadline — forcing new epoch (consensus safety)"
            );
            let _ = event_tx.send(CoordinatorEvent::EpochStalled {
                epoch,
                phase: current_phase,
                reason: format!(
                    "hard deadline exceeded after {}ms",
                    self.epoch_state.epoch_elapsed().as_millis()
                ),
            });
            self.consecutive_stalls += 1;
            self.start_epoch(epoch + 1, event_tx).await;
            return;
        }

        // Check for stall (phase + stall_timeout exceeded)
        if self.epoch_state.is_stalled(&self.config) {
            self.handle_stall(event_tx).await;
            return;
        }

        // Try to advance to the next phase
        match self.epoch_state.try_advance(&self.config) {
            Some(new_phase) if new_phase != current_phase => {
                tracing::info!(
                    epoch,
                    from = %current_phase,
                    to = %new_phase,
                    "phase transition"
                );

                match new_phase {
                    EpochPhase::Mine => {
                        if let Some(salt) = self.epoch_state.salt {
                            let _ = event_tx.send(CoordinatorEvent::MinePhaseStarted {
                                epoch,
                                salt,
                            });
                        }
                    }
                    EpochPhase::Commit => {
                        let _ = event_tx.send(CoordinatorEvent::CommitPhaseStarted { epoch });
                    }
                    EpochPhase::Seal => {
                        self.handle_seal(event_tx).await;
                    }
                    EpochPhase::Salt => {} // Shouldn't happen in forward transitions
                }
            }
            None => {
                // Seal phase completed → start next epoch
                tracing::info!(epoch, "epoch completed, starting next");
                self.start_epoch(epoch + 1, event_tx).await;
            }
            _ => {} // Phase not ready to advance yet
        }
    }

    /// Handle the Seal phase: select winner, build block if we won, emit events.
    async fn handle_seal(
        &mut self,
        event_tx: &mpsc::UnboundedSender<CoordinatorEvent>,
    ) {
        let epoch = self.epoch_state.epoch;
        let peer_count = self.peers.len();

        if self.collector.has_quorum(peer_count, self.config.quorum_threshold) {
            if let Some(winner) = self.collector.select_winner() {
                tracing::info!(
                    epoch,
                    winner = hex::encode(&winner.node_id[..4]),
                    score = winner.work_score,
                    commits = self.collector.commit_count(),
                    "epoch sealed with winner"
                );

                let _ = event_tx.send(CoordinatorEvent::EpochSealed {
                    epoch,
                    winner: winner.node_id,
                    work_score: winner.work_score,
                    commit_count: self.collector.commit_count(),
                });

                // If WE are the winner, build the block
                if winner.node_id == self.our_id {
                    if let Some(local) = self.local_solution.take() {
                        let prev_hash = self.chain_hash;
                        let height = self.chain_height + 1;
                        let miner_address = self.miner_address;
                        let difficulty = 1; // Minimum difficulty for now

                        tracing::info!(epoch, height, "we won! building block...");

                        let block = crate::build_block_from_solution(
                            prev_hash,
                            height,
                            miner_address,
                            local.problem,
                            local.solution,
                            local.solve_time,
                            local.work_score,
                            difficulty,
                            Vec::new(), // No pending transactions yet
                        );

                        if let Some(block) = block {
                            let block_hash = block.header.hash();
                            tracing::info!(
                                epoch,
                                height,
                                hash = hex::encode(&block_hash.as_bytes()[..4]),
                                "block produced"
                            );

                            // Update our chain tip
                            self.chain_height = height;
                            self.chain_hash = block_hash;

                            let _ = event_tx.send(CoordinatorEvent::BlockProduced {
                                block,
                                epoch,
                            });
                        } else {
                            tracing::error!(epoch, "failed to build block from solution");
                        }
                    } else {
                        tracing::warn!(epoch, "we won but have no cached solution");
                    }
                }
            }
        } else {
            tracing::warn!(
                epoch,
                commits = self.collector.commit_count(),
                peers = peer_count,
                threshold = self.config.quorum_threshold,
                "seal phase: quorum not reached"
            );
        }
    }

    /// Handle a stalled epoch.
    async fn handle_stall(
        &mut self,
        event_tx: &mpsc::UnboundedSender<CoordinatorEvent>,
    ) {
        let epoch = self.epoch_state.epoch;
        let phase = self.epoch_state.phase;
        self.consecutive_stalls += 1;

        tracing::warn!(
            epoch,
            phase = %phase,
            consecutive = self.consecutive_stalls,
            "epoch stalled"
        );

        let _ = event_tx.send(CoordinatorEvent::EpochStalled {
            epoch,
            phase,
            reason: format!(
                "phase {} exceeded timeout (stall #{})",
                phase, self.consecutive_stalls
            ),
        });

        if self.consecutive_stalls >= self.config.max_consecutive_stalls {
            tracing::error!(
                stalls = self.consecutive_stalls,
                "max consecutive stalls reached, hard reset"
            );
            self.consecutive_stalls = 0;
        }

        // Move to next epoch, effectively rotating the leader
        self.start_epoch(epoch + 1, event_tx).await;
    }

    /// Handle an incoming command.
    async fn handle_command(
        &mut self,
        command: CoordinatorCommand,
        event_tx: &mpsc::UnboundedSender<CoordinatorEvent>,
    ) {
        match command {
            CoordinatorCommand::SaltReceived { epoch, salt, from } => {
                if epoch != self.epoch_state.epoch {
                    tracing::debug!(
                        expected = self.epoch_state.epoch,
                        received = epoch,
                        "ignoring salt for wrong epoch"
                    );
                    return;
                }

                // Verify it came from the expected leader
                if let Some(expected_leader) = leader::elect_leader(epoch, &self.chain_hash, &self.peers) {
                    if from != expected_leader {
                        tracing::warn!(
                            from = hex::encode(&from[..4]),
                            expected = hex::encode(&expected_leader[..4]),
                            "salt from non-leader, ignoring"
                        );
                        return;
                    }
                }

                self.epoch_state.set_salt(salt);
                tracing::debug!(epoch, "salt received from leader");
            }

            CoordinatorCommand::LocalSolutionReady {
                epoch, solution_hash, work_score, problem, solution, solve_time,
            } => {
                if epoch != self.epoch_state.epoch {
                    return;
                }

                // Cache the solution for block production if we win
                self.local_solution = Some(LocalSolution {
                    problem,
                    solution,
                    work_score,
                    solve_time,
                });

                // Add our own commit
                let commit = SolutionCommit {
                    node_id: self.our_id,
                    solution_hash,
                    work_score,
                    signature: Vec::new(), // TODO: sign with node key
                };
                self.collector.add_commit(commit);

                // Broadcast our commitment
                let _ = event_tx.send(CoordinatorEvent::BroadcastCommit {
                    epoch,
                    solution_hash,
                    work_score,
                });
            }

            CoordinatorCommand::CommitReceived { epoch, commit } => {
                if epoch != self.epoch_state.epoch {
                    return;
                }
                if self.epoch_state.phase != EpochPhase::Commit
                    && self.epoch_state.phase != EpochPhase::Seal
                {
                    tracing::debug!(
                        epoch,
                        phase = %self.epoch_state.phase,
                        "commit received outside commit/seal phase, buffering anyway"
                    );
                }
                let node = commit.node_id;
                if self.collector.add_commit(commit) {
                    tracing::debug!(
                        epoch,
                        from = hex::encode(&node[..4]),
                        commits = self.collector.commit_count(),
                        "commit accepted"
                    );
                }
            }

            CoordinatorCommand::PeerJoined { node_id } => {
                self.peers.insert(node_id);
                tracing::debug!(
                    peer = hex::encode(&node_id[..4]),
                    total = self.peers.len(),
                    "peer joined"
                );
            }

            CoordinatorCommand::PeerLeft { node_id } => {
                self.peers.remove(&node_id);
                tracing::debug!(
                    peer = hex::encode(&node_id[..4]),
                    total = self.peers.len(),
                    "peer left"
                );
            }

            CoordinatorCommand::ChainTipUpdated { height, hash } => {
                self.chain_height = height;
                self.chain_hash = hash;
                tracing::debug!(height, "chain tip updated");
            }
        }
    }

    /// Derive the epoch salt: H(epoch_bytes || chain_hash).
    fn derive_salt(&self, epoch: u64) -> [u8; 32] {
        let mut data = Vec::with_capacity(8 + 32);
        data.extend_from_slice(&epoch.to_le_bytes());
        data.extend_from_slice(self.chain_hash.as_bytes());
        *Hash::new(&data).as_bytes()
    }

    /// Update the shared state for external queries.
    async fn update_shared_state(&self) {
        let leader = leader::elect_leader(
            self.epoch_state.epoch,
            &self.chain_hash,
            &self.peers,
        );

        let mut state = self.shared_state.write().await;
        state.epoch = self.epoch_state.epoch;
        state.phase = self.epoch_state.phase;
        state.leader = leader;
        state.commit_count = self.collector.commit_count();
        state.peer_count = self.peers.len();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{self, Duration};

    fn test_node_id(byte: u8) -> NodeId {
        let mut id = [0u8; 32];
        id[0] = byte;
        id
    }

    #[tokio::test]
    async fn test_coordinator_starts_epoch_1() {
        let our_id = test_node_id(1);
        let config = CoordinatorConfig {
            salt_duration: Duration::from_millis(50),
            mine_duration: Duration::from_millis(50),
            commit_duration: Duration::from_millis(50),
            seal_duration: Duration::from_millis(50),
            stall_timeout: Duration::from_secs(5),
            ..CoordinatorConfig::default()
        };

        let (coordinator, _state) = EpochCoordinator::new(
            our_id,
            config,
            0,
            Hash::from_bytes([0; 32]),
        );

        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();

        // Run coordinator in background
        tokio::spawn(async move {
            coordinator.run(cmd_rx, event_tx).await;
        });

        // Should receive EpochStarted event
        let event = time::timeout(Duration::from_secs(1), event_rx.recv())
            .await
            .expect("timeout waiting for event")
            .expect("channel closed");

        match event {
            CoordinatorEvent::EpochStarted { epoch, .. } => {
                assert_eq!(epoch, 1);
            }
            other => panic!("expected EpochStarted, got {:?}", other),
        }

        drop(cmd_tx);
    }

    #[tokio::test]
    async fn test_phase_transitions() {
        let our_id = test_node_id(1);
        let config = CoordinatorConfig {
            salt_duration: Duration::from_millis(20),
            mine_duration: Duration::from_millis(20),
            commit_duration: Duration::from_millis(20),
            seal_duration: Duration::from_millis(20),
            stall_timeout: Duration::from_secs(5),
            quorum_threshold: 0.0, // No quorum needed for this test
            ..CoordinatorConfig::default()
        };

        let (coordinator, state) = EpochCoordinator::new(
            our_id,
            config,
            0,
            Hash::from_bytes([0; 32]),
        );

        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            coordinator.run(cmd_rx, event_tx).await;
        });

        // Collect events for ~200ms (should see at least Salt→Mine→Commit→Seal transitions)
        let mut events = Vec::new();
        let deadline = time::Instant::now() + Duration::from_millis(300);

        loop {
            match time::timeout_at(deadline, event_rx.recv()).await {
                Ok(Some(event)) => events.push(event),
                _ => break,
            }
        }

        // Verify we got phase transition events
        let has_mine = events.iter().any(|e| matches!(e, CoordinatorEvent::MinePhaseStarted { .. }));
        let has_commit = events.iter().any(|e| matches!(e, CoordinatorEvent::CommitPhaseStarted { .. }));

        assert!(has_mine, "should have received MinePhaseStarted");
        assert!(has_commit, "should have received CommitPhaseStarted");

        drop(cmd_tx);
    }

    #[tokio::test]
    async fn test_peer_management() {
        let our_id = test_node_id(1);
        let config = CoordinatorConfig {
            salt_duration: Duration::from_secs(60), // Long so we stay in Salt
            ..CoordinatorConfig::default()
        };

        let (coordinator, state) = EpochCoordinator::new(
            our_id,
            config,
            0,
            Hash::from_bytes([0; 32]),
        );

        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (event_tx, _event_rx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            coordinator.run(cmd_rx, event_tx).await;
        });

        // Add peers
        cmd_tx.send(CoordinatorCommand::PeerJoined { node_id: test_node_id(2) }).unwrap();
        cmd_tx.send(CoordinatorCommand::PeerJoined { node_id: test_node_id(3) }).unwrap();

        tokio::time::sleep(Duration::from_millis(50)).await;

        let s = state.read().await;
        // our_id + 2 peers = 3
        assert_eq!(s.peer_count, 3);
        drop(s);

        // Remove a peer
        cmd_tx.send(CoordinatorCommand::PeerLeft { node_id: test_node_id(3) }).unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;

        let s = state.read().await;
        assert_eq!(s.peer_count, 2);

        drop(cmd_tx);
    }

    #[tokio::test]
    async fn test_commit_collection() {
        let our_id = test_node_id(1);
        let config = CoordinatorConfig {
            salt_duration: Duration::from_millis(10),
            mine_duration: Duration::from_millis(10),
            commit_duration: Duration::from_millis(200), // Long commit phase
            seal_duration: Duration::from_millis(10),
            stall_timeout: Duration::from_secs(5),
            quorum_threshold: 0.5,
            ..CoordinatorConfig::default()
        };

        let (coordinator, state) = EpochCoordinator::new(
            our_id,
            config,
            0,
            Hash::from_bytes([0; 32]),
        );

        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();

        // Add a peer so quorum math works (2 peers, need 1 commit for 50%)
        cmd_tx.send(CoordinatorCommand::PeerJoined { node_id: test_node_id(2) }).unwrap();

        tokio::spawn(async move {
            coordinator.run(cmd_rx, event_tx).await;
        });

        // Wait for commit phase
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Submit our solution (with dummy problem/solution for the new fields)
        cmd_tx.send(CoordinatorCommand::LocalSolutionReady {
            epoch: 1,
            solution_hash: [0xAA; 32],
            work_score: 150.0,
            problem: ProblemType::SubsetSum { numbers: vec![1, 2, 3, 4, 5], target: 9 },
            solution: Solution::SubsetSum(vec![3, 4]),  // 4+5=9
            solve_time: Duration::from_millis(100),
        }).unwrap();

        // Submit peer's commit
        cmd_tx.send(CoordinatorCommand::CommitReceived {
            epoch: 1,
            commit: SolutionCommit {
                node_id: test_node_id(2),
                solution_hash: [0xBB; 32],
                work_score: 100.0,
                signature: vec![],
            },
        }).unwrap();

        tokio::time::sleep(Duration::from_millis(50)).await;

        let s = state.read().await;
        assert_eq!(s.commit_count, 2);

        drop(cmd_tx);
    }

    #[tokio::test]
    async fn test_stall_recovery() {
        let our_id = test_node_id(1);
        let config = CoordinatorConfig {
            salt_duration: Duration::from_millis(10),
            mine_duration: Duration::from_millis(10),
            commit_duration: Duration::from_millis(10),
            seal_duration: Duration::from_millis(10),
            stall_timeout: Duration::from_millis(20),
            max_consecutive_stalls: 2,
            ..CoordinatorConfig::default()
        };

        let (coordinator, state) = EpochCoordinator::new(
            our_id,
            config,
            0,
            Hash::from_bytes([0; 32]),
        );

        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            coordinator.run(cmd_rx, event_tx).await;
        });

        // Collect events for 500ms — should see multiple epoch starts due to stalls
        let mut epoch_starts = Vec::new();
        let deadline = time::Instant::now() + Duration::from_millis(500);

        loop {
            match time::timeout_at(deadline, event_rx.recv()).await {
                Ok(Some(CoordinatorEvent::EpochStarted { epoch, .. })) => {
                    epoch_starts.push(epoch);
                }
                Ok(Some(_)) => {} // Other events
                _ => break,
            }
        }

        // Should have progressed through multiple epochs
        assert!(epoch_starts.len() >= 2, "should have started at least 2 epochs, got {}", epoch_starts.len());
        assert!(epoch_starts.contains(&1), "should have started epoch 1");

        drop(cmd_tx);
    }
}
