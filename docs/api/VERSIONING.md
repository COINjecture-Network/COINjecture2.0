# API Versioning Strategy

## Current Version

**API Version: 1** (`v1`)

The API does not currently include a version prefix in method names. All methods are accessed at the default JSON-RPC endpoint.

---

## Versioning Approach

COINjecture uses **method-level additive versioning**, not URL versioning.

### Principles

1. **Non-breaking changes** are added without version bumps:
   - Adding optional parameters (with sensible defaults)
   - Adding new fields to response objects
   - Adding new methods

2. **Breaking changes** introduce a new method name:
   - Changing parameter types or required parameters
   - Removing response fields
   - Changing return types

3. **Deprecation**: Old method names are kept for at least 2 minor releases with a deprecation warning in the response `data` field before removal.

### Method Naming Convention

Current methods use the pattern `{namespace}_{verb}[{Object}]`:

```
account_getBalance
chain_getBlock
marketplace_submitSolution
```

When a breaking change is needed, the new version appends a version suffix:

```
chain_getBlock_v2   ← new signature
chain_getBlock      ← kept for backwards compatibility, deprecated
```

---

## API Lifecycle

| State | Meaning |
|-------|---------|
| `stable` | Production-ready; backwards compatibility guaranteed for current major version |
| `beta` | Functional but signature may change; not recommended for production clients |
| `deprecated` | Still works; will be removed in next major version |
| `removed` | No longer available |

### Current Method Status

All documented methods in `CoinjectRpc` trait are **stable** as of v4.8.4.

---

## Transport Versioning

### JSON-RPC over HTTP

- Endpoint: `http://{rpc_addr}` (default: `127.0.0.1:9933`)
- No URL versioning; method names carry version information
- Content-Type: `application/json`

### WebSocket

- Endpoint: `ws://{ws_addr}` (default: `0.0.0.0:8080`)
- Messages use `type` field for routing; no explicit version field
- WebSocket protocol is **v1**; future breaking changes will introduce `protocol_version` in `Auth` message

---

## Changelog Policy

All API changes are logged in `docs/api/CHANGELOG.md` with:
- The method affected
- The nature of the change (added / changed / deprecated / removed)
- The version in which it took effect

---

## Version Negotiation (Future)

A `meta_getApiVersion` method will be added in a future release:

```json
// Request
{"jsonrpc": "2.0", "method": "meta_getApiVersion", "id": 1}

// Response
{"jsonrpc": "2.0", "id": 1, "result": {"version": 1, "supported": [1]}}
```

Clients should call this on first connection to verify compatibility.
