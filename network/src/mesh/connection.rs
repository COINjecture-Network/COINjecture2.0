// =============================================================================
// Connection State Machine
// =============================================================================
//
// Manages a single peer connection through its lifecycle:
// Disconnected → Connecting → Handshaking → Connected → Dead
//
// Each connected peer has separate read and write tasks operating on a split
// TCP stream. The handshake authenticates both sides via Ed25519 challenge-response.

use std::net::SocketAddr;

use tokio::net::TcpStream;
use tokio::sync::mpsc;

use super::error::NetworkError;
use super::identity::{verify_signature_for_node, Keypair, NodeId};
use super::protocol::{HandshakeMessage, WireMessage};
use super::transport;

/// The state of a connection to a peer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Not connected, may attempt reconnection.
    Disconnected,
    /// TCP connect in progress.
    Connecting,
    /// TCP connected, performing Ed25519 handshake.
    Handshaking,
    /// Fully authenticated and operational.
    Connected,
    /// Connection lost, scheduled for reconnection.
    Dead,
}

impl std::fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionState::Disconnected => write!(f, "Disconnected"),
            ConnectionState::Connecting => write!(f, "Connecting"),
            ConnectionState::Handshaking => write!(f, "Handshaking"),
            ConnectionState::Connected => write!(f, "Connected"),
            ConnectionState::Dead => write!(f, "Dead"),
        }
    }
}

/// Events emitted by connection tasks to the main event loop.
///
/// This is the internal event type — it carries everything the event loop
/// needs to register new connections and handle disconnections.
pub enum ConnectionEvent {
    /// A new peer connection was established (handshake complete).
    /// Carries the write channel so the event loop can send messages to this peer.
    Connected {
        peer_id: NodeId,
        listen_addr: SocketAddr,
        public_key: Vec<u8>,
        write_tx: mpsc::UnboundedSender<WireMessage>,
        outbound: bool,
    },
    /// A wire message was received from this peer.
    MessageReceived {
        peer_id: NodeId,
        message: WireMessage,
    },
    /// The connection was closed or errored out.
    Disconnected { peer_id: NodeId, reason: String },
}

// ConnectionEvent can't derive Debug because write_tx doesn't implement Debug
impl std::fmt::Debug for ConnectionEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionEvent::Connected {
                peer_id,
                listen_addr,
                outbound,
                ..
            } => f
                .debug_struct("Connected")
                .field("peer_id", peer_id)
                .field("listen_addr", listen_addr)
                .field("outbound", outbound)
                .finish(),
            ConnectionEvent::MessageReceived { peer_id, .. } => f
                .debug_struct("MessageReceived")
                .field("peer_id", peer_id)
                .finish(),
            ConnectionEvent::Disconnected { peer_id, reason } => f
                .debug_struct("Disconnected")
                .field("peer_id", peer_id)
                .field("reason", reason)
                .finish(),
        }
    }
}

/// Perform the outbound (initiator) handshake on an established TCP stream.
///
/// Protocol:
/// 1. Send Hello with our identity + random challenge
/// 2. Receive HelloAck: verify their sig over our challenge, get their challenge
/// 3. Send ChallengeResponse: sign their challenge
///
/// Returns the peer's NodeId, public key, and listen address on success.
pub async fn perform_outbound_handshake(
    stream: &mut TcpStream,
    keypair: &Keypair,
    listen_addr: SocketAddr,
    max_msg_size: usize,
    timeout: std::time::Duration,
) -> Result<(NodeId, Vec<u8>, SocketAddr), NetworkError> {
    let addr = stream.peer_addr().map_err(NetworkError::Io)?;

    // Generate our challenge
    let mut our_challenge = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut our_challenge);

    // Step 1: Send Hello
    let hello = WireMessage::Handshake(HandshakeMessage::Hello {
        node_id: *keypair.node_id(),
        public_key: keypair.public_key_bytes(),
        challenge: our_challenge,
        listen_addr,
    });

    tokio::time::timeout(timeout, async {
        transport::write_message(stream, &hello, max_msg_size).await?;

        // Step 2: Read HelloAck
        let response = transport::read_message(stream, max_msg_size)
            .await?
            .ok_or_else(|| NetworkError::HandshakeFailed {
                addr,
                reason: "connection closed during handshake".into(),
            })?;

        let (peer_id, peer_pk, their_challenge, peer_listen_addr) = match response {
            WireMessage::Handshake(HandshakeMessage::HelloAck {
                node_id,
                public_key,
                challenge_response,
                challenge,
                listen_addr: peer_listen,
            }) => {
                // Verify their signature over our challenge
                verify_signature_for_node(
                    &node_id,
                    &public_key,
                    &our_challenge,
                    &challenge_response,
                )?;
                (node_id, public_key, challenge, peer_listen)
            }
            _ => {
                return Err(NetworkError::HandshakeFailed {
                    addr,
                    reason: "expected HelloAck, got something else".into(),
                });
            }
        };

        // Step 3: Sign their challenge and send
        let our_response = keypair.sign(&their_challenge);
        let challenge_msg = WireMessage::Handshake(HandshakeMessage::ChallengeResponse {
            challenge_response: our_response,
        });
        transport::write_message(stream, &challenge_msg, max_msg_size).await?;

        Ok((peer_id, peer_pk, peer_listen_addr))
    })
    .await
    .map_err(|_| NetworkError::HandshakeFailed {
        addr,
        reason: "handshake timed out".into(),
    })?
}

