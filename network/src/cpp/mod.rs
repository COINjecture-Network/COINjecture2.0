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
