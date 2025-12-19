// COINjecture P2P Networking
// CPP (COINjecture P2P Protocol) - PRIMARY NETWORK
// libp2p modules kept temporarily for compatibility (will be removed)

// Legacy libp2p modules (temporarily kept, will be removed after full CPP migration)
pub mod addr_filter;
pub mod protocol;
pub mod eclipse;
pub mod sync_guardrails;

// Active modules
pub mod reputation; // Used by CPP
pub mod cpp; // COINjecture P2P Protocol (CPP) - PRIMARY NETWORK

// Legacy exports (libp2p - temporarily kept for compatibility)
pub use addr_filter::{AddressFilterConfig, AddressFilterResult, validate_multiaddr, filter_multiaddrs_with_logging};
pub use protocol::*;
pub use eclipse::*;
pub use sync_guardrails::{SyncGuardrails, SyncGuardConfig, BackpressureMetrics};

// Active exports
pub use reputation::*; // Used by CPP
pub use cpp::PeerId as CppPeerId; // CPP PeerId type ([u8; 32]) - PRIMARY
pub use libp2p::PeerId; // Legacy libp2p PeerId (temporarily kept for compatibility)
