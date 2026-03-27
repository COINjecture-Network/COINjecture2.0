//! Noise Protocol Framework transport layer using the `snow` crate.
//!
//! Provides Noise_XX_25519_ChaChaPoly_SHA256 encrypted connections.
//! This sits alongside the existing custom encryption in `cpp/encryption.rs`
//! and can be used as a drop-in replacement for formally-auditable security.

use sha2::{Digest, Sha256};
use snow::{Builder, TransportState};
use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Noise protocol parameters for COINjecture P2P.
pub const NOISE_PARAMS: &str = "Noise_XX_25519_ChaChaPoly_SHA256";

/// Maximum Noise payload before encryption overhead (16-byte AEAD tag).
pub const MAX_NOISE_MSG_LEN: usize = 65535 - 16;

// ── Keypair ─────────────────────────────────────────────────────────────────

/// A static Noise keypair for this node, persisted to disk.
#[derive(Clone)]
pub struct NoiseKeypair {
    pub private_key: [u8; 32],
    pub public_key: [u8; 32],
}

impl NoiseKeypair {
    /// Generate a new random Noise keypair.
    pub fn generate() -> Self {
        let params: snow::params::NoiseParams = NOISE_PARAMS.parse().unwrap();
        let builder = Builder::new(params);
        let kp = builder.generate_keypair().unwrap();
        let mut private_key = [0u8; 32];
        let mut public_key = [0u8; 32];
        private_key.copy_from_slice(&kp.private);
        public_key.copy_from_slice(&kp.public);
        Self {
            private_key,
            public_key,
        }
    }

    /// Load from a file, or generate and save if not found.
    pub fn load_or_generate(path: &Path) -> Result<Self, NoiseError> {
        if path.exists() {
            let bytes = std::fs::read(path).map_err(|e| NoiseError::KeyLoad(e.to_string()))?;
            if bytes.len() != 64 {
                return Err(NoiseError::KeyLoad("Invalid key file size".into()));
            }
            let mut private_key = [0u8; 32];
            let mut public_key = [0u8; 32];
            private_key.copy_from_slice(&bytes[..32]);
            public_key.copy_from_slice(&bytes[32..]);
            Ok(Self {
                private_key,
                public_key,
            })
        } else {
            let keypair = Self::generate();
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| NoiseError::KeyLoad(e.to_string()))?;
            }
            let mut bytes = Vec::with_capacity(64);
            bytes.extend_from_slice(&keypair.private_key);
            bytes.extend_from_slice(&keypair.public_key);
            std::fs::write(path, &bytes).map_err(|e| NoiseError::KeyLoad(e.to_string()))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
                    .map_err(|e| NoiseError::KeyLoad(e.to_string()))?;
            }
            Ok(keypair)
        }
    }

    /// Derive a peer_id from the public key (first 20 bytes of SHA-256).
    pub fn peer_id(&self) -> String {
        let hash = Sha256::digest(self.public_key);
        hex::encode(&hash[..20])
    }
}

// ── Connection ──────────────────────────────────────────────────────────────

/// An encrypted P2P connection using the Noise Protocol.
pub struct NoiseConnection {
    stream: TcpStream,
    transport: TransportState,
    remote_public_key: [u8; 32],
    write_buf: Vec<u8>,
}

