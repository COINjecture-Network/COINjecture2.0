# COINjecture2.0 — Production Readiness Plan

> **Version:** 1.0.0
> **Created:** 2026-03-24
> **Author:** Security Audit & Architecture Review
> **Status:** Active
> **Repo:** `github.com/Quigles1337/COINjecture2.0` (v4.8.4)

---

## Executive Summary

This document defines a 20-phase production readiness plan for COINjecture2.0, a Rust-based blockchain platform built on a custom Conjecture Propagation Protocol (CPP) consensus mechanism. The plan addresses **9 critical**, **16 high**, and **18 medium** severity findings from the comprehensive security audit, spanning 200+ actionable tasks organized by priority and dependency.

The 13-crate workspace (`adzdb`, `core`, `consensus`, `network`, `state`, `mempool`, `rpc`, `tokenomics`, `node`, `wallet`, `marketplace-export`, `huggingface`, `mobile-sdk`) represents a well-architected foundation. This plan hardens it for production use.

### Priority Legend

| Priority | Meaning | SLA |
|----------|---------|-----|
| **P0** | Ship-blocking, immediate security risk | Fix before any deployment |
| **P1** | Critical for safe operation | Fix before testnet public access |
| **P2** | Required for production quality | Fix before mainnet |
| **P3** | Improvement, technical debt | Fix post-launch or iteratively |

### Dependency Graph (Simplified)

```
Phase 1 (Critical Security) ──┬──> Phase 2 (RPC Auth)
                               ├──> Phase 3 (Error Handling)
                               └──> Phase 4 (Consensus Safety)
Phase 2 ──> Phase 5 (Network Security) ──> Phase 6 (Input Validation)
Phase 3 ──> Phase 7 (Logging) ──> Phase 8 (Unit Tests)
Phase 8 ──> Phase 9 (Integration Tests) ──> Phase 10 (CI/CD)
Phase 5 ──> Phase 11 (Docker Security)
Phase 6 ──> Phase 12 (Web Wallet)
Phase 4 ──> Phase 13 (Database & State)
Phase 10 ──> Phase 14 (Performance)
Phase 12 ──> Phase 15 (API Docs)
Phase 4 ──> Phase 16 (Protocol Versioning)
Phase 16 ──> Phase 17 (Governance & Bridge)
Phase 14 ──> Phase 18 (Load Testing)
Phase 15 ──> Phase 19 (Documentation)
All ──> Phase 20 (Final Audit & Launch)
```

---

## Phase 1: Critical Security Fixes

**Priority:** P0
**Estimated Effort:** 2–3 weeks
**Dependencies:** None (start immediately)

### Description

Address the most dangerous vulnerabilities: plaintext private key storage, placeholder zero-knowledge proofs that accept all inputs, and hardcoded seeds/keys in source code. These issues could lead to total fund loss or consensus manipulation.

### Tasks

- [ ] **1.1** Implement encrypted keystore in `node/src/keystore.rs` — replace plaintext private key serialization with AES-256-GCM encryption using a passphrase-derived key (argon2id KDF). Store encrypted blobs in `~/.coinjecture/keystore/` with file permissions `0600`.
- [ ] **1.2** Implement encrypted keystore in `wallet/src/keystore.rs` — apply the same AES-256-GCM + argon2id pattern to the CLI wallet's key management. Ensure key material is zeroized on drop using the `zeroize` crate.
- [ ] **1.3** Audit and replace all placeholder ZK proof verification in `core/src/privacy.rs` — the current `verify()` function returns `true` for all inputs. Implement proper Groth16 or Bulletproofs verification, or gate the privacy feature behind a feature flag that disables it until real proofs are ready.
- [ ] **1.4** Remove all hardcoded seeds, keys, and mnemonics from source code — scan all `.rs` files for string literals that look like hex keys, base58 addresses, or seed phrases. Move test fixtures to `tests/fixtures/` with clear `#[cfg(test)]` guards.
- [ ] **1.5** Implement cryptographic commitment signing in `core/src/commitment.rs` — currently consensus commitments are unsigned. Add ed25519-dalek signatures to `Commitment` structs with validator identity binding.
- [ ] **1.6** Add the `zeroize` crate as a workspace dependency and apply `Zeroize` + `ZeroizeOnDrop` derives to all structs that hold private keys, seeds, or secret shares in `core/src/crypto.rs`, `node/src/keystore.rs`, and `wallet/src/keystore.rs`.
- [ ] **1.7** Implement secure key generation ceremony tooling — create a CLI subcommand (`coinjecture keygen`) that generates keys in a secure context, optionally air-gapped, with BIP39 mnemonic backup support.
- [ ] **1.8** Add a `cargo deny` configuration (`deny.toml`) to block crates with known CVEs. Add it to the CI pipeline in `.github/workflows/ci.yml`.
- [ ] **1.9** Run `cargo audit` and fix or pin all dependencies with known advisories. Document any accepted risks in `.systemx/security/audit-findings/dependency-audit.md`.
- [ ] **1.10** Implement secure memory handling for all cryptographic operations — ensure no private key material is written to logs, error messages, or debug output. Add `#[derive(Debug)]` overrides that redact sensitive fields.
- [ ] **1.11** Create a secret scanning pre-commit hook (`.systemx/ci-cd/hooks/secret-scan.sh`) that blocks commits containing patterns matching private keys, API tokens, or seed phrases.
- [ ] **1.12** Write unit tests for encrypted keystore round-trip (encrypt → store → load → decrypt) in both `node` and `wallet` crates. Minimum 95% branch coverage for keystore modules.

### Success Criteria

- No plaintext private keys anywhere in the codebase or on disk at runtime
- ZK proof verification correctly rejects invalid proofs (or feature is gated off)
- All consensus commitments are cryptographically signed and verified
- `cargo audit` passes with zero unaddressed advisories
- Secret scanning hook catches test key patterns in CI

---

## Phase 2: RPC Security & Authentication

**Priority:** P0
**Estimated Effort:** 1–2 weeks
**Dependencies:** Phase 1 (crypto primitives must be secure)

### Description

The RPC layer (`rpc/src/server.rs`, `rpc/src/websocket.rs`) currently has no authentication, no CORS policy, and no rate limiting. Any network-reachable attacker can call administrative endpoints.

### Tasks

- [ ] **2.1** Implement API key authentication middleware for `rpc/src/server.rs` — add a `Bearer` token check that validates against a hashed API key stored in node configuration (`node/src/config.rs`).
- [ ] **2.2** Implement JWT-based session authentication for the WebSocket RPC in `rpc/src/websocket.rs` — issue short-lived tokens (15 min) with refresh capability. Use `jsonwebtoken` crate with HS256 or EdDSA signing.
- [ ] **2.3** Add CORS configuration to the RPC server — restrict `Access-Control-Allow-Origin` to configured domains. Default to same-origin only. Make configurable via `node/src/config.rs`.
- [ ] **2.4** Implement role-based access control (RBAC) — separate RPC methods into `public` (chain_getInfo, chain_getBlock), `authenticated` (wallet operations), and `admin` (node management). Enforce roles in the jsonrpsee middleware layer.
- [ ] **2.5** Add per-IP rate limiting to the RPC server — use a token bucket algorithm with configurable limits (default: 100 req/s per IP for public, 20 req/s for authenticated). Store state in an LRU cache bounded to 10,000 entries.
- [ ] **2.6** Add per-method rate limiting — allow different rate limits per RPC method category. Administrative methods should be limited to 5 req/s.
- [ ] **2.7** Implement request size validation in the RPC layer — reject payloads exceeding 1MB (configurable). Currently the 10MB default message size enables DoS.
- [ ] **2.8** Add IP allowlisting/blocklisting for administrative RPC endpoints — configurable in `node/src/config.rs` with a default of localhost-only for admin methods.
- [ ] **2.9** Implement RPC request logging with caller identity — log method, params (redacted), caller IP, auth status, and response time to structured logs. Do not log sensitive parameters.
- [ ] **2.10** Write integration tests for all auth scenarios in `rpc/tests/` — unauthenticated access denied, valid token accepted, expired token rejected, rate limit triggered, CORS preflight handled.
- [ ] **2.11** Add CSRF token validation for any state-changing RPC calls originating from web contexts — generate and validate per-session CSRF tokens in the web wallet flow.
- [ ] **2.12** Document all RPC endpoints with auth requirements in `.systemx/docs/api/rpc-reference.md`.

