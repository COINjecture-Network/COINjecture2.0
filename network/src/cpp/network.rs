// =============================================================================
// COINjecture P2P Protocol (CPP) - Network Service (COMPLETE)
// =============================================================================

use crate::cpp::{
    block_provider::{BlockProvider, EmptyBlockProvider},
    config::{
        CppConfig, NodeType, KEEPALIVE_INTERVAL, MAX_BLOCKS_PER_RESPONSE, MESSAGE_READ_TIMEOUT,
        MIN_HEALTHY_PEERS, PEER_TIMEOUT, SECURITY_MALFORMED_STRIKE_THRESHOLD,
        SECURITY_RATE_STRIKE_THRESHOLD,
    },
    encryption,
    flock::{FlockState, FlockStateCompact},
    message::{
        sign_hello, BlocksMessage, GetBlocksMessage, HelloAckMessage, HelloMessage, MessageType,
        NewBlockMessage, NewTransactionMessage, PingMessage, PongMessage, StatusMessage,
    },
    node_integration::{NodeMetrics, PeerSelector},
    peer::{Peer, PeerId, PeerState},
    protocol::{MessageCodec, MessageEnvelope, ProtocolError},
    router::{EquilibriumRouter, PeerInfo},
};
use crate::reputation::ReputationManager;
use crate::security::{
    BanList, ConnectionLimiter, EclipseGuard, NetworkSecurityMetrics, TokenBucket,
};
use coinject_core::{Block, BlockHeader, Hash, Transaction};
use ed25519_dalek::SigningKey;
use rand::Rng;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio::time::{interval, Duration, Instant};

// =============================================================================
// Network Events & Commands
// =============================================================================

/// Events sent from network to node service
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    /// Peer connected successfully
    PeerConnected {
        peer_id: PeerId,
        addr: SocketAddr,
        node_type: NodeType,
        best_height: u64,
        best_hash: Hash,
    },

    /// Peer disconnected
    PeerDisconnected { peer_id: PeerId, reason: String },

    /// Status update from peer
    StatusUpdate {
        peer_id: PeerId,
        best_height: u64,
        best_hash: Hash,
        node_type: NodeType,
    },

    /// New block received
    BlockReceived { block: Block, peer_id: PeerId },

    /// New transaction received
    TransactionReceived {
        transaction: Transaction,
        peer_id: PeerId,
    },

    /// Blocks received (sync response)
    BlocksReceived {
        blocks: Vec<Block>,
        request_id: u64,
        peer_id: PeerId,
    },

    /// Headers received (light sync)
    HeadersReceived {
        headers: Vec<BlockHeader>,
        request_id: u64,
        peer_id: PeerId,
    },
}

/// Commands sent from node service to network
#[derive(Debug, Clone)]
pub enum NetworkCommand {
    /// Connect to bootnode
    ConnectBootnode { addr: SocketAddr },

    /// Broadcast new block
    BroadcastBlock { block: Block },

    /// Broadcast new transaction
    BroadcastTransaction { transaction: Transaction },

    /// Request blocks from peer
    RequestBlocks {
        peer_id: PeerId,
        from_height: u64,
        to_height: u64,
        request_id: u64,
    },

    /// Request headers (light sync)
    RequestHeaders {
        peer_id: PeerId,
        from_height: u64,
        to_height: u64,
        request_id: u64,
    },

    /// Disconnect from peer
    DisconnectPeer { peer_id: PeerId, reason: String },

    /// Update local chain state
    UpdateChainState { best_height: u64, best_hash: Hash },
}

// =============================================================================
// Chain State (shared with node service)
// =============================================================================

/// Chain state shared between network and node service
#[derive(Debug, Clone)]
pub struct ChainState {
    pub best_height: u64,
    pub best_hash: Hash,
    pub genesis_hash: Hash,
}

impl ChainState {
    pub fn new(genesis_hash: Hash) -> Self {
        ChainState {
            best_height: 0,
            best_hash: genesis_hash,
            genesis_hash,
        }
    }
}

// =============================================================================
// Network Service
// =============================================================================

pub struct CppNetwork {
    /// Configuration
    config: CppConfig,

    /// Local peer ID
    local_peer_id: PeerId,

    /// Connected peers
    peers: Arc<RwLock<HashMap<PeerId, Peer>>>,

    /// Equilibrium router
    router: Arc<RwLock<EquilibriumRouter>>,

    /// Reputation manager
    reputation: Arc<RwLock<ReputationManager>>,

    /// Local node metrics
    metrics: Arc<RwLock<NodeMetrics>>,

    /// Chain state
    chain_state: Arc<RwLock<ChainState>>,

    /// Event sender (to node service)
    event_tx: mpsc::UnboundedSender<NetworkEvent>,

    /// Command receiver (from node service)
    command_rx: mpsc::UnboundedReceiver<NetworkCommand>,

    /// Shutdown signal
    shutdown_tx: broadcast::Sender<()>,
    shutdown_rx: broadcast::Receiver<()>,

    /// Block provider for serving sync requests
    block_provider: Arc<dyn BlockProvider>,

    /// Pending block requests (for tracking responses)
    pending_requests: Arc<RwLock<HashMap<u64, Instant>>>,

    /// Bootnode reconnection backoff times
    bootnode_backoff: Arc<RwLock<HashMap<SocketAddr, Duration>>>,

    /// Last bootnode connection attempt times
    last_bootnode_attempt: Arc<RwLock<HashMap<SocketAddr, Instant>>>,

    /// Murmuration flock state for swarm coordination
    flock_state: Arc<RwLock<FlockState>>,

    /// Seen-message deduplication cache (hash, timestamp) — max 5000 entries, 60s TTL
    #[allow(clippy::type_complexity)]
    seen_messages: Arc<RwLock<std::collections::VecDeque<([u8; 32], Instant)>>>,

    // =========================================================================
    // Security subsystems (Phase 5)
    // =========================================================================
    /// Optional ed25519 signing key for peer authentication.
    /// When Some, outbound Hello/HelloAck messages are signed and inbound
    /// authentication is enforced (if config.require_encryption is true).
    signing_key: Option<Arc<SigningKey>>,

    /// Per-IP / total connection limiter.
    connection_limiter: Arc<RwLock<ConnectionLimiter>>,

    /// Peer and IP ban list with configurable expiry.
    ban_list: Arc<RwLock<BanList>>,

    /// Eclipse attack protection — enforces subnet diversity.
    eclipse_guard: Arc<RwLock<EclipseGuard>>,

    /// Per-peer token-bucket rate limiters.
    rate_limiters: Arc<RwLock<HashMap<PeerId, TokenBucket>>>,

    /// Per-peer malformed message strike counters.
    malformed_strikes: Arc<RwLock<HashMap<PeerId, u32>>>,

    /// Network security metrics (connected peers, bandwidth, churn, drops).
    security_metrics: Arc<RwLock<NetworkSecurityMetrics>>,
}

impl CppNetwork {
    /// Create new CPP network service
    pub fn new(
        config: CppConfig,
        local_peer_id: PeerId,
        genesis_hash: Hash,
    ) -> (
        Self,
        mpsc::UnboundedSender<NetworkCommand>,
        mpsc::UnboundedReceiver<NetworkEvent>,
    ) {
        Self::new_with_chain_state(config, local_peer_id, genesis_hash, 0, genesis_hash)
    }

    /// Create new CPP network service with custom block provider (for sync)
    pub fn new_with_block_provider(
        config: CppConfig,
        local_peer_id: PeerId,
        genesis_hash: Hash,
        initial_height: u64,
        initial_hash: Hash,
        block_provider: Arc<dyn BlockProvider>,
    ) -> (
        Self,
        mpsc::UnboundedSender<NetworkCommand>,
        mpsc::UnboundedReceiver<NetworkEvent>,
    ) {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

        let chain_state = Arc::new(RwLock::new(ChainState {
            best_height: initial_height,
            best_hash: initial_hash,
            genesis_hash,
        }));

        let connection_limiter =
            ConnectionLimiter::new(config.max_connections_per_ip, config.max_total_connections);
        let ban_duration = std::time::Duration::from_secs(config.ban_duration_secs);
        let eclipse_guard = EclipseGuard::new(config.max_peers_per_subnet);
        let (rate_cap, rate_refill) = (config.rate_bucket_capacity, config.rate_msgs_per_sec);

        let network = CppNetwork {
            config,
            local_peer_id,
            peers: Arc::new(RwLock::new(HashMap::new())),
            router: Arc::new(RwLock::new(EquilibriumRouter::new())),
            reputation: Arc::new(RwLock::new(ReputationManager::new())),
            metrics: Arc::new(RwLock::new(NodeMetrics::new())),
            chain_state,
            event_tx,
            command_rx,
            shutdown_tx,
            shutdown_rx,
            block_provider,
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
            bootnode_backoff: Arc::new(RwLock::new(HashMap::new())),
            last_bootnode_attempt: Arc::new(RwLock::new(HashMap::new())),
            flock_state: Arc::new(RwLock::new(FlockState::new(
                &genesis_hash,
                initial_height,
                &local_peer_id,
            ))),
            seen_messages: Arc::new(RwLock::new(std::collections::VecDeque::new())),
            // Security
            signing_key: None,
            connection_limiter: Arc::new(RwLock::new(connection_limiter)),
            ban_list: Arc::new(RwLock::new(BanList::new(ban_duration))),
            eclipse_guard: Arc::new(RwLock::new(eclipse_guard)),
            rate_limiters: Arc::new(RwLock::new(HashMap::new())),
            malformed_strikes: Arc::new(RwLock::new(HashMap::new())),
            security_metrics: Arc::new(RwLock::new(NetworkSecurityMetrics::default())),
        };

        let _ = (rate_cap, rate_refill); // stored in config; used when creating per-peer buckets

        (network, command_tx, event_rx)
    }
    /// Create new CPP network service with initial chain state
    pub fn new_with_chain_state(
        config: CppConfig,
        local_peer_id: PeerId,
        genesis_hash: Hash,
        initial_height: u64,
        initial_hash: Hash,
    ) -> (
        Self,
        mpsc::UnboundedSender<NetworkCommand>,
        mpsc::UnboundedReceiver<NetworkEvent>,
    ) {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

        let chain_state = Arc::new(RwLock::new(ChainState {
            best_height: initial_height,
            best_hash: initial_hash,
            genesis_hash,
        }));

        let connection_limiter =
            ConnectionLimiter::new(config.max_connections_per_ip, config.max_total_connections);
        let ban_duration = std::time::Duration::from_secs(config.ban_duration_secs);
        let eclipse_guard = EclipseGuard::new(config.max_peers_per_subnet);

        let network = CppNetwork {
            config,
            local_peer_id,
            peers: Arc::new(RwLock::new(HashMap::new())),
            router: Arc::new(RwLock::new(EquilibriumRouter::new())),
            reputation: Arc::new(RwLock::new(ReputationManager::new())),
            metrics: Arc::new(RwLock::new(NodeMetrics::new())),
            chain_state,
            event_tx,
            command_rx,
            shutdown_tx,
            shutdown_rx,
            block_provider: Arc::new(EmptyBlockProvider),
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
            bootnode_backoff: Arc::new(RwLock::new(HashMap::new())),
            last_bootnode_attempt: Arc::new(RwLock::new(HashMap::new())),
            flock_state: Arc::new(RwLock::new(FlockState::new(
                &genesis_hash,
                initial_height,
                &local_peer_id,
            ))),
            seen_messages: Arc::new(RwLock::new(std::collections::VecDeque::new())),
            // Security
            signing_key: None,
            connection_limiter: Arc::new(RwLock::new(connection_limiter)),
            ban_list: Arc::new(RwLock::new(BanList::new(ban_duration))),
            eclipse_guard: Arc::new(RwLock::new(eclipse_guard)),
            rate_limiters: Arc::new(RwLock::new(HashMap::new())),
            malformed_strikes: Arc::new(RwLock::new(HashMap::new())),
            security_metrics: Arc::new(RwLock::new(NetworkSecurityMetrics::default())),
        };

        (network, command_tx, event_rx)
    }

