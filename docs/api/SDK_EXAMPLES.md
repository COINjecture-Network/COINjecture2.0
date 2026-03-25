# SDK Examples

Common operations using the COINjecture RPC API in Rust and JavaScript.

## Setup

### Rust

```toml
# Cargo.toml
[dependencies]
reqwest = { version = "0.12", features = ["json"] }
serde_json = "1"
hex = "0.4"
tokio = { version = "1", features = ["full"] }
tokio-tungstenite = "0.24"
```

### JavaScript / Node.js

```bash
npm install ws @noble/ed25519 @noble/hashes
```

---

## JSON-RPC Helper

### Rust

```rust
use serde_json::{json, Value};

async fn rpc_call(
    client: &reqwest::Client,
    url: &str,
    method: &str,
    params: Value,
) -> anyhow::Result<Value> {
    let body = json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
        "id": 1,
    });
    let resp = client.post(url).json(&body).send().await?;
    let json: Value = resp.json().await?;
    if let Some(err) = json.get("error") {
        anyhow::bail!("RPC error: {}", err);
    }
    Ok(json["result"].clone())
}
```

### JavaScript

```javascript
async function rpcCall(url, method, params = []) {
  const resp = await fetch(url, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ jsonrpc: '2.0', method, params, id: 1 }),
  });
  const json = await resp.json();
  if (json.error) throw new Error(`RPC error ${json.error.code}: ${json.error.message}`);
  return json.result;
}
```

---

## Chain Queries

### Get Chain Info

**Rust:**
```rust
let info = rpc_call(&client, RPC_URL, "chain_getInfo", json!([])).await?;
println!("Height: {}", info["best_height"]);
println!("Syncing: {}", info["is_syncing"]);
```

**JavaScript:**
```javascript
const info = await rpcCall(RPC_URL, 'chain_getInfo');
console.log(`Height: ${info.best_height}, syncing: ${info.is_syncing}`);
```

### Get Block by Height

**Rust:**
```rust
let block = rpc_call(&client, RPC_URL, "chain_getBlock", json!([42])).await?;
println!("{}", serde_json::to_string_pretty(&block)?);
```

**JavaScript:**
```javascript
const block = await rpcCall(RPC_URL, 'chain_getBlock', [42]);
console.log(block);
```

### Get Latest Block

**Rust:**
```rust
let block = rpc_call(&client, RPC_URL, "chain_getLatestBlock", json!([])).await?;
```

**JavaScript:**
```javascript
const block = await rpcCall(RPC_URL, 'chain_getLatestBlock');
```

---

## Account Operations

### Get Balance

**Rust:**
```rust
let address = "a1b2c3...64hexchars";
let balance = rpc_call(&client, RPC_URL, "account_getBalance", json!([address])).await?;
println!("Balance: {}", balance);
```

**JavaScript:**
```javascript
const balance = await rpcCall(RPC_URL, 'account_getBalance', [address]);
console.log(`Balance: ${balance}`);
```

### Get Nonce (for transaction construction)

**Rust:**
```rust
let nonce: u64 = rpc_call(&client, RPC_URL, "account_getNonce", json!([address]))
    .await?
    .as_u64()
    .unwrap_or(0);
```

**JavaScript:**
```javascript
const nonce = await rpcCall(RPC_URL, 'account_getNonce', [address]);
```

---

## Transactions

### Submit a Transfer (Rust, using coinject-core)

```rust
use coinject_core::{KeyPair, Transaction, Address};
use hex;

async fn send_transfer(
    client: &reqwest::Client,
    rpc_url: &str,
    keypair: &KeyPair,
    to_address: Address,
    amount: u128,
    fee: u128,
) -> anyhow::Result<String> {
    let from = keypair.public_address();

    // Fetch current nonce
    let nonce: u64 = rpc_call(client, rpc_url, "account_getNonce",
        json!([hex::encode(from.as_bytes())]))
        .await?
        .as_u64()
        .unwrap_or(0);

    // Build and sign transaction
    let tx = Transaction::new_transfer(from, to_address, amount, fee, nonce, keypair);
    let tx_hex = hex::encode(bincode::serialize(&tx)?);

    // Submit
    let tx_hash = rpc_call(client, rpc_url, "transaction_submit", json!([tx_hex]))
        .await?
        .as_str()
        .unwrap_or("")
        .to_string();

    Ok(tx_hash)
}
```

