// =============================================================================
// COINjecture P2P Protocol (CPP)
// =============================================================================
// Custom P2P protocol designed to replace libp2p with:
// - Equilibrium-based flow control (η = λ = 1/√2 ≈ 0.7071)
// - Dimensional message priorities
// - RPC-integrated light client mining
// - Simple, debuggable architecture

pub mod config;
pub mod message;
pub mod flow_control;
pub mod router;
pub mod protocol;
pub mod peer;
pub mod node_integration;
pub mod network;
pub mod flock;

// Re-export commonly used types
pub use config::{
    CppConfig, NodeType, CPP_PORT, WEBSOCKET_PORT, 
    DEFAULT_P2P_LISTEN, DEFAULT_WS_LISTEN, ETA, SQRT_2
};

pub use message::{
    MessageType, MessagePriority,
    HelloMessage, HelloAckMessage, StatusMessage,
    GetBlocksMessage, BlocksMessage,
    GetHeadersMessage, HeadersMessage,
    NewBlockMessage, NewTransactionMessage,
    SubmitWorkMessage, WorkAcceptedMessage, WorkRejectedMessage,
    GetWorkMessage, WorkMessage,
    PingMessage, PongMessage, DisconnectMessage,
};

pub use flow_control::{FlowControl, FlowControlStats};

pub use router::{EquilibriumRouter, PeerInfo, PeerId};

pub use protocol::{MessageEnvelope, MessageCodec, ProtocolError};

pub use peer::{Peer, PeerState, PeerStats};

pub use node_integration::{NodeMetrics, PeerSelector, thresholds};

pub use network::{CppNetwork, NetworkEvent, NetworkCommand};
pub mod block_provider;

pub use block_provider::{BlockProvider, EmptyBlockProvider, MAX_BLOCKS_PER_REQUEST};

pub use flock::{
    GoldenGenerator, FlockState, FlockStateCompact, MurmurationRules,
    PHI, PHI_INV, GOLDEN_SEED, FLOCK_EPOCH_BLOCKS, FLOCK_PHASES,
};
