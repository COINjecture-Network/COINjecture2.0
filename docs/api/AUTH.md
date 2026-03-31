# Authentication Guide

## JSON-RPC (HTTP)

The HTTP JSON-RPC API is **unauthenticated by default**.  All requests are accepted as long as the client can reach the endpoint.

For production deployments, authentication should be handled at the reverse-proxy layer using one of:

- **IP allowlisting** — restrict access to known client IPs via firewall or Nginx `allow`/`deny`
- **Basic Auth** — Nginx or Caddy can require HTTP Basic Authentication before proxying
- **TLS client certificates** — mutual TLS (mTLS) for strict client identity

The node itself does not issue or verify API keys today.

---

## WebSocket Authentication

The WebSocket endpoint (`ws://{ws_addr}`, default port `8080`) supports an optional application-level authentication handshake.

### Auth Flow

1. Client connects to WebSocket.
2. Client sends an `Auth` message:

```json
{
  "type": "auth",
  "client_id": "my-miner-node-1",
  "signature": "<base64-encoded Ed25519 signature of client_id bytes>"
}
```

3. Server responds with `AuthResponse`:

```json
{
  "type": "auth_response",
  "success": true,
  "message": "Authenticated as my-miner-node-1"
}
```

If authentication fails:
```json
{
  "type": "auth_response",
  "success": false,
  "message": "Invalid signature"
}
```

4. Authenticated clients can access mining work via `GetWork` / `SubmitWork`.  Unauthenticated clients can still use read-only messages (`GetStatus`, `GetBalance`, `GetBlock`, `GetTransaction`).

### Signature Construction

The `signature` is an Ed25519 signature over the raw UTF-8 bytes of `client_id`, using the miner's private key:

```rust
// Rust example
let keypair = KeyPair::generate(); // or load from keystore
let sig = keypair.sign(client_id.as_bytes());
let sig_b64 = base64::encode(sig.to_bytes());
```

```javascript
// JavaScript example (using @noble/ed25519)
import { sign } from '@noble/ed25519';
const sig = await sign(Buffer.from(clientId), privateKey);
const sigB64 = Buffer.from(sig).toString('base64');
```

---

## Transaction Signing

Transactions are signed by the sender's Ed25519 private key before submission.  The node verifies the signature on receipt.

**The node never handles private keys directly.** Signing always happens client-side.

### Signing a Transfer

```rust
let keypair = KeyPair::from_bytes(private_key_bytes); // load from keystore
let tx = Transaction::new_transfer(from, to, amount, fee, nonce, &keypair);
let tx_hex = hex::encode(bincode::serialize(&tx)?);

// Submit via RPC
rpc.submit_transaction(tx_hex).await?;
```

```javascript
// JavaScript (see SDK_EXAMPLES.md for full example)
const tx = buildTransferTransaction({ from, to, amount, fee, nonce, privateKey });
await rpc.call('transaction_submit', [tx.toHex()]);
```

---

## Key Management

Keys are stored in the node keystore at `{data_dir}/keystore/`:

```
data/
└── keystore/
    ├── validator.key     ← validator signing key (encrypted)
    └── miner.key         ← miner reward address key
```

The wallet CLI manages keys:

```bash
# Generate a new key
coinject-wallet account create

# List keys
coinject-wallet account list

# Export public key (for RPC auth)
coinject-wallet account export-pubkey <address>
```

Keys are encrypted at rest using the passphrase provided on creation.  Use `--keystore-passphrase` or the `COINJECT_KEYSTORE_PASSPHRASE` environment variable.

---

## HTTPS / TLS

To expose the RPC over HTTPS, terminate TLS at a reverse proxy.  Example with Caddy:

```caddyfile
rpc.example.com {
    reverse_proxy localhost:9933
    tls /path/to/cert.pem /path/to/key.pem
}
```

The node does not manage TLS certificates directly.