### Success Criteria

- All administrative RPC endpoints require authentication
- Rate limiting prevents > 100 req/s from any single IP
- CORS headers are present and correctly restrictive
- WebSocket connections require valid JWT
- All auth paths have integration test coverage

---

## Phase 3: Error Handling Hardening

**Priority:** P0
**Estimated Effort:** 2 weeks
**Dependencies:** None (can start in parallel with Phase 1)

### Description

The codebase contains approximately 159 instances of `.unwrap()` and `.expect()` in non-test code. In a blockchain node, any panic in production causes a node crash, potentially enabling eclipse attacks or consensus stalls.

### Tasks

- [ ] **3.1** Audit and catalog all `.unwrap()` and `.expect()` calls in the `core` crate (`core/src/*.rs`) — replace with proper `Result` propagation using `?` operator or explicit error handling. Target: zero unwraps in `block.rs`, `transaction.rs`, `crypto.rs`, `commitment.rs`.
- [ ] **3.2** Audit and replace unwraps in `consensus/src/*.rs` — critical files: `miner.rs`, `difficulty.rs`, `work_score.rs`, `problem_registry.rs`. Consensus code must never panic.
- [ ] **3.3** Audit and replace unwraps in `network/src/lib.rs` and `network/src/reputation.rs` — network-facing code is the primary attack surface for triggering panics via malformed input.
- [ ] **3.4** Audit and replace unwraps in `node/src/*.rs` — high-priority files: `chain.rs`, `chain_adzdb.rs`, `validator.rs`, `peer_consensus.rs`, `genesis.rs`, `node_manager.rs`.
- [ ] **3.5** Audit and replace unwraps in `state/src/*.rs` — critical state management files: `accounts.rs`, `accounts_adzdb.rs`, `escrows.rs`, `channels.rs`, `trustlines.rs`.
- [ ] **3.6** Audit and replace unwraps in `mempool/src/*.rs` — files: `pool.rs`, `fee_market.rs`, `marketplace.rs`, `data_pricing.rs`, `mining_incentives.rs`.
- [ ] **3.7** Audit and replace unwraps in `rpc/src/server.rs` and `rpc/src/websocket.rs` — RPC handlers must return JSON-RPC error responses, never panic.
- [ ] **3.8** Audit and replace unwraps in `tokenomics/src/*.rs` — files: `amm.rs`, `emission.rs`, `staking.rs`, `rewards.rs`, `deflation.rs`, `distributor.rs`, `governance.rs`, `bounty_pricing.rs`.
- [ ] **3.9** Define a unified error type hierarchy for the workspace — create `core/src/error.rs` with domain-specific error enums (`CryptoError`, `ConsensusError`, `NetworkError`, `StateError`, `RpcError`). Use `thiserror` for all error types. Ensure all crates use consistent error patterns.
- [ ] **3.10** Add panic hooks in `node/src/main.rs` — install a custom `std::panic::set_hook` that logs the panic with full backtrace to structured logs and attempts graceful shutdown before exiting.
- [ ] **3.11** Configure `#![deny(clippy::unwrap_used, clippy::expect_used)]` as workspace-level Clippy lints in the root `Cargo.toml` to prevent future regression.
- [ ] **3.12** Add a CI step to enforce zero unwrap/expect outside of `#[cfg(test)]` blocks — create a script `.systemx/scripts/test/check-unwraps.sh` that greps for unwrap/expect and exits non-zero.

### Success Criteria

- Zero `.unwrap()` or `.expect()` calls in non-test production code
- All errors propagate through typed `Result<T, E>` with meaningful context
- Clippy lint enforcement prevents regression
- Panic hook provides structured crash reporting
- CI blocks PRs that introduce new unwrap calls

---

## Phase 4: Consensus Safety

**Priority:** P0
**Estimated Effort:** 2–3 weeks
**Dependencies:** Phase 1 (commitment signing), Phase 3 (error handling)

### Description

The consensus mechanism uses floating-point arithmetic in critical paths and lacks transaction replay protection. Floating-point non-determinism across platforms can cause consensus splits.

### Tasks

- [ ] **4.1** Identify all floating-point (`f32`/`f64`) usage in `consensus/src/difficulty.rs` — replace with fixed-point arithmetic using `num-rational` (already a workspace dependency) or a dedicated `FixedPoint` type in `core/src/types.rs`.
- [ ] **4.2** Replace floating-point in `consensus/src/work_score.rs` — work score calculations must be deterministic. Use `num-bigint` rational arithmetic for all division and scaling operations.
- [ ] **4.3** Replace floating-point in `tokenomics/src/emission.rs` and `tokenomics/src/rewards.rs` — emission curves and reward calculations affect token supply and must be bit-exact across all nodes.
- [ ] **4.4** Replace floating-point in `tokenomics/src/amm.rs` — AMM constant-product formula must use integer math. Implement checked arithmetic with overflow protection.
- [ ] **4.5** Replace floating-point in `mempool/src/fee_market.rs` and `mempool/src/data_pricing.rs` — fee calculations must be deterministic across all validators.
- [ ] **4.6** Implement transaction replay protection — add a `nonce` field to the `Transaction` struct in `core/src/transaction.rs`. Validate monotonically increasing nonces per-account in `state/src/accounts.rs` and `state/src/accounts_adzdb.rs`.
- [ ] **4.7** Implement chain ID binding in transactions — add a `chain_id` field to prevent replay across testnet/mainnet. Validate chain ID in `node/src/validator.rs`.
- [ ] **4.8** Add transaction expiry — implement a `valid_until_block` field in transactions. Reject transactions older than a configurable window (default: 256 blocks) in the mempool admission logic (`mempool/src/pool.rs`).
- [ ] **4.9** Implement deterministic block ordering — ensure transaction ordering within blocks is deterministic (e.g., by hash) in `node/src/chain.rs` so all nodes produce identical state roots.
- [ ] **4.10** Add consensus-critical unit tests — write property-based tests (using `proptest`) that verify: identical inputs produce identical outputs for difficulty, work score, and reward calculations across 10,000 random inputs.
- [ ] **4.11** Create a fixed-point arithmetic library module in `core/src/fixed_point.rs` — implement `Add`, `Sub`, `Mul`, `Div` traits with overflow checking. Provide conversion methods to/from human-readable decimal strings.
- [ ] **4.12** Implement checked arithmetic throughout the `tokenomics` crate — replace all uses of `+`, `-`, `*`, `/` with `.checked_add()`, `.checked_sub()`, etc. Return errors on overflow instead of wrapping.

### Success Criteria

- Zero floating-point operations in any consensus-critical or state-modifying code path
- All transactions include nonce, chain_id, and expiry
- Property-based tests confirm determinism across 10,000+ random inputs
- Checked arithmetic prevents integer overflow in all financial calculations
- Identical transaction sets produce identical state roots on all platforms

---

## Phase 5: Network Security

**Priority:** P0
**Estimated Effort:** 2–3 weeks
**Dependencies:** Phase 2 (RPC auth must be in place)

### Description

The CPP (Conjecture Propagation Protocol) network layer uses unencrypted TCP connections, has a 10MB message size limit enabling DoS, and lacks peer authentication. All network traffic is susceptible to eavesdropping and MITM attacks.

### Tasks

