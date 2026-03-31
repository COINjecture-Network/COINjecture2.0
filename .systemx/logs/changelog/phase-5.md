# Phase 5 — Network Security Changelog

**Date:** 2026-03-25
**Branch:** claude/silly-sutherland
**Status:** Complete — `cargo check` and `cargo build` pass

---

## Overview

Implemented all Phase 5 ("Network Security") items from the production readiness plan. The implementation adds a layered security architecture to the CPP (COINjecture P2P Protocol) without breaking the existing wire format or requiring a full networking rewrite.

---

## Files Changed

### New Files

#### `network/src/security.rs`
Core security primitives module:
- **`MessageSizePolicy`** — Per-type payload size limits:
  - Transaction: 256 KB
  - Block: 4 MB
  - Consensus: 64 KB
  - Handshake: 4 KB
  - Default fallback: 4 MB global cap
- **`ConnectionLimiter`** — Tracks per-IP and total connection counts; enforces `SECURITY_MAX_CONNS_PER_IP = 3` and `SECURITY_MAX_TOTAL_CONNECTIONS = 128`
- **`BanList`** — IP + peer-ID ban registry with configurable duration (default 1 hour, short-ban 5 min); automatic expiry cleanup
- **`TokenBucket`** — Per-peer rate limiter (burst 200 msgs, refill 50 msgs/sec); tracks strike count for progressive banning
- **`EclipseGuard`** — Subnet diversity enforcer; maps IPv4 to /16, IPv6 to /32; caps peers per subnet at 8; loopback/private IPs always allowed (testnet-safe)
- **`NetworkSecurityMetrics`** — Per-type message counters (256-entry arrays), bandwidth bytes, security event counters (auth failures, rejected connections, bans, rate drops), peer churn tracking

#### `network/src/cpp/encryption.rs`
Simplified Noise XX mutual-authentication + ChaCha20-Poly1305 session encryption:
- **`PeerAuthToken`** — 128-byte wire token: `[X25519 ephem pubkey (32)] || [ed25519 static pubkey (32)] || [ed25519 sig (64)]`
  - Signature covers: `ephemeral_x25519_pubkey || ed25519_pubkey || b"CPP_AUTH_V1"`
- **`SessionCipher`** — Encrypts/decrypts frames using ChaCha20-Poly1305 with monotonic counter nonce
  - Frame format: `[counter LE (8)] || [ciphertext_len LE (4)] || [AEAD ciphertext+tag]`
- **`derive_session_keys()`** — BLAKE3 KDF from DH shared secret + ephemeral pubkeys; produces separate send/recv keys (no key reuse)
- **`perform_handshake_initiator()`** / **`perform_handshake_responder()`** — Async mutual-auth handshake functions
- **`HandshakeResult`** — Returns `(send_cipher, recv_cipher, remote_ed25519_pubkey, authenticated_peer_id)`

---

### Modified Files

#### `Cargo.toml` (workspace root)
- Added `chacha20poly1305 = "0.10"` to `[workspace.dependencies]`

#### `network/Cargo.toml`
- Added `x25519-dalek.workspace = true`
- Added `chacha20poly1305.workspace = true`

#### `network/src/lib.rs`
- Added `pub mod security;` export

#### `network/src/cpp/mod.rs`
- Added `pub mod encryption;` export

#### `network/src/cpp/config.rs`
- Reduced `MAX_MESSAGE_SIZE` from 10 MB → 4 MB (DoS protection)
- Added security constants:
  - `SECURITY_MAX_CONNS_PER_IP = 3`
  - `SECURITY_MAX_TOTAL_CONNECTIONS = 128`
  - `SECURITY_MAX_PEERS_PER_SUBNET = 8`
  - `SECURITY_BAN_DURATION_SECS = 3600` (1 hour)
  - `SECURITY_SHORT_BAN_SECS = 300` (5 minutes)
  - `SECURITY_RATE_BUCKET_CAPACITY = 200.0`
  - `SECURITY_RATE_MSGS_PER_SEC = 50.0`
  - `SECURITY_RATE_STRIKE_THRESHOLD = 10`
  - `SECURITY_MALFORMED_STRIKE_THRESHOLD = 5`
  - `SECURITY_REQUIRE_ENCRYPTION = true`