### Submit a Transfer (JavaScript)

```javascript
import * as ed from '@noble/ed25519';
import { blake3 } from '@noble/hashes/blake3';

function buildTransferTransaction({ from, to, amount, fee, nonce, privateKey }) {
  // Build transaction object
  const tx = {
    from: from,
    to: to,
    amount: amount.toString(),
    fee: fee.toString(),
    nonce: nonce,
    type: 'transfer',
  };

  // Sign: hash the canonical JSON, then Ed25519-sign the hash
  const msgBytes = new TextEncoder().encode(JSON.stringify(tx));
  const msgHash = blake3(msgBytes);
  const signature = ed.sign(msgHash, privateKey);

  return { ...tx, signature: Buffer.from(signature).toString('hex') };
}

async function sendTransfer({ from, to, amount, fee, privateKey }) {
  const nonce = await rpcCall(RPC_URL, 'account_getNonce', [from]);
  const tx = buildTransferTransaction({ from, to, amount, fee, nonce, privateKey });
  const txHash = await rpcCall(RPC_URL, 'transaction_submit', [JSON.stringify(tx)]);
  console.log(`Transaction submitted: ${txHash}`);
  return txHash;
}
```

### Poll Transaction Status

**Rust:**
```rust
async fn wait_for_confirmation(
    client: &reqwest::Client,
    rpc_url: &str,
    tx_hash: &str,
    timeout_secs: u64,
) -> anyhow::Result<()> {
    let deadline = std::time::Instant::now()
        + std::time::Duration::from_secs(timeout_secs);

    loop {
        let status = rpc_call(client, rpc_url, "transaction_getStatus",
            json!([tx_hash])).await?;

        match status["status"].as_str() {
            Some("confirmed") => {
                println!("Confirmed at block {}", status["block_height"]);
                return Ok(());
            }
            Some("failed") => anyhow::bail!("Transaction failed"),
            _ => {} // pending
        }

        if std::time::Instant::now() > deadline {
            anyhow::bail!("Timeout waiting for confirmation");
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}
```

**JavaScript:**
```javascript
async function waitForConfirmation(txHash, timeoutMs = 60000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const status = await rpcCall(RPC_URL, 'transaction_getStatus', [txHash]);
    if (status.status === 'confirmed') return status;
    if (status.status === 'failed') throw new Error('Transaction failed');
    await new Promise(r => setTimeout(r, 2000));
  }
  throw new Error('Timeout waiting for confirmation');
}
```

---

## Marketplace

### List Open Problems

**Rust:**
```rust
let problems = rpc_call(&client, RPC_URL, "marketplace_getOpenProblems", json!([])).await?;
for p in problems.as_array().unwrap_or(&vec![]) {
    println!("{} — bounty: {} expires: {}",
        p["problem_id"], p["bounty"], p["expires_at"]);
}
```

**JavaScript:**
```javascript
const problems = await rpcCall(RPC_URL, 'marketplace_getOpenProblems');
problems.forEach(p => {
  console.log(`${p.problem_id} — bounty: ${p.bounty}, expires: ${p.expires_at}`);
});
```

### Submit a Public Subset-Sum Problem

**Rust:**
```rust
let result = rpc_call(&client, RPC_URL, "marketplace_submitPublicSubsetSum", json!([{
    "submitter": submitter_hex,
    "set": [10, 20, 30, 40, 50],
    "target": 60,
    "bounty": 100000,
    "min_work_score": 0.1,
    "expiration_days": 7,
}])).await?;
println!("Problem ID: {}", result);
```

### Submit a Solution

