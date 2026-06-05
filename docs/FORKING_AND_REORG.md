# Forking, chain work, and reorganization

This document describes how the COINjecture node **detects competing branches**, **chooses a canonical tip**, and **reorganizes** local state. It is aimed at protocol engineers and operators debugging divergent hosts.

## Terms

| Term | Meaning |
|------|--------|
| **Block / header** | A committed unit of consensus. At height *h* there may be **multiple** distinct headers with the same numeric height (competing miners or partitions). |
| **Canonical chain** | The best chain stored locally: `best_height` / `best_hash` in `ChainState`. |
| **Fork** | Two valid blocks (or chains) that **disagree** on history: same genesis (or shared prefix) then different headers at some height. |
| **Reorganization (reorg)** | Atomically switching the canonical tip from chain **A** to chain **B** by unwinding state past a **common ancestor** and applying blocks from **B**. |
| **Sync buffer** | In-memory `HashMap<height, Block>` used while P2P delivers blocks out of order or on an alternate parent before persistence (`node/src/service/mod.rs`). |

**Important:** Identical **height numbers** on two hosts do **not** imply the same chain. Always compare **header bytes** (e.g. `chain_getBlockHeader`) or **hashes**, or walk **parents** until a match.

## Fork choice (which branch is “winning”?)

At a high level the node prefers the chain with **greater cumulative work** (and related tie-break rules implemented in consensus / `WorkScoreCalculator` — see `node/src/peer_consensus.rs` and `node/src/service/fork.rs`).

- RPC exposes **`best_cumulative_work`** on `chain_getInfo` as a decimal string (`u128` sum along the canonical path).
- Peers advertise height and cumulative work on the CPP layer; the node tracks them in **`PeerConsensus`**.

When partitions heal, nodes are expected to **reorg** toward the heavier chain **if** they can validate the blocks and find a **safe common ancestor** (below).

## Common ancestor

Reorgs must not jump blindly. The node finds **`(common_hash, common_height)`** where:

- The ancestor lies on **both** the current canonical chain and the candidate branch, and  
- It is **anchored** deep enough to limit abuse (see `MIN_ANCHOR_DEPTH` in `attempt_reorganization_if_longer_chain` in `node/src/service/fork.rs` — currently **6** blocks).

### Algorithm location

- **`ChainState::find_common_ancestor`** — disk-backed canonical chain only (`node/src/chain.rs`).
- **`ChainState::find_common_ancestor_with_alt_chain`** — same walk, but resolves **candidate-branch** headers from an extra slice `alt_chain` when they are **not yet in the database** (`node/src/chain.rs`).

The ADZDB backend mirrors the same API when built with `--features adzdb` (`node/src/chain_adzdb.rs`).

### Why `find_common_ancestor_with_alt_chain` exists

During sync, blocks for the **competing** branch often live only in the **sync buffer** until a reorg succeeds. The older code walked alternate parents using **only** `get_block_by_hash` on disk. That returned `None` for buffered headers, so the node logged **“complete fork: no common ancestor”** even when the branch **did** connect to an ancestor already on disk — it simply was not persisted yet.

**Going forward:** fork detection and `attempt_reorganization_if_longer_chain` pass the buffer snapshot into `find_common_ancestor_with_alt_chain` so the ancestor walk can traverse **buffered** alternate headers (`node/src/service/fork.rs`).

## End-to-end reorg flow (simplified)

