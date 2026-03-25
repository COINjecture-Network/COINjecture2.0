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
//
// NOTE: Some methods are prepared for future node type classification
#![allow(dead_code)]

use clap::{Parser, ValueEnum};
use coinject_core::{BLOCK_VERSION_STANDARD, BLOCK_VERSION_GOLDEN};
use std::net::SocketAddr;
use std::path::PathBuf;

// =============================================================================
// Block Version Configuration
// =============================================================================

/// Supported block versions (locked at compile-time)
pub const SUPPORTED_VERSIONS: [u32; 2] = [BLOCK_VERSION_STANDARD, BLOCK_VERSION_GOLDEN];

/// Human-readable version name
pub fn version_name(version: u32) -> &'static str {
    match version {
        1 => "standard",
        2 => "golden-enhanced",
        _ => "unknown",
    }
}

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

    /// [DEPRECATED: use --cpp-p2p-addr] Legacy libp2p P2P address (ignored in CPP mode)
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

    /// Bootstrap peers (host:port format for CPP, e.g., 'bootnode:707')
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

    /// [DEPRECATED: libp2p only] External address to advertise. Not used in CPP mode.
    #[arg(long)]
    pub external_addr: Option<String>,

    /// [DEPRECATED: libp2p only] Allow private addresses. CPP mode handles this differently.
    #[arg(long)]
    pub allow_private_addrs: bool,

    /// [DEPRECATED] mDNS was removed with libp2p. This flag has no effect.
    #[arg(long)]
    pub disable_mdns: bool,

    // ==========================================================================
    // MESH NETWORK CONFIGURATION
    // ==========================================================================

    /// Enable mesh network layer (P2P gossip transport alongside CPP)
    #[arg(long)]
    pub enable_mesh: bool,

    /// Mesh network listen address (default: 0.0.0.0:9000)
    #[arg(long, default_value = "0.0.0.0:9000")]
    pub mesh_listen: String,

    /// Mesh network seed nodes (can be specified multiple times)
    #[arg(long)]
    pub mesh_seed: Vec<String>,

    // ==========================================================================
    // BLOCK VERSION CONFIGURATION
    // ==========================================================================

    /// Minimum block version to accept (1=standard, 2=golden-enhanced)
    /// Blocks below this version will be rejected with clear logging
    #[arg(long, default_value = "1")]
    pub min_block_version: u32,

    /// Produce blocks with this version (1=standard, 2=golden-enhanced)
    /// Default is v2 (golden-enhanced) for new blocks
    #[arg(long, default_value = "2")]
    pub produce_block_version: u32,

    /// Strict version mode: reject blocks not matching produce_block_version
    /// WARNING: Use only for testing version upgrades
    #[arg(long)]
    pub strict_version: bool,

    // ==========================================================================
    // GOLDEN ACTIVATION (height-based upgrade)
    // ==========================================================================

    /// Block height at which golden-enhanced features activate
    /// Before this height: produce v1 (standard) blocks
    /// At/after this height: produce v2 (golden-enhanced) blocks
    /// Default: 0 (golden features active from genesis)
    #[arg(long, default_value = "0")]
    pub golden_activation_height: u64,
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

    // =========================================================================
    // Block Version Helpers
    // =========================================================================

    /// Check if a block version is supported
    pub fn is_version_supported(&self, version: u32) -> bool {
        SUPPORTED_VERSIONS.contains(&version)
    }

    /// Check if a block version meets minimum requirements
    pub fn meets_version_requirement(&self, version: u32) -> bool {
        version >= self.min_block_version
    }

    /// Check if a block should be accepted based on version policy
    pub fn should_accept_version(&self, version: u32) -> Result<(), String> {
        // Check if version is in supported list
        if !self.is_version_supported(version) {
            return Err(format!(
                "unsupported version {} (supported: {:?})",
                version, SUPPORTED_VERSIONS
            ));
        }

        // Check minimum version requirement
        if version < self.min_block_version {
            return Err(format!(
                "version {} below minimum {} (node requires version >= {})",
                version, self.min_block_version, self.min_block_version
            ));
        }

        // Strict mode: must match produce version exactly
        if self.strict_version && version != self.produce_block_version {
            return Err(format!(
                "strict mode requires version {} (got {})",
                self.produce_block_version, version
            ));
        }

        Ok(())
    }

    /// Get version info string for logging
    pub fn version_info(&self, version: u32) -> String {
        format!("version={} ({})", version, version_name(version))
    }

    /// Determine which block version to produce at a given height
    ///
    /// If golden_activation_height is set:
    /// - Heights < activation: produce v1 (standard)
    /// - Heights >= activation: produce v2 (golden-enhanced)
    ///
    /// If golden_activation_height is 0, uses produce_block_version directly.
    pub fn block_version_for_height(&self, height: u64) -> u32 {
        if self.golden_activation_height > 0 && height < self.golden_activation_height {
            BLOCK_VERSION_STANDARD
        } else if self.golden_activation_height > 0 {
            BLOCK_VERSION_GOLDEN
        } else {
            self.produce_block_version
        }
    }

    /// Check if golden features should be active at a given height
    pub fn is_golden_active(&self, height: u64) -> bool {
        self.block_version_for_height(height) >= BLOCK_VERSION_GOLDEN
    }

    pub fn validate(&self) -> Result<(), String> {
        use coinject_core::validation::{validate_socket_addr_str, validate_port, validate_file_path};

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

        // Validate max_peers range
        if self.max_peers == 0 {
            return Err("max_peers must be at least 1".to_string());
        }
        if self.max_peers > 1000 {
            return Err("max_peers must not exceed 1000".to_string());
        }

        // Validate RPC address
        validate_socket_addr_str(&self.rpc_addr)
            .map_err(|e| format!("Invalid rpc_addr '{}': {}", self.rpc_addr, e))?;

        // Validate CPP P2P address
        validate_socket_addr_str(&self.cpp_p2p_addr)
            .map_err(|e| format!("Invalid cpp_p2p_addr '{}': {}", self.cpp_p2p_addr, e))?;

        // Validate CPP WebSocket address
        validate_socket_addr_str(&self.cpp_ws_addr)
            .map_err(|e| format!("Invalid cpp_ws_addr '{}': {}", self.cpp_ws_addr, e))?;

        // Validate metrics address
        validate_socket_addr_str(&self.metrics_addr)
            .map_err(|e| format!("Invalid metrics_addr '{}': {}", self.metrics_addr, e))?;

        // Validate faucet_cooldown is reasonable (min 1 second, max 30 days)
        if self.enable_faucet {
            if self.faucet_cooldown == 0 {
                return Err("faucet_cooldown must be at least 1 second".to_string());
            }
            if self.faucet_cooldown > 30 * 24 * 3600 {
                return Err("faucet_cooldown exceeds 30 days".to_string());
            }
        }

        // Validate data_dir path (no traversal)
        let data_dir_str = self.data_dir.to_string_lossy();
        validate_file_path(&data_dir_str)
            .map_err(|e| format!("Invalid data_dir: {}", e))?;

        // Validate oracle sources if oracle mode enabled
        if self.is_oracle_mode() && self.oracle_sources.is_empty() {
            tracing::warn!("Oracle mode enabled but no oracle_sources specified");
        }

        // Validate bootnode address formats (must be host:port)
        for bootnode in &self.bootnodes {
            let parts: Vec<&str> = bootnode.rsplitn(2, ':').collect();
            if parts.len() != 2 {
                return Err(format!("Invalid bootnode address '{}': expected host:port format", bootnode));
            }
            let port_str = parts[0];
            let port: u32 = port_str.parse().map_err(|_| {
                format!("Invalid port in bootnode address '{}'", bootnode)
            })?;
            validate_port(port)
                .map_err(|e| format!("Invalid port in bootnode '{}': {}", bootnode, e))?;
        }

        // Warn about conflicting settings
        if self.headers_only && self.mine {
            return Err("Cannot mine in headers-only mode (Light nodes don't store full blocks)".to_string());
        }

        // Validate block version configuration
        if !SUPPORTED_VERSIONS.contains(&self.min_block_version) {
            return Err(format!(
                "min_block_version {} not in supported versions {:?}",
                self.min_block_version, SUPPORTED_VERSIONS
            ));
        }
        if !SUPPORTED_VERSIONS.contains(&self.produce_block_version) {
            return Err(format!(
                "produce_block_version {} not in supported versions {:?}",
                self.produce_block_version, SUPPORTED_VERSIONS
            ));
        }
        if self.produce_block_version < self.min_block_version {
            return Err(format!(
                "produce_block_version {} cannot be lower than min_block_version {}",
                self.produce_block_version, self.min_block_version
            ));
        }

        // Warn about deprecated libp2p-era flags
        if self.p2p_addr != "/ip4/0.0.0.0/tcp/30333" {
            tracing::warn!("--p2p-addr is DEPRECATED (libp2p removed). Use --cpp-p2p-addr instead. This flag is ignored.");
        }
        if self.disable_mdns {
            tracing::warn!("--disable-mdns is DEPRECATED (libp2p removed). mDNS no longer exists. This flag is ignored.");
        }
        if self.external_addr.is_some() {
            tracing::warn!("--external-addr is DEPRECATED (libp2p removed). This flag is ignored in CPP mode.");
        }
        if self.allow_private_addrs {
            tracing::warn!("--allow-private-addrs is DEPRECATED (libp2p removed). This flag is ignored in CPP mode.");
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
            enable_mesh: false,
            mesh_listen: "0.0.0.0:9000".to_string(),
            mesh_seed: vec![],
            min_block_version: 1,
            produce_block_version: 2,
            strict_version: false,
            golden_activation_height: 0,
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
    fn test_invalid_rpc_addr_port_zero() {
        let mut config = test_config();
        config.rpc_addr = "127.0.0.1:0".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_invalid_cpp_p2p_addr() {
        let mut config = test_config();
        config.cpp_p2p_addr = "not-an-address".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_max_peers_zero_rejected() {
        let mut config = test_config();
        config.max_peers = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_max_peers_over_limit_rejected() {
        let mut config = test_config();
        config.max_peers = 1001;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_invalid_bootnode_format() {
        let mut config = test_config();
        config.bootnodes = vec!["notahost".to_string()];
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_valid_bootnode() {
        let mut config = test_config();
        config.bootnodes = vec!["192.0.2.1:707".to_string()];
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_faucet_cooldown_zero_rejected() {
        let mut config = test_config();
        config.enable_faucet = true;
        config.faucet_cooldown = 0;
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

    // =========================================================================
    // Block Version Tests
    // =========================================================================

    #[test]
    fn test_version_supported() {
        let config = test_config();
        assert!(config.is_version_supported(1));
        assert!(config.is_version_supported(2));
        assert!(!config.is_version_supported(0));
        assert!(!config.is_version_supported(3));
    }

    #[test]
    fn test_version_meets_requirement() {
        let mut config = test_config();
        config.min_block_version = 1;
        assert!(config.meets_version_requirement(1));
        assert!(config.meets_version_requirement(2));

        config.min_block_version = 2;
        assert!(!config.meets_version_requirement(1));
        assert!(config.meets_version_requirement(2));
    }

    #[test]
    fn test_should_accept_version_default() {
        let config = test_config(); // min=1, produce=2, strict=false
        assert!(config.should_accept_version(1).is_ok());
        assert!(config.should_accept_version(2).is_ok());
        assert!(config.should_accept_version(0).is_err());
        assert!(config.should_accept_version(3).is_err());
    }

    #[test]
    fn test_should_accept_version_strict() {
        let mut config = test_config();
        config.strict_version = true;
        config.produce_block_version = 2;

        // Strict mode: only accept exact match
        assert!(config.should_accept_version(2).is_ok());
        assert!(config.should_accept_version(1).is_err());
    }

    #[test]
    fn test_should_accept_version_v2_only() {
        let mut config = test_config();
        config.min_block_version = 2;

        // V2-only mode: reject v1
        assert!(config.should_accept_version(2).is_ok());
        assert!(config.should_accept_version(1).is_err());
    }

    #[test]
    fn test_invalid_version_config() {
        let mut config = test_config();

        // Invalid min_block_version
        config.min_block_version = 99;
        assert!(config.validate().is_err());

        // Reset and test invalid produce_block_version
        config.min_block_version = 1;
        config.produce_block_version = 99;
        assert!(config.validate().is_err());

        // Reset and test produce < min
        config.produce_block_version = 1;
        config.min_block_version = 2;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_version_name() {
        assert_eq!(version_name(1), "standard");
        assert_eq!(version_name(2), "golden-enhanced");
        assert_eq!(version_name(99), "unknown");
    }

    // =========================================================================
    // Golden Activation Tests
    // =========================================================================

    #[test]
    fn test_golden_activation_height_zero() {
        // When activation_height is 0, use produce_block_version
        let mut config = test_config();
        config.golden_activation_height = 0;
        config.produce_block_version = 2;
        
        assert_eq!(config.block_version_for_height(0), 2);
        assert_eq!(config.block_version_for_height(100), 2);
        assert!(config.is_golden_active(0));
    }

    #[test]
    fn test_golden_activation_height_set() {
        let mut config = test_config();
        config.golden_activation_height = 1000;
        
        // Before activation: v1
        assert_eq!(config.block_version_for_height(0), 1);
        assert_eq!(config.block_version_for_height(500), 1);
        assert_eq!(config.block_version_for_height(999), 1);
        assert!(!config.is_golden_active(999));
        
        // At/after activation: v2
        assert_eq!(config.block_version_for_height(1000), 2);
        assert_eq!(config.block_version_for_height(1001), 2);
        assert_eq!(config.block_version_for_height(10000), 2);
        assert!(config.is_golden_active(1000));
    }
}
