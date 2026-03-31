# Phase 3: Testing Strategy

**Goal**: Ensure CPP protocol and WebSocket RPC are production-ready  
**Coverage Target**: >80%  
**Timeline**: Integrated with implementation (test-driven development)

---

## Testing Pyramid

```
                    ┌─────────────────┐
                    │   E2E Tests     │  Manual, exploratory
                    │   (5 tests)     │  Two-node sync, light client
                    └─────────────────┘
                  ┌───────────────────────┐
                  │  Integration Tests    │  Automated
                  │  (15 tests)           │  Multi-component
                  └───────────────────────┘
              ┌─────────────────────────────────┐
              │      Unit Tests                 │  Automated
              │      (50+ tests)                │  Single component
              └─────────────────────────────────┘
```

---

## Unit Tests (50+ tests)

### **Network Service Tests** (`network/src/cpp/network.rs`)

#### **Connection Management** (10 tests)

```rust
#[tokio::test]
async fn test_accept_connection() {
    // Test accepting incoming connection
}

#[tokio::test]
async fn test_handshake_success() {
    // Test successful Hello/HelloAck exchange
}

#[tokio::test]
async fn test_handshake_genesis_mismatch() {
    // Test handshake failure due to genesis mismatch
}

#[tokio::test]
async fn test_handshake_timeout() {
    // Test handshake timeout handling
}

#[tokio::test]
async fn test_add_peer() {
    // Test adding peer to peer list
}

#[tokio::test]
async fn test_remove_peer() {
    // Test removing peer and sending disconnect event
}

#[tokio::test]
async fn test_connect_bootnode() {
    // Test connecting to bootnode
}

#[tokio::test]
async fn test_peer_limit_enforcement() {
    // Test max peer limit (50 peers)
}

#[tokio::test]
async fn test_duplicate_connection_detection() {
    // Test rejecting duplicate peer connections
}

#[tokio::test]
async fn test_peer_state_transitions() {
    // Test peer state: Connecting → Connected → Disconnected
}
```

#### **Message Handling** (15 tests)

```rust
#[tokio::test]
async fn test_handle_status() {
    // Test processing status updates
}

#[tokio::test]
async fn test_handle_get_blocks() {
    // Test serving block requests
}

#[tokio::test]
async fn test_handle_blocks() {
    // Test processing block responses
}

#[tokio::test]
async fn test_handle_new_block() {
    // Test processing new block announcements
}

#[tokio::test]
async fn test_handle_new_transaction() {
    // Test processing new transactions
}

#[tokio::test]
async fn test_handle_ping() {
    // Test responding to pings
}

#[tokio::test]
async fn test_handle_pong() {
    // Test processing pong responses
}

#[tokio::test]
async fn test_message_priority_handling() {
    // Test D1-D8 priority handling
}

#[tokio::test]
async fn test_duplicate_block_detection() {
    // Test rejecting duplicate blocks
}

#[tokio::test]
async fn test_duplicate_transaction_detection() {
    // Test rejecting duplicate transactions
}

#[tokio::test]
async fn test_flow_control_integration() {
    // Test flow control window updates
}

#[tokio::test]
async fn test_reputation_updates() {
    // Test reputation updates on faults
}

#[tokio::test]
async fn test_message_timeout_handling() {
    // Test handling message timeouts
}

#[tokio::test]
async fn test_invalid_message_handling() {
    // Test handling invalid messages
}

#[tokio::test]
async fn test_message_rate_limiting() {
    // Test rate limiting per peer
}
```

#### **Broadcasting** (8 tests)

```rust
#[tokio::test]
async fn test_broadcast_block() {
    // Test broadcasting block to selected peers
}

#[tokio::test]
async fn test_broadcast_transaction() {
    // Test broadcasting transaction to selected peers
}

#[tokio::test]
async fn test_equilibrium_fanout() {
    // Test fanout = √n × η
}

#[tokio::test]
async fn test_peer_selection_by_node_type() {
    // Test prioritizing Validators > Full > Archive
}

#[tokio::test]
async fn test_peer_selection_by_quality() {
    // Test selecting high-quality peers
}

#[tokio::test]
async fn test_broadcast_duplicate_prevention() {
    // Test not sending to peers that already have the block
}

#[tokio::test]
async fn test_broadcast_failure_handling() {
    // Test handling peer disconnection during broadcast
}

#[tokio::test]
async fn test_broadcast_metrics() {
    // Test tracking broadcast success/failure
}
```

