# CPP Protocol - Phase 3 Implementation Plan

**Date**: December 18, 2025  
**Status**: Planning  
**Branch**: `feature/cpp-protocol`  
**Goal**: Complete network service and WebSocket RPC integration

---

## Executive Summary

Phase 3 completes the COINjecture P2P Protocol (CPP) by implementing:

1. **Full Network Service** (`network.rs`) - Connection management, message handling, periodic maintenance
2. **WebSocket RPC** (`rpc/src/websocket.rs`) - Light client support, browser-based mining
3. **Node Service Integration** (`node/src/service.rs`) - Replace libp2p entirely
4. **Testing & Validation** - Two-node sync, light client mining, performance benchmarks

**Timeline**: 2-3 weeks  
**Complexity**: Medium-High  
**Risk**: Low (Phase 2 provides solid foundation)

---

## Phase 2 Recap

### **What's Complete** ✅

| Component | Lines | Tests | Status |
|-----------|-------|-------|--------|
| Protocol encoding | 350 | 4 | ✅ |
| Peer management | 450 | 2 | ✅ |
| Node integration | 400 | 4 | ✅ |
| Flow control | 340 | 6 | ✅ |
| Router | 330 | 5 | ✅ |
| Message types | 440 | 3 | ✅ |
| Config | 230 | 3 | ✅ |
| Integration tests | 668 | 16 | ✅ |
| **Total** | **3,808** | **44+** | ✅ |

### **What's Missing** 🚧

1. **Network Service** (`network.rs`) - Only skeleton exists (200 lines)
2. **WebSocket RPC** - Not yet implemented
3. **Node Service Integration** - Still using libp2p
4. **End-to-end testing** - Two-node sync not tested

---

## Phase 3 Architecture

### **System Overview**

```
┌─────────────────────────────────────────────────────────────┐
│                      Node Service                            │
│  (Chain state, mempool, validation, mining)                 │
└────────────┬────────────────────────────────┬───────────────┘
             │                                │
             │ NetworkEvent                   │ NetworkCommand
             │ NetworkCommand                 │ NetworkEvent
             ▼                                ▼
┌────────────────────────────────┐  ┌────────────────────────┐
│      CPP Network Service       │  │   WebSocket RPC        │
│  (Full node P2P on port 707)   │  │ (Light clients on 8080)│
├────────────────────────────────┤  ├────────────────────────┤
│ - Connection management        │  │ - Light client connect │
│ - Message handling             │  │ - Mining work distrib  │
│ - Periodic maintenance         │  │ - PoW submission       │
│ - Peer discovery               │  │ - Reward distribution  │
└────────────┬───────────────────┘  └────────┬───────────────┘
             │                                │
             │ TCP (port 707)                 │ WebSocket (8080)
             ▼                                ▼
┌────────────────────────────────┐  ┌────────────────────────┐
│      Full Nodes (Peers)        │  │   Light Clients        │
│  - Validators                  │  │ - Browser miners       │
│  - Full nodes                  │  │ - Mobile wallets       │
│  - Archive nodes               │  │ - Web apps             │
└────────────────────────────────┘  └────────────────────────┘
```

### **Key Components**

#### **1. CPP Network Service** (`network/src/cpp/network.rs`)

**Responsibilities**:
- Accept incoming TCP connections (port 707)
- Perform handshake (Hello/HelloAck)
- Dispatch messages to handlers
- Manage peer lifecycle
- Periodic maintenance (ping, cleanup, metrics)
- Broadcast blocks/transactions
- Serve sync requests

**Architecture**:
```rust
pub struct CppNetwork {
    // Core state
    config: CppConfig,
    local_peer_id: PeerId,
    genesis_hash: Hash,
    
    // Peer management
    peers: Arc<RwLock<HashMap<PeerId, Peer>>>,
    router: Arc<RwLock<EquilibriumRouter>>,
    reputation: Arc<RwLock<ReputationManager>>,
    
    // Chain state (from node service)
    chain_state: Arc<RwLock<ChainState>>,
    
    // Communication channels
    event_tx: mpsc::UnboundedSender<NetworkEvent>,
    command_rx: mpsc::UnboundedReceiver<NetworkCommand>,
    
    // Shutdown signal
    shutdown: broadcast::Receiver<()>,
}
```

#### **2. WebSocket RPC** (`rpc/src/websocket.rs`)

**Responsibilities**:
- Accept WebSocket connections (port 8080)
- Authenticate light clients
- Distribute mining work
- Accept PoW submissions
- Distribute rewards
- Provide chain state queries

