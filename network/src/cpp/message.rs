// =============================================================================
// COINjecture P2P Protocol (CPP) - Message Types
// =============================================================================
// Message definitions for the CPP protocol

use coinject_core::{Block, Transaction, Hash, BlockHeader};
use serde::{Deserialize, Serialize};
use crate::cpp::flock::FlockStateCompact;

/// Message type identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageType {
    // === HANDSHAKE ===
    /// Initial connection handshake
    Hello = 0x01,
    /// Handshake acknowledgment
    HelloAck = 0x02,
    
    // === SYNC ===
    /// Peer status update (height, hash, node type)
    Status = 0x10,
    /// Request blocks by height range
    GetBlocks = 0x11,
    /// Block response (batch of blocks)
    Blocks = 0x12,
    /// Request headers (light clients)
    GetHeaders = 0x13,
    /// Header response
    Headers = 0x14,
    
    // === PROPAGATION ===
    /// Newly mined block announcement
    NewBlock = 0x20,
    /// New transaction announcement
    NewTransaction = 0x21,
    
    // === LIGHT CLIENT MINING ===
    /// Light client submits proof-of-work
    SubmitWork = 0x30,
    /// Work accepted by validator
    WorkAccepted = 0x31,
    /// Work rejected (invalid PoW, stale, etc.)
    WorkRejected = 0x32,
    /// Request mining work template
    GetWork = 0x33,
    /// Mining work template response
    Work = 0x34,
    
    // === CONTROL ===
    /// Keep-alive ping
    Ping = 0xF0,
    /// Keep-alive pong
    Pong = 0xF1,
    /// Graceful disconnect
    Disconnect = 0xFF,
}

impl MessageType {
    pub fn from_u8(value: u8) -> Result<Self, String> {
        match value {
            0x01 => Ok(MessageType::Hello),
            0x02 => Ok(MessageType::HelloAck),
            0x10 => Ok(MessageType::Status),
            0x11 => Ok(MessageType::GetBlocks),
            0x12 => Ok(MessageType::Blocks),
            0x13 => Ok(MessageType::GetHeaders),
            0x14 => Ok(MessageType::Headers),
            0x20 => Ok(MessageType::NewBlock),
            0x21 => Ok(MessageType::NewTransaction),
            0x30 => Ok(MessageType::SubmitWork),
            0x31 => Ok(MessageType::WorkAccepted),
            0x32 => Ok(MessageType::WorkRejected),
            0x33 => Ok(MessageType::GetWork),
            0x34 => Ok(MessageType::Work),
            0xF0 => Ok(MessageType::Ping),
            0xF1 => Ok(MessageType::Pong),
            0xFF => Ok(MessageType::Disconnect),
            _ => Err(format!("Unknown message type: 0x{:02X}", value)),
        }
    }
    
    /// Get message priority based on dimensional scale
    pub fn priority(&self) -> MessagePriority {
        match self {
            MessageType::NewBlock => MessagePriority::D1_Critical,
            MessageType::NewTransaction => MessagePriority::D2_High,
            MessageType::Status => MessagePriority::D3_Normal,
            MessageType::Hello | MessageType::HelloAck => MessagePriority::D3_Normal,
            MessageType::GetBlocks => MessagePriority::D4_Low,
            MessageType::Blocks => MessagePriority::D5_Background,
            MessageType::GetHeaders => MessagePriority::D6_Bulk,
            MessageType::Headers => MessagePriority::D7_Archive,
            MessageType::SubmitWork | MessageType::WorkAccepted | MessageType::WorkRejected => MessagePriority::D2_High,
            MessageType::GetWork | MessageType::Work => MessagePriority::D3_Normal,
            MessageType::Ping | MessageType::Pong => MessagePriority::D8_Historical,
            MessageType::Disconnect => MessagePriority::D1_Critical,
        }
    }
}

/// Message priority based on dimensional scales
/// 
/// Each priority level corresponds to a dimensional scale Dₙ = e^(-τₙ/√2)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MessagePriority {
    D1_Critical = 1,      // τ = 0.00, scale = 1.000 (immediate)
    D2_High = 2,          // τ = 0.20, scale = 0.867
    D3_Normal = 3,        // τ = 0.41, scale = 0.750
    D4_Low = 4,           // τ = 0.68, scale = 0.618 (φ⁻¹)
    D5_Background = 5,    // τ = 0.98, scale = 0.500 (2⁻¹)
    D6_Bulk = 6,          // τ = 1.36, scale = 0.382 (φ⁻²)
    D7_Archive = 7,       // τ = 1.96, scale = 0.250 (2⁻²)
    D8_Historical = 8,    // τ = 2.72, scale = 0.146 (e⁻¹)
}

impl MessagePriority {
    /// Get the dimensional scale for this priority
    pub fn scale(&self) -> f64 {
        let tau = self.tau();
        let eta = std::f64::consts::FRAC_1_SQRT_2;
        (-eta * tau).exp()
    }
    
    /// Get the tau value for this priority
    pub fn tau(&self) -> f64 {
        match self {
            MessagePriority::D1_Critical => 0.00,
            MessagePriority::D2_High => 0.20,
            MessagePriority::D3_Normal => 0.41,
            MessagePriority::D4_Low => 0.68,
            MessagePriority::D5_Background => 0.98,
            MessagePriority::D6_Bulk => 1.36,
            MessagePriority::D7_Archive => 1.96,
            MessagePriority::D8_Historical => 2.72,
        }
    }
}

// === MESSAGE PAYLOADS ===

