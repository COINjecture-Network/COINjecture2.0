# CPP Protocol Upgrade Procedure

This document describes the step-by-step procedure for bumping the CPP protocol version.
Follow every step in order; skipping steps can cause network partitions.

---

## When to Bump the Protocol Version

Bump `CURRENT_PROTOCOL_VERSION` when ANY of the following is true:

- A new field is added to an existing message struct (even with `#[serde(default)]`).
- A new message type byte is allocated.
- The wire-frame layout changes (header bytes, checksum algorithm, framing).
- A new behavior is required that old nodes cannot safely perform.

Bump `MIN_SUPPORTED_VERSION` only when you are ready to **reject** connections from nodes running the old version.  This is a network-breaking change and requires at minimum one full release cycle of warning.

---

## Step-by-Step Checklist

### 1. Implement the new protocol feature

- Add new message fields with `#[serde(default)]` so V(N-1) nodes can decode V(N) messages (field simply defaults to zero/None/false).
- Add new message type bytes in `MessageType` enum only in the unused space (`0x03`–`0x0F`, `0x15`–`0x1F`, etc.).
- Guard new behavior behind `FeatureFlags` or a version check.

### 2. Update version constants (two places)

**`network/src/cpp/config.rs`:**
```rust
pub const VERSION: u8 = N;           // was N-1
// MIN_PROTOCOL_VERSION stays at N-1 unless you are dropping old peers
```

**`network/src/cpp/version.rs`:**
```rust
pub const CURRENT_PROTOCOL_VERSION: u8 = N;   // was N-1
// pub const MIN_SUPPORTED_VERSION: u8 = N-1; // keep until next cycle
```

### 3. Add the new `ProtocolVersion` variant

```rust
pub enum ProtocolVersion {
    V1 = 1,
    V2 = 2,
    VN = N,  // add this
}
```
Update `from_u8()` and `is_supported()` accordingly.

### 4. Update `FeatureFlags::for_version()`

Add a new arm in the `match` block that enables new feature flags for version N.
Keep the V(N-1) arm returning `false` for the new flags.

### 5. Add version-specific handler dispatch

In `VersionDispatch::for_version()`, map the new version to a new variant or the existing `V2Current` variant — whichever is appropriate.

### 6. Update `PROTOCOL_CHANGELOG.md`

Add a new `## Version N` section documenting:
- Every changed message and the new field(s).
- The rationale.
- The `min peer version` line.

### 7. Update deprecation warnings

- Adjust the deprecation warning string in `NegotiatedVersion::deprecation_warning()` if you are changing which versions are deprecated.
- If bumping `MIN_SUPPORTED_VERSION`, update the **Deprecation Schedule** table in the changelog.

### 8. Run the full test suite

```sh
cargo test --workspace
```

Pay special attention to:
- `network/src/cpp/version.rs` tests
- `network/src/cpp/protocol.rs` tests
- Integration tests under `network/tests/`

### 9. Rolling upgrade plan (network operations)

When deploying to a live network:

1. **Phase A — Deploy V(N) nodes alongside V(N-1) nodes.**
   Because `MIN_SUPPORTED_VERSION` is still N-1, all nodes can communicate.
   V(N) nodes log deprecation warnings for V(N-1) peers; this is expected.

2. **Phase B — Monitor.**
   Confirm that V(N) nodes are propagating blocks and transactions correctly with V(N-1) peers.
   Use metrics: `cpp_negotiated_version_histogram`, `cpp_legacy_peer_count`.

3. **Phase C — Raise `MIN_SUPPORTED_VERSION` to N (in the NEXT release).**
   At this point V(N-1) nodes receive a `DisconnectMessage` with reason "version too old".
   This change should be announced at least 30 days in advance.

4. **Phase D — Remove V(N-1) compatibility code.**
   Remove legacy serde defaults that were only needed for V(N-1) compat, update tests.

### 10. Tag the release

Include the new protocol version in the git tag and release notes:
```
git tag v4.9.0-proto-v3
```

---

## Common Pitfalls

| Pitfall | Mitigation |
|---------|------------|
| Adding a required field without `#[serde(default)]` | V(N-1) nodes will fail to deserialize — always add `#[serde(default)]` |
| Bumping `MIN_SUPPORTED_VERSION` without announcing | Hard-partitions nodes mid-upgrade — always give at least one release cycle of warning |
| Forgetting to update both `config.rs` and `version.rs` | Tests will fail; CI catches this |
| Reusing a retired message type byte | Causes silent misinterpretation — allocate only from unused ranges |
| Not running integration tests | Version mismatch bugs surface in prod — run `cargo test --workspace` |

---

## File Locations

| File | What to update |
|------|----------------|
| `network/src/cpp/config.rs` | `VERSION`, `MIN_PROTOCOL_VERSION` constants |
| `network/src/cpp/version.rs` | `CURRENT_PROTOCOL_VERSION`, `MIN_SUPPORTED_VERSION`, `ProtocolVersion` enum, `FeatureFlags::for_version()`, `NegotiatedVersion::deprecation_warning()` |
| `network/src/cpp/message.rs` | New message structs / new fields with `#[serde(default)]` |
| `network/src/cpp/protocol.rs` | `ConnectionPolicy` usage in decode paths |
| `docs/PROTOCOL_CHANGELOG.md` | New version section |
| `docs/PROTOCOL_UPGRADE_PROCEDURE.md` | (this file) update deprecation schedule |
