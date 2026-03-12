# COINjecture CPP Network — Surgical Audit Report

**Auditor**: Claude Opus 4.6
**Date**: 2026-03-05
**Codebase**: `network/src/cpp/` (COINjecture P2P Protocol)
**Commit**: `e0a809c` (post-merge of remove-libp2p)

---

## Executive Summary

The CPP layer is architecturally sound — the equilibrium math (η = 1/√2),
murmuration flocking, flow control, and reputation systems are well-designed.
However, **critical wiring gaps** between components prevent production use.
The EquilibriumRouter is never populated, FlockState never advances, peer
shutdown is never called (task leak), and there is no message deduplication.
These are all integration bugs, not design bugs.

---

## BUG 1: EquilibriumRouter Never Populated (CRITICAL)

**Files**: `network.rs:907-910`, `network.rs:462-468`, `network.rs:957-970`, `network.rs:946-954`
**Type**: Dead code / integration gap

The router is created in constructors but never has peers added:

```rust
// network.rs:907-910 (connect_bootnode)
let _router = self.router.write().await;
// TODO: Add peer to router

// network.rs:462-468 (handle_incoming_connection)
// _router parameter is prefixed with underscore — never used

// network.rs:952-953 (add_peer helper)
let _router = self.router.write().await;
// TODO: Add peer to router

// network.rs:967-968 (remove_peer)
let _router = self.router.write().await;
// TODO: Remove peer from router
```

**Impact**: `select_broadcast_peers()` at line 1323 falls back to naive
`peers.keys().take(fanout)` — all murmuration math runs on an empty HashMap.
The `EquilibriumRouter::select_broadcast_peers_flock()` method is pure dead code.

---

## BUG 2: Peer Shutdown Never Called (RESOURCE LEAK)

**Files**: `network.rs:612-619`, `network.rs:957-970`, `network.rs:1453-1471`
**Type**: Memory/resource leak

`Peer::shutdown()` (peer.rs:373-382) sets `write_task_cancel` to true, but:

- `remove_peer()` (network.rs:959): `peers.remove(peer_id)` without calling shutdown
- `cleanup_stale_peers()` (network.rs:1465): `peers.remove(&peer_id)` without shutdown
- `peer_message_loop()` (network.rs:614): `peers_guard.remove(&peer_id)` without shutdown

**Impact**: Every disconnected peer leaves a leaked tokio write task running
indefinitely. For a long-running testnet, this accumulates unboundedly.

---

## BUG 3: FlockState Never Advances (LOGIC BUG)

**Files**: `network.rs:1003-1007`, `flock.rs:115-136`, `flock.rs:339-345`
**Type**: Dead logic

FlockState is created at genesis height (network.rs:257/300) but never updated:

- `UpdateChainState` command (network.rs:1003-1007) updates `chain_state` only
- `FlockState::update_from_peers()` (flock.rs:115) exists but is never called
- `MurmurationRules::advance_epoch()` (flock.rs:339) exists but is never called

**Impact**: Flock epoch stays at 0, phase is frozen, cohesion is always 1.0.
All murmuration coordination is effectively disabled.

---

## BUG 4: StatusMessage FlockState Ignored (LOGIC BUG)

**Files**: `network.rs:1018-1048`
**Type**: Dead field / integration gap

`StatusMessage.flock_state: Option<FlockStateCompact>` is deserialized in
`handle_status()` but the field is never read. `EquilibriumRouter::update_peer_flock()`
(router.rs:329-340) exists but is never called.

Additionally, `handle_status()` doesn't have access to the router — it's not
in the function signature.

**Impact**: Peer flock data is transmitted over the wire but discarded. Router
never knows peers' flock phases or velocities.

---

## BUG 5: pending_requests Grows Unbounded (MEMORY LEAK)

**Files**: `network.rs:1385-1388`, `network.rs:1110-1127`
**Type**: Memory leak

`request_blocks()` inserts `(request_id, Instant::now())` into `pending_requests`
at line 1386-1388. The `Instant` value is clearly intended for TTL tracking.
However:

- `handle_blocks()` (line 1110-1127) never removes fulfilled requests
- No periodic TTL cleanup exists anywhere

**Impact**: `pending_requests` grows by one entry per block sync request, forever.

---

## BUG 6: No Message Deduplication (PROTOCOL BUG)

**Files**: `network.rs:1129-1165`
**Type**: Missing feature

`handle_new_block()` and `handle_new_transaction()` emit events but have no
seen-message cache. When node A receives a block from B and re-broadcasts it,
node C may receive it from both A and B, processing it twice. With more nodes,
this creates broadcast amplification.

**Impact**: Redundant block/tx processing, wasted bandwidth, potential for
exponential message amplification in larger networks.

---

## BUG 7: handle_status Missing Router Parameter (BLOCKER)

**Files**: `network.rs:1018-1024`, `network.rs:643`
**Type**: Missing parameter

`handle_status()` signature does not include `router: Arc<RwLock<EquilibriumRouter>>`.
This blocks Fix 1.4 (flock data propagation to router).

