# WebSocket API

## Endpoint

```
ws://{ws_addr}    (default: 0.0.0.0:8080)
```

The WebSocket server accepts standard RFC 6455 connections. All messages are JSON-encoded UTF-8 text frames.

---

## Message Format

Messages use a tagged-union format with a `type` field for routing:

```json
{ "type": "<message_type>", ...fields }
```

All field names use `snake_case`.

---

## Authentication

WebSocket authentication is optional but required for write operations (`GetWork`, `SubmitWork`).

### Auth Flow

1. Connect to the WebSocket endpoint.
2. Send an `Auth` message:

```json
{
  "type": "auth",
  "client_id": "my-miner-1",
  "signature": [12, 34, 56, ...]
}
```

The `signature` is the raw Ed25519 signature bytes (64 bytes) over the UTF-8 bytes of `client_id`, encoded as a JSON array.

3. Receive `AuthResponse`:

```json
{
  "type": "auth_response",
  "success": true,
  "message": "Authenticated"
}
```

**Access levels:**
- Unauthenticated: `GetStatus`, `GetBalance`, `GetBlock`, `GetTransaction` (read-only)
- Authenticated: all above + `GetWork`, `SubmitWork`, `SubmitTransaction`

---

## Client → Server Messages

### `get_status`

Query the current chain status.

```json
{ "type": "get_status" }
```

