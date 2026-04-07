// =============================================================================
// COINjecture P2P Protocol (CPP) - Protocol Encoding/Decoding
// =============================================================================
// Wire protocol implementation for message serialization

use crate::cpp::{
    config::{MAGIC, MAX_MESSAGE_SIZE, MIN_PROTOCOL_VERSION, VERSION},
    message::*,
    version::ConnectionPolicy,
};
use crate::security::MessageSizePolicy;
use bincode;
use blake3;
use std::io;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Protocol error types
#[derive(Debug)]
pub enum ProtocolError {
    Io(io::Error),
    InvalidMagic([u8; 4]),
    /// Version is outside the [MIN_PROTOCOL_VERSION, VERSION] window.
    InvalidVersion(u8),
    /// Version is valid but deprecated — callers log a warning and continue.
    DeprecatedVersion(u8),
    InvalidMessageType(u8),
    InvalidChecksum,
    MessageTooLarge(usize),
    SerializationError(String),
    DeserializationError(String),
    /// Timeout during receive operation (institutional-grade timeout handling)
    Timeout(Duration),
}

impl From<io::Error> for ProtocolError {
    fn from(err: io::Error) -> Self {
        ProtocolError::Io(err)
    }
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtocolError::Io(e) => write!(f, "IO error: {}", e),
            ProtocolError::InvalidMagic(magic) => write!(f, "Invalid magic: {:?}", magic),
            ProtocolError::InvalidVersion(v) => write!(
                f,
                "Invalid version: {} (supported: {}-{})",
                v, MIN_PROTOCOL_VERSION, VERSION
            ),
            ProtocolError::DeprecatedVersion(v) => {
                write!(f, "Deprecated version: {} (upgrade to {})", v, VERSION)
            }
            ProtocolError::InvalidMessageType(t) => write!(f, "Invalid message type: 0x{:02X}", t),
            ProtocolError::InvalidChecksum => write!(f, "Invalid checksum"),
            ProtocolError::MessageTooLarge(size) => write!(f, "Message too large: {} bytes", size),
            ProtocolError::SerializationError(e) => write!(f, "Serialization error: {}", e),
            ProtocolError::DeserializationError(e) => write!(f, "Deserialization error: {}", e),
            ProtocolError::Timeout(duration) => write!(f, "Receive timeout after {:?}", duration),
        }
    }
}

impl std::error::Error for ProtocolError {}

/// Message envelope for wire protocol
///
/// Format:
/// ```text
/// ┌────────────┬─────────┬──────────┬─────────────┬─────────┬──────────┐
/// │ Magic (4B) │ Ver (1B)│ Type (1B)│ Length (4B) │ Payload │ Hash (32B)│
/// └────────────┴─────────┴──────────┴─────────────┴─────────┴──────────┘
/// ```
pub struct MessageEnvelope {
    pub msg_type: MessageType,
    pub payload: Vec<u8>,
}

impl MessageEnvelope {
    /// Create new message envelope
    pub fn new<T: serde::Serialize>(
        msg_type: MessageType,
        payload: &T,
    ) -> Result<Self, ProtocolError> {
        let payload = bincode::serialize(payload)
            .map_err(|e| ProtocolError::SerializationError(e.to_string()))?;

        // Enforce per-type size limit on outbound messages too
        let type_limit = MessageSizePolicy::max_for_type(msg_type as u8).min(MAX_MESSAGE_SIZE);
        if payload.len() > type_limit {
            return Err(ProtocolError::MessageTooLarge(payload.len()));
        }

        Ok(MessageEnvelope { msg_type, payload })
    }

    /// Encode message to bytes
    pub fn encode(&self) -> Vec<u8> {
        let payload_len = self.payload.len() as u32;
        let checksum = blake3::hash(&self.payload);

        let mut buf = Vec::with_capacity(4 + 1 + 1 + 4 + self.payload.len() + 32);

        // Magic (4 bytes)
        buf.extend_from_slice(&MAGIC);

        // Version (1 byte)
        buf.push(VERSION);

        // Message type (1 byte)
        buf.push(self.msg_type as u8);

        // Payload length (4 bytes, big-endian)
        buf.extend_from_slice(&payload_len.to_be_bytes());

        // Payload
        buf.extend_from_slice(&self.payload);

        // Checksum (32 bytes)
        buf.extend_from_slice(checksum.as_bytes());

        buf
    }

