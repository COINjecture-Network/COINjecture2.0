# API Changelog

All notable API changes are documented here.
Format follows [API Versioning Strategy](VERSIONING.md).

---

## [Unreleased]

No pending breaking changes.

---

## v4.8.4 — 2026-03-25

### Added

| Method | Change | Notes |
|--------|--------|-------|
| `chain_getInfo` | Added `total_work` field to response | Cumulative work score for fork-choice |
| `chain_getInfo` | Added `is_syncing` field to response | Indicates whether node is actively syncing |
| `marketplace_submitPublicSubsetSum` | New method | Submit a public subset-sum problem with bounty |
| `marketplace_revealProblem` | New method | Reveal a private problem after commitment |
| `timelock_getByRecipient` | New method | Query time-locked outputs by recipient |
| `timelock_getUnlocked` | New method | List all currently unlocked time-locks |
| `escrow_getBySender` | New method | Query escrows by sender address |
| `escrow_getByRecipient` | New method | Query escrows by recipient address |
| `escrow_getActive` | New method | List all active escrows |
| `channel_getByAddress` | New method | Query payment channels by participant address |
| `channel_getOpen` | New method | List all open payment channels |
| `channel_getDisputed` | New method | List all disputed payment channels |
| `network_getInfo` | New method | P2P peer info and listen addresses |
| `chain_submitBlock` | New method | Submit a fully-formed block (for miners) |

### Changed

| Method | Change | Notes |
|--------|--------|-------|
| `transaction_submit` | Now accepts JSON-encoded transaction in addition to hex-encoded bincode | Web wallet compatibility |

### WebSocket

| Change | Notes |
|--------|-------|
| `reward_notification` push message added | Server pushes reward info asynchronously after work submission |
| `new_block` push message added | Broadcast to all connected clients on chain tip advance |
| Idle timeout set to 5 minutes | Clients not sending any message within 5 minutes are disconnected |

---

## v4.8.0 — prior

### Stable methods at this version

All methods in this version are in `stable` state:

- `account_getBalance`
- `account_getNonce`
- `account_getInfo`
- `transaction_submit`
- `transaction_getStatus`
- `chain_getBlock`
- `chain_getLatestBlock`
- `chain_getBlockHeader`
- `chain_getInfo`
- `marketplace_getOpenProblems`
- `marketplace_getProblem`
- `marketplace_getStats`
- `marketplace_submitPrivateProblem`
- `marketplace_submitSolution`
- `faucet_requestTokens`

---

## Error Code Additions

| Version | Code | Name | Description |
|---------|------|------|-------------|
| v4.8.4 | `-32015` | `CHAIN_SYNCING` | Node is still syncing; data may be incomplete |
| v4.8.4 | `-32016` | `INVALID_BLOCK_VERSION` | Block version not supported |
| v4.8.4 | `-32017` | `COMMITMENT_MISMATCH` | Commitment does not match revealed problem |
| v4.8.4 | `-32018` | `UNAUTHORIZED` | Client not authenticated (WebSocket) |
| v4.8.4 | `-32019` | `RATE_LIMITED` | Request rate limit exceeded |
| v4.8.4 | `-32020` | `FEATURE_NOT_AVAILABLE` | Feature not available on this node type |
| v4.8.0 | `-32001` | `NOT_FOUND` | Requested resource does not exist |
| v4.8.0 | `-32002` | `INVALID_ADDRESS` | Malformed address |
| v4.8.0 | `-32003` | `INVALID_SIGNATURE` | Signature verification failed |
| v4.8.0 | `-32004` | `INSUFFICIENT_BALANCE` | Account balance too low |
| v4.8.0 | `-32005` | `INVALID_NONCE` | Incorrect transaction nonce |
| v4.8.0 | `-32006` | `TX_TOO_LARGE` | Transaction exceeds size limit |
| v4.8.0 | `-32007` | `POOL_FULL` | Mempool at capacity |
| v4.8.0 | `-32008` | `FEE_TOO_LOW` | Transaction fee below minimum |
| v4.8.0 | `-32009` | `PROBLEM_NOT_FOUND` | Problem ID not in marketplace |
| v4.8.0 | `-32010` | `PROBLEM_EXPIRED` | Problem bounty expired |
| v4.8.0 | `-32011` | `INVALID_SOLUTION` | Solution does not satisfy the problem |
| v4.8.0 | `-32012` | `SOLUTION_ALREADY_SUBMITTED` | Correct solution already accepted |
| v4.8.0 | `-32013` | `FAUCET_COOLDOWN` | Faucet cooldown active |
| v4.8.0 | `-32014` | `FAUCET_DISABLED` | Faucet not enabled |

---

## Deprecation Notices

No methods are currently deprecated.

---

## Removed Methods

No methods have been removed.