impl NoiseConnection {
    /// Perform the Noise_XX handshake as the INITIATOR (outbound).
    pub async fn connect(
        mut stream: TcpStream,
        local_keypair: &NoiseKeypair,
    ) -> Result<Self, NoiseError> {
        let params: snow::params::NoiseParams = NOISE_PARAMS
            .parse()
            .map_err(|e: snow::error::Error| NoiseError::Handshake(format!("Invalid params: {e}")))?;

        let mut hs = Builder::new(params)
            .local_private_key(&local_keypair.private_key)
            .build_initiator()
            .map_err(|e| NoiseError::Handshake(format!("Builder: {e}")))?;

        let mut buf = vec![0u8; 65535];
        let mut read_buf = vec![0u8; 65535];

        // -> e
        let len = hs
            .write_message(&[], &mut buf)
            .map_err(|e| NoiseError::Handshake(format!("Msg1: {e}")))?;
        send_frame(&mut stream, &buf[..len]).await?;

        // <- e, ee, s, es
        let payload = recv_frame(&mut stream).await?;
        hs.read_message(&payload, &mut read_buf)
            .map_err(|e| NoiseError::Handshake(format!("Msg2: {e}")))?;

        // -> s, se
        let len = hs
            .write_message(&[], &mut buf)
            .map_err(|e| NoiseError::Handshake(format!("Msg3: {e}")))?;
        send_frame(&mut stream, &buf[..len]).await?;

        let remote_public_key: [u8; 32] = hs
            .get_remote_static()
            .ok_or_else(|| NoiseError::Handshake("No remote static key".into()))?
            .try_into()
            .map_err(|_| NoiseError::Handshake("Bad key length".into()))?;

        let transport = hs
            .into_transport_mode()
            .map_err(|e| NoiseError::Handshake(format!("Transport: {e}")))?;

        tracing::debug!(
            remote_key = hex::encode(remote_public_key),
            "Noise_XX handshake complete (initiator)"
        );

        Ok(Self {
            stream,
            transport,
            remote_public_key,
            write_buf: vec![0u8; 65535],
        })
    }

    /// Perform the Noise_XX handshake as the RESPONDER (inbound).
    pub async fn accept(
        mut stream: TcpStream,
        local_keypair: &NoiseKeypair,
    ) -> Result<Self, NoiseError> {
        let params: snow::params::NoiseParams = NOISE_PARAMS
            .parse()
            .map_err(|e: snow::error::Error| NoiseError::Handshake(format!("Invalid params: {e}")))?;

        let mut hs = Builder::new(params)
            .local_private_key(&local_keypair.private_key)
            .build_responder()
            .map_err(|e| NoiseError::Handshake(format!("Builder: {e}")))?;

        let mut buf = vec![0u8; 65535];
        let mut read_buf = vec![0u8; 65535];

        // <- e
        let payload = recv_frame(&mut stream).await?;
        hs.read_message(&payload, &mut read_buf)
            .map_err(|e| NoiseError::Handshake(format!("Msg1: {e}")))?;

        // -> e, ee, s, es
        let len = hs
            .write_message(&[], &mut buf)
            .map_err(|e| NoiseError::Handshake(format!("Msg2: {e}")))?;
        send_frame(&mut stream, &buf[..len]).await?;

        // <- s, se
        let payload = recv_frame(&mut stream).await?;
        hs.read_message(&payload, &mut read_buf)
            .map_err(|e| NoiseError::Handshake(format!("Msg3: {e}")))?;

        let remote_public_key: [u8; 32] = hs
            .get_remote_static()
            .ok_or_else(|| NoiseError::Handshake("No remote static key".into()))?
            .try_into()
            .map_err(|_| NoiseError::Handshake("Bad key length".into()))?;

        let transport = hs
            .into_transport_mode()
            .map_err(|e| NoiseError::Handshake(format!("Transport: {e}")))?;

        tracing::debug!(
            remote_key = hex::encode(remote_public_key),
            "Noise_XX handshake complete (responder)"
        );

        Ok(Self {
            stream,
            transport,
            remote_public_key,
            write_buf: vec![0u8; 65535],
        })
    }

    /// Send an encrypted message (length-prefixed + Noise AEAD).
    pub async fn send(&mut self, plaintext: &[u8]) -> Result<(), NoiseError> {
        if plaintext.len() > MAX_NOISE_MSG_LEN {
            return Err(NoiseError::MessageTooLarge(plaintext.len()));
        }
        let len = self
            .transport
            .write_message(plaintext, &mut self.write_buf)
            .map_err(|e| NoiseError::Encrypt(e.to_string()))?;
        send_frame(&mut self.stream, &self.write_buf[..len]).await
    }

