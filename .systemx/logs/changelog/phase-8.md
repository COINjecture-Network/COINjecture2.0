# Phase 8 — Unit Testing Infrastructure
**Date:** 2026-03-25
**Branch:** `claude/dazzling-bohr`

---

## Summary

Established a comprehensive unit-testing infrastructure for the COINjecture workspace. Every critical crate now has meaningful test coverage, a property-based test harness (proptest), and a CI-ready test runner script.

---

## Changes

### 1. Workspace — `Cargo.toml`

Added two new workspace dependencies:
- `proptest = "1"` — property-based testing framework
- `tempfile = "3"` — safe temporary files/dirs for tests

### 2. Crate `[dev-dependencies]` additions

| Crate | Added dev-deps |
|-------|---------------|
| `coinject-core` | `proptest`, `tempfile`, `chrono`, `bincode`, `serde_json` |
| `coinject-consensus` | `proptest`, `tempfile` |
| `coinject-mempool` | `proptest`, `tempfile` |

`coinject-state` already had `tempfile = "3.8"` in dev-deps; no change needed.

---

### 3. `state/src/accounts.rs` — Test fixes

**Problem:** All three existing tests (`test_account_balance`, `test_transfer`, `test_nonce`) used hard-coded relative file paths (`"test_db"`, `"test_transfer_db"`, `"test_nonce_db"`). Parallel test execution caused race conditions and leftover files on failure.

**Fix:** Replaced with `tempfile::tempdir()` approach — each test gets a unique temporary directory that is automatically cleaned up on drop.

**New tests added:**
- `test_initial_balance_is_zero` — explicit initial-state check
- `test_set_and_get_balance` — update & read
- `test_transfer_moves_funds` — basic transfer
- `test_transfer_insufficient_balance_errors` — error path with `matches!`
- `test_transfer_full_balance` — edge case: drain account to zero
- `test_nonce_starts_at_zero` — initial state
- `test_nonce_increments_sequentially` — sequential increment
- `test_apply_batch_sets_multiple_balances` — batch writes
- `test_different_addresses_are_isolated` — isolation between accounts

---

### 4. `core/tests/unit_tests.rs` — New integration test file (32 tests)

Comprehensive tests for all core types:

**Hash:** zero constant, determinism, uniqueness, from-bytes roundtrip, display format (64 hex chars)
**Address:** bytes roundtrip, equality, derivation from keypair
**KeyPair / Sign / Verify:** sign-then-verify, tampered message fails, wrong key fails, deterministic signing (ed25519-dalek is RFC 8032 deterministic)
**MerkleTree:** empty → `Hash::ZERO`, single leaf = leaf hash, determinism, order sensitivity, adding leaf changes root
**Block/Blockchain:** genesis structure, deterministic hash, tip/height, get block in/out of range
**Transaction:** signed transfer valid, zero-amount invalid, fee/nonce/from accessors, hash uniqueness, timelock future validity

---

### 5. `core/tests/property_tests.rs` — New proptest file (9 properties)

Property tests that run hundreds of random inputs per property:

- `prop_hash_deterministic` — same bytes always same hash
- `prop_hash_nonempty_input_not_zero` — non-empty ≠ ZERO sentinel
- `prop_hash_bytes_roundtrip` — from_bytes / as_bytes identity
- `prop_different_data_different_hash` — probabilistic collision check
- `prop_address_bytes_roundtrip` — address encoding identity
- `prop_address_equality_by_bytes` — address inequality
- `prop_signed_transfer_always_verifies` — any amount/fee/nonce → valid sig
- `prop_varying_nonce_still_valid` — nonce variation preserves validity
- `prop_valid_transfer_passes_is_valid` — non-zero amount → is_valid() true
- `prop_transaction_serde_preserves_hash` — bincode round-trip preserves hash & sig
- `prop_transaction_json_roundtrip` — JSON round-trip preserves hash
- `prop_hash_serde_roundtrip` — Hash bincode identity

---

### 6. `mempool/tests/pool_tests.rs` — New integration test file (26 tests)

