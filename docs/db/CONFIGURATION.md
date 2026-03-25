# Database Configuration Reference

All database options are passed as CLI flags to `coinject`. They can also be set via environment variables using SCREAMING_SNAKE_CASE (e.g. `COINJECT_DB_CACHE_MB=128`).

---

## Cache

| Flag | Default | Description |
|------|---------|-------------|
| `--db-cache-mb` | `64` | Page cache size in megabytes. Larger values improve read performance for large chains at the cost of RAM. Recommended: 256 MB for full nodes, 64 MB for light nodes. |
| `--block-cache-size` | `512` | Number of recent deserialized `Block` objects held in the in-memory LRU cache. Reduces bincode deserialization overhead for the hot path. |
| `--state-cache-size` | `1024` | Number of account-state entries held in the in-memory LRU cache. Reduces state-DB reads during transaction validation. |

---

## Pruning

| Flag | Default | Description |
|------|---------|-------------|
| `--pruning-mode` | `archive` | `archive` — keep all blocks. `full` — keep only the most recent `--pruning-keep-blocks` blocks. |
| `--pruning-keep-blocks` | `10000` | How many of the most recent blocks to retain in `full` pruning mode. Minimum: 100. |

### Pruning Notes

- Genesis (height 0) is **never** pruned.
- The best block and any block above the prune horizon are **never** pruned.
- After pruning redb, run `coinject compact` to reclaim disk space.
- After pruning ADZDB (logical prune), run `compact_to` to produce a smaller clean copy.
- Light nodes (`--node-type light`) automatically skip block body storage; pruning is not needed.

---

## Compaction

| Flag | Default | Description |
|------|---------|-------------|
| `--compaction-interval-hours` | `24` | Run compaction every N hours. `0` disables automatic compaction. Compaction reclaims disk space freed by pruned entries. |

### Manual Compaction (redb)

Stop the node, then run:

```bash
coinject compact-db --data-dir ./data
```

This calls `ChainState::compact_database(path)` which requires exclusive access.

### Online Compaction (ADZDB)

ADZDB supports online compaction via `compact_to(dest_config)`:

```rust
let new_db = db.compact_to(Config::new("./data/adzdb_compact"))?;
```

This writes a clean copy containing only currently-indexed entries. Swap the directories when done.

---

## Snapshots and Backup

| Operation | API | Notes |
|-----------|-----|-------|
| Export chain snapshot | `ChainState::export_snapshot(dest_dir)` | Embeds current height in filename |
| Backup chain DB | `ChainState::backup(dest_dir)` | Copies to `chain.db.bak`; stop node for safety |
| Export ADZDB snapshot | `Database::export_snapshot(dest_dir)` | Copies all 4 files |
| Import ADZDB snapshot | `Database::import_snapshot(src, config)` | Opens imported copy |

---

## Connection Pooling

Not applicable. Both redb and ADZDB are embedded single-process databases. There are no network connections or connection pools to manage.

---

## Index Structure

### redb

| Table | Key | Value | Purpose |
|-------|-----|-------|---------|
| `blocks` | `[u8; 32]` (hash) | `&[u8]` (serialized Block) | Block storage |
| `height_index` | `u64` | `[u8; 32]` | Height → hash |
| `metadata` | `&str` | `&[u8]` | Chain state (best height, genesis hash, schema version) |

All tables use redb's B-tree structure with O(log n) lookups. The `height_index` provides efficient range scans for pruning.

### ADZDB

| File | Structure | Purpose |
|------|-----------|---------|
| `adzdb.dat` | Sequential append | Raw block data |
| `adzdb.idx` | Fixed-width entries (56 bytes) | Hash → offset mapping |
| `adzdb.hgt` | Fixed-width entries (40 bytes) | Height → hash mapping |
| `adzdb.meta` | 96-byte header | Entry count, latest height, genesis hash |

In-memory: `HashMap<Hash, IndexEntry>` and `HashMap<u64, Hash>` for O(1) lookups.

---

## Monitoring

Database metrics are exported at the Prometheus endpoint (`--metrics-addr`, default `127.0.0.1:9090`):

```
coinject_db_chain_size_bytes        — chain.db file size
coinject_db_state_size_bytes        — state.db file size
coinject_db_block_count             — blocks in chain DB
coinject_db_read_latency_seconds    — read latency by table
coinject_db_write_latency_seconds   — write latency by table
coinject_db_reads_total             — total reads by table
coinject_db_writes_total            — total writes by table
coinject_db_last_compaction_timestamp — last compaction (epoch)
```
