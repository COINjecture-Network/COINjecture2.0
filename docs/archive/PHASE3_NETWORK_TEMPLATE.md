# Phase 3: Network Service Implementation Template

**File**: `network/src/cpp/network.rs`  
**Target Lines**: ~800 (complete implementation)  
**Current Lines**: 200 (skeleton)  
**To Add**: ~600 lines

---

## Complete Network Service Structure

```rust
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
        router: Arc<RwLock<EquilibriumRouter>>,
        reputation: Arc<RwLock<ReputationManager>>,
        chain_state: Arc<RwLock<ChainState>>,
        event_tx: mpsc::UnboundedSender<NetworkEvent>,
        mut shutdown: broadcast::Receiver<()>,
    ) -> Result<(), NetworkError> {
        // TODO: Implement handshake
        // 1. Receive Hello message
        // 2. Validate genesis hash
        // 3. Send HelloAck
        // 4. Create Peer instance
        // 5. Add to peer list
        // 6. Send PeerConnected event
        // 7. Start message loop
        
        Ok(())
    }
    
    /// Perform handshake with peer
    async fn handshake(
        stream: &mut TcpStream,
        local_peer_id: PeerId,
        chain_state: &ChainState,
    ) -> Result<(PeerId, NodeType, u64, Hash), NetworkError> {
        // TODO: Implement handshake logic
        // 1. Receive Hello
        // 2. Validate version, genesis
        // 3. Send HelloAck
        // 4. Return peer info
        
        unimplemented!()
    }
    
    /// Connect to bootnode
    async fn connect_bootnode(&mut self, addr: SocketAddr) -> Result<(), NetworkError> {
        // TODO: Implement bootnode connection
        // 1. Connect TCP stream
        // 2. Perform handshake
        // 3. Add peer
        // 4. Start message loop
        
        Ok(())
    }
    
    /// Add peer to peer list
    async fn add_peer(&self, peer: Peer) {
        let peer_id = peer.id;
        let mut peers = self.peers.write().await;
        peers.insert(peer_id, peer);
        
        // Update router
        let mut router = self.router.write().await;
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
            let mut router = self.router.write().await;
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
    // Message Handling (see PHASE3_MESSAGE_HANDLERS.md for details)
    // =========================================================================
    
    // TODO: Implement message handlers
    // - handle_status()
    // - handle_get_blocks()
    // - handle_blocks()
    // - handle_new_block()
    // - handle_new_transaction()
    // - handle_ping()
    // - handle_pong()
    
    // =========================================================================
    // Broadcasting
    // =========================================================================
    
    /// Broadcast block to selected peers
    async fn broadcast_block(&self, block: Block) -> Result<(), NetworkError> {
        // TODO: Implement block broadcast
        // 1. Select peers (equilibrium fanout: √n × η)
        // 2. Send NewBlock message to each
        // 3. Update flow control
        
        Ok(())
    }
    
    /// Broadcast transaction to selected peers
    async fn broadcast_transaction(&self, tx: Transaction) -> Result<(), NetworkError> {
        // TODO: Implement transaction broadcast
        
        Ok(())
    }
    
    /// Select peers for broadcast (equilibrium fanout)
    async fn select_broadcast_peers(&self, msg_type: MessageType) -> Vec<PeerId> {
        // TODO: Implement peer selection
        // Use PeerSelector::select_for_propagation()
        
        vec![]
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
        // TODO: Implement block request
        // 1. Get peer from peer list
        // 2. Send GetBlocks message
        // 3. Track pending request
        
        Ok(())
    }
    
    /// Request headers from peer (light sync)
    async fn request_headers(
        &self,
        peer_id: PeerId,
        from_height: u64,
        to_height: u64,
        request_id: u64,
    ) -> Result<(), NetworkError> {
        // TODO: Implement header request
        
        Ok(())
    }
    
    // =========================================================================
    // Periodic Maintenance
    // =========================================================================
    
    /// Send keepalive pings to all peers
    async fn send_pings(&self) {
        // TODO: Implement ping loop
        // For each peer that needs ping:
        // 1. Send Ping message
        // 2. Update last_ping time
    }
    
    /// Remove timed-out peers
    async fn cleanup_stale_peers(&self) {
        // TODO: Implement cleanup
        // For each peer:
        // 1. Check if timed out (last_seen > PEER_TIMEOUT)
        // 2. Remove if timed out
    }
    
    /// Update node metrics
    async fn update_metrics(&self) {
        // TODO: Implement metrics update
        // Calculate:
        // - storage_ratio
        // - validation_speed
        // - solve_rate
        // - uptime_ratio
        // - bandwidth_ratio
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
```

---

## Implementation Checklist

### **Connection Management** ✅

- [ ] `handle_incoming_connection()` - Accept and process new connections
- [ ] `handshake()` - Perform Hello/HelloAck exchange
- [ ] `connect_bootnode()` - Connect to bootnode
- [ ] `add_peer()` - Add peer to peer list and router
- [ ] `remove_peer()` - Remove peer and send disconnect event

### **Command Handling** ✅

- [ ] `handle_command()` - Dispatch commands from node service
- [ ] Handle `ConnectBootnode`
- [ ] Handle `BroadcastBlock`
- [ ] Handle `BroadcastTransaction`
- [ ] Handle `RequestBlocks`
- [ ] Handle `RequestHeaders`
- [ ] Handle `DisconnectPeer`
- [ ] Handle `UpdateChainState`

### **Message Handling** (see separate document)

- [ ] `handle_status()` - Process status updates
- [ ] `handle_get_blocks()` - Serve block requests
- [ ] `handle_blocks()` - Process block responses
- [ ] `handle_new_block()` - Process new block announcements
- [ ] `handle_new_transaction()` - Process new transactions
- [ ] `handle_ping()` - Respond to pings
- [ ] `handle_pong()` - Process pong responses

### **Broadcasting** ✅

- [ ] `broadcast_block()` - Broadcast block to selected peers
- [ ] `broadcast_transaction()` - Broadcast transaction to selected peers
- [ ] `select_broadcast_peers()` - Select peers using equilibrium fanout

### **Sync Requests** ✅

- [ ] `request_blocks()` - Request blocks from peer
- [ ] `request_headers()` - Request headers from peer (light sync)

### **Periodic Maintenance** ✅

- [ ] `send_pings()` - Send keepalive pings
- [ ] `cleanup_stale_peers()` - Remove timed-out peers
- [ ] `update_metrics()` - Update node metrics

---

## Next: Message Handlers

See `PHASE3_MESSAGE_HANDLERS.md` for detailed message handling implementation.
