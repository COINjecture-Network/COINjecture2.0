# Phase 4: Consensus Safety — Changelog

**Date:** 2026-03-25
**Branch:** `claude/objective-wing`
**Build status:** `cargo check` ✓ | `cargo build` ✓

---

## Summary

Phase 4 hardened the consensus layer against a class of correctness and safety bugs that could cause network splits, double-spend, Byzantine stalling, or non-deterministic block selection. All changes are backward-compatible at the binary level (no wire format changes).

---

## Changes by file

### NEW: `core/src/fixed_point.rs`

Deterministic integer arithmetic for all consensus-critical computations. Eliminates IEEE 754 platform-specific rounding (ARM vs x86, x87 80-bit extended precision) that can cause nodes to disagree on work scores.

Key exports:
- `SCALE: u64 = 1_000_000` — scale factor (6 decimal places)
- `log2_ratio(numerator, denominator) -> Option<Fixed64>` — integer log₂ via bit-counting + 32-bit fixed-point linear interpolation. Max error < 0.086 bits. Platform-exact.
- `apply_quality(score, quality_bps) -> Fixed64` — multiply by basis-point quality score
- `isqrt(n: u128) -> u128` — Newton's method integer square root (replaces `f64::sqrt()` in difficulty adjustment)
- `from_f64_lossy` / `to_f64` — display-only bridges; explicitly banned from consensus paths

### `core/src/lib.rs`

Added `pub mod fixed_point;` export with documentation note.

### `consensus/src/work_score.rs`

Added deterministic work score path:
- `calculate_deterministic(solve_time_us, verify_time_us, quality_bps) -> Fixed64` — integer-only, uses `log2_ratio()` and `apply_quality()`. Safe under NaN/zero/division-by-zero.
- `chain_security_fixed(work_scores: &[Fixed64]) -> u128` — sum over integer scores
- `block_a_wins(a_score, b_score) -> bool` — deterministic fork choice comparison
- Constants: `MIN_VERIFY_TIME_US = 1_000` (1 ms floor), `MIN_ASYMMETRY_US = 2` (minimum 2:1 solve/verify ratio)

### `consensus/src/difficulty.rs`

Replaced floating-point difficulty adjustment with deterministic integer math:

| Before | After |
|--------|-------|
| `recent_solve_times: VecDeque<f64>` (seconds, IEEE 754) | `recent_solve_times_us: VecDeque<u64>` (microseconds, exact integer) |
| `f64::sqrt(current_size² × ratio)` | `isqrt(current_size² × target_us / avg_us)` |
| No floor on difficulty | `ABSOLUTE_MIN_SIZE = 1` — difficulty can never reach zero |
| `record_solve_time(f64)` | `record_solve_time_us(u64)` via `Duration::as_micros()` |

f64 is still used in `stats()` / `stats_async()` (monitoring boundary, never in consensus decisions).

Added `DEFAULT_TARGET_US: u64 = 5_000_000` (5 s in µs) to replace the old `f64` bootstrap constant.

### `node/src/validator.rs`

Three new validation functions enforcing consensus invariants:

1. **`validate_block_sequence(block, parent)`** — Rejects blocks where:
   - `block.header.height != parent.header.height + 1`
   - `block.header.prev_hash != parent.header.hash()`
   - `block.header.timestamp <= parent.header.timestamp` (strict monotonicity)

2. **`validate_transaction_ordering(block)`** — Enforces canonical transaction ordering:
   - Transactions must be sorted ascending by hash
   - No duplicate transactions (detects equivocation)
   - Required so all validators compute the same `transactions_root`

3. **`validate_nonces(block, state)`** — Pre-validates all transaction nonces before any state changes:
   - Uses in-memory HashMap to simulate within-block nonce progression
   - Rejects any transaction whose nonce ≠ current_nonce + 1 (prevents replay and skip)
   - Catches within-block nonce reuse (e.g. two txs from same sender with same nonce)

New `ValidationError` variants: `TimestampNotAfterParent`, `InvalidSequence`, `DuplicateTransaction`, `TransactionOrderViolation`.

### `node/src/genesis.rs`

Hardened genesis validation against genesis-replacement attacks:

- `is_valid_genesis()` now includes step 6: `block.header.hash() == genesis_hash()`. An attacker who modifies any genesis field (address, supply, problem, timestamp) produces a different hash and is rejected.
- Added `is_genesis_attack(block) -> bool` — returns `true` if a block claims height 0 but doesn't match the canonical genesis hash.
- New tests: `test_invalid_genesis_wrong_prev_hash`, `test_genesis_attack_detection`, `test_is_genesis_attack_canonical_returns_false`.

### `state/src/accounts.rs`

Atomic block application to prevent double-spend races and partial block state:

- **`apply_block_atomically(balance_changes, nonce_increments)`** — applies all balance changes AND nonce increments in a single redb write transaction. Either everything commits or nothing does. Prevents partial block application on crash and eliminates the double-spend window between individual `transfer()` calls.
- **`get_balances_batch(addresses)`** — reads multiple balances in a single read transaction (more efficient than repeated `get_balance()` calls for block pre-checks).
- **`get_nonces_batch(addresses)`** — same pattern for nonces.
- Fixed missing `use redb::ReadableTable;` import required by `apply_block_atomically`.

### `consensus/src/coordinator/epoch.rs`

Added hard deadline tracking to prevent Byzantine leaders from blocking consensus indefinitely:

- `epoch_start: Instant` field added to `EpochState`
- `epoch_elapsed() -> Duration` — total epoch age
- **`has_exceeded_hard_deadline(config) -> bool`** — true when `epoch_elapsed >= sum(all phase durations) + 2 × stall_timeout`. This gives room for normal operation plus one stall recovery before forcing a new epoch.

### `consensus/src/coordinator/mod.rs`

Applied hard deadline check at the top of `handle_phase_expiry()`:

```rust
if self.epoch_state.has_exceeded_hard_deadline(&self.config) {
    tracing::error!(epoch, phase = %current_phase, elapsed_ms = ...,
        "epoch exceeded hard deadline — forcing new epoch (consensus safety)");
    let _ = event_tx.send(CoordinatorEvent::EpochStalled { ... });
    self.consecutive_stalls += 1;
    self.start_epoch(epoch + 1, event_tx).await;
    return;
}
```

### `consensus/src/coordinator/commit.rs`

Fixed `add_commit()` to reject NaN and infinite work scores:

```rust
if !commit.work_score.is_finite() || commit.work_score <= 0.0 {
    return false;
}
```

Without this guard, a malicious or buggy node could inject a NaN score that causes non-deterministic sort/max behavior during winner selection, potentially causing an honest subset of nodes to select a different winner.

---

## Items assessed but deferred

**State root validation (item 8):** Full Merkle state root validation would require adding a `state_root: Hash` field to `BlockHeader`, which touches the wire format and 20+ files across multiple crates. Deferred to a dedicated phase to avoid scope creep and because the double-spend protection (`apply_block_atomically`) and nonce pre-validation (`validate_nonces`) already close the most critical correctness gaps it would address.

---

## Build verification

```
cargo check  → Finished (1 harmless warning: unused private f64 display helpers in difficulty.rs)
cargo build  → Finished dev profile in 55.88s
```
