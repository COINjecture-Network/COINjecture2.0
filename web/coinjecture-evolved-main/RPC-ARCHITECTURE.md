# RPC Architecture

## Overview

The COINjecture frontend communicates directly with the RPC nodes over HTTPS.
There is no intermediate proxy layer (no Lambda@Edge, no CloudFront origin forwarding).

## Request Flow

```
Browser (HTTPS)
  → rpc1.coinjecture.com  (TLS termination on the node)
  → rpc2.coinjecture.com  (fallback / parallel)
  → rpc3.coinjecture.com  (fallback / parallel)
```

## Environment Configuration

| Variable | Example | Description |
|---|---|---|
| `VITE_RPC_URL` | `https://rpc1.coinjecture.com,https://rpc2.coinjecture.com` | Comma-separated list of RPC endpoint URLs |
| `VITE_API_URL` | `https://api.coinjecture.com` | Backend REST API (metrics, chain summary) |

### Development

Set `VITE_RPC_URL` to comma-separated HTTPS URLs, or rely on the Vite dev-server proxy
(`/api/rpc` → first entry in `VITE_RPC_URL`) to avoid CORS during local development.

### Production

Set `VITE_RPC_URL` to the public HTTPS RPC domains.  The client maps legacy IP addresses to
their canonical domain names automatically (see `createProxyUrls` in `src/lib/rpc-client.ts`).

## Multi-Node Support

`src/lib/rpc-client.ts` provides two call strategies:

| Function | Behaviour |
|---|---|
| `call()` | Tries nodes in order; returns first successful response |
| `callAll()` | Sends to all nodes in parallel; returns the best result |

CORS headers (`Access-Control-Allow-Origin: *`) must be enabled on the RPC servers.