**Architecture**:
```rust
pub struct WebSocketRpc {
    // Configuration
    config: RpcConfig,
    
    // Connected light clients
    clients: Arc<RwLock<HashMap<ClientId, LightClient>>>,
    
    // Mining work distribution
    work_queue: Arc<RwLock<WorkQueue>>,
    
    // Communication with node service
    event_tx: mpsc::UnboundedSender<RpcEvent>,
    command_rx: mpsc::UnboundedReceiver<RpcCommand>,
    
    // Metrics
    metrics: Arc<RwLock<RpcMetrics>>,
}
```

#### **3. Node Service Integration** (`node/src/service.rs`)

**Changes Required**:
- Replace `libp2p::NetworkService` with `CppNetwork`
- Replace `libp2p::NetworkEvent` with `NetworkEvent`
- Replace `libp2p::NetworkCommand` with `NetworkCommand`
- Add `WebSocketRpc` integration
- Update event loop to handle both P2P and RPC events

---

## Implementation Breakdown

### **Part 1: Network Service Core** (800 lines)

#### **1.1 Connection Management** (200 lines)

```rust
impl CppNetwork {
    /// Accept incoming connection
    async fn accept_connection(&mut self, stream: TcpStream, addr: SocketAddr);
    
    /// Perform handshake
    async fn handshake(&mut self, stream: &mut TcpStream) -> Result<PeerId, ProtocolError>;
    
    /// Add peer to peer list
    fn add_peer(&mut self, peer: Peer);
    
    /// Remove peer
    fn remove_peer(&mut self, peer_id: &PeerId, reason: &str);
    
    /// Connect to bootnode
    async fn connect_bootnode(&mut self, addr: SocketAddr) -> Result<(), NetworkError>;
}
```

**Key Features**:
- Concurrent connection handling (tokio::spawn)
- Handshake timeout (10 seconds)
- Genesis hash validation
- Peer limit enforcement (max 50 peers)
- Duplicate connection detection

#### **1.2 Message Handling** (300 lines)

```rust
impl CppNetwork {
    /// Main message dispatcher
    async fn handle_message(&mut self, peer_id: PeerId, envelope: MessageEnvelope);
    
    /// Handle Status update
    async fn handle_status(&mut self, peer_id: PeerId, msg: StatusMessage);
    
    /// Handle GetBlocks request
    async fn handle_get_blocks(&mut self, peer_id: PeerId, msg: GetBlocksMessage);
    
    /// Handle Blocks response
    async fn handle_blocks(&mut self, peer_id: PeerId, msg: BlocksMessage);
    
    /// Handle NewBlock announcement
    async fn handle_new_block(&mut self, peer_id: PeerId, msg: NewBlockMessage);
    
    /// Handle NewTransaction announcement
    async fn handle_new_transaction(&mut self, peer_id: PeerId, msg: NewTransactionMessage);
    
    /// Handle Ping
    async fn handle_ping(&mut self, peer_id: PeerId, msg: PingMessage);
    
    /// Handle Pong
    async fn handle_pong(&mut self, peer_id: PeerId, msg: PongMessage);
}
```

**Key Features**:
- Message priority handling (D1-D8)
- Flow control integration
- Reputation updates
- Error handling and recovery
- Duplicate detection (blocks, transactions)

#### **1.3 Periodic Maintenance** (150 lines)

```rust
impl CppNetwork {
    /// Send keepalive pings
    async fn send_pings(&mut self);
    
    /// Remove timed-out peers
    async fn cleanup_stale_peers(&mut self);
    
    /// Update node metrics
    async fn update_metrics(&mut self);
    
    /// Update router with peer info
    async fn update_router(&mut self);
    
    /// Broadcast status to peers
    async fn broadcast_status(&mut self);
}
```

**Schedule**:
- Ping: Every 30 seconds
- Cleanup: Every 60 seconds
- Metrics: Every 100 blocks
- Status: Every 10 blocks

#### **1.4 Broadcasting** (150 lines)

```rust
impl CppNetwork {
    /// Broadcast block to selected peers
    async fn broadcast_block(&self, block: Block) -> Result<(), NetworkError>;
    
    /// Broadcast transaction to selected peers
    async fn broadcast_transaction(&self, tx: Transaction) -> Result<(), NetworkError>;
    
    /// Select peers for broadcast (equilibrium fanout)
    fn select_broadcast_peers(&self, msg_type: MessageType) -> Vec<PeerId>;
}
```

**Key Features**:
- Equilibrium fanout: √n × η peers
- Node type prioritization
- Quality-based selection
- Duplicate prevention

### **Part 2: WebSocket RPC** (600 lines)

#### **2.1 Connection Management** (150 lines)

```rust
impl WebSocketRpc {
    /// Accept WebSocket connection
    async fn accept_connection(&mut self, ws: WebSocket, addr: SocketAddr);
    
    /// Authenticate light client
    async fn authenticate(&mut self, client_id: &ClientId, auth: AuthMessage) -> Result<(), RpcError>;
    
    /// Add client to client list
    fn add_client(&mut self, client: LightClient);
    
    /// Remove client
    fn remove_client(&mut self, client_id: &ClientId, reason: &str);
}
```

