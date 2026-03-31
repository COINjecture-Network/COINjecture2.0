// =============================================================================
// Mesh Network Service — Public API
// =============================================================================
//
// This is the main entry point for the P2P mesh networking layer. It wires
// together identity, transport, connection management, gossip, and routing
// into a single NetworkService that the consensus engine interacts with
// through NetworkCommand / NetworkEvent channels.

pub mod bridge;
pub mod config;
pub mod connection;
pub mod error;
pub mod gossip;
pub mod identity;
pub mod peer_manager;
pub mod protocol;
pub mod router;
pub mod transport;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::{mpsc, watch};

use self::config::NetworkConfig;
use self::connection::{ConnectionEvent, ConnectionState};
use self::error::NetworkError;
use self::gossip::GossipEngine;
use self::identity::{Keypair, NodeId};
use self::peer_manager::PeerManager;
use self::protocol::{Payload, RoutingMode, WireMessage};
use self::router::RouteAction;

// ─── Public API types ────────────────────────────────────────────────────────

/// Commands the application sends to the network layer.
#[derive(Debug)]
pub enum NetworkCommand {
    /// Broadcast a payload to all peers via gossip.
    Broadcast(Payload),
    /// Send a payload directly to a specific peer.
    SendDirect { target: NodeId, payload: Payload },
    /// Request the current peer list.
    GetPeers,
    /// Gracefully shut down the network.
    Shutdown,
}

/// Events the network delivers to the application layer.
#[derive(Debug)]
pub enum NetworkEvent {
    /// A message was received from a peer.
    MessageReceived {
        from: NodeId,
        payload: Payload,
        msg_id: [u8; 32],
    },
    /// A new peer connected and completed handshake.
    PeerConnected(NodeId),
    /// A peer disconnected.
    PeerDisconnected(NodeId),
    /// Response to GetPeers command.
    PeerList(Vec<(NodeId, SocketAddr, ConnectionState)>),
}

/// Handle to a running network service.
///
/// The application interacts with the mesh through this handle by sending
/// NetworkCommands and receiving NetworkEvents.
pub struct NetworkService {
    cmd_tx: mpsc::UnboundedSender<NetworkCommand>,
    node_id: NodeId,
    shutdown_tx: watch::Sender<bool>,
    event_loop_handle: Option<tokio::task::JoinHandle<()>>,
    listener_handle: Option<tokio::task::JoinHandle<()>>,
}

impl NetworkService {
    /// Start the mesh network service.
    ///
    /// Returns a handle for sending commands and a receiver for events.
    /// Begins listening for connections and dialing seed nodes immediately.
    pub async fn start(
        config: NetworkConfig,
    ) -> Result<(Self, mpsc::UnboundedReceiver<NetworkEvent>), NetworkError> {
        let keypair = Arc::new(Keypair::load_or_generate(&config.data_dir)?);
        let node_id = *keypair.node_id();
        tracing::info!(node_id = %node_id.short(), listen = %config.listen_addr, "starting mesh network");

        let listener = transport::bind_listener(config.listen_addr).await?;
        let actual_addr = listener.local_addr().map_err(NetworkError::Io)?;
        tracing::info!(addr = %actual_addr, "mesh listener bound");

        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (conn_event_tx, conn_event_rx) = mpsc::unbounded_channel();
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        // Listener task: accepts incoming TCP connections
        let listener_handle = tokio::spawn(accept_loop(
            listener,
            keypair.clone(),
            actual_addr,
            conn_event_tx.clone(),
            config.max_message_size,
            config.handshake_timeout,
            shutdown_rx.clone(),
        ));

        // Main event loop: coordinates everything
        let event_loop_handle = tokio::spawn(event_loop(
            config.clone(),
            keypair,
            actual_addr,
            cmd_rx,
            event_tx,
            conn_event_tx,
            conn_event_rx,
            shutdown_rx,
        ));

        Ok((
            Self {
                cmd_tx,
                node_id,
                shutdown_tx,
                event_loop_handle: Some(event_loop_handle),
                listener_handle: Some(listener_handle),
            },
            event_rx,
        ))
    }

