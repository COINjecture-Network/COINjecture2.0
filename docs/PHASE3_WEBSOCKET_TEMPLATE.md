# Phase 3: WebSocket RPC Implementation Template

**File**: `rpc/src/websocket.rs`  
**Target Lines**: ~600  
**Purpose**: Light client support, browser-based mining

---

## Complete WebSocket RPC Structure

```rust
// =============================================================================
// COINjecture WebSocket RPC - Light Client Support
// =============================================================================

use coinject_core::{Block, Transaction, Hash, Address, BlockHeader};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, RwLock, broadcast};
use tokio_tungstenite::{accept_async, WebSocketStream, tungstenite::Message};
use futures::{StreamExt, SinkExt};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::time::{interval, Duration, Instant};

// =============================================================================
// RPC Message Types
// =============================================================================

/// RPC messages (JSON-encoded over WebSocket)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RpcMessage {
    // === Client → Server ===
    
    /// Authenticate light client
    Auth {
        client_id: String,
        signature: Vec<u8>,
    },
    
    /// Request mining work
    GetWork,
    
    /// Submit proof-of-work
    SubmitWork {
        work_id: u64,
        solution: Vec<u8>,
        nonce: u64,
    },
    
    /// Query chain status
    GetStatus,
    
    /// Query balance
    GetBalance {
        address: String,
    },
    
    /// Query block
    GetBlock {
        height: u64,
    },
    
    /// Query transaction
    GetTransaction {
        tx_hash: String,
    },
    
    /// Submit transaction
    SubmitTransaction {
        transaction: String, // JSON-encoded
    },
    
    // === Server → Client ===
    
    /// Authentication response
    AuthResponse {
        success: bool,
        message: String,
    },
    
    /// Mining work response
    WorkResponse {
        work_id: u64,
        problem: String, // JSON-encoded problem
        difficulty: f64,
        reward: u128,
        expires_at: i64, // Unix timestamp
    },
    
    /// Work submission response
    SubmitResponse {
        accepted: bool,
        message: String,
        reward: Option<u128>,
    },
    
    /// Status response
    StatusResponse {
        best_height: u64,
        best_hash: String,
        peer_count: usize,
        sync_progress: f64,
    },
    
    /// Balance response
    BalanceResponse {
        address: String,
        balance: u128,
        pending: u128,
    },
    
    /// Block response
    BlockResponse {
        block: String, // JSON-encoded block
    },
    
    /// Transaction response
    TransactionResponse {
        transaction: String, // JSON-encoded transaction
        confirmations: u64,
    },
    
    /// Error response
    Error {
        code: i32,
        message: String,
    },
    
    /// New block notification (push)
    NewBlock {
        height: u64,
        hash: String,
    },
    
    /// Reward notification (push)
    RewardNotification {
        amount: u128,
        block_height: u64,
    },
}

// =============================================================================
// RPC Events & Commands
// =============================================================================

/// Events sent from RPC to node service
#[derive(Debug, Clone)]
pub enum RpcEvent {
    /// Light client connected
    ClientConnected {
        client_id: ClientId,
        addr: SocketAddr,
    },
    
    /// Light client disconnected
    ClientDisconnected {
        client_id: ClientId,
    },
    
    /// PoW submission received
    WorkSubmitted {
        client_id: ClientId,
        work_id: u64,
        solution: Vec<u8>,
        nonce: u64,
    },
    
    /// Transaction submitted
    TransactionSubmitted {
        transaction: Transaction,
        client_id: ClientId,
    },
}

/// Commands sent from node service to RPC
#[derive(Debug, Clone)]
pub enum RpcCommand {
    /// Distribute mining work to clients
    DistributeWork {
        work: MiningWork,
    },
    
    /// Notify client of reward
    NotifyReward {
        client_id: ClientId,
        amount: u128,
        block_height: u64,
    },
    
    /// Broadcast new block to all clients
    BroadcastBlock {
        height: u64,
        hash: Hash,
    },
    
    /// Disconnect client
    DisconnectClient {
        client_id: ClientId,
        reason: String,
    },
}

// =============================================================================
// Types
// =============================================================================

pub type ClientId = String;

/// Light client connection
#[derive(Debug)]
pub struct LightClient {
    pub id: ClientId,
    pub addr: SocketAddr,
    pub connected_at: Instant,
    pub last_seen: Instant,
    pub authenticated: bool,
    pub address: Option<Address>,
    
    // Mining stats
    pub work_requests: u64,
    pub submissions: u64,
    pub accepted: u64,
    pub rejected: u64,
    pub total_reward: u128,
    
    // WebSocket sender
    pub tx: mpsc::UnboundedSender<Message>,
}

/// Mining work for light clients
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiningWork {
    pub work_id: u64,
    pub problem: coinject_core::problem::ProblemType,
    pub difficulty: f64,
    pub reward: u128,
    pub expires_at: i64,
}

/// Work queue for distributing mining work
#[derive(Debug)]
pub struct WorkQueue {
    /// Available work
    work: Vec<MiningWork>,
    
    /// Assigned work (client_id → work_id)
    assigned: HashMap<ClientId, u64>,
    
    /// Work ID counter
    next_work_id: u64,
}

impl WorkQueue {
    pub fn new() -> Self {
        WorkQueue {
            work: Vec::new(),
            assigned: HashMap::new(),
            next_work_id: 1,
        }
    }
    
    /// Add work to queue
    pub fn add_work(&mut self, mut work: MiningWork) {
        work.work_id = self.next_work_id;
        self.next_work_id += 1;
        self.work.push(work);
    }
    
    /// Get work for client
    pub fn get_work(&mut self, client_id: &ClientId) -> Option<MiningWork> {
        // Remove expired work
        let now = chrono::Utc::now().timestamp();
        self.work.retain(|w| w.expires_at > now);
        
        // Get next available work
        if let Some(work) = self.work.pop() {
            self.assigned.insert(client_id.clone(), work.work_id);
            Some(work)
        } else {
            None
        }
    }
    
    /// Verify work assignment
    pub fn verify_assignment(&self, client_id: &ClientId, work_id: u64) -> bool {
        self.assigned.get(client_id) == Some(&work_id)
    }
}

/// RPC metrics
#[derive(Debug, Default)]
pub struct RpcMetrics {
    pub total_connections: u64,
    pub active_connections: usize,
    pub total_work_requests: u64,
    pub total_submissions: u64,
    pub total_accepted: u64,
    pub total_rejected: u64,
    pub total_rewards_distributed: u128,
}

// =============================================================================
// WebSocket RPC Service
// =============================================================================

pub struct WebSocketRpc {
    /// Listen address
    listen_addr: SocketAddr,
    
    /// Connected clients
    clients: Arc<RwLock<HashMap<ClientId, LightClient>>>,
    
    /// Work queue
    work_queue: Arc<RwLock<WorkQueue>>,
    
    /// Metrics
    metrics: Arc<RwLock<RpcMetrics>>,
    
    /// Event sender (to node service)
    event_tx: mpsc::UnboundedSender<RpcEvent>,
    
    /// Command receiver (from node service)
    command_rx: mpsc::UnboundedReceiver<RpcCommand>,
    
    /// Shutdown signal
    shutdown_tx: broadcast::Sender<()>,
    shutdown_rx: broadcast::Receiver<()>,
}

impl WebSocketRpc {
    /// Create new WebSocket RPC service
    pub fn new(
        listen_addr: SocketAddr,
    ) -> (
        Self,
        mpsc::UnboundedSender<RpcCommand>,
        mpsc::UnboundedReceiver<RpcEvent>,
    ) {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
        
        let rpc = WebSocketRpc {
            listen_addr,
            clients: Arc::new(RwLock::new(HashMap::new())),
            work_queue: Arc::new(RwLock::new(WorkQueue::new())),
            metrics: Arc::new(RwLock::new(RpcMetrics::default())),
            event_tx,
            command_rx,
            shutdown_tx,
            shutdown_rx,
        };
        
        (rpc, command_tx, event_rx)
    }
    
    // =========================================================================
    // Main Event Loop
    // =========================================================================
    
    /// Start the WebSocket RPC service
    pub async fn start(mut self) -> Result<(), RpcError> {
        // Bind TCP listener
        let listener = TcpListener::bind(&self.listen_addr).await?;
        println!("WebSocket RPC listening on {}", self.listen_addr);
        
        // Periodic cleanup interval
        let mut cleanup_interval = interval(Duration::from_secs(60));
        
        loop {
            tokio::select! {
                // Accept incoming WebSocket connections
                Ok((stream, addr)) = listener.accept() => {
                    let clients = self.clients.clone();
                    let work_queue = self.work_queue.clone();
                    let metrics = self.metrics.clone();
                    let event_tx = self.event_tx.clone();
                    let shutdown = self.shutdown_tx.subscribe();
                    
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_client_connection(
                            stream,
                            addr,
                            clients,
                            work_queue,
                            metrics,
                            event_tx,
                            shutdown,
                        ).await {
                            eprintln!("Client connection error from {}: {}", addr, e);
                        }
                    });
                }
                
                // Handle commands from node service
                Some(command) = self.command_rx.recv() => {
                    if let Err(e) = self.handle_command(command).await {
                        eprintln!("RPC command error: {}", e);
                    }
                }
                
                // Periodic: Cleanup stale clients
                _ = cleanup_interval.tick() => {
                    self.cleanup_stale_clients().await;
                }
                
                // Shutdown signal
                _ = self.shutdown_rx.recv() => {
                    println!("WebSocket RPC shutting down");
                    break;
                }
            }
        }
        
        Ok(())
    }
    
    // =========================================================================
    // Client Connection Handling
    // =========================================================================
    
    /// Handle client WebSocket connection
    async fn handle_client_connection(
        stream: tokio::net::TcpStream,
        addr: SocketAddr,
        clients: Arc<RwLock<HashMap<ClientId, LightClient>>>,
        work_queue: Arc<RwLock<WorkQueue>>,
        metrics: Arc<RwLock<RpcMetrics>>,
        event_tx: mpsc::UnboundedSender<RpcEvent>,
        mut shutdown: broadcast::Receiver<()>,
    ) -> Result<(), RpcError> {
        // Accept WebSocket handshake
        let ws_stream = accept_async(stream).await?;
        println!("WebSocket client connected: {}", addr);
        
        // TODO: Implement client message loop
        // 1. Wait for Auth message
        // 2. Validate authentication
        // 3. Create LightClient
        // 4. Add to clients map
        // 5. Send ClientConnected event
        // 6. Handle messages in loop
        
        Ok(())
    }
    
    // =========================================================================
    // Message Handling
    // =========================================================================
    
    /// Handle RPC message from client
    async fn handle_message(
        client_id: &ClientId,
        msg: RpcMessage,
        clients: &Arc<RwLock<HashMap<ClientId, LightClient>>>,
        work_queue: &Arc<RwLock<WorkQueue>>,
        event_tx: &mpsc::UnboundedSender<RpcEvent>,
    ) -> Result<Option<RpcMessage>, RpcError> {
        match msg {
            RpcMessage::Auth { client_id: id, signature } => {
                // TODO: Validate signature
                Ok(Some(RpcMessage::AuthResponse {
                    success: true,
                    message: "Authenticated".to_string(),
                }))
            }
            
            RpcMessage::GetWork => {
                // Get work from queue
                let mut queue = work_queue.write().await;
                if let Some(work) = queue.get_work(client_id) {
                    Ok(Some(RpcMessage::WorkResponse {
                        work_id: work.work_id,
                        problem: serde_json::to_string(&work.problem).unwrap(),
                        difficulty: work.difficulty,
                        reward: work.reward,
                        expires_at: work.expires_at,
                    }))
                } else {
                    Ok(Some(RpcMessage::Error {
                        code: 404,
                        message: "No work available".to_string(),
                    }))
                }
            }
            
            RpcMessage::SubmitWork { work_id, solution, nonce } => {
                // Verify assignment
                let queue = work_queue.read().await;
                if !queue.verify_assignment(client_id, work_id) {
                    return Ok(Some(RpcMessage::SubmitResponse {
                        accepted: false,
                        message: "Invalid work ID".to_string(),
                        reward: None,
                    }));
                }
                
                // Send to node service for validation
                let _ = event_tx.send(RpcEvent::WorkSubmitted {
                    client_id: client_id.clone(),
                    work_id,
                    solution,
                    nonce,
                });
                
                // Response will come via RpcCommand::NotifyReward
                Ok(None)
            }
            
            RpcMessage::GetStatus => {
                // TODO: Query chain state
                Ok(Some(RpcMessage::StatusResponse {
                    best_height: 0,
                    best_hash: "".to_string(),
                    peer_count: 0,
                    sync_progress: 1.0,
                }))
            }
            
            RpcMessage::GetBalance { address } => {
                // TODO: Query balance
                Ok(Some(RpcMessage::BalanceResponse {
                    address,
                    balance: 0,
                    pending: 0,
                }))
            }
            
            RpcMessage::GetBlock { height } => {
                // TODO: Query block
                Ok(Some(RpcMessage::Error {
                    code: 404,
                    message: "Block not found".to_string(),
                }))
            }
            
            RpcMessage::GetTransaction { tx_hash } => {
                // TODO: Query transaction
                Ok(Some(RpcMessage::Error {
                    code: 404,
                    message: "Transaction not found".to_string(),
                }))
            }
            
            RpcMessage::SubmitTransaction { transaction } => {
                // TODO: Parse and submit transaction
                Ok(Some(RpcMessage::Error {
                    code: 400,
                    message: "Invalid transaction".to_string(),
                }))
            }
            
            _ => {
                Ok(Some(RpcMessage::Error {
                    code: 400,
                    message: "Invalid message type".to_string(),
                }))
            }
        }
    }
    
    // =========================================================================
    // Command Handling
    // =========================================================================
    
    /// Handle command from node service
    async fn handle_command(&mut self, command: RpcCommand) -> Result<(), RpcError> {
        match command {
            RpcCommand::DistributeWork { work } => {
                let mut queue = self.work_queue.write().await;
                queue.add_work(work);
            }
            
            RpcCommand::NotifyReward { client_id, amount, block_height } => {
                self.notify_reward(&client_id, amount, block_height).await?;
            }
            
            RpcCommand::BroadcastBlock { height, hash } => {
                self.broadcast_block(height, hash).await?;
            }
            
            RpcCommand::DisconnectClient { client_id, reason } => {
                self.disconnect_client(&client_id, &reason).await;
            }
        }
        
        Ok(())
    }
    
    // =========================================================================
    // Broadcasting
    // =========================================================================
    
    /// Notify client of reward
    async fn notify_reward(
        &self,
        client_id: &ClientId,
        amount: u128,
        block_height: u64,
    ) -> Result<(), RpcError> {
        let clients = self.clients.read().await;
        if let Some(client) = clients.get(client_id) {
            let msg = RpcMessage::RewardNotification {
                amount,
                block_height,
            };
            let json = serde_json::to_string(&msg)?;
            let _ = client.tx.send(Message::Text(json));
        }
        
        Ok(())
    }
    
    /// Broadcast new block to all clients
    async fn broadcast_block(&self, height: u64, hash: Hash) -> Result<(), RpcError> {
        let msg = RpcMessage::NewBlock {
            height,
            hash: hex::encode(hash.as_bytes()),
        };
        let json = serde_json::to_string(&msg)?;
        
        let clients = self.clients.read().await;
        for client in clients.values() {
            let _ = client.tx.send(Message::Text(json.clone()));
        }
        
        Ok(())
    }
    
    // =========================================================================
    // Maintenance
    // =========================================================================
    
    /// Disconnect client
    async fn disconnect_client(&self, client_id: &ClientId, reason: &str) {
        let mut clients = self.clients.write().await;
        if let Some(_client) = clients.remove(client_id) {
            println!("Client {} disconnected: {}", client_id, reason);
            
            let _ = self.event_tx.send(RpcEvent::ClientDisconnected {
                client_id: client_id.clone(),
            });
        }
    }
    
    /// Cleanup stale clients
    async fn cleanup_stale_clients(&self) {
        let now = Instant::now();
        let timeout = Duration::from_secs(300); // 5 minutes
        
        let mut clients = self.clients.write().await;
        clients.retain(|id, client| {
            if now.duration_since(client.last_seen) > timeout {
                println!("Removing stale client: {}", id);
                false
            } else {
                true
            }
        });
    }
}

// =============================================================================
// Error Types
// =============================================================================

#[derive(Debug)]
pub enum RpcError {
    Io(std::io::Error),
    WebSocket(tokio_tungstenite::tungstenite::Error),
    Json(serde_json::Error),
    InvalidMessage(String),
    ClientNotFound(ClientId),
}

impl From<std::io::Error> for RpcError {
    fn from(err: std::io::Error) -> Self {
        RpcError::Io(err)
    }
}

impl From<tokio_tungstenite::tungstenite::Error> for RpcError {
    fn from(err: tokio_tungstenite::tungstenite::Error) -> Self {
        RpcError::WebSocket(err)
    }
}

impl From<serde_json::Error> for RpcError {
    fn from(err: serde_json::Error) -> Self {
        RpcError::Json(err)
    }
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RpcError::Io(e) => write!(f, "IO error: {}", e),
            RpcError::WebSocket(e) => write!(f, "WebSocket error: {}", e),
            RpcError::Json(e) => write!(f, "JSON error: {}", e),
            RpcError::InvalidMessage(msg) => write!(f, "Invalid message: {}", msg),
            RpcError::ClientNotFound(id) => write!(f, "Client not found: {}", id),
        }
    }
}

impl std::error::Error for RpcError {}
```

