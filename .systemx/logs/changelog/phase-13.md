# Phase 13 — Database & State Management

**Date:** 2026-03-25
**Branch:** claude/nice-babbage

---

## Summary

Comprehensive database and state management hardening across `adzdb`, `node/chain`, `node/config`, and `node/metrics`.

---

## Changes

### 1. Database Migration Strategy (`docs/db/MIGRATION_STRATEGY.md`)
- Documented forward-only version migration for both redb and ADZDB
- Defined schema versioning stored in metadata tables
- Established migration runner pattern for `node --migrate` subcommand

### 2. State Pruning (`node/src/chain.rs`, `adzdb/src/lib.rs`)
- **`ChainState::prune_blocks_before(keep_height)`** — deletes block + height-index entries below `keep_height` from redb; preserves genesis and best block
- **`Database::prune_before(keep_height)`** — logical prune from in-memory ADZDB indices; data remains on disk until `compact_to` is called
- Both return the number of pruned entries

### 3. Backup / Restore (`node/src/chain.rs`, `adzdb/src/lib.rs`)
- **`ChainState::backup(dest_dir)`** — copies the redb file to `{dest_dir}/chain.db.bak`
- **`Database::export_snapshot(dest_dir)`** — copies all 4 ADZDB files to destination directory
- **`Database::import_snapshot(src_dir, dest_config)`** — creates a new DB from a snapshot directory

### 4. Connection Pooling
- Not applicable: redb and ADZDB are embedded single-process databases with no network connections. Documented in `docs/db/CONFIGURATION.md`.

### 5. Write-Ahead Logging (`docs/db/WAL_AND_RECOVERY.md`)
- Documented that redb uses a copy-on-write B-tree with ACID semantics; no separate WAL file needed
- ADZDB uses `sync_on_write: true` (configurable) for per-write fsync

### 6. State Snapshots (`node/src/chain.rs`, `adzdb/src/lib.rs`)
- **`ChainState::export_snapshot(dest_dir)`** — exports redb file named `chain-snapshot-{height}.db`
- ADZDB `export_snapshot` + `import_snapshot` for full portability

### 7. Index Optimization (`docs/db/CONFIGURATION.md`)
- redb HEIGHT_INDEX_TABLE (u64 key) already provides O(log n) height lookups
- ADZDB height_index is an O(1) in-memory `HashMap<u64, Hash>`
- Documented appropriate use cases for each index type

### 8. Compaction (`node/src/chain.rs`, `adzdb/src/lib.rs`)
- **`ChainState::compact_database(path)`** — standalone function (offline, node must be stopped); calls `redb::Database::compact()`
- **`Database::compact_to(dest_config)`** — ADZDB online compaction: writes clean copy containing only currently-indexed entries

### 9. Database Metrics (`node/src/metrics.rs`)
Added Prometheus metrics:
- `coinject_db_chain_size_bytes` — chain database file size
- `coinject_db_state_size_bytes` — state database file size
- `coinject_db_block_count` — number of stored blocks
- `coinject_db_read_latency_seconds{table}` — read latency histogram
- `coinject_db_write_latency_seconds{table}` — write latency histogram
- `coinject_db_reads_total{table}` — total read counter
- `coinject_db_writes_total{table}` — total write counter
- `coinject_db_last_compaction_timestamp` — last compaction epoch
- Added `update_db_metrics(chain_bytes, state_bytes, block_count)` helper

### 10. Configuration (`node/src/config.rs`, `docs/db/CONFIGURATION.md`)
New `NodeConfig` fields:
- `--db-cache-mb` (default: 64) — page cache size in MB
- `--pruning-mode` (`archive`|`full`, default: `archive`)
- `--pruning-keep-blocks` (default: 10 000) — blocks retained in `full` mode
- `--compaction-interval-hours` (default: 24) — periodic compaction schedule
- `--block-cache-size` (default: 512) — in-memory block cache entries
- `--state-cache-size` (default: 1024) — in-memory state cache entries
- Added `PruningMode` enum (`Archive`, `Full`)
- Added validation for all new fields

### Extended Stats
- `ChainStats` now includes `db_file_size_bytes`
- ADZDB `ExtendedDatabaseStats` includes `db_size_bytes` and `index_entries_in_memory`

---

## cargo check
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.43s
```
Zero errors.
