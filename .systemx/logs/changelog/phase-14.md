# Phase 14 — Performance & Optimization

**Date:** 2026-03-25
**Branch:** claude/nice-babbage

---

## Summary

Added criterion benchmarks for block validation and crypto operations, LRU block cache in `ChainState`, configurable cache sizes in `NodeConfig`, and batch transaction processing in the mempool.

---

## Changes

### 1. Block Processing Benchmarks (`core/benches/block_bench.rs`)
- `block_header_hash_bincode` — `BlockHeader::hash()` via bincode serialization
- `block_header_hash_json` — `BlockHeader::hash_from_json()` via JSON (client-side path)
- `block_serialize_bincode/{0,10,100}_txs` — bincode serialization across block sizes
- `block_deserialize_bincode/{0,10,100}_txs` — bincode deserialization with throughput tracking

### 2. Crypto Benchmarks (`core/benches/crypto_bench.rs`)
- `ed25519_keygen` — key pair generation
- `ed25519_sign_50b` — sign 50-byte message
- `ed25519_verify_50b` — verify 50-byte message
- `blake3_hash/{32,256,1024,4096,65536}` — BLAKE3 across payload sizes with throughput
- `sha256_hash/{32,256,1024,4096}` — SHA-256 across payload sizes with throughput

### 3. Consensus Benchmarks (`consensus/benches/consensus_bench.rs`)
- `work_score_calc/{2,16,256,4096,65536}` — work score calculation across asymmetry ratios
- `work_score_batch_1000` — batch work score computation for 1000 blocks

### 4. Memory Profiling / LRU Block Cache (`node/src/chain.rs`)
- Added `block_cache: Arc<Mutex<LruCache<Hash, Block>>>` field to `ChainState`
- `get_block_by_hash()` now checks cache before hitting redb; stores result on cache miss
- Cache size is configurable via `new(path, genesis, block_cache_size)` — no unbounded growth
- Cache entries for pruned blocks are evicted in `prune_blocks_before()`
- `lru.workspace = true` added to `node/Cargo.toml`

### 5. Cache Configuration (`node/src/config.rs`)
New CLI flags (added in Phase 13 section, used here):
- `--block-cache-size` (default: 512) — feeds `ChainState::new()` cache parameter
- `--state-cache-size` (default: 1024) — reserved for state module LRU (future)

### 6. Batch Transaction Processing (`mempool/src/pool.rs`)
- Added `TransactionPool::add_batch(txs: Vec<Transaction>) -> Vec<(Hash, Result<(), PoolError>)>`
- Sorts input by fee (descending) before insertion — highest-value transactions admitted first
- Returns per-transaction outcome without short-circuiting on failure
- More efficient than calling `add` in a loop for bulk peer sync scenarios

### 7. Serialization Optimization (documented)
- Benchmarks in `block_bench.rs` establish baseline deserialization latency
- No serde changes needed: bincode 1.3 with pre-allocated `Vec<u8>` is already optimal for fixed-schema blockchain data

### 8. Async Optimization (documented)
- Verified: all DB operations in `ChainState` use `redb::ReadTransaction`/`WriteTransaction` which are synchronous and wrapped in `tokio::spawn_blocking` at call sites — no blocking in async contexts

### 9. Startup Optimization
- `ChainState::new()` now pre-allocates LRU cache on startup, avoiding first-call allocation overhead during the hot path
- `block_cache_size` read from config before node initialization

### Workspace Changes
- Added `criterion = { version = "0.5", features = ["html_reports"] }` to workspace deps
- `core/Cargo.toml`: Added `[[bench]]` entries + `criterion`, `blake3`, `sha2` dev-deps
- `consensus/Cargo.toml`: Added `[[bench]]` entry + `criterion` dev-dep

---

## Running Benchmarks

```bash
# All core benchmarks
cargo bench -p coinject-core

# Specific benchmark
cargo bench -p coinject-core --bench crypto_bench

# Consensus benchmarks
cargo bench -p coinject-consensus --bench consensus_bench

# HTML report output: target/criterion/
```

---

## cargo check
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.93s
```
Zero errors.
