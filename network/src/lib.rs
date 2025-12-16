// COINjecture P2P Networking
// libp2p-based gossip and discovery with eclipse attack mitigation

pub mod addr_filter;
pub mod protocol;
pub mod eclipse;
pub mod reputation;

pub use addr_filter::{AddressFilterConfig, AddressFilterResult, validate_multiaddr, filter_multiaddrs_with_logging};
pub use protocol::*;
pub use eclipse::*;
pub use reputation::*;
pub use libp2p::PeerId;

// Re-export request-response types for service layer
pub use protocol::{SyncRequest, SyncResponse, SyncCodec};
