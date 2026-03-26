// COINjecture Network B - Core Types
// Custom Layer 1 blockchain with NP-hard consensus

pub mod block;
pub mod commitment;
pub mod crypto;
pub mod dimensional;
pub mod error;
/// Deterministic fixed-point integer arithmetic for consensus-critical paths.
/// Replaces f64 in any code that must agree across heterogeneous validator
/// platforms (ARM, x86, RISC-V) without floating-point non-determinism.
pub mod fixed_point;
pub mod golden;
pub mod privacy;
pub mod problem;
pub mod transaction;
pub mod types;
pub mod validation;

pub use error::{
    unix_now_secs, unix_now_secs_i64, BlockError, ConfigError, ConsensusError, CryptoError,
    NetworkError, StateError, TransactionError,
};

// Re-exports
pub use block::*;
pub use commitment::*;
pub use crypto::*;
pub use dimensional::*;
pub use golden::*;
pub use privacy::*;
pub use problem::*;
pub use transaction::*;
pub use types::*;
// validation is accessed as coinject_core::validation::<item> (not wildcard-re-exported)