    /// Send a command to the network service.
    pub async fn send(&self, cmd: NetworkCommand) -> Result<(), NetworkError> {
        self.cmd_tx
            .send(cmd)
            .map_err(|_| NetworkError::ChannelClosed("command channel closed".into()))
    }

    /// Get a clone of the command sender for use from other tasks.
    pub fn command_sender(&self) -> mpsc::UnboundedSender<NetworkCommand> {
        self.cmd_tx.clone()
    }

    /// Our node's identity.
    pub fn local_id(&self) -> &NodeId {
        &self.node_id
    }

    /// Gracefully shut down the network service.
    pub async fn shutdown(mut self) -> Result<(), NetworkError> {
        tracing::info!("mesh network shutting down");
        let _ = self.shutdown_tx.send(true);
        if let Some(h) = self.event_loop_handle.take() {
            let _ = h.await;
        }
        if let Some(h) = self.listener_handle.take() {
            let _ = h.await;
        }
        Ok(())
    }
}

// ─── Accept loop ─────────────────────────────────────────────────────────────

/// Accepts incoming TCP connections, performs inbound handshake, then
/// calls `finalize_connection` to split the stream and notify the event loop.
async fn accept_loop(
    listener: tokio::net::TcpListener,
    keypair: Arc<Keypair>,
    listen_addr: SocketAddr,
    conn_event_tx: mpsc::UnboundedSender<ConnectionEvent>,
    max_msg_size: usize,
    handshake_timeout: std::time::Duration,
    mut shutdown: watch::Receiver<bool>,
) {
    loop {
        tokio::select! {
            accept = listener.accept() => {
                match accept {
                    Ok((stream, peer_addr)) => {
                        tracing::debug!(peer = %peer_addr, "incoming connection");
                        let kp = keypair.clone();
                        let tx = conn_event_tx.clone();
                        let sd = shutdown.clone();

                        tokio::spawn(async move {
                            handle_inbound(
                                stream, peer_addr, kp, listen_addr,
                                tx, max_msg_size, handshake_timeout, sd,
                            ).await;
                        });
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "accept error");
                    }
                }
            }
            _ = shutdown.changed() => {
                tracing::info!("listener shutting down");
                break;
            }
        }
    }
}

/// Perform inbound handshake, then finalize the connection.
async fn handle_inbound(
    mut stream: tokio::net::TcpStream,
    peer_addr: SocketAddr,
    keypair: Arc<Keypair>,
    listen_addr: SocketAddr,
    conn_event_tx: mpsc::UnboundedSender<ConnectionEvent>,
    max_msg_size: usize,
    handshake_timeout: std::time::Duration,
    shutdown: watch::Receiver<bool>,
) {
    match connection::perform_inbound_handshake(
        &mut stream,
        &keypair,
        listen_addr,
        max_msg_size,
        handshake_timeout,
    )
    .await
    {
        Ok((peer_id, public_key, peer_listen_addr)) => {
            tracing::info!(peer = %peer_id.short(), addr = %peer_listen_addr, "inbound handshake OK");
            connection::finalize_connection(
                peer_id,
                peer_listen_addr,
                public_key,
                stream,
                conn_event_tx,
                max_msg_size,
                shutdown,
                false, // inbound
            );
        }
        Err(e) => {
            tracing::warn!(peer = %peer_addr, error = %e, "inbound handshake failed");
        }
    }
}

// ─── Dial helper ─────────────────────────────────────────────────────────────

/// Dial a remote peer, perform outbound handshake, then finalize the connection.
async fn dial_and_handshake(
    addr: SocketAddr,
    keypair: Arc<Keypair>,
    listen_addr: SocketAddr,
    conn_event_tx: mpsc::UnboundedSender<ConnectionEvent>,
    max_msg_size: usize,
    connect_timeout: std::time::Duration,
    handshake_timeout: std::time::Duration,
    shutdown: watch::Receiver<bool>,
) {
    let mut stream = match transport::dial(addr, connect_timeout).await {
        Ok(s) => s,
        Err(e) => {
            tracing::debug!(addr = %addr, error = %e, "dial failed");
            return;
        }
    };

    match connection::perform_outbound_handshake(
        &mut stream,
        &keypair,
        listen_addr,
        max_msg_size,
        handshake_timeout,
    )
    .await
    {
        Ok((peer_id, public_key, peer_listen_addr)) => {
            tracing::info!(peer = %peer_id.short(), addr = %peer_listen_addr, "outbound handshake OK");
            connection::finalize_connection(
                peer_id,
                peer_listen_addr,
                public_key,
                stream,
                conn_event_tx,
                max_msg_size,
                shutdown,
                true, // outbound
            );
        }
        Err(e) => {
            tracing::warn!(addr = %addr, error = %e, "outbound handshake failed");
        }
    }
}

