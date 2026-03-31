# COINjecture Load Testing & Stress Testing

## Overview

The `load-test` crate provides a CLI harness for performance, stress, and stability testing of COINjecture nodes.  All tests communicate with the node via its JSON-RPC endpoint and/or CPP TCP port.

---

## Building

```sh
cargo build --release -p load-test
```

Binary: `target/release/load-test`

---

## Test Commands

### 1. Transaction Flood (`tx-flood`)

Generates and submits transactions at configurable TPS with multiple signing keys to simulate a realistic multi-sender workload.

```sh
load-test tx-flood \
  --rpc http://127.0.0.1:9933 \
  --tps 100 \
  --duration 60 \
  --keys 20 \
  --amount 1
```

**Options:**
| Flag | Default | Description |
|------|---------|-------------|
| `--rpc` | `http://127.0.0.1:9933` | RPC endpoint |
| `--tps` | 50 | Target transactions per second |
| `--duration` | 60 | Duration in seconds |
| `--keys` | 10 | Number of signing keypairs |
| `--amount` | 1 | Transfer amount per tx (base units) |

**Pass criteria:** actual TPS ≥ 80% of target AND error rate < 5%.

---

### 2. Mempool Flood (`mempool-flood`)

Submits more transactions than the mempool can hold to test capacity enforcement and fee market behavior under congestion.

```sh
load-test mempool-flood \
  --rpc http://127.0.0.1:9933 \
  --count 10000 \
  --concurrency 20
```

**Pass criteria:** node stays responsive; capacity rejections (error code -32603) are expected; unexpected errors < 5%.

---

### 3. RPC Blast (`rpc-blast`)

Hits all RPC endpoints concurrently for a sustained period.

```sh
load-test rpc-blast \
  --rpc http://127.0.0.1:9933 \
  --concurrency 50 \
  --duration 30
```

**Endpoints tested:**
- `chain_getBlockNumber`, `chain_getBlockHash`, `chain_getBlock`, `chain_getBestBlock`
- `net_peerCount`, `net_version`
- `system_health`, `system_name`, `system_version`
- `mempool_size`, `mempool_pendingTxs`
- `rpc_methods`

**Pass criteria:** error rate < 10% and at least one successful response.

---

### 4. Stability Test (`stability`)

Runs the node under moderate load for an extended period while monitoring memory, block production, and responsiveness.

```sh
load-test stability \
  --rpc http://127.0.0.1:9933 \
  --duration 3600 \
  --tps 10 \
  --sample-interval 60
```

**Memory leak detection:** fails if process RSS grows > 100% over the test duration.

**Pass criteria:**
- No RPC outages during monitoring
- Memory growth < 100%
- TX error rate < 20%
- At least one block produced

---

### 5. Network Stress (`network-stress`)

Connects many simulated CPP peers to the P2P port to test connection handling and peer limit enforcement.

```sh
load-test network-stress \
  --target 127.0.0.1:707 \
  --peers 100 \
  --duration 30
```

Each simulated peer completes a valid CPP Hello handshake and holds the connection open for the duration.

**Pass criteria:** node doesn't crash; some connections accepted; rejected connections expected once `MAX_PEERS` is reached.

---

### 6. Large Block Test (`large-block`)

Fills the mempool with transactions then waits for the node to mine a full block, measuring:
- Transactions included per block
- Block propagation/mining latency
- Node stability with a large block

```sh
load-test large-block \
  --rpc http://127.0.0.1:9933 \
  --tx-count 1000
```

**Pass criteria:** at least one block mined with transactions within 120 seconds.

---

### 7. Recovery Test (`recovery`)

Tests that a node recovers cleanly from a crash:

```sh
load-test recovery \
  --rpc http://127.0.0.1:9933 \
  --restart-wait 10 \
  --restart-cmd "systemctl start coinject-node"
```

**Steps:**
1. Record pre-crash block height
2. Execute restart command (or wait for manual restart)
3. Wait up to 60s for node to come back
4. Verify post-restart height ≥ pre-crash height

**Pass criteria:** node returns with block height ≥ pre-crash height within 60 seconds.

---

## Output Formats

### Human-readable (default)

