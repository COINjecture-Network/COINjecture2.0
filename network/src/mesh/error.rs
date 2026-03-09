// =============================================================================
// Mesh Network Error Types
// =============================================================================
//
// Thiserror-based error hierarchy for the mesh networking layer.
// Every public function returns Result<T, NetworkError>.

use std::net::SocketAddr;
use thiserror::Error;

/// Top-level error type for the mesh networking layer.
#[derive(Error, Debug)]
pub enum NetworkError {
    /// I/O error from TCP or filesystem operations.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Failed to serialize or deserialize a message.
    #[error("serialization error: {0}")]
    Serialization(String),

    /// Cryptographic operation failed (signing, verification, key loading).
    #[error("crypto error: {0}")]
    Crypto(String),

    /// The handshake with a peer failed or was rejected.
    #[error("handshake failed with {addr}: {reason}")]
    HandshakeFailed { addr: SocketAddr, reason: String },

    /// A message exceeded the maximum allowed size (16 MB).
    #[error("message too large: {size} bytes (max {max})")]
    MessageTooLarge { size: usize, max: usize },

    /// A message had an invalid or unverifiable signature.
    #[error("invalid signature from {sender}")]
    InvalidSignature { sender: String },

    /// The peer sent a malformed or unparseable message.
    #[error("protocol violation: {0}")]
    ProtocolViolation(String),

    /// A connection to a peer timed out.
    #[error("connection timeout to {0}")]
    ConnectionTimeout(SocketAddr),

    /// The network service has been shut down.
    #[error("network service shut down")]
    Shutdown,

    /// The command/event channel is closed.
    #[error("channel closed: {0}")]
    ChannelClosed(String),

    /// Rate limit exceeded for a peer.
    #[error("rate limit exceeded for peer {0}")]
    RateLimited(String),

    /// Generic catch-all for unexpected conditions.
    #[error("{0}")]
    Other(String),
}

impl From<bincode::Error> for NetworkError {
    fn from(e: bincode::Error) -> Self {
        NetworkError::Serialization(e.to_string())
    }
}