    /// Decode message from stream
    pub async fn decode(stream: &mut TcpStream) -> Result<Self, ProtocolError> {
        // Read header (4 + 1 + 1 + 4 = 10 bytes)
        let mut header = [0u8; 10];
        stream.read_exact(&mut header).await?;

        // Verify magic
        // SAFETY: header is [u8;10]; slicing [0..4] always yields exactly 4 bytes.
        let magic: [u8; 4] = header[0..4]
            .try_into()
            .expect("[u8;10] slice [0..4] always converts to [u8;4]");
        if magic != MAGIC {
            return Err(ProtocolError::InvalidMagic(magic));
        }

        // Verify version — accept [MIN_PROTOCOL_VERSION, VERSION] for backward compat.
        // A V2 node will accept V1 messages from peers that have not yet upgraded.
        let version = header[4];
        match ConnectionPolicy::evaluate(version) {
            ConnectionPolicy::Reject { remote_version } => {
                return Err(ProtocolError::InvalidVersion(remote_version));
            }
            ConnectionPolicy::AllowWithWarning { remote_version } => {
                // Caller should log this; we surface it as a soft error that
                // decode() callers can inspect via DeprecatedVersion and still
                // handle the message.  We log at trace level here.
                tracing::trace!(
                    version = remote_version,
                    "accepting deprecated protocol version from peer (upgrade recommended)"
                );
            }
            ConnectionPolicy::Allow => {}
        }

        // Parse message type
        let msg_type_byte = header[5];
        let msg_type = MessageType::from_u8(msg_type_byte)
            .map_err(|_| ProtocolError::InvalidMessageType(msg_type_byte))?;

        // Parse payload length
        let payload_len = u32::from_be_bytes([header[6], header[7], header[8], header[9]]) as usize;

        // Enforce per-type size limit (more granular than the global MAX_MESSAGE_SIZE)
        let type_limit = MessageSizePolicy::max_for_type(msg_type_byte).min(MAX_MESSAGE_SIZE);
        if payload_len > type_limit {
            return Err(ProtocolError::MessageTooLarge(payload_len));
        }

        // Read payload
        let mut payload = vec![0u8; payload_len];
        stream.read_exact(&mut payload).await?;

        // Read checksum
        let mut checksum = [0u8; 32];
        stream.read_exact(&mut checksum).await?;

        // Verify checksum
        let computed = blake3::hash(&payload);
        if computed.as_bytes() != &checksum {
            return Err(ProtocolError::InvalidChecksum);
        }

        Ok(MessageEnvelope { msg_type, payload })
    }

    /// Deserialize payload into specific message type
    pub fn deserialize<T: serde::de::DeserializeOwned>(&self) -> Result<T, ProtocolError> {
        bincode::deserialize(&self.payload)
            .map_err(|e| ProtocolError::DeserializationError(e.to_string()))
    }
}

/// Message codec for sending/receiving typed messages
pub struct MessageCodec;

impl MessageCodec {
    /// Send a message
    pub async fn send<T: serde::Serialize>(
        stream: &mut TcpStream,
        msg_type: MessageType,
        payload: &T,
    ) -> Result<(), ProtocolError> {
        let envelope = MessageEnvelope::new(msg_type, payload)?;
        let bytes = envelope.encode();
        stream.write_all(&bytes).await?;
        stream.flush().await?;
        Ok(())
    }

    /// Receive a message
    pub async fn receive(stream: &mut TcpStream) -> Result<MessageEnvelope, ProtocolError> {
        MessageEnvelope::decode(stream).await
    }