/// Hello message (initial handshake)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloMessage {
    /// Protocol version
    pub version: u8,
    /// Peer ID (32-byte hash of public key)
    pub peer_id: [u8; 32],
    /// Best block height
    pub best_height: u64,
    /// Best block hash
    pub best_hash: Hash,
    /// Genesis block hash (for chain validation)
    pub genesis_hash: Hash,
    /// Node type
    pub node_type: u8,
    /// Timestamp (for replay protection)
    pub timestamp: u64,
    /// Connection nonce for deterministic tie-breaking of simultaneous connections
    /// Lower nonce wins in race condition resolution (backward compatible with default 0)
    #[serde(default)]
    pub connection_nonce: u64,
}

/// HelloAck message (handshake response)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloAckMessage {
    /// Protocol version
    pub version: u8,
    /// Peer ID
    pub peer_id: [u8; 32],
    /// Best block height
    pub best_height: u64,
    /// Best block hash
    pub best_hash: Hash,
    /// Genesis block hash
    pub genesis_hash: Hash,
    /// Node type
    pub node_type: u8,
    /// Timestamp
    pub timestamp: u64,
    /// Connection nonce for deterministic tie-breaking of simultaneous connections
    /// Lower nonce wins in race condition resolution (backward compatible with default 0)
    #[serde(default)]
    pub connection_nonce: u64,
}

/// Status update message with murmuration coordination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusMessage {
    /// Best block height
    pub best_height: u64,
    /// Best block hash
    pub best_hash: Hash,
    /// Node type (may change, e.g., Light -> Full)
    pub node_type: u8,
    /// Timestamp
    pub timestamp: u64,
    /// Flock state for murmuration coordination (optional for backwards compat)
    #[serde(default)]
    pub flock_state: Option<FlockStateCompact>,
}

/// GetBlocks request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetBlocksMessage {
    /// Start height (inclusive)
    pub from_height: u64,
    /// End height (inclusive)
    pub to_height: u64,
    /// Request ID (for tracking)
    pub request_id: u64,
}

/// Blocks response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlocksMessage {
    /// Blocks (ordered by height)
    pub blocks: Vec<Block>,
    /// Request ID (matches GetBlocks)
    pub request_id: u64,
}

/// GetHeaders request (light clients)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetHeadersMessage {
    /// Start height (inclusive)
    pub from_height: u64,
    /// Maximum number of headers
    pub max_headers: u32,
    /// Request ID
    pub request_id: u64,
}

/// Headers response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadersMessage {
    /// Block headers (ordered by height)
    pub headers: Vec<BlockHeader>,
    /// Request ID
    pub request_id: u64,
}

/// NewBlock announcement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewBlockMessage {
    /// Newly mined block
    pub block: Block,
}

/// NewTransaction announcement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewTransactionMessage {
    /// New transaction
    pub transaction: Transaction,
}

/// SubmitWork message (light client mining)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitWorkMessage {
    /// Block with proof-of-work solution
    pub block: Block,
    /// Light client address (for reward)
    pub miner_address: [u8; 32],
}

/// WorkAccepted response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkAcceptedMessage {
    /// Block hash
    pub block_hash: Hash,
    /// Block height
    pub height: u64,
    /// Reward amount
    pub reward: u128,
}

/// WorkRejected response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkRejectedMessage {
    /// Block hash
    pub block_hash: Hash,
    /// Rejection reason
    pub reason: String,
}

/// GetWork request (light client)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetWorkMessage {
    /// Light client address
    pub miner_address: [u8; 32],
}

/// Work template response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkMessage {
    /// Block header template (without nonce)
    pub header: BlockHeader,
    /// Target difficulty
    pub difficulty: u64,
    /// Transactions to include
    pub transactions: Vec<Transaction>,
}

/// Ping message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingMessage {
    /// Timestamp
    pub timestamp: u64,
    /// Nonce (for matching with pong)
    pub nonce: u64,
}

/// Pong message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PongMessage {
    /// Timestamp
    pub timestamp: u64,
    /// Nonce (matches ping)
    pub nonce: u64,
}

/// Disconnect message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisconnectMessage {
    /// Reason for disconnect
    pub reason: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_message_type_conversion() {
        let types = vec![
            MessageType::Hello,
            MessageType::HelloAck,
            MessageType::Status,
            MessageType::GetBlocks,
            MessageType::Blocks,
            MessageType::NewBlock,
            MessageType::NewTransaction,
            MessageType::Ping,
            MessageType::Pong,
            MessageType::Disconnect,
        ];
        
        for msg_type in types {
            let byte = msg_type as u8;
            let recovered = MessageType::from_u8(byte).unwrap();
            assert_eq!(msg_type, recovered);
        }
    }
    
    #[test]
    fn test_message_priorities() {
        // NewBlock should have highest priority
        assert_eq!(MessageType::NewBlock.priority(), MessagePriority::D1_Critical);
        
        // Ping should have lowest priority
        assert_eq!(MessageType::Ping.priority(), MessagePriority::D8_Historical);
        
        // Verify priority ordering
        assert!(MessagePriority::D1_Critical < MessagePriority::D8_Historical);
    }
    
    #[test]
    fn test_dimensional_scales() {
        // D1 should be 1.0 (e^0)
        assert!((MessagePriority::D1_Critical.scale() - 1.0).abs() < 0.01);
        
        // D4 should be ~0.618 (golden ratio)
        let d4_scale = MessagePriority::D4_Low.scale();
        assert!(d4_scale > 0.6 && d4_scale < 0.65);
        
        // D5 should be ~0.5 (2^-1)
        let d5_scale = MessagePriority::D5_Background.scale();
        assert!(d5_scale > 0.48 && d5_scale < 0.52);
    }
}
