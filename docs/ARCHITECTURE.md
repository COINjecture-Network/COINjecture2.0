# COINjecture 2.0 Architecture

## System Overview

COINjecture 2.0 is a Proof-of-Useful-Work (PoUW) Layer 1 blockchain built in Rust. Instead of wasteful hash grinding, miners solve real NP-complete problems (SubsetSum, SAT, TSP) submitted through an on-chain marketplace. Solutions are verified in polynomial time, and bounties are paid atomically in the same block. The protocol uses the equilibrium constant eta = 1/sqrt(2) throughout its design — from dimensional pool economics to network routing fanout.

## CPP Protocol (COINjecture P2P Protocol)

Custom TCP wire protocol on port 707, replacing libp2p.

### Wire Format

```
┌────────────┬─────────┬──────────┬─────────────┬─────────┬──────────┐
│ MAGIC (4B) │ VER (1B)│ TYPE (1B)│ LENGTH (4B) │ PAYLOAD │ HASH(32B)│
│  "COIN"    │   0x01  │  0x01-FF │  LE uint32  │  N bytes│  blake3  │
└────────────┴─────────┴──────────┴─────────────┴─────────┴──────────┘
```

### Message Types (17 total)

| Code | Type | Category | Description |
|------|------|----------|-------------|
| 0x01 | Hello | Handshake | Initial connection |
| 0x02 | HelloAck | Handshake | Handshake acknowledgment |
| 0x10 | Status | Sync | Peer status (height, hash, node type) |
| 0x11 | GetBlocks | Sync | Request blocks by height range |
| 0x12 | Blocks | Sync | Block response batch |
| 0x13 | GetHeaders | Sync | Request headers (light clients) |
| 0x14 | Headers | Sync | Header response |
| 0x20 | NewBlock | Propagation | Newly mined block |
| 0x21 | NewTransaction | Propagation | New transaction |
| 0x30 | SubmitWork | Light Mining | Light client PoW submission |
| 0x31 | WorkAccepted | Light Mining | Work accepted |
| 0x32 | WorkRejected | Light Mining | Work rejected |
| 0x33 | GetWork | Light Mining | Request mining template |
| 0x34 | Work | Light Mining | Mining template response |
| 0xF0 | Ping | Control | Keep-alive |
| 0xF1 | Pong | Control | Keep-alive response |
| 0xFF | Disconnect | Control | Graceful disconnect |

### EquilibriumRouter

Broadcast fanout: `ceil(sqrt(n) * eta)` where n = connected peers, eta = 1/sqrt(2).

Peer selection considers:
- Connection quality (0.0-1.0, decays by `1 - eta` on failure)
- Block height (dimensional distance for sync peer selection)
- Flock phase (Reynolds murmuration coordination)

### FlockState (Murmuration)

Peers coordinate using Reynolds flocking rules:
- **Separation**: Avoid broadcasting to peers too close in the peer graph
- **Alignment**: Prefer peers with similar flock phase
- **Cohesion**: Maintain connectivity to the swarm center

Epochs advance with chain state updates. Phases (0-7) rotate for broadcast diversity.

### Flow Control

Window-based congestion control:
- Additive increase by eta on success
- Multiplicative decrease by eta on congestion
- Maximum message size: 10 MB
- Max blocks per sync response: 16

### Timeouts (eta-derived)

| Constant | Value | Derivation |
|----------|-------|------------|
| Network peer timeout | 90s | Base |
| Consensus peer timeout | ~127s | 90 / eta |
| Consensus stale threshold | ~153s | 90 * (1 + eta) |
| Handshake timeout | 10s | - |
| Keep-alive interval | 30s | - |
| Message read timeout | 30s | - |

## Consensus — Proof of Useful Work

### Mining Flow

1. Miner requests open problems from marketplace via RPC
2. Miner solves NP-complete problem (exponential time)
3. Miner submits solution transaction
4. Block validator verifies solution (polynomial time)
5. Work score calculated; bounty released from escrow atomically

### Work Score

```
score = (solve_time / verify_time) * sqrt(solve_memory / verify_memory) * problem_weight * quality * energy_efficiency
```

### Difficulty Adjustment

