# Write-Ahead Logging and Crash Recovery

## redb (chain.db)

redb uses a **copy-on-write (CoW) B-tree** design with ACID semantics. There is no separate WAL file.

### How It Works

1. All writes happen in a transaction (`db.begin_write()`).
2. Modified pages are written to new locations; original pages remain untouched until the transaction commits.
3. Commit is a single atomic operation that swaps the root pointer.
4. On crash: if the root pointer was not updated, all partially-written pages are orphaned and ignored on the next open.

### Crash Recovery

**You do not need to do anything after a crash.** redb recovers automatically:

- On `Database::create(path)`, redb reads the committed root.
- Any uncommitted data from before the crash is invisible.
- The node resumes from the last committed block.

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
