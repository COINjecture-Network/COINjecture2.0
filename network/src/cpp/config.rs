// =============================================================================
// COINjecture P2P Protocol (CPP) - Configuration
// =============================================================================
// Port configuration based on the equilibrium constant η = λ = 1/√2 ≈ 0.7071

use std::time::Duration;

/// COINjecture P2P port: 707 (η × 1000 ≈ 707)
///
/// Named after the dimensionless equilibrium constant that governs
/// critical damping in complex systems. This port is used for:
/// - Block propagation between full nodes
/// - Transaction broadcasting
/// - Peer discovery and status updates
/// - Blockchain synchronization
pub const CPP_PORT: u16 = 707;

/// WebSocket RPC port: 8080 (standard HTTP alternative)
///
/// Standard port for WebSocket connections, used for:
/// - Light client mining (browser-based)
/// - Wallet interactions
/// - RPC queries
/// - Real-time block notifications
pub const WEBSOCKET_PORT: u16 = 8080;

/// Default P2P listen address
pub const DEFAULT_P2P_LISTEN: &str = "0.0.0.0:707";

/// Default WebSocket listen address
pub const DEFAULT_WS_LISTEN: &str = "0.0.0.0:8080";

/// Magic bytes for CPP protocol messages
/// "COIN" in ASCII: 0x43 0x4F 0x49 0x4E
pub const MAGIC: [u8; 4] = *b"COIN";

/// Protocol version (wire-level header byte for outbound messages).
/// Matches `version::CURRENT_PROTOCOL_VERSION`; kept here so the hot-path
/// encode path has a single const in scope without importing version.
pub const VERSION: u8 = 2;

/// Oldest protocol version accepted from remote peers.
/// Mirrors `version::MIN_SUPPORTED_VERSION`.
pub const MIN_PROTOCOL_VERSION: u8 = 1;

/// Maximum message size (legacy global cap — per-type limits in `security::MessageSizePolicy`
/// supersede this for known message types; this remains as a hard backstop for unknown types)
pub const MAX_MESSAGE_SIZE: usize = 4 * 1024 * 1024; // 4 MB (was 10 MB — reduced for DoS protection)

// ---------------------------------------------------------------------------
// Security constants (Phase 5 — Network Security)
// ---------------------------------------------------------------------------

/// Maximum inbound connections from a single IP address.
pub const SECURITY_MAX_CONNS_PER_IP: usize = 3;

/// Maximum total concurrent P2P connections (inbound + outbound).
pub const SECURITY_MAX_TOTAL_CONNECTIONS: usize = 128;

/// Maximum peers allowed from the same /16 subnet (eclipse attack protection).
pub const SECURITY_MAX_PEERS_PER_SUBNET: usize = 8;

/// Default ban duration for peers that send malformed/malicious messages.
pub const SECURITY_BAN_DURATION_SECS: u64 = 3600; // 1 hour

/// Short-ban duration for rate-limit offenders (less severe).
pub const SECURITY_SHORT_BAN_SECS: u64 = 300; // 5 minutes

/// Token-bucket capacity for per-peer rate limiting (burst allowance).
pub const SECURITY_RATE_BUCKET_CAPACITY: f64 = 200.0;

/// Token-bucket refill rate — sustained messages per second per peer.
pub const SECURITY_RATE_MSGS_PER_SEC: f64 = 50.0;

/// How many rate-limit strikes before a peer is short-banned.
pub const SECURITY_RATE_STRIKE_THRESHOLD: u32 = 10;

/// How many malformed message strikes before a peer is (long) banned.
pub const SECURITY_MALFORMED_STRIKE_THRESHOLD: u32 = 5;

/// Whether to require full encryption + mutual authentication for all peers.
/// When true, inbound connections are rejected unless `CppNetwork` is built with
/// a signing key (`with_signing_key`). The default node uses plain CPP Hello/HelloAck.
pub const SECURITY_REQUIRE_ENCRYPTION: bool = false;

/// Connection timeout
pub const CONNECTION_TIMEOUT: Duration = Duration::from_secs(30);

/// Handshake timeout (much faster than libp2p's Noise+Yamux)
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// Keep-alive interval (ping every 30 seconds)
pub const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(30);

/// Peer timeout (disconnect if no message for 90 seconds)
pub const PEER_TIMEOUT: Duration = Duration::from_secs(90);

/// Message read timeout (30 seconds - matches existing external usage)
/// Used by timeout-aware receive methods in MessageCodec
pub const MESSAGE_READ_TIMEOUT: Duration = Duration::from_secs(30);

/// Maximum consecutive read timeouts before forced disconnect
/// After this many consecutive timeouts, the peer is considered dead
pub const MAX_CONSECUTIVE_TIMEOUTS: u32 = 3;