    /// Receive and decrypt a message.
    pub async fn recv(&mut self) -> Result<Vec<u8>, NoiseError> {
        let ciphertext = recv_frame(&mut self.stream).await?;
        let mut plaintext = vec![0u8; 65535];
        let len = self
            .transport
            .read_message(&ciphertext, &mut plaintext)
            .map_err(|e| NoiseError::Decrypt(e.to_string()))?;
        plaintext.truncate(len);
        Ok(plaintext)
    }

    /// Remote peer's static Noise public key.
    pub fn remote_public_key(&self) -> &[u8; 32] {
        &self.remote_public_key
    }

    /// Remote peer's peer_id (sha256(pubkey)[..20] hex).
    pub fn remote_peer_id(&self) -> String {
        let hash = Sha256::digest(self.remote_public_key);
        hex::encode(&hash[..20])
    }

    pub fn peer_addr(&self) -> std::io::Result<std::net::SocketAddr> {
        self.stream.peer_addr()
    }
}

// ── Wire framing ────────────────────────────────────────────────────────────

/// Send a 4-byte big-endian length-prefixed frame.
async fn send_frame(stream: &mut TcpStream, data: &[u8]) -> Result<(), NoiseError> {
    let len = (data.len() as u32).to_be_bytes();
    stream
        .write_all(&len)
        .await
        .map_err(|e| NoiseError::Io(e.to_string()))?;
    stream
        .write_all(data)
        .await
        .map_err(|e| NoiseError::Io(e.to_string()))?;
    stream
        .flush()
        .await
        .map_err(|e| NoiseError::Io(e.to_string()))?;
    Ok(())
}

/// Receive a 4-byte big-endian length-prefixed frame.
async fn recv_frame(stream: &mut TcpStream) -> Result<Vec<u8>, NoiseError> {
    let mut len_bytes = [0u8; 4];
    stream
        .read_exact(&mut len_bytes)
        .await
        .map_err(|e| NoiseError::Io(e.to_string()))?;
    let len = u32::from_be_bytes(len_bytes) as usize;
    if len > 65535 || len == 0 {
        return Err(NoiseError::MessageTooLarge(len));
    }
    let mut buf = vec![0u8; len];
    stream
        .read_exact(&mut buf)
        .await
        .map_err(|e| NoiseError::Io(e.to_string()))?;
    Ok(buf)
}