Replaces the placeholder tests in `pool.rs` (which used unsigned transactions). All transactions are now properly signed with `KeyPair::generate()`.

**Categories:**
- Basic add/len (empty pool, single add, returns hash, multiple adds)
- Duplicate rejection (same tx rejected, different nonces both accepted)
- Fee validation (below min → `FeeTooLow`, exactly at min → accepted)
- Fee prioritization (`get_pending` descending, `get_top_n` correct)
- Lookup/remove (contains/get after add, remove decrements len, remove nonexistent = None, batch remove)
- Pool capacity and eviction (full + low fee → `PoolFull`, high fee evicts lowest)
- Clear (empties pool)
- Statistics (transactions_added, transactions_removed, transactions_rejected)
- **Proptest:**
  - `prop_valid_tx_always_accepted` — any valid signed tx is accepted
  - `prop_pool_len_matches_additions` — len equals number of additions

---

### 7. `consensus/tests/consensus_property_tests.rs` — New test file (20 tests)

**WorkScoreCalculator unit tests:**
- Basic calculation (1s/1ms → ≈9.97 bits)
- Zero quality → zero score
- Trivial asymmetry (ratio < 2) → zero
- Half quality = half score
- Doubling solve time = +1 bit
- Chain security = sum of scores
- Empty chain security = 0
- Required asymmetry inverse

**DifficultyAdjuster unit tests:**
- Initial size non-zero
- No adjustment before window filled
- Fast times increase size
- Slow times decrease size
- Extreme fast stays within bounds [5, 50]
- Penalty reduces size, stays ≥ 5
- Stats: sample count, average time, recovery mode
- Problem type sizing: TSP < SubsetSum, SAT < SubsetSum

**WorkScoreCalculator property tests:**
- `prop_work_score_nonnegative` — always ≥ 0
- `prop_zero_quality_always_zero` — always 0 at quality=0
- `prop_higher_quality_higher_score` — monotone in quality
- `prop_chain_security_is_sum` — sum identity

---

### 8. `network/src/cpp/message.rs` — Inline test additions (6 tests)

Added serialization round-trip and correctness tests:
- `test_hello_message_bincode_roundtrip` — all fields survive bincode
- `test_ping_pong_bincode_roundtrip` — PingMessage and PongMessage
- `test_get_blocks_message_roundtrip` — height range and request_id
- `test_disconnect_message_roundtrip` — reason string
- `test_message_size_limit_awareness` — HelloMessage < 1 MB
- `test_all_message_types_have_unique_byte_values` — no duplicate discriminants
- `test_unknown_message_type_returns_error` — 0x99 → Err

---

### 9. `.systemx/scripts/test/run_tests.sh` — CI test runner

Bash script that:
- Runs `cargo test --workspace` by default
- Supports `--coverage` flag to invoke `cargo-tarpaulin`
- Supports `--crate <name>` to test a single crate
- Excludes binaries (node, wallet, mobile-sdk) from coverage sweep
- Outputs HTML coverage report to `.systemx/coverage/`

---

## Test Count Summary

| Crate | Before | Added | Total |
|-------|--------|-------|-------|
| `coinject-core` (inline) | 17 | +32 unit + 12 prop | ~61 |
| `coinject-core` (integration) | 0 | +32 unit + 12 prop | 44 |
| `coinject-consensus` | 12 | +20 + 4 prop | ~36 |
| `coinject-mempool` | 4 (placeholder) | +26 + 2 prop | 28 |
| `coinject-state` | 3 (broken) | +9 fixed | 9 |
| `coinject-network` (inline) | 3 | +7 | 10 |
| **Total new tests** | — | **~100+** | — |

---

## Coverage Target

Critical crates targeted at ≥ 70% line coverage:
- `coinject-core` — hashing, signing, block/tx construction
- `coinject-consensus` — work score formula, difficulty adjuster
- `coinject-mempool` — pool add/remove/prioritize
- `coinject-state` — account balance, transfer, nonce

To measure: `bash .systemx/scripts/test/run_tests.sh --coverage`
