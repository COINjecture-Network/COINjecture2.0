# COINjecture P2P Protocol (CPP)

**Version**: 1.0  
**Status**: Initial Implementation  
**Date**: December 17, 2025

---

## Overview

The **COINjecture P2P Protocol (CPP)** is a custom peer-to-peer networking protocol designed to replace libp2p in COINjecture Network B. It is built on the principle of **equilibrium-based optimization** using the dimensionless constant **η = λ = 1/√2 ≈ 0.7071**.

### Why Replace libp2p?

libp2p, while powerful, introduced unnecessary complexity and reliability issues:

- **Complex handshake**: Noise + Yamux (5-6 RTT)
- **NAT traversal failures**: autonat, relay, hole punching issues
- **GossipSub deduplication**: Historical blocks rejected as duplicates
- **Difficult debugging**: Black-box behavior, hard to diagnose
- **Over-engineered**: Features we don't need (Kademlia DHT, mDNS, etc.)

CPP is **simpler, faster, and mathematically grounded**.

---

## Core Principles

### 1. **Equilibrium-Based Design**

Every aspect of CPP uses the equilibrium constant **η = 1/√2 ≈ 0.7071**:

- **Flow control**: Window adapts using η (additive increase, multiplicative decrease)
- **Message routing**: Fanout = √n × η (optimal propagation)
- **Sync chunking**: Adaptive chunk size using η
- **Peer quality**: Exponential decay using (1 - η)

### 2. **Dimensional Message Priorities**

Messages are prioritized using **8 dimensional scales** derived from exponential decay:

| Priority | τ | Scale (Dₙ = e^(-τ/√2)) | Use Case |
|----------|---|------------------------|----------|
| D1_Critical | 0.00 | 1.000 | New blocks, disconnect |
| D2_High | 0.20 | 0.867 | Transactions, work submission |
| D3_Normal | 0.41 | 0.750 | Status updates, handshake |
| D4_Low | 0.68 | 0.618 (φ⁻¹) | Block requests |
| D5_Background | 0.98 | 0.500 (2⁻¹) | Block responses |
| D6_Bulk | 1.36 | 0.382 (φ⁻²) | Header requests |
| D7_Archive | 1.96 | 0.250 (2⁻²) | Header responses |
| D8_Historical | 2.72 | 0.146 (e⁻¹) | Ping/pong |

### 3. **RPC-Integrated Light Client Mining**

Light clients (browsers, mobile) connect via **WebSocket** and mine via RPC:

- **No P2P required**: Light clients don't need full P2P stack
- **Browser-based mining**: JavaScript can submit PoW solutions
- **Reward distribution**: Light clients receive mining rewards via RPC

---

