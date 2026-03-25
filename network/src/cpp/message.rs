// =============================================================================
// COINjecture P2P Protocol (CPP) - Message Types
// =============================================================================
// Message definitions for the CPP protocol

use coinject_core::{Block, Transaction, Hash, BlockHeader};
use serde::{Deserialize, Serialize};
use crate::cpp::flock::FlockStateCompact;
// blake3 and ed25519_dalek used in authentication helpers below

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
/// Names use D{n}_ prefix to match dimensional notation in whitepaper
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[allow(non_camel_case_types)]
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

// === SERDE HELPERS FOR LARGE ARRAYS ===
//
// serde_core v1.0.x only implements Serialize/Deserialize for arrays up to [T; 32].
// For [u8; 64] (ed25519 signatures) we need a custom module.

mod serde_sig64 {
    use serde::{Deserializer, Serializer};

    pub fn serialize<S: Serializer>(arr: &[u8; 64], s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeTuple;
        let mut tup = s.serialize_tuple(64)?;
        for byte in arr {
            tup.serialize_element(byte)?;
        }
        tup.end()
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 64], D::Error> {
        use serde::de::{SeqAccess, Visitor};
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = [u8; 64];
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "a 64-byte array")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<[u8; 64], A::Error> {
                let mut arr = [0u8; 64];
                for (i, slot) in arr.iter_mut().enumerate() {
                    *slot = seq.next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(i, &self))?;
                }
                Ok(arr)
            }
        }
        d.deserialize_tuple(64, V)
    }
}

fn default_sig() -> [u8; 64] { [0u8; 64] }

// =============================================================================
// Message-level validation
// =============================================================================

use coinject_core::validation::{
    validate_node_type_byte, validate_reason_string,
    validate_get_blocks_range, validate_get_headers,
};

// === MESSAGE PAYLOADS ===

/// Hello message (initial handshake)
///
/// After the encryption/auth handshake, the sender proves ownership of `peer_id`
/// by including their ed25519 public key and a signature over the challenge data.
/// Challenge = genesis_hash || timestamp_bytes || connection_nonce_bytes || peer_id
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloMessage {
    /// Protocol version
    pub version: u8,
    /// Peer ID (BLAKE3 hash of ed25519 public key, 32 bytes)
    pub peer_id: [u8; 32],
    /// Best block height
    pub best_height: u64,
    /// Best block hash
    pub best_hash: Hash,
    /// Genesis block hash (for chain validation)
    pub genesis_hash: Hash,
    /// Node type
    pub node_type: u8,
    /// Timestamp (for replay protection, Unix seconds)
    pub timestamp: u64,
    /// Connection nonce for deterministic tie-breaking of simultaneous connections
    /// Lower nonce wins in race condition resolution (backward compatible with default 0)
    #[serde(default)]
    pub connection_nonce: u64,
    /// Ed25519 verifying (public) key of this node (32 bytes).
    /// All zeros = unauthenticated (legacy / dev mode).
    #[serde(default)]
    pub ed25519_pubkey: [u8; 32],
    /// Ed25519 signature over the challenge:
    ///   genesis_hash || timestamp (8 LE) || connection_nonce (8 LE) || peer_id
    /// All zeros = unauthenticated (legacy / dev mode).
    #[serde(with = "serde_sig64", default = "default_sig")]
    pub auth_signature: [u8; 64],
}

impl HelloMessage {
    /// Validate structural constraints of a received HelloMessage.
    pub fn validate(&self) -> Result<(), String> {
        validate_node_type_byte(self.node_type)
            .map_err(|e| e.to_string())?;
        // Peer ID must not be all-zeros (would collide with default)
        if self.peer_id == [0u8; 32] {
            return Err("peer_id must not be all-zeros".to_string());
        }
        Ok(())
    }
}

/// HelloAck message (handshake response)
///
/// Same authentication scheme as HelloMessage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloAckMessage {
    /// Protocol version
    pub version: u8,
    /// Peer ID (BLAKE3 hash of ed25519 public key, 32 bytes)
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
    /// Ed25519 verifying key (32 bytes).  All zeros = unauthenticated.
    #[serde(default)]
    pub ed25519_pubkey: [u8; 32],
    /// Ed25519 signature over challenge (same scheme as HelloMessage).
    /// All zeros = unauthenticated.
    #[serde(with = "serde_sig64", default = "default_sig")]
    pub auth_signature: [u8; 64],
}

