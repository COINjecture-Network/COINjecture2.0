// COINjecture State Management
// Account-based state with advanced transaction types

// State functions return rich error enums via redb — boxing would add overhead
// and the errors are consumed immediately at call sites.
#![allow(clippy::result_large_err)]
// State functions require redb handles, state refs, and transaction data together —
// builders would add boilerplate without benefit in this internal API.
#![allow(clippy::too_many_arguments)]
// Iterator patterns that silently skip errors are intentional — missing or
// corrupt state entries should not crash the node.
#![allow(clippy::manual_flatten)]

// Account state: conditionally uses ADZDB when compiled with --features adzdb
#[cfg(not(feature = "adzdb"))]
pub mod accounts;
#[cfg(feature = "adzdb")]
pub mod accounts_adzdb;

pub mod channels;
pub mod dimensional_pools;
pub mod escrows;
pub mod marketplace;
pub mod timelocks;
pub mod trustlines;

// Re-export account state (uses ADZDB version when feature is enabled)
#[cfg(not(feature = "adzdb"))]
pub use accounts::*;
#[cfg(feature = "adzdb")]
pub use accounts_adzdb::{AdzdbAccountState as AccountState, StateError};

pub use channels::*;
pub use dimensional_pools::*;
pub use escrows::*;
pub use marketplace::*;
pub use timelocks::*;
pub use trustlines::*;