1. **Ingestion** — CPP delivers blocks; sequential extensions apply immediately; mismatched parents go to the **buffer** (`node/src/service/mod.rs`, block receive / batch sync paths).
2. **Periodic + event-driven checks** — `CoinjectNode::check_and_reorganize_chain` (`node/src/service/fork.rs`, spawned every ~15s and after “stalled sync” warnings).
3. **Buffered tip analysis** — If `max_buffered_height > best_height`, take the highest buffered block and run **`find_common_ancestor_with_alt_chain(..., &buffer_snapshot)`**.
4. **Longer stored chain** — Scans may find stored blocks ahead of the tip; reorg attempts use the same ancestor + work rules.
5. **`attempt_reorganization_if_longer_chain`** — Validates ancestor depth, compares work vs height, then either:
   - **Normal reorg** — unwind from ancestor through old tip, apply new segment; or  
   - **“Complete fork” path** — `find_common_ancestor` returns `None` **even with buffer help**: candidate chain does not connect to our DB/buffer view back to a shared block. Then **`validate_chain_from_genesis`** may run (expensive) and, if successful, **`reorganize_chain_from_genesis`** (`node/src/service/fork.rs`).

## Initial P2P sync (hash-anchored)

Height-range `GetBlocks` alone is not sufficient when a peer’s database still maps an old fork at some heights:

1. **`get_block_by_height`** (and `BlockProvider::get_blocks_range`) walk the **current best tip** backward, not a stale `height_index` entry left by a side chain.
2. **`height_index`** is updated only when a block extends the best chain (or after **`rebuild_height_index_from_canonical_tip`** following a reorg).
3. Before applying a sequential sync block, the node checks that the block hash lies on the **peer’s advertised tip chain** when the peer reports higher **`cumulative_work`** (`node/src/sync_canonical.rs`, `BlocksReceived` in `node/src/service/mod.rs`).
4. Sync continuation requests prefer the active peer with the greatest advertised **`cumulative_work`**.
5. **Fork-height re-download:** when the local tip hash is **not** on the peer’s advertised tip chain and the peer has greater cumulative work, sync requests start at **`local_height`** (not `local_height + 1`) so the fork-point block on the winning branch is fetched and buffered for reorg (`sync_canonical::plan_sync_batch`, `request_hash_anchored_sync` in `node/src/service/mod.rs`).
6. **Same-height reorg:** if the buffer holds a competing block at the current tip height, `attempt_reorganization_if_longer_chain` may replace the tip hash without requiring a higher block number (`node/src/service/fork.rs`).
7. **Reorg emission work:** during `reorganize_chain`, parent cumulative work is walked in-memory along `new_chain_blocks` (common ancestor from disk, then each buffered successor). A DB-only lookup at the fork height fails with `Emission parent work … Block not found` when the winning branch block is not yet stored.
8. **Heaviest-peer recovery (no median gate):** when sync stalls (`SuspectFork`) or a same-height buffered fork is shorter than the mesh winner, the node requests hash-anchored blocks from `PeerConsensus::best_peer_for_fork_recovery` — cumulative work first, height tie-break — even when the local tip is far below median peer height (e.g. h=746 vs canonical h=2750). Stall recovery no longer re-requests from the peer that sent the incompatible batch.
9. **Same-height reorg is not deferred:** once the fork-point block at the local tip height is buffered, reorg runs immediately. Blocks at `h+1` parent off the winning hash at `h`; waiting for a longer buffered segment while the heaviest peer is far ahead leaves followers stuck at `h` with `blocks_applied=0`.
10. **Fork-point capture:** when buffered block `h+1` parents off a hash ≠ local tip, the node must buffer the winning header at `h` (replacing the orphan tip entry). Hash-anchored sync issues an explicit `GetBlocks { from: h, to: h }` and always buffers same-height replacements (`missing_fork_point_parent_hash` in `node/src/sync_canonical.rs`).
11. **Sync plan must not skip fork height when work=0:** `sync_from_height_for_heavier_peer` re-fetches `local.height` whenever `peer.height > local.height` and the local tip hash is not on the peer's advertised chain — even if `cumulative_work` has not been advertised yet (otherwise batches start at `h+1` and the fork-point block is never buffered).

If a wiped node still cannot catch up, verify bootnodes point at the canonical miner only (never another lagging follower) or clone a canonical datadir as a last resort.

