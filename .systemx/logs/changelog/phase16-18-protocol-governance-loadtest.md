# Changelog — Phases 16, 17, 18

**Date:** 2026-03-25
**Branch:** claude/epic-pasteur
**Author:** Claude (Production Readiness Sprint)

---

## Phase 16 — Protocol Versioning & Migration

### Files Created
- `network/src/cpp/version.rs` — Protocol versioning infrastructure

### Files Modified
- `network/src/cpp/config.rs` — `VERSION` bumped to 2, `MIN_PROTOCOL_VERSION = 1` added
- `network/src/cpp/protocol.rs` — Backward-compatible decode (accepts V1 in V2 node)
- `network/src/cpp/mod.rs` — Exports `version` module

### Documents Created
- `docs/PROTOCOL_CHANGELOG.md`
- `docs/PROTOCOL_UPGRADE_PROCEDURE.md`

### Summary
- Added `ProtocolVersion` enum (V1, V2), `NegotiatedVersion`, `FeatureFlags`, `ConnectionPolicy`, `VersionDispatch` types.
- `ConnectionPolicy::evaluate()` allows V1 ↔ V2 communication during rolling upgrades (network partition prevention).
- `NegotiatedVersion::negotiate()` agrees on `min(local, remote)` version.
- `NegotiatedVersion::deprecation_warning()` surfaces V1 peer warnings to callers.
- `FeatureFlags::for_version()` derives capabilities from negotiated version without wire transmission.
- Both `MessageEnvelope::decode()` and `receive_from_read_half()` now accept `[1, 2]` version bytes instead of requiring exact match.
- 12 unit tests added to `version.rs`.
- Protocol changelog documents V1 history and V2 additions.
- Upgrade procedure doc covers all 10 checklist steps for future version bumps.

---

## Phase 17 — Governance & Bridge Crate Completion

### Files Modified
- `tokenomics/src/governance.rs` — Added `ProposalAction`, `ExecutionReceipt`, execution methods

### Documents Created
- `docs/GOVERNANCE.md`
- `docs/BRIDGE_PROTOCOL.md`

### Governance Additions (`tokenomics/src/governance.rs`)
- `ProposalAction` enum: `ChangeParameter`, `ProtocolUpgrade`, `TreasuryTransfer`, `ConstitutionalAmendment`, `EmergencyAction`
- `EmergencyActionType` enum: `PauseNetwork`, `ResumeNetwork`, `FreezeAccount`, `SlashValidator`
- `ExecutionReceipt` struct with `proposal_id`, `action`, `executed_at`, `success`, `message`
- `ExecutionError` enum: `NotPassed`, `TimelockNotExpired`, `NoAction`, `ActionFailed`, `AlreadyExecuted`
- `Proposal::new_with_action()` — creates proposals with typed on-chain actions
- `Proposal::execute()` — validates lifecycle and records receipt
- `GovernanceManager::create_proposal_with_action()` — creates proposals with actions
- `GovernanceManager::execute_proposal()` — executes passed proposals after timelock
- `GovernanceManager::executable_proposals()` — queries proposals ready to execute
- `GovernanceManager::execution_history()` — all execution receipts
- 4 new tests: parameter change execution, status validation, timelock enforcement, protocol upgrade proposals

### Bridge Audit
The bridge (`network/src/mesh/bridge.rs`) was audited and found to be substantially complete:
- All `BridgeCommand` variants handled
- All `MeshEvent` variants translated to `BridgeEvent`
- Consensus salt and commit forwarding operational
- Known limitations documented in `BRIDGE_PROTOCOL.md`

---

## Phase 18 — Load Testing & Stress Testing

### Files Created
- `load-test/Cargo.toml`
- `load-test/src/main.rs` — CLI with 7 subcommands
- `load-test/src/tx_generator.rs` — Transaction flood generator
- `load-test/src/mempool_flood.rs` — Mempool saturation test
- `load-test/src/rpc_load.rs` — Concurrent RPC blast
- `load-test/src/network_stress.rs` — Simulated peer connections
- `load-test/src/large_block.rs` — Max-capacity block test
- `load-test/src/stability.rs` — Long-running stability + recovery tests
- `load-test/src/monitor.rs` — Health polling, memory leak detection, disk monitoring
- `load-test/src/results.rs` — Structured results, LatencyStats, ThroughputCounter
- `docs/LOAD_TESTING.md`

### Files Modified
- `Cargo.toml` — Added `load-test` to workspace members

### Capabilities
| Command | What it tests |
|---------|---------------|
| `tx-flood` | Sustainable TPS, latency under load |
| `mempool-flood` | Capacity enforcement, fee market congestion |
| `rpc-blast` | RPC server throughput, per-method latency |
| `stability` | Memory leaks, block production, long-run health |
| `network-stress` | Peer acceptance, MAX_PEERS enforcement |
| `large-block` | Max-tx block mining and validation |
| `recovery` | Node restart correctness, chain state persistence |

Output is structured JSON or human-readable table.
`LatencyStats` computes P50/P95/P99/max/mean from recorded samples.
`NodeMonitor` polls RPC health at configurable intervals and detects memory leaks.

---

## Test Impact

All changes are additive or backward-compatible:
- Phase 16: V2 nodes still accept V1 peers → no network partition
- Phase 17: Governance adds optional fields with `#[serde(default)]` → no breaking change
- Phase 18: New crate, no existing crate modified except workspace `Cargo.toml`