/// Minimum healthy peer count before triggering reconnection attempts
/// Quality-based reconnection kicks in when healthy peers < this value
pub const MIN_HEALTHY_PEERS: usize = 2;

/// Peer quality threshold below which peer is considered unhealthy (0.0-1.0)
/// Peers with quality below this are not counted toward MIN_HEALTHY_PEERS
pub const PEER_QUALITY_THRESHOLD: f64 = 0.3;

/// Maximum number of peers
pub const MAX_PEERS: usize = 50;

/// Maximum number of pending connections
pub const MAX_PENDING_CONNECTIONS: usize = 10;

/// Maximum blocks per sync response (prevents large frame overflow)
/// Keeps message sizes predictable and prevents "early eof" on big syncs
pub const MAX_BLOCKS_PER_RESPONSE: u64 = 16;

/// Maximum frame bytes for sync responses (1 MB)
pub const MAX_SYNC_FRAME_BYTES: usize = 1_000_000;
/// Equilibrium constant: η = λ = 1/√2 ≈ 0.7071
///
/// This constant governs:
/// - Flow control window adaptation
/// - Message routing fanout
/// - Congestion control
/// - Sync chunk sizing
pub const ETA: f64 = std::f64::consts::FRAC_1_SQRT_2; // 0.7071067811865476

/// Square root of 2: √2 ≈ 1.414
///
/// Used for inverse calculations and dimensional scaling
pub const SQRT_2: f64 = std::f64::consts::SQRT_2; // 1.4142135623730951

// =============================================================================
// Unified Timeout Module - ETA-Derived Constants for Network/Consensus Alignment
// =============================================================================
//
// All timeouts are derived from the base NETWORK_PEER_TIMEOUT (90s) using the
// equilibrium constant η = 1/√2. This ensures consistent behavior across layers.
//
// Formulas:
//   - CONSENSUS_PEER_TIMEOUT = NETWORK / η ≈ 127s (consensus has more patience)
//   - CONSENSUS_STALE_THRESHOLD = NETWORK × (1 + η) ≈ 153s (for is_stale checks)
//
// Rationale:
//   - Network layer detects dead peers quickly (90s) for reconnection
//   - Consensus layer allows more time for sync/churn recovery
//   - The ratio η maintains dimensional consistency across the system

/// Unified timeout constants for network and consensus layer alignment
pub mod timeouts {
    use super::{ETA, PEER_TIMEOUT};
    use std::time::Duration;

    /// Base network peer timeout (90 seconds)
    /// This is the foundation for all derived timeouts
    pub const NETWORK_PEER_TIMEOUT: Duration = PEER_TIMEOUT;

    /// Network peer timeout in seconds (for consensus layer calculations)
    pub const NETWORK_PEER_TIMEOUT_SECS: u64 = 90;

    /// Consensus peer timeout: NETWORK / η ≈ 127 seconds
    /// The consensus layer allows more time before marking peers as stale,
    /// accounting for sync delays and connection churn recovery
    pub const CONSENSUS_PEER_TIMEOUT_SECS: f64 = NETWORK_PEER_TIMEOUT_SECS as f64 / ETA;

    /// Get consensus peer timeout as Duration
    pub fn consensus_peer_timeout() -> Duration {
        Duration::from_secs_f64(CONSENSUS_PEER_TIMEOUT_SECS)
    }

    /// Consensus stale threshold: NETWORK × (1 + η) ≈ 153 seconds
    /// Used by PeerState::is_stale() to determine when a peer is truly gone
    /// The (1 + η) factor provides a grace period beyond the network timeout
    pub const CONSENSUS_STALE_THRESHOLD_SECS: f64 = NETWORK_PEER_TIMEOUT_SECS as f64 * (1.0 + ETA);

    /// Get consensus stale threshold as Duration
    pub fn consensus_stale_threshold() -> Duration {
        Duration::from_secs_f64(CONSENSUS_STALE_THRESHOLD_SECS)
    }

    /// Maximum missed rounds before filtering (scaled by √2)
    /// Rounds = base × √2, where base = 3
    pub const MAX_MISSED_ROUNDS_CONSENSUS: u32 = 4; // ceil(3 × 1.414) = 4

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_timeout_ratios() {
            // Network timeout should be 90s
            assert_eq!(NETWORK_PEER_TIMEOUT_SECS, 90);

            // Consensus peer timeout should be ~127s (90 / 0.7071)
            assert!((CONSENSUS_PEER_TIMEOUT_SECS - 127.28).abs() < 0.1);

            // Consensus stale threshold should be ~153s (90 × 1.7071)
            assert!((CONSENSUS_STALE_THRESHOLD_SECS - 153.64).abs() < 0.1);

            // Consensus timeout > Network timeout (consensus is more patient)
            assert!(CONSENSUS_PEER_TIMEOUT_SECS > NETWORK_PEER_TIMEOUT_SECS as f64);

            // Stale threshold > Consensus timeout (final cutoff)
            assert!(CONSENSUS_STALE_THRESHOLD_SECS > CONSENSUS_PEER_TIMEOUT_SECS);
        }