#### **2.2 Mining Work Distribution** (200 lines)

```rust
impl WebSocketRpc {
    /// Distribute mining work to light clients
    async fn distribute_work(&mut self, work: MiningWork);
    
    /// Handle PoW submission
    async fn handle_submission(&mut self, client_id: ClientId, submission: PoWSubmission);
    
    /// Validate PoW submission
    async fn validate_submission(&self, submission: &PoWSubmission) -> Result<bool, RpcError>;
    
    /// Distribute rewards
    async fn distribute_rewards(&mut self, block_height: u64, rewards: Vec<Reward>);
}
```

#### **2.3 Chain State Queries** (150 lines)

```rust
impl WebSocketRpc {
    /// Handle GetStatus query
    async fn handle_get_status(&self, client_id: ClientId) -> StatusResponse;
    
    /// Handle GetBalance query
    async fn handle_get_balance(&self, client_id: ClientId, address: Address) -> BalanceResponse;
    
    /// Handle GetBlock query
    async fn handle_get_block(&self, client_id: ClientId, height: u64) -> BlockResponse;
    
    /// Handle GetTransaction query
    async fn handle_get_transaction(&self, client_id: ClientId, tx_hash: Hash) -> TransactionResponse;
}
```

#### **2.4 Message Handling** (100 lines)

```rust
impl WebSocketRpc {
    /// Main message dispatcher
    async fn handle_message(&mut self, client_id: ClientId, msg: RpcMessage);
    
    /// Send message to client
    async fn send_message(&self, client_id: &ClientId, msg: RpcMessage) -> Result<(), RpcError>;
    
    /// Broadcast message to all clients
    async fn broadcast_message(&self, msg: RpcMessage);
}
```

### **Part 3: Node Service Integration** (400 lines)

#### **3.1 Replace libp2p** (200 lines)

**Changes in `node/src/service.rs`**:

```rust
// OLD (libp2p)
use libp2p::{NetworkService, NetworkEvent, NetworkCommand};

// NEW (CPP)
use coinject_network::cpp::{CppNetwork, NetworkEvent, NetworkCommand};

pub struct NodeService {
    // OLD
    // network: NetworkService,
    
    // NEW
    network_cmd_tx: mpsc::UnboundedSender<NetworkCommand>,
    network_event_rx: mpsc::UnboundedReceiver<NetworkEvent>,
    
    // NEW: WebSocket RPC
    rpc_cmd_tx: mpsc::UnboundedSender<RpcCommand>,
    rpc_event_rx: mpsc::UnboundedReceiver<RpcEvent>,
    
    // ... rest of fields
}
```

#### **3.2 Event Loop Integration** (200 lines)

```rust
impl NodeService {
    pub async fn start(mut self) -> Result<(), NodeError> {
        loop {
            tokio::select! {
                // Handle network events (from CPP)
                Some(event) = self.network_event_rx.recv() => {
                    self.handle_network_event(event).await?;
                }
                
                // Handle RPC events (from WebSocket)
                Some(event) = self.rpc_event_rx.recv() => {
                    self.handle_rpc_event(event).await?;
                }
                
                // Handle chain events (new block, etc.)
                Some(event) = self.chain_event_rx.recv() => {
                    self.handle_chain_event(event).await?;
                }
                
                // Periodic tasks
                _ = self.maintenance_interval.tick() => {
                    self.perform_maintenance().await?;
                }
                
                // Shutdown signal
                _ = self.shutdown.recv() => {
                    break;
                }
            }
        }
        
        Ok(())
    }
}
```

---

## File Structure

### **New Files** (1,800 lines total)

```
network/src/cpp/
├── network.rs                    # 800 lines (complete implementation)
└── network_handlers.rs           # 400 lines (message handlers)

rpc/src/
├── websocket.rs                  # 600 lines (WebSocket RPC)
├── light_client.rs               # 200 lines (light client management)
└── mining_work.rs                # 200 lines (work distribution)

node/src/
└── service.rs                    # 400 lines (modified, CPP integration)
```

### **Modified Files**

```
network/src/cpp/mod.rs            # Add network exports
rpc/src/lib.rs                    # Add WebSocket exports
node/src/lib.rs                   # Update service exports
Cargo.toml                        # Add dependencies (tokio-tungstenite)
```

---

## Dependencies

### **New Dependencies** (add to `Cargo.toml`)

```toml
[dependencies]
# WebSocket support
tokio-tungstenite = "0.21"
tungstenite = "0.21"

# JSON-RPC (for WebSocket messages)
serde_json = "1.0"

# Already have:
# tokio, bincode, blake3, serde, etc.
```