#### **Periodic Maintenance** (7 tests)

```rust
#[tokio::test]
async fn test_send_pings() {
    // Test sending keepalive pings
}

#[tokio::test]
async fn test_cleanup_stale_peers() {
    // Test removing timed-out peers
}

#[tokio::test]
async fn test_update_metrics() {
    // Test updating node metrics
}

#[tokio::test]
async fn test_update_router() {
    // Test updating router with peer info
}

#[tokio::test]
async fn test_broadcast_status() {
    // Test broadcasting status to peers
}

#[tokio::test]
async fn test_maintenance_intervals() {
    // Test maintenance task scheduling
}

#[tokio::test]
async fn test_graceful_shutdown() {
    // Test graceful shutdown handling
}
```

### **WebSocket RPC Tests** (`rpc/src/websocket.rs`)

#### **Connection Management** (5 tests)

```rust
#[tokio::test]
async fn test_accept_websocket_connection() {
    // Test accepting WebSocket connection
}

#[tokio::test]
async fn test_authenticate_client() {
    // Test client authentication
}

#[tokio::test]
async fn test_add_client() {
    // Test adding client to client map
}

#[tokio::test]
async fn test_remove_client() {
    // Test removing client
}

#[tokio::test]
async fn test_client_timeout() {
    // Test removing stale clients
}
```

#### **Mining Work Distribution** (8 tests)

```rust
#[tokio::test]
async fn test_distribute_work() {
    // Test distributing work to clients
}

#[tokio::test]
async fn test_get_work() {
    // Test client requesting work
}

#[tokio::test]
async fn test_submit_work() {
    // Test client submitting PoW
}

#[tokio::test]
async fn test_validate_submission() {
    // Test validating PoW submission
}

#[tokio::test]
async fn test_notify_reward() {
    // Test sending reward notification
}

#[tokio::test]
async fn test_work_expiration() {
    // Test work expiring after timeout
}

#[tokio::test]
async fn test_work_assignment_verification() {
    // Test verifying work was assigned to client
}

#[tokio::test]
async fn test_multiple_submissions() {
    // Test handling multiple submissions from same client
}
```

#### **Chain Queries** (5 tests)

```rust
#[tokio::test]
async fn test_get_status() {
    // Test querying chain status
}

#[tokio::test]
async fn test_get_balance() {
    // Test querying balance
}

#[tokio::test]
async fn test_get_block() {
    // Test querying block
}

#[tokio::test]
async fn test_get_transaction() {
    // Test querying transaction
}

#[tokio::test]
async fn test_submit_transaction() {
    // Test submitting transaction
}
```

---

## Integration Tests (15 tests)

### **Two-Node Sync** (`network/tests/cpp_two_node_sync.rs`)

```rust
#[tokio::test]
async fn test_two_node_handshake() {
    // Start two nodes, verify handshake
}

#[tokio::test]
async fn test_two_node_status_exchange() {
    // Verify status updates between nodes
}

#[tokio::test]
async fn test_two_node_block_propagation() {
    // Node 1 mines block, verify Node 2 receives it
}

#[tokio::test]
async fn test_two_node_sync_from_genesis() {
    // Node 2 syncs entire chain from Node 1
}

#[tokio::test]
async fn test_two_node_sync_partial() {
    // Node 2 syncs missing blocks from Node 1
}

#[tokio::test]
async fn test_two_node_fork_detection() {
    // Nodes detect fork and reorganize
}

#[tokio::test]
async fn test_two_node_transaction_propagation() {
    // Node 1 broadcasts transaction, Node 2 receives it
}
```

### **Light Client Mining** (`network/tests/cpp_light_client_mining.rs`)

```rust
#[tokio::test]
async fn test_light_client_connect() {
    // Light client connects via WebSocket
}

#[tokio::test]
async fn test_light_client_authenticate() {
    // Light client authenticates
}

#[tokio::test]
async fn test_light_client_get_work() {
    // Light client requests mining work
}

#[tokio::test]
async fn test_light_client_submit_work() {
    // Light client submits PoW
}

#[tokio::test]
async fn test_light_client_receive_reward() {
    // Light client receives reward notification
}

#[tokio::test]
async fn test_light_client_multiple_submissions() {
    // Multiple light clients mining simultaneously
}
```