- [ ] **5.1** Implement TLS encryption for all peer-to-peer connections in `network/src/lib.rs` — use `rustls` with certificate pinning. Generate per-node TLS certificates derived from the node's ed25519 identity key.
- [ ] **5.2** Implement the Noise protocol (XX handshake pattern) as an alternative to TLS for P2P encryption — use `snow` crate. Noise is more suitable for P2P networks as it provides mutual authentication without a CA.
- [ ] **5.3** Reduce the maximum message size from 10MB to 1MB in the network codec — add configurable `max_message_size` to `node/src/config.rs`. Implement streaming for larger payloads (block sync) with chunked transfer.
- [ ] **5.4** Implement peer authentication — on connection, peers must prove ownership of their claimed identity by signing a challenge with their node key. Reject connections from unauthenticated peers.
- [ ] **5.5** Add connection rate limiting — limit inbound connections per IP to 5 concurrent, with a connection rate of 10/minute. Implement in the connection accept loop in `network/src/lib.rs`.
- [ ] **5.6** Implement peer reputation scoring in `network/src/reputation.rs` — track message validity, response times, and protocol compliance. Disconnect and temporarily ban peers below threshold.
- [ ] **5.7** Add message validation at the network layer — verify message checksums, reject malformed protocol messages, and validate message type before deserialization to prevent deser-based attacks.
- [ ] **5.8** Implement graceful connection handling — add proper TCP keepalive, connection timeout (30s), read timeout (10s), and write timeout (10s). Handle `BrokenPipe` and `ConnectionReset` without panicking.
- [ ] **5.9** Add network-level DoS protection — implement SYN cookie equivalent for the CPP handshake. Reject connections during high load based on available file descriptors.
- [ ] **5.10** Implement encrypted peer discovery — ensure the peer exchange protocol doesn't leak the full network topology. Use onion-style routing for peer advertisements.
- [ ] **5.11** Add network partition detection — implement a protocol that detects when a node is isolated from the majority of the network. Alert via metrics and logs.
- [ ] **5.12** Write E2E network security tests using the Docker testnet — test scenarios: MITM attempt, message replay, oversized message, peer impersonation. Add to `tests/harness/`.

### Success Criteria

- All P2P connections are encrypted (TLS or Noise)
- Peers mutually authenticate via cryptographic handshake
- Maximum message size ≤ 1MB (with streaming for larger payloads)
- DoS attacks from a single IP are mitigated by rate limiting
- E2E tests validate all security properties in Docker testnet

---

## Phase 6: Input Validation & Sanitization

**Priority:** P1
**Estimated Effort:** 1–2 weeks
**Dependencies:** Phase 2 (RPC layer), Phase 5 (network layer)

### Description

RPC endpoints and network message handlers lack input validation. Malformed inputs can trigger panics, memory exhaustion, or logic errors.

### Tasks

- [ ] **6.1** Implement input validation for all RPC endpoints in `rpc/src/server.rs` — validate parameter types, ranges, and lengths before processing. Reject with JSON-RPC error code -32602 (Invalid params).
- [ ] **6.2** Add transaction validation in `node/src/validator.rs` — validate all transaction fields: signature validity, nonce monotonicity, balance sufficiency, gas limits, address format, and amount non-negativity.
- [ ] **6.3** Implement block validation in `node/src/chain.rs` — validate block header fields: parent hash linkage, timestamp bounds (not in future, not too far in past), difficulty target, merkle root, and block size limits.
- [ ] **6.4** Add address format validation — create `core/src/address.rs` with `Address::from_str()` that validates checksum, length, and prefix. Reject malformed addresses at all entry points.
- [ ] **6.5** Implement amount validation — ensure all monetary amounts are non-negative, within supply bounds, and don't overflow when summed. Add validation in `core/src/transaction.rs` and `state/src/accounts.rs`.
- [ ] **6.6** Sanitize all string inputs in RPC handlers — prevent injection attacks by validating and escaping user-provided strings (block hashes, transaction IDs, addresses). Limit string lengths.
- [ ] **6.7** Implement deserialization limits — configure `serde` and `bincode` with maximum depth, maximum collection size, and maximum string length to prevent memory exhaustion during deserialization.
- [ ] **6.8** Add validation to the escrow system in `state/src/escrows.rs` — validate multi-sig thresholds (M ≤ N), participant uniqueness, timelock bounds, and escrow amounts.
- [ ] **6.9** Validate all configuration inputs in `node/src/config.rs` — reject invalid port numbers, non-existent paths, negative numeric values, and overly large cache sizes at node startup.
- [ ] **6.10** Add fuzz targets for critical input parsers — create fuzz targets for transaction deserialization, block deserialization, RPC parameter parsing, and network message parsing. Place in `.systemx/tests/fuzz/`.
- [ ] **6.11** Implement a validation middleware layer — create a reusable `Validator<T>` trait in `core/src/validation.rs` that all input-accepting functions use. This ensures consistent validation across all crates.
- [ ] **6.12** Validate marketplace and data pricing inputs in `mempool/src/marketplace.rs` and `mempool/src/data_pricing.rs` — prevent negative prices, zero-division, and timestamp manipulation.

### Success Criteria

- All RPC endpoints validate inputs before processing
- No transaction or block accepted without full field validation
- Deserialization cannot exhaust memory regardless of input
- Fuzz testing finds zero crashes after 10M iterations
- Validation is consistent and centralized via shared traits

---

## Phase 7: Structured Logging & Observability

**Priority:** P1
**Estimated Effort:** 1–2 weeks
**Dependencies:** Phase 3 (error types must be defined)

### Description

The codebase uses `tracing` but lacks structured logging standards, has no metrics collection, no health check endpoints, and no monitoring integration. Production nodes need observability.

### Tasks

- [ ] **7.1** Define a structured logging standard — create `.systemx/docs/guides/logging-standard.md` specifying required fields for each log level: `target`, `span`, `message`, `error_code`, `peer_id`, `block_height`, `tx_hash` where applicable.
- [ ] **7.2** Implement structured logging across the `node` crate — replace all `println!`, `eprintln!`, and ad-hoc `tracing::info!` with structured events using typed fields. Cover `main.rs`, `chain.rs`, `validator.rs`, `node_manager.rs`.
- [ ] **7.3** Implement structured logging in `network/src/lib.rs` — log connection events, message types, peer identity, bytes transferred, and errors with structured fields.
- [ ] **7.4** Implement structured logging in `consensus/src/miner.rs` — log mining attempts, difficulty adjustments, block proposals, and solution validations.
- [ ] **7.5** Implement structured logging in `rpc/src/server.rs` — log request method, caller IP, response time, status code, and error details (without sensitive parameters).
- [ ] **7.6** Expand the Prometheus metrics in `node/src/metrics.rs` — add gauges and counters for: blocks_processed, transactions_validated, peers_connected, mempool_size, chain_height, rpc_requests_total, rpc_request_duration, consensus_rounds, cache_hits/misses.
- [ ] **7.7** Implement the health check endpoint — enhance the existing `/health` endpoint to return structured JSON: `{ "status": "healthy", "chain_height": N, "peers": N, "syncing": bool, "uptime_seconds": N }`.
- [ ] **7.8** Add a readiness probe endpoint at `/ready` — return 200 only when the node has completed initial sync and is ready to accept transactions.
- [ ] **7.9** Implement log rotation and retention — configure `tracing-subscriber` with `tracing-appender` for file-based logging with daily rotation and 30-day retention. Make configurable via `node/src/config.rs`.
- [ ] **7.10** Create a Grafana dashboard template — define a JSON dashboard in `.systemx/docs/guides/grafana-dashboard.json` that visualizes all Prometheus metrics. Include alerts for: node down, sync stalled, mempool full, peer count low.
- [ ] **7.11** Add distributed tracing support — implement `trace_id` propagation across RPC calls, network messages, and consensus rounds using `tracing` spans. This enables end-to-end request tracing.
- [ ] **7.12** Implement sensitive data redaction in logs — create a `Redactable` trait that auto-redacts private keys, seeds, and full transaction details in log output. Apply to all logging points.

### Success Criteria

- All log events are structured JSON with consistent fields
- Prometheus metrics cover all critical subsystems
- Health and readiness endpoints return accurate status
- Grafana dashboard provides single-pane-of-glass monitoring
- No sensitive data appears in any log output

---

## Phase 8: Testing Infrastructure (Unit Test Framework)

**Priority:** P1
**Estimated Effort:** 2 weeks
**Dependencies:** Phase 3 (error handling), Phase 7 (logging for test diagnostics)

