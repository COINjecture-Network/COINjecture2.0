# Changelog

All notable changes to COINjecture 2.0 are documented in this file.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
Versioning follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added
- Comprehensive README rewrite with architecture, quickstart, API reference
- `CONTRIBUTING.md` — code style, PR process, commit conventions
- `CHANGELOG.md` — full project history
- `LICENSE` — MIT license file
- `CODE_OF_CONDUCT.md` — contributor covenant
- `SECURITY.md` — responsible disclosure policy
- `docs/GETTING_STARTED.md` — step-by-step developer onboarding guide
- `docs/TROUBLESHOOTING.md` — common issues and solutions
- `docs/ARCHITECTURE.md` — updated with Phase 18 consensus changes
- Doc comments on all public types in `core` crate
- `.systemx/plans/launch-checklist.md` — pre-mainnet readiness checklist
- `.systemx/plans/executive-summary.md` — 20-phase project summary

### Fixed
- `adzdb`: Replace deprecated `io::Error::new(io::ErrorKind::Other, ...)` with `io::Error::other()`
- `adzdb`: Remove redundant `.write(true)` on append-mode file opens
- `adzdb`: Add explicit `.truncate()` to all `create(true)` file opens
- Trailing whitespace in `node/src/service/mod.rs`
- All workspace formatting via `cargo fmt --all`

---

## [4.8.4] — 2026-03-25

### Phase 18 — Consensus Work Score Rewrite

#### Added
- `consensus/src/work_score.rs`: `WorkScoreCalculator` now accepts `ProblemRegistry` for problem-type-aware scoring
- `consensus/src/difficulty.rs`: `DifficultyAdjuster` queries `ProblemRegistry` instead of hardcoded match
- `consensus/src/problem_registry.rs`: Central registry for NP-hard problem type metadata

#### Changed
- **Work score formula rewritten** to log₂ bit-equivalent scoring:
  ```
  score = log2(solve_bits) * quality * problem_weight * time_efficiency
  ```
  where `solve_bits = solve_time_us / verify_time_us * difficulty_weight`
- `WorkScoreCalculator::new()` now requires `Arc<ProblemRegistry>`
- `DifficultyAdjuster::new()` now requires `Arc<ProblemRegistry>`
- Node startup wires `ProblemRegistry` into all consensus components

#### Documentation
- `docs/BOOTSTRAP.md`: Documents bootstrap phase and mathematical constants (η-framework)
- `docs/ARCHITECTURE.md`: Updated crate dependency graph

---

## [4.8.3] — 2026-03-25

### Phase 12 — Web Wallet Security & UX

#### Added
- `web-wallet/src/components/Toast.tsx`: Context-based toast notifications (replaces `alert()`)
- `web-wallet/src/lib/secure-storage.ts`: AES-256-GCM encrypted localStorage with PBKDF2 key derivation
- `web-wallet/index.html`: CSP meta headers (`script-src 'self'`, `frame-ancestors 'none'`, `upgrade-insecure-requests`)
- `web-wallet/src/components/TransactionModal.tsx`: Transaction confirmation dialog showing recipient/amount/fee
- Full responsive design (`.wallet-grid`, `.tab-nav`, 720px media query)
- ARIA landmarks throughout (nav, dialog, alert, status, switch, tab)
- Loading skeletons and spinner states on all async operations
- Inline `<ConfirmDialog>` replacing `confirm()` for account deletion
- Salt display for private bounties with copy button

#### Changed
- Production sourcemaps disabled in `vite.config.ts`
- All `alert()` / `confirm()` calls replaced with proper UI components

---

## [4.8.2] — 2026-03-25

### Phase 8 — Unit Testing Infrastructure

#### Added
- `core/tests/unit_tests.rs`: Comprehensive unit tests for types, crypto, transactions, blocks
- `core/tests/property_tests.rs`: Proptest property tests for cryptographic invariants
- `mempool/tests/pool_tests.rs`: Mempool behavior tests (ordering, eviction, fee market)
- `consensus/tests/consensus_property_tests.rs`: Property tests for difficulty/work score
- Network message serde round-trip tests in `network/src/cpp/message.rs`
- `proptest = "1"` and `tempfile = "3"` added to workspace dependencies
- `.systemx/scripts/test/run_tests.sh`: CI test runner with optional `--coverage` flag

#### Fixed
- `state/src/accounts.rs` tests: Replaced hard-coded paths with `tempfile::tempdir()` to fix parallel test conflicts

#### Result
- `cargo test --workspace`: **665 passed, 0 failed, 2 ignored** across 29 test binaries

---

## [4.8.1] — 2026-03 (Phase 7 — Node Service Decomposition)