### **Performance Benchmarks** (`network/tests/cpp_performance_bench.rs`)

```rust
#[tokio::test]
async fn bench_handshake_time() {
    // Measure handshake time (target: <100ms)
}

#[tokio::test]
async fn bench_block_propagation_latency() {
    // Measure block propagation time (target: <1s)
}

#[tokio::test]
async fn bench_sync_throughput() {
    // Measure sync throughput (target: >1000 blocks/s)
}

#[tokio::test]
async fn bench_memory_usage() {
    // Measure memory usage (target: <100 MB)
}

#[tokio::test]
async fn bench_cpu_usage() {
    // Measure CPU usage (target: <10% idle)
}
```

---

## End-to-End Tests (Manual)

### **Test 1: Two-Node Sync**

**Setup**:
1. Start Node 1 (bootnode) on port 707
2. Mine 100 blocks on Node 1
3. Start Node 2, connect to Node 1

**Expected Behavior**:
- Node 2 connects successfully
- Node 2 syncs all 100 blocks
- Node 2 reaches same best_height as Node 1
- Blocks propagate in real-time after sync

**Success Criteria**:
- ✅ Handshake completes in <100ms
- ✅ Sync completes in <10 seconds
- ✅ All blocks validated correctly
- ✅ No memory leaks
- ✅ No crashes

**Commands**:
```bash
# Terminal 1: Start bootnode
cargo run --bin coinject-node -- \
  --p2p-port 707 \
  --mine \
  --bootnode

# Terminal 2: Start syncing node
cargo run --bin coinject-node -- \
  --p2p-port 708 \
  --bootnode cpp://127.0.0.1:707/BOOTNODE_PEER_ID

# Monitor sync progress
tail -f node2.log | grep "Sync"
```

### **Test 2: Light Client Mining**

**Setup**:
1. Start full node on port 707 (P2P) and 8080 (WebSocket)
2. Open browser, connect to ws://localhost:8080
3. Authenticate light client
4. Request mining work
5. Submit PoW

**Expected Behavior**:
- Light client connects successfully
- Light client receives mining work
- Light client submits valid PoW
- Light client receives reward notification

**Success Criteria**:
- ✅ WebSocket connection stable
- ✅ Work distribution in <1 second
- ✅ Reward notification received
- ✅ Balance updated correctly

**Browser Console**:
```javascript
const ws = new WebSocket('ws://localhost:8080');

ws.onopen = () => {
    console.log('Connected');
    ws.send(JSON.stringify({
        type: 'auth',
        client_id: 'test_miner',
        signature: []
    }));
};

ws.onmessage = (event) => {
    const msg = JSON.parse(event.data);
    console.log('Received:', msg);
    
    if (msg.type === 'work_response') {
        // Solve problem (mock)
        ws.send(JSON.stringify({
            type: 'submit_work',
            work_id: msg.work_id,
            solution: [1, 2, 3],
            nonce: 12345
        }));
    }
};

// Request work
ws.send(JSON.stringify({ type: 'get_work' }));
```

### **Test 3: Fork Resolution**

**Setup**:
1. Start Node 1 and Node 2, let them sync
2. Disconnect Node 2
3. Mine 10 blocks on Node 1
4. Mine 5 blocks on Node 2 (fork)
5. Reconnect Node 2

**Expected Behavior**:
- Node 2 detects fork
- Node 2 requests Node 1's chain
- Node 2 reorganizes to Node 1's chain (longer)
- Node 2 discards its 5 blocks

**Success Criteria**:
- ✅ Fork detected correctly
- ✅ Reorganization completes
- ✅ Node 2 matches Node 1's chain
- ✅ No orphaned blocks remain

### **Test 4: Stress Test (50 Peers)**

**Setup**:
1. Start bootnode
2. Start 50 nodes, all connecting to bootnode
3. Mine blocks on random nodes
4. Measure propagation time

**Expected Behavior**:
- All nodes connect successfully
- Blocks propagate to all nodes
- Network remains stable

**Success Criteria**:
- ✅ All 50 nodes connected
- ✅ Block propagation <5 seconds
- ✅ Memory usage <100 MB per node
- ✅ CPU usage <50% per node
- ✅ No crashes or disconnections