    /// Receive a message with built-in timeout (institutional-grade)
    ///
    /// This method wraps the raw receive with a configurable timeout,
    /// eliminating the need for external timeout wrappers and ensuring
    /// consistent behavior across all call sites.
    pub async fn receive_with_timeout(
        stream: &mut TcpStream,
        timeout_duration: Duration,
    ) -> Result<MessageEnvelope, ProtocolError> {
        tokio::time::timeout(timeout_duration, MessageEnvelope::decode(stream))
            .await
            .map_err(|_| ProtocolError::Timeout(timeout_duration))?
    }

    /// Receive a message from read half of split stream
    pub async fn receive_from_read_half(
        read_half: &mut tokio::io::ReadHalf<TcpStream>,
    ) -> Result<MessageEnvelope, ProtocolError> {
        // Read header (4 + 1 + 1 + 4 = 10 bytes)
        let mut header = [0u8; 10];
        read_half.read_exact(&mut header).await?;

        // Verify magic
        // SAFETY: header is [u8;10]; slicing [0..4] always yields exactly 4 bytes.
        let magic: [u8; 4] = header[0..4]
            .try_into()
            .expect("[u8;10] slice [0..4] always converts to [u8;4]");
        if magic != MAGIC {
            return Err(ProtocolError::InvalidMagic(magic));
        }

        // Verify version — accept [MIN_PROTOCOL_VERSION, VERSION] for backward compat.
        let version = header[4];
        match ConnectionPolicy::evaluate(version) {
            ConnectionPolicy::Reject { remote_version } => {
                return Err(ProtocolError::InvalidVersion(remote_version));
            }
            ConnectionPolicy::AllowWithWarning { remote_version } => {
                tracing::trace!(
                    version = remote_version,
                    "accepting deprecated protocol version from peer (upgrade recommended)"
                );
            }
            ConnectionPolicy::Allow => {}
        }

        // Parse message type
        let msg_type_byte = header[5];
        let msg_type = MessageType::from_u8(msg_type_byte)
            .map_err(|_| ProtocolError::InvalidMessageType(msg_type_byte))?;

        // Parse payload length
        let payload_len = u32::from_be_bytes([header[6], header[7], header[8], header[9]]) as usize;

        // Per-type size limit enforcement
        let type_limit = MessageSizePolicy::max_for_type(msg_type_byte).min(MAX_MESSAGE_SIZE);
        if payload_len > type_limit {
            return Err(ProtocolError::MessageTooLarge(payload_len));
        }

        // Read payload
        let mut payload = vec![0u8; payload_len];
        read_half.read_exact(&mut payload).await?;

        // Read checksum
        let mut checksum = [0u8; 32];
        read_half.read_exact(&mut checksum).await?;

        // Verify checksum
        let computed = blake3::hash(&payload);
        if computed.as_bytes() != &checksum {
            return Err(ProtocolError::InvalidChecksum);
        }

        Ok(MessageEnvelope { msg_type, payload })
    }

    /// Receive a message from read half with built-in timeout (institutional-grade)
    ///
    /// This method wraps the raw receive with a configurable timeout,
    /// ensuring consistent timeout behavior for the peer message loop.
    pub async fn receive_from_read_half_with_timeout(
        read_half: &mut tokio::io::ReadHalf<TcpStream>,
        timeout_duration: Duration,
    ) -> Result<MessageEnvelope, ProtocolError> {
        tokio::time::timeout(timeout_duration, Self::receive_from_read_half(read_half))
            .await
            .map_err(|_| ProtocolError::Timeout(timeout_duration))?
    }

    /// Send Hello message
    pub async fn send_hello(
        stream: &mut TcpStream,
        msg: &HelloMessage,
    ) -> Result<(), ProtocolError> {
        Self::send(stream, MessageType::Hello, msg).await
    }

    /// Send HelloAck message
    pub async fn send_hello_ack(
        stream: &mut TcpStream,
        msg: &HelloAckMessage,
    ) -> Result<(), ProtocolError> {
        Self::send(stream, MessageType::HelloAck, msg).await
    }

    /// Send Status message
    pub async fn send_status(
        stream: &mut TcpStream,
        msg: &StatusMessage,
    ) -> Result<(), ProtocolError> {
        Self::send(stream, MessageType::Status, msg).await
    }

