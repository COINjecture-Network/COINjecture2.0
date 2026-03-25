// COINjecture Node Library
// Re-exports for external use (tests, etc.)

// HeaderServer stores heterogeneous callbacks in Box<dyn Fn> — type aliases would add noise.
#![allow(clippy::type_complexity)]
// ChainError wraps redb errors that are inherently large (160 bytes); boxing would require
// propagating Box<ChainError> throughout the codebase.
#![allow(clippy::result_large_err)]
// Service/fork/mining functions require many Arc<RwLock<...>> parameters; no logical grouping.
#![allow(clippy::too_many_arguments)]

pub mod config;
pub mod light_client;
pub mod light_sync;
pub mod metrics;
pub mod node_manager;
pub mod node_types;