    /// Create a new CPP network with an ed25519 signing key for peer authentication.
    ///
    /// Use this constructor when running a node that participates in authenticated
    /// peer connections.  Peers that cannot authenticate will be rejected when
    /// `config.require_encryption` is true.
    pub fn with_signing_key(mut self, signing_key: SigningKey) -> Self {
        self.signing_key = Some(Arc::new(signing_key));
        self
    }

    // =========================================================================
    // Main Event Loop
    // =========================================================================

    /// Start the network service
    pub async fn start(mut self) -> Result<(), NetworkError> {
        // Bind TCP listener
        let listener = TcpListener::bind(&self.config.p2p_listen).await?;
        let local_addr = listener.local_addr()?;

        tracing::info!("CPP Network listening on {}", local_addr);

        // Periodic maintenance intervals
        let mut ping_interval = interval(KEEPALIVE_INTERVAL);
        let mut cleanup_interval = interval(Duration::from_secs(60));
        let mut metrics_interval = interval(Duration::from_secs(300)); // 5 minutes
        let mut status_interval = interval(Duration::from_secs(10));
        let mut bootnode_reconnect_interval = interval(Duration::from_secs(5)); // Status broadcast every 10s
        let mut sync_check_interval = interval(Duration::from_secs(10)); // Sync check every 10s

        tracing::info!("[CPP] Event loop starting with {} intervals", 5);

        loop {
            tokio::select! {
                // Accept incoming connections
                Ok((stream, addr)) = listener.accept() => {
                    let ip = addr.ip();

                    // --- Phase 5: Security gate ---

                    // 1. Ban list check
                    {
                        let ban = self.ban_list.read().await;
                        if ban.is_ip_banned(ip) {
                            let mut m = self.security_metrics.write().await;
                            m.connections_rejected_ban += 1;
                            tracing::info!("[SECURITY][ACCEPT] Rejecting banned IP {}", ip);
                            continue;
                        }
                    }

                    // 2. Per-IP / total connection limit
                    {
                        let mut limiter = self.connection_limiter.write().await;
                        if let Err(e) = limiter.try_acquire(ip) {
                            let mut m = self.security_metrics.write().await;
                            match &e {
                                crate::security::ConnectionDenied::TooManyFromIp { .. } =>
                                    m.connections_rejected_ip_limit += 1,
                                crate::security::ConnectionDenied::TotalCapacityReached { .. } =>
                                    m.connections_rejected_total_limit += 1,
                            }
                            tracing::warn!("[SECURITY][ACCEPT] Connection rejected from {}: {}", addr, e);
                            continue;
                        }
                    }

                    // 3. Eclipse attack protection
                    {
                        let mut eclipse = self.eclipse_guard.write().await;
                        if !eclipse.try_add(ip) {
                            // Release the connection slot we just acquired
                            self.connection_limiter.write().await.release(ip);
                            let mut m = self.security_metrics.write().await;
                            m.connections_rejected_eclipse += 1;
                            tracing::warn!("[SECURITY][ECLIPSE] Rejecting {} — subnet at capacity", addr);
                            continue;
                        }
                    }

                    tracing::info!("[CPP][ACCEPT] Incoming connection from {}", addr);
                    let peers = self.peers.clone();
                    let router = self.router.clone();
                    let reputation = self.reputation.clone();
                    let chain_state = self.chain_state.clone();
                    let flock_state = self.flock_state.clone();
                    let pending_requests = self.pending_requests.clone();
                    let seen_messages = self.seen_messages.clone();
                    let event_tx = self.event_tx.clone();
                    let local_peer_id = self.local_peer_id;
                    let shutdown = self.shutdown_tx.subscribe();
                    let block_provider = self.block_provider.clone();
                    let ban_list = self.ban_list.clone();
                    let connection_limiter = self.connection_limiter.clone();
                    let eclipse_guard = self.eclipse_guard.clone();
                    let rate_limiters = self.rate_limiters.clone();
                    let malformed_strikes = self.malformed_strikes.clone();
                    let security_metrics = self.security_metrics.clone();
                    let signing_key = self.signing_key.clone();
                    let require_encryption = self.config.require_encryption;
                    let rate_cap = self.config.rate_bucket_capacity;
                    let rate_refill = self.config.rate_msgs_per_sec;

                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_incoming_connection(
                            stream,
                            addr,
                            local_peer_id,
                            peers,
                            router,
                            reputation,
                            chain_state,
                            flock_state,
                            pending_requests,
                            seen_messages,
                            event_tx,
                            block_provider,
                            shutdown,
                            ban_list,
                            connection_limiter,
                            eclipse_guard,
                            rate_limiters,
                            malformed_strikes,
                            security_metrics,
                            signing_key,
                            require_encryption,
                            rate_cap,
                            rate_refill,
                        ).await {
                            tracing::error!("Connection error from {}: {}", addr, e);
                        }
                    });
                }

                // Handle commands from node service
                Some(command) = self.command_rx.recv() => {
                    if let Err(e) = self.handle_command(command).await {
                        tracing::error!("Command handling error: {}", e);
                    }
                }

                // Periodic: Send pings
                _ = ping_interval.tick() => {
                    self.send_pings().await;
                }

                // Periodic: Cleanup stale peers
                _ = cleanup_interval.tick() => {
                    self.cleanup_stale_peers().await;
                }

                // Periodic: Update metrics
                _ = metrics_interval.tick() => {
                    self.update_metrics().await;
                }

                // Periodic: Broadcast status to peers
                _ = status_interval.tick() => {
                    tracing::debug!("[CPP] Status interval fired!");
                    self.broadcast_status().await;
                }

                // Periodic: Check if sync is needed
                _ = sync_check_interval.tick() => {
                    tracing::debug!("[CPP] Sync check interval fired!");
                    self.check_sync_status().await;
                }

                // Periodic: Check bootnode reconnection
                _ = bootnode_reconnect_interval.tick() => {
                    self.check_bootnode_reconnection().await;
                }

                // Shutdown signal
                _ = self.shutdown_rx.recv() => {
                    tracing::info!("CPP Network shutting down");
                    break;
                }
            }
        }

        Ok(())
    }

    // =========================================================================
    // Connection Management
    // =========================================================================

    /// Handle incoming connection
    #[allow(clippy::too_many_arguments, clippy::type_complexity)]
    async fn handle_incoming_connection(
        mut stream: TcpStream,
        addr: SocketAddr,
        local_peer_id: PeerId,
        peers: Arc<RwLock<HashMap<PeerId, Peer>>>,
        router: Arc<RwLock<EquilibriumRouter>>,
        _reputation: Arc<RwLock<ReputationManager>>,
        chain_state: Arc<RwLock<ChainState>>,
        flock_state: Arc<RwLock<FlockState>>,
        pending_requests: Arc<RwLock<HashMap<u64, Instant>>>,
        seen_messages: Arc<RwLock<std::collections::VecDeque<([u8; 32], Instant)>>>,
        event_tx: mpsc::UnboundedSender<NetworkEvent>,
        block_provider: Arc<dyn BlockProvider>,
        _shutdown: broadcast::Receiver<()>,
        // Security parameters
        ban_list: Arc<RwLock<BanList>>,
        connection_limiter: Arc<RwLock<ConnectionLimiter>>,
        eclipse_guard: Arc<RwLock<EclipseGuard>>,
        rate_limiters: Arc<RwLock<HashMap<PeerId, TokenBucket>>>,
        malformed_strikes: Arc<RwLock<HashMap<PeerId, u32>>>,
        security_metrics: Arc<RwLock<NetworkSecurityMetrics>>,
        signing_key: Option<Arc<SigningKey>>,
        require_encryption: bool,
        rate_cap: f64,
        rate_refill: f64,
    ) -> Result<(), NetworkError> {
        let ip = addr.ip();
        tracing::debug!("[CPP][INCOMING] Starting handshake with {}", addr);

        // ---- Phase 5: Encryption + peer authentication ----
        // If we have a signing key, perform the mutual auth handshake first.
        // This happens before the CPP Hello/HelloAck so all subsequent frames
        // are encrypted.
        let authenticated_peer_id: Option<[u8; 32]> = if let Some(ref sk) = signing_key {
            match tokio::time::timeout(
                crate::cpp::config::HANDSHAKE_TIMEOUT,
                encryption::perform_handshake_responder(&mut stream, sk),
            )
            .await
            {
                Ok(Ok(result)) => {
                    tracing::info!(
                        "[SECURITY][AUTH] Peer {} authenticated with pubkey {}",
                        addr,
                        hex::encode(&result.remote_ed25519_pubkey[..4])
                    );
                    security_metrics.write().await.on_peer_connect();
                    Some(result.authenticated_peer_id)
                }
                Ok(Err(e)) => {
                    tracing::warn!(
                        "[SECURITY][AUTH] Authentication failed from {}: {}",
                        addr,
                        e
                    );
                    security_metrics.write().await.auth_failures += 1;
                    // Release connection slot before returning
                    connection_limiter.write().await.release(ip);
                    eclipse_guard.write().await.remove(ip);
                    return Err(NetworkError::InvalidHandshake(format!(
                        "Auth failed: {}",
                        e
                    )));
                }
                Err(_) => {
                    tracing::warn!("[SECURITY][AUTH] Auth handshake timeout from {}", addr);
                    security_metrics.write().await.auth_failures += 1;
                    connection_limiter.write().await.release(ip);
                    eclipse_guard.write().await.remove(ip);
                    return Err(NetworkError::Timeout);
                }
            }
        } else if require_encryption {
            // Encryption required but no signing key configured — reject
            tracing::error!("[SECURITY] require_encryption=true but no signing_key configured!");
            connection_limiter.write().await.release(ip);
            eclipse_guard.write().await.remove(ip);
            return Err(NetworkError::InvalidHandshake(
                "Node not configured for encryption".to_string(),
            ));
        } else {
            // No encryption — count the connection but without auth
            security_metrics.write().await.on_peer_connect();
            None
        };

        // Perform CPP Hello/HelloAck handshake
        let state = chain_state.read().await;
        let handshake_result =
            Self::handshake(&mut stream, local_peer_id, &state, signing_key.as_deref()).await;
        drop(state);

        // Fix 2: Release connection slots on handshake failure (slot leak fix)
        let (peer_id, node_type, best_height, best_hash) = match handshake_result {
            Ok(v) => v,
            Err(e) => {
                connection_limiter.write().await.release(ip);
                eclipse_guard.write().await.remove(ip);
                return Err(e);
            }
        };

        // Verify the authenticated peer ID matches the claimed peer_id in Hello
        if let Some(auth_id) = authenticated_peer_id {
            if auth_id != peer_id {
                tracing::warn!(
                    "[SECURITY][AUTH] peer_id mismatch: auth={} hello={}",
                    hex::encode(&auth_id[..4]),
                    hex::encode(&peer_id[..4])
                );
                ban_list
                    .write()
                    .await
                    .ban_ip(ip, "peer_id/auth key mismatch");
                connection_limiter.write().await.release(ip);
                eclipse_guard.write().await.remove(ip);
                return Err(NetworkError::InvalidHandshake(
                    "peer_id does not match authenticated key".to_string(),
                ));
            }
        }

        // Check if peer already connected — release slots to prevent leak
        {
            let peers_guard = peers.read().await;
            if peers_guard.contains_key(&peer_id) {
                drop(peers_guard);
                connection_limiter.write().await.release(ip);
                eclipse_guard.write().await.remove(ip);
                return Err(NetworkError::InvalidHandshake(
                    "Peer already connected".to_string(),
                ));
            }
        }

        // Create peer instance (returns peer and read half of stream)
        // For incoming connections: is_outbound = false, generate nonce for tie-breaking
        let connection_nonce = rand::random::<u64>();
        let (peer, read_half) = Peer::new(
            peer_id,
            addr,
            stream,
            node_type,
            best_height,
            best_hash,
            chain_state.read().await.genesis_hash,
            connection_nonce,
            false, // is_outbound = false for incoming connections
        );

        // Add peer to peer list and set state to Connected
        {
            let mut peers_guard = peers.write().await;
            peers_guard.insert(peer_id, peer);
            // Update peer state to Connected after successful handshake
            if let Some(p) = peers_guard.get_mut(&peer_id) {
                p.state = PeerState::Connected;
            }
        }

        // Add peer to router for equilibrium-based broadcast selection
        {
            let peers_guard = peers.read().await;
            if let Some(peer) = peers_guard.get(&peer_id) {
                let mut router_guard = router.write().await;
                router_guard.add_peer(PeerInfo::from(peer));
            }
        }

        // Send PeerConnected event
        let _ = event_tx.send(NetworkEvent::PeerConnected {
            peer_id,
            addr,
            node_type,
            best_height,
            best_hash,
        });

        // Initialize per-peer rate limiter
        {
            let mut limiters = rate_limiters.write().await;
            limiters
                .entry(peer_id)
                .or_insert_with(|| TokenBucket::new(rate_cap, rate_refill));
        }

        // Start message loop for this peer (using read half)
        let peers_clone = peers.clone();
        let router_clone = router.clone();
        let flock_clone = flock_state.clone();
        let pending_clone = pending_requests.clone();
        let seen_clone = seen_messages.clone();
        let chain_state_clone = chain_state.clone();
        let event_tx_clone = event_tx.clone();
        let block_provider_clone = block_provider.clone();
        let peer_id_clone = peer_id;
        let ban_list_clone = ban_list.clone();
        let rate_limiters_clone = rate_limiters.clone();
        let malformed_clone = malformed_strikes.clone();
        let sec_metrics_clone = security_metrics.clone();
        let conn_limiter_clone = connection_limiter.clone();
        let eclipse_clone = eclipse_guard.clone();
        let ip_clone = ip;

        tokio::spawn(async move {
            if let Err(e) = Self::peer_message_loop(
                peer_id_clone,
                read_half,
                peers_clone,
                router_clone,
                flock_clone,
                pending_clone,
                seen_clone,
                chain_state_clone,
                event_tx_clone,
                block_provider_clone,
                ban_list_clone,
                rate_limiters_clone,
                malformed_clone,
                sec_metrics_clone,
            )
            .await
            {
                tracing::error!(
                    "Peer {} message loop error: {}",
                    peer_id_clone
                        .iter()
                        .take(4)
                        .map(|b| format!("{:02x}", b))
                        .collect::<String>(),
                    e
                );
            }
            // Release security slots when peer disconnects
            conn_limiter_clone.write().await.release(ip_clone);
            eclipse_clone.write().await.remove(ip_clone);
        });

        Ok(())
    }

    /// Peer message loop - continuously read and process messages from a peer
    ///
    /// This is the full implementation that:
    /// 1. Continuously reads messages from the read half of the stream
    /// 2. Processes each message through the handler
    /// 3. Handles timeouts and disconnections
    /// 4. Updates peer state and metrics
    #[allow(clippy::too_many_arguments, clippy::type_complexity)]
    async fn peer_message_loop(
        peer_id: PeerId,
        mut read_half: tokio::io::ReadHalf<TcpStream>,
        peers: Arc<RwLock<HashMap<PeerId, Peer>>>,
        router: Arc<RwLock<EquilibriumRouter>>,
        flock_state: Arc<RwLock<FlockState>>,
        pending_requests: Arc<RwLock<HashMap<u64, Instant>>>,
        seen_messages: Arc<RwLock<std::collections::VecDeque<([u8; 32], Instant)>>>,
        chain_state: Arc<RwLock<ChainState>>,
        event_tx: mpsc::UnboundedSender<NetworkEvent>,
        block_provider: Arc<dyn BlockProvider>,
        // Security parameters
        ban_list: Arc<RwLock<BanList>>,
        rate_limiters: Arc<RwLock<HashMap<PeerId, TokenBucket>>>,
        malformed_strikes: Arc<RwLock<HashMap<PeerId, u32>>>,
        security_metrics: Arc<RwLock<NetworkSecurityMetrics>>,
    ) -> Result<(), NetworkError> {
        loop {
            // Check if peer still exists and is connected
            let peer_exists = {
                let peers_guard = peers.read().await;
                peers_guard
                    .get(&peer_id)
                    .map(|p| p.state == PeerState::Connected)
                    .unwrap_or(false)
            };

            if !peer_exists {
                break;
            }

            // Read message with timeout using institutional-grade timeout-aware receive
            let envelope_result = MessageCodec::receive_from_read_half_with_timeout(
                &mut read_half,
                MESSAGE_READ_TIMEOUT,
            )
            .await;

            let peer_id_short: String = peer_id
                .iter()
                .take(4)
                .map(|b| format!("{:02x}", b))
                .collect();
            let envelope = match envelope_result {
                Ok(envelope) => {
                    // Successful read - reset consecutive timeout counter
                    {
                        let mut peers_guard = peers.write().await;
                        if let Some(p) = peers_guard.get_mut(&peer_id) {
                            p.on_successful_read(envelope.payload.len());
                        }
                    }

                    // Phase 5: Rate limiting — consume one token for this message
                    let rate_allowed = {
                        let mut limiters = rate_limiters.write().await;
                        if let Some(bucket) = limiters.get_mut(&peer_id) {
                            bucket.try_consume()
                        } else {
                            true // No bucket yet = first message, always allow
                        }
                    };

                    if !rate_allowed {
                        let strikes = {
                            let limiters = rate_limiters.read().await;
                            limiters.get(&peer_id).map(|b| b.strikes()).unwrap_or(0)
                        };
                        security_metrics.write().await.messages_dropped_rate_limit += 1;

                        if strikes >= SECURITY_RATE_STRIKE_THRESHOLD {
                            let peer_addr = peers.read().await.get(&peer_id).map(|p| p.addr);
                            if let Some(paddr) = peer_addr {
                                let ip = paddr.ip();
                                let mut ban = ban_list.write().await;
                                ban.ban_ip_short(ip, "rate limit strikes exceeded");
                                ban.ban_peer_short(&peer_id, "rate limit strikes exceeded");
                            }
                            tracing::warn!(
                                "[SECURITY][RATE_LIMIT] peer={} short-banned after {} strikes",
                                peer_id_short,
                                strikes
                            );
                            break; // disconnect
                        }
                        tracing::debug!(
                            "[SECURITY][RATE_LIMIT] peer={} message dropped (strike {})",
                            peer_id_short,
                            strikes
                        );
                        continue; // drop this message, keep connection
                    }

                    // Update security metrics for successful receive
                    {
                        let mut metrics = security_metrics.write().await;
                        metrics.on_message_received(
                            envelope.msg_type as u8,
                            envelope.payload.len() as u64,
                        );
                    }

                    envelope
                }
                Err(ProtocolError::Timeout(_)) => {
                    // Timeout - track consecutive timeouts and check if we should disconnect
                    let should_disconnect = {
                        let mut peers_guard = peers.write().await;
                        if let Some(p) = peers_guard.get_mut(&peer_id) {
                            p.on_read_timeout() // Returns true if exceeded MAX_CONSECUTIVE_TIMEOUTS
                        } else {
                            true // Peer gone, disconnect
                        }
                    };

                    if should_disconnect {
                        tracing::warn!("[CPP][CONN][TIMEOUT_DISCONNECT] peer={} exceeded max consecutive timeouts", peer_id_short);
                        break;
                    }
                    // Not enough timeouts yet, continue reading
                    continue;
                }
                Err(e) => {
                    // Detailed READ_ERR logging
                    let err_type = match &e {
                        ProtocolError::Io(io_err) => {
                            // Handle EOF gracefully — peer may have closed connection
                            if io_err.kind() == std::io::ErrorKind::UnexpectedEof {
                                tracing::info!("[CPP][CONN][READ_EOF] peer={} - peer closed connection gracefully", peer_id_short);
                                break;
                            }
                            format!("IO({})", io_err.kind())
                        }
                        ProtocolError::InvalidMagic(_)
                        | ProtocolError::InvalidVersion(_)
                        | ProtocolError::InvalidMessageType(_)
                        | ProtocolError::InvalidChecksum => {
                            // Phase 5: Track malformed message strikes → may lead to ban
                            let strikes = {
                                let mut s = malformed_strikes.write().await;
                                let count = s.entry(peer_id).or_insert(0);
                                *count += 1;
                                *count
                            };
                            security_metrics.write().await.messages_dropped_malformed += 1;

                            if strikes >= SECURITY_MALFORMED_STRIKE_THRESHOLD {
                                let peer_addr = peers.read().await.get(&peer_id).map(|p| p.addr);
                                if let Some(paddr) = peer_addr {
                                    let ip = paddr.ip();
                                    let mut ban = ban_list.write().await;
                                    ban.ban_ip(ip, "malformed messages");
                                    ban.ban_peer(&peer_id, "malformed messages");
                                    security_metrics.write().await.peers_banned += 1;
                                }
                                tracing::warn!("[SECURITY][MALFORMED] peer={} banned after {} malformed messages",
                                    peer_id_short, strikes);
                            }

                            match &e {
                                ProtocolError::InvalidMagic(_) => "InvalidMagic",
                                ProtocolError::InvalidVersion(_) => "InvalidVersion",
                                ProtocolError::InvalidMessageType(_) => "InvalidMsgType",
                                _ => "InvalidChecksum",
                            }
                            .to_string()
                        }
                        ProtocolError::DeprecatedVersion(v) => format!("DeprecatedVersion({})", v),
                        ProtocolError::MessageTooLarge(sz) => {
                            security_metrics.write().await.messages_dropped_size_limit += 1;
                            format!("TooLarge({})", sz)
                        }
                        ProtocolError::SerializationError(_) => "SerializeErr".to_string(),
                        ProtocolError::DeserializationError(_) => "DeserializeErr".to_string(),
                        ProtocolError::Timeout(_) => "Timeout".to_string(), // Should not reach here
                    };
                    tracing::error!(
                        "[CPP][CONN][READ_ERR] peer={} stage=MessageRead err_type={} err={}",
                        peer_id_short,
                        err_type,
                        e
                    );
                    break;
                }
            };

            // Process message
            let peers_clone = peers.clone();
            let router_clone = router.clone();
            let flock_clone = flock_state.clone();
            let pending_clone = pending_requests.clone();
            let seen_clone = seen_messages.clone();
            let chain_state_clone = chain_state.clone();
            let event_tx_clone = event_tx.clone();
            let block_provider_clone = block_provider.clone();
            let peer_id_clone = peer_id;

            if let Err(e) = Self::handle_peer_message(
                peer_id_clone,
                envelope,
                peers_clone,
                router_clone,
                flock_clone,
                pending_clone,
                seen_clone,
                chain_state_clone,
                event_tx_clone,
                block_provider_clone,
            )
            .await
            {
                tracing::error!(
                    "Error handling message from peer {}: {}",
                    peer_id_clone
                        .iter()
                        .take(4)
                        .map(|b| format!("{:02x}", b))
                        .collect::<String>(),
                    e
                );
            }
        }

        // Remove peer on disconnect with proper cleanup
        let mut peers_guard = peers.write().await;
        if let Some(mut peer) = peers_guard.remove(&peer_id) {
            peer.shutdown(); // Signal write task to stop
            let mut router_guard = router.write().await;
            router_guard.remove_peer(&peer_id);
            let _ = event_tx.send(NetworkEvent::PeerDisconnected {
                peer_id,
                reason: "Connection closed".to_string(),
            });
        }

        // Clean up per-peer security state
        rate_limiters.write().await.remove(&peer_id);
        malformed_strikes.write().await.remove(&peer_id);
        security_metrics.write().await.on_peer_disconnect();

        Ok(())
    }

    /// Handle incoming message from peer
    #[allow(clippy::too_many_arguments, clippy::type_complexity)]
    async fn handle_peer_message(
        peer_id: PeerId,
        envelope: MessageEnvelope,
        peers: Arc<RwLock<HashMap<PeerId, Peer>>>,
        router: Arc<RwLock<EquilibriumRouter>>,
        flock_state: Arc<RwLock<FlockState>>,
        pending_requests: Arc<RwLock<HashMap<u64, Instant>>>,
        seen_messages: Arc<RwLock<std::collections::VecDeque<([u8; 32], Instant)>>>,
        _chain_state: Arc<RwLock<ChainState>>,
        event_tx: mpsc::UnboundedSender<NetworkEvent>,
        block_provider: Arc<dyn BlockProvider>,
    ) -> Result<(), NetworkError> {
        // Update peer's last_seen
        {
            let mut peers_guard = peers.write().await;
            if let Some(peer) = peers_guard.get_mut(&peer_id) {
                peer.on_message_received(envelope.payload.len());
            }
        }

        match envelope.msg_type {
            MessageType::Status => {
                Self::handle_status(
                    peer_id,
                    &envelope,
                    peers,
                    router,
                    flock_state,
                    event_tx,
                    block_provider.clone(),
                )
                .await?;
            }
            MessageType::GetBlocks => {
                Self::handle_get_blocks(peer_id, &envelope, peers, block_provider).await?;
            }
            MessageType::Blocks => {
                Self::handle_blocks(
                    peer_id,
                    &envelope,
                    pending_requests,
                    event_tx,
                    block_provider.clone(),
                )
                .await?;
            }
            MessageType::NewBlock => {
                Self::handle_new_block(
                    peer_id,
                    &envelope,
                    seen_messages.clone(),
                    event_tx,
                    block_provider.clone(),
                )
                .await?;
            }
            MessageType::NewTransaction => {
                Self::handle_new_transaction(
                    peer_id,
                    &envelope,
                    seen_messages.clone(),
                    event_tx,
                    block_provider.clone(),
                )
                .await?;
            }
            MessageType::Ping => {
                Self::handle_ping(peer_id, &envelope, peers).await?;
            }
            MessageType::Pong => {
                Self::handle_pong(peer_id, &envelope, peers).await?;
            }
            MessageType::Disconnect => {
                // Peer wants to disconnect gracefully
                return Err(NetworkError::InvalidHandshake(
                    "Peer requested disconnect".to_string(),
                ));
            }
            _ => {
                // Unknown or unsupported message type
                tracing::warn!(
                    "Unknown message type from peer {}: {:?}",
                    peer_id
                        .iter()
                        .take(4)
                        .map(|b| format!("{:02x}", b))
                        .collect::<String>(),
                    envelope.msg_type
                );
            }
        }

        Ok(())
    }

    /// Perform handshake with peer (incoming connection)
    ///
    /// IMPORTANT: This function now has timeouts matching connect_bootnode() to prevent
    /// silent hangs when the remote peer doesn't respond. This fixes the asymmetric
    /// timeout issue where outgoing connections had timeouts but incoming did not.
    async fn handshake(
        stream: &mut TcpStream,
        local_peer_id: PeerId,
        chain_state: &ChainState,
        signing_key: Option<&SigningKey>,
    ) -> Result<(PeerId, NodeType, u64, Hash), NetworkError> {
        tracing::debug!(
            "[CPP][HANDSHAKE] Waiting for Hello message (timeout: {:?})...",
            crate::cpp::config::HANDSHAKE_TIMEOUT
        );

        // Receive Hello message WITH TIMEOUT (fixes silent hang issue)
        let envelope = match tokio::time::timeout(
            crate::cpp::config::HANDSHAKE_TIMEOUT,
            MessageCodec::receive(stream),
        )
        .await
        {
            Ok(Ok(e)) => e,
            Ok(Err(e)) => {
                tracing::error!("[CPP][HANDSHAKE] Hello receive failed: {}", e);
                return Err(NetworkError::Protocol(e));
            }
            Err(_) => {
                tracing::warn!(
                    "[CPP][HANDSHAKE] Hello receive timeout - peer did not send Hello in time"
                );
                return Err(NetworkError::Timeout);
            }
        };
        tracing::debug!("[CPP][HANDSHAKE] Received Hello message");

        let hello: HelloMessage = envelope.deserialize().map_err(NetworkError::Protocol)?;

        // Validate genesis hash
        if hello.genesis_hash != chain_state.genesis_hash {
            tracing::error!(
                "[CPP][HANDSHAKE] Genesis hash mismatch: expected {:?}, got {:?}",
                chain_state.genesis_hash,
                hello.genesis_hash
            );
            return Err(NetworkError::InvalidHandshake(format!(
                "Genesis hash mismatch: expected {:?}, got {:?}",
                chain_state.genesis_hash, hello.genesis_hash
            )));
        }

        // Convert node_type from u8
        let node_type = NodeType::from_u8(hello.node_type)
            .map_err(|e| NetworkError::InvalidHandshake(format!("Invalid node type: {}", e)))?;

        // Send HelloAck with connection nonce for tie-breaking + optional auth
        let ack_timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let ack_nonce = rand::random::<u64>();

        let (ack_ed25519_pubkey, ack_auth_signature) = if let Some(sk) = signing_key {
            sign_hello(
                sk,
                &chain_state.genesis_hash,
                ack_timestamp,
                ack_nonce,
                &local_peer_id,
            )
        } else {
            ([0u8; 32], [0u8; 64])
        };

        let hello_ack = HelloAckMessage {
            version: crate::cpp::config::VERSION,
            peer_id: local_peer_id,
            best_height: chain_state.best_height,
            best_hash: chain_state.best_hash,
            genesis_hash: chain_state.genesis_hash,
            node_type: NodeType::Full.as_u8(), // TODO: Get from config
            timestamp: ack_timestamp,
            connection_nonce: ack_nonce,
            ed25519_pubkey: ack_ed25519_pubkey,
            auth_signature: ack_auth_signature,
        };

        tracing::debug!(
            "[CPP][HANDSHAKE] Sending HelloAck (timeout: {:?})...",
            crate::cpp::config::HANDSHAKE_TIMEOUT
        );

        // Send HelloAck WITH TIMEOUT (fixes silent hang on write)
        match tokio::time::timeout(
            crate::cpp::config::HANDSHAKE_TIMEOUT,
            MessageCodec::send_hello_ack(stream, &hello_ack),
        )
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                tracing::error!("[CPP][HANDSHAKE] HelloAck send failed: {}", e);
                return Err(NetworkError::Protocol(e));
            }
            Err(_) => {
                tracing::warn!("[CPP][HANDSHAKE] HelloAck send timeout - peer not accepting data");
                return Err(NetworkError::Timeout);
            }
        }

        tracing::debug!(
            "[CPP][HANDSHAKE] HelloAck sent successfully, peer_id={}",
            hello
                .peer_id
                .iter()
                .take(4)
                .map(|b| format!("{:02x}", b))
                .collect::<String>()
        );

        Ok((hello.peer_id, node_type, hello.best_height, hello.best_hash))
    }

    /// Connect to bootnode
    async fn connect_bootnode(&mut self, addr: SocketAddr) -> Result<(), NetworkError> {
        // Check if peer already connected BEFORE attempting connection
        // This prevents duplicate connections when both nodes connect simultaneously
        {
            let peers = self.peers.read().await;
            // Check if any peer has this address (in case peer_id isn't known yet)
            for peer in peers.values() {
                if peer.addr == addr {
                    return Err(NetworkError::InvalidHandshake(
                        "Peer already connected".to_string(),
                    ));
                }
            }
        }

        // Connect TCP stream with timeout
        tracing::info!("[CPP][BOOTNODE] Connecting TCP to {}...", addr);
        let mut stream = match tokio::time::timeout(
            crate::cpp::config::CONNECTION_TIMEOUT,
            TcpStream::connect(addr),
        )
        .await
        {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => {
                tracing::warn!("[CPP][BOOTNODE] TCP connect failed to {}: {}", addr, e);
                return Err(NetworkError::Io(e));
            }
            Err(_) => {
                tracing::warn!("[CPP][BOOTNODE] TCP connect timeout to {}", addr);
                return Err(NetworkError::Timeout);
            }
        };

        tracing::info!(
            "[CPP][BOOTNODE] TCP connection established to {}, starting handshake...",
            addr
        );

        let state = self.chain_state.read().await;

        // Generate connection nonce for deterministic tie-breaking of simultaneous connections
        let our_connection_nonce = rand::random::<u64>();

        // Send Hello message first (with timeout) — include auth signature if signing key available
        let hello_timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let (hello_ed25519_pubkey, hello_auth_signature) = if let Some(ref sk) = self.signing_key {
            sign_hello(
                sk,
                &state.genesis_hash,
                hello_timestamp,
                our_connection_nonce,
                &self.local_peer_id,
            )
        } else {
            ([0u8; 32], [0u8; 64])
        };

        let hello = HelloMessage {
            version: crate::cpp::config::VERSION,
            peer_id: self.local_peer_id,
            best_height: state.best_height,
            best_hash: state.best_hash,
            genesis_hash: state.genesis_hash,
            node_type: self.config.node_type.as_u8(),
            timestamp: hello_timestamp,
            connection_nonce: our_connection_nonce,
            ed25519_pubkey: hello_ed25519_pubkey,
            auth_signature: hello_auth_signature,
        };

        tracing::debug!("[CPP][BOOTNODE] Sending Hello message to {}...", addr);
        match tokio::time::timeout(
            crate::cpp::config::HANDSHAKE_TIMEOUT,
            MessageCodec::send_hello(&mut stream, &hello),
        )
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                tracing::warn!("[CPP][BOOTNODE] Hello send failed to {}: {}", addr, e);
                return Err(NetworkError::Protocol(e));
            }
            Err(_) => {
                tracing::warn!("[CPP][BOOTNODE] Hello send timeout to {}", addr);
                return Err(NetworkError::Timeout);
            }
        }

        tracing::debug!("[CPP][BOOTNODE] Waiting for HelloAck from {}...", addr);
        // Receive HelloAck (with timeout)
        let envelope = match tokio::time::timeout(
            crate::cpp::config::HANDSHAKE_TIMEOUT,
            MessageCodec::receive(&mut stream),
        )
        .await
        {
            Ok(Ok(e)) => e,
            Ok(Err(e)) => {
                tracing::warn!(
                    "[CPP][BOOTNODE] HelloAck receive failed from {}: {}",
                    addr,
                    e
                );
                return Err(NetworkError::Protocol(e));
            }
            Err(_) => {
                tracing::warn!("[CPP][BOOTNODE] HelloAck receive timeout from {}", addr);
                return Err(NetworkError::Timeout);
            }
        };

        tracing::debug!(
            "[CPP][BOOTNODE] Received HelloAck from {}, validating...",
            addr
        );
        let hello_ack: HelloAckMessage = envelope.deserialize().map_err(NetworkError::Protocol)?;

        // Validate genesis hash
        if hello_ack.genesis_hash != state.genesis_hash {
            return Err(NetworkError::InvalidHandshake(
                "Genesis hash mismatch".to_string(),
            ));
        }

        // Convert node_type from u8
        let node_type = NodeType::from_u8(hello_ack.node_type)
            .map_err(|e| NetworkError::InvalidHandshake(format!("Invalid node type: {}", e)))?;

        drop(state);

        // Check again if peer already connected (race condition check)
        let peer_id = hello_ack.peer_id;
        {
            let peers = self.peers.read().await;
            if peers.contains_key(&peer_id) {
                return Err(NetworkError::InvalidHandshake(
                    "Peer already connected".to_string(),
                ));
            }
        }

        // Create peer instance (returns peer and read half)
        // For outbound connections: is_outbound = true, use our nonce for tie-breaking
        let (peer, read_half) = Peer::new(
            peer_id,
            addr,
            stream,
            node_type,
            hello_ack.best_height,
            hello_ack.best_hash,
            self.chain_state.read().await.genesis_hash,
            our_connection_nonce,
            true, // is_outbound = true for bootnode connections
        );

        // Add peer to peer list and set state to Connected
        {
            let mut peers = self.peers.write().await;
            // Final check before inserting (double-check for race condition)
            if peers.contains_key(&peer_id) {
                return Err(NetworkError::InvalidHandshake(
                    "Peer already connected".to_string(),
                ));
            }
            peers.insert(peer_id, peer);
            // Update peer state to Connected after successful handshake
            if let Some(p) = peers.get_mut(&peer_id) {
                p.state = PeerState::Connected;
            }
        }

        // Add peer to router for equilibrium-based broadcast selection
        {
            let peers_guard = self.peers.read().await;
            if let Some(peer) = peers_guard.get(&peer_id) {
                let mut router = self.router.write().await;
                router.add_peer(PeerInfo::from(peer));
            }
        }

        // Send PeerConnected event
        let _ = self.event_tx.send(NetworkEvent::PeerConnected {
            peer_id,
            addr,
            node_type,
            best_height: hello_ack.best_height,
            best_hash: hello_ack.best_hash,
        });

        // Eclipse guard for outbound connections too
        {
            let mut eclipse = self.eclipse_guard.write().await;
            if !eclipse.try_add(addr.ip()) {
                tracing::warn!(
                    "[SECURITY][ECLIPSE] Not adding outbound peer {} — subnet at capacity",
                    addr
                );
                // Don't error out hard for outbound — just log. The connection was already made.
            }
        }

        // Initialize per-peer rate limiter for outbound peer
        {
            let cap = self.config.rate_bucket_capacity;
            let refill = self.config.rate_msgs_per_sec;
            let mut limiters = self.rate_limiters.write().await;
            limiters
                .entry(peer_id)
                .or_insert_with(|| TokenBucket::new(cap, refill));
        }

        // Start message loop for this peer
        let peers_clone = self.peers.clone();
        let router_clone = self.router.clone();
        let flock_clone = self.flock_state.clone();
        let pending_clone = self.pending_requests.clone();
        let seen_clone = self.seen_messages.clone();
        let chain_state_clone = self.chain_state.clone();
        let block_provider_clone = self.block_provider.clone();
        let event_tx_clone = self.event_tx.clone();
        let peer_id_clone = peer_id;
        let ban_clone = self.ban_list.clone();
        let rl_clone = self.rate_limiters.clone();
        let ms_clone = self.malformed_strikes.clone();
        let sm_clone = self.security_metrics.clone();
        let cl_clone = self.connection_limiter.clone();
        let eg_clone = self.eclipse_guard.clone();
        let outbound_ip = addr.ip();

        tokio::spawn(async move {
            if let Err(e) = Self::peer_message_loop(
                peer_id_clone,
                read_half,
                peers_clone,
                router_clone,
                flock_clone,
                pending_clone,
                seen_clone,
                chain_state_clone,
                event_tx_clone,
                block_provider_clone,
                ban_clone,
                rl_clone,
                ms_clone,
                sm_clone,
            )
            .await
            {
                tracing::error!(
                    "Peer {} message loop error: {}",
                    peer_id_clone
                        .iter()
                        .take(4)
                        .map(|b| format!("{:02x}", b))
                        .collect::<String>(),
                    e
                );
            }
            // Release eclipse slot for outbound peer
            eg_clone.write().await.remove(outbound_ip);
            let _ = cl_clone; // outbound not tracked in connection_limiter (inbound only)
        });

        Ok(())
    }

    /// Add peer to peer list (internal helper - now handled inline)
    #[allow(dead_code)]
    async fn add_peer(&self, peer: Peer) {
        let peer_id = peer.id;
        let peer_info = PeerInfo::from(&peer);
        let mut peers = self.peers.write().await;
        peers.insert(peer_id, peer);

        // Add to router for equilibrium broadcast selection
        let mut router = self.router.write().await;
        router.add_peer(peer_info);
    }

    /// Remove peer from peer list
    async fn remove_peer(&self, peer_id: &PeerId, reason: &str) {
        let mut peers = self.peers.write().await;
        if let Some(mut peer) = peers.remove(peer_id) {
            // Signal write task to stop (prevents task leak)
            peer.shutdown();

            // Remove from router
            let mut router = self.router.write().await;
            router.remove_peer(peer_id);

            // Send disconnect event
            let _ = self.event_tx.send(NetworkEvent::PeerDisconnected {
                peer_id: *peer_id,
                reason: reason.to_string(),
            });
        }
    }

    // =========================================================================
    // Command Handling
    // =========================================================================

    /// Handle command from node service
    async fn handle_command(&mut self, command: NetworkCommand) -> Result<(), NetworkError> {
        match command {
            NetworkCommand::ConnectBootnode { addr } => {
                self.connect_bootnode(addr).await?;
            }

            NetworkCommand::BroadcastBlock { block } => {
                self.broadcast_block(block).await?;
            }

            NetworkCommand::BroadcastTransaction { transaction } => {
                self.broadcast_transaction(transaction).await?;
            }

            NetworkCommand::RequestBlocks {
                peer_id,
                from_height,
                to_height,
                request_id,
            } => {
                self.request_blocks(peer_id, from_height, to_height, request_id)
                    .await?;
            }

            NetworkCommand::RequestHeaders {
                peer_id,
                from_height,
                to_height,
                request_id,
            } => {
                self.request_headers(peer_id, from_height, to_height, request_id)
                    .await?;
            }

            NetworkCommand::DisconnectPeer { peer_id, reason } => {
                self.remove_peer(&peer_id, &reason).await;
            }

            NetworkCommand::UpdateChainState {
                best_height,
                best_hash,
            } => {
                let mut state = self.chain_state.write().await;
                state.best_height = best_height;
                state.best_hash = best_hash;
                let genesis = state.genesis_hash;
                drop(state);

                // Advance flock epoch if height crosses boundary
                let mut flock = self.flock_state.write().await;
                let new_epoch = best_height / crate::cpp::flock::FLOCK_EPOCH_BLOCKS;
                if new_epoch > flock.epoch {
                    *flock = FlockState::new(&genesis, best_height, &self.local_peer_id);
                }
            }
        }

        Ok(())
    }

    // =========================================================================
    // Message Handlers
    // =========================================================================

    /// Handle Status message from peer
    async fn handle_status(
        peer_id: PeerId,
        envelope: &MessageEnvelope,
        peers: Arc<RwLock<HashMap<PeerId, Peer>>>,
        router: Arc<RwLock<EquilibriumRouter>>,
        flock_state: Arc<RwLock<FlockState>>,
        event_tx: mpsc::UnboundedSender<NetworkEvent>,
        _block_provider: Arc<dyn BlockProvider>,
    ) -> Result<(), NetworkError> {
        let status: StatusMessage = envelope.deserialize().map_err(NetworkError::Protocol)?;

        let node_type = NodeType::from_u8(status.node_type)
            .map_err(|e| NetworkError::InvalidHandshake(format!("Invalid node type: {}", e)))?;

        // Update peer status
        {
            let mut peers_guard = peers.write().await;
            if let Some(peer) = peers_guard.get_mut(&peer_id) {
                peer.update_status(status.best_height, status.best_hash, node_type);
            }
        }

        // Propagate flock state to router for murmuration coordination
        if let Some(flock) = &status.flock_state {
            let mut router_guard = router.write().await;
            router_guard.update_peer_flock(&peer_id, flock, status.best_height);
        }

        // Update swarm metrics from peer heights
        {
            let peers_guard = peers.read().await;
            let heights: Vec<u64> = peers_guard.values().map(|p| p.best_height).collect();
            if !heights.is_empty() {
                let mut flock_guard = flock_state.write().await;
                flock_guard.update_from_peers(&heights);
            }
        }

        // Send StatusUpdate event
        let _ = event_tx.send(NetworkEvent::StatusUpdate {
            peer_id,
            best_height: status.best_height,
            best_hash: status.best_hash,
            node_type,
        });

        Ok(())
    }

    /// Handle GetBlocks request from peer - with chunking support for M2 fix
    async fn handle_get_blocks(
        peer_id: PeerId,
        envelope: &MessageEnvelope,
        peers: Arc<RwLock<HashMap<PeerId, Peer>>>,
        block_provider: Arc<dyn BlockProvider>,
    ) -> Result<(), NetworkError> {
        let get_blocks: GetBlocksMessage =
            envelope.deserialize().map_err(NetworkError::Protocol)?;

        let peer_id_short: String = peer_id
            .iter()
            .take(4)
            .map(|b| format!("{:02x}", b))
            .collect();

        // Clamp request to MAX_BLOCKS_PER_RESPONSE to prevent large frames causing "early eof"
        let requested_count = get_blocks.to_height.saturating_sub(get_blocks.from_height) + 1;
        let clamped_to = if requested_count > MAX_BLOCKS_PER_RESPONSE {
            let clamped = get_blocks.from_height + MAX_BLOCKS_PER_RESPONSE - 1;
            tracing::info!("[CPP][SYNC] Clamping GetBlocks: peer={} requested {}-{} ({} blocks), serving {}-{} ({} blocks)",
                peer_id_short, get_blocks.from_height, get_blocks.to_height, requested_count,
                get_blocks.from_height, clamped, MAX_BLOCKS_PER_RESPONSE);
            clamped
        } else {
            get_blocks.to_height
        };

        // Get blocks from the canonical chain via block provider
        let blocks = block_provider.get_blocks_range(get_blocks.from_height, clamped_to);
        tracing::info!(
            "[CPP][SYNC] Serving {} blocks (heights {}-{}) to peer {}",
            blocks.len(),
            get_blocks.from_height,
            clamped_to,
            peer_id_short
        );

        let blocks_msg = BlocksMessage {
            blocks,
            request_id: get_blocks.request_id,
        };

        // Send Blocks response
        let peers_guard = peers.read().await;
        if let Some(peer) = peers_guard.get(&peer_id) {
            let envelope = MessageEnvelope::new(MessageType::Blocks, &blocks_msg)
                .map_err(NetworkError::Protocol)?;
            let data = envelope.encode();

            tracing::info!(
                "[CPP][SYNC] Sending Blocks response: peer={} frame_len={} bytes",
                peer_id_short,
                data.len()
            );

            peer.send_message(data.clone())
                .map_err(NetworkError::InvalidHandshake)?;

            drop(peers_guard);

            // Update peer stats
            let mut peers_write = peers.write().await;
            if let Some(p) = peers_write.get_mut(&peer_id) {
                p.on_message_sent(data.len());
            }
        }

        Ok(())
    }

    /// Handle Blocks response from peer
    async fn handle_blocks(
        peer_id: PeerId,
        envelope: &MessageEnvelope,
        pending_requests: Arc<RwLock<HashMap<u64, Instant>>>,
        event_tx: mpsc::UnboundedSender<NetworkEvent>,
        _block_provider: Arc<dyn BlockProvider>,
    ) -> Result<(), NetworkError> {
        let blocks_msg: BlocksMessage = envelope.deserialize().map_err(NetworkError::Protocol)?;

        // Remove fulfilled request from pending
        {
            let mut pending = pending_requests.write().await;
            pending.remove(&blocks_msg.request_id);
        }

        // Send BlocksReceived event
        let _ = event_tx.send(NetworkEvent::BlocksReceived {
            blocks: blocks_msg.blocks,
            request_id: blocks_msg.request_id,
            peer_id,
        });

        Ok(())
    }

    /// Check if a message has been seen before (deduplication)
    /// Returns true if already seen, false if new (and adds to cache)
    #[allow(clippy::type_complexity)]
    async fn check_seen(
        seen_messages: &Arc<RwLock<std::collections::VecDeque<([u8; 32], Instant)>>>,
        payload: &[u8],
    ) -> bool {
        let hash = *blake3::hash(payload).as_bytes();
        let now = Instant::now();
        let mut cache = seen_messages.write().await;

        // Evict expired entries (older than 60s)
        while let Some((_, ts)) = cache.front() {
            if now.duration_since(*ts) > Duration::from_secs(60) {
                cache.pop_front();
            } else {
                break;
            }
        }

        // Check if already seen
        if cache.iter().any(|(h, _)| *h == hash) {
            return true; // Duplicate
        }

        // Add to cache (cap at 5000)
        if cache.len() >= 5000 {
            cache.pop_front();
        }
        cache.push_back((hash, now));
        false
    }

    /// Handle NewBlock announcement from peer
    #[allow(clippy::type_complexity)]
    async fn handle_new_block(
        peer_id: PeerId,
        envelope: &MessageEnvelope,
        seen_messages: Arc<RwLock<std::collections::VecDeque<([u8; 32], Instant)>>>,
        event_tx: mpsc::UnboundedSender<NetworkEvent>,
        _block_provider: Arc<dyn BlockProvider>,
    ) -> Result<(), NetworkError> {
        // Dedup check
        if Self::check_seen(&seen_messages, &envelope.payload).await {
            return Ok(()); // Already seen, drop silently
        }

        let new_block: NewBlockMessage = envelope.deserialize().map_err(NetworkError::Protocol)?;

        // Send BlockReceived event
        let _ = event_tx.send(NetworkEvent::BlockReceived {
            block: new_block.block,
            peer_id,
        });

        Ok(())
    }

    /// Handle NewTransaction announcement from peer
    #[allow(clippy::type_complexity)]
    async fn handle_new_transaction(
        peer_id: PeerId,
        envelope: &MessageEnvelope,
        seen_messages: Arc<RwLock<std::collections::VecDeque<([u8; 32], Instant)>>>,
        event_tx: mpsc::UnboundedSender<NetworkEvent>,
        _block_provider: Arc<dyn BlockProvider>,
    ) -> Result<(), NetworkError> {
        // Dedup check
        if Self::check_seen(&seen_messages, &envelope.payload).await {
            return Ok(()); // Already seen, drop silently
        }

        let new_tx: NewTransactionMessage =
            envelope.deserialize().map_err(NetworkError::Protocol)?;

        // Send TransactionReceived event
        let _ = event_tx.send(NetworkEvent::TransactionReceived {
            transaction: new_tx.transaction,
            peer_id,
        });

        Ok(())
    }

    /// Handle Ping message from peer
    async fn handle_ping(
        peer_id: PeerId,
        envelope: &MessageEnvelope,
        peers: Arc<RwLock<HashMap<PeerId, Peer>>>,
    ) -> Result<(), NetworkError> {
        let ping: PingMessage = envelope.deserialize().map_err(NetworkError::Protocol)?;

        // Send Pong response
        let peers_guard = peers.read().await;
        if let Some(peer) = peers_guard.get(&peer_id) {
            let pong = PongMessage {
                nonce: ping.nonce,
                timestamp: coinject_core::unix_now_secs(),
            };

            let envelope =
                MessageEnvelope::new(MessageType::Pong, &pong).map_err(NetworkError::Protocol)?;
            let data = envelope.encode();

            peer.send_message(data.clone())
                .map_err(NetworkError::InvalidHandshake)?;

            drop(peers_guard);

            // Update peer stats
            let mut peers_write = peers.write().await;
            if let Some(p) = peers_write.get_mut(&peer_id) {
                p.on_message_sent(data.len());
            }
        }

        Ok(())
    }

    /// Handle Pong message from peer
    async fn handle_pong(
        peer_id: PeerId,
        envelope: &MessageEnvelope,
        peers: Arc<RwLock<HashMap<PeerId, Peer>>>,
    ) -> Result<(), NetworkError> {
        let _pong: PongMessage = envelope.deserialize().map_err(NetworkError::Protocol)?;

        // Update RTT and flow control
        let mut peers_guard = peers.write().await;
        if let Some(peer) = peers_guard.get_mut(&peer_id) {
            if let Some(ping_time) = peer.last_ping {
                let rtt = ping_time.elapsed();
                peer.on_ack(rtt);
                peer.pending_ping_nonce = None;
            }
        }

        Ok(())
    }

    // =========================================================================
    // Broadcasting
    // =========================================================================

    /// Broadcast block to selected peers using router + PeerSelector
    async fn broadcast_block(&self, block: Block) -> Result<(), NetworkError> {
        // Select peers via equilibrium router, then filter through PeerSelector
        let router_peers = self.select_broadcast_peers(MessageType::NewBlock).await;
        let peers_guard = self.peers.read().await;
        let peer_refs: Vec<&Peer> = router_peers
            .iter()
            .filter_map(|id| peers_guard.get(id))
            .collect();
        let peer_ids = if !peer_refs.is_empty() {
            PeerSelector::select_for_propagation(&peer_refs, router_peers.len())
        } else {
            router_peers
        };
        drop(peers_guard);

        // Collect sent data info first (to avoid holding read lock while getting write lock)
        let mut sent_info: Vec<(PeerId, usize)> = Vec::new();

        // Send NewBlock message to each peer
        {
            let peers = self.peers.read().await;
            for peer_id in peer_ids {
                if let Some(peer) = peers.get(&peer_id) {
                    let new_block_msg = NewBlockMessage {
                        block: block.clone(),
                    };

                    // Serialize message
                    let envelope = MessageEnvelope::new(MessageType::NewBlock, &new_block_msg)
                        .map_err(NetworkError::Protocol)?;
                    let data = envelope.encode();

                    // Send via channel
                    if let Err(e) = peer.send_message(data.clone()) {
                        tracing::warn!(
                            "Failed to send NewBlock to peer {}: {}",
                            peer_id
                                .iter()
                                .take(4)
                                .map(|b| format!("{:02x}", b))
                                .collect::<String>(),
                            e
                        );
                        continue;
                    }

                    // Track for stats update (will update after dropping read lock)
                    sent_info.push((peer_id, data.len()));
                }
            }
        } // Read lock dropped here

        // Update peer stats (now we can safely get write lock)
        if !sent_info.is_empty() {
            let mut peers_guard = self.peers.write().await;
            for (peer_id, data_len) in sent_info {
                if let Some(p) = peers_guard.get_mut(&peer_id) {
                    p.on_message_sent(data_len);
                }
            }
        }

        Ok(())
    }

    /// Broadcast transaction to selected peers
    async fn broadcast_transaction(&self, tx: Transaction) -> Result<(), NetworkError> {
        // Select peers (equilibrium fanout)
        let peer_ids = self
            .select_broadcast_peers(MessageType::NewTransaction)
            .await;

        // Collect sent data info first (to avoid holding read lock while getting write lock)
        let mut sent_info: Vec<(PeerId, usize)> = Vec::new();

        // Send NewTransaction message to each peer
        {
            let peers = self.peers.read().await;
            for peer_id in peer_ids {
                if let Some(peer) = peers.get(&peer_id) {
                    let new_tx_msg = NewTransactionMessage {
                        transaction: tx.clone(),
                    };

                    // Serialize message
                    let envelope = MessageEnvelope::new(MessageType::NewTransaction, &new_tx_msg)
                        .map_err(NetworkError::Protocol)?;
                    let data = envelope.encode();

                    // Send via channel
                    if let Err(e) = peer.send_message(data.clone()) {
                        tracing::warn!(
                            "Failed to send NewTransaction to peer {}: {}",
                            peer_id
                                .iter()
                                .take(4)
                                .map(|b| format!("{:02x}", b))
                                .collect::<String>(),
                            e
                        );
                        continue;
                    }

                    // Track for stats update (will update after dropping read lock)
                    sent_info.push((peer_id, data.len()));
                }
            }
        } // Read lock dropped here

        // Update peer stats (now we can safely get write lock)
        if !sent_info.is_empty() {
            let mut peers_guard = self.peers.write().await;
            for (peer_id, data_len) in sent_info {
                if let Some(p) = peers_guard.get_mut(&peer_id) {
                    p.on_message_sent(data_len);
                }
            }
        }

        Ok(())
    }

    /// Select peers for broadcast using equilibrium router with murmuration
    async fn select_broadcast_peers(&self, _msg_type: MessageType) -> Vec<PeerId> {
        let router = self.router.read().await;
        if router.peer_count() == 0 {
            // Fallback: if router is empty, use all connected peers
            let peers = self.peers.read().await;
            return peers.keys().copied().collect();
        }
        let chain_state = self.chain_state.read().await;
        let flock_state = self.flock_state.read().await;
        router.select_broadcast_peers_flock(chain_state.best_height, flock_state.phase)
    }

    // =========================================================================
    // Sync Requests
    // =========================================================================

    /// Request blocks from peer
    async fn request_blocks(
        &self,
        peer_id: PeerId,
        from_height: u64,
        to_height: u64,
        request_id: u64,
    ) -> Result<(), NetworkError> {
        let peers = self.peers.read().await;
        let peer = peers
            .get(&peer_id)
            .ok_or(NetworkError::PeerNotFound(peer_id))?;

        // Create GetBlocks message
        let get_blocks = GetBlocksMessage {
            from_height,
            to_height,
            request_id,
        };

        // Serialize and send message
        let envelope = MessageEnvelope::new(MessageType::GetBlocks, &get_blocks)
            .map_err(NetworkError::Protocol)?;
        let data = envelope.encode();

        peer.send_message(data.clone())
            .map_err(NetworkError::InvalidHandshake)?;

        drop(peers);

        // Update peer stats
        {
            let mut peers_guard = self.peers.write().await;
            if let Some(p) = peers_guard.get_mut(&peer_id) {
                p.on_message_sent(data.len());
            }
        }

        // Track pending request
        {
            let mut pending = self.pending_requests.write().await;
            pending.insert(request_id, Instant::now());
        }

        Ok(())
    }

    /// Request headers from peer (light sync)
    ///
    /// NOTE: Header-only sync is not yet implemented. This logs a warning
    /// so operators know the feature is pending.
    async fn request_headers(
        &self,
        peer_id: PeerId,
        from_height: u64,
        to_height: u64,
        _request_id: u64,
    ) -> Result<(), NetworkError> {
        let peers = self.peers.read().await;
        let _peer = peers
            .get(&peer_id)
            .ok_or(NetworkError::PeerNotFound(peer_id))?;

        let peer_id_short: String = peer_id
            .iter()
            .take(4)
            .map(|b| format!("{:02x}", b))
            .collect();
        tracing::warn!(
            "[CPP] request_headers not yet implemented: peer={} heights={}-{}",
            peer_id_short,
            from_height,
            to_height
        );

        Ok(())
    }

    // =========================================================================
    // Periodic Maintenance
    // =========================================================================

    /// Send keepalive pings to all peers
    async fn send_pings(&self) {
        use rand::Rng;

        let mut peers = self.peers.write().await;
        for peer in peers.values_mut() {
            // Check if ping is needed
            if peer.needs_ping() {
                let nonce: u64 = rand::thread_rng().gen();
                let ping = PingMessage {
                    nonce,
                    timestamp: coinject_core::unix_now_secs(),
                };

                let envelope = match MessageEnvelope::new(MessageType::Ping, &ping) {
                    Ok(e) => e,
                    Err(e) => {
                        tracing::error!("Failed to create ping envelope: {}", e);
                        continue;
                    }
                };
                let data = envelope.encode();

                if let Err(e) = peer.send_message(data.clone()) {
                    tracing::warn!(
                        "Failed to send ping to peer {}: {}",
                        peer.id
                            .iter()
                            .take(4)
                            .map(|b| format!("{:02x}", b))
                            .collect::<String>(),
                        e
                    );
                    continue;
                }

                peer.last_ping = Some(Instant::now());
                peer.pending_ping_nonce = Some(nonce);
                peer.on_message_sent(data.len());
            }
        }
    }

    /// Remove timed-out peers
    async fn cleanup_stale_peers(&self) {
        let now = Instant::now();
        let mut peers = self.peers.write().await;

        let mut to_remove = vec![];
        for (peer_id, peer) in peers.iter() {
            if now.duration_since(peer.last_seen) > PEER_TIMEOUT {
                to_remove.push(*peer_id);
            }
        }

        if !to_remove.is_empty() {
            let mut router = self.router.write().await;
            for peer_id in &to_remove {
                if let Some(mut peer) = peers.remove(peer_id) {
                    peer.shutdown(); // Signal write task to stop
                    router.remove_peer(peer_id);
                    let _ = self.event_tx.send(NetworkEvent::PeerDisconnected {
                        peer_id: *peer_id,
                        reason: "Timeout".to_string(),
                    });
                }
            }

            // Clean up stale pending requests (TTL: 30s)
            let mut pending = self.pending_requests.write().await;
            pending.retain(|_, instant| instant.elapsed() < Duration::from_secs(30));

            // Clean up per-peer security state for removed peers
            {
                let mut limiters = self.rate_limiters.write().await;
                let mut strikes = self.malformed_strikes.write().await;
                for peer_id in &to_remove {
                    limiters.remove(peer_id);
                    strikes.remove(peer_id);
                }
            }
        }

        // Phase 5: Periodic ban expiry cleanup
        self.ban_list.write().await.cleanup_expired();

        // Log security metrics snapshot periodically
        {
            let m = self.security_metrics.read().await;
            if m.churn() > 0 || m.total_messages_received() > 0 {
                tracing::debug!(
                    "[SECURITY][METRICS] peers={} sent={} recv={} bans={} rate_drops={} malformed_drops={}",
                    m.connected_peers,
                    m.total_messages_sent(),
                    m.total_messages_received(),
                    m.peers_banned,
                    m.messages_dropped_rate_limit,
                    m.messages_dropped_malformed,
                );
            }
        }
    }

    /// Broadcast status to all connected peers
    async fn broadcast_status(&self) {
        let chain_state = self.chain_state.read().await;
        let best_height = chain_state.best_height;
        let best_hash = chain_state.best_hash;
        drop(chain_state);

        let peers = self.peers.read().await;
        let peer_count = peers.len();

        if peer_count == 0 {
            return; // No peers to broadcast to
        }

        tracing::info!(
            "[CPP] Broadcasting Status: height {}, hash {:?} to {} peers",
            best_height,
            best_hash,
            peer_count
        );

        // Get flock state for murmuration coordination
        let flock_state = self.flock_state.read().await;
        let flock_compact = FlockStateCompact::from(&*flock_state);
        drop(flock_state);

        let status = StatusMessage {
            best_height,
            best_hash,
            node_type: self.config.node_type.as_u8(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            flock_state: Some(flock_compact),
        };

        let envelope = match MessageEnvelope::new(MessageType::Status, &status) {
            Ok(e) => e,
            Err(e) => {
                tracing::error!("Failed to create status envelope: {}", e);
                return;
            }
        };
        let data = envelope.encode();

        for peer in peers.values() {
            if let Err(e) = peer.send_message(data.clone()) {
                tracing::warn!("Failed to send status to peer: {}", e);
            }
        }
    }

    /// Check if sync is needed using equilibrium mathematics
    /// This runs every 10 seconds and triggers sync requests if we're behind peers
    async fn check_sync_status(&self) {
        // Get our current height
        let chain_state = self.chain_state.read().await;
        let our_height = chain_state.best_height;
        drop(chain_state);

        // Get peer heights
        let peers = self.peers.read().await;
        if peers.is_empty() {
            return; // No peers to sync from
        }

        // Calculate median peer height (robust to outliers)
        let mut peer_heights: Vec<u64> = peers.values().map(|p| p.best_height).collect();
        peer_heights.sort();

        let median_height = if peer_heights.is_empty() {
            return;
        } else if peer_heights.len().is_multiple_of(2) {
            let mid = peer_heights.len() / 2;
            (peer_heights[mid - 1] + peer_heights[mid]) / 2
        } else {
            peer_heights[peer_heights.len() / 2]
        };

        if median_height <= our_height {
            return; // We're caught up
        }

        let delta_h = median_height - our_height;

        // Sync if we're behind by more than 1 block (reduced from 5 for faster sync)
        if delta_h > 1 {
            tracing::info!(
                "[CPP] Equilibrium sync check: we're at {}, median at {} (delta_h = {})",
                our_height,
                median_height,
                delta_h
            );

            // Select best peer (highest height with good quality)
            let best_peer = peers
                .iter()
                .max_by_key(|(_, p)| p.best_height)
                .map(|(id, p)| (*id, p.best_height));

            drop(peers);

            if let Some((peer_id, peer_height)) = best_peer {
                // Calculate optimal chunk size: capped by MAX_BLOCKS_PER_RESPONSE (16)
                let from_height = our_height + 1;
                let to_height = peer_height.min(our_height + MAX_BLOCKS_PER_RESPONSE);

                tracing::info!(
                    "[CPP] Requesting sync: blocks {}-{} from peer {:?}",
                    from_height,
                    to_height,
                    peer_id
                        .iter()
                        .take(4)
                        .map(|b| format!("{:02x}", b))
                        .collect::<String>()
                );

                // === FIX: Actually request the blocks instead of sending placeholder event ===
                let request_id: u64 = rand::thread_rng().gen();
                if let Err(e) = self
                    .request_blocks(peer_id, from_height, to_height, request_id)
                    .await
                {
                    tracing::error!("[CPP] Failed to request sync blocks: {}", e);
                }
            }
        }
    }

    /// Check if we need to reconnect to bootnodes (for M2 recovery)
    async fn check_bootnode_reconnection(&mut self) {
        // Count HEALTHY peers (not just connected) - quality-based reconnection
        let (total_peers, healthy_peers) = {
            let peers = self.peers.read().await;
            let total = peers.len();
            let healthy = peers
                .values()
                .filter(|p| p.is_healthy()) // Uses quality threshold and half-dead detection
                .count();
            (total, healthy)
        };

        // Log peer health status periodically (only when there are peers)
        if total_peers > 0 {
            tracing::debug!(
                "[CPP][HEALTH] Peers: {} total, {} healthy (min required: {})",
                total_peers,
                healthy_peers,
                MIN_HEALTHY_PEERS
            );
        }

        // If we have enough HEALTHY peers, no need to reconnect
        if healthy_peers >= MIN_HEALTHY_PEERS {
            return;
        }

        // If we have some peers but they're unhealthy, log that
        if total_peers > 0 && healthy_peers < MIN_HEALTHY_PEERS {
            tracing::info!("[CPP][BOOTNODE] Insufficient healthy peers ({}/{} healthy, {} required), attempting reconnection...",
                healthy_peers, total_peers, MIN_HEALTHY_PEERS);
        } else {
            tracing::info!("[CPP][BOOTNODE] No connected peers, checking bootnode reconnection...");
        }

        let now = Instant::now();
        let initial_backoff = Duration::from_secs(1);
        let max_backoff = Duration::from_secs(60);

        for bootnode_str in &self.config.bootnodes.clone() {
            // Parse bootnode address — try direct SocketAddr first, then DNS resolution for hostnames
            let addr: SocketAddr = match bootnode_str.parse() {
                Ok(a) => a,
                Err(_) => {
                    // Try DNS resolution (supports Docker service names like "bootnode:707")
                    match tokio::net::lookup_host(bootnode_str.as_str()).await {
                        Ok(mut addrs) => match addrs.next() {
                            Some(a) => a,
                            None => {
                                tracing::warn!(
                                    "[CPP][BOOTNODE] No addresses resolved for: {}",
                                    bootnode_str
                                );
                                continue;
                            }
                        },
                        Err(e) => {
                            tracing::warn!(
                                "[CPP][BOOTNODE] Failed to resolve bootnode '{}': {}",
                                bootnode_str,
                                e
                            );
                            continue;
                        }
                    }
                }
            };

            // Check if we should attempt to reconnect (exponential backoff)
            let should_attempt = {
                let last_attempt = self.last_bootnode_attempt.read().await;
                let backoff = self.bootnode_backoff.read().await;

                if let Some(last) = last_attempt.get(&addr) {
                    let current_backoff = backoff.get(&addr).copied().unwrap_or(initial_backoff);
                    now.duration_since(*last) >= current_backoff
                } else {
                    true // Never attempted, should try
                }
            };

            if !should_attempt {
                continue;
            }

            tracing::info!("[CPP][BOOTNODE] Attempting reconnection to {}", addr);

            // Record this attempt
            {
                let mut last_attempt = self.last_bootnode_attempt.write().await;
                last_attempt.insert(addr, now);
            }

            // Try to connect
            match self.connect_bootnode(addr).await {
                Ok(()) => {
                    tracing::info!("[CPP][BOOTNODE] Successfully reconnected to {}", addr);
                    // Reset backoff on success
                    {
                        let mut backoff = self.bootnode_backoff.write().await;
                        backoff.insert(addr, initial_backoff);
                    }
                    break; // One successful connection is enough to start
                }
                Err(e) => {
                    tracing::warn!("[CPP][BOOTNODE] Failed to reconnect to {}: {}", addr, e);
                    // Increase backoff exponentially (double it, up to max)
                    {
                        let mut backoff = self.bootnode_backoff.write().await;
                        let current = backoff.get(&addr).copied().unwrap_or(initial_backoff);
                        let new_backoff = (current * 2).min(max_backoff);
                        backoff.insert(addr, new_backoff);
                        tracing::info!(
                            "[CPP][BOOTNODE] Next retry for {} in {:?}",
                            addr,
                            new_backoff
                        );
                    }
                }
            }
        }
    }
    /// Update node metrics from peer observations
    async fn update_metrics(&self) {
        let peers = self.peers.read().await;
        let peer_count = peers.len();

        let (avg_quality, avg_rtt) = if peer_count > 0 {
            let quality_sum: f64 = peers.values().map(|p| p.quality).sum();
            let rtt_sum: Duration = peers.values().map(|p| p.average_rtt()).sum();
            (quality_sum / peer_count as f64, rtt_sum / peer_count as u32)
        } else {
            (0.0, Duration::ZERO)
        };

        let chain_state = self.chain_state.read().await;
        tracing::info!(
            peer_count,
            avg_quality = format!("{:.3}", avg_quality),
            avg_rtt_ms = avg_rtt.as_millis(),
            best_height = chain_state.best_height,
            "Network metrics update"
        );
        drop(chain_state);

        // Update local NodeMetrics
        let mut metrics = self.metrics.write().await;
        metrics.uptime_ratio = 1.0; // TODO: Track from start time
        metrics.avg_response_time = avg_rtt.as_secs_f64();
    }
}