**JavaScript:**
```javascript
const accepted = await rpcCall(RPC_URL, 'marketplace_submitSolution', [{
  problem_id: problemId,
  solver: solverAddress,
  solution_type: 'SubsetSum',
  solution_data: JSON.stringify({ indices: [0, 2, 4] }),
  work_score: 0.85,
}]);
console.log(`Solution accepted: ${accepted}`);
```

---

## Faucet (Testnet Only)

**Rust:**
```rust
let resp = rpc_call(&client, RPC_URL, "faucet_requestTokens",
    json!([my_address])).await?;
if resp["success"].as_bool().unwrap_or(false) {
    println!("Received {} tokens", resp["amount"]);
} else {
    println!("Cooldown: {}s remaining", resp["cooldown_remaining"]);
}
```

**JavaScript:**
```javascript
const resp = await rpcCall(RPC_URL, 'faucet_requestTokens', [myAddress]);
if (resp.success) {
  console.log(`Received ${resp.amount} tokens`);
} else {
  console.log(`Cooldown: ${resp.cooldown_remaining}s remaining`);
}
```

---

## WebSocket: Mining Client

### Rust (tokio-tungstenite)

```rust
use tokio_tungstenite::connect_async;
use futures::{SinkExt, StreamExt};
use serde_json::json;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (mut ws, _) = connect_async("ws://127.0.0.1:8080").await?;

    // Authenticate
    ws.send(tokio_tungstenite::tungstenite::Message::Text(
        json!({
            "type": "auth",
            "client_id": "rust-miner-1",
            "signature": sign_client_id("rust-miner-1"),
        }).to_string()
    )).await?;

    while let Some(msg) = ws.next().await {
        let text = msg?.into_text()?;
        let parsed: serde_json::Value = serde_json::from_str(&text)?;

        match parsed["type"].as_str() {
            Some("auth_response") if parsed["success"].as_bool() == Some(true) => {
                ws.send(tokio_tungstenite::tungstenite::Message::Text(
                    json!({ "type": "get_work" }).to_string()
                )).await?;
            }
            Some("work_response") => {
                let work_id = parsed["work_id"].as_u64().unwrap();
                let solution = mine(&parsed["problem"].as_str().unwrap())?;
                ws.send(tokio_tungstenite::tungstenite::Message::Text(
                    json!({
                        "type": "submit_work",
                        "work_id": work_id,
                        "solution": solution,
                        "nonce": 0u64,
                    }).to_string()
                )).await?;
            }
            Some("reward_notification") => {
                println!("Reward: {} at block {}", parsed["amount"], parsed["block_height"]);
                // Request next work
                ws.send(tokio_tungstenite::tungstenite::Message::Text(
                    json!({ "type": "get_work" }).to_string()
                )).await?;
            }
            Some("new_block") => {
                println!("New block: {} ({})", parsed["height"], parsed["hash"]);
            }
            _ => {}
        }
    }
    Ok(())
}
```

---

## Error Handling

Always check for the `error` field in JSON-RPC responses:

```javascript
try {
  const result = await rpcCall(RPC_URL, 'chain_getBlock', [999999999]);
} catch (err) {
  // err.message will be "RPC error -32001: Block not found at height 999999999"
  console.error(err.message);
}
```

For 429 (rate limit), implement exponential backoff:

```javascript
async function rpcCallWithRetry(url, method, params, maxRetries = 3) {
  for (let i = 0; i < maxRetries; i++) {
    try {
      return await rpcCall(url, method, params);
    } catch (err) {
      if (err.message.includes('-32019') && i < maxRetries - 1) {
        const delay = Math.pow(2, i) * 1000 + Math.random() * 500;
        await new Promise(r => setTimeout(r, delay));
      } else {
        throw err;
      }
    }
  }
}
```

---

## Constants Reference

| Constant | Value |
|----------|-------|
| Default RPC port | 9933 |
| Default WebSocket port | 8080 |
| Address length | 32 bytes / 64 hex chars |
| Hash length | 32 bytes / 64 hex chars |
| Balance unit | 1 (smallest denomination) |
| Max transaction size | see `-32006` error |
