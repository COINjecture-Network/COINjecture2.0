// Node Configuration
// CLI args and runtime configuration
//
// Supports 6 specialized node types with dynamic behavioral classification:
// - Light: Header-only sync, minimal storage (mobile-friendly)
// - Full: Complete validation, standard storage (default)
// - Archive: Complete history, 2TB+ storage
// - Validator: Block production, high validation speed
// - Bounty: NP-problem solving focused
// - Oracle: External data feeds

use clap::{Parser, ValueEnum};
use std::net::SocketAddr;
use std::path::PathBuf;

/// Node type preference (actual classification is based on behavior)
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum NodeTypePreference {
    /// Header-only sync for mobile/embedded devices (minimal storage)
    Light,
    /// Full validation with standard storage (default)
    Full,
    /// Complete historical data preservation (2TB+ storage)
    Archive,
    /// Block production and high-speed validation
    Validator,
    /// NP-problem solving and bounty hunting
    Bounty,
    /// External data feeds and cross-chain bridges
    Oracle,
}

impl Default for NodeTypePreference {
    fn default() -> Self {
        NodeTypePreference::Full
    }
}

impl std::fmt::Display for NodeTypePreference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeTypePreference::Light => write!(f, "light"),
            NodeTypePreference::Full => write!(f, "full"),
            NodeTypePreference::Archive => write!(f, "archive"),
            NodeTypePreference::Validator => write!(f, "validator"),
            NodeTypePreference::Bounty => write!(f, "bounty"),
            NodeTypePreference::Oracle => write!(f, "oracle"),
        }
    }
}

#[derive(Parser, Debug, Clone)]
#[command(author, version, about = "COINjecture Network B - NP-hard Consensus Blockchain", long_about = None)]
pub struct NodeConfig {
    /// Data directory for blockchain storage
    #[arg(long, default_value = "./data")]
    pub data_dir: PathBuf,

    // ==========================================================================
    // NODE TYPE CONFIGURATION
    // ==========================================================================
    
    /// Target node type (preference, actual type is determined by behavior)
    /// Options: light, full, archive, validator, bounty, oracle
    #[arg(long, value_enum, default_value = "full")]
    pub node_type: NodeTypePreference,
    
    /// Run in headers-only mode (Light node sync)
    /// Only downloads and validates block headers, not full blocks
    #[arg(long)]
    pub headers_only: bool,
    
    /// Enable bounty hunting mode (actively solve NP-problems)
    #[arg(long)]
    pub bounty_hunter: bool,
    
    /// Enable oracle mode (provide external data feeds)
    #[arg(long)]
    pub oracle_mode: bool,
    
    /// Oracle data sources (URLs for external feeds)
    #[arg(long)]
    pub oracle_sources: Vec<String>,

    // ==========================================================================
    // STANDARD CONFIGURATION
    // ==========================================================================

    /// Run in development mode (auto-mining, no peers)
    #[arg(long)]
    pub dev: bool,

    /// Enable mining
    #[arg(long)]
    pub mine: bool,

    /// Miner address (hex, 64 chars)
    #[arg(long)]
    pub miner_address: Option<String>,

    /// P2P listen address
    #[arg(long, default_value = "/ip4/0.0.0.0/tcp/30333")]
    pub p2p_addr: String,

    /// RPC listen address
    #[arg(long, default_value = "127.0.0.1:9933")]
    pub rpc_addr: String,

    /// CPP P2P listen address (for CPP protocol, default: 0.0.0.0:707)
    #[arg(long, default_value = "0.0.0.0:707")]
    pub cpp_p2p_addr: String,

    /// CPP WebSocket listen address (for light client mining, default: 0.0.0.0:8080)
    #[arg(long, default_value = "0.0.0.0:8080")]
    pub cpp_ws_addr: String,

    /// Prometheus metrics listen address
    #[arg(long, default_value = "127.0.0.1:9090")]
    pub metrics_addr: String,

    /// Bootstrap peers (multiaddr format)
    #[arg(long)]
    pub bootnodes: Vec<String>,

