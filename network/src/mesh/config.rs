// =============================================================================
// Mesh Network Configuration
// =============================================================================
//
// All tunable parameters for the mesh networking layer. Sensible defaults
// provided; override via NetworkConfig builder or CLI flags.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

/// Configuration for the mesh networking layer.
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    /// Address to listen for incoming TCP connections.
    pub listen_addr: SocketAddr,

    /// Bootstrap seed nodes to connect to on startup.
    pub seed_nodes: Vec<SocketAddr>,

    /// Directory for persistent data (keypair file).
    pub data_dir: PathBuf,

    /// Interval between heartbeat messages sent to each peer.
    pub heartbeat_interval: Duration,

    /// Number of consecutive missed heartbeats before declaring a peer dead.
    pub max_missed_heartbeats: u32,

    /// Base delay for exponential backoff on reconnection attempts.
    pub reconnect_base_delay: Duration,

    /// Maximum delay cap for exponential backoff.
    pub reconnect_max_delay: Duration,

    /// Default TTL for broadcast messages (decremented each hop).
    pub default_ttl: u8,

    /// Capacity of the seen-message LRU dedup cache.
    pub dedup_cache_capacity: usize,

    /// How long a message ID stays in the dedup cache before eviction.
    pub dedup_cache_ttl: Duration,

    /// Maximum message payload size in bytes (framing layer rejects larger).
    pub max_message_size: usize,

    /// Maximum inbound messages per second per peer before rate limiting.
    pub max_messages_per_second_per_peer: u32,

    /// Interval between peer exchange rounds.
    pub peer_exchange_interval: Duration,

    /// Handshake timeout — how long to wait for a handshake to complete.
    pub handshake_timeout: Duration,

    /// Connection timeout — how long to wait for TCP connect.
    pub connect_timeout: Duration,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            listen_addr: "0.0.0.0:9000".parse().expect("valid default addr"),
            seed_nodes: Vec::new(),
            data_dir: PathBuf::from("./data"),
            heartbeat_interval: Duration::from_secs(10),
            max_missed_heartbeats: 3,
            reconnect_base_delay: Duration::from_secs(1),
            reconnect_max_delay: Duration::from_secs(60),
            default_ttl: 10,
            dedup_cache_capacity: 100_000,
            dedup_cache_ttl: Duration::from_secs(300),
            max_message_size: 16 * 1024 * 1024, // 16 MB
            max_messages_per_second_per_peer: 100,
            peer_exchange_interval: Duration::from_secs(30),
            handshake_timeout: Duration::from_secs(10),
            connect_timeout: Duration::from_secs(5),
        }
    }
}

impl NetworkConfig {
    /// Create a config suitable for integration tests (fast timeouts, localhost).
    #[cfg(test)]
    pub fn test_config(port: u16) -> Self {
        Self {
            listen_addr: SocketAddr::from(([127, 0, 0, 1], port)),
            seed_nodes: Vec::new(),
            data_dir: PathBuf::from(format!("/tmp/coinject-test-{}", port)),
            heartbeat_interval: Duration::from_secs(2),
            max_missed_heartbeats: 3,
            reconnect_base_delay: Duration::from_millis(100),
            reconnect_max_delay: Duration::from_secs(2),
            default_ttl: 10,
            dedup_cache_capacity: 1_000,
            dedup_cache_ttl: Duration::from_secs(60),
            max_message_size: 16 * 1024 * 1024,
            max_messages_per_second_per_peer: 1000,
            peer_exchange_interval: Duration::from_secs(5),
            handshake_timeout: Duration::from_secs(3),
            connect_timeout: Duration::from_secs(2),
        }
    }
}
