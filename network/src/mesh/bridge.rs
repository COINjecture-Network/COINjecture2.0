// =============================================================================
// Mesh ↔ Node Bridge
// =============================================================================
//
// Translates between the mesh layer's generic Payload types and the node
// service's concrete Block/Transaction types. This bridge allows the node
// service to use the mesh layer as a drop-in transport alongside CPP.
//
// The bridge runs as an async task that:
// 1. Receives CppNetworkCommand variants from the node service
// 2. Translates them to mesh NetworkCommand + Payload
// 3. Receives mesh NetworkEvent messages
// 4. Translates them back to CppNetworkEvent for the node service

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::sync::{mpsc, RwLock};

use coinject_core::{Block, Hash, Transaction};

use super::identity::NodeId;
use super::protocol::{NodeCommit, Payload};
use super::{NetworkCommand as MeshCommand, NetworkEvent as MeshEvent};

// ─── Bridge Commands (node service → mesh) ───────────────────────────────────

/// Commands the node service sends to the mesh bridge.
/// Mirrors the CPP NetworkCommand interface so the node can use both transports.
#[derive(Debug, Clone)]
pub enum BridgeCommand {
    /// Broadcast a newly mined block to all mesh peers.
    BroadcastBlock { block: Block },

    /// Broadcast a new transaction.
    BroadcastTransaction { transaction: Transaction },

    /// Request blocks from a specific peer for chain sync.
    RequestBlocks {
        peer_id: NodeId,
        from_height: u64,
        to_height: u64,
        request_id: u64,
    },

    /// Update our advertised chain state (so heartbeats carry correct info).
    UpdateChainState { best_height: u64, best_hash: Hash },

    /// Connect to a seed/bootnode address.
    ConnectBootnode { addr: SocketAddr },

    /// Broadcast a consensus salt via mesh (coordinator → mesh).
    BroadcastConsensusSalt { epoch: u64, salt: [u8; 32] },

    /// Broadcast a solution commit via mesh (coordinator → mesh).
    BroadcastCommit {
        epoch: u64,
        solution_hash: [u8; 32],
        node_id: [u8; 32],
        work_score: f64,
        signature: Vec<u8>,
    },
}

// ─── Bridge Events (mesh → node service) ─────────────────────────────────────

/// Events the mesh bridge delivers to the node service.
/// Mirrors the CPP NetworkEvent interface.
#[derive(Debug, Clone)]
pub enum BridgeEvent {
    /// A mesh peer completed handshake and connected.
    PeerConnected {
        peer_id: NodeId,
        addr: SocketAddr,
        best_height: u64,
        best_hash: Hash,
    },

    /// A mesh peer disconnected.
    PeerDisconnected { peer_id: NodeId, reason: String },

    /// Status update from a peer (via heartbeat).
    StatusUpdate {
        peer_id: NodeId,
        best_height: u64,
        best_hash: Hash,
    },

    /// A block was received from a peer.
    BlockReceived { block: Block, peer_id: NodeId },

    /// A transaction was received from a peer.
    TransactionReceived {
        transaction: Transaction,
        peer_id: NodeId,
    },

    /// Blocks received in response to a sync request.
    BlocksReceived {
        blocks: Vec<Block>,
        request_id: u64,
        peer_id: NodeId,
    },

    /// Consensus salt received from a peer (forwarded to coordinator).
    ConsensusSaltReceived {
        epoch: u64,
        salt: [u8; 32],
        from: NodeId,
    },

    /// Solution commit received from a peer (forwarded to coordinator).
    ConsensusCommitReceived {
        epoch: u64,
        block_hash: [u8; 32],
        commits: Vec<NodeCommit>,
        from: NodeId,
    },
}

// ─── Bridge State ────────────────────────────────────────────────────────────

/// Shared mutable state for the bridge (chain tip for heartbeats).
pub struct BridgeState {
    pub best_height: u64,
    pub best_hash: Hash,
    pub epoch: u64,
}

// ─── Bridge Task ─────────────────────────────────────────────────────────────

