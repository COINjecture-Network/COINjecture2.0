// COINjecture Network B - Core Types
// Custom Layer 1 blockchain with NP-hard consensus

pub mod types;
pub mod crypto;
pub mod transaction;
pub mod block;
pub mod problem;
pub mod commitment;
pub mod dimensional;
pub mod privacy;
pub mod golden;
pub mod validation;

// Re-exports
pub use types::*;
pub use crypto::*;
pub use transaction::*;
pub use block::*;
pub use problem::*;
pub use commitment::*;
pub use dimensional::*;
pub use privacy::*;
pub use golden::*;
// validation is accessed as coinject_core::validation::<item> (not wildcard-re-exported)