### Description

The codebase needs a comprehensive unit test suite. While some tests exist (`network/tests/`, `rpc/tests/`, `state/tests/`), many crates have zero or minimal test coverage.

### Tasks

- [ ] **8.1** Set up code coverage infrastructure — add `cargo-tarpaulin` or `cargo-llvm-cov` to the CI pipeline. Set minimum coverage threshold at 60% (to increase over time).
- [ ] **8.2** Write unit tests for `core/src/crypto.rs` — test key generation, signing, verification, hash functions, and edge cases (empty input, max-length input, invalid signatures). Target: 95% coverage.
- [ ] **8.3** Write unit tests for `core/src/transaction.rs` — test transaction creation, serialization round-trip, signature verification, field validation, and malformed transaction rejection.
- [ ] **8.4** Write unit tests for `core/src/block.rs` — test block creation, header validation, merkle root computation, serialization, and parent hash chaining.
- [ ] **8.5** Write unit tests for `consensus/src/difficulty.rs` and `consensus/src/work_score.rs` — test difficulty adjustment algorithm, boundary conditions, and determinism across inputs.
- [ ] **8.6** Write unit tests for `state/src/accounts.rs` and `state/src/accounts_adzdb.rs` — test account creation, balance updates, nonce management, and concurrent access patterns.
- [ ] **8.7** Write unit tests for `state/src/escrows.rs` — test escrow creation, multi-sig validation (M-of-N), timelock expiry, release conditions, and edge cases (M > N, duplicate signers).
- [ ] **8.8** Write unit tests for `mempool/src/pool.rs` — test transaction admission, eviction, ordering by fee, size limits, duplicate rejection, and nonce gap handling.
- [ ] **8.9** Write unit tests for `tokenomics/src/emission.rs` and `tokenomics/src/rewards.rs` — test emission curve correctness, reward distribution, halving events, and supply cap enforcement.
- [ ] **8.10** Write unit tests for `rpc/src/server.rs` — test all RPC method handlers with valid inputs, invalid inputs, auth scenarios, and error responses.
- [ ] **8.11** Create test utilities crate or module — build shared test helpers: mock keypairs, test transactions, genesis block builder, in-memory state. Place in `core/src/test_utils.rs` (behind `#[cfg(test)]`).
- [ ] **8.12** Add property-based tests using `proptest` — write generators for `Transaction`, `Block`, `Commitment` and verify invariants: serialization round-trip, deterministic hashing, valid signatures verify.

### Success Criteria

- All 13 crates have unit tests
- Code coverage ≥ 60% overall, ≥ 90% for `core` and `consensus`
- Property-based tests run 10,000+ iterations in CI
- Test utilities enable rapid test development
- CI blocks PRs that reduce coverage

---

## Phase 9: Integration Testing Suite

**Priority:** P1
**Estimated Effort:** 2–3 weeks
**Dependencies:** Phase 8 (unit test framework), Phase 5 (network security)

### Description

Integration tests verify cross-crate interactions and end-to-end flows. The existing tests in `network/tests/` and `tests/harness/` provide a foundation to expand.

### Tasks

- [ ] **9.1** Implement transaction lifecycle integration test — test the full flow: wallet creates tx → submits to RPC → mempool accepts → miner includes in block → state updates → balance changes confirmed. Place in `tests/integration/`.
- [ ] **9.2** Implement block propagation integration test — test: node A mines block → propagates via CPP → node B validates and accepts → both nodes have identical state root.
- [ ] **9.3** Implement consensus agreement test — expand `network/tests/chain_agreement.rs` to verify that N nodes reach consensus on conflicting transactions (double-spend resolution).
- [ ] **9.4** Implement mempool synchronization test — verify that transactions submitted to node A appear in node B's mempool within a bounded time.
- [ ] **9.5** Implement chain reorganization test — submit a longer valid chain to a node and verify it correctly reorganizes, updating state to reflect the new canonical chain.
- [ ] **9.6** Implement escrow integration test — test the full escrow lifecycle: creation → funding → multi-sig approval → release (or timeout → refund). Verify state changes at each step.
- [ ] **9.7** Implement tokenomics integration test — verify emission schedule produces correct supply over 1000 simulated blocks. Verify staking rewards and deflation mechanisms interact correctly.
- [ ] **9.8** Implement light client sync test — expand `node/src/light_client.rs` and `node/src/light_sync.rs` testing: light client connects → syncs headers → verifies proof of inclusion for a specific transaction.
- [ ] **9.9** Implement peer discovery and reputation test — verify new peers are discovered, reputation scores update based on behavior, and malicious peers are banned.
- [ ] **9.10** Implement Docker testnet scenario tests — expand `tests/harness/run-scenarios.sh` with scripted scenarios: network partition, node restart, Byzantine behavior simulation.
- [ ] **9.11** Implement WebSocket RPC integration test — expand `rpc/tests/websocket_rpc_tests.rs` to test subscription lifecycle: connect → subscribe to new blocks → receive notifications → unsubscribe → disconnect.
- [ ] **9.12** Create a test harness library — build a reusable harness that spins up N nodes in-process with configurable network conditions (latency, packet loss). Place in `tests/harness/lib.rs`.

### Success Criteria

- All critical cross-crate flows have integration test coverage
- Docker testnet tests run automatically in CI
- Chain reorg, double-spend, and Byzantine scenarios are tested
- Test harness enables rapid creation of new scenarios
- All integration tests pass reliably (no flaky tests)

---

## Phase 10: CI/CD Pipeline Hardening

**Priority:** P1
**Estimated Effort:** 1–2 weeks
**Dependencies:** Phase 8 (tests), Phase 9 (integration tests)

### Description

The current CI pipeline (`.github/workflows/ci.yml`) runs `cargo build` and `cargo test` but is missing formatting checks, Clippy lints, security audits, coverage reporting, and artifact management.

### Tasks

- [ ] **10.1** Enable `cargo fmt --all -- --check` in CI — uncomment the existing TODO in `.github/workflows/ci.yml`. First run `cargo fmt --all` locally to fix all formatting issues.
- [ ] **10.2** Add `cargo clippy --all -- -D warnings` to CI — fix all existing Clippy warnings first. Include the workspace-level `unwrap_used` and `expect_used` lints.
- [ ] **10.3** Add `cargo audit` step to CI — run `cargo audit` and fail the build on any advisory with severity ≥ medium. Allow ignored advisories only with documented justification.
- [ ] **10.4** Add `cargo deny check` to CI — enforce license compliance, ban problematic crates, and detect duplicate dependencies using the `deny.toml` created in Phase 1.
- [ ] **10.5** Add code coverage reporting to CI — run `cargo tarpaulin` or `cargo-llvm-cov` and upload results. Fail if coverage drops below threshold. Add coverage badge to `README.md`.
- [ ] **10.6** Add the unwrap-checking script from Phase 3 (`.systemx/scripts/test/check-unwraps.sh`) as a CI step — ensure zero unwrap/expect in non-test code.
- [ ] **10.7** Implement build caching optimization — improve the cargo cache key in CI to include Rust toolchain version. Add `sccache` for faster incremental builds.
- [ ] **10.8** Add release build and binary artifact publishing — on tagged releases, build optimized binaries for Linux x86_64, create checksums, and upload as GitHub release assets.
- [ ] **10.9** Add Docker image CI — build and push Docker images to a registry on main branch merges. Tag with commit SHA and `latest`. Scan images with `trivy` for vulnerabilities.
- [ ] **10.10** Implement PR checks — require all CI checks to pass before merge. Add branch protection rules for `main`. Require at least 1 approval.
- [ ] **10.11** Add a nightly CI job — run extended tests (fuzz targets for 1 hour, load tests, full Docker testnet scenarios) on a nightly schedule.
- [ ] **10.12** Implement CI notifications — send build status to a configured webhook (Slack, Discord) on failure. Include link to failing step and logs.

### Success Criteria

- CI runs fmt, clippy, audit, deny, tests, coverage, and Docker smoke tests
- Build failures block PR merges
- Code coverage is tracked and enforced
- Release binaries are automatically built and published
- Nightly extended testing catches intermittent issues

