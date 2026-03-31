# COINjecture Mesh Bridge Protocol

## Overview

The **Mesh Bridge** (`network/src/mesh/bridge.rs`) connects the node service to the **Mesh networking layer** — a generic P2P overlay that runs alongside the primary CPP (COINjecture P2P Protocol) on port 707.

The bridge is a translation layer: it converts concrete `Block`/`Transaction` types from the node service into mesh `Payload` variants, and maps incoming mesh messages back to typed events for the node service.

```
  Node Service
      │
      ├── BridgeCommand ──▶ bridge task ──▶ MeshCommand ──▶ MeshNetworkService
      │
      └── BridgeEvent  ◀── bridge task ◀── MeshEvent   ◀── MeshNetworkService
```

---

## Architecture

### Transport Layers

| Layer | Protocol | Port | Purpose |
|-------|----------|------|---------|
| CPP | TCP/custom | 707 | Primary: full nodes, validators, sync |
| Mesh | TCP/custom | configurable | Secondary: discovery, gossip, consensus coordination |

The CPP layer handles block propagation and chain sync for full nodes.
The Mesh layer handles:
- Node discovery and peer exchange
- Consensus salt and commit broadcasts
- Light-client connectivity
- Redundant transaction propagation

### Bridge Commands (Node → Mesh)

| Command | Mesh Payload | Description |
|---------|-------------|-------------|
| `BroadcastBlock` | `Payload::Solution` | Broadcast newly mined block |
| `BroadcastTransaction` | `Payload::BountySubmit` | Broadcast new transaction |
| `RequestBlocks` | `Payload::ChainSyncRequest` | Request blocks from peer |
| `UpdateChainState` | (internal state) | Update heartbeat chain tip |
| `ConnectBootnode` | (mesh config) | Bootstrap connection |
| `BroadcastConsensusSalt` | `Payload::ConsensusSalt` | Epoch salt broadcast |
| `BroadcastCommit` | `Payload::Commit` | Solution commit broadcast |

### Bridge Events (Mesh → Node)

| Event | Trigger | Description |
|-------|---------|-------------|
| `PeerConnected` | `MeshEvent::PeerConnected` | Mesh peer handshake complete |
| `PeerDisconnected` | `MeshEvent::PeerDisconnected` | Mesh peer left |
| `StatusUpdate` | `Payload::Heartbeat` | Peer chain tip update |
| `BlockReceived` | `Payload::Solution` | Block from mesh peer |
| `TransactionReceived` | `Payload::BountySubmit` (type="transaction") | Tx from mesh peer |
| `BlocksReceived` | `Payload::ChainSyncResponse` | Sync response |
| `ConsensusSaltReceived` | `Payload::ConsensusSalt` | Epoch salt from peer |
| `ConsensusCommitReceived` | `Payload::Commit` | Solution commit from peer |

---

## Payload Encoding

### Block → Mesh

Blocks are encoded as `Payload::Solution`:

```rust
Payload::Solution {
    epoch: current_epoch,
    problem_id: format!("block-{}", block.header.height),
    solution_hash: *block.header.hash().as_bytes(),
    proof: bincode::serialize(&block),  // full block bytes
}
```

### Transaction → Mesh

Transactions are encoded as `Payload::BountySubmit` with `problem_type = "transaction"`:

```rust
Payload::BountySubmit {
    bounty_id: format!("tx-{}", hex::encode(&tx_hash[..8])),
    problem_type: "transaction".into(),
    payload: bincode::serialize(&transaction),
}
```

### Consensus Salt → Mesh

```rust
Payload::ConsensusSalt { epoch, salt: [u8; 32] }
```

### Solution Commit → Mesh

```rust
Payload::Commit {
    epoch,
    block_hash: solution_hash,
    commits: vec![NodeCommit {
        node_id: NodeId(node_id),
        solution_hash,
        work_score,
        signature,
    }],
}
```

---

## Fee Structure

The mesh bridge does not charge fees directly — fees are handled at the mempool level before transactions enter the network.  However, the bridge participates in fee-adjacent mechanisms:

| Mechanism | Description |
|-----------|-------------|
| **Relay incentive** | Nodes that relay transactions and blocks to peers they discovered via mesh connectivity contribute to network health, tracked via peer reputation scoring in `network/src/reputation.rs` |
| **Consensus rewards** | Nodes that broadcast valid consensus salts and commits on time receive higher coordinator scores (epoch coordination bonus) |
| **Block provider priority** | Peers with high reputation (derived partly from mesh behavior) are preferred for sync requests |

---

## Message Verification

All mesh messages are authenticated at the mesh transport layer:

1. **Handshake**: `HandshakeMessage` in `network/src/mesh/protocol.rs` contains a node signature.
2. **Envelope signature**: `Envelope.signature` signs the `(msg_id, sender, payload)` tuple with the sender's Ed25519 key.
3. **TTL enforcement**: `Envelope.ttl` is decremented at each hop; messages with `ttl = 0` are dropped, preventing message storms.
4. **Checksum**: BLAKE3 checksum in the CPP layer (for CPP messages routed via the bridge, the CPP layer already validated the checksum before delivery).

---

## Bridge State

The bridge maintains a `BridgeState` (wrapped in `Arc<RwLock<BridgeState>>`) with:

```rust
pub struct BridgeState {
    pub best_height: u64,   // Updated by UpdateChainState commands
    pub best_hash: Hash,    // Used in heartbeat broadcasts
    pub epoch: u64,         // Current consensus epoch
}
```

Heartbeat broadcasts automatically reflect the latest chain state.

---

## Known Limitations & Future Work

| Item | Status | Notes |
|------|--------|-------|
| Block serving via mesh | Partial | `ChainSyncRequest` received by bridge but not served — CPP layer handles block serving for now |
| Peer address in `PeerConnected` | Placeholder | `0.0.0.0:0` — mesh layer does not currently expose remote socket addr in events |
| Request-ID correlation for sync | Best-effort | Multiple concurrent sync requests may correlate to wrong responses; use CPP sync for production |
| Encrypted mesh transport | Planned | Noise XX upgrade in protocol V3 |

---

## Code Location

| Component | File |
|-----------|------|
| Bridge task | `network/src/mesh/bridge.rs` |
| Mesh payloads | `network/src/mesh/protocol.rs` |
| Mesh identity | `network/src/mesh/identity.rs` |
| Mesh gossip | `network/src/mesh/gossip.rs` |
| Node lib exports | `network/src/lib.rs` |
