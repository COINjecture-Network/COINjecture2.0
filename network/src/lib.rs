// =============================================================================
// COINjecture P2P Networking
// CPP (COINjecture P2P Protocol) - The sole networking protocol
// =============================================================================

// CPP network functions require many handles (tx channels, peer maps, config,
// state) — collapsing to a builder would obscure the spawn-time wiring.
#![allow(clippy::too_many_arguments)]
// CPP uses Arc<Mutex<HashMap<PeerId, Sender<...>>>> and similar compound types
// that are fundamental to the actor-model peer management design.
#![allow(clippy::type_complexity)]
// Many callbacks passed to tokio::spawn use explicit closures for clarity
// at call sites, rather than bare function pointers.
#![allow(clippy::redundant_closure)]
//
// libp2p has been fully removed. CPP is a lightweight, blockchain-optimized
// protocol inspired by XRPL's peer protocol design.

// Active modules
pub mod cpp; // COINjecture P2P Protocol (CPP)
pub mod mesh;
pub mod reputation; // Peer reputation tracking // P2P Mesh Networking Layer (discovery, gossip, direct messaging)

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
