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

/// Protocol version
pub const VERSION: u8 = 1;

/// Maximum message size (10 MB)
/// Large enough for block batches, small enough to prevent DoS
pub const MAX_MESSAGE_SIZE: usize = 10 * 1024 * 1024;

/// Connection timeout
pub const CONNECTION_TIMEOUT: Duration = Duration::from_secs(30);

/// Handshake timeout (much faster than libp2p's Noise+Yamux)
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// Keep-alive interval (ping every 30 seconds)
pub const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(30);

/// Peer timeout (disconnect if no message for 90 seconds)
pub const PEER_TIMEOUT: Duration = Duration::from_secs(90);

/// Maximum number of peers
pub const MAX_PEERS: usize = 50;

/// Maximum number of pending connections
pub const MAX_PENDING_CONNECTIONS: usize = 10;

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