## Sync vs fork (`is_syncing`)

`chain_getInfo.is_syncing` is exposed over JSON-RPC. It is now driven by **peer-relative lag** (same periodic task that triggers fork checks):

- `true` when  
  `best_height + sync_threshold < median_peer_height`  
  **or**  
  `best_height + sync_threshold < best_peer_height`  
  using `PeerConsensus` (`node/src/service/mod.rs`).

This is a **heuristic** (“we appear behind the mesh”), not a guarantee that a specific peer’s branch is valid. Operators should still compare **`best_hash` / headers** across hosts when investigating forks.

## Operational guidance when hosts diverge

1. Confirm **same `genesis_hash`** (`chain_getInfo`).
2. Binary-search **header equality** with `chain_getBlockHeader` from both tips downward until the last matching height — that is the **last common block** between those two RPC endpoints.
3. Prefer the chain with **higher cumulative work** among nodes you trust.
4. If a single container is corrupt or stuck on a ghost branch (logs: “no common ancestor”, “sync batch made no progress”), **reset only that container’s volume** and resync — see `docker-compose.yml` volume names and `docker-compose.bootnode-health-metrics-only.yml` for bootnode health under load.

## Mesh resilience: RPC must stay live under P2P load

Production incidents where **`/chain/info` timed out** while the node was “healthy” were caused by **blocking the shared Tokio runtime**, not by a bad chain tip alone.

### Root causes (fixed in v4.8.5+)

| Issue | Symptom | Fix |
|-------|---------|-----|
| **Outbound bootnode dials inside the CPP `select!` loop** | `accept()` stalled 30–60s; followers could not complete Hello/HelloAck; JSON-RPC queued for minutes | `OutboundConnectCtx` + `schedule_connect_bootnode()` — TCP + handshake run in a **spawned task** |
| **`chain_getInfo` scanned every header** | O(height) DB reads on the RPC worker; tip ≈ 2750 → multi-second stalls | O(1) `calculate_chain_work` via chain metadata (`node/src/chain.rs`) + `spawn_blocking` in `rpc/src/server.rs` |
| **`BlockProvider::get_best_height` used `block_on`** | Sync serving wedged async workers under concurrent GetBlocks | Lock-free `AtomicU64` tip mirror on `ChainState` |
| **Eclipse slots not released on stale peers** | `[SECURITY][ECLIPSE] subnet full (8/8)` while peers were dead | Release eclipse + connection limiter on timeout / `remove_peer` |
| **Heavy GetBlocks serve on RPC thread** | CPU starvation on single-core VPS | `spawn_blocking` for `get_blocks_range` in `handle_get_blocks` |

### Operator notes

- **Canonical host (193):** keep JSON-RPC responsive; optional `docker-compose.bootnode-no-mine.yml` or `docker-compose.node1-only-mine.yml` separates mining from the API-facing bootnode.
- **Health checks:** use `docker-compose.bootnode-health-metrics-only.yml` on busy bootnodes — do not curl `chain_getInfo` from Docker healthcheck while serving large sync batches.
- **API:** `GET /chain/info` uses an 8s upstream timeout and falls back to the last cached tip if the node is momentarily busy (`api-server/src/routes/chain.rs`).

## Primary source files

| Area | File |
|------|------|
| Ancestor walk + disk | `node/src/chain.rs` |
| ADZDB variant | `node/src/chain_adzdb.rs` (feature `adzdb`) |
| Fork / reorg orchestration | `node/src/service/fork.rs` |
| Sync buffer + CPP events | `node/src/service/mod.rs` |
| Peer height / median / work | `node/src/peer_consensus.rs` |
| RPC `chain_getInfo` | `rpc/src/server.rs` |

When changing fork semantics, update **this document** and keep **`chain.rs` / `chain_adzdb.rs` ancestor walks aligned** (see comment in `chain_adzdb.rs`).
