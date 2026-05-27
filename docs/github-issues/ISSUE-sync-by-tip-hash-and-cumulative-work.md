# GitHub issue draft: sync-by-tip-hash / cumulative work for empty nodes

**Use this file to open an issue** (copy body below or run the `gh` command at the end).

---

## Title

`P2P sync: empty nodes can adopt orphan chain via height-indexed GetBlocks before cumulative-work fork choice`

## Labels (suggested)

`bug`, `network`, `consensus`, `good first issue` (optional)

---

## Body

### Summary

Fresh or wiped nodes can sync through a **stale fork** (shared prefix, divergent block at height *N*) and then **stall forever** when the peer advertises a higher tip on the winning branch. Height-based `GetBlocks` + sequential `prev_hash` extension does not anchor initial sync to the peer’s advertised **`best_hash` / `best_cumulative_work`**.

We hit this in production on the mesh VPS fleet (May 2026): canonical miner at **h=936** (`2eb4e159…`, W≈6791) while two followers stuck at **h=746** (`0000ea6c…`, W=4764) with logs:

```text
sync progress blocks_applied=0 current_height=746 peer_height=933
sync batch made no progress; skipping immediate continuation request
```

Wiping volumes and setting `COINJECT_BOOTNODES` to the canonical host **only** still re-synced to the orphan tip at 746 within ~30s. **Workaround:** clone canonical `bootnode-data` volume (~9 MB) to followers. **Desired fix:** code path so empty nodes follow the heaviest tip by hash/work without manual DB copy.

### Expected behavior

1. Empty node receives peer `StatusUpdate { best_height, best_hash, cumulative_work }`.
2. Node downloads the chain that connects **genesis → `best_hash`**, preferring the branch with **greater cumulative work** when multiple headers exist at the same height.
3. After sync, `chain_getInfo` matches trusted peers on **`best_hash`** and **`best_cumulative_work`**, not only height.

### Actual behavior

1. `StatusUpdate` triggers `RequestBlocks { from_height: local+1, to_height: min(peer, local+100) }` (`node/src/service/mod.rs`, ~1701–1720).
2. Peer serves blocks via `BlockProvider::get_blocks_range` → `get_block_by_height` → **`height_index` table** (`network/src/cpp/network.rs` ~1745–1746, `node/src/chain.rs` ~442–453).
3. Sync applies blocks only when `height == best_height+1 && prev_hash == best_hash` (~1224–1226).
4. Through the shared prefix (here blocks 0–745), extension succeeds.
5. At the fork height (746), the served block can be the **orphan** header still referenced in `height_index[746]` on a peer that has already reorganized to a higher tip—parent matches local tip, so it is stored.
6. Blocks 747+ from the winning branch have a different parent hash → `blocks_applied=0`, “sync batch made no progress”, follower stuck on ghost tip.

`docs/FORKING_AND_REORG.md` already warns that identical heights ≠ identical chains; initial sync does not fully act on that.

### Reproduction (operator)

1. Run a canonical node with tip at height **H** on branch **B_win** after a fork at height **F** (&lt; H).
2. Ensure another branch **B_orphan** through **F** remains in the DB (`height_index[F]` → orphan hash); or keep a peer that still serves **B_orphan**.
3. Wipe a follower; set bootnodes to the canonical P2P address only.
4. Start follower; observe `chain_getInfo` stops at **F** with orphan `best_hash` while `peer_height` ≈ **H**.
5. Logs: `sync batch made no progress` / `sync batch stalled on alternate branch`.

### Code pointers

| Area | Location |
|------|----------|
| Status-driven height range sync | `node/src/service/mod.rs` (`CppNetworkEvent::StatusUpdate`, `RequestBlocks`) |
| Sequential apply + buffer on parent mismatch | `node/src/service/mod.rs` (~1224–1306) |
| Stall → reorg hook | `node/src/service/mod.rs` (~1464–1488) |
| Serve blocks by height index | `network/src/cpp/network.rs` `handle_get_blocks`, `node/src/chain.rs` `ChainBlockProvider` |
| Fork / cumulative work | `node/src/service/fork.rs`, `node/src/peer_consensus.rs`, `docs/FORKING_AND_REORG.md` |
| RPC fields | `chain_getInfo`: `best_hash`, `best_cumulative_work` |

### Proposed directions (pick one or combine)

**A. Headers-first / hash-anchored sync (preferred)**

- On empty or far-behind node, after `StatusUpdate`, request headers walking back from `best_hash` until connecting to local tip (or genesis).
- Compare **`cumulative_work`** (and tie-breaks) across peer tips before committing blocks.
- Only then download full blocks along the chosen chain (by hash chain, not “all blocks at height *h*”).

**B. Fix GetBlocks serving on reorg**

- Audit `height_index` updates during `reorganize_chain` / `store_block`: after reorg to **B_win**, must `get_block_by_height(F)` return **B_win**’s block at *F*, not the orphan.
- Add test: build fork at F, reorg to heavier chain, assert `get_blocks_range(F, F)` matches winning header hash.

**C. Stricter empty-node gate at fork height**

- When `best_cumulative_work` from peer ≫ local, and next block at `expected_height` connects locally but peer’s advertised tip hash is not reachable from that block, **do not** store—buffer and trigger `check_and_reorganize_chain` / hash-anchored download.
- Compare peer `StatusUpdate.best_hash` with `header.hash()` that would result from applying the next block.

**D. Operator RPC (interim)**

- Document `checkpoint_sync` / import canonical datadir (current workaround).
- Optional: `chain_getBlockHeader` batch by hash for trusted checkpoint.

### Acceptance criteria

- [ ] Integration test: two branches at height F, heavier tip at H; empty node syncs only from peer serving **B_win** → matches `best_hash` and `best_cumulative_work`.
- [ ] Integration test: reorged node does not serve orphan block at F via `get_block_by_height(F)` after tip is on **B_win**.
- [ ] No manual volume clone required for mesh followers after fork.
- [ ] `docs/FORKING_AND_REORG.md` updated with “initial sync” subsection.

### References

- Production incident: mesh followers stuck at 746 vs canonical 936 (May 2026).
- Related log strings: `sync batch made no progress`, `sync batch stalled on alternate branch`, `next sync block has mismatched parent hash, buffering for fork resolution`.

---

## Create on GitHub

```bash
gh auth login   # if needed
gh issue create \
  --repo COINjecture-Network/COINjecture2.0 \
  --title "P2P sync: empty nodes can adopt orphan chain via height-indexed GetBlocks before cumulative-work fork choice" \
  --body-file docs/github-issues/ISSUE-sync-by-tip-hash-and-cumulative-work.md \
  --label bug
```

Or open: https://github.com/COINjecture-Network/COINjecture2.0/issues/new