// ---------------------------------------------------------------------------
// Authentication helpers for Hello / HelloAck
// ---------------------------------------------------------------------------

/// Build the challenge bytes that are signed / verified in Hello and HelloAck.
///
/// challenge = genesis_hash (32) || timestamp LE (8) || connection_nonce LE (8) || peer_id (32)
pub fn hello_challenge(
    genesis_hash: &Hash,
    timestamp: u64,
    connection_nonce: u64,
    peer_id: &[u8; 32],
) -> Vec<u8> {
    let mut msg = Vec::with_capacity(80);
    msg.extend_from_slice(genesis_hash.as_bytes());
    msg.extend_from_slice(&timestamp.to_le_bytes());
    msg.extend_from_slice(&connection_nonce.to_le_bytes());
    msg.extend_from_slice(peer_id);
    msg
}

/// Sign a Hello challenge with an ed25519 signing key.
/// Returns (ed25519_pubkey, auth_signature).
pub fn sign_hello(
    signing_key: &ed25519_dalek::SigningKey,
    genesis_hash: &Hash,
    timestamp: u64,
    connection_nonce: u64,
    peer_id: &[u8; 32],
) -> ([u8; 32], [u8; 64]) {
    use ed25519_dalek::Signer;
    let challenge = hello_challenge(genesis_hash, timestamp, connection_nonce, peer_id);
    let sig = signing_key.sign(&challenge);
    (signing_key.verifying_key().to_bytes(), sig.to_bytes())
}

/// Verify the authentication signature in a Hello or HelloAck message.
///
/// Returns `Err(description)` if verification fails.
pub fn verify_hello_auth(
    ed25519_pubkey: &[u8; 32],
    auth_signature: &[u8; 64],
    genesis_hash: &Hash,
    timestamp: u64,
    connection_nonce: u64,
    peer_id: &[u8; 32],
) -> Result<(), String> {
    use ed25519_dalek::{Verifier, VerifyingKey, Signature};

    // All-zeros means unauthenticated (legacy/dev mode) — caller decides whether to allow
    if *ed25519_pubkey == [0u8; 32] {
        return Err("ed25519_pubkey is all-zeros (unauthenticated)".to_string());
    }

    let verifying_key = VerifyingKey::from_bytes(ed25519_pubkey)
        .map_err(|e| format!("Invalid ed25519 pubkey: {}", e))?;

    let challenge = hello_challenge(genesis_hash, timestamp, connection_nonce, peer_id);
    let signature = Signature::from_bytes(auth_signature);

    verifying_key.verify(&challenge, &signature)
        .map_err(|e| format!("Signature verification failed: {}", e))?;

    // Also verify peer_id matches BLAKE3(ed25519_pubkey)
    let derived_peer_id = *blake3::hash(ed25519_pubkey).as_bytes();
    if derived_peer_id != *peer_id {
        return Err(format!(
            "peer_id mismatch: claimed {:?} but derived from pubkey {:?}",
            &peer_id[..4], &derived_peer_id[..4]
        ));
    }

    Ok(())
}

