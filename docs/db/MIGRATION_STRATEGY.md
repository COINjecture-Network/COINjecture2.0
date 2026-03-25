# Database Migration Strategy

## Overview

COINjecture uses two embedded databases:

| Database | Primary Use | Format |
|----------|------------|--------|
| **redb** (`chain.db`) | Block storage, height index, chain metadata | Copy-on-write B-tree |
| **ADZDB** (`adzdb/`) | Append-only block storage (optional `--use-adzdb`) | Four flat files |

Both databases are embedded and single-process. There are no network connections to manage.

---

## Schema Versioning

### redb (chain.db)

Schema version is stored in the `METADATA_TABLE`:

```
metadata["schema_version"] = u32 (little-endian)
```

On startup, `ChainState::new()` reads this value and applies any pending migrations before opening the node.

Current schema version: **1**

### ADZDB

Version is stored in `adzdb.meta` at byte offset 4 (`Metadata::version`, a `u32`). The current file format version is **1** (`adzdb::VERSION`).

---

## Migration Procedure

### Forward-Only Migrations

Schema changes are always forward-only. There is no rollback support. Before applying a migration:

1. **Stop the node.**
2. **Create a backup**: `coinject backup --dest /path/to/backup`
3. **Run migrations**: `coinject --migrate` (applies all pending migrations and exits)
4. **Restart the node.**

### Adding a New Migration

Create a function in `node/src/chain.rs`:

```rust
fn migrate_v1_to_v2(db: &Database) -> Result<(), ChainError> {
    let write_txn = db.begin_write()?;
    {
        // Example: add a new index table
        let _ = write_txn.open_table(NEW_TABLE)?;
        let mut meta = write_txn.open_table(METADATA_TABLE)?;
        meta.insert("schema_version", 2u32.to_le_bytes().as_ref())?;
    }
    write_txn.commit()?;
    Ok(())
}
```

Register it in the migration runner:

```rust
fn run_migrations(db: &Database, current_version: u32) -> Result<(), ChainError> {
    if current_version < 2 { migrate_v1_to_v2(db)?; }
    // add future migrations here
    Ok(())
}
```

### ADZDB Migrations

ADZDB's append-only design means schema changes require:

1. Export a snapshot: `database.export_snapshot(&dest_dir)?`
2. Transform the snapshot files (custom migration script)
3. Import the transformed snapshot: `Database::import_snapshot(&src_dir, config)?`

For format version upgrades (e.g. `VERSION = 2`), update `Metadata::from_bytes()` to handle both old and new formats during the transition period.

---

## Upgrade Path Summary

| Version Pair | Migration Type | Downtime Required |
|-------------|---------------|-------------------|
| Any → same version | None | No |
| v1 → v2 (redb schema) | In-process migration | Brief (seconds) |
| ADZDB format v1 → v2 | Export/transform/import | Minutes to hours |
| redb → ADZDB | `--use-adzdb` + re-sync | Full re-sync |

---

## Emergency Recovery

If the database is corrupted:

1. **redb**: Delete `chain.db`, restart the node. It will re-sync from peers.
2. **ADZDB**: Delete the `adzdb/` directory, restart. Re-sync from peers.
3. **Both**: Restore from backup, then catch up from peers.

The node includes corruption detection: heights above `MAX_REASONABLE_HEIGHT` (10 million) are automatically rejected and trigger a reset to genesis.