The call site at line 643 also doesn't pass router.

---

## BUG 8: request_headers is a Silent No-Op (STUB)

**Files**: `network.rs:1394-1408`
**Type**: Stub / silent failure

```rust
async fn request_headers(...) -> Result<(), NetworkError> {
    let _peer = peers.get(&peer_id)...;
    // TODO: Send GetHeaders message
    Ok(())
}
```

Returns `Ok(())` without doing anything. Callers think headers were requested.

---

## BUG 9: println!/eprintln! Instead of tracing (OPERATIONAL)

**Files**: `network.rs` (60+ instances), `peer.rs` (8 instances)
**Type**: Operational debt

The entire network layer uses `println!`/`eprintln!` for logging despite
`tracing` and `tracing-subscriber` being workspace dependencies. This prevents:
- Log level filtering (debug/info/warn/error)
- Structured logging with span context
- Log aggregation in production

---

## BUG 10: update_metrics() is Empty Stub

**Files**: `network.rs:1679-1690`
**Type**: Stub

```rust
async fn update_metrics(&self) {
    let _metrics = self.metrics.write().await;
    let _peers = self.peers.read().await;
    // TODO: Calculate metrics
}
```

Acquires locks and does nothing. Runs every 5 minutes.

---

## BUG 11: PeerSelector Never Used

**Files**: `network.rs:1336-1337`
**Type**: Dead code

```rust
let _selector = PeerSelector;
// TODO: Implement peer selection based on reputation and metrics
```

`PeerSelector::select_for_propagation()` in `node_integration.rs:214-237` is
fully implemented but never called.

---

## BUG 12: Hardcoded NodeType in Handshake

**Files**: `network.rs:730`
**Type**: Minor bug

```rust
node_type: NodeType::Full.as_u8(), // TODO: Get from config
```

Incoming handshake always reports as `Full` regardless of actual config.
`connect_bootnode()` correctly uses `self.config.node_type.as_u8()`.

---

## BUG 13: Unused block_provider Parameters

**Files**: `network.rs:1114`, `network.rs:1134`, `network.rs:1153`
**Type**: Dead parameter

`handle_blocks()`, `handle_new_block()`, `handle_new_transaction()` all accept
`block_provider: Arc<dyn BlockProvider>` but never use it. This adds unnecessary
Arc cloning on every message.

---

## BUG 14: peer_message_loop Doesn't Clean Router on Disconnect

**Files**: `network.rs:612-622`
**Type**: Integration gap

When a peer disconnects in the message loop, it's removed from `peers` but
never removed from the router. This leaves stale entries in the router's
peer map.

---

## BUG 15: CURRENT_ISSUES.md Documents Old Codebase

**Files**: `CURRENT_ISSUES.md`
**Type**: Documentation debt

References gossipsub mesh parameters, `12D3KooW...` PeerIds, `coinject-network-b`
topics. The CPP rewrite replaced libp2p entirely.

---

## BUG 16: docker-compose.yml Uses Wrong Ports

**Files**: `docker-compose.yml`
**Type**: Configuration error

Uses ports 30333-30336 (libp2p era) instead of CPP_PORT 707. Also references
`--mine` flag and `--rpc-port` which may not match current node CLI.

---

## BUG 17: No tracing_subscriber Initialization

**Files**: `node/src/main.rs` (needs verification)
**Type**: Missing initialization

Even after replacing println with tracing macros, `tracing_subscriber::fmt::init()`
must be called at startup for output to appear.

---

## Summary Table

| # | Severity | Type | Location | Status |
|---|----------|------|----------|--------|
| 1 | CRITICAL | Dead code | Router never populated | Phase 1.1 |
| 2 | HIGH | Resource leak | Peer shutdown not called | Phase 1.2 |
| 3 | HIGH | Logic bug | FlockState frozen | Phase 1.3 |
| 4 | MEDIUM | Dead field | Flock data discarded | Phase 1.4 |
| 5 | MEDIUM | Memory leak | pending_requests unbounded | Phase 1.5 |
| 6 | MEDIUM | Protocol bug | No message dedup | Phase 1.6 |
| 7 | BLOCKER | Missing param | handle_status no router | Phase 1.7 |
| 8 | LOW | Stub | request_headers silent | Phase 1.8 |
| 9 | MEDIUM | Operational | println instead of tracing | Phase 1.9 |
| 10 | LOW | Stub | update_metrics empty | Phase 2.1 |
| 11 | MEDIUM | Dead code | PeerSelector unused | Phase 2.2 |
| 12 | LOW | Bug | Hardcoded NodeType::Full | Phase 1.1 |
| 13 | LOW | Dead param | Unused block_provider args | Cleanup |
| 14 | MEDIUM | Gap | Router not cleaned on dc | Phase 1.1 |
| 15 | LOW | Docs | CURRENT_ISSUES.md stale | Phase 5.1 |
| 16 | LOW | Config | docker-compose wrong ports | Phase 4.1 |
| 17 | LOW | Missing init | tracing_subscriber | Phase 1.9 |
