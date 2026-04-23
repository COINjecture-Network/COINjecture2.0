// =============================================================================
// COINjecture P2P Networking
// CPP (COINjecture P2P Protocol) - The sole networking protocol
// =============================================================================
//
// libp2p has been fully removed. CPP is a lightweight, blockchain-optimized
// protocol inspired by XRPL's peer protocol design.

// Active modules
pub mod cpp; // COINjecture P2P Protocol (CPP)
pub mod discovery; // Cascading peer discovery (DB → DNS → hardcoded → manual)
pub mod encrypted_connection;
pub mod mesh; // P2P Mesh Networking Layer (discovery, gossip, direct messaging)
pub mod noise; // Noise_XX encrypted transport via snow crate
pub mod noise_identity; // Peer identity verification for Noise connections
pub mod peer_scoring; // Ban-score + reputation scoring
pub mod peer_store; // Persistent peer database with vetted/unvetted buckets
pub mod pex; // PEX (Peer Exchange) reactor for automatic peer discovery
pub mod reputation; // Peer reputation tracking
pub mod security; // Network security primitives (Phase 5) // Dual-mode connection wrapper (Noise or legacy)

// Core exports
pub use cpp::config::NodeType;
pub use cpp::PeerId;
pub use reputation::*;

// Mesh layer exports
pub use mesh::bridge::{
    BridgeCommand as MeshBridgeCommand, BridgeEvent as MeshBridgeEvent,
    BridgeState as MeshBridgeState,
};
pub use mesh::config::NetworkConfig as MeshNetworkConfig;
pub use mesh::identity::NodeId as MeshNodeId;
pub use mesh::{NetworkCommand, NetworkEvent, NetworkService};

// Backwards compatibility alias (for code that used NetworkNodeType)
pub type NetworkNodeType = cpp::config::NodeType;
