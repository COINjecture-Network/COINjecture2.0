// COINjecture Node Library
// Re-exports for external use (tests, etc.)
#![recursion_limit = "512"]

pub mod node_types;
pub mod node_manager;
pub mod config;
pub mod light_sync;
pub mod light_client;
pub mod metrics;
pub mod sync_optimizer;
pub mod faucet;
pub mod keystore;
pub mod metrics_integration;
pub mod metrics_server;
pub mod mobile_sdk;
pub mod peer_consensus;
pub mod service;

// Exposed for integration tests
pub mod chain;
#[cfg(feature = "adzdb")]
pub mod chain_adzdb;
pub mod genesis;
pub mod validator;