/// Run the mesh bridge as an async task.
///
/// This sits between the node service and the mesh NetworkService, translating
/// commands and events. It handles:
/// - Serializing blocks/transactions into mesh Payload bytes
/// - Deserializing received payloads back to typed objects
/// - Mapping mesh heartbeats to status updates
/// - Tracking pending sync requests for response correlation
/// - Forwarding consensus payloads (salt, commits) to the node service
pub async fn run_bridge(
    // Node service channels
    mut cmd_rx: mpsc::UnboundedReceiver<BridgeCommand>,
    event_tx: mpsc::UnboundedSender<BridgeEvent>,
    // Mesh network channels
    mesh_cmd_tx: mpsc::UnboundedSender<MeshCommand>,
    mut mesh_event_rx: mpsc::UnboundedReceiver<MeshEvent>,
    // Shared state
    state: Arc<RwLock<BridgeState>>,
) {
    // Track pending sync requests: request_id → (from_height, to_height)
    let mut pending_syncs: HashMap<u64, (u64, u64)> = HashMap::new();

    loop {
        tokio::select! {
            // ── Node service → Mesh ──────────────────────────────────────
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(BridgeCommand::BroadcastBlock { block }) => {
                        let block_bytes = match bincode::serialize(&block) {
                            Ok(b) => b,
                            Err(e) => {
                                tracing::error!(error = %e, "failed to serialize block for mesh broadcast");
                                continue;
                            }
                        };
                        let payload = Payload::Solution {
                            epoch: state.read().await.epoch,
                            problem_id: format!("block-{}", block.header.height),
                            solution_hash: *block.header.hash().as_bytes(),
                            proof: block_bytes,
                        };
                        let _ = mesh_cmd_tx.send(MeshCommand::Broadcast(payload));
                    }

                    Some(BridgeCommand::BroadcastTransaction { transaction }) => {
                        let tx_bytes = match bincode::serialize(&transaction) {
                            Ok(b) => b,
                            Err(e) => {
                                tracing::error!(error = %e, "failed to serialize tx for mesh broadcast");
                                continue;
                            }
                        };
                        let payload = Payload::BountySubmit {
                            bounty_id: format!("tx-{}", hex::encode(&transaction.hash().as_bytes()[..8])),
                            problem_type: "transaction".into(),
                            payload: tx_bytes,
                        };
                        let _ = mesh_cmd_tx.send(MeshCommand::Broadcast(payload));
                    }

                    Some(BridgeCommand::RequestBlocks { peer_id, from_height, to_height, request_id }) => {
                        pending_syncs.insert(request_id, (from_height, to_height));
                        let payload = Payload::ChainSyncRequest {
                            from_block: from_height,
                            to_block: Some(to_height),
                        };
                        let _ = mesh_cmd_tx.send(MeshCommand::SendDirect {
                            target: peer_id,
                            payload,
                        });
                    }

                    Some(BridgeCommand::UpdateChainState { best_height, best_hash }) => {
                        let mut s = state.write().await;
                        s.best_height = best_height;
                        s.best_hash = best_hash;
                        // Heartbeats automatically pick up the updated state
                    }

                    Some(BridgeCommand::ConnectBootnode { addr }) => {
                        tracing::info!(addr = %addr, "bridge: bootnode connect requested (handled by mesh seed config)");
                    }

                    Some(BridgeCommand::BroadcastConsensusSalt { epoch, salt }) => {
                        tracing::debug!(epoch, "bridge: broadcasting consensus salt");
                        let payload = Payload::ConsensusSalt { epoch, salt };
                        let _ = mesh_cmd_tx.send(MeshCommand::Broadcast(payload));
                    }

                    Some(BridgeCommand::BroadcastCommit { epoch, solution_hash, node_id, work_score, signature }) => {
                        tracing::debug!(epoch, "bridge: broadcasting solution commit");
                        let payload = Payload::Commit {
                            epoch,
                            block_hash: solution_hash,
                            commits: vec![NodeCommit {
                                node_id: NodeId(node_id),
                                solution_hash,
                                work_score,
                                signature,
                            }],
                        };
                        let _ = mesh_cmd_tx.send(MeshCommand::Broadcast(payload));
                    }

                    None => {
                        tracing::info!("bridge: command channel closed, shutting down");
                        break;
                    }
                }
            }

            // ── Mesh → Node service ──────────────────────────────────────
            event = mesh_event_rx.recv() => {
                match event {
                    Some(MeshEvent::PeerConnected(peer_id)) => {
                        let s = state.read().await;
                        let _ = event_tx.send(BridgeEvent::PeerConnected {
                            peer_id,
                            addr: "0.0.0.0:0".parse().expect("static addr literal always parses"), // Will be enriched when mesh exposes addr
                            best_height: s.best_height,
                            best_hash: s.best_hash,
                        });
                    }

                    Some(MeshEvent::PeerDisconnected(peer_id)) => {
                        let _ = event_tx.send(BridgeEvent::PeerDisconnected {
                            peer_id,
                            reason: "mesh peer disconnected".into(),
                        });
                    }

                    Some(MeshEvent::MessageReceived { from, payload, .. }) => {
                        match payload {
                            // Block broadcast (encoded as Solution)
                            Payload::Solution { proof, .. } => {
                                match bincode::deserialize::<Block>(&proof) {
                                    Ok(block) => {
                                        let _ = event_tx.send(BridgeEvent::BlockReceived {
                                            block,
                                            peer_id: from,
                                        });
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            peer = %from.short(),
                                            error = %e,
                                            "failed to deserialize block from mesh"
                                        );
                                    }
                                }
                            }

                            // Transaction broadcast (encoded as BountySubmit)
                            Payload::BountySubmit { problem_type, payload: tx_bytes, .. }
                                if problem_type == "transaction" =>
                            {
                                match bincode::deserialize::<Transaction>(&tx_bytes) {
                                    Ok(tx) => {
                                        let _ = event_tx.send(BridgeEvent::TransactionReceived {
                                            transaction: tx,
                                            peer_id: from,
                                        });
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            peer = %from.short(),
                                            error = %e,
                                            "failed to deserialize tx from mesh"
                                        );
                                    }
                                }
                            }

                            // Heartbeat → StatusUpdate
                            Payload::Heartbeat { epoch, peer_count: _, best_block } => {
                                let _ = event_tx.send(BridgeEvent::StatusUpdate {
                                    peer_id: from,
                                    best_height: epoch, // Epoch maps to height in heartbeats
                                    best_hash: Hash::from_bytes(best_block),
                                });
                            }

                            // Chain sync response
                            Payload::ChainSyncResponse { blocks: block_bytes, has_more: _ } => {
                                let mut blocks = Vec::new();
                                for bytes in &block_bytes {
                                    match bincode::deserialize::<Block>(bytes) {
                                        Ok(b) => blocks.push(b),
                                        Err(e) => {
                                            tracing::warn!(error = %e, "bad block in sync response");
                                        }
                                    }
                                }
                                if !blocks.is_empty() {
                                    // Find the matching request_id (simple: use first pending)
                                    let request_id = pending_syncs
                                        .keys()
                                        .next()
                                        .copied()
                                        .unwrap_or(0);
                                    pending_syncs.remove(&request_id);

                                    let _ = event_tx.send(BridgeEvent::BlocksReceived {
                                        blocks,
                                        request_id,
                                        peer_id: from,
                                    });
                                }
                            }

                            // Chain sync request (we need to respond)
                            Payload::ChainSyncRequest { from_block, to_block } => {
                                tracing::debug!(
                                    peer = %from.short(),
                                    from = from_block,
                                    to = ?to_block,
                                    "received chain sync request (bridge doesn't serve blocks yet)"
                                );
                                // TODO: Wire to BlockProvider to serve blocks
                                // For now, the CPP layer handles block serving.
                            }

                            // Consensus salt → forward to service/coordinator
                            Payload::ConsensusSalt { epoch, salt } => {
                                tracing::debug!(
                                    epoch,
                                    peer = %from.short(),
                                    "bridge: forwarding consensus salt to service"
                                );
                                let _ = event_tx.send(BridgeEvent::ConsensusSaltReceived {
                                    epoch,
                                    salt,
                                    from,
                                });
                            }

                            // Commit → forward to service/coordinator
                            Payload::Commit { epoch, block_hash, commits } => {
                                tracing::debug!(
                                    epoch,
                                    peer = %from.short(),
                                    commit_count = commits.len(),
                                    "bridge: forwarding commit to service"
                                );
                                let _ = event_tx.send(BridgeEvent::ConsensusCommitReceived {
                                    epoch,
                                    block_hash,
                                    commits,
                                    from,
                                });
                            }

                            // Peer exchange is handled internally by mesh layer
                            Payload::PeerExchange { .. } => {}

                            // Other payloads
                            other => {
                                tracing::debug!(payload = %other, "unhandled mesh payload type");
                            }
                        }
                    }

                    Some(MeshEvent::PeerList(_)) => {
                        // Internal response to GetPeers, not forwarded to node
                    }

                    None => {
                        tracing::info!("bridge: mesh event channel closed");
                        break;
                    }
                }
            }
        }
    }

    tracing::info!("mesh bridge task exited");
}