---

## Implementation Checklist

### **Connection Management** ✅

- [ ] `handle_client_connection()` - Accept WebSocket connections
- [ ] Authenticate clients
- [ ] Create `LightClient` instances
- [ ] Add clients to client map
- [ ] Send `ClientConnected` event

### **Message Handling** ✅

- [ ] `handle_message()` - Dispatch RPC messages
- [ ] Handle `Auth`
- [ ] Handle `GetWork`
- [ ] Handle `SubmitWork`
- [ ] Handle `GetStatus`
- [ ] Handle `GetBalance`
- [ ] Handle `GetBlock`
- [ ] Handle `GetTransaction`
- [ ] Handle `SubmitTransaction`

### **Command Handling** ✅

- [ ] `handle_command()` - Process commands from node service
- [ ] Handle `DistributeWork`
- [ ] Handle `NotifyReward`
- [ ] Handle `BroadcastBlock`
- [ ] Handle `DisconnectClient`

### **Broadcasting** ✅

- [ ] `notify_reward()` - Send reward notification to client
- [ ] `broadcast_block()` - Broadcast new block to all clients

### **Maintenance** ✅

- [ ] `disconnect_client()` - Remove client from client map
- [ ] `cleanup_stale_clients()` - Remove timed-out clients

---

## Browser Client Example

```javascript
// Browser-based light client
const ws = new WebSocket('ws://bootnode.coinject.io:8080');

ws.onopen = () => {
    // Authenticate
    ws.send(JSON.stringify({
        type: 'auth',
        client_id: 'browser_miner_1',
        signature: []
    }));
};

ws.onmessage = (event) => {
    const msg = JSON.parse(event.data);
    
    if (msg.type === 'work_response') {
        // Solve problem
        const solution = solveProblem(msg.problem);
        
        // Submit solution
        ws.send(JSON.stringify({
            type: 'submit_work',
            work_id: msg.work_id,
            solution: solution,
            nonce: 12345
        }));
    }
    
    if (msg.type === 'reward_notification') {
        console.log(`Earned ${msg.amount} coins at block ${msg.block_height}!`);
    }
};

// Request mining work
function requestWork() {
    ws.send(JSON.stringify({ type: 'get_work' }));
}
```

---

## Next: Testing Strategy

See `PHASE3_TESTING.md` for comprehensive testing plan.