---

## Testing Strategy

### **Unit Tests** (300 lines)

```
network/src/cpp/network.rs
├── test_connection_management
├── test_handshake_success
├── test_handshake_failure
├── test_message_dispatch
├── test_broadcast_fanout
├── test_peer_cleanup
└── test_periodic_maintenance

rpc/src/websocket.rs
├── test_client_connection
├── test_work_distribution
├── test_pow_submission
├── test_reward_distribution
└── test_chain_queries
```

### **Integration Tests** (500 lines)

```
network/tests/
├── cpp_two_node_sync.rs          # Two-node sync test
├── cpp_light_client_mining.rs    # Light client mining test
└── cpp_performance_bench.rs      # Performance benchmarks
```

### **End-to-End Tests** (Manual)

1. **Two-Node Sync**
   - Start bootnode (Node 1)
   - Start syncing node (Node 2)
   - Verify sync completes
   - Verify blocks propagate

2. **Light Client Mining**
   - Start full node
   - Connect browser light client
   - Receive mining work
   - Submit PoW
   - Verify reward distribution

3. **Performance Benchmarks**
   - Measure handshake time
   - Measure block propagation latency
   - Measure sync throughput
   - Measure memory usage

---

## Implementation Timeline

### **Week 1: Network Service Core**

**Days 1-2**: Connection management (200 lines)
- Accept connections
- Handshake
- Add/remove peers
- Connect to bootnode

**Days 3-4**: Message handling (300 lines)
- Message dispatcher
- Status, GetBlocks, Blocks
- NewBlock, NewTransaction
- Ping, Pong

**Days 5-6**: Periodic maintenance & broadcasting (300 lines)
- Ping loop
- Cleanup loop
- Metrics updates
- Broadcast implementation

**Day 7**: Testing & debugging
- Unit tests
- Integration tests
- Bug fixes

### **Week 2: WebSocket RPC**

**Days 8-9**: Connection management (150 lines)
- Accept WebSocket connections
- Authentication
- Client management

**Days 10-11**: Mining work distribution (200 lines)
- Work distribution
- PoW submission
- Validation
- Reward distribution

**Days 12-13**: Chain state queries (150 lines)
- Status queries
- Balance queries
- Block/transaction queries

**Day 14**: Testing & debugging
- Unit tests
- Integration tests
- Bug fixes

### **Week 3: Integration & Testing**

**Days 15-16**: Node service integration (400 lines)
- Replace libp2p
- Event loop integration
- Chain state integration

**Days 17-18**: End-to-end testing
- Two-node sync test
- Light client mining test
- Performance benchmarks

**Days 19-20**: Bug fixes & optimization
- Address test failures
- Optimize performance
- Documentation

**Day 21**: Deployment & validation
- Deploy to testnet
- Monitor network behavior
- Collect empirical data

---

## Success Criteria

### **Functional Requirements** ✅

- [ ] Two full nodes can sync via CPP protocol
- [ ] Blocks propagate in <1 second
- [ ] Light clients can connect via WebSocket
- [ ] Light clients can mine and submit PoW
- [ ] Rewards are correctly distributed
- [ ] Reputation system tracks faults
- [ ] Peer quality converges to optimal

### **Performance Requirements** ✅

- [ ] Handshake: <100ms (target: 50ms)
- [ ] Block propagation: <1s (target: 500ms)
- [ ] Sync throughput: >1000 blocks/s
- [ ] Memory usage: <100 MB per node
- [ ] CPU usage: <10% idle, <50% sync

### **Quality Requirements** ✅

- [ ] All unit tests pass
- [ ] All integration tests pass
- [ ] Code coverage: >80%
- [ ] No memory leaks
- [ ] Graceful error handling
- [ ] Comprehensive logging

---

## Risk Mitigation

### **Risk 1: Handshake Failures**

**Mitigation**:
- Comprehensive timeout handling
- Retry logic with exponential backoff
- Detailed error logging
- Fallback to other peers

### **Risk 2: Sync Stalls**

**Mitigation**:
- Multiple sync peers (3-5)
- Adaptive chunk sizing
- Timeout detection
- Automatic peer rotation

### **Risk 3: WebSocket Disconnections**

**Mitigation**:
- Automatic reconnection
- Work queue persistence
- Reward tracking
- Client state recovery

### **Risk 4: Performance Bottlenecks**

**Mitigation**:
- Async/await throughout
- Connection pooling
- Message batching
- Profiling and optimization

---

## Next Steps

1. **Review this plan** - Ensure alignment with vision
2. **Start Week 1** - Implement network service core
3. **Iterate** - Test, debug, optimize
4. **Deploy** - Testnet validation
5. **Measure** - Collect empirical data

**Phase 3 is the final piece. Let's complete the CPP protocol!** 🚀
