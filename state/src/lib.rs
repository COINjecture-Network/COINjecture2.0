// COINjecture State Management
// Account-based state with advanced transaction types

// Account state: conditionally uses ADZDB when compiled with --features adzdb
#[cfg(not(feature = "adzdb"))]
pub mod accounts;
#[cfg(feature = "adzdb")]
pub mod accounts_adzdb;

pub mod timelocks;
pub mod escrows;
pub mod channels;
pub mod trustlines;
pub mod dimensional_pools;
pub mod marketplace;

// Re-export account state (uses ADZDB version when feature is enabled)
#[cfg(not(feature = "adzdb"))]
pub use accounts::*;
#[cfg(feature = "adzdb")]
pub use accounts_adzdb::{AdzdbAccountState as AccountState, StateError};

pub use timelocks::*;
pub use escrows::*;
pub use channels::*;
pub use trustlines::*;
pub use dimensional_pools::*;
pub use marketplace::*;
