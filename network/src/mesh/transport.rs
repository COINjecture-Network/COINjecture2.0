// =============================================================================
// TCP Transport & Length-Prefixed Framing
// =============================================================================
//
// Handles raw TCP connections with 4-byte big-endian length-prefixed framing.
// Provides async read/write of WireMessage over a split TCP stream.
// Rejects messages exceeding max_message_size at the framing layer.

use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use super::error::NetworkError;
use super::protocol::WireMessage;

/// Maximum frame header: 4 bytes for length prefix.
const LENGTH_PREFIX_SIZE: usize = 4;

/// Bind a TCP listener on the given address.
pub async fn bind_listener(addr: SocketAddr) -> Result<TcpListener, NetworkError> {
    TcpListener::bind(addr).await.map_err(NetworkError::Io)
}

/// Connect to a remote address with a timeout.
pub async fn dial(
    addr: SocketAddr,
    timeout: std::time::Duration,
) -> Result<TcpStream, NetworkError> {
    match tokio::time::timeout(timeout, TcpStream::connect(addr)).await {
        Ok(Ok(stream)) => {
            stream.set_nodelay(true).ok();
            Ok(stream)
        }
        Ok(Err(e)) => Err(NetworkError::Io(e)),
        Err(_) => Err(NetworkError::ConnectionTimeout(addr)),
    }
}

/// Write a length-prefixed frame containing a serialized WireMessage.
///
/// Frame format: [4-byte BE length][bincode payload]
pub async fn write_message<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    msg: &WireMessage,
    max_size: usize,
) -> Result<(), NetworkError> {
    let bytes = bincode::serialize(msg)?;
    if bytes.len() > max_size {
        return Err(NetworkError::MessageTooLarge {
            size: bytes.len(),
            max: max_size,
        });
    }
    let len = (bytes.len() as u32).to_be_bytes();
    writer.write_all(&len).await.map_err(NetworkError::Io)?;
    writer.write_all(&bytes).await.map_err(NetworkError::Io)?;
    writer.flush().await.map_err(NetworkError::Io)?;
    Ok(())
}

/// Read a length-prefixed frame and deserialize it into a WireMessage.
///
/// Returns None if the connection is cleanly closed (EOF on length prefix).
pub async fn read_message<R: AsyncReadExt + Unpin>(
    reader: &mut R,
    max_size: usize,
) -> Result<Option<WireMessage>, NetworkError> {
    // Read the 4-byte length prefix
    let mut len_buf = [0u8; LENGTH_PREFIX_SIZE];
    match reader.read_exact(&mut len_buf).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(NetworkError::Io(e)),
    }

    let len = u32::from_be_bytes(len_buf) as usize;
    if len > max_size {
        return Err(NetworkError::MessageTooLarge {
            size: len,
            max: max_size,
        });
    }
    if len == 0 {
        return Err(NetworkError::ProtocolViolation("zero-length frame".into()));
    }

    // Read the payload
    let mut payload = vec![0u8; len];
    reader
        .read_exact(&mut payload)
        .await
        .map_err(NetworkError::Io)?;

    // Deserialize — catch malformed data
    let msg: WireMessage = bincode::deserialize(&payload).map_err(|e| {
        NetworkError::ProtocolViolation(format!("failed to deserialize frame: {}", e))
    })?;

    Ok(Some(msg))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::identity::NodeId;
    use crate::mesh::protocol::HandshakeMessage;

    #[tokio::test]
    async fn test_framing_roundtrip() {
        let msg = WireMessage::Handshake(HandshakeMessage::Hello {
            node_id: NodeId([0xAA; 32]),
            public_key: vec![0xBB; 32],
            challenge: [0xCC; 32],
            listen_addr: "127.0.0.1:9000".parse().unwrap(),
        });

        let mut buf = Vec::new();
        write_message(&mut buf, &msg, 16 * 1024 * 1024)
            .await
            .unwrap();

        let mut cursor = std::io::Cursor::new(buf);
        let decoded = read_message(&mut cursor, 16 * 1024 * 1024)
            .await
            .unwrap()
            .expect("should decode");

        match decoded {
            WireMessage::Handshake(HandshakeMessage::Hello { node_id, .. }) => {
                assert_eq!(node_id, NodeId([0xAA; 32]));
            }
            _ => panic!("wrong variant"),
        }
    }

    #[tokio::test]
    async fn test_rejects_oversized_frame() {
        let msg = WireMessage::Handshake(HandshakeMessage::Hello {
            node_id: NodeId([0; 32]),
            public_key: vec![0; 32],
            challenge: [0; 32],
            listen_addr: "127.0.0.1:9000".parse().unwrap(),
        });

        let mut buf = Vec::new();
        // Try writing with a tiny max size
        let result = write_message(&mut buf, &msg, 10).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            NetworkError::MessageTooLarge { .. } => {}
            e => panic!("expected MessageTooLarge, got {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_eof_returns_none() {
        let buf: Vec<u8> = Vec::new();
        let mut cursor = std::io::Cursor::new(buf);
        let result = read_message(&mut cursor, 16 * 1024 * 1024).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_rejects_oversized_incoming_frame() {
        // Craft a frame header claiming a large size
        let mut buf = Vec::new();
        buf.extend_from_slice(&(20_000_000u32).to_be_bytes()); // 20MB claim
        buf.extend_from_slice(&[0u8; 100]); // Some junk payload

        let mut cursor = std::io::Cursor::new(buf);
        let result = read_message(&mut cursor, 16 * 1024 * 1024).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            NetworkError::MessageTooLarge { .. } => {}
            e => panic!("expected MessageTooLarge, got {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_tcp_roundtrip() {
        let listener = bind_listener("127.0.0.1:0".parse().unwrap()).await.unwrap();
        let addr = listener.local_addr().unwrap();
        let max_size = 16 * 1024 * 1024;

        let msg = WireMessage::Handshake(HandshakeMessage::Hello {
            node_id: NodeId([0x01; 32]),
            public_key: vec![0x02; 32],
            challenge: [0x03; 32],
            listen_addr: addr,
        });

        let msg_clone = msg.clone();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let received = read_message(&mut stream, max_size)
                .await
                .unwrap()
                .expect("should read");
            received
        });

        let client = tokio::spawn(async move {
            let mut stream = dial(addr, std::time::Duration::from_secs(2)).await.unwrap();
            write_message(&mut stream, &msg_clone, max_size)
                .await
                .unwrap();
        });

        client.await.unwrap();
        let received = server.await.unwrap();
        match received {
            WireMessage::Handshake(HandshakeMessage::Hello { node_id, .. }) => {
                assert_eq!(node_id, NodeId([0x01; 32]));
            }
            _ => panic!("wrong variant"),
        }
    }
}
