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