    /// Chain ID (v2 = fresh network after 2025-11-30 reset)
    #[arg(long, default_value = "coinject-network-b-v2")]
    pub chain_id: String,

    /// Mining difficulty (leading zeros in hash)
    #[arg(long, default_value = "4")]
    pub difficulty: u32,

    /// Target block time in seconds
    #[arg(long, default_value = "60")]
    pub block_time: u64,

    /// Maximum number of peers
    #[arg(long, default_value = "50")]
    pub max_peers: usize,

    /// Enable verbose logging
    #[arg(short, long)]
    pub verbose: bool,

    /// Enable testnet faucet (allows free token requests for testing)
    #[arg(long)]
    pub enable_faucet: bool,

    /// Faucet amount (tokens per request)
    #[arg(long, default_value = "10000")]
    pub faucet_amount: u128,

    /// Faucet cooldown (seconds between requests per address)
    #[arg(long, default_value = "3600")]
    pub faucet_cooldown: u64,

    /// HuggingFace API token for dataset uploads
    #[arg(long)]
    pub hf_token: Option<String>,

    /// HuggingFace dataset name (format: username/dataset-name)
    #[arg(long)]
    pub hf_dataset_name: Option<String>,

    /// Use ADZDB instead of redb for chain storage (experimental)
    /// Requires compilation with --features adzdb
    #[arg(long)]
    pub use_adzdb: bool,

    // ==========================================================================
    // NETWORK CONNECTIVITY
    // ==========================================================================

    /// External/public address to advertise to peers (multiaddr format)
    /// Use this when running behind NAT or Docker to ensure peers dial the correct address.
    /// Example: /ip4/143.110.139.166/tcp/30333
    #[arg(long)]
    pub external_addr: Option<String>,

    /// Allow private RFC1918 addresses (for local/LAN testing)
    /// By default, private addresses (10.x.x.x, 172.16-31.x.x, 192.168.x.x) are rejected
    #[arg(long)]
    pub allow_private_addrs: bool,

    /// Disable mDNS peer discovery (for network isolation testing)
    /// Useful for partition/fork testing where you don't want local nodes to find each other
    #[arg(long)]
    pub disable_mdns: bool,
}

impl NodeConfig {
    pub fn parse_args() -> Self {
        NodeConfig::parse()
    }

    pub fn rpc_socket_addr(&self) -> Result<SocketAddr, Box<dyn std::error::Error>> {
        Ok(self.rpc_addr.parse()?)
    }

    pub fn metrics_socket_addr(&self) -> Result<SocketAddr, Box<dyn std::error::Error>> {
        Ok(self.metrics_addr.parse()?)
    }

    pub fn state_db_path(&self) -> PathBuf {
        self.data_dir.join("state.db")
    }

    pub fn chain_db_path(&self) -> PathBuf {
        self.data_dir.join("chain.db")
    }
    
    /// Check if this node is configured for header-only mode (Light node)
    pub fn is_light_mode(&self) -> bool {
        self.headers_only || matches!(self.node_type, NodeTypePreference::Light)
    }
    
    /// Check if this node is configured for archive mode
    pub fn is_archive_mode(&self) -> bool {
        matches!(self.node_type, NodeTypePreference::Archive)
    }
    
    /// Check if bounty hunting is enabled
    pub fn is_bounty_hunter(&self) -> bool {
        self.bounty_hunter || matches!(self.node_type, NodeTypePreference::Bounty)
    }
    
    /// Check if oracle mode is enabled
    pub fn is_oracle_mode(&self) -> bool {
        self.oracle_mode || matches!(self.node_type, NodeTypePreference::Oracle)
    }
    