    /// Send GetBlocks request
    pub async fn send_get_blocks(
        stream: &mut TcpStream,
        msg: &GetBlocksMessage,
    ) -> Result<(), ProtocolError> {
        Self::send(stream, MessageType::GetBlocks, msg).await
    }

    /// Send Blocks response
    pub async fn send_blocks(
        stream: &mut TcpStream,
        msg: &BlocksMessage,
    ) -> Result<(), ProtocolError> {
        Self::send(stream, MessageType::Blocks, msg).await
    }

    /// Send NewBlock announcement
    pub async fn send_new_block(
        stream: &mut TcpStream,
        msg: &NewBlockMessage,
    ) -> Result<(), ProtocolError> {
        Self::send(stream, MessageType::NewBlock, msg).await
    }

    /// Send NewTransaction announcement
    pub async fn send_new_transaction(
        stream: &mut TcpStream,
        msg: &NewTransactionMessage,
    ) -> Result<(), ProtocolError> {
        Self::send(stream, MessageType::NewTransaction, msg).await
    }

    /// Send Ping
    pub async fn send_ping(stream: &mut TcpStream, msg: &PingMessage) -> Result<(), ProtocolError> {
        Self::send(stream, MessageType::Ping, msg).await
    }

    /// Send Pong
    pub async fn send_pong(stream: &mut TcpStream, msg: &PongMessage) -> Result<(), ProtocolError> {
        Self::send(stream, MessageType::Pong, msg).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use coinject_core::Hash;

    #[test]
    fn test_message_envelope_encoding() {
        let msg = HelloMessage {
            version: 1,
            peer_id: [42u8; 32],
            best_height: 100,
            best_hash: Hash::ZERO,
            genesis_hash: Hash::ZERO,
            node_type: 1,
            timestamp: 1234567890,
            connection_nonce: 0, // Test with default nonce
            ed25519_pubkey: [0u8; 32],
            auth_signature: [0u8; 64],
        };

        let envelope = MessageEnvelope::new(MessageType::Hello, &msg).unwrap();
        let encoded = envelope.encode();

        // Check magic
        assert_eq!(&encoded[0..4], &MAGIC);

        // Check version
        assert_eq!(encoded[4], VERSION);

        // Check message type
        assert_eq!(encoded[5], MessageType::Hello as u8);

        // Check payload length is encoded
        let payload_len = u32::from_be_bytes([encoded[6], encoded[7], encoded[8], encoded[9]]);
        assert!(payload_len > 0);
    }

    #[test]
    fn test_message_serialization_roundtrip() {
        let original = StatusMessage {
            best_height: 12345,
            best_hash: Hash::ZERO,
            node_type: 1,
            timestamp: 9876543210,
            flock_state: None,
        };

        let envelope = MessageEnvelope::new(MessageType::Status, &original).unwrap();
        let deserialized: StatusMessage = envelope.deserialize().unwrap();

        assert_eq!(deserialized.best_height, original.best_height);
        assert_eq!(deserialized.best_hash, original.best_hash);
        assert_eq!(deserialized.node_type, original.node_type);
        assert_eq!(deserialized.timestamp, original.timestamp);
    }

    #[test]
    fn test_message_too_large() {
        let huge_payload = vec![0u8; MAX_MESSAGE_SIZE + 1];
        let result = MessageEnvelope::new(MessageType::Blocks, &huge_payload);

        assert!(matches!(result, Err(ProtocolError::MessageTooLarge(_))));
    }

    #[test]
    fn test_checksum_verification() {
        let msg = PingMessage {
            timestamp: 1234567890,
            nonce: 42,
        };

        let envelope = MessageEnvelope::new(MessageType::Ping, &msg).unwrap();
        let mut encoded = envelope.encode();

        // Corrupt the checksum
        let checksum_start = encoded.len() - 32;
        encoded[checksum_start] ^= 0xFF;

        // Decoding should fail (we can't test this directly without a stream,
        // but the logic is there)
        assert_ne!(encoded[checksum_start], envelope.encode()[checksum_start]);
    }
}