Adaptive difficulty targeting 60-second block time (configurable via `--block-time`).

## State Layer — redb

ACID-compliant embedded database (pure Rust). All state stored in typed tables with explicit transaction boundaries.

### Tables

| Table | Purpose |
|-------|---------|
| BALANCES_TABLE | Account balances |
| NONCES_TABLE | Account nonces |
| PROBLEMS_TABLE | Marketplace problem metadata & solutions |
| ESCROW_TABLE | Marketplace bounty escrow |
| PROBLEM_INDEX | Fast lookup by submitter address |
| POOL_LIQUIDITY_TABLE | Dimensional pool state |
| SWAP_RECORDS_TABLE | Swap history |
| TIMELOCKS_TABLE | Time-locked balances |
| ESCROWS_TABLE | Multi-party escrow |
| CHANNELS_TABLE | Payment channels |
| TRUSTLINES_TABLE | XRPL-inspired credit lines |

### Chain Storage

Block headers and bodies stored in separate redb tables. Block retrieval by height or hash.

## Marketplace

### Problem Lifecycle

```
OPEN → SOLVED   (valid solution + work score check → bounty auto-paid)
OPEN → EXPIRED  (expiration deadline → bounty refunded)
OPEN → CANCELLED (submitter cancels → bounty refunded)
```

### Supported Problem Types

- **SubsetSum**: Find subset summing to target. Verification: O(n)
- **Boolean SAT**: Satisfiability. Verification: O(n*m)
- **TSP**: Traveling salesman. Verification: O(n^2)
- **Custom**: User-defined with pluggable verification

### Escrow

Bounty funds are escrowed on-chain at problem submission. Released atomically to solver in the same block as solution verification.

## Dimensional Pools

Three economic tiers governed by eta = 1/sqrt(2):

| Pool | tau | D_n = e^(-eta*tau) | Allocation |
|------|-----|-------------------|------------|
| D1 Genesis | 0.00 | 1.000 | 56.1% |
| D2 Coupling | 0.20 | 0.867 | 48.6% |
| D3 First Harmonic | 0.41 | 0.750 | 42.1% |

Swap formula: `amount_out = amount_in * (D_from / D_to)`

Unit circle constraint: `|mu|^2 = eta^2 + lambda^2 = 1` (critical damping).

## RPC

JSON-RPC server on port 9933 (HTTP) and 8080 (WebSocket).

### Key Endpoints

| Method | Description |
|--------|-------------|
| chain_getInfo | Chain height, hash, identity |
| chain_getBlock | Get block by height or hash |
| account_getBalance | Account balance |
| account_getNonce | Account nonce |
| transaction_submit | Submit signed transaction |
| marketplace_getOpenProblems | List open marketplace problems |
| marketplace_getProblem | Get specific problem by ID |
| marketplace_getStats | Marketplace statistics |
| pool_getLiquidity | Dimensional pool state |

### Health & Metrics

- `GET /health` on metrics port (default 9090)
- `GET /metrics` for Prometheus scraping

## Crate Map

| Crate | Path | Purpose |
|-------|------|---------|
| coinject-core | core/ | Cryptography, block/transaction types, problem definitions |
| coinject-state | state/ | redb state management (accounts, marketplace, pools) |
| coinject-consensus | consensus/ | PoUW mining, work score, difficulty adjustment |
| coinject-network | network/ | CPP protocol implementation |
| coinject-mempool | mempool/ | Transaction pool, fee market |
| coinject-rpc | rpc/ | JSON-RPC server |
| coinject-tokenomics | tokenomics/ | Dimensional math, reward distribution |
| coinject (node) | node/ | Full node binary, service orchestration |
| coinject-wallet | wallet/ | CLI wallet, keystore |
| adzdb | adzdb/ | Experimental custom database backend |
| marketplace-export | marketplace-export/ | Dataset export tooling |
| huggingface | huggingface/ | HuggingFace dataset integration |
| mobile-sdk | mobile-sdk/ | Mobile SDK (experimental) |

## Ports

| Port | Service |
|------|---------|
| 707 | CPP P2P (TCP) |
| 8080 | WebSocket RPC |
| 9090 | Metrics + Health |
| 9933 | JSON-RPC |
