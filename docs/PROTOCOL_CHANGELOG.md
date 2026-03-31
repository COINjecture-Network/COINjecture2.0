# COINjecture P2P Protocol (CPP) — Changelog

All protocol-level changes are documented here.
Wire-format additions are **backward compatible** unless explicitly marked **BREAKING**.
Every entry records the version it applies to, the changed message(s), and the rationale.

---

## Version 2 (current)

**Released:** Phase 16 production readiness
**Min peer version:** 1 (V1 peers are still accepted; they simply lack optional fields)
**Constants:** `CURRENT_PROTOCOL_VERSION = 2`, `MIN_SUPPORTED_VERSION = 1`

### New: Version negotiation in handshake
- Both `HelloMessage` and `HelloAckMessage` carry a `version: u8` field.
- Peers agree on `min(local_version, remote_version)` as the session version.
- V2 nodes emit a deprecation warning (tracing log) when connected to a V1 peer.

### New: Connection nonce for simultaneous-connection tie-breaking
- `HelloMessage.connection_nonce: u64` (`#[serde(default)]` → 0 on V1 peers).
- `HelloAckMessage.connection_nonce: u64` (same).
- When two peers open connections to each other simultaneously, the peer with the **lower** nonce wins; the duplicate is dropped.

### New: Flock/murmuration state in StatusMessage
- `StatusMessage.flock_state: Option<FlockStateCompact>` (`#[serde(default)]` → `None` on V1 peers).
- Carries epoch, phase, cohesion, and swarm height for murmuration-aware routing.
- V1 peers send `None`; V2 nodes treat absent flock state as "unknown phase".

### New: Feature flags
- `FeatureFlags` struct derived from negotiated version (not transmitted over wire).
- Guards: `connection_nonces`, `flock_state_in_status`, `version_negotiation`, `murmuration_routing`, `deprecation_warnings`.

### New: Backward-compatible decode
- `MessageEnvelope::decode()` and `receive_from_read_half()` now accept messages with `version` byte in `[MIN_SUPPORTED_VERSION, CURRENT_PROTOCOL_VERSION]`.
- `ConnectionPolicy::Reject` is returned for peers below `MIN_SUPPORTED_VERSION`; the stream is closed with `DisconnectMessage`.

### New: Network partition prevention
- `ConnectionPolicy::AllowWithWarning` allows V1 ↔ V2 communication during rolling upgrades.
- Nodes on different versions can sync blocks and transactions; only optional V2 features (flock state, nonces) are unavailable to V1 peers.

---

## Version 1 (legacy, still supported)

**Released:** Initial implementation
**Deprecated:** yes — V1-only nodes will be rejected once `MIN_SUPPORTED_VERSION` is raised to 2 in a future release.

### Messages
| Type | Byte | Description |
|------|------|-------------|
| Hello | 0x01 | Initial handshake |
| HelloAck | 0x02 | Handshake acknowledgment |
| Status | 0x10 | Peer status update |
| GetBlocks | 0x11 | Block range request |
| Blocks | 0x12 | Block batch response |
| GetHeaders | 0x13 | Header request (light clients) |
| Headers | 0x14 | Header response |
| NewBlock | 0x20 | Newly mined block announcement |
| NewTransaction | 0x21 | New transaction announcement |
| SubmitWork | 0x30 | Light-client PoW submission |
| WorkAccepted | 0x31 | Work accepted by validator |
| WorkRejected | 0x32 | Work rejected |
| GetWork | 0x33 | Request mining template |
| Work | 0x34 | Mining template response |
| Ping | 0xF0 | Keep-alive ping |
| Pong | 0xF1 | Keep-alive pong |
| Disconnect | 0xFF | Graceful disconnect |

### Wire format
```
┌────────────┬─────────┬──────────┬─────────────┬─────────┬──────────┐
│ Magic (4B) │ Ver (1B)│ Type (1B)│ Length (4B) │ Payload │ Hash (32B)│
└────────────┴─────────┴──────────┴─────────────┴─────────┴──────────┘
```
- **Magic**: `COIN` (0x43 0x4F 0x49 0x4E)
- **Ver**: Protocol version byte
- **Type**: MessageType byte
- **Length**: Big-endian u32 payload byte count
- **Payload**: bincode-encoded message struct
- **Hash**: BLAKE3 checksum of payload bytes

---

## Planned: Version 3

The following features are candidates for V3.  No wire-format commitments yet.

- **Encrypted transport**: Noise XX handshake wrapping the TCP stream.
- **Multiplexed streams**: Multiple logical streams per TCP connection (eliminates head-of-line blocking for sync vs. propagation).
- **Compact block announcements**: Send only the block header + short txid list for NewBlock; peers fetch missing transactions.
- **Extended message types**: `GetMempool`, `Mempool`, `FeeFilter`, `AddrV2`.
- **Raise `MIN_SUPPORTED_VERSION` to 2**: Drop V1 support.

---

## Deprecation Schedule

| Version | Status | Deprecation Warning Since | Drop Support In |
|---------|--------|--------------------------|-----------------|
| V1 | Deprecated | V2 | V3 release |
| V2 | Current | — | — |

---

*Maintainer note: always bump `CURRENT_PROTOCOL_VERSION` in `network/src/cpp/config.rs` **and** `network/src/cpp/version.rs` together, update this changelog, and add a migration entry to `PROTOCOL_UPGRADE_PROCEDURE.md`.*
