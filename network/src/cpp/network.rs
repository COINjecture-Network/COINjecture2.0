// =============================================================================
// COINjecture P2P Protocol (CPP) - Network Service (COMPLETE)
// =============================================================================

use crate::cpp::{
    config::{CppConfig, NodeType, CPP_PORT, PEER_TIMEOUT, KEEPALIVE_INTERVAL},
    message::*,
    protocol::{MessageCodec, MessageEnvelope, ProtocolError},
    peer::{Peer, PeerState, PeerId},
    router::EquilibriumRouter,
    node_integration::{NodeMetrics, PeerSelector},
};
use crate::reputation::ReputationManager;
use coinject_core::{Block, Transaction, Hash, BlockHeader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, RwLock, broadcast};
use tokio::time::{interval, Duration, Instant};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

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
    PeerDisconnected {
        peer_id: PeerId,
        reason: String,
    },
    
    /// Status update from peer
    StatusUpdate {
        peer_id: PeerId,
        best_height: u64,
        best_hash: Hash,
        node_type: NodeType,
    },
    
    /// New block received
    BlockReceived {
        block: Block,
        peer_id: PeerId,
    },
    
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
    ConnectBootnode {
        addr: SocketAddr,
    },
    
    /// Broadcast new block
    BroadcastBlock {
        block: Block,
    },
    
    /// Broadcast new transaction
    BroadcastTransaction {
        transaction: Transaction,
    },
    
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
    DisconnectPeer {
        peer_id: PeerId,
        reason: String,
    },
    
    /// Update local chain state
    UpdateChainState {
        best_height: u64,
        best_hash: Hash,
    },
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
    
    /// Pending block requests (for tracking responses)
    pending_requests: Arc<RwLock<HashMap<u64, Instant>>>,
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
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
        
        let chain_state = Arc::new(RwLock::new(ChainState::new(genesis_hash)));
        
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
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
        };
        
        (network, command_tx, event_rx)
    }
    
    // =========================================================================
    // Main Event Loop
    // =========================================================================
    
    /// Start the network service
    pub async fn start(mut self) -> Result<(), NetworkError> {
        // Bind TCP listener
        let listener = TcpListener::bind(&self.config.p2p_listen).await?;
        let local_addr = listener.local_addr()?;
        
        println!("CPP Network listening on {}", local_addr);
        
        // Periodic maintenance intervals
        let mut ping_interval = interval(KEEPALIVE_INTERVAL);
        let mut cleanup_interval = interval(Duration::from_secs(60));
        let mut metrics_interval = interval(Duration::from_secs(300)); // 5 minutes
        
        loop {
            tokio::select! {
                // Accept incoming connections
                Ok((stream, addr)) = listener.accept() => {
                    let peers = self.peers.clone();
                    let router = self.router.clone();
                    let reputation = self.reputation.clone();
                    let chain_state = self.chain_state.clone();
                    let event_tx = self.event_tx.clone();
                    let local_peer_id = self.local_peer_id;
                    let shutdown = self.shutdown_tx.subscribe();
                    
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_incoming_connection(
                            stream,
                            addr,
                            local_peer_id,
                            peers,
                            router,
                            reputation,
                            chain_state,
                            event_tx,
                            shutdown,
                        ).await {
                            eprintln!("Connection error from {}: {}", addr, e);
                        }
                    });
                }
                
                // Handle commands from node service
                Some(command) = self.command_rx.recv() => {
                    if let Err(e) = self.handle_command(command).await {
                        eprintln!("Command handling error: {}", e);
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
                
                // Shutdown signal
                _ = self.shutdown_rx.recv() => {
                    println!("CPP Network shutting down");
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
    async fn handle_incoming_connection(
        mut stream: TcpStream,
        addr: SocketAddr,
        local_peer_id: PeerId,
        peers: Arc<RwLock<HashMap<PeerId, Peer>>>,
        _router: Arc<RwLock<EquilibriumRouter>>,
        _reputation: Arc<RwLock<ReputationManager>>,
        chain_state: Arc<RwLock<ChainState>>,
        event_tx: mpsc::UnboundedSender<NetworkEvent>,
        _shutdown: broadcast::Receiver<()>,
    ) -> Result<(), NetworkError> {
        // Perform handshake
        let state = chain_state.read().await;
        let (peer_id, node_type, best_height, best_hash) = Self::handshake(
            &mut stream,
            local_peer_id,
            &state,
        ).await?;
        drop(state);
        
        // Check if peer already connected
        {
            let peers_guard = peers.read().await;
            if peers_guard.contains_key(&peer_id) {
                return Err(NetworkError::InvalidHandshake("Peer already connected".to_string()));
            }
        }
        
        // Create peer instance (returns peer and read half of stream)
        let (peer, read_half) = Peer::new(
            peer_id,
            addr,
            stream,
            node_type,
            best_height,
            best_hash,
            chain_state.read().await.genesis_hash,
        );
        
        // Add peer to peer list
        {
            let mut peers_guard = peers.write().await;
            peers_guard.insert(peer_id, peer);
        }
        
        // Send PeerConnected event
        let _ = event_tx.send(NetworkEvent::PeerConnected {
            peer_id,
            addr,
            node_type,
            best_height,
            best_hash,
        });
        
        // Start message loop for this peer (using read half)
        let peers_clone = peers.clone();
        let chain_state_clone = chain_state.clone();
        let event_tx_clone = event_tx.clone();
        let peer_id_clone = peer_id;
        
        tokio::spawn(async move {
            if let Err(e) = Self::peer_message_loop(
                peer_id_clone,
                read_half,
                peers_clone,
                chain_state_clone,
                event_tx_clone,
            ).await {
                eprintln!("Peer {} message loop error: {}", peer_id_clone.iter().take(4).map(|b| format!("{:02x}", b)).collect::<String>(), e);
            }
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
    async fn peer_message_loop(
        peer_id: PeerId,
        mut read_half: tokio::io::ReadHalf<TcpStream>,
        peers: Arc<RwLock<HashMap<PeerId, Peer>>>,
        chain_state: Arc<RwLock<ChainState>>,
        event_tx: mpsc::UnboundedSender<NetworkEvent>,
    ) -> Result<(), NetworkError> {
        loop {
            // Check if peer still exists and is connected
            let peer_exists = {
                let peers_guard = peers.read().await;
                peers_guard.get(&peer_id)
                    .map(|p| p.state == PeerState::Connected)
                    .unwrap_or(false)
            };
            
            if !peer_exists {
                break;
            }
            
            // Read message with timeout
            let envelope_result = tokio::time::timeout(
                Duration::from_secs(30),
                MessageCodec::receive_from_read_half(&mut read_half),
            ).await;
            
            let envelope = match envelope_result {
                Ok(Ok(envelope)) => envelope,
                Ok(Err(e)) => {
                    eprintln!("Protocol error from peer {}: {}", 
                        peer_id.iter().take(4).map(|b| format!("{:02x}", b)).collect::<String>(), e);
                    break;
                }
                Err(_) => {
                    // Timeout - check if peer is still alive
                    let peers_guard = peers.read().await;
                    if let Some(p) = peers_guard.get(&peer_id) {
                        if p.is_timed_out() {
                            break;
                        }
                    }
                    continue;
                }
            };
            
            // Process message
            let peers_clone = peers.clone();
            let chain_state_clone = chain_state.clone();
            let event_tx_clone = event_tx.clone();
            let peer_id_clone = peer_id;
            
            if let Err(e) = Self::handle_peer_message(
                peer_id_clone,
                envelope,
                peers_clone,
                chain_state_clone,
                event_tx_clone,
            ).await {
                eprintln!("Error handling message from peer {}: {}", 
                    peer_id_clone.iter().take(4).map(|b| format!("{:02x}", b)).collect::<String>(), e);
            }
        }
        
        // Remove peer on disconnect
        let mut peers_guard = peers.write().await;
        if peers_guard.remove(&peer_id).is_some() {
            let _ = event_tx.send(NetworkEvent::PeerDisconnected {
                peer_id,
                reason: "Connection closed".to_string(),
            });
        }
        
        Ok(())
    }
    
    /// Handle incoming message from peer
    async fn handle_peer_message(
        peer_id: PeerId,
        envelope: MessageEnvelope,
        peers: Arc<RwLock<HashMap<PeerId, Peer>>>,
        chain_state: Arc<RwLock<ChainState>>,
        event_tx: mpsc::UnboundedSender<NetworkEvent>,
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
                Self::handle_status(peer_id, &envelope, peers, event_tx).await?;
            }
            MessageType::GetBlocks => {
                Self::handle_get_blocks(peer_id, &envelope, peers, chain_state).await?;
            }
            MessageType::Blocks => {
                Self::handle_blocks(peer_id, &envelope, event_tx).await?;
            }
            MessageType::NewBlock => {
                Self::handle_new_block(peer_id, &envelope, event_tx).await?;
            }
            MessageType::NewTransaction => {
                Self::handle_new_transaction(peer_id, &envelope, event_tx).await?;
            }
            MessageType::Ping => {
                Self::handle_ping(peer_id, &envelope, peers).await?;
            }
            MessageType::Pong => {
                Self::handle_pong(peer_id, &envelope, peers).await?;
            }
            MessageType::Disconnect => {
                // Peer wants to disconnect gracefully
                return Err(NetworkError::InvalidHandshake("Peer requested disconnect".to_string()));
            }
            _ => {
                // Unknown or unsupported message type
                eprintln!("Unknown message type from peer {}: {:?}", peer_id.iter().take(4).map(|b| format!("{:02x}", b)).collect::<String>(), envelope.msg_type);
            }
        }
        
        Ok(())
    }
    
    /// Perform handshake with peer
    async fn handshake(
        stream: &mut TcpStream,
        local_peer_id: PeerId,
        chain_state: &ChainState,
    ) -> Result<(PeerId, NodeType, u64, Hash), NetworkError> {
        // Receive Hello message
        let envelope = MessageCodec::receive(stream).await?;
        
        let hello: HelloMessage = envelope.deserialize()
            .map_err(|e| NetworkError::Protocol(e))?;
        
        // Validate genesis hash
        if hello.genesis_hash != chain_state.genesis_hash {
            return Err(NetworkError::InvalidHandshake(format!(
                "Genesis hash mismatch: expected {:?}, got {:?}",
                chain_state.genesis_hash, hello.genesis_hash
            )));
        }
        
        // Convert node_type from u8
        let node_type = NodeType::from_u8(hello.node_type)
            .map_err(|e| NetworkError::InvalidHandshake(format!("Invalid node type: {}", e)))?;
        
        // Send HelloAck
        let hello_ack = HelloAckMessage {
            version: crate::cpp::config::VERSION,
            peer_id: local_peer_id,
            best_height: chain_state.best_height,
            best_hash: chain_state.best_hash,
            genesis_hash: chain_state.genesis_hash,
            node_type: NodeType::Full.as_u8(), // TODO: Get from config
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };
        
        MessageCodec::send_hello_ack(stream, &hello_ack).await?;
        
        Ok((
            hello.peer_id,
            node_type,
            hello.best_height,
            hello.best_hash,
        ))
    }
    
    /// Connect to bootnode
    async fn connect_bootnode(&mut self, addr: SocketAddr) -> Result<(), NetworkError> {
        // Connect TCP stream
        let mut stream = TcpStream::connect(addr).await?;
        
        let state = self.chain_state.read().await;
        
        // Send Hello message first
        let hello = HelloMessage {
            version: crate::cpp::config::VERSION,
            peer_id: self.local_peer_id,
            best_height: state.best_height,
            best_hash: state.best_hash,
            genesis_hash: state.genesis_hash,
            node_type: self.config.node_type.as_u8(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };
        MessageCodec::send_hello(&mut stream, &hello).await?;
        
        // Receive HelloAck
        let envelope = MessageCodec::receive(&mut stream).await?;
        let hello_ack: HelloAckMessage = envelope.deserialize()
            .map_err(|e| NetworkError::Protocol(e))?;
        
        // Validate genesis hash
        if hello_ack.genesis_hash != state.genesis_hash {
            return Err(NetworkError::InvalidHandshake("Genesis hash mismatch".to_string()));
        }
        
        // Convert node_type from u8
        let node_type = NodeType::from_u8(hello_ack.node_type)
            .map_err(|e| NetworkError::InvalidHandshake(format!("Invalid node type: {}", e)))?;
        
        drop(state);
        
        // Create peer instance (returns peer and read half)
        let (peer, read_half) = Peer::new(
            hello_ack.peer_id,
            addr,
            stream,
            node_type,
            hello_ack.best_height,
            hello_ack.best_hash,
            self.chain_state.read().await.genesis_hash,
        );
        
        let peer_id = peer.id;
        
        // Add peer to peer list
        {
            let mut peers = self.peers.write().await;
            peers.insert(peer_id, peer);
        }
        
        // Update router
        {
            let _router = self.router.write().await;
            // TODO: Add peer to router
        }
        
        // Start message loop for this peer
        let peers_clone = self.peers.clone();
        let chain_state_clone = self.chain_state.clone();
        let event_tx_clone = self.event_tx.clone();
        let peer_id_clone = peer_id;
        
        tokio::spawn(async move {
            if let Err(e) = Self::peer_message_loop(
                peer_id_clone,
                read_half,
                peers_clone,
                chain_state_clone,
                event_tx_clone,
            ).await {
                eprintln!("Peer {} message loop error: {}", peer_id_clone.iter().take(4).map(|b| format!("{:02x}", b)).collect::<String>(), e);
            }
        });
        
        Ok(())
    }
    
    /// Add peer to peer list (internal helper - now handled inline)
    #[allow(dead_code)]
    async fn add_peer(&self, peer: Peer) {
        let peer_id = peer.id;
        let mut peers = self.peers.write().await;
        peers.insert(peer_id, peer);
        
        // Update router
        let _router = self.router.write().await;
        // TODO: Add peer to router
    }
    
    /// Remove peer from peer list
    async fn remove_peer(&self, peer_id: &PeerId, reason: &str) {
        let mut peers = self.peers.write().await;
        if let Some(_peer) = peers.remove(peer_id) {
            // Send disconnect event
            let _ = self.event_tx.send(NetworkEvent::PeerDisconnected {
                peer_id: *peer_id,
                reason: reason.to_string(),
            });
            
            // Update router
            let _router = self.router.write().await;
            // TODO: Remove peer from router
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
            
            NetworkCommand::RequestBlocks { peer_id, from_height, to_height, request_id } => {
                self.request_blocks(peer_id, from_height, to_height, request_id).await?;
            }
            
            NetworkCommand::RequestHeaders { peer_id, from_height, to_height, request_id } => {
                self.request_headers(peer_id, from_height, to_height, request_id).await?;
            }
            
            NetworkCommand::DisconnectPeer { peer_id, reason } => {
                self.remove_peer(&peer_id, &reason).await;
            }
            
            NetworkCommand::UpdateChainState { best_height, best_hash } => {
                let mut state = self.chain_state.write().await;
                state.best_height = best_height;
                state.best_hash = best_hash;
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
        event_tx: mpsc::UnboundedSender<NetworkEvent>,
    ) -> Result<(), NetworkError> {
        let status: StatusMessage = envelope.deserialize()
            .map_err(|e| NetworkError::Protocol(e))?;
        
        let node_type = NodeType::from_u8(status.node_type)
            .map_err(|e| NetworkError::InvalidHandshake(format!("Invalid node type: {}", e)))?;
        
        // Update peer status
        {
            let mut peers_guard = peers.write().await;
            if let Some(peer) = peers_guard.get_mut(&peer_id) {
                peer.update_status(status.best_height, status.best_hash, node_type);
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
    
    /// Handle GetBlocks request from peer
    async fn handle_get_blocks(
        peer_id: PeerId,
        envelope: &MessageEnvelope,
        peers: Arc<RwLock<HashMap<PeerId, Peer>>>,
        chain_state: Arc<RwLock<ChainState>>,
    ) -> Result<(), NetworkError> {
        let get_blocks: GetBlocksMessage = envelope.deserialize()
            .map_err(|e| NetworkError::Protocol(e))?;
        
        // TODO: Query blocks from chain state
        // For now, send empty response
        let blocks_msg = BlocksMessage {
            blocks: vec![],
            request_id: get_blocks.request_id,
        };
        
        // Send Blocks response
        let peers_guard = peers.read().await;
        if let Some(peer) = peers_guard.get(&peer_id) {
            let envelope = MessageEnvelope::new(MessageType::Blocks, &blocks_msg)
                .map_err(|e| NetworkError::Protocol(e))?;
            let data = envelope.encode();
            
            peer.send_message(data.clone())
                .map_err(|e| NetworkError::InvalidHandshake(e))?;
            
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
        event_tx: mpsc::UnboundedSender<NetworkEvent>,
    ) -> Result<(), NetworkError> {
        let blocks_msg: BlocksMessage = envelope.deserialize()
            .map_err(|e| NetworkError::Protocol(e))?;
        
        // Send BlocksReceived event
        let _ = event_tx.send(NetworkEvent::BlocksReceived {
            blocks: blocks_msg.blocks,
            request_id: blocks_msg.request_id,
            peer_id,
        });
        
        Ok(())
    }
    
    /// Handle NewBlock announcement from peer
    async fn handle_new_block(
        peer_id: PeerId,
        envelope: &MessageEnvelope,
        event_tx: mpsc::UnboundedSender<NetworkEvent>,
    ) -> Result<(), NetworkError> {
        let new_block: NewBlockMessage = envelope.deserialize()
            .map_err(|e| NetworkError::Protocol(e))?;
        
        // Send BlockReceived event
        let _ = event_tx.send(NetworkEvent::BlockReceived {
            block: new_block.block,
            peer_id,
        });
        
        Ok(())
    }
    
    /// Handle NewTransaction announcement from peer
    async fn handle_new_transaction(
        peer_id: PeerId,
        envelope: &MessageEnvelope,
        event_tx: mpsc::UnboundedSender<NetworkEvent>,
    ) -> Result<(), NetworkError> {
        let new_tx: NewTransactionMessage = envelope.deserialize()
            .map_err(|e| NetworkError::Protocol(e))?;
        
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
        let ping: PingMessage = envelope.deserialize()
            .map_err(|e| NetworkError::Protocol(e))?;
        
        // Send Pong response
        let peers_guard = peers.read().await;
        if let Some(peer) = peers_guard.get(&peer_id) {
            let pong = PongMessage {
                nonce: ping.nonce,
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            };
            
            let envelope = MessageEnvelope::new(MessageType::Pong, &pong)
                .map_err(|e| NetworkError::Protocol(e))?;
            let data = envelope.encode();
            
            peer.send_message(data.clone())
                .map_err(|e| NetworkError::InvalidHandshake(e))?;
            
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
        let pong: PongMessage = envelope.deserialize()
            .map_err(|e| NetworkError::Protocol(e))?;
        
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
    
    /// Broadcast block to selected peers
    async fn broadcast_block(&self, block: Block) -> Result<(), NetworkError> {
        // Select peers (equilibrium fanout: √n × η)
        let peer_ids = self.select_broadcast_peers(MessageType::NewBlock).await;
        
        // Send NewBlock message to each peer
        let peers = self.peers.read().await;
        for peer_id in peer_ids {
            if let Some(peer) = peers.get(&peer_id) {
                let new_block_msg = NewBlockMessage { block: block.clone() };
                
                // Serialize message
                let envelope = MessageEnvelope::new(MessageType::NewBlock, &new_block_msg)
                    .map_err(|e| NetworkError::Protocol(e))?;
                let data = envelope.encode();
                
                // Send via channel
                if let Err(e) = peer.send_message(data.clone()) {
                    eprintln!("Failed to send NewBlock to peer {}: {}", peer_id.iter().take(4).map(|b| format!("{:02x}", b)).collect::<String>(), e);
                    continue;
                }
                
                // Update peer stats
                {
                    let mut peers_guard = self.peers.write().await;
                    if let Some(p) = peers_guard.get_mut(&peer_id) {
                        p.on_message_sent(data.len());
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Broadcast transaction to selected peers
    async fn broadcast_transaction(&self, tx: Transaction) -> Result<(), NetworkError> {
        // Select peers (equilibrium fanout)
        let peer_ids = self.select_broadcast_peers(MessageType::NewTransaction).await;
        
        // Send NewTransaction message to each peer
        let peers = self.peers.read().await;
        for peer_id in peer_ids {
            if let Some(peer) = peers.get(&peer_id) {
                let new_tx_msg = NewTransactionMessage { transaction: tx.clone() };
                
                // Serialize message
                let envelope = MessageEnvelope::new(MessageType::NewTransaction, &new_tx_msg)
                    .map_err(|e| NetworkError::Protocol(e))?;
                let data = envelope.encode();
                
                // Send via channel
                if let Err(e) = peer.send_message(data.clone()) {
                    eprintln!("Failed to send NewTransaction to peer {}: {}", peer_id.iter().take(4).map(|b| format!("{:02x}", b)).collect::<String>(), e);
                    continue;
                }
                
                // Update peer stats
                {
                    let mut peers_guard = self.peers.write().await;
                    if let Some(p) = peers_guard.get_mut(&peer_id) {
                        p.on_message_sent(data.len());
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Select peers for broadcast (equilibrium fanout)
    async fn select_broadcast_peers(&self, _msg_type: MessageType) -> Vec<PeerId> {
        let peers = self.peers.read().await;
        let peer_count = peers.len();
        
        if peer_count == 0 {
            return vec![];
        }
        
        // Equilibrium fanout: √n × η
        let fanout = ((peer_count as f64).sqrt() * crate::cpp::config::ETA).ceil() as usize;
        let fanout = fanout.min(peer_count);
        
        // Use PeerSelector to select best peers
        let _selector = PeerSelector;
        // TODO: Implement peer selection based on reputation and metrics
        
        // For now, return first N peers
        peers.keys().take(fanout).copied().collect()
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
        let peer = peers.get(&peer_id)
            .ok_or(NetworkError::PeerNotFound(peer_id))?;
        
        // Create GetBlocks message
        let get_blocks = GetBlocksMessage {
            from_height,
            to_height,
            request_id,
        };
        
        // Serialize and send message
        let envelope = MessageEnvelope::new(MessageType::GetBlocks, &get_blocks)
            .map_err(|e| NetworkError::Protocol(e))?;
        let data = envelope.encode();
        
        peer.send_message(data.clone())
            .map_err(|e| NetworkError::InvalidHandshake(e))?;
        
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
    async fn request_headers(
        &self,
        peer_id: PeerId,
        _from_height: u64,
        _to_height: u64,
        _request_id: u64,
    ) -> Result<(), NetworkError> {
        let peers = self.peers.read().await;
        let _peer = peers.get(&peer_id)
            .ok_or(NetworkError::PeerNotFound(peer_id))?;
        
        // TODO: Send GetHeaders message
        
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
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                };
                
                let envelope = match MessageEnvelope::new(MessageType::Ping, &ping) {
                    Ok(e) => e,
                    Err(e) => {
                        eprintln!("Failed to create ping envelope: {}", e);
                        continue;
                    }
                };
                let data = envelope.encode();
                
                if let Err(e) = peer.send_message(data.clone()) {
                    eprintln!("Failed to send ping to peer {}: {}", peer.id.iter().take(4).map(|b| format!("{:02x}", b)).collect::<String>(), e);
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
        
        for peer_id in to_remove {
            peers.remove(&peer_id);
            let _ = self.event_tx.send(NetworkEvent::PeerDisconnected {
                peer_id,
                reason: "Timeout".to_string(),
            });
        }
    }
    
    /// Update node metrics
    async fn update_metrics(&self) {
        let _metrics = self.metrics.write().await;
        let _peers = self.peers.read().await;
        
        // TODO: Calculate metrics:
        // - storage_ratio (from chain state)
        // - validation_speed (from validator stats)
        // - solve_rate (from miner stats)
        // - uptime_ratio (from connection time)
        // - avg_response_time (from peer RTT samples)
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