```
═══════════════════════════════════════════════════════════
 Load Test: tx-flood  [PASS]
═══════════════════════════════════════════════════════════
 Duration : 60.2s
 Summary  : Submitted 5987 txs in 60.2s — actual 99.4 TPS, 0.3% errors

 Metrics:
   config.duration_secs                    60.00  s
   config.keys                             10.00  count
   config.tps                             100.00  ops/s
   tx.error_rate                            0.30  %
   tx.latency.mean_ms                       4.21  ms
   tx.latency.p95_ms                       12.80  ms
   tx.latency.p99_ms                       28.40  ms
   tx.total                              5987.00  ops
   tx.tps                                  99.45  ops/s
═══════════════════════════════════════════════════════════
```

### JSON output

```sh
load-test tx-flood --rpc http://127.0.0.1:9933 --tps 100 --duration 60 --json
```

```sh
load-test tx-flood --rpc http://127.0.0.1:9933 --tps 100 --json --output results.json
```

---

## Load Test Results Template

Use this template when documenting test results manually:

```markdown
## Load Test Results — [DATE]

**Environment:**
- Node version: X.X.X
- OS: [Windows/Linux/macOS]
- CPU: [model, cores]
- RAM: [GB]
- Storage: [type, free space]
- Network: [local/remote, bandwidth]

**Test: tx-flood**
- Target TPS: 100
- Duration: 60s
- Keys: 20
- Result: PASS/FAIL
- Actual TPS: ___
- Error rate: ___%
- P50 latency: ___ms | P95: ___ms | P99: ___ms

**Test: mempool-flood**
- Count: 10000
- Concurrency: 20
- Result: PASS/FAIL
- Accepted: ___ | Rejected (capacity): ___ | Other errors: ___

**Test: rpc-blast**
- Concurrency: 50
- Duration: 30s
- Result: PASS/FAIL
- Throughput: ___ req/s
- Error rate: ___%
- P95 latency: ___ms

**Test: stability**
- Duration: 3600s
- TPS: 10
- Result: PASS/FAIL
- Blocks produced: ___
- Peak memory: ___MB
- Memory leak detected: YES/NO
- RPC outages: ___

**Test: network-stress**
- Peers: 100
- Duration: 30s
- Result: PASS/FAIL
- Connected: ___ | Rejected: ___ | Timeout: ___

**Test: large-block**
- TX count: 1000
- Result: PASS/FAIL
- Block height: ___
- TXs included: ___
- Block size: ___KB
- Mining latency: ___s

**Test: recovery**
- Restart wait: 10s
- Result: PASS/FAIL
- Pre-crash height: ___
- Post-restart height: ___
- Recovery time: ___s

**Notes:**
- [observations, anomalies, follow-up actions]
```

---

## Interpreting Results

| Metric | Healthy | Warning | Critical |
|--------|---------|---------|----------|
| TPS throughput | ≥ target | 80–100% | < 80% |
| TX error rate | < 1% | 1–5% | > 5% |
| RPC P95 latency | < 50ms | 50–200ms | > 200ms |
| Memory growth (stability) | < 20% | 20–100% | > 100% |
| Recovery time | < 30s | 30–60s | > 60s |
| RPC outages (stability) | 0 | 1–2 | > 2 |

---

## Disk Growth Monitoring

During long-running tests, monitor database growth with:

```sh
# Linux/macOS
watch -n 30 "du -sh data/ && df -h ."

# Windows PowerShell
while ($true) {
    Get-Item data | Select-Object FullName, @{n='SizeMB';e={(Get-ChildItem $_.FullName -Recurse | Measure-Object -Property Length -Sum).Sum / 1MB}}
    Start-Sleep 30
}
```

Expected growth rate: ~1–5 MB/1000 blocks depending on transaction density.

---

## Code Location

| Component | File |
|-----------|------|
| CLI entry point | `load-test/src/main.rs` |
| Transaction generator | `load-test/src/tx_generator.rs` |
| Mempool flood | `load-test/src/mempool_flood.rs` |
| RPC load | `load-test/src/rpc_load.rs` |
| Network stress | `load-test/src/network_stress.rs` |
| Large block | `load-test/src/large_block.rs` |
| Stability & recovery | `load-test/src/stability.rs` |
| Health monitor | `load-test/src/monitor.rs` |
| Results / reporting | `load-test/src/results.rs` |
