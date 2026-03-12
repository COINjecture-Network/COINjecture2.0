// =============================================================================
// Mesh Network Protocol Types
// =============================================================================
//
// Defines the Envelope, Payload, RoutingMode, and all message types used
// in the mesh P2P layer. Serialized with serde + bincode for compact wire format.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::net::SocketAddr;

use super::identity::NodeId;

/// Unique identifier for a committed solution from a node.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NodeCommit {
    pub node_id: NodeId,
    pub solution_hash: [u8; 32],
    pub work_score: f64,
    pub signature: Vec<u8>,
}

/// Determines how a message is routed through the mesh.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum RoutingMode {
    /// Flood to all connected peers (epidemic gossip).
    Broadcast,
    /// Route to a specific target node.
    Direct { target: NodeId },
}

/// The application-level message types carried by the mesh layer.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Payload {
    // --- Broadcast messages (gossip to all peers) ---
    /// A new salt value for an epoch's consensus round.
    ConsensusSalt { epoch: u64, salt: [u8; 32] },

    /// A solution submission for an NP-hard problem.
    Solution {
        epoch: u64,
        problem_id: String,
        solution_hash: [u8; 32],
        proof: Vec<u8>,
    },

    /// A block commit aggregating node solutions.
    Commit {
        epoch: u64,
        block_hash: [u8; 32],
        commits: Vec<NodeCommit>,
    },

    /// A bounty problem submitted to the network.
    BountySubmit {
        bounty_id: String,
        problem_type: String,
        payload: Vec<u8>,
    },

    /// Periodic heartbeat advertising node state.
    Heartbeat {
        epoch: u64,
        peer_count: u16,
        best_block: [u8; 32],
    },

    // --- Direct messages (routed to specific peer) ---
    /// Response to a bounty submission.
    BountyResult {
        bounty_id: String,
        accepted: bool,
        reward: u64,
        details: Vec<u8>,
    },

    /// Request a range of blocks from a peer.
    ChainSyncRequest {
        from_block: u64,
        to_block: Option<u64>,
    },

    /// Response with serialized blocks.
    ChainSyncResponse {
        blocks: Vec<Vec<u8>>,
        has_more: bool,
    },

    // --- Internal protocol messages ---
    /// Exchange known peer addresses for discovery.
    PeerExchange {
        known_peers: Vec<(NodeId, SocketAddr)>,
    },
}

impl fmt::Display for Payload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Payload::ConsensusSalt { epoch, .. } => write!(f, "ConsensusSalt(epoch={})", epoch),
            Payload::Solution { epoch, problem_id, .. } => {
                write!(f, "Solution(epoch={}, problem={})", epoch, problem_id)
            }
            Payload::Commit { epoch, .. } => write!(f, "Commit(epoch={})", epoch),
            Payload::BountySubmit { bounty_id, .. } => {
                write!(f, "BountySubmit(id={})", bounty_id)
            }
            Payload::Heartbeat { epoch, peer_count, .. } => {
                write!(f, "Heartbeat(epoch={}, peers={})", epoch, peer_count)
            }
            Payload::BountyResult { bounty_id, accepted, .. } => {
                write!(f, "BountyResult(id={}, ok={})", bounty_id, accepted)
            }
            Payload::ChainSyncRequest { from_block, to_block } => {
                write!(f, "ChainSyncReq(from={}, to={:?})", from_block, to_block)
            }
            Payload::ChainSyncResponse { blocks, has_more } => {
                write!(f, "ChainSyncResp(blocks={}, more={})", blocks.len(), has_more)
            }
            Payload::PeerExchange { known_peers } => {
                write!(f, "PeerExchange(peers={})", known_peers.len())
            }
        }
    }
}

/// The wire-level envelope wrapping every message on the mesh network.
///
/// Every message is identified by a unique msg_id, signed by the sender,
/// and carries a TTL to prevent infinite propagation.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Envelope {
    /// Unique message ID: SHA-256(sender_id || nonce || payload).
    pub msg_id: [u8; 32],
    /// Identity of the originating node.
    pub sender: NodeId,
    /// How this message should be routed.
    pub routing: RoutingMode,
    /// The application payload.
    pub payload: Payload,
    /// Ed25519 signature over (msg_id || routing || payload).
    pub signature: Vec<u8>,
    /// Time-to-live: decremented each hop, dropped at 0. Default 10.
    pub ttl: u8,
    /// Origination timestamp (Unix milliseconds).
    pub timestamp: u64,
}

