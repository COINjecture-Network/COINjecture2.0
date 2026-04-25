# Write-Ahead Logging and Crash Recovery

## redb (chain.db)

redb uses a **copy-on-write (CoW) B-tree** design with ACID semantics. There is no separate WAL file.

### How It Works

1. All writes happen in a transaction (`db.begin_write()`).
2. Modified pages are written to new locations; original pages remain untouched until the transaction commits.
3. Commit is a single atomic operation that swaps the root pointer.
4. On crash: if the root pointer was not updated, all partially-written pages are orphaned and ignored on the next open.

### Crash Recovery

**redb recovers each `WriteTransaction::commit()` durably.** What redb itself guarantees:

- On `Database::create(path)`, redb reads the committed root.
- Any uncommitted data from before the crash is invisible.
- Per-method state mutations (`set_balance`, `submit_solution`, `claim_bounty`, etc.) survive crash with full ACID at the **individual commit** boundary.

**However**, block storage and state application are managed as **separate redb transactions**. Block headers/bodies are committed by `node/src/chain.rs` independently of per-method state commits in `state/src/{accounts,marketplace,escrows,channels,trustlines,dimensional_pools,timelocks}.rs`. A single `SubmitSolution` lands ~5 sequential redb commits along `node/src/service/block_processing.rs:1333-1370` — fee deduction, problem update, escrow release, solver credit, nonce.

If a crash interrupts a block-apply between those commits, redb recovers each individual commit but the **block-apply as a whole is left partially applied**. Re-running the block on restart is **not** idempotent: `submit_solution` returns `MarketplaceError::ProblemNotOpen` (`state/src/marketplace.rs:185-187`) and the transaction is skipped at `node/src/service/block_processing.rs:373` (`continue`). The resulting state — escrow not released, solver not credited, problem `Solved` — persists locally.

The system relies on **chain consensus**, not storage-level rollback, to recover from this state: a node with partially-applied state diverges from peers in chain hashes, fork detection triggers, and `unwind_block_transactions` (`node/src/service/block_processing.rs:393`) replays the canonical chain from a common ancestor. See [`FORKING_AND_REORG.md`](../FORKING_AND_REORG.md) for the authoritative reorg semantics.

**Mainnet roadmap (Phase 19+)**: compose per-block apply into a single redb `WriteTransaction` so that block storage and all state mutations share one commit boundary — partial-block-apply rollback then becomes automatic. Until that lands, **partial-block-apply recovery in the absence of healthy peers is not guaranteed** and is a known gap to be closed before mainnet.

### Durability Configuration

redb always calls `fsync` on commit in its default configuration. There is no option to disable this in the current version.

---

## ADZDB

ADZDB is an **append-only** storage engine. Crash recovery relies on the append-only invariant:

- Data is written to `adzdb.dat` sequentially.
- Index entries are appended to `adzdb.idx` only after the data write succeeds.
- Metadata (`adzdb.meta`) is written last.

### Crash Recovery

On startup, ADZDB:

1. Reads `adzdb.meta` to get the committed `entry_count`.
2. Scans `adzdb.idx` up to `entry_count` entries.
3. Any partial write at the end of `.idx` or `.dat` is ignored (the index scan stops at the validated count).

If `sync_on_write = true` (default), every `put()` calls `sync_all()` on all four files before returning. This guarantees durability at the cost of write latency.

To disable per-write sync (higher throughput, lower durability):

```toml
# Not a config file option yet — set programmatically:
Config { sync_on_write: false, .. }
```

### When to Manually Verify Integrity

After an unexpected shutdown on a system without battery-backed write cache:

```bash
# Check ADZDB consistency
coinject verify-db --data-dir /path/to/data
```

This command will be added in a future release. For now, restart the node — ADZDB's append-only design means partial writes simply result in the last block being absent, which triggers a re-sync of that block from peers.

---

## Backup Before Upgrades

Always back up before node upgrades:

```bash
# Stop the node first, then:
coinject backup --data-dir ./data --dest ./backups/pre-upgrade-$(date +%Y%m%d)
```

Or via CLI flags: `--pruning-mode`, `--compaction-interval-hours`.