### **Test 5: Light Client Swarm (100 Clients)**

**Setup**:
1. Start full node
2. Connect 100 light clients via WebSocket
3. Distribute mining work
4. Measure throughput

**Expected Behavior**:
- All clients connect successfully
- Work distributed evenly
- Submissions processed correctly

**Success Criteria**:
- ✅ All 100 clients connected
- ✅ Work distribution <1 second per client
- ✅ Submission processing <100ms
- ✅ No dropped connections
- ✅ Memory usage <200 MB

---

## Performance Targets

| Metric | Target | Measurement |
|--------|--------|-------------|
| **Handshake Time** | <100ms | Average of 100 handshakes |
| **Block Propagation** | <1s | Time from broadcast to receipt |
| **Sync Throughput** | >1000 blocks/s | 10,000 blocks / time |
| **Memory Usage** | <100 MB | Per full node |
| **CPU Usage (idle)** | <10% | Average over 5 minutes |
| **CPU Usage (sync)** | <50% | Average during sync |
| **WebSocket Latency** | <50ms | Ping-pong round trip |
| **Work Distribution** | <1s | Time from request to response |

---

## Continuous Integration

### **GitHub Actions Workflow**

```yaml
name: CPP Protocol Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      
      - name: Run unit tests
        run: cargo test --package coinject-network --lib cpp
      
      - name: Run integration tests
        run: cargo test --package coinject-network --test cpp_protocol_tests
      
      - name: Run two-node sync test
        run: cargo test --package coinject-network --test cpp_two_node_sync
      
      - name: Run light client test
        run: cargo test --package coinject-network --test cpp_light_client_mining
      
      - name: Check code coverage
        run: cargo tarpaulin --out Xml
      
      - name: Upload coverage
        uses: codecov/codecov-action@v3
```

---

## Test Data

### **Test Genesis Block**

```rust
pub fn test_genesis_hash() -> Hash {
    Hash::from_bytes([0x42u8; 32])
}

pub fn test_genesis_block() -> Block {
    Block {
        header: BlockHeader {
            version: 1,
            height: 0,
            prev_hash: Hash::ZERO,
            timestamp: 1700000000,
            transactions_root: Hash::ZERO,
            solutions_root: Hash::ZERO,
            commitment: Commitment {
                hash: Hash::ZERO,
                problem_hash: Hash::ZERO,
            },
            work_score: 0.0,
            miner: Address::from_bytes([0u8; 32]),
            nonce: 0,
            solve_time_us: 0,
            verify_time_us: 0,
            time_asymmetry_ratio: 0.0,
            solution_quality: 0.0,
            complexity_weight: 0.0,
            energy_estimate_joules: 0.0,
        },
        coinbase: CoinbaseTransaction::new(Address::from_bytes([0u8; 32]), 0, 0),
        transactions: Vec::new(),
        solution_reveal: SolutionReveal::default(),
    }
}
```

### **Test Peer IDs**

```rust
pub fn test_peer_id(seed: u8) -> PeerId {
    let mut id = [0u8; 32];
    id[0] = seed;
    id
}

pub fn test_bootnode_peer_id() -> PeerId {
    test_peer_id(1)
}

pub fn test_node2_peer_id() -> PeerId {
    test_peer_id(2)
}
```

---

## Success Criteria Summary

### **Functional** ✅

- [ ] Two nodes can sync via CPP
- [ ] Blocks propagate correctly
- [ ] Light clients can mine
- [ ] Rewards distributed correctly
- [ ] Forks detected and resolved
- [ ] Reputation tracks faults

### **Performance** ✅

- [ ] Handshake: <100ms
- [ ] Propagation: <1s
- [ ] Sync: >1000 blocks/s
- [ ] Memory: <100 MB
- [ ] CPU: <10% idle, <50% sync

### **Quality** ✅

- [ ] All unit tests pass
- [ ] All integration tests pass
- [ ] Code coverage: >80%
- [ ] No memory leaks
- [ ] Graceful error handling
- [ ] Comprehensive logging

---

## Next: Implementation

With this testing strategy, you can implement Phase 3 using **test-driven development**:

1. Write test first
2. Implement feature
3. Run test
4. Fix bugs
5. Refactor
6. Repeat

**This ensures production-ready code from day one.** 🚀
