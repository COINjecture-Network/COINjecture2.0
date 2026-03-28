# Rate Limits

## Current Status

Rate limiting is **not yet enforced** by default in v4.8.4. This document describes the intended limits for production deployments and how to configure enforcement.

---

## Recommended Limits (Production)

| Endpoint | Limit | Window | Applies To |
|----------|-------|--------|------------|
| JSON-RPC (all methods) | 100 req | 10 seconds | Per IP |
| `transaction_submit` | 10 req | 1 second | Per IP |
| `marketplace_submitPublicSubsetSum` | 5 req | 60 seconds | Per address |
| `marketplace_submitSolution` | 20 req | 60 seconds | Per address |
| `faucet_requestTokens` | 1 req | `--faucet-cooldown` (default 3600s) | Per address |
| WebSocket connections | 10 simultaneous | — | Per IP |
| WebSocket messages | 50 msg | 1 second | Per connection |

---

## 429 Response Handling

When a rate limit is enforced (via a reverse proxy or future middleware), the response will be:

```http
HTTP/1.1 429 Too Many Requests
Retry-After: 5
Content-Type: application/json

{
  "jsonrpc": "2.0",
  "id": null,
  "error": {
    "code": -32019,
    "message": "Rate limit exceeded",
    "data": {
      "retry_after_seconds": 5,
      "limit": 100,
      "window_seconds": 10
    }
  }
}
```

**Client behaviour:**
1. Read `Retry-After` header (seconds to wait).
2. Implement exponential backoff with jitter on repeated 429s.
3. Never retry immediately — this worsens the congestion.

---

## Configuring Rate Limits (Reverse Proxy)

Until native rate limiting is implemented, use a reverse proxy such as Nginx or Caddy:

### Nginx Example

```nginx
limit_req_zone $binary_remote_addr zone=rpc:10m rate=10r/s;

location / {
    limit_req zone=rpc burst=20 nodelay;
    limit_req_status 429;
    proxy_pass http://127.0.0.1:9933;
}
```

### Caddy Example

```caddyfile
rpc.example.com {
    rate_limit {
        zone dynamic {
            key    {remote_host}
            events 100
            window 10s
        }
    }
    reverse_proxy localhost:9933
}
```

### CORS and browser preflight

Cross-origin browser clients send a **CORS preflight** (`OPTIONS`) before the JSON-RPC **`POST`**. The node applies CORS middleware before other gates so preflight succeeds when traffic reaches the app.

If you terminate TLS or rate-limit in **Nginx** (or similar):

- **Prefer** forwarding **`OPTIONS` and `POST`** to `127.0.0.1:9933` unchanged so the node can attach `Access-Control-*` headers consistently with the actual `POST` response.
- If the proxy **answers `OPTIONS` itself** (e.g. `return 204`), it **must** still emit matching **`Access-Control-Allow-Origin`**, **`Access-Control-Allow-Methods`**, **`Access-Control-Allow-Headers`**, and usually **`Access-Control-Max-Age`**. A bare `204` without those headers breaks the browser.
- Avoid Nginx `if` blocks that return early for `OPTIONS` unless you duplicate the full CORS header set there.

---

## Faucet Cooldown

The faucet has a built-in per-address cooldown enforced by the node:

```
--faucet-cooldown <SECONDS>   (default: 3600)
```

The `faucet_requestTokens` response includes `cooldown_remaining` when a request is denied:

```json
{
  "success": false,
  "message": "Cooldown active",
  "cooldown_remaining": 2847
}
```

---

## Public vs. Private Endpoints

| Endpoint Type | Default Access | Recommendation |
|---------------|---------------|----------------|
| Read methods (`chain_getBlock`, `account_getBalance`, …) | Public | No auth required; rate limit by IP |
| Write methods (`transaction_submit`, `marketplace_submit*`) | Public | Rate limit strictly; consider API key for production |
| `faucet_requestTokens` | Public (testnet only) | Enable only with `--enable-faucet` on testnet |
| WebSocket | Public | Require `Auth` message; rate limit connections per IP |
| Prometheus metrics (`/metrics`) | Local only | **Do not expose publicly** — use VPN or firewall |