### Changed
- `node/src/service/`: Decomposed monolithic `NodeService` into focused modules:
  - `mod.rs` — Node struct, lifecycle, startup
  - `block_processing.rs` — Transaction apply/unwind logic
  - `fork.rs` — Chain reorganization and fork detection
  - `mining.rs` — PoUW mining loop
  - `merkle.rs` — Merkle proof utilities

---

## [4.8.0] — 2026-03 (Phase 6 — CPP Network Protocol)

### Added
- Complete CPP (COINjecture P2P Protocol) implementation in `network/src/cpp/`
- `EquilibriumRouter`: Broadcast fanout = `ceil(sqrt(n) * η)` where η = 1/√2
- `FlockState`: Reynolds murmuration-based peer coordination
- Window-based flow control with adaptive congestion management
- 8 dimensional priority levels for message routing
- 8/8 integration tests passing for CPP protocol

### Removed
- **libp2p removed** — replaced with pure stdlib TCP implementation
- Eliminated ~50 transitive dependencies

### Changed
- Network port: 707 (CPP protocol)
- Wire format: `COIN magic (4B) + version (1B) + type (1B) + length (4B) + payload + blake3 (32B)`

---

## [4.7.0] — 2026-02 (Phase 5 — GoldenSeed Integration)

### Added
- `core/src/golden.rs`: GoldenGenerator for deterministic golden ratio streams
- Golden-enhanced Merkle tree (`MerkleTree::new_with_golden`)
- Golden-ordered Merkle tree (`MerkleTree::new_with_golden_ordering`)
- Block version 2: GoldenSeed-enhanced hashing (commitments + Merkle + MMR)
- `BLOCK_VERSION_GOLDEN = 2` constant

### Changed
- Block headers now include `version` field
- `BlockHeader::uses_golden_enhancements()` / `uses_standard_hashing()` helpers

---

## [4.6.0] — 2026-02 (Phase 4 — AdZDB Integration)

### Added
- `adzdb/` crate: Custom pure-Rust append-only block database
  - `adzdb.idx` — fixed-width index entries (56 bytes each)
  - `adzdb.dat` — raw serialized block data
  - `adzdb.hgt` — height → hash lookup
  - `adzdb.meta` — chain metadata (tip hash, height, genesis hash)
- `node/src/chain_adzdb.rs`: Chain backend using AdZDB

---

## [4.5.0] — 2026-01 (Phase 3 — State & Marketplace)

### Added
- `state/src/marketplace.rs`: Full PoUW marketplace state with redb persistence
  - `marketplace_problems` table, `marketplace_index`, `marketplace_escrow`
  - Autonomous bounty payout on solution verification
- `state/src/dimensional_pools.rs`: Dimensional pool swaps with D1–D8 scales
- `state/src/trustlines.rs`: XRPL-inspired bilateral credit lines
- `state/src/channels.rs`: Payment channel state
- `state/src/timelocks.rs`: Time-locked balance state
- `state/src/escrows.rs`: Multi-party escrow state
- `state/tests/privacy_marketplace_tests.rs`

---

## [4.4.0] — 2025-12 (Phase 2 — Database Migration)

### Changed
- **Replaced Sled with redb** — production-grade ACID-compliant embedded database
  - Full ACID compliance with explicit transaction boundaries
  - Compile-time type-safe table definitions
  - Cross-platform: Windows/Linux/macOS
  - Actively maintained (sled unmaintained since 2021)
- All state tables migrated to redb `TableDefinition`

---

## [4.3.0] — 2025-12 (Phase 1 — Core Foundation)

### Added
- `core/src/types.rs`: `Hash`, `Address`, `Balance`, `BlockHeight`, `Timestamp`, `WorkScore`
- `core/src/crypto.rs`: `KeyPair`, `PublicKey`, `Ed25519Signature`, `MerkleTree`
- `core/src/transaction.rs`: 7 transaction types (Transfer, TimeLock, Escrow, Channel, TrustLine, DimensionalPoolSwap, Marketplace)
- `core/src/block.rs`: `Block`, `BlockHeader`, `Blockchain`
- `core/src/problem.rs`: `ProblemType` (SubsetSum, SAT, TSP, Custom), `Solution`
- `core/src/commitment.rs`: Commit-reveal scheme for PoUW
- `core/src/dimensional.rs`: Dimensional math (η = λ = 1/√2, D_n = e^(-η·τ_n))
- `core/src/privacy.rs`: Privacy-preserving transaction types
- `tokenomics/`: Emission, staking, AMM, deflation, governance, bounty pricing
- `rpc/`: JSON-RPC server with HTTP/WebSocket support
- `wallet/`: CLI wallet with Ed25519 keystore
- `mempool/`: Transaction pool with fee market

---

## [4.0.0] — 2025-11 (Initial Architecture)

### Added
- Initial Rust workspace with 13 crates
- Proof-of-Useful-Work (PoUW) consensus concept
- NP-hard problem marketplace design
- Dimensional tokenomics whitepaper implementation
- 4-node Docker testnet