    /// Get the target node type for classification
    pub fn target_node_type(&self) -> crate::node_types::NodeType {
        use crate::node_types::NodeType;
        
        // Override based on specific flags
        if self.headers_only {
            return NodeType::Light;
        }
        if self.bounty_hunter {
            return NodeType::Bounty;
        }
        if self.oracle_mode {
            return NodeType::Oracle;
        }
        
        // Map preference to node type
        match self.node_type {
            NodeTypePreference::Light => NodeType::Light,
            NodeTypePreference::Full => NodeType::Full,
            NodeTypePreference::Archive => NodeType::Archive,
            NodeTypePreference::Validator => NodeType::Validator,
            NodeTypePreference::Bounty => NodeType::Bounty,
            NodeTypePreference::Oracle => NodeType::Oracle,
        }
    }
    
    /// Get storage requirements for the target node type (in GB)
    pub fn storage_requirement_gb(&self) -> u32 {
        self.target_node_type().hardware_requirements().min_storage_gb
    }

    pub fn validate(&self) -> Result<(), String> {
        // Validate miner address format if provided
        if let Some(ref addr) = self.miner_address {
            if addr.len() != 64 {
                return Err("Miner address must be 64 hex characters (32 bytes)".to_string());
            }
            if !addr.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err("Miner address must be valid hex".to_string());
            }
        }

        // Validate difficulty range
        if self.difficulty < 1 || self.difficulty > 64 {
            return Err("Difficulty must be between 1 and 64".to_string());
        }

        // Validate block time
        if self.block_time < 10 {
            return Err("Block time must be at least 10 seconds".to_string());
        }
        
        // Validate oracle sources if oracle mode enabled
        if self.is_oracle_mode() && self.oracle_sources.is_empty() {
            tracing::warn!("Oracle mode enabled but no oracle_sources specified");
        }
        
        // Warn about conflicting settings
        if self.headers_only && self.mine {
            return Err("Cannot mine in headers-only mode (Light nodes don't store full blocks)".to_string());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> NodeConfig {
        NodeConfig {
            data_dir: PathBuf::from("./data"),
            node_type: NodeTypePreference::Full,
            headers_only: false,
            bounty_hunter: false,
            oracle_mode: false,
            oracle_sources: vec![],
            dev: false,
            mine: false,
            miner_address: Some("0000000000000000000000000000000000000000000000000000000000000001".to_string()),
            p2p_addr: "/ip4/0.0.0.0/tcp/30333".to_string(),
            rpc_addr: "127.0.0.1:9933".to_string(),
            cpp_p2p_addr: "0.0.0.0:707".to_string(),
            cpp_ws_addr: "0.0.0.0:8080".to_string(),
            metrics_addr: "127.0.0.1:9090".to_string(),
            bootnodes: vec![],
            chain_id: "test".to_string(),
            difficulty: 4,
            block_time: 60,
            max_peers: 50,
            verbose: false,
            enable_faucet: false,
            faucet_amount: 10000,
            faucet_cooldown: 3600,
            hf_token: None,
            hf_dataset_name: None,
            use_adzdb: false,
            external_addr: None,
            allow_private_addrs: false,
            disable_mdns: false,
        }
    }

    #[test]
    fn test_config_validation() {
        let config = test_config();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_invalid_miner_address() {
        let mut config = test_config();
        config.miner_address = Some("invalid".to_string());
        assert!(config.validate().is_err());
    }
    
    #[test]
    fn test_node_type_light() {
        let mut config = test_config();
        config.node_type = NodeTypePreference::Light;
        assert!(config.is_light_mode());
    }
    
    #[test]
    fn test_headers_only_implies_light() {
        let mut config = test_config();
        config.headers_only = true;
        assert!(config.is_light_mode());
    }
    
    #[test]
    fn test_headers_only_cannot_mine() {
        let mut config = test_config();
        config.headers_only = true;
        config.mine = true;
        assert!(config.validate().is_err());
    }
    
    #[test]
    fn test_bounty_hunter_mode() {
        let mut config = test_config();
        config.bounty_hunter = true;
        assert!(config.is_bounty_hunter());
    }
    
    #[test]
    fn test_oracle_mode() {
        let mut config = test_config();
        config.oracle_mode = true;
        config.oracle_sources = vec!["https://api.example.com".to_string()];
        assert!(config.is_oracle_mode());
    }
}
