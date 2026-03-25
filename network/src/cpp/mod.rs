// =============================================================================
// COINjecture P2P Protocol (CPP)
// =============================================================================
// Custom P2P protocol designed to replace libp2p with:
// - Equilibrium-based flow control (η = λ = 1/√2 ≈ 0.7071)
// - Dimensional message priorities
// - RPC-integrated light client mining
// - Simple, debuggable architecture
//
// NOTE: Some protocol components are prepared for future protocol extensions
#![allow(dead_code)]

pub mod config;
pub mod flock;
pub mod flow_control;
pub mod message;
pub mod network;
pub mod node_integration;
pub mod peer;
pub mod protocol;
pub mod router;

// Re-export commonly used types
pub use config::{
    timeouts, // Unified timeout constants for network/consensus alignment
    CppConfig,
    NodeType,
    CPP_PORT,
    DEFAULT_P2P_LISTEN,
    DEFAULT_WS_LISTEN,
    ETA,
    SQRT_2,
    WEBSOCKET_PORT,
};

pub use message::{
    BlocksMessage, DisconnectMessage, GetBlocksMessage, GetHeadersMessage, GetWorkMessage,
    HeadersMessage, HelloAckMessage, HelloMessage, MessagePriority, MessageType, NewBlockMessage,
    NewTransactionMessage, PingMessage, PongMessage, StatusMessage, SubmitWorkMessage,
    WorkAcceptedMessage, WorkMessage, WorkRejectedMessage,
};

pub use flow_control::{FlowControl, FlowControlStats};

pub use router::{EquilibriumRouter, PeerId, PeerInfo};

pub use protocol::{MessageCodec, MessageEnvelope, ProtocolError};

pub use peer::{Peer, PeerState, PeerStats};

pub use node_integration::{thresholds, NodeMetrics, PeerSelector};

pub use network::{CppNetwork, NetworkCommand, NetworkEvent};
pub mod block_provider;

pub use block_provider::{BlockProvider, EmptyBlockProvider, MAX_BLOCKS_PER_REQUEST};

pub use flock::{
    FlockState, FlockStateCompact, GoldenGenerator, MurmurationRules, FLOCK_EPOCH_BLOCKS,
    FLOCK_PHASES, GOLDEN_SEED, PHI, PHI_INV,
};
