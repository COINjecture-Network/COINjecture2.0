# Phase 15 — API Documentation & Standards

**Date:** 2026-03-25
**Branch:** claude/nice-babbage

---

## Summary

Created comprehensive API documentation suite: OpenRPC specification, versioning strategy, error code catalog, rate limit documentation, authentication guide, WebSocket endpoint documentation, multi-language SDK examples, Postman/Insomnia collection, and API changelog.

---

## Changes

### 1. OpenRPC Specification (`docs/api/openrpc.json`)
- OpenRPC 1.3.2-compliant JSON document
- All 26 JSON-RPC methods documented with full parameter and result schemas
- Component schemas for `Block`, `BlockHeader`, `ProblemInfo`
- All 20 application error codes defined in `components.errors`
- Server entry pointing to `http://127.0.0.1:9933`

### 2. API Versioning Strategy (`docs/api/VERSIONING.md`)
- Method-level additive versioning (not URL versioning)
- Non-breaking change policy (optional params, new fields, new methods)
- Breaking change policy (new method name with `_v2` suffix)
- Deprecation guarantee: 2 minor releases before removal
- API lifecycle states: `stable`, `beta`, `deprecated`, `removed`
- All 26 methods classified as `stable` as of v4.8.4
- Future `meta_getApiVersion` method described

### 3. Error Code Catalog (`docs/api/ERROR_CODES.md`)
- Standard JSON-RPC 2.0 error codes (-32700 to -32603)
- 20 application-specific codes (-32001 to -32020) with descriptions and recovery actions
- HTTP status codes table (200, 400, 429, 503)
- Error response JSON shape
- Source location (`rpc/src/server.rs`)

### 4. Rate Limit Documentation (`docs/api/RATE_LIMITS.md`)
- Recommended production limits table (all endpoints)
- 429 response JSON shape and `Retry-After` header handling
- Client exponential backoff guidance
- Nginx and Caddy reverse proxy configuration examples
- Faucet cooldown mechanics (`--faucet-cooldown` flag, `cooldown_remaining` field)
- Public vs. private endpoint access policy table

### 5. Authentication Guide (`docs/api/AUTH.md`)
- HTTP JSON-RPC: unauthenticated by default; reverse-proxy options (IP allowlist, Basic Auth, mTLS)
- WebSocket Ed25519 auth flow (4-step with JSON examples)
- Signature construction in Rust and JavaScript
- Transaction signing model (client-side only; node never handles private keys)
- Key management via wallet CLI (`coinject-wallet account`)
- TLS termination via Caddy reverse proxy

### 6. WebSocket Documentation (`docs/api/WEBSOCKET.md`)
- Endpoint: `ws://{ws_addr}` (default `0.0.0.0:8080`)
- Message format: tagged union with `type` field, `snake_case` names
- Auth flow and access levels (read-only vs. authenticated)
- All 8 client→server messages documented with field tables
- All 9 server→client messages documented (including push: `new_block`, `reward_notification`)
- Connection limits: 10 per IP, 50 msg/s, 5-minute idle timeout
- Complete JavaScript example mining client

### 7. SDK Examples (`docs/api/SDK_EXAMPLES.md`)
- Rust and JavaScript setup instructions
- JSON-RPC helper functions (both languages)
- Chain queries: `chain_getInfo`, `chain_getBlock`, `chain_getLatestBlock`
- Account operations: `account_getBalance`, `account_getNonce`
- Transaction lifecycle: build → sign → submit → poll confirmation
- Marketplace: list problems, submit subset-sum, submit solution
- Faucet usage (testnet)
- WebSocket mining client in Rust (tokio-tungstenite)
- Error handling and exponential backoff patterns

### 8. Postman/Insomnia Collection (`docs/api/postman_collection.json`)
- Postman Collection v2.1.0 format (importable into both Postman and Insomnia)
- 25 pre-built requests organized in folders: Chain, Account, Transactions, Marketplace, Timelocks, Escrows, Channels, Network, Faucet
- Collection variables: `rpc_url`, `address`, `tx_hash`, `block_height`, `problem_id`
- All requests pre-filled with correct JSON-RPC envelope

### 9. API Changelog (`docs/api/CHANGELOG.md`)
- Documents all methods added in v4.8.4 (14 new methods)
- Documents `transaction_submit` change (JSON format support)
- WebSocket protocol changes in v4.8.4
- Error code addition history (v4.8.0 and v4.8.4)
- Deprecation and removal tracking sections

---

## cargo check

```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.39s
```

Zero errors. (Phase 15 is documentation-only; no source code changes.)

---

## Files Created

```
docs/api/
├── openrpc.json
├── VERSIONING.md
├── ERROR_CODES.md
├── RATE_LIMITS.md
├── AUTH.md
├── WEBSOCKET.md
├── SDK_EXAMPLES.md
├── postman_collection.json
└── CHANGELOG.md
```