// ── Errors ──────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum NoiseError {
    #[error("Handshake failed: {0}")]
    Handshake(String),
    #[error("Encryption failed: {0}")]
    Encrypt(String),
    #[error("Decryption failed: {0}")]
    Decrypt(String),
    #[error("IO error: {0}")]
    Io(String),
    #[error("Message too large: {0} bytes")]
    MessageTooLarge(usize),
    #[error("Key load/save error: {0}")]
    KeyLoad(String),
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_noise_keypair_generate() {
        let kp = NoiseKeypair::generate();
        assert_eq!(kp.private_key.len(), 32);
        assert_eq!(kp.public_key.len(), 32);
        assert_ne!(kp.private_key, kp.public_key);
    }

    #[test]
    fn test_noise_keypair_persistence() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_noise_key");

        let kp1 = NoiseKeypair::load_or_generate(&path).unwrap();
        let kp2 = NoiseKeypair::load_or_generate(&path).unwrap();

        assert_eq!(kp1.public_key, kp2.public_key);
        assert_eq!(kp1.private_key, kp2.private_key);
    }

    #[test]
    fn test_peer_id_derivation() {
        let kp = NoiseKeypair::generate();
        let id = kp.peer_id();
        assert_eq!(id.len(), 40); // 20 bytes = 40 hex chars

        // Deterministic
        assert_eq!(id, kp.peer_id());
    }

    #[test]
    fn test_snow_noise_xx_handshake() {
        let params: snow::params::NoiseParams = NOISE_PARAMS.parse().unwrap();

        let builder_i = Builder::new(params.clone());
        let kp_i = builder_i.generate_keypair().unwrap();
        let mut initiator = builder_i
            .local_private_key(&kp_i.private)
            .build_initiator()
            .unwrap();

        let builder_r = Builder::new(params);
        let kp_r = builder_r.generate_keypair().unwrap();
        let mut responder = builder_r
            .local_private_key(&kp_r.private)
            .build_responder()
            .unwrap();

        let mut buf = vec![0u8; 65535];
        let mut rbuf = vec![0u8; 65535];

        // -> e
        let len = initiator.write_message(&[], &mut buf).unwrap();
        responder.read_message(&buf[..len], &mut rbuf).unwrap();

        // <- e, ee, s, es
        let len = responder.write_message(&[], &mut buf).unwrap();
        initiator.read_message(&buf[..len], &mut rbuf).unwrap();

        // -> s, se
        let len = initiator.write_message(&[], &mut buf).unwrap();
        responder.read_message(&buf[..len], &mut rbuf).unwrap();

        let mut init_transport = initiator.into_transport_mode().unwrap();
        let mut resp_transport = responder.into_transport_mode().unwrap();

        // Encrypted message exchange
        let msg = b"Hello from COINjecture!";
        let len = init_transport.write_message(msg, &mut buf).unwrap();
        let mut pt = vec![0u8; 65535];
        let plen = resp_transport.read_message(&buf[..len], &mut pt).unwrap();
        assert_eq!(&pt[..plen], msg.as_slice());
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn test_noise_connection_over_tcp() {
        let kp_a = NoiseKeypair::generate();
        let kp_b = NoiseKeypair::generate();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server_kp = kp_b.clone();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            NoiseConnection::accept(stream, &server_kp).await.unwrap()
        });

        let stream = TcpStream::connect(addr).await.unwrap();
        let mut client = NoiseConnection::connect(stream, &kp_a).await.unwrap();
        let mut server = server.await.unwrap();

        // Bidirectional
        client.send(b"Hello from client").await.unwrap();
        let msg = server.recv().await.unwrap();
        assert_eq!(msg, b"Hello from client");

        server.send(b"Hello from server").await.unwrap();
        let msg = client.recv().await.unwrap();
        assert_eq!(msg, b"Hello from server");

        // Verify keys
        assert_eq!(client.remote_public_key(), &kp_b.public_key);
        assert_eq!(server.remote_public_key(), &kp_a.public_key);
    }

    #[tokio::test]
    async fn test_noise_large_message() {
        let kp_a = NoiseKeypair::generate();
        let kp_b = NoiseKeypair::generate();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server_kp = kp_b.clone();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            NoiseConnection::accept(stream, &server_kp).await.unwrap()
        });

        let stream = TcpStream::connect(addr).await.unwrap();
        let mut client = NoiseConnection::connect(stream, &kp_a).await.unwrap();
        let mut server = server.await.unwrap();

        let large_msg = vec![42u8; 60000];
        client.send(&large_msg).await.unwrap();
        let received = server.recv().await.unwrap();
        assert_eq!(received, large_msg);
    }

    #[tokio::test]
    async fn test_noise_multiple_messages() {
        let kp_a = NoiseKeypair::generate();
        let kp_b = NoiseKeypair::generate();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server_kp = kp_b.clone();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            NoiseConnection::accept(stream, &server_kp).await.unwrap()
        });

        let stream = TcpStream::connect(addr).await.unwrap();
        let mut client = NoiseConnection::connect(stream, &kp_a).await.unwrap();
        let mut server = server.await.unwrap();

        for i in 0..100u32 {
            let msg = format!("msg {i}");
            client.send(msg.as_bytes()).await.unwrap();
            assert_eq!(server.recv().await.unwrap(), msg.as_bytes());

            let reply = format!("reply {i}");
            server.send(reply.as_bytes()).await.unwrap();
            assert_eq!(client.recv().await.unwrap(), reply.as_bytes());
        }
    }
}
