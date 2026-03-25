# Phase 6 — Input Validation & Sanitization

**Date:** 2026-03-25
**Branch:** claude/heuristic-napier
**Scope:** Full codebase hardening — every consensus-critical input path now
validated before use.

---

## New Files

### `core/src/validation.rs`
Central validation module. Provides:

| Export | Purpose |
|--------|---------|
| `ValidationError` | Rich error type covering all validation failure modes |
| `validate_amount` / `validate_fee` | Non-zero, ≤ MAX_AMOUNT checks |
| `checked_add` / `checked_sub` | Overflow-safe Balance arithmetic |
| `validate_amount_and_fee` | Combined amount + fee + overflow check |
| `validate_transfer_fields` | Transfer-specific amount/fee validation |
| `validate_timelock_fields` | TimeLock-specific amount/fee validation |
| `validate_escrow_fields` | Escrow Create amount/fee validation |
| `validate_additional_signatures_count` | ≤ 8 additional sigs guard |
| `validate_dispute_proof` | ≤ 1 KB dispute proof guard |
| `validate_data_payload` | ≤ 64 KB marketplace data guard |
| `validate_string_field` | ≤ 256 char string guard |
| `validate_block_header_fields` | Version, timestamp, f64 sanity, tx count |
| `validate_get_blocks_range` | Height order + ≤ 512 blocks per request |
| `validate_get_headers` | ≤ 2 048 headers per request |
| `validate_node_type_byte` | Node type enum 0–5 guard |
| `validate_reason_string` | Disconnect/rejection reason length guard |
| `validate_blocks_response_count` | Response vs request range coherence |
| `is_private_or_loopback` | IP class predicate |
| `validate_peer_address` | Port non-zero + production IP policy |
| `validate_peer_addr_str` | Parse + validate peer address string |
| `validate_port` | 1–65535 range check |
| `validate_socket_addr_str` | Parse + port check for config addresses |
| `validate_file_path` | Path traversal + null-byte prevention |
| `sanitize_for_log` | Newline / control-char stripping for logs |
| `sanitize_for_html` | HTML entity escaping (XSS prevention) |

**Constants:**
- `MAX_AMOUNT` = `u128::MAX / 2` — prevents amount+fee overflow
- `MIN_FEE` = 1
- `MAX_TX_DATA_SIZE` = 64 KB
- `MAX_BLOCK_TRANSACTIONS` = 10 000
- `MAX_ADDITIONAL_SIGNATURES` = 8
- `MAX_DISPUTE_PROOF_SIZE` = 1 024 bytes
- `MAX_REASON_STRING_LEN` = 256
- `MAX_BLOCKS_PER_REQUEST` = 512
- `MAX_HEADERS_PER_REQUEST` = 2 048
- `MAX_FUTURE_DRIFT_SECS` = 120

**Test coverage:** 47 unit tests covering every public function including
boundary values, overflow attacks, NaN/Inf injection, path traversal strings,
and known XSS payloads.

---

## Modified Files

### `core/src/lib.rs`
- Added `pub mod validation` and a note that it is accessed via the full path
  `coinject_core::validation::*` (not re-exported with `*`).

### `core/src/transaction.rs`
- `TransferTransaction::is_valid()` — calls `validate_transfer_fields` (amount
  non-zero, fee ≥ 1, sum no overflow).
- `TimeLockTransaction::is_valid()` — calls `validate_timelock_fields`.
- `EscrowTransaction::is_valid()` — validates `additional_signatures` count
  (≤ 8); calls `validate_escrow_fields` on Create, `validate_fee` on
  Release/Refund.
- `ChannelTransaction::is_valid()` — validates additional sigs count, checks
  Open deposit amounts, checks dispute proof size for UnilateralClose, and
  validates fee.
- `MarketplaceTransaction::is_valid()` — validates fee, bounty amount and
  fee+bounty overflow, work score finite/positive, data payload size for
  Custom problems and Custom solutions.

### `core/src/block.rs`
- `Block::verify()` — calls `validate_block_header_fields` as step 0 before
  any other checks. Rejects blocks with:
  - Unsupported version
  - Negative or far-future timestamp
  - NaN/Inf/negative work_score, time_asymmetry_ratio, complexity_weight,
    energy_estimate_joules
  - solution_quality outside [0.0, 1.0]
  - Transaction count > 10 000

### `node/src/config.rs`
- `NodeConfig::validate()` — added:
  - Socket address validation for `rpc_addr`, `cpp_p2p_addr`, `cpp_ws_addr`,
    `metrics_addr` (must be parseable and port 1–65535).
  - `max_peers` range 1–1000.
  - Faucet cooldown 1 s to 30 days when faucet is enabled.
  - `data_dir` path traversal prevention.
  - Bootnode address `host:port` format + port range check.
- New tests: `test_invalid_rpc_addr_port_zero`, `test_invalid_cpp_p2p_addr`,
  `test_max_peers_zero_rejected`, `test_max_peers_over_limit_rejected`,
  `test_invalid_bootnode_format`, `test_valid_bootnode`,
  `test_faucet_cooldown_zero_rejected`.

### `network/src/cpp/message.rs`
- `HelloMessage::validate()` — node_type byte check + non-zero peer_id.
- `HelloAckMessage::validate()` — same.
- `StatusMessage::validate()` — node_type byte check.
- `GetBlocksMessage::validate()` — height range order + ≤ 512 blocks.
- `BlocksMessage::validate()` — response count ≤ MAX_BLOCKS_PER_REQUEST.
- `GetHeadersMessage::validate()` — max_headers ≤ 2 048.
- `WorkRejectedMessage::validate()` — reason string ≤ 256 chars.
- `DisconnectMessage::validate()` — reason string ≤ 256 chars.
- New tests: `hello_validates_node_type`, `hello_rejects_zero_peer_id`,
  `get_blocks_range_validated`, `get_headers_validated`,
  `disconnect_reason_length_validated`, `work_rejected_reason_length_validated`.

---

## Security Properties Established

| Attack Vector | Mitigation |
|---------------|-----------|
| Integer overflow in balance arithmetic | `checked_add`/`checked_sub` + `MAX_AMOUNT` bound |
| Fee-free / dust spam | `MIN_FEE = 1` enforced at validation layer |
| Transaction data bomb | 64 KB hard cap on payload fields |
| Block stuffing (too many txs) | 10 000 transaction limit per block |
| NaN/Inf injection in PoUW metrics | `is_finite()` guards on all f64 fields |
| Future-dated blocks | 2-minute timestamp drift limit |
| Invalid block versions | Allowlist `[1, 2]` checked before processing |
| Unbounded sync requests | GetBlocks ≤ 512 / GetHeaders ≤ 2 048 |
| Peer message string bombs | 256-char cap on reason/label fields |
| Private network peer injection | IP class check in production mode |
| Path traversal via config | `..` component and null-byte detection |
| Log injection | Newline stripping in `sanitize_for_log` |
| XSS in web outputs | HTML entity escaping in `sanitize_for_html` |
| Invalid node_type enum | 0–5 range check on every handshake |
| Oversized dispute proofs | 1 KB cap before signature verification |
| Bad port in config | 1–65535 range enforced at startup |
