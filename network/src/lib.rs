// =============================================================================
// COINjecture P2P Networking
// CPP (COINjecture P2P Protocol) - The sole networking protocol
// =============================================================================
//
// libp2p has been fully removed. CPP is a lightweight, blockchain-optimized
// protocol inspired by XRPL's peer protocol design.

// Active modules
pub mod reputation; // Peer reputation tracking
pub mod cpp;        // COINjecture P2P Protocol (CPP)

// Core exports
pub use reputation::*;
pub use cpp::PeerId;
pub use cpp::config::NodeType;

// Backwards compatibility alias (for code that used NetworkNodeType)
pub type NetworkNodeType = cpp::config::NodeType;
