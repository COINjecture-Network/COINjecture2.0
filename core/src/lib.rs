//! # coinject-core
//!
//! Primitive types, cryptography, and data structures for the COINjecture 2.0
//! WEB4 blockchain protocol.
//!
//! This crate is the foundation of the workspace — it has **no internal
//! dependencies** on other COINjecture crates. Every other crate depends on
//! `coinject-core`.
//!
//! ## Modules
//!
//! | Module | Contents |
//! |--------|----------|
//! | [`types`] | Fundamental types: [`Hash`], [`Address`], [`Balance`], etc. |
//! | [`crypto`] | Ed25519 key pairs, signatures, Merkle trees |
//! | [`transaction`] | All 7 transaction variants including [`MarketplaceTransaction`] |
//! | [`block`] | [`Block`], [`BlockHeader`], [`Blockchain`] |
//! | [`problem`] | NP-hard [`ProblemType`]s (SubsetSum, SAT, TSP) and [`Solution`] |
//! | [`commitment`] | Commit-reveal scheme for PoUW solution hiding |
//! | [`dimensional`] | Dimensional math: η = λ = 1/√2, D_n = e^(-η·τ_n) |
//! | [`privacy`] | Privacy-preserving transaction extensions |
//! | [`golden`] | GoldenSeed streams derived from the genesis hash |
//!
//! ## Key Constants
//!
//! - [`ETA`] — The Satoshi constant η = 1/√2 ≈ 0.7071 (damping coefficient)
//! - [`LAMBDA`] — λ = 1/√2 (phase evolution rate; equal to η by unit circle constraint)
//! - [`BLOCK_VERSION_GOLDEN`] — Block version 2, using GoldenSeed-enhanced hashing
//!
//! ## Example: Create and sign a transfer
//!
//! ```rust
//! use coinject_core::{KeyPair, Address, Transaction};
//!
//! let keypair = KeyPair::generate();
//! let sender = keypair.address();
//! let recipient = Address::from_bytes([1u8; 32]);
//!
//! let tx = Transaction::new_transfer(sender, recipient, 1000, 10, 1, &keypair);
//! assert!(tx.verify_signature());
//! ```

pub mod block;
pub mod commitment;
pub mod crypto;
pub mod dimensional;
pub mod golden;
pub mod privacy;
pub mod problem;
pub mod transaction;
pub mod types;

// Re-exports — all public items available at the crate root
pub use block::*;
pub use commitment::*;
pub use crypto::*;
pub use dimensional::*;
pub use golden::*;
pub use privacy::*;
pub use problem::*;
pub use transaction::*;
pub use types::*;
