// COINjecture P2P Networking
// libp2p-based gossip and discovery with eclipse attack mitigation

pub mod addr_filter;
pub mod protocol;
pub mod eclipse;
pub mod reputation;
pub mod sync_guardrails;
pub mod cpp; // COINjecture P2P Protocol (CPP)

pub use addr_filter::{AddressFilterConfig, AddressFilterResult, validate_multiaddr, filter_multiaddrs_with_logging};
pub use protocol::*;
pub use eclipse::*;
pub use reputation::*;
pub use sync_guardrails::{SyncGuardrails, SyncGuardConfig, BackpressureMetrics};
pub use libp2p::PeerId;