/// Messages used during the initial handshake before a connection is authenticated.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum HandshakeMessage {
    /// Step 1: Initiator sends their identity and a random challenge.
    Hello {
        node_id: NodeId,
        public_key: Vec<u8>,
        challenge: [u8; 32],
        listen_addr: SocketAddr,
    },
    /// Step 2: Responder sends their identity, signs the received challenge,
    /// and issues their own challenge.
    HelloAck {
        node_id: NodeId,
        public_key: Vec<u8>,
        challenge_response: Vec<u8>,
        challenge: [u8; 32],
        listen_addr: SocketAddr,
    },
    /// Step 3: Initiator signs the responder's challenge, completing mutual auth.
    ChallengeResponse {
        challenge_response: Vec<u8>,
    },
}

/// Top-level wire message: either a handshake or an authenticated envelope.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum WireMessage {
    Handshake(HandshakeMessage),
    Envelope(Envelope),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_payload_serde_roundtrip() {
        let payloads = vec![
            Payload::ConsensusSalt {
                epoch: 42,
                salt: [0xAB; 32],
            },
            Payload::Solution {
                epoch: 1,
                problem_id: "tsp-100".into(),
                solution_hash: [0xCD; 32],
                proof: vec![1, 2, 3, 4],
            },
            Payload::Commit {
                epoch: 5,
                block_hash: [0xEF; 32],
                commits: vec![NodeCommit {
                    node_id: NodeId([0x11; 32]),
                    solution_hash: [0x22; 32],
                    work_score: 150.0,
                    signature: vec![0x33; 64],
                }],
            },
            Payload::BountySubmit {
                bounty_id: "bounty-1".into(),
                problem_type: "sat".into(),
                payload: vec![5, 6, 7],
            },
            Payload::Heartbeat {
                epoch: 10,
                peer_count: 5,
                best_block: [0xFF; 32],
            },
            Payload::BountyResult {
                bounty_id: "bounty-1".into(),
                accepted: true,
                reward: 1000,
                details: vec![],
            },
            Payload::ChainSyncRequest {
                from_block: 0,
                to_block: Some(100),
            },
            Payload::ChainSyncResponse {
                blocks: vec![vec![1, 2], vec![3, 4]],
                has_more: false,
            },
            Payload::PeerExchange {
                known_peers: vec![(
                    NodeId([0xAA; 32]),
                    "127.0.0.1:9000".parse().unwrap(),
                )],
            },
        ];

        for payload in payloads {
            let bytes = bincode::serialize(&payload).expect("serialize");
            let decoded: Payload = bincode::deserialize(&bytes).expect("deserialize");
            // Verify roundtrip by re-serializing
            let bytes2 = bincode::serialize(&decoded).expect("re-serialize");
            assert_eq!(bytes, bytes2, "roundtrip failed for {:?}", decoded);
        }
    }

    #[test]
    fn test_envelope_serde_roundtrip() {
        let envelope = Envelope {
            msg_id: [0x01; 32],
            sender: NodeId([0x02; 32]),
            routing: RoutingMode::Broadcast,
            payload: Payload::Heartbeat {
                epoch: 1,
                peer_count: 3,
                best_block: [0; 32],
            },
            signature: vec![0x03; 64],
            ttl: 10,
            timestamp: 1700000000000,
        };

        let bytes = bincode::serialize(&envelope).expect("serialize");
        let decoded: Envelope = bincode::deserialize(&bytes).expect("deserialize");
        assert_eq!(decoded.msg_id, envelope.msg_id);
        assert_eq!(decoded.sender, envelope.sender);
        assert_eq!(decoded.ttl, 10);
        assert_eq!(decoded.timestamp, 1700000000000);
    }

    #[test]
    fn test_wire_message_serde_roundtrip() {
        let handshake = WireMessage::Handshake(HandshakeMessage::Hello {
            node_id: NodeId([0xAA; 32]),
            public_key: vec![0xBB; 32],
            challenge: [0xCC; 32],
            listen_addr: "127.0.0.1:9000".parse().unwrap(),
        });

        let bytes = bincode::serialize(&handshake).expect("serialize");
        let decoded: WireMessage = bincode::deserialize(&bytes).expect("deserialize");
        match decoded {
            WireMessage::Handshake(HandshakeMessage::Hello { node_id, .. }) => {
                assert_eq!(node_id, NodeId([0xAA; 32]));
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_routing_mode_equality() {
        let target = NodeId([0xFF; 32]);
        assert_eq!(RoutingMode::Broadcast, RoutingMode::Broadcast);
        assert_eq!(
            RoutingMode::Direct { target },
            RoutingMode::Direct { target }
        );
        assert_ne!(
            RoutingMode::Broadcast,
            RoutingMode::Direct { target }
        );
    }
}
