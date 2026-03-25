// COINjecture Network B - Core Types
// Custom Layer 1 blockchain with NP-hard consensus

pub mod error;
pub mod types;
pub mod crypto;
pub mod transaction;
pub mod block;
pub mod problem;
pub mod commitment;
pub mod dimensional;
pub mod privacy;
pub mod golden;
/// Deterministic fixed-point integer arithmetic for consensus-critical paths.
/// Replaces f64 in any code that must agree across heterogeneous validator
/// platforms (ARM, x86, RISC-V) without floating-point non-determinism.
pub mod fixed_point;
pub mod validation;

pub use error::{
    unix_now_secs, unix_now_secs_i64,
    CryptoError, BlockError, TransactionError, ConsensusError, NetworkError, StateError, ConfigError,
};

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