/// Perform the inbound (responder) handshake on an accepted TCP stream.
///
/// Protocol:
/// 1. Receive Hello: verify format, get their identity + challenge
/// 2. Send HelloAck: sign their challenge, issue our own challenge
/// 3. Receive ChallengeResponse: verify their sig over our challenge
///
/// Returns the peer's NodeId, public key, and listen address on success.
pub async fn perform_inbound_handshake(
    stream: &mut TcpStream,
    keypair: &Keypair,
    listen_addr: SocketAddr,
    max_msg_size: usize,
    timeout: std::time::Duration,
) -> Result<(NodeId, Vec<u8>, SocketAddr), NetworkError> {
    let addr = stream.peer_addr().map_err(NetworkError::Io)?;

    tokio::time::timeout(timeout, async {
        // Step 1: Read Hello
        let hello = transport::read_message(stream, max_msg_size)
            .await?
            .ok_or_else(|| NetworkError::HandshakeFailed {
                addr,
                reason: "connection closed before handshake".into(),
            })?;

        let (peer_id, peer_pk, their_challenge, peer_listen_addr) = match hello {
            WireMessage::Handshake(HandshakeMessage::Hello {
                node_id,
                public_key,
                challenge,
                listen_addr: peer_listen,
            }) => {
                // Verify the public key matches the claimed NodeId
                let derived = NodeId::from_public_key(
                    &ed25519_dalek::VerifyingKey::from_bytes(
                        &<[u8; 32]>::try_from(public_key.as_slice()).map_err(|_| {
                            NetworkError::HandshakeFailed {
                                addr,
                                reason: "invalid public key length".into(),
                            }
                        })?,
                    )
                    .map_err(|e| NetworkError::HandshakeFailed {
                        addr,
                        reason: format!("invalid public key: {}", e),
                    })?,
                );
                if derived != node_id {
                    return Err(NetworkError::HandshakeFailed {
                        addr,
                        reason: "public key doesn't match node ID".into(),
                    });
                }
                (node_id, public_key, challenge, peer_listen)
            }
            _ => {
                return Err(NetworkError::HandshakeFailed {
                    addr,
                    reason: "expected Hello, got something else".into(),
                });
            }
        };

        // Step 2: Sign their challenge and send HelloAck with our own challenge
        let our_response = keypair.sign(&their_challenge);
        let mut our_challenge = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut our_challenge);

        let hello_ack = WireMessage::Handshake(HandshakeMessage::HelloAck {
            node_id: *keypair.node_id(),
            public_key: keypair.public_key_bytes(),
            challenge_response: our_response,
            challenge: our_challenge,
            listen_addr,
        });
        transport::write_message(stream, &hello_ack, max_msg_size).await?;

        // Step 3: Read ChallengeResponse
        let response = transport::read_message(stream, max_msg_size)
            .await?
            .ok_or_else(|| NetworkError::HandshakeFailed {
                addr,
                reason: "connection closed during challenge response".into(),
            })?;

        match response {
            WireMessage::Handshake(HandshakeMessage::ChallengeResponse { challenge_response }) => {
                verify_signature_for_node(&peer_id, &peer_pk, &our_challenge, &challenge_response)?;
            }
            _ => {
                return Err(NetworkError::HandshakeFailed {
                    addr,
                    reason: "expected ChallengeResponse".into(),
                });
            }
        }

        Ok((peer_id, peer_pk, peer_listen_addr))
    })
    .await
    .map_err(|_| NetworkError::HandshakeFailed {
        addr,
        reason: "handshake timed out".into(),
    })?
}

