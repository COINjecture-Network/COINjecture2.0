# Phase 7 — Structured Logging & Observability

**Date:** 2026-03-25
**Branch:** claude/serene-lichterman
**Build status:** `cargo check` ✓ · `cargo build` ✓ (0 errors, 0 warnings)

---

## Summary

Replaced all ad-hoc `println!`/`eprintln!`/`dbg!` calls across the core node crates
with structured `tracing` macros, and wired up configurable log output with JSON
support and rotating file output.

---

## Changes by file

### `Cargo.toml` (workspace)
- Upgraded `tracing-subscriber` to include `env-filter`, `json`, and `fmt` features.
- Added `tracing-appender = "0.2"` workspace dependency (non-blocking rolling file writer).

### `node/Cargo.toml`
- Added `tracing-appender.workspace = true`.
- Expanded `tracing-subscriber` features to `["env-filter", "json", "fmt"]`.

### `rpc/Cargo.toml`
- Added `tracing.workspace = true`.
- Added `uuid = { version = "1.6", features = ["v4"] }` (foundation for RPC trace IDs).

### `wallet/Cargo.toml`
- Added `tracing.workspace = true`.

### `node/src/main.rs` — logging init + startup/shutdown
- Replaced simple `tracing_subscriber::fmt().init()` with `init_logging()` that supports:
  - **`LOG_FORMAT=json`** — newline-delimited JSON (production).
  - **`LOG_FORMAT=pretty`** (default) — human-readable pretty-print (development).
  - **`LOG_DIR=<path>`** — daily-rotating log file written alongside console output.
  - **`RUST_LOG`** — standard level/filter env var (unchanged).
- Added structured startup `info!` event with sanitized config fields:
  `version`, `node_type`, `chain_id`, `rpc_addr`, `cpp_p2p_addr`, `cpp_ws_addr`,
  `metrics_addr`, `data_dir`, `mining`, `dev_mode`, `bootnode_count`, `max_peers`,
  `difficulty`, `block_time_s`, `hf_sync`.  No secrets (no `hf_token`/`miner_address`).
- Added structured shutdown `info!` event with `uptime_s`.
- Replaced `println!()`/`eprintln!()` in `main()` with `info!`/`error!`.
- `print_banner()` terminal output intentionally retained as `println!` — it is a
  startup UX artifact, not a log event.

### `consensus/src/difficulty.rs`
- Added `use tracing::{debug, warn};`.
- Replaced 17 `println!` calls across `adjust_difficulty`, `adjust_difficulty_async`,
  `penalize_failure`, `penalize_failure_async`, `apply_stall_penalty`:
  - Normal difficulty adjustments → `debug!` with structured fields:
    `avg_solve_time_s`, `target_s`, `time_ratio`, `scale_factor`, `old_size`, `new_size`,
    `min_size`, `max_size`, `recovery_mode`.
  - Mining failure penalties and stall detection → `warn!` with `old_size`, `new_size`,
    `reason`.

### `consensus/src/miner.rs`
- Added `use tracing::{debug, info, warn, error};`.
- Replaced 42 `println!` calls across `mine_header_blocking`, `solve_sat_with_timeout`,
  `mine_block`, `update_stats`, `adjust_difficulty`:
  - Mining target / hash progress → `debug!` with `target_prefix`, `nonce`, `hash_rate`.
  - Nonce found / block mined → `info!` with `block_height`, `hash`, `nonce`.
  - SAT solver timeout / DPLL failure → `warn!` with `variables`, `num_clauses`, `solve_time_s`.
  - Solution verification failure → `error!` with `block_height`.
  - PoUW metrics (solve/verify times, asymmetry, quality, energy) → `debug!` as a
    single structured event.
  - Difficulty stats including stall-ratio warning → `debug!`/`warn!`.

### `node/src/service/block_processing.rs`
- Added `use tracing::{debug, info, warn, error};`.
- Replaced 37 `println!`/`eprintln!` calls:
  - Buffered block applied → `info!` with `block_height`.
  - Fork/orphan block detection → `warn!` with `block_height`, `prev_hash`.
  - State apply/store errors → `error!`.

### `node/src/service/mining.rs`
- Added tracing import.
- Replaced 60 `println!` calls:
  - Mining start/stop lifecycle → `info!`.
  - Block mined → `info!` with `block_height`, `block_hash`.
  - Peer sync progress → `debug!`.
  - Broadcast/store failures → `error!`.
  - Non-fatal failures → `warn!`.

### `node/src/service/fork.rs`
- Added tracing import.
- Replaced 62 `println!` calls:
  - **Chain reorgs (alert-worthy)** → `warn!` with `old_tip_hash`, `new_tip_hash`,
    `reorg_depth`, `fork_height`, `old_work`, `new_work`.
  - Fork detection and chain switch → `warn!`.
  - Diagnostic scan results → `trace!`/`debug!`.

### `node/src/service/mod.rs`
- Added `use tracing::{debug, info, warn, error};`.
- Replaced 141 `println!`/`eprintln!` calls:
  - Node startup, genesis, RPC/network listen → `info!`.
  - Block received/applied → `info!` with `block_height`, `block_hash`.
  - Peer connected/disconnected → `info!` with `peer_id`.
  - Version rejection → `warn!` with `block_height`, `block_version`.
  - Store/apply errors → `error!`.
  - TX pool operations → `debug!`.

---

## What was intentionally NOT changed

| Location | Reason |
|---|---|
| `node/src/main.rs` `print_banner()` | Terminal UX — intentional stdout, not a log event |
| `wallet/src/commands/*.rs` | CLI user-facing output — `println!` is correct for a terminal tool |
| `huggingface/src/client.rs` | Upstream-facing buffering diagnostics — deferred to a separate pass |
| `network/tests/*` and `node/tests/*` | Test harness output — `println!` is standard in tests |

---

## Environment variables

| Variable | Values | Default | Effect |
|---|---|---|---|
| `RUST_LOG` | e.g. `info`, `debug`, `coinject_node=debug` | `info` | Log level / filter |
| `LOG_FORMAT` | `json` · `pretty` | `pretty` | Output format |
| `LOG_DIR` | path string | unset | Enable daily-rotating file output |

---

## Build verification

```
cargo check   → Finished dev profile [unoptimized + debuginfo] (0 errors, 0 warnings)
cargo build   → Finished dev profile [unoptimized + debuginfo] (0 errors, 0 warnings)
```
