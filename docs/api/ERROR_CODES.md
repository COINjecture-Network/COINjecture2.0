# Error Code Catalog

## JSON-RPC Standard Errors

These are standard JSON-RPC 2.0 error codes returned in the `error.code` field.

| Code | Constant | Description |
|------|----------|-------------|
| `-32700` | `PARSE_ERROR` | Invalid JSON received |
| `-32600` | `INVALID_REQUEST` | Not a valid JSON-RPC request |
| `-32601` | `METHOD_NOT_FOUND` | Method does not exist |
| `-32602` | `INVALID_PARAMS` | Invalid method parameters |
| `-32603` | `INTERNAL_ERROR` | Internal server error |

## COINjecture Application Errors

Custom error codes in the range `-32000` to `-32099`.

| Code | Name | Description | Recovery |
|------|------|-------------|----------|
| `-32001` | `NOT_FOUND` | Requested resource does not exist (block, transaction, account) | Use a different identifier |
| `-32002` | `INVALID_ADDRESS` | Malformed address (must be 64 hex chars / 32 bytes) | Check address format |
| `-32003` | `INVALID_SIGNATURE` | Transaction signature verification failed | Re-sign with the correct key |
| `-32004` | `INSUFFICIENT_BALANCE` | Account balance too low for transaction | Fund the account or reduce amount |
| `-32005` | `INVALID_NONCE` | Transaction nonce is incorrect (replay protection) | Fetch current nonce with `account_getNonce` |
| `-32006` | `TX_TOO_LARGE` | Transaction exceeds maximum size limit | Split into smaller transactions |
| `-32007` | `POOL_FULL` | Mempool is at capacity | Wait and retry; increase `--min-fee` |
| `-32008` | `FEE_TOO_LOW` | Transaction fee below minimum (`--min-fee`) | Increase transaction fee |
| `-32009` | `PROBLEM_NOT_FOUND` | Problem ID does not exist in marketplace | Check the problem ID |
| `-32010` | `PROBLEM_EXPIRED` | Problem bounty has expired | Problem is no longer active |
| `-32011` | `INVALID_SOLUTION` | Submitted solution does not satisfy the problem | Verify your solution |
| `-32012` | `SOLUTION_ALREADY_SUBMITTED` | A correct solution was already accepted | Problem already solved |
| `-32013` | `FAUCET_COOLDOWN` | Faucet cooldown period not elapsed | Wait for `cooldown_remaining` seconds |
| `-32014` | `FAUCET_DISABLED` | Testnet faucet not enabled (`--enable-faucet`) | Enable faucet or use a different endpoint |
| `-32015` | `CHAIN_SYNCING` | Node is still syncing; data may be incomplete | Wait for sync to complete |
| `-32016` | `INVALID_BLOCK_VERSION` | Block version not supported | Update client or check network version |
| `-32017` | `COMMITMENT_MISMATCH` | Commitment does not match revealed problem | Re-commit with correct data |
| `-32018` | `UNAUTHORIZED` | Client not authenticated (WebSocket only) | Send `Auth` message first |
| `-32019` | `RATE_LIMITED` | Request rate limit exceeded | Reduce request rate; see RATE_LIMITS.md |
| `-32020` | `FEATURE_NOT_AVAILABLE` | Feature not available on this node type | Connect to a different node type |

## HTTP Status Codes

The HTTP layer wraps JSON-RPC. These HTTP codes indicate transport-level issues:

| HTTP Code | Meaning |
|-----------|---------|
| `200` | Request processed (check JSON body for RPC-level errors) |
| `400` | Malformed HTTP request body |
| `429` | Rate limit exceeded — see `Retry-After` header |
| `503` | Node unavailable (starting up, shutting down, or overloaded) |

## Error Response Shape

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32001,
    "message": "Block not found at height 99999",
    "data": {
      "height": 99999
    }
  }
}
```

The `data` field is optional and provides additional context where available.

## Finding Error Codes in Source

Error codes are defined in `rpc/src/server.rs`:

```rust
const INVALID_PARAMS: i32 = -32602;
const INTERNAL_ERROR: i32 = -32603;
const NOT_FOUND: i32     = -32001;
```

Application-specific codes above (`-32002` to `-32020`) are application-level and constructed inline at the relevant RPC handler using `ErrorObjectOwned::owned(code, message, data)`.