// ─── Main event loop ─────────────────────────────────────────────────────────

/// The core event loop that coordinates all mesh networking activity.
async fn event_loop(
    config: NetworkConfig,
    keypair: Arc<Keypair>,
    listen_addr: SocketAddr,
    mut cmd_rx: mpsc::UnboundedReceiver<NetworkCommand>,
    event_tx: mpsc::UnboundedSender<NetworkEvent>,
    conn_event_tx: mpsc::UnboundedSender<ConnectionEvent>,
    mut conn_event_rx: mpsc::UnboundedReceiver<ConnectionEvent>,
    mut shutdown: watch::Receiver<bool>,
) {
    let local_id = *keypair.node_id();

    let mut peer_mgr = PeerManager::new(
        local_id,
        config.reconnect_base_delay,
        config.reconnect_max_delay,
        config.max_missed_heartbeats,
    );

    let mut gossip = GossipEngine::new(config.dedup_cache_capacity, config.dedup_cache_ttl);

    // Per-peer write channels (received via ConnectionEvent::Connected)
    let mut peer_writers: HashMap<NodeId, mpsc::UnboundedSender<WireMessage>> = HashMap::new();

    // Rate limiting: (message_count, window_start) per peer
    let mut rate_counters: HashMap<NodeId, (u32, Instant)> = HashMap::new();

    // Timers
    let mut heartbeat_tick = tokio::time::interval(config.heartbeat_interval);
    let mut peer_exchange_tick = tokio::time::interval(config.peer_exchange_interval);
    let mut reconnect_tick = tokio::time::interval(std::time::Duration::from_secs(2));

    // Dial seed nodes on startup
    for seed_addr in &config.seed_nodes {
        tracing::info!(seed = %seed_addr, "dialing seed node");
        let kp = keypair.clone();
        let tx = conn_event_tx.clone();
        let sd = shutdown.clone();
        let addr = *seed_addr;
        let ct = config.connect_timeout;
        let ht = config.handshake_timeout;
        let ms = config.max_message_size;
        let la = listen_addr;

        tokio::spawn(async move {
            dial_and_handshake(addr, kp, la, tx, ms, ct, ht, sd).await;
        });
    }

    loop {
        tokio::select! {
            // ── Application commands ──────────────────────────────────────
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(NetworkCommand::Broadcast(payload)) => {
                        let envelope = router::create_envelope(
                            &keypair, RoutingMode::Broadcast, payload, config.default_ttl,
                        );
                        gossip.check_and_mark(&envelope.msg_id);
                        let wire = WireMessage::Envelope(envelope);
                        for (pid, tx) in &peer_writers {
                            if tx.send(wire.clone()).is_err() {
                                tracing::warn!(peer = %pid.short(), "broadcast queue failed");
                            }
                        }
                    }
                    Some(NetworkCommand::SendDirect { target, payload }) => {
                        let envelope = router::create_envelope(
                            &keypair, RoutingMode::Direct { target }, payload, config.default_ttl,
                        );
                        gossip.check_and_mark(&envelope.msg_id);
                        let wire = WireMessage::Envelope(envelope);

                        if let Some(tx) = peer_writers.get(&target) {
                            let _ = tx.send(wire);
                        } else {
                            // Forward to all peers for routing
                            for (pid, tx) in &peer_writers {
                                if tx.send(wire.clone()).is_err() {
                                    tracing::warn!(peer = %pid.short(), "direct forward queue failed");
                                }
                            }
                        }
                    }
                    Some(NetworkCommand::GetPeers) => {
                        let _ = event_tx.send(NetworkEvent::PeerList(peer_mgr.all_peers()));
                    }
                    Some(NetworkCommand::Shutdown) | None => {
                        tracing::info!("event loop: shutdown command");
                        break;
                    }
                }
            }

            // ── Connection events ─────────────────────────────────────────
            conn_evt = conn_event_rx.recv() => {
                match conn_evt {
                    Some(ConnectionEvent::Connected {
                        peer_id, listen_addr: peer_listen, public_key, write_tx, outbound,
                    }) => {
                        // Check for duplicate connection
                        if peer_writers.contains_key(&peer_id) {
                            tracing::debug!(peer = %peer_id.short(), "duplicate connection, dropping new one");
                            drop(write_tx); // This will close the write loop
                            continue;
                        }

                        peer_mgr.register_peer(peer_id, peer_listen, public_key, outbound);
                        peer_writers.insert(peer_id, write_tx);
                        tracing::info!(
                            peer = %peer_id.short(),
                            direction = if outbound { "outbound" } else { "inbound" },
                            total_peers = peer_writers.len(),
                            "peer registered"
                        );
                        let _ = event_tx.send(NetworkEvent::PeerConnected(peer_id));
                    }

                    Some(ConnectionEvent::MessageReceived { peer_id, message }) => {
                        // Rate limiting
                        let now = Instant::now();
                        let (count, window_start) = rate_counters.entry(peer_id).or_insert((0, now));
                        if now.duration_since(*window_start) >= std::time::Duration::from_secs(1) {
                            *count = 0;
                            *window_start = now;
                        }
                        *count += 1;
                        if *count > config.max_messages_per_second_per_peer {
                            tracing::warn!(peer = %peer_id.short(), "rate limited");
                            continue;
                        }

                        peer_mgr.touch(&peer_id);

                        match message {
                            WireMessage::Envelope(envelope) => {
                                // Handle PeerExchange payloads specially
                                if let Payload::PeerExchange { ref known_peers } = envelope.payload {
                                    let new_peers = peer_mgr.ingest_peer_exchange(known_peers);
                                    for (new_id, new_addr) in new_peers {
                                        if peer_mgr.should_dial(&new_id) && !peer_writers.contains_key(&new_id) {
                                            tracing::info!(peer = %new_id.short(), addr = %new_addr, "discovered via peer exchange");
                                            let kp = keypair.clone();
                                            let tx = conn_event_tx.clone();
                                            let sd = shutdown.clone();
                                            let ct = config.connect_timeout;
                                            let ht = config.handshake_timeout;
                                            let ms = config.max_message_size;
                                            let la = listen_addr;
                                            tokio::spawn(async move {
                                                dial_and_handshake(new_addr, kp, la, tx, ms, ct, ht, sd).await;
                                            });
                                        }
                                    }
                                }

                                let pk_map = peer_mgr.public_key_map();
                                let action = router::process_incoming(
                                    &envelope, &local_id, &mut gossip, &pk_map,
                                );

                                match action {
                                    RouteAction::DeliverAndForward => {
                                        let _ = event_tx.send(NetworkEvent::MessageReceived {
                                            from: envelope.sender,
                                            payload: envelope.payload.clone(),
                                            msg_id: envelope.msg_id,
                                        });
                                        let targets = GossipEngine::forward_targets(
                                            &peer_mgr.connected_peers(),
                                            &envelope.sender,
                                            &local_id,
                                        );
                                        if let Some(new_ttl) = GossipEngine::decrement_ttl(envelope.ttl) {
                                            let mut fwd = envelope;
                                            fwd.ttl = new_ttl;
                                            let wire = WireMessage::Envelope(fwd);
                                            for t in targets {
                                                if let Some(tx) = peer_writers.get(&t) {
                                                    let _ = tx.send(wire.clone());
                                                }
                                            }
                                        }
                                    }
                                    RouteAction::DeliverOnly => {
                                        let _ = event_tx.send(NetworkEvent::MessageReceived {
                                            from: envelope.sender,
                                            payload: envelope.payload,
                                            msg_id: envelope.msg_id,
                                        });
                                    }
                                    RouteAction::ForwardOnly => {
                                        if let Some(new_ttl) = GossipEngine::decrement_ttl(envelope.ttl) {
                                            let mut fwd = envelope;
                                            fwd.ttl = new_ttl;
                                            let wire = WireMessage::Envelope(fwd);
                                            for (pid, tx) in &peer_writers {
                                                if *pid != peer_id {
                                                    let _ = tx.send(wire.clone());
                                                }
                                            }
                                        }
                                    }
                                    RouteAction::Drop(reason) => {
                                        tracing::debug!(peer = %peer_id.short(), reason = %reason, "dropped msg");
                                    }
                                }
                            }
                            WireMessage::Handshake(_) => {
                                tracing::warn!(peer = %peer_id.short(), "unexpected handshake on established conn");
                            }
                        }
                    }

                    Some(ConnectionEvent::Disconnected { peer_id, reason }) => {
                        tracing::info!(peer = %peer_id.short(), reason = %reason, "peer disconnected");
                        peer_writers.remove(&peer_id);
                        peer_mgr.mark_disconnected(&peer_id);
                        rate_counters.remove(&peer_id);
                        let _ = event_tx.send(NetworkEvent::PeerDisconnected(peer_id));
                    }

                    None => break,
                }
            }

            // ── Heartbeat ─────────────────────────────────────────────────
            _ = heartbeat_tick.tick() => {
                if !peer_writers.is_empty() {
                    let payload = Payload::Heartbeat {
                        epoch: 0,
                        peer_count: peer_mgr.connected_count() as u16,
                        best_block: [0; 32],
                    };
                    let envelope = router::create_envelope(
                        &keypair, RoutingMode::Broadcast, payload, config.default_ttl,
                    );
                    gossip.check_and_mark(&envelope.msg_id);
                    let wire = WireMessage::Envelope(envelope);
                    for tx in peer_writers.values() {
                        let _ = tx.send(wire.clone());
                    }
                }

                // Liveness checks
                let connected: Vec<NodeId> = peer_mgr.connected_peers();
                for pid in connected {
                    if let Some(record) = peer_mgr.get_peer(&pid) {
                        if record.last_seen.elapsed() > config.heartbeat_interval * 2 {
                            let is_dead = peer_mgr.heartbeat_missed(&pid);
                            if is_dead {
                                tracing::warn!(peer = %pid.short(), "declared dead (heartbeat)");
                                peer_writers.remove(&pid);
                                let _ = event_tx.send(NetworkEvent::PeerDisconnected(pid));
                            }
                        }
                    }
                }
            }

            // ── Peer exchange ─────────────────────────────────────────────
            _ = peer_exchange_tick.tick() => {
                let list = peer_mgr.peer_exchange_list();
                if !list.is_empty() && !peer_writers.is_empty() {
                    let payload = Payload::PeerExchange { known_peers: list };
                    let envelope = router::create_envelope(
                        &keypair, RoutingMode::Broadcast, payload, 2,
                    );
                    gossip.check_and_mark(&envelope.msg_id);
                    let wire = WireMessage::Envelope(envelope);
                    for tx in peer_writers.values() {
                        let _ = tx.send(wire.clone());
                    }
                }
            }

            // ── Reconnection ──────────────────────────────────────────────
            _ = reconnect_tick.tick() => {
                let to_reconnect = peer_mgr.peers_needing_reconnect();
                for (pid, addr) in to_reconnect {
                    if !peer_mgr.should_dial(&pid) || peer_writers.contains_key(&pid) {
                        continue;
                    }
                    tracing::debug!(peer = %pid.short(), addr = %addr, "reconnecting");
                    peer_mgr.mark_connecting(&pid);

                    let kp = keypair.clone();
                    let tx = conn_event_tx.clone();
                    let sd = shutdown.clone();
                    let ct = config.connect_timeout;
                    let ht = config.handshake_timeout;
                    let ms = config.max_message_size;
                    let la = listen_addr;

                    tokio::spawn(async move {
                        dial_and_handshake(addr, kp, la, tx, ms, ct, ht, sd).await;
                    });
                }
            }

            // ── Shutdown ──────────────────────────────────────────────────
            _ = shutdown.changed() => {
                tracing::info!("event loop: shutdown signal");
                break;
            }
        }
    }

    tracing::info!("mesh event loop exited");
}