// =============================================================================
// Error Types
// =============================================================================

#[derive(Debug)]
pub enum NetworkError {
    Io(std::io::Error),
    Protocol(ProtocolError),
    InvalidHandshake(String),
    PeerNotFound(PeerId),
    Timeout,
    Shutdown,
}

impl From<std::io::Error> for NetworkError {
    fn from(err: std::io::Error) -> Self {
        NetworkError::Io(err)
    }
}

impl From<ProtocolError> for NetworkError {
    fn from(err: ProtocolError) -> Self {
        NetworkError::Protocol(err)
    }
}

impl std::fmt::Display for NetworkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NetworkError::Io(e) => write!(f, "IO error: {}", e),
            NetworkError::Protocol(e) => write!(f, "Protocol error: {}", e),
            NetworkError::InvalidHandshake(msg) => write!(f, "Invalid handshake: {}", msg),
            NetworkError::PeerNotFound(id) => write!(f, "Peer not found: {:?}", id),
            NetworkError::Timeout => write!(f, "Operation timed out"),
            NetworkError::Shutdown => write!(f, "Network shutting down"),
        }
    }
}

impl std::error::Error for NetworkError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_creation() {
        let config = CppConfig::default();
        let peer_id = [1u8; 32];
        let genesis = Hash::ZERO;

        let (network, _cmd_tx, _event_rx) = CppNetwork::new(config, peer_id, genesis);

        assert_eq!(network.local_peer_id, peer_id);
    }
}
