//! Dual-mode P2P connection supporting both Noise-encrypted and legacy transports.
//!
//! New nodes use Noise_XX; old nodes fall back to the existing custom encryption
//! in `cpp/encryption.rs`. This wrapper provides a uniform send/recv interface.

use crate::noise::{NoiseConnection, NoiseKeypair};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// A P2P connection that operates in either encrypted or legacy mode.
pub enum PeerConnection {
    /// Noise_XX encrypted connection (preferred).
    Encrypted(NoiseConnection),
    /// Legacy plaintext with length-prefixed framing.
    Legacy(LegacyConnection),
}

pub struct LegacyConnection {
    pub stream: TcpStream,
}

impl PeerConnection {
    /// Attempt a Noise-encrypted outbound connection.
    pub async fn connect_encrypted(
        addr: SocketAddr,
        local_keypair: &NoiseKeypair,
        timeout: Duration,
    ) -> Result<Self, ConnectionError> {
        let stream = tokio::time::timeout(timeout, TcpStream::connect(addr))
            .await
            .map_err(|_| ConnectionError::Timeout)?
            .map_err(|e| ConnectionError::Io(e.to_string()))?;

        let conn = NoiseConnection::connect(stream, local_keypair)
            .await
            .map_err(|e| ConnectionError::HandshakeFailed(e.to_string()))?;

        tracing::info!(%addr, "Connected with Noise encryption");
        Ok(PeerConnection::Encrypted(conn))
    }

    /// Accept an inbound Noise-encrypted connection.
    pub async fn accept_encrypted(
        stream: TcpStream,
        local_keypair: &NoiseKeypair,
    ) -> Result<Self, ConnectionError> {
        let addr = stream
            .peer_addr()
            .map_err(|e| ConnectionError::Io(e.to_string()))?;

        let conn = NoiseConnection::accept(stream, local_keypair)
            .await
            .map_err(|e| ConnectionError::HandshakeFailed(e.to_string()))?;

        tracing::info!(%addr, "Accepted with Noise encryption");
        Ok(PeerConnection::Encrypted(conn))
    }

    /// Create a legacy (unencrypted, length-prefixed) connection.
    pub fn legacy(stream: TcpStream) -> Self {
        PeerConnection::Legacy(LegacyConnection { stream })
    }

    /// Send a serialized message.
    pub async fn send_message(&mut self, data: &[u8]) -> Result<(), ConnectionError> {
        match self {
            PeerConnection::Encrypted(conn) => conn
                .send(data)
                .await
                .map_err(|e| ConnectionError::Send(e.to_string())),
            PeerConnection::Legacy(conn) => {
                conn.stream
                    .write_all(&(data.len() as u32).to_be_bytes())
                    .await
                    .map_err(|e| ConnectionError::Send(e.to_string()))?;
                conn.stream
                    .write_all(data)
                    .await
                    .map_err(|e| ConnectionError::Send(e.to_string()))?;
                conn.stream
                    .flush()
                    .await
                    .map_err(|e| ConnectionError::Send(e.to_string()))?;
                Ok(())
            }
        }
    }

    /// Receive a message.
    pub async fn recv_message(&mut self) -> Result<Vec<u8>, ConnectionError> {
        match self {
            PeerConnection::Encrypted(conn) => conn
                .recv()
                .await
                .map_err(|e| ConnectionError::Recv(e.to_string())),
            PeerConnection::Legacy(conn) => {
                let mut len_bytes = [0u8; 4];
                conn.stream
                    .read_exact(&mut len_bytes)
                    .await
                    .map_err(|e| ConnectionError::Recv(e.to_string()))?;
                let len = u32::from_be_bytes(len_bytes) as usize;
                if len > 4 * 1024 * 1024 {
                    return Err(ConnectionError::Recv("Message too large".into()));
                }
                let mut buf = vec![0u8; len];
                conn.stream
                    .read_exact(&mut buf)
                    .await
                    .map_err(|e| ConnectionError::Recv(e.to_string()))?;
                Ok(buf)
            }
        }
    }

    pub fn is_encrypted(&self) -> bool {
        matches!(self, PeerConnection::Encrypted(_))
    }

    pub fn remote_public_key(&self) -> Option<&[u8; 32]> {
        match self {
            PeerConnection::Encrypted(conn) => Some(conn.remote_public_key()),
            PeerConnection::Legacy(_) => None,
        }
    }

    pub fn peer_addr(&self) -> std::io::Result<SocketAddr> {
        match self {
            PeerConnection::Encrypted(conn) => conn.peer_addr(),
            PeerConnection::Legacy(conn) => conn.stream.peer_addr(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConnectionError {
    #[error("Connection timeout")]
    Timeout,
    #[error("IO error: {0}")]
    Io(String),
    #[error("Handshake failed: {0}")]
    HandshakeFailed(String),
    #[error("Send error: {0}")]
    Send(String),
    #[error("Recv error: {0}")]
    Recv(String),
}