**Response:** [`status_response`](#status_response)

---

### `get_balance`

Query the balance of an address.

```json
{
  "type": "get_balance",
  "address": "a1b2c3...64hexchars"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `address` | string | 64-character hex address (32 bytes) |

**Response:** [`balance_response`](#balance_response)

---

### `get_block`

Query a block by height.

```json
{
  "type": "get_block",
  "height": 42
}
```

| Field | Type | Description |
|-------|------|-------------|
| `height` | u64 | Block height |

**Response:** [`block_response`](#block_response)

---

### `get_transaction`

Query a transaction by hash.

```json
{
  "type": "get_transaction",
  "tx_hash": "deadbeef...64hexchars"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `tx_hash` | string | 64-character hex transaction hash |

**Response:** [`transaction_response`](#transaction_response)

---

### `submit_transaction`

Submit a signed transaction. Requires authentication.

```json
{
  "type": "submit_transaction",
  "transaction": "{...json-encoded transaction...}"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `transaction` | string | JSON-encoded `Transaction` object |

**Response:** [`error`](#error) on failure; no response on success (transaction enters mempool).

---

### `get_work` *(requires auth)*

Request mining work from the node.

```json
{ "type": "get_work" }
```

**Response:** [`work_response`](#work_response) or [`error`](#error) if no work is available.

---

### `submit_work` *(requires auth)*

Submit a proof-of-work solution.

```json
{
  "type": "submit_work",
  "work_id": 7,
  "solution": [0, 1, 2, ...],
  "nonce": 123456789
}
```

| Field | Type | Description |
|-------|------|-------------|
| `work_id` | u64 | Work ID from the `work_response` |
| `solution` | byte array | Serialized solution bytes |
| `nonce` | u64 | Proof-of-work nonce |

**Response:** Asynchronous — node sends [`reward_notification`](#reward_notification) if accepted.

---

## Server → Client Messages

### `auth_response`

```json
{
  "type": "auth_response",
  "success": true,
  "message": "Authenticated"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `success` | bool | Whether authentication succeeded |
| `message` | string | Human-readable status |

---

### `status_response`

```json
{
  "type": "status_response",
  "best_height": 10042,
  "best_hash": "a1b2c3...64hexchars",
  "peer_count": 8,
  "sync_progress": 1.0
}
```

| Field | Type | Description |
|-------|------|-------------|
| `best_height` | u64 | Current chain tip height |
| `best_hash` | string | Hash of the best block |
| `peer_count` | usize | Number of connected peers |
| `sync_progress` | f64 | 0.0–1.0; 1.0 = fully synced |

---

### `balance_response`

```json
{
  "type": "balance_response",
  "address": "a1b2c3...64hexchars",
  "balance": 1000000,
  "pending": 5000
}
```

| Field | Type | Description |
|-------|------|-------------|
| `address` | string | Queried address |
| `balance` | u128 | Confirmed balance (smallest unit) |
| `pending` | u128 | Pending balance from unconfirmed txs |

---

### `block_response`

```json
{
  "type": "block_response",
  "block": "{...json-encoded Block...}"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `block` | string | JSON-encoded `Block` object |

---

### `transaction_response`

```json
{
  "type": "transaction_response",
  "transaction": "{...json-encoded Transaction...}",
  "confirmations": 12
}
```

| Field | Type | Description |
|-------|------|-------------|
| `transaction` | string | JSON-encoded `Transaction` object |
| `confirmations` | u64 | Number of block confirmations |

---

### `work_response`

```json
{
  "type": "work_response",
  "work_id": 7,
  "problem": "{...json-encoded ProblemType...}",
  "difficulty": 0.42,
  "reward": 5000000,
  "expires_at": 1735000000
}
```

| Field | Type | Description |
|-------|------|-------------|
| `work_id` | u64 | Unique work identifier |
| `problem` | string | JSON-encoded `ProblemType` |
| `difficulty` | f64 | Current difficulty target |
| `reward` | u128 | Block reward amount |
| `expires_at` | i64 | Unix timestamp when work expires |

---

### `submit_response`

Sent when work submission outcome is immediately known (e.g., invalid work ID).

```json
{
  "type": "submit_response",
  "accepted": false,
  "message": "Invalid work ID",
  "reward": null
}
```

| Field | Type | Description |
|-------|------|-------------|
| `accepted` | bool | Whether the submission was accepted |
| `message` | string | Human-readable outcome |
| `reward` | u128 \| null | Reward amount if accepted |

---

### `reward_notification` *(push)*

Sent asynchronously when a submitted solution is validated and reward is issued.

```json
{
  "type": "reward_notification",
  "amount": 5000000,
  "block_height": 10043
}
```

---

### `new_block` *(push)*

Broadcast to all connected clients when a new block is added.

```json
{
  "type": "new_block",
  "height": 10043,
  "hash": "a1b2c3...64hexchars"
}
```

---

### `error`

```json
{
  "type": "error",
  "code": 404,
  "message": "No work available"
}
```

| Code | Meaning |
|------|---------|
| 400 | Bad request / invalid message |
| 401 | Unauthorized (send `auth` first) |
| 404 | Resource not found |
| 429 | Rate limit exceeded |

---

## Connection Limits

| Limit | Value |
|-------|-------|
| Max simultaneous connections per IP | 10 |
| Max messages per connection per second | 50 |
| Idle timeout (no messages) | 5 minutes |

---

## Keepalive

The server does not send explicit ping frames. Clients should send any message (e.g., `get_status`) at least once every 4 minutes to avoid the idle timeout. Standard WebSocket ping/pong frames are also supported.

---

## Example Session (JavaScript)

```javascript
const ws = new WebSocket('ws://127.0.0.1:8080');

ws.onopen = () => {
  // Authenticate
  ws.send(JSON.stringify({
    type: 'auth',
    client_id: 'my-miner',
    signature: Array.from(signatureBytes),
  }));
};

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  switch (msg.type) {
    case 'auth_response':
      if (msg.success) ws.send(JSON.stringify({ type: 'get_work' }));
      break;
    case 'work_response':
      const solution = solve(msg.problem);
      ws.send(JSON.stringify({
        type: 'submit_work',
        work_id: msg.work_id,
        solution: Array.from(solution),
        nonce: computeNonce(),
      }));
      break;
    case 'reward_notification':
      console.log(`Reward: ${msg.amount} at block ${msg.block_height}`);
      ws.send(JSON.stringify({ type: 'get_work' }));
      break;
    case 'new_block':
      console.log(`New block: ${msg.height} (${msg.hash})`);
      break;
  }
};
```