/// Spawn the read loop for a connected peer. Reads WireMessages and sends
/// them as ConnectionEvents to the peer manager. Exits on EOF or error.
pub fn spawn_read_loop(
    peer_id: NodeId,
    mut reader: tokio::io::ReadHalf<TcpStream>,
    event_tx: mpsc::UnboundedSender<ConnectionEvent>,
    max_msg_size: usize,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                result = transport::read_message(&mut reader, max_msg_size) => {
                    match result {
                        Ok(Some(message)) => {
                            if event_tx
                                .send(ConnectionEvent::MessageReceived {
                                    peer_id,
                                    message,
                                })
                                .is_err()
                            {
                                break; // Channel closed
                            }
                        }
                        Ok(None) => {
                            // Clean EOF
                            let _ = event_tx.send(ConnectionEvent::Disconnected {
                                peer_id,
                                reason: "connection closed by peer".into(),
                            });
                            break;
                        }
                        Err(e) => {
                            let _ = event_tx.send(ConnectionEvent::Disconnected {
                                peer_id,
                                reason: format!("read error: {}", e),
                            });
                            break;
                        }
                    }
                }
                _ = shutdown.changed() => {
                    let _ = event_tx.send(ConnectionEvent::Disconnected {
                        peer_id,
                        reason: "shutdown".into(),
                    });
                    break;
                }
            }
        }
    })
}

/// Spawn the write loop for a connected peer. Takes messages from a channel
/// and writes them to the TCP stream.
pub fn spawn_write_loop(
    peer_id: NodeId,
    mut writer: tokio::io::WriteHalf<TcpStream>,
    mut msg_rx: mpsc::UnboundedReceiver<WireMessage>,
    event_tx: mpsc::UnboundedSender<ConnectionEvent>,
    max_msg_size: usize,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                msg = msg_rx.recv() => {
                    match msg {
                        Some(wire_msg) => {
                            if let Err(e) = transport::write_message(&mut writer, &wire_msg, max_msg_size).await {
                                tracing::warn!(peer = %peer_id.short(), error = %e, "write error, closing connection");
                                let _ = event_tx.send(ConnectionEvent::Disconnected {
                                    peer_id,
                                    reason: format!("write error: {}", e),
                                });
                                break;
                            }
                        }
                        None => break, // Channel closed — peer removed
                    }
                }
                _ = shutdown.changed() => {
                    break;
                }
            }
        }
    })
}

/// Complete connection setup: split stream, spawn read/write loops, and notify
/// the event loop via a ConnectionEvent::Connected carrying the write_tx.
#[allow(clippy::too_many_arguments)]
pub fn finalize_connection(
    peer_id: NodeId,
    listen_addr: SocketAddr,
    public_key: Vec<u8>,
    stream: TcpStream,
    conn_event_tx: mpsc::UnboundedSender<ConnectionEvent>,
    max_msg_size: usize,
    shutdown: tokio::sync::watch::Receiver<bool>,
    outbound: bool,
) {
    let (reader, writer) = tokio::io::split(stream);
    let (write_tx, write_rx) = mpsc::unbounded_channel();

    // Spawn read loop
    spawn_read_loop(
        peer_id,
        reader,
        conn_event_tx.clone(),
        max_msg_size,
        shutdown.clone(),
    );

    // Spawn write loop
    spawn_write_loop(
        peer_id,
        writer,
        write_rx,
        conn_event_tx.clone(),
        max_msg_size,
        shutdown,
    );

    // Notify the event loop that this connection is ready
    let _ = conn_event_tx.send(ConnectionEvent::Connected {
        peer_id,
        listen_addr,
        public_key,
        write_tx,
        outbound,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn test_handshake_mutual_auth() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let kp_a = Arc::new(Keypair::generate());
        let kp_b = Arc::new(Keypair::generate());
        let max_size = 16 * 1024 * 1024;
        let timeout = std::time::Duration::from_secs(5);

        let kp_b_clone = kp_b.clone();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            perform_inbound_handshake(&mut stream, &kp_b_clone, addr, max_size, timeout).await
        });

        let kp_a_clone = kp_a.clone();
        let client = tokio::spawn(async move {
            let mut stream = TcpStream::connect(addr).await.unwrap();
            perform_outbound_handshake(
                &mut stream,
                &kp_a_clone,
                "127.0.0.1:0".parse().unwrap(),
                max_size,
                timeout,
            )
            .await
        });

        let (server_result, client_result) = tokio::join!(server, client);
        let (peer_id_a, _, _) = server_result.unwrap().unwrap();
        let (peer_id_b, _, _) = client_result.unwrap().unwrap();

        // Server should see client's ID, client should see server's ID
        assert_eq!(peer_id_a, *kp_a.node_id());
        assert_eq!(peer_id_b, *kp_b.node_id());
    }

    #[test]
    fn test_connection_state_display() {
        assert_eq!(format!("{}", ConnectionState::Connected), "Connected");
        assert_eq!(format!("{}", ConnectionState::Dead), "Dead");
    }
}
