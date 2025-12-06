// COINjecture P2P Networking
// libp2p-based gossip and discovery with eclipse attack mitigation

pub mod protocol;
pub mod eclipse;

pub use protocol::*;
pub use eclipse::*;
pub use libp2p::PeerId;