## Network Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    CPP Network Layer                     │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  ┌──────────────┐      ┌──────────────┐                │
│  │ Full Nodes   │◄────►│ Full Nodes   │  TCP/QUIC      │
│  │ (Validators) │      │ (Archive)    │  Port 707      │
│  └──────┬───────┘      └──────┬───────┘                │
│         │                     │                         │
│         │  Equilibrium-based  │                         │
│         │  Message Routing    │                         │
│         │                     │                         │
│  ┌──────▼───────┐      ┌──────▼───────┐                │
│  │ Light Clients│      │ Light Clients│  WebSocket     │
│  │ (Browser)    │      │ (Mobile)     │  Port 8080     │
│  └──────────────┘      └──────────────┘  (RPC)         │
│                                                          │
└─────────────────────────────────────────────────────────┘
```

---

## Port Configuration

### **Port 707** (P2P)

Named after the equilibrium constant **η ≈ 0.707**.

- **Protocol**: TCP (QUIC fallback)
- **Purpose**: Full node communication
- **Traffic**: Blocks, transactions, sync, status

### **Port 8080** (WebSocket)

Standard HTTP alternative port.

- **Protocol**: WebSocket (HTTP upgrade)
- **Purpose**: Light client RPC
- **Traffic**: Mining work, block queries, wallet operations

---

## Wire Protocol

### Message Envelope

All messages use this binary format:

```
┌────────────┬─────────┬──────────┬─────────────┬─────────┬──────────┐
│ Magic (4B) │ Ver (1B)│ Type (1B)│ Length (4B) │ Payload │ Hash (32B)│
└────────────┴─────────┴──────────┴─────────────┴─────────┴──────────┘
```

- **Magic**: `0x43 0x4F 0x49 0x4E` ("COIN")
- **Version**: Protocol version (currently `1`)
- **Type**: Message type (see below)
- **Length**: Payload length in bytes (big-endian)
- **Payload**: Bincode-serialized message
- **Hash**: Blake3 hash of payload (integrity check)

### Message Types

| Type | Name | Description |
|------|------|-------------|
| `0x01` | Hello | Initial handshake |
| `0x02` | HelloAck | Handshake response |
| `0x10` | Status | Peer status update |
| `0x11` | GetBlocks | Request blocks by range |
| `0x12` | Blocks | Block response |
| `0x13` | GetHeaders | Request headers (light) |
| `0x14` | Headers | Header response |
| `0x20` | NewBlock | New block announcement |
| `0x21` | NewTransaction | New transaction |
| `0x30` | SubmitWork | Light client PoW submission |
| `0x31` | WorkAccepted | Work accepted |
| `0x32` | WorkRejected | Work rejected |
| `0x33` | GetWork | Request mining work |
| `0x34` | Work | Mining work template |
| `0xF0` | Ping | Keep-alive |
| `0xF1` | Pong | Keep-alive response |
| `0xFF` | Disconnect | Graceful disconnect |

---

## Handshake Sequence

### Full Node to Full Node

```
Client                                Server
  │                                     │
  ├─────── Hello ────────────────────►│
  │  (version, peer_id, height, hash)  │
  │                                     │
  │◄────── HelloAck ───────────────────┤
  │  (version, peer_id, height, hash)  │
  │                                     │
  ├─────── Status ────────────────────►│
  │  (periodic updates)                 │
  │                                     │
  │◄────── Status ─────────────────────┤
  │  (periodic updates)                 │
  │                                     │
```

**Total**: 1 RTT (vs 5-6 RTT for libp2p)

### Light Client to Full Node (WebSocket)

```
Client                                Server
  │                                     │
  ├─────── WebSocket Upgrade ─────────►│
  │  (HTTP → WS)                        │
  │                                     │
  │◄────── 101 Switching Protocols ────┤
  │                                     │
  ├─────── GetWork ───────────────────►│
  │  (miner_address)                    │
  │                                     │
  │◄────── Work ───────────────────────┤
  │  (header, difficulty, txs)          │
  │                                     │
  ├─────── SubmitWork ────────────────►│
  │  (block with PoW solution)          │
  │                                     │
  │◄────── WorkAccepted ───────────────┤
  │  (reward amount)                    │
  │                                     │
```

---

## Equilibrium-Based Flow Control

### Algorithm

```rust
// On ACK (successful delivery)
window += η  // Additive increase

