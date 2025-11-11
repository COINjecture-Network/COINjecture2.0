// Node Configuration
// CLI args and runtime configuration

use clap::Parser;
use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about = "COINjecture Network B - NP-hard Consensus Blockchain", long_about = None)]
pub struct NodeConfig {
    /// Data directory for blockchain storage
    #[arg(long, default_value = "./data")]
    pub data_dir: PathBuf,

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

    /// Bootstrap peers (multiaddr format)
    #[arg(long)]
    pub bootnodes: Vec<String>,

    /// Chain ID
    #[arg(long, default_value = "coinject-network-b")]
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
}

impl NodeConfig {
    pub fn parse_args() -> Self {
        NodeConfig::parse()
    }

    pub fn rpc_socket_addr(&self) -> Result<SocketAddr, Box<dyn std::error::Error>> {
        Ok(self.rpc_addr.parse()?)
    }

    pub fn state_db_path(&self) -> PathBuf {
        self.data_dir.join("state")
    }

    pub fn chain_db_path(&self) -> PathBuf {
        self.data_dir.join("chain")
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

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        let config = NodeConfig {
            data_dir: PathBuf::from("./data"),
            dev: false,
            mine: false,
            miner_address: Some("0000000000000000000000000000000000000000000000000000000000000001".to_string()),
            p2p_addr: "/ip4/0.0.0.0/tcp/30333".to_string(),
            rpc_addr: "127.0.0.1:9933".to_string(),
            bootnodes: vec![],
            chain_id: "test".to_string(),
            difficulty: 4,
            block_time: 60,
            max_peers: 50,
            verbose: false,
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_invalid_miner_address() {
        let config = NodeConfig {
            data_dir: PathBuf::from("./data"),
            dev: false,
            mine: false,
            miner_address: Some("invalid".to_string()),
            p2p_addr: "/ip4/0.0.0.0/tcp/30333".to_string(),
            rpc_addr: "127.0.0.1:9933".to_string(),
            bootnodes: vec![],
            chain_id: "test".to_string(),
            difficulty: 4,
            block_time: 60,
            max_peers: 50,
            verbose: false,
        };

        assert!(config.validate().is_err());
    }
}