---

## Phase 11: Docker & Deployment Security

**Priority:** P1
**Estimated Effort:** 1–2 weeks
**Dependencies:** Phase 5 (network security), Phase 10 (CI/CD)

### Description

The Docker configuration runs containers as root, lacks resource limits, and doesn't follow security best practices. The deployment scripts need hardening.

### Tasks

- [ ] **11.1** Create a non-root user in the `Dockerfile` — add `RUN adduser --disabled-password --gecos '' coinjecture` and `USER coinjecture`. Ensure data directories are owned by this user.
- [ ] **11.2** Implement multi-stage Docker build — use a `rust:slim` builder stage and a `debian:bookworm-slim` or `distroless` runtime stage. This reduces image size and attack surface.
- [ ] **11.3** Add resource limits to `docker-compose.yml` — set `mem_limit`, `cpus`, `pids_limit`, and `read_only: true` for each node container. Add `tmpfs` for writable directories.
- [ ] **11.4** Implement Docker health checks — add `HEALTHCHECK` directive to `Dockerfile` that calls the `/health` endpoint. Configure interval, timeout, and retries.
- [ ] **11.5** Add security-focused Docker labels and scanning — add `LABEL` directives for maintainer and version. Run `trivy` scan in CI and fail on critical/high vulnerabilities.
- [ ] **11.6** Implement secrets management for Docker — use Docker secrets or environment variable injection (not hardcoded in docker-compose) for node keys, API tokens, and TLS certificates.
- [ ] **11.7** Secure the Docker network — create an internal-only Docker network for inter-node communication. Expose only RPC ports to the host. Add network policies.
- [ ] **11.8** Implement container logging — configure Docker logging driver to output structured JSON. Integrate with the node's structured logging from Phase 7.
- [ ] **11.9** Harden the `scripts/build-docker.sh` and `scripts/deployment/deploy-gcloud-run.sh` — add error handling, input validation, confirmation prompts, and rollback capability.
- [ ] **11.10** Create a Docker Compose profile for production — separate from the testnet config. Include TLS termination, persistent volumes, backup mounts, and monitoring sidecar.
- [ ] **11.11** Implement container orchestration templates — create Kubernetes manifests (or Helm chart) in `.systemx/scripts/deploy/` for production deployment with proper RBAC, network policies, and pod security standards.
- [ ] **11.12** Add a Docker-based local development environment — create `docker-compose.dev.yml` that mounts source code, enables hot-reload via `cargo watch`, and includes a local block explorer.

### Success Criteria

- All containers run as non-root
- Docker images are minimal (< 100MB) with multi-stage builds
- Resource limits prevent container escape and resource exhaustion
- Health checks enable automatic container restart
- Production Docker Compose profile is deployment-ready

---

## Phase 12: Web Wallet Security & UX

**Priority:** P1
**Estimated Effort:** 2 weeks
**Dependencies:** Phase 2 (RPC auth), Phase 6 (input validation)

### Description

The web wallet (`web-wallet/index.html`, `web-wallet/dist/index.html`) handles private keys in the browser, lacks CSRF protection, and has no Content Security Policy. The evolved web frontend (`web/coinjecture-evolved-main/`) needs similar hardening.

### Tasks

- [ ] **12.1** Implement Content Security Policy (CSP) headers — add strict CSP meta tags to `web-wallet/index.html`: `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'self' https://rpc.coinjecture.io`.
- [ ] **12.2** Implement CSRF token flow — generate CSRF tokens server-side in the RPC layer, include them in the wallet HTML, and validate on all state-changing RPC requests.
- [ ] **12.3** Implement client-side key encryption — encrypt private keys in the browser using WebCrypto API (AES-GCM with PBKDF2-derived key). Never store plaintext keys in localStorage or sessionStorage.
- [ ] **12.4** Add Subresource Integrity (SRI) hashes — for all externally loaded scripts (if any), add `integrity` attributes. For `@noble` crypto libraries, pin specific versions with SRI.
- [ ] **12.5** Implement secure key generation in the browser — use `window.crypto.getRandomValues()` exclusively. Add entropy quality checks. Display mnemonic backup during generation.
- [ ] **12.6** Add transaction confirmation UI — before any transaction, show a confirmation dialog with: recipient address, amount, fee, and total cost. Require explicit user approval.
- [ ] **12.7** Implement session timeout — auto-lock the wallet after 5 minutes of inactivity. Require password re-entry to unlock. Clear sensitive data from memory on lock.
- [ ] **12.8** Add input validation on all form fields — validate addresses (format, checksum), amounts (positive, within balance, decimal precision), and prevent XSS in user-supplied strings.
- [ ] **12.9** Implement network indicator — show a clear indicator of which network (testnet/mainnet) the wallet is connected to. Use distinct visual themes to prevent testnet/mainnet confusion.
- [ ] **12.10** Add error handling and user feedback — replace `alert()` calls with proper toast notifications. Show meaningful error messages for RPC failures, network issues, and validation errors.
- [ ] **12.11** Implement secure communication between wallet and RPC — ensure all wallet-to-RPC communication uses HTTPS (leverage `scripts/setup-https-*.sh`). Validate TLS certificates.
- [ ] **12.12** Add automated web wallet testing — create Playwright or Cypress tests for the web wallet covering: key generation, transaction flow, error states, session timeout, and CSP compliance.

### Success Criteria

- CSP headers prevent XSS and injection attacks
- CSRF tokens protect all state-changing operations
- Private keys are never stored in plaintext in the browser
- Transaction confirmation prevents accidental sends
- Automated tests cover all critical wallet flows

---

## Phase 13: Database & State Management

**Priority:** P2
**Estimated Effort:** 2 weeks
**Dependencies:** Phase 4 (consensus safety), Phase 3 (error handling)

### Description

The state layer uses `redb` (ACID-compliant) but lacks migration strategy, state pruning, backup mechanisms, and bounded growth controls. The `adzdb` crate needs integration hardening.

### Tasks