// On timeout (congestion/loss)
window *= (1 - η)  // Multiplicative decrease
```

### Properties

- **Critical damping**: Fastest convergence without overshoot
- **No oscillation**: Smooth adaptation to network conditions
- **Provably optimal**: Based on fundamental physics (damped harmonic oscillator)

### Comparison to TCP

| Algorithm | Increase | Decrease | Convergence |
|-----------|----------|----------|-------------|
| **TCP Reno** | +1/window | ×0.5 | Slow, oscillates |
| **TCP Cubic** | Cubic | ×0.7 | Fast, complex |
| **CPP** | +η | ×(1-η) | Optimal, simple |

---

## Equilibrium-Based Routing

### Broadcast Fanout

When broadcasting a new block, send to **√n × η** peers:

```rust
let fanout = (total_peers as f64).sqrt() * 0.7071;
```

**Why?**
- Too few peers: Slow propagation
- Too many peers: Network congestion
- **√n × η**: Critical damping (fastest without congestion)

### Sync Peer Selection

Select peer with smallest **dimensional distance**:

```rust
let tau = |peer_height - required_height| * η;
```

Peer with smallest τ is closest in dimensional space.

### Adaptive Chunk Sizing

```rust
let chunk_size = base * (1 + delta_height * η / 10);
```

Chunk size grows with height difference, capped at maximum.

---

## Light Client Mining

### Workflow

1. **Light client connects** via WebSocket (port 8080)
2. **Requests work** via `GetWork` message
3. **Receives template** (header, difficulty, transactions)
4. **Mines locally** (JavaScript, WebAssembly, or native)
5. **Submits solution** via `SubmitWork` message
6. **Receives reward** if work is accepted

### Advantages

- **No full node required**: Light clients don't store blockchain
- **Browser-based**: Mine directly from web browser
- **Mobile-friendly**: Low bandwidth, low storage
- **Instant rewards**: Receive mining rewards immediately

### Security

- **PoW validation**: Full nodes validate all submitted work
- **Difficulty adjustment**: Same as full node mining
- **Reward distribution**: Light clients receive proportional rewards
- **Sybil resistance**: PoW prevents spam

---

## Implementation Status

### ✅ Completed (Phase 1)

- [x] Port configuration (707, 8080)
- [x] Message type definitions
- [x] Equilibrium-based flow control
- [x] Equilibrium-based routing
- [x] Dimensional message priorities

### 🚧 In Progress (Phase 2)

- [ ] Protocol encoding/decoding
- [ ] Peer management
- [ ] Connection handling
- [ ] Message serialization

### 📋 Planned (Phase 3)

- [ ] WebSocket RPC integration
- [ ] Light client mining
- [ ] Integration with node service
- [ ] Replace libp2p completely

---

## Testing

### Unit Tests

```bash
cargo test --package coinject-network --lib cpp
```

### Integration Tests

```bash
# Start bootnode
cargo run --bin coinject-node -- --p2p-port 707 --ws-port 8080

# Start second node
cargo run --bin coinject-node -- --p2p-port 708 --ws-port 8081 --bootnode 127.0.0.1:707
```

### Light Client Test

```javascript
// Browser console
const ws = new WebSocket('ws://localhost:8080');

ws.onopen = () => {
  // Request mining work
  ws.send(JSON.stringify({
    method: 'mining_getWork',
    params: { miner_address: '0x...' },
    id: 1
  }));
};

ws.onmessage = (event) => {
  const response = JSON.parse(event.data);
  console.log('Received:', response);
};
```

---

## Performance Targets

| Metric | Target | Current (libp2p) |
|--------|--------|------------------|
| **Handshake time** | < 100ms | ~500ms |
| **Block propagation** | < 1s | ~3s |
| **Sync throughput** | > 1000 blocks/s | ~100 blocks/s |
| **Memory usage** | < 100 MB | ~300 MB |
| **CPU usage** | < 10% | ~20% |

---

## Future Enhancements

### Phase 4: QUIC Transport

- UDP-based transport for better NAT traversal
- 0-RTT handshake (even faster than TCP)
- Connection migration (survives IP changes)

### Phase 5: Compression

- Zstd compression for large messages
- Dictionary-based compression for repeated data
- Adaptive compression based on message type

### Phase 6: Encryption

- Optional TLS for sensitive data
- Noise protocol for peer authentication
- End-to-end encryption for light clients

---

## References

1. **Equilibrium Constant**: Proof(3).pdf (Satoshi Constant derivation)
2. **Dimensional Tokenomics**: COINjecture-Whitepaper(3).pdf
3. **TCP Congestion Control**: RFC 5681
4. **WebSocket Protocol**: RFC 6455
5. **Blake3 Hashing**: https://github.com/BLAKE3-team/BLAKE3

---

## Contact

For questions or contributions, see:
- **Repository**: https://github.com/Quigles1337/COINjecture2.0
- **Branch**: `remove-libp2p`
- **Documentation**: `/docs/CPP_PROTOCOL.md`
