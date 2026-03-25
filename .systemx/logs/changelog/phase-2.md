# Phase 2 — RPC Security & Authentication

**Date:** 2026-03-25
**Branch:** claude/flamboyant-chaplygin
**Crate:** `coinject-rpc`

---

## Summary

Ten security hardening items implemented across three new/modified files inside
the `rpc` crate.

---

## New files

### `rpc/src/middleware.rs`
Tower middleware layers applied to every HTTP request handled by the jsonrpsee
server.

- **`SecurityGateLayer` / `SecurityGateService`** — single combined layer that:
  - (1) **Rate limiting** — per-IP token-bucket (default 100 req/min, env
    `RPC_RATE_LIMIT_RPM`).  Uses `dashmap` for lock-free per-IP bucket storage.
    Returns HTTP 429 on exhaustion.
  - (3) **Request size limits** — checks `Content-Length` header; default 1 MB
    general (`RPC_MAX_BODY_BYTES`), 256 KB for `transaction_submit`
    (`RPC_MAX_TX_BODY_BYTES`).  Returns HTTP 413.
  - (8) **Admin endpoint IP filtering** — paths under `/admin` restricted to
    `RPC_ADMIN_ALLOWED_IPS` allowlist (default `127.0.0.1`).  Returns HTTP 403.
  - (9) **API key hashing** — Bearer tokens are blake3-hashed at validation time
    and compared against stored blake3 hashes (`RPC_API_KEYS`).  Plaintext keys
    are never stored.  `SecurityConfig::rotate_api_keys()` enables key rotation.
    Auth enabled via `RPC_REQUIRE_AUTH=true`.  Returns HTTP 401.
  - (6) **Security headers** — added to every response (both rejections and
    pass-through): `X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`,
    `X-XSS-Protection: 1; mode=block`,
    `Strict-Transport-Security: max-age=31536000; includeSubDomains`,
    `Content-Security-Policy: default-src 'none'`.

- **`AuditLogLayer` / `AuditLogService`** — body-type-preserving layer that logs
  every request via structured `tracing` events (timestamp via tracing
  subscriber, client IP from `X-Forwarded-For`/`X-Real-IP`, method, path, auth
  status, response status code, latency in ms).  Field names: `rpc.request`,
  `rpc.request.error`.

### `rpc/src/tls.rs`
Optional TLS termination proxy.

- **`TlsConfig`** — reads cert/key paths from `RPC_TLS_CERT` / `RPC_TLS_KEY`
  env vars; bind address from `RPC_TLS_BIND`.
- **`build_server_config`** — loads PEM cert chain and PKCS#8 private key,
  constructs `rustls::ServerConfig`.
- **`run_tls_proxy`** — async task that accepts TLS connections on the external
  address and bidirectionally proxies decrypted traffic to the plain-HTTP
  jsonrpsee server.  Spawned automatically by `RpcServer::new` when TLS is
  configured; task is aborted on `RpcServer::stop`.

---

## Modified files

### `rpc/src/server.rs`

- Added imports for `middleware`, `tls`, `TimeoutLayer`, `Duration`.
- **`RpcServerImpl::validate_str_len`** — rejects inputs exceeding a
  configurable byte limit with `INVALID_PARAMS` (-32602).
- **`RpcServerImpl::internal_error`** — (4) **Error sanitization** — logs the
  real error via `tracing::error!` and returns a generic
  `"Internal server error"` message to the caller.  Prevents file paths, DB
  errors, and stack details from leaking over the wire.
- Input validation added to **every** RPC handler that accepts string
  parameters (`address`, `tx_hash`, `problem_id`, `recipient`, `sender`, etc.)
  — max 256 bytes for address/hash fields, 1 MB for problem JSON.  Array
  lengths capped at 10 000 elements.
- (3) Hard 256 KB payload limit inside `submit_transaction` before
  deserialisation.
- `RpcServer::new` middleware stack (outer → inner):
  1. `AuditLogLayer` — structured request logging
  2. `TimeoutLayer(30s)` — (7) prevents slow-loris; configurable via
     `RPC_TIMEOUT_SECS`
  3. `SecurityGateLayer` — rate limit, auth, body size, admin IP filter,
     security headers
  4. `CorsLayer` — unchanged CORS behaviour
- `RpcServer` struct gains an optional `tls_task: JoinHandle` field; aborted in
  `stop()`.
- Removed `println!` calls; replaced with structured `tracing::info!/warn!`.

### `rpc/src/lib.rs`
Exports `pub mod middleware` and `pub mod tls`.

### `rpc/Cargo.toml`
Added: `dashmap = "6"`, `bytes = "1"`, `http-body = "1.0"`,
`http-body-util` (workspace), `blake3` (workspace), `tracing` (workspace),
`tower` with `timeout`/`util` features, `tokio-rustls = "0.26"`,
`rustls = "0.23"` (ring backend), `rustls-pemfile = "2"`.

---

## Environment variables reference

| Variable | Default | Description |
|---|---|---|
| `RPC_RATE_LIMIT_RPM` | `100` | Requests/minute per IP |
| `RPC_REQUIRE_AUTH` | `false` | Enable Bearer token auth |
| `RPC_API_KEYS` | _(empty)_ | Comma-separated plaintext API keys (stored as blake3 hashes) |
| `RPC_ADMIN_ALLOWED_IPS` | `127.0.0.1` | Comma-separated IPs for admin paths |
| `RPC_MAX_BODY_BYTES` | `1048576` | General request body size cap |
| `RPC_MAX_TX_BODY_BYTES` | `262144` | Transaction-submit body size cap |
| `RPC_TIMEOUT_SECS` | `30` | Per-request timeout |
| `RPC_TLS_CERT` | _(unset)_ | Path to PEM certificate chain |
| `RPC_TLS_KEY` | _(unset)_ | Path to PKCS#8 private key |
| `RPC_TLS_BIND` | `<addr>:port+1` | External TLS bind address |