- [ ] **13.1** Implement a database migration framework — create a versioned migration system in `state/src/migrations.rs` that tracks schema version and applies forward migrations on node startup.
- [ ] **13.2** Implement state pruning — add configurable state pruning in `state/src/lib.rs` that removes data older than N blocks (default: 10,000). Keep all account balances but prune intermediate state.
- [ ] **13.3** Implement database backup and recovery — create `node/src/backup.rs` with hot backup capability (using redb's snapshot feature). Schedule automatic backups and verify integrity on restore.
- [ ] **13.4** Bound all in-memory caches — audit all `LruCache` and `HashMap` usage across all crates. Ensure every cache has a configurable maximum size. Add cache eviction metrics.
- [ ] **13.5** Implement state snapshot and sync — allow new nodes to bootstrap from a state snapshot instead of replaying the entire chain. Implement snapshot creation and verification.
- [ ] **13.6** Add database integrity checks — implement a `verify-db` CLI subcommand that walks the state trie, verifies merkle proofs, and checks account balance consistency.
- [ ] **13.7** Implement atomic state transitions in `state/src/accounts_adzdb.rs` — ensure that multi-account updates (e.g., transfers) are atomic. If any part fails, the entire transaction rolls back.
- [ ] **13.8** Add state metrics — track database size, read/write latency, compaction events, and cache hit rates. Export via Prometheus metrics from Phase 7.
- [ ] **13.9** Implement state diff logging — log state changes per block (account deltas, new escrows, channel updates) for debugging and auditing.
- [ ] **13.10** Optimize `adzdb/src/lib.rs` — profile database operations and optimize hot paths: batch writes, read-ahead for sequential access, and index optimization.
- [ ] **13.11** Implement state export/import — create tools to export state to JSON or Parquet for analytics. Import capability for test data seeding.
- [ ] **13.12** Add database stress tests — write tests that perform 100,000+ read/write operations concurrently, verifying ACID properties hold under load. Place in `state/tests/`.

### Success Criteria

- Migration framework handles schema changes without data loss
- State pruning keeps database size bounded
- Backup and recovery is tested and documented
- All caches have bounded sizes with eviction policies
- Database passes stress tests with 100K+ concurrent operations

---

## Phase 14: Performance & Optimization

**Priority:** P2
**Estimated Effort:** 2–3 weeks
**Dependencies:** Phase 10 (CI/CD for benchmarks), Phase 13 (database optimization)

### Description

The codebase lacks benchmarks, has memory-unbounded data structures, and hasn't been profiled. Before mainnet, critical paths need performance validation.

### Tasks

- [ ] **14.1** Set up `criterion` benchmarking framework — add benchmark harness to workspace. Create initial benchmarks for: block validation, transaction signing/verification, state root computation.
- [ ] **14.2** Benchmark transaction throughput — measure max transactions per second for block creation, validation, and state application. Establish baseline and target (e.g., 1000 TPS).
- [ ] **14.3** Benchmark block propagation latency — measure time from block creation on node A to validation on node B in the Docker testnet. Target: < 500ms for 4-node network.
- [ ] **14.4** Profile memory usage — use `dhat` or `jemalloc` profiling to identify memory hotspots. Focus on: mempool growth, peer connection buffers, state cache, and block processing.
- [ ] **14.5** Optimize transaction serialization — benchmark `bincode` vs `borsh` vs `postcard` for transaction serialization. Choose the fastest option that maintains determinism.
- [ ] **14.6** Implement connection pooling for RPC — reuse database connections and reduce allocation overhead in the RPC hot path (`rpc/src/server.rs`).
- [ ] **14.7** Optimize consensus critical path — profile `consensus/src/miner.rs` and `consensus/src/difficulty.rs`. Identify and optimize hot loops, unnecessary allocations, and redundant computations.
- [ ] **14.8** Implement parallel block validation — validate transactions within a block in parallel where dependencies allow. Use `rayon` for CPU-bound work.
- [ ] **14.9** Optimize network message handling — batch small messages, implement message compression (LZ4) for large payloads, and minimize allocations in the message codec.
- [ ] **14.10** Add performance regression CI — run `criterion` benchmarks in CI and fail if any benchmark regresses by more than 10%. Track performance over time.
- [ ] **14.11** Implement memory pool optimization — optimize `mempool/src/pool.rs` with an efficient data structure for fee-ordered insertion and eviction (e.g., `BTreeMap` with secondary index).
- [ ] **14.12** Profile and optimize the `tokenomics` crate — benchmark `amm.rs` calculations, reward distribution, and emission computation. Optimize big-integer arithmetic hot paths.

### Success Criteria

- Baseline benchmarks established for all critical paths
- Transaction throughput meets target (≥ 1000 TPS)
- Block propagation latency < 500ms in 4-node testnet
- Memory usage profiled and bounded
- Performance regression detection in CI

---

## Phase 15: API Documentation & Standards

**Priority:** P2
**Estimated Effort:** 1–2 weeks
**Dependencies:** Phase 2 (RPC endpoints finalized), Phase 12 (web wallet API)

### Description

There is no API documentation. Developers and wallet integrators need comprehensive, versioned API references.

### Tasks

- [ ] **15.1** Document all JSON-RPC methods — create `.systemx/docs/api/rpc-reference.md` listing every RPC method with: name, description, parameters (with types), return value, errors, authentication requirement, and example request/response.
- [ ] **15.2** Document the WebSocket subscription API — create `.systemx/docs/api/websocket-reference.md` covering: connection flow, available subscriptions (new blocks, pending transactions, account updates), message format, and reconnection strategy.
- [ ] **15.3** Create an OpenRPC specification — generate a machine-readable `openrpc.json` that describes all RPC methods. Enable auto-generation of client libraries.
- [ ] **15.4** Document the CPP network protocol — create `.systemx/docs/architecture/cpp-protocol.md` describing: message types, handshake flow, equilibrium-based flow control, peer discovery, and wire format.
- [ ] **15.5** Document the transaction format — create `.systemx/docs/api/transaction-format.md` specifying: field layout, serialization format, signing procedure, and fee calculation.
- [ ] **15.6** Document the block format — create `.systemx/docs/api/block-format.md` specifying: header fields, body structure, merkle tree construction, and block hash computation.
- [ ] **15.7** Create integration examples — write example scripts in `.systemx/docs/guides/` showing: connect to node, query balance, send transaction, subscribe to events. Include Rust, Python, and JavaScript examples.
- [ ] **15.8** Document error codes — create `.systemx/docs/api/error-codes.md` listing all error codes with: numeric code, description, likely cause, and suggested resolution.
- [ ] **15.9** Document the tokenomics model — create `.systemx/docs/architecture/tokenomics.md` describing: emission schedule, reward distribution, staking mechanics, AMM formulas, and deflation mechanisms.
- [ ] **15.10** Add inline Rustdoc comments to all public APIs — run `cargo doc --no-deps` and verify all public functions, structs, and enums have documentation. Add `#![warn(missing_docs)]` to all crate roots.
- [ ] **15.11** Create a developer quickstart guide — write `.systemx/docs/guides/quickstart.md` covering: install Rust, clone repo, build, run testnet, send first transaction. Target: working in 15 minutes.
- [ ] **15.12** Implement API versioning strategy — document the API versioning plan in `.systemx/docs/decisions/api-versioning.md`. Define how breaking changes are introduced and communicated.

### Success Criteria

- All RPC methods documented with examples
- OpenRPC specification enables auto-generated clients
- CPP protocol fully documented for third-party implementations
- Rustdoc generates comprehensive API documentation
- Developer quickstart enables onboarding in < 15 minutes

---

## Phase 16: Protocol Versioning & Migration

**Priority:** P2
**Estimated Effort:** 2 weeks
**Dependencies:** Phase 4 (consensus safety), Phase 9 (integration tests)

### Description

The protocol has no versioning scheme. Without versioning, protocol upgrades require hard forks and can split the network.

### Tasks

- [ ] **16.1** Define a protocol version scheme — implement `ProtocolVersion { major, minor, patch }` in `core/src/types.rs`. Major = consensus-breaking, Minor = backward-compatible feature, Patch = bug fix.
- [ ] **16.2** Add protocol version to block headers — extend the `Block` struct in `core/src/block.rs` with a `protocol_version` field. Validators reject blocks with unsupported versions.
- [ ] **16.3** Add protocol version to network handshake — include version negotiation in the CPP handshake (`network/src/lib.rs`). Peers agree on the highest mutually supported version.
- [ ] **16.4** Implement feature flags — create `core/src/features.rs` with feature activation by block height. This enables soft forks where new rules activate at a predetermined height.
- [ ] **16.5** Implement protocol upgrade signaling — allow validators to signal readiness for new protocol versions in their block proposals. Upgrade activates when ≥ 75% signal readiness.
- [ ] **16.6** Create a migration framework — build `core/src/migration.rs` with per-version migration functions that transform state when a protocol upgrade activates.
- [ ] **16.7** Implement backward compatibility layer — ensure nodes running version N can communicate with nodes running version N-1 for at least 1000 blocks after upgrade activation.
- [ ] **16.8** Add protocol version to RPC responses — include the node's protocol version in `chain_getInfo` and other informational endpoints.
- [ ] **16.9** Create a protocol upgrade testing framework — build tools that simulate protocol upgrades in the Docker testnet: pre-upgrade state → upgrade signal → activation → post-upgrade validation.
- [ ] **16.10** Document the protocol upgrade process — write `.systemx/docs/guides/protocol-upgrade.md` covering: proposal → signaling → activation → rollback plan.
- [ ] **16.11** Implement version-aware serialization — ensure serialization format includes version tags so old data can be deserialized by new code and vice versa.
- [ ] **16.12** Add protocol version metrics — track the distribution of protocol versions across connected peers. Alert when the network is split across versions.

### Success Criteria

- All blocks, messages, and data include protocol version
- Feature flags enable soft-fork upgrades by block height
- Nodes of adjacent versions can interoperate for 1000 blocks
- Upgrade simulation passes in Docker testnet
- Protocol upgrade process is documented and tested

---

## Phase 17: Governance & Bridge Crate Completion

**Priority:** P2
**Estimated Effort:** 3–4 weeks
**Dependencies:** Phase 16 (protocol versioning for governance proposals)

### Description

The `tokenomics/src/governance.rs` module is stubbed out, and there is no bridge crate for cross-chain interoperability. These features are on the roadmap and need implementation.

### Tasks

- [ ] **17.1** Design the governance model — create `.systemx/docs/decisions/governance-model.md` specifying: proposal types (parameter change, protocol upgrade, treasury spend), voting mechanics (token-weighted), quorum requirements, and timelock periods.
- [ ] **17.2** Implement governance proposal submission in `tokenomics/src/governance.rs` — allow stakers to submit proposals with a deposit. Proposals include: description, proposed changes, voting period, and execution delay.
- [ ] **17.3** Implement governance voting — allow token holders to vote yes/no/abstain on active proposals. Votes are weighted by staked balance. Implement delegation.
- [ ] **17.4** Implement proposal execution — when a proposal passes (quorum met + majority yes), queue it for execution after a timelock. Implement automatic execution at the activation block.
- [ ] **17.5** Implement treasury management — create a treasury account funded by a percentage of block rewards and transaction fees. Treasury spends require governance approval.
- [ ] **17.6** Design the bridge architecture — create `.systemx/docs/architecture/bridge-design.md` specifying: supported chains, lock-and-mint vs burn-and-release, validator set for bridge attestations, and security model.
- [ ] **17.7** Implement bridge deposit detection — create `bridge/src/deposit.rs` (new crate) that monitors external chains for deposits to bridge contracts. Use light client verification where possible.
- [ ] **17.8** Implement bridge minting — when a deposit is confirmed on the source chain, mint wrapped tokens on COINjecture. Require M-of-N bridge validator attestations.
- [ ] **17.9** Implement bridge withdrawal — allow users to burn wrapped tokens to trigger a withdrawal on the source chain. Implement withdrawal proof generation.
- [ ] **17.10** Add governance integration tests — test the full proposal lifecycle: submission → voting → execution → effect. Test edge cases: quorum not met, proposal expired, insufficient deposit.
- [ ] **17.11** Add bridge integration tests — test deposit → mint → transfer → burn → withdrawal flow. Use mock external chains for testing.
- [ ] **17.12** Implement governance and bridge metrics — track active proposals, vote participation, bridge volume, attestation latency, and bridge validator health.

### Success Criteria

- Governance system enables decentralized protocol management
- Proposals can modify protocol parameters without hard forks
- Bridge architecture is designed and documented
- Bridge MVP supports at least one external chain
- Full integration test coverage for both governance and bridge

---

## Phase 18: Load Testing & Stress Testing

**Priority:** P2
**Estimated Effort:** 2 weeks
**Dependencies:** Phase 14 (performance baselines), Phase 9 (test harness)

### Description

The system has never been load tested. Before mainnet, we need to understand breaking points, resource limits, and degradation behavior under load.

### Tasks

- [ ] **18.1** Create a transaction generator tool — build a CLI tool in `.systemx/scripts/test/tx-generator.rs` that generates and submits N transactions per second to a target node. Support configurable: TPS, duration, transaction types, and account distribution.
- [ ] **18.2** Run sustained load test — submit transactions at increasing rates (100, 500, 1000, 2000 TPS) for 1 hour each. Record: block production rate, mempool size, latency, error rate, and resource usage.
- [ ] **18.3** Run spike load test — submit 10x normal load for 60 seconds, then return to normal. Verify the system recovers gracefully without lost transactions or state corruption.
- [ ] **18.4** Run soak test — run the 4-node Docker testnet under moderate load (500 TPS) for 24 hours. Monitor for: memory leaks, disk growth, peer disconnections, and consensus failures.
- [ ] **18.5** Run network partition test — using Docker network manipulation, simulate a 50/50 network split for 10 minutes, then heal. Verify consensus reconciliation and no double-spends.
- [ ] **18.6** Run node crash and recovery test — kill a node process during block production. Restart and verify: database integrity, chain sync from peers, and state consistency.
- [ ] **18.7** Run Byzantine node test — deploy a modified node that sends conflicting blocks/votes. Verify the honest majority rejects Byzantine behavior and the network continues.
- [ ] **18.8** Run mempool exhaustion test — submit 1M+ transactions to fill the mempool. Verify: eviction works correctly, the node remains responsive, and memory stays bounded.
- [ ] **18.9** Run large block test — create blocks with the maximum number of transactions. Verify propagation, validation, and state application complete within target time.
- [ ] **18.10** Run RPC load test — use `wrk` or `k6` to load test RPC endpoints at 10,000 req/s. Verify response times, error rates, and rate limiting behavior.
- [ ] **18.11** Document all load test results — create `.systemx/status/reports/load-test-results.md` with: test parameters, results, bottlenecks identified, and recommendations.
- [ ] **18.12** Implement automated load test CI job — add a nightly CI job that runs a 30-minute load test and fails if performance degrades beyond thresholds.

### Success Criteria

- System handles ≥ 1000 TPS sustained without degradation
- Recovery from network partition within 60 seconds
- No memory leaks after 24-hour soak test
- Byzantine behavior correctly rejected
- All results documented with benchmarks

---

## Phase 19: Documentation & Developer Experience

**Priority:** P3
**Estimated Effort:** 2 weeks
**Dependencies:** Phase 15 (API docs), Phase 17 (governance/bridge design)

### Description

The project needs comprehensive documentation for developers, operators, and contributors. Good documentation accelerates community growth and reduces support burden.

### Tasks

- [ ] **19.1** Write a comprehensive `README.md` for the root project — cover: what COINjecture is, architecture overview, quick start, build instructions, Docker usage, and links to detailed docs.
- [ ] **19.2** Create a contribution guide — write `CONTRIBUTING.md` covering: code style, PR process, commit message format, testing requirements, and code review expectations.
- [ ] **19.3** Create a changelog — write `CHANGELOG.md` following Keep a Changelog format. Backfill from git history and `CURRENT_ISSUES.md`.
- [ ] **19.4** Write architecture documentation — create `.systemx/docs/architecture/overview.md` with: high-level architecture diagram (Mermaid), crate dependency graph, data flow diagrams, and component descriptions for all 13 crates.
- [ ] **19.5** Write operator guide — create `.systemx/docs/guides/operator-guide.md` covering: hardware requirements, installation, configuration reference, monitoring setup, backup/recovery, and troubleshooting.
- [ ] **19.6** Write the security model document — create `.systemx/security/threat-model/security-model.md` describing: trust assumptions, attack surface, threat actors, mitigations, and incident response plan.
- [ ] **19.7** Create ADR (Architecture Decision Record) templates — create `.systemx/docs/decisions/template.md` and backfill key decisions: why CPP instead of libp2p, why redb, why ed25519-dalek, why no-std crypto.
- [ ] **19.8** Add inline documentation to all crate lib.rs files — each crate's `lib.rs` should have a top-level doc comment explaining: purpose, key types, usage examples, and relationship to other crates.
- [ ] **19.9** Create a glossary — write `.systemx/docs/guides/glossary.md` defining: CPP, equilibrium flow, dimensional pools, conjecture propagation, trustlines, golden ratio mechanics, and other domain terms.
- [ ] **19.10** Write a testnet guide — create `.systemx/docs/guides/testnet-guide.md` expanding on `TESTNET_QUICKSTART.md` with: detailed setup, faucet usage, monitoring, common issues, and how to participate.
- [ ] **19.11** Create code walkthroughs — write `.systemx/docs/guides/code-walkthrough.md` tracing a transaction from wallet submission to block inclusion, referencing specific files and functions.
- [ ] **19.12** Set up documentation site — configure `mdbook` or a static site generator to publish docs from `.systemx/docs/` to GitHub Pages. Add CI step to build and deploy docs on merge.

### Success Criteria

- README enables a new developer to understand and build the project in 15 minutes
- Contribution guide enables first PR in 30 minutes
- Architecture docs provide clear system understanding
- Operator guide enables production deployment
- Documentation site is live and auto-updated

---

## Phase 20: Final Audit & Launch Readiness

**Priority:** P0 (gates launch)
**Estimated Effort:** 3–4 weeks
**Dependencies:** ALL previous phases

### Description

Final pre-launch verification. External audit, penetration testing, chaos engineering, and formal sign-off.

### Tasks

- [ ] **20.1** Conduct internal security review — walk through all Phase 1–6 fixes with a security-focused review. Verify each fix with targeted tests. Document results in `.systemx/security/audit-findings/internal-review.md`.
- [ ] **20.2** Engage external security audit firm — provide auditors with: source code, architecture docs, threat model, and a pre-audit briefing on the CPP consensus mechanism.
- [ ] **20.3** Conduct smart contract / escrow audit — specifically audit `state/src/escrows.rs`, `state/src/channels.rs`, and `state/src/timelocks.rs` for logic errors, reentrancy, and fund lock conditions.
- [ ] **20.4** Run penetration testing on the RPC layer — test all endpoints for: injection, authentication bypass, rate limit circumvention, DoS, and information disclosure.
- [ ] **20.5** Run penetration testing on the web wallet — test for: XSS, CSRF, key exfiltration, session hijacking, and clickjacking.
- [ ] **20.6** Run chaos engineering tests — use the Docker testnet to simulate: random node kills, network partitions, disk full, clock skew, and resource exhaustion. Verify recovery for all scenarios.
- [ ] **20.7** Verify all CI checks pass — ensure the full CI pipeline (fmt, clippy, audit, deny, tests, coverage, Docker smoke, unwrap check) passes on the release branch.
- [ ] **20.8** Verify documentation completeness — check that all docs referenced in this plan exist and are accurate. Run link checker. Verify API docs match implementation.
- [ ] **20.9** Create a launch checklist — write `.systemx/plans/launch-checklist.md` with every pre-launch item: DNS, TLS certs, monitoring alerts, backup schedule, incident response contacts, and rollback plan.
- [ ] **20.10** Conduct a tabletop incident response exercise — simulate a scenario (e.g., consensus bug causing fork) and walk through the response: detection, communication, fix, and post-mortem.
- [ ] **20.11** Final performance validation — run the full load test suite from Phase 18 on the release candidate build. Verify all performance targets are met.
- [ ] **20.12** Create the release — tag the release in git, build release binaries, publish Docker images, update documentation site, write release notes, and announce.

### Success Criteria

- External audit finds zero critical or high issues (or all are resolved)
- Penetration testing finds zero exploitable vulnerabilities
- Chaos engineering demonstrates recovery from all failure modes
- All CI checks pass on release candidate
- Launch checklist 100% complete
- Release tagged, built, and published

---

## Appendix A: Crate Reference

| Crate | Path | Purpose |
|-------|------|---------|
| `adzdb` | `adzdb/src/lib.rs` | Custom database layer |
| `core` | `core/src/` | Types, crypto, transactions, blocks, commitments, privacy |
| `consensus` | `consensus/src/` | Mining, difficulty, work scoring, problem registry |
| `network` | `network/src/` | CPP protocol, peer management, reputation |
| `state` | `state/src/` | Accounts, escrows, channels, trustlines, marketplace |
| `mempool` | `mempool/src/` | Transaction pool, fee market, data pricing |
| `rpc` | `rpc/src/` | JSON-RPC server, WebSocket subscriptions |
| `tokenomics` | `tokenomics/src/` | Emission, rewards, staking, AMM, governance, deflation |
| `node` | `node/src/` | Full node binary, chain management, config, keystore |
| `wallet` | `wallet/src/` | CLI wallet, keystore, RPC client |
| `marketplace-export` | `marketplace-export/src/` | Marketplace data export |
| `huggingface` | `huggingface/src/` | AI model integration, metrics, streaming |
| `mobile-sdk` | `mobile-sdk/src/` | Mobile client SDK |

## Appendix B: Key Files Reference

| File | Significance |
|------|-------------|
| `core/src/crypto.rs` | All cryptographic primitives |
| `core/src/privacy.rs` | ZK proof verification (CRITICAL: placeholder) |
| `core/src/commitment.rs` | Consensus commitments (CRITICAL: unsigned) |
| `node/src/keystore.rs` | Node private key storage (CRITICAL: plaintext) |
| `wallet/src/keystore.rs` | Wallet private key storage (CRITICAL: plaintext) |
| `rpc/src/server.rs` | RPC endpoint definitions (CRITICAL: no auth) |
| `rpc/src/websocket.rs` | WebSocket RPC (CRITICAL: no auth) |
| `network/src/lib.rs` | P2P networking (CRITICAL: no TLS) |
| `node/src/config.rs` | Node configuration |
| `node/src/main.rs` | Entry point |
| `node/src/validator.rs` | Block/tx validation |
| `node/src/chain.rs` | Chain management |
| `.github/workflows/ci.yml` | CI pipeline |
| `Dockerfile` | Container build |
| `docker-compose.yml` | Testnet orchestration |
| `web-wallet/index.html` | Web wallet frontend |

## Appendix C: Dependency Additions

Crates to add to `[workspace.dependencies]` during this plan:

| Crate | Version | Purpose | Phase |
|-------|---------|---------|-------|
| `zeroize` | 1.x | Secure memory clearing | 1 |
| `argon2` | 0.5 | Key derivation | 1 |
| `aes-gcm` | 0.10 | Key encryption | 1 |
| `cargo-deny` | (CI tool) | Dependency auditing | 1 |
| `jsonwebtoken` | 9.x | JWT for RPC auth | 2 |
| `rustls` | 0.23 | TLS for P2P | 5 |
| `snow` | 0.9 | Noise protocol | 5 |
| `proptest` | 1.x | Property-based testing | 4, 8 |
| `criterion` | 0.5 | Benchmarking | 14 |
| `rayon` | 1.x | Parallel processing | 14 |
| `tracing-appender` | 0.2 | Log rotation | 7 |

---

## Appendix D: Timeline Estimate

| Phase | Priority | Est. Effort | Cumulative |
|-------|----------|-------------|------------|
| 1. Critical Security | P0 | 2–3 weeks | 2–3 weeks |
| 2. RPC Security | P0 | 1–2 weeks | 3–5 weeks |
| 3. Error Handling | P0 | 2 weeks | 3–5 weeks (parallel w/ 1) |
| 4. Consensus Safety | P0 | 2–3 weeks | 5–8 weeks |
| 5. Network Security | P0 | 2–3 weeks | 7–11 weeks |
| 6. Input Validation | P1 | 1–2 weeks | 8–13 weeks |
| 7. Logging | P1 | 1–2 weeks | 6–9 weeks (parallel w/ 4-5) |
| 8. Unit Tests | P1 | 2 weeks | 8–11 weeks |
| 9. Integration Tests | P1 | 2–3 weeks | 10–14 weeks |
| 10. CI/CD | P1 | 1–2 weeks | 11–16 weeks |
| 11. Docker Security | P1 | 1–2 weeks | 12–18 weeks |
| 12. Web Wallet | P1 | 2 weeks | 13–20 weeks |
| 13. Database | P2 | 2 weeks | 14–22 weeks |
| 14. Performance | P2 | 2–3 weeks | 16–25 weeks |
| 15. API Docs | P2 | 1–2 weeks | 14–22 weeks (parallel) |
| 16. Protocol Versioning | P2 | 2 weeks | 16–24 weeks |
| 17. Governance & Bridge | P2 | 3–4 weeks | 19–28 weeks |
| 18. Load Testing | P2 | 2 weeks | 18–27 weeks |
| 19. Documentation | P3 | 2 weeks | 20–30 weeks |
| 20. Final Audit | P0 | 3–4 weeks | 23–34 weeks |

**Total estimated timeline: 6–8 months** (with parallelization)

---

*This document is a living plan. Update task status as work progresses. Move completed items to `.systemx/todos/done/`. Track blockers in `.systemx/todos/blocked/`.*