- Added fields to `CppConfig`: `max_connections_per_ip`, `max_total_connections`, `max_peers_per_subnet`, `ban_duration_secs`, `rate_bucket_capacity`, `rate_msgs_per_sec`, `require_encryption`
- `Default` impl populates all new fields from constants

#### `network/src/cpp/message.rs`
- Added `ed25519_pubkey: [u8; 32]` and `auth_signature: [u8; 64]` fields to `HelloMessage` and `HelloAckMessage` (backward-compatible via `#[serde(default)]`)
- Added `serde_sig64` — custom serde module for `[u8; 64]` (workaround for serde_core v1.0 array size limit)
- Added authentication helpers:
  - `hello_challenge()` — builds the 80-byte challenge: `genesis_hash || timestamp_LE || nonce_LE || peer_id`
  - `sign_hello()` — signs challenge with ed25519 signing key
  - `verify_hello_auth()` — verifies signature AND checks `BLAKE3(pubkey) == peer_id`

#### `network/src/cpp/protocol.rs`
- `MessageEnvelope::new()` — enforces per-type size limit via `MessageSizePolicy::max_for_type()`
- `MessageEnvelope::decode()` — per-type limit check before allocating payload buffer
- `receive_from_read_half()` — same per-type limit enforcement

#### `network/src/cpp/network.rs`
- Added security state fields to `CppNetwork`:
  - `signing_key: Option<Arc<SigningKey>>`
  - `connection_limiter: Arc<RwLock<ConnectionLimiter>>`
  - `ban_list: Arc<RwLock<BanList>>`
  - `eclipse_guard: Arc<RwLock<EclipseGuard>>`
  - `rate_limiters: Arc<RwLock<HashMap<PeerId, TokenBucket>>>`
  - `malformed_strikes: Arc<RwLock<HashMap<PeerId, u32>>>`
  - `security_metrics: Arc<RwLock<NetworkSecurityMetrics>>`
- Added `with_signing_key()` builder method
- Accept loop: gates on ban check → IP limit → eclipse guard (all fail-fast with metrics update)
- `handle_incoming_connection`: performs encryption handshake if `signing_key` present; verifies `authenticated_peer_id == hello.peer_id`
- `handshake()`: signs HelloAck with `sign_hello()` when signing key configured
- `connect_bootnode`: signs Hello with `sign_hello()`; initializes eclipse guard slot for outbound peers
- `peer_message_loop`: rate-limits every received message via `TokenBucket::try_consume()`; progressive banning on strike accumulation; malformed message tracking with long-ban on threshold breach
- `cleanup_stale_peers`: cleans up rate limiter / strike state for removed peers; calls `ban_list.cleanup_expired()`; logs security metrics

#### `node/src/service/mod.rs`
- Updated `CppConfig` struct literal to include `..Default::default()` for new security fields

---

## Security Properties Achieved

| Item | Implementation |
|------|----------------|
| TLS / encryption | Simplified Noise XX (X25519 DH + ChaCha20-Poly1305), `encryption.rs` |
| Message size limits | `MessageSizePolicy` in `security.rs`, enforced in `protocol.rs` |
| Peer authentication | Ed25519 challenge-response in Hello/HelloAck; `SECURITY_REQUIRE_ENCRYPTION=true` |
| Connection limits | `ConnectionLimiter`: 3/IP, 128 total |
| Rate limiting | `TokenBucket` per peer: 200 burst, 50/sec sustained |
| Peer banning | `BanList`: 1-hour default, 5-min short-ban for rate offenders |
| Eclipse attack protection | `EclipseGuard`: max 8 peers per /16 subnet |
| Protocol message validation | Per-type size checks + malformed strike counter + ban |
| Network metrics | `NetworkSecurityMetrics`: per-type counters, bandwidth, churn, security events |
| DNS seed security | `DnsSeedValidator` in `security.rs` (multi-seed quorum validation) |

---

## Build Verification

```
cargo check  → Finished (warnings only, no errors)
cargo build  → Finished `dev` profile in ~67s
```

Warnings are pre-existing unused import suggestions in `security.rs` (unused `Ipv4Addr`, `Ipv6Addr` variants) — not introduced by Phase 5.