impl HelloAckMessage {
    /// Validate structural constraints of a received HelloAckMessage.
    pub fn validate(&self) -> Result<(), String> {
        validate_node_type_byte(self.node_type)
            .map_err(|e| e.to_string())?;
        if self.peer_id == [0u8; 32] {
            return Err("peer_id must not be all-zeros".to_string());
        }
        Ok(())
    }
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

impl StatusMessage {
    /// Validate structural constraints of a received StatusMessage.
    pub fn validate(&self) -> Result<(), String> {
        validate_node_type_byte(self.node_type)
            .map_err(|e| e.to_string())
    }
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

impl GetBlocksMessage {
    /// Validate the height range requested.
    pub fn validate(&self) -> Result<(), String> {
        validate_get_blocks_range(self.from_height, self.to_height)
            .map_err(|e| e.to_string())
    }
}

/// Blocks response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlocksMessage {
    /// Blocks (ordered by height)
    pub blocks: Vec<Block>,
    /// Request ID (matches GetBlocks)
    pub request_id: u64,
}

impl BlocksMessage {
    /// Validate block count does not exceed the maximum per response.
    pub fn validate(&self) -> Result<(), String> {
        use coinject_core::validation::MAX_BLOCKS_PER_REQUEST;
        if self.blocks.len() as u64 > MAX_BLOCKS_PER_REQUEST {
            return Err(format!(
                "Blocks response contains {} blocks, limit is {}",
                self.blocks.len(),
                MAX_BLOCKS_PER_REQUEST
            ));
        }
        Ok(())
    }
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

impl GetHeadersMessage {
    /// Validate the max_headers field.
    pub fn validate(&self) -> Result<(), String> {
        validate_get_headers(self.max_headers)
            .map_err(|e| e.to_string())
    }
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

impl WorkRejectedMessage {
    /// Validate the rejection reason string length.
    pub fn validate(&self) -> Result<(), String> {
        validate_reason_string(&self.reason)
            .map_err(|e| e.to_string())
    }
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

impl DisconnectMessage {
    /// Validate the reason string length.
    pub fn validate(&self) -> Result<(), String> {
        validate_reason_string(&self.reason)
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- message validation ----

    #[test]
    fn hello_validates_node_type() {
        let valid = HelloMessage {
            version: 1,
            peer_id: [1u8; 32],
            best_height: 0,
            best_hash: coinject_core::Hash::ZERO,
            genesis_hash: coinject_core::Hash::ZERO,
            node_type: 1,
            timestamp: 0,
            connection_nonce: 0,
            ed25519_pubkey: [0u8; 32],
            auth_signature: [0u8; 64],
        };
        assert!(valid.validate().is_ok());

        let invalid_type = HelloMessage { node_type: 99, ..valid.clone() };
        assert!(invalid_type.validate().is_err());
    }

    #[test]
    fn hello_rejects_zero_peer_id() {
        let msg = HelloMessage {
            version: 1,
            peer_id: [0u8; 32],
            best_height: 0,
            best_hash: coinject_core::Hash::ZERO,
            genesis_hash: coinject_core::Hash::ZERO,
            node_type: 1,
            timestamp: 0,
            connection_nonce: 0,
            ed25519_pubkey: [0u8; 32],
            auth_signature: [0u8; 64],
        };
        assert!(msg.validate().is_err());
    }

    #[test]
    fn get_blocks_range_validated() {
        let valid = GetBlocksMessage { from_height: 10, to_height: 20, request_id: 1 };
        assert!(valid.validate().is_ok());

        let inverted = GetBlocksMessage { from_height: 50, to_height: 10, request_id: 1 };
        assert!(inverted.validate().is_err());

        // Exceeds limit (513 blocks)
        let too_big = GetBlocksMessage { from_height: 0, to_height: 512, request_id: 1 };
        assert!(too_big.validate().is_err());
    }

    #[test]
    fn get_headers_validated() {
        let valid = GetHeadersMessage { from_height: 0, max_headers: 100, request_id: 1 };
        assert!(valid.validate().is_ok());

        let too_many = GetHeadersMessage { from_height: 0, max_headers: 9999, request_id: 1 };
        assert!(too_many.validate().is_err());
    }

    #[test]
    fn disconnect_reason_length_validated() {
        let ok = DisconnectMessage { reason: "normal".to_string() };
        assert!(ok.validate().is_ok());

        let too_long = DisconnectMessage { reason: "x".repeat(300) };
        assert!(too_long.validate().is_err());
    }

    #[test]
    fn work_rejected_reason_length_validated() {
        let ok = WorkRejectedMessage {
            block_hash: coinject_core::Hash::ZERO,
            reason: "stale block".to_string(),
        };
        assert!(ok.validate().is_ok());

        let too_long = WorkRejectedMessage {
            block_hash: coinject_core::Hash::ZERO,
            reason: "y".repeat(300),
        };
        assert!(too_long.validate().is_err());
    }

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

    // =========================================================================
    // Serialization round-trip tests
    // =========================================================================

    #[test]
    fn test_hello_message_bincode_roundtrip() {
        use coinject_core::Hash;
        let msg = HelloMessage {
            version: 1,
            peer_id: [0xABu8; 32],
            best_height: 42,
            best_hash: Hash::new(b"best"),
            genesis_hash: Hash::new(b"genesis"),
            node_type: 2,
            timestamp: 1_700_000_000,
            connection_nonce: 99,
            ed25519_pubkey: [0u8; 32],
            auth_signature: [0u8; 64],
        };
        let bytes = bincode::serialize(&msg).expect("HelloMessage must serialize");
        let recovered: HelloMessage = bincode::deserialize(&bytes).expect("HelloMessage must deserialize");
        assert_eq!(recovered.version, msg.version);
        assert_eq!(recovered.peer_id, msg.peer_id);
        assert_eq!(recovered.best_height, msg.best_height);
        assert_eq!(recovered.best_hash, msg.best_hash);
        assert_eq!(recovered.genesis_hash, msg.genesis_hash);
        assert_eq!(recovered.connection_nonce, msg.connection_nonce);
    }

    #[test]
    fn test_ping_pong_bincode_roundtrip() {
        let ping = PingMessage { timestamp: 123_456, nonce: 7_890 };
        let bytes = bincode::serialize(&ping).unwrap();
        let recovered: PingMessage = bincode::deserialize(&bytes).unwrap();
        assert_eq!(recovered.timestamp, ping.timestamp);
        assert_eq!(recovered.nonce, ping.nonce);

        let pong = PongMessage { timestamp: 123_456, nonce: 7_890 };
        let bytes = bincode::serialize(&pong).unwrap();
        let recovered: PongMessage = bincode::deserialize(&bytes).unwrap();
        assert_eq!(recovered.nonce, pong.nonce);
    }

    #[test]
    fn test_get_blocks_message_roundtrip() {
        let msg = GetBlocksMessage { from_height: 10, to_height: 50, request_id: 42 };
        let bytes = bincode::serialize(&msg).unwrap();
        let recovered: GetBlocksMessage = bincode::deserialize(&bytes).unwrap();
        assert_eq!(recovered.from_height, 10);
        assert_eq!(recovered.to_height, 50);
        assert_eq!(recovered.request_id, 42);
    }

    #[test]
    fn test_disconnect_message_roundtrip() {
        let msg = DisconnectMessage { reason: "too many peers".to_string() };
        let bytes = bincode::serialize(&msg).unwrap();
        let recovered: DisconnectMessage = bincode::deserialize(&bytes).unwrap();
        assert_eq!(recovered.reason, msg.reason);
    }

    #[test]
    fn test_message_size_limit_awareness() {
        // Verify HelloMessage can be measured — network layer enforces a limit
        use coinject_core::Hash;
        let msg = HelloMessage {
            version: 1,
            peer_id: [0u8; 32],
            best_height: 0,
            best_hash: Hash::ZERO,
            genesis_hash: Hash::ZERO,
            node_type: 0,
            timestamp: 0,
            connection_nonce: 0,
            ed25519_pubkey: [0u8; 32],
            auth_signature: [0u8; 64],
        };
        let bytes = bincode::serialize(&msg).unwrap();
        // A basic handshake must be well under 1 MB
        assert!(bytes.len() < 1024 * 1024, "HelloMessage must fit in 1 MB");
    }

    #[test]
    fn test_all_message_types_have_unique_byte_values() {
        let all_types = [
            MessageType::Hello,
            MessageType::HelloAck,
            MessageType::Status,
            MessageType::GetBlocks,
            MessageType::Blocks,
            MessageType::GetHeaders,
            MessageType::Headers,
            MessageType::NewBlock,
            MessageType::NewTransaction,
            MessageType::SubmitWork,
            MessageType::WorkAccepted,
            MessageType::WorkRejected,
            MessageType::GetWork,
            MessageType::Work,
            MessageType::Ping,
            MessageType::Pong,
            MessageType::Disconnect,
        ];
        let mut seen = std::collections::HashSet::new();
        for mt in &all_types {
            let byte = *mt as u8;
            assert!(seen.insert(byte), "Duplicate byte value 0x{:02X} for {:?}", byte, mt);
        }
    }

    #[test]
    fn test_unknown_message_type_returns_error() {
        // 0x99 is not a defined message type
        let result = MessageType::from_u8(0x99);
        assert!(result.is_err(), "Unknown byte must return Err");
    }
}