        #[test]
        fn test_timeout_durations() {
            let consensus_timeout = consensus_peer_timeout();
            let stale_threshold = consensus_stale_threshold();

            // Durations should be approximately correct
            assert!(consensus_timeout.as_secs() >= 127);
            assert!(stale_threshold.as_secs() >= 153);
        }
    }
}

/// Configuration for CPP network
#[derive(Debug, Clone)]
pub struct CppConfig {
    /// P2P listen address
    pub p2p_listen: String,

    /// WebSocket listen address
    pub ws_listen: String,

    /// Bootnode addresses (format: "IP:PORT")
    pub bootnodes: Vec<String>,

    /// Maximum number of peers
    pub max_peers: usize,

    /// Enable WebSocket RPC
    pub enable_websocket: bool,

    /// Node type
    pub node_type: NodeType,

    // -----------------------------------------------------------------------
    // Security settings (Phase 5)
    // -----------------------------------------------------------------------
    /// Maximum inbound connections from a single IP.
    pub max_connections_per_ip: usize,

    /// Maximum total connections (inbound + outbound).
    pub max_total_connections: usize,

    /// Maximum peers per /16 subnet (eclipse attack protection).
    pub max_peers_per_subnet: usize,

    /// Ban duration in seconds for misbehaving peers.
    pub ban_duration_secs: u64,

    /// Token-bucket burst capacity for per-peer rate limiting.
    pub rate_bucket_capacity: f64,

    /// Token-bucket refill rate (msgs/sec) for per-peer rate limiting.
    pub rate_msgs_per_sec: f64,

    /// When true, peers must complete the encryption + auth handshake.
    pub require_encryption: bool,
}

impl Default for CppConfig {
    fn default() -> Self {
        CppConfig {
            p2p_listen: DEFAULT_P2P_LISTEN.to_string(),
            ws_listen: DEFAULT_WS_LISTEN.to_string(),
            bootnodes: vec![],
            max_peers: MAX_PEERS,
            enable_websocket: true,
            node_type: NodeType::Full,
            // Security defaults
            max_connections_per_ip: SECURITY_MAX_CONNS_PER_IP,
            max_total_connections: SECURITY_MAX_TOTAL_CONNECTIONS,
            max_peers_per_subnet: SECURITY_MAX_PEERS_PER_SUBNET,
            ban_duration_secs: SECURITY_BAN_DURATION_SECS,
            rate_bucket_capacity: SECURITY_RATE_BUCKET_CAPACITY,
            rate_msgs_per_sec: SECURITY_RATE_MSGS_PER_SEC,
            require_encryption: SECURITY_REQUIRE_ENCRYPTION,
        }
    }
}

/// Node type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    /// Full node (validates and stores all blocks)
    Full,

    /// Archive node (stores full history, serves historical queries)
    Archive,

    /// Validator node (participates in consensus)
    Validator,

    /// Light client (headers only, minimal storage)
    Light,

    /// Bounty node (specialized for problem marketplace)
    Bounty,

    /// Oracle node (provides external data)
    Oracle,
}

impl NodeType {
    pub fn as_u8(&self) -> u8 {
        match self {
            NodeType::Full => 1,
            NodeType::Archive => 2,
            NodeType::Validator => 3,
            NodeType::Light => 0,
            NodeType::Bounty => 4,
            NodeType::Oracle => 5,
        }
    }

    pub fn from_u8(value: u8) -> Result<Self, String> {
        match value {
            0 => Ok(NodeType::Light),
            1 => Ok(NodeType::Full),
            2 => Ok(NodeType::Archive),
            3 => Ok(NodeType::Validator),
            4 => Ok(NodeType::Bounty),
            5 => Ok(NodeType::Oracle),
            _ => Err(format!("Invalid node type: {}", value)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_equilibrium_constant() {
        // Verify η = 1/√2 ≈ 0.7071
        assert!((ETA - 0.7071).abs() < 0.0001);
        assert!((ETA * SQRT_2 - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_port_configuration() {
        assert_eq!(CPP_PORT, 707);
        assert_eq!(WEBSOCKET_PORT, 8080);
    }

    #[test]
    fn test_node_type_conversion() {
        for node_type in [
            NodeType::Light,
            NodeType::Full,
            NodeType::Archive,
            NodeType::Validator,
            NodeType::Bounty,
            NodeType::Oracle,
        ] {
            let byte = node_type.as_u8();
            let recovered = NodeType::from_u8(byte).unwrap();
            assert_eq!(node_type, recovered);
        }
    }
}
