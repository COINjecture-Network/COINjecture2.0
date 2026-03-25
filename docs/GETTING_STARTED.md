# Getting Started with COINjecture 2.0

A step-by-step guide for new developers: clone, build, run a local testnet, and make your first transaction.

---

## Prerequisites

| Tool | Version | Install |
|------|---------|---------|
| Rust | 1.88+ | [rustup.rs](https://rustup.rs/) |
| Docker | 24+ | [docker.com](https://www.docker.com/) |
| Docker Compose | v2 | Included with Docker Desktop |
| Git | any | [git-scm.com](https://git-scm.com/) |

Verify your setup:
```bash
rustc --version     # rustc 1.88.0 or later
cargo --version     # cargo 1.88.0 or later
docker --version    # Docker version 24+
```

---

## Step 1 — Clone the Repository

```bash
git clone https://github.com/Quigles1337/COINjecture2.0.git
cd COINjecture2.0
```

---

## Step 2 — Build

### Build all workspace crates

```bash
cargo build
```

This compiles all 13 crates. First build takes ~2–5 minutes; subsequent builds are incremental.

### Build release binaries

```bash
cargo build --release
```

Produces optimized binaries in `target/release/`:
- `coinject` — full node
- `coinject-wallet` — CLI wallet

### Verify the build

```bash
./target/release/coinject --help
./target/release/coinject-wallet --help
```

---

## Step 3 — Run Tests

Confirm everything works before making changes:

```bash
cargo test --workspace
```

Expected: **665 tests pass, 0 failures**.

To run tests for a specific crate:
```bash
cargo test -p coinject-core
cargo test -p coinject-consensus
cargo test -p coinject-network
```

---

## Step 4 — Start the Local Testnet

The easiest way to run a local testnet is Docker Compose:

```bash
# Build Docker images and start 4-node testnet
docker-compose up -d --build

# Check all nodes are healthy
curl http://localhost:9090/health   # bootnode
curl http://localhost:9091/health   # node1
curl http://localhost:9092/health   # node2
curl http://localhost:9093/health   # node3
```

Each node response should be `{"status":"ok","height":<N>,"peers":<M>}`.

### Watch logs

```bash
# Bootnode mining logs
docker-compose logs -f bootnode

# All nodes
docker-compose logs -f
```

### Stop the testnet

```bash
docker-compose down
```

---

## Step 5 — Run a Node Natively

For development, running a node directly is faster than Docker:

### Start a mining node

```bash
# Replace with your miner address (64 hex characters)
MINER_ADDR=0000000000000000000000000000000000000000000000000000000000000001

./target/release/coinject \
  --mine \
  --miner-address $MINER_ADDR \
  --data-dir ./node_data \
  --rpc-addr 127.0.0.1:9933 \
  --cpp-p2p-addr 0.0.0.0:707
```

### Start a non-mining node

```bash
./target/release/coinject \
  --data-dir ./node_data \
  --rpc-addr 127.0.0.1:9933 \
  --cpp-p2p-addr 0.0.0.0:707
```

### Connect to a peer

```bash
./target/release/coinject \
  --data-dir ./node_data \
  --rpc-addr 127.0.0.1:9934 \
  --cpp-p2p-addr 0.0.0.0:708 \
  --peer 127.0.0.1:707
```

---

## Step 6 — Create a Wallet

```bash
# Generate a new keypair
./target/release/coinject-wallet account new --name alice

# List accounts
./target/release/coinject-wallet account list

# Show account address
./target/release/coinject-wallet account show alice
```

Your account address is your Ed25519 public key encoded as base58.

---

## Step 7 — Make Your First Transaction

### Get your balance

```bash
./target/release/coinject-wallet account balance alice \
  --rpc http://localhost:9933
```

On testnet, the faucet funds accounts at genesis. If your balance is 0:

```bash
# Request testnet tokens from faucet
curl -X POST http://localhost:9933 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"faucet_request","params":["<YOUR_ADDRESS>"],"id":1}'
```

### Send a transfer

```bash
./target/release/coinject-wallet transaction send \
  --from alice \
  --to <RECIPIENT_ADDRESS> \
  --amount 100 \
  --fee 1 \
  --rpc http://localhost:9933
```

### Check the transaction

```bash
# Query transaction by hash
curl -X POST http://localhost:9933 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"transaction_get","params":["<TX_HASH>"],"id":1}'
```

---

## Step 8 — Submit a Marketplace Problem

COINjecture's core innovation — submit an NP-hard problem and reward solvers.

### Via CLI wallet

```bash
./target/release/coinject-wallet marketplace submit-problem \
  --from alice \
  --type subset-sum \
  --numbers "15,22,14,26,32,9,16,8" \
  --target 53 \
  --bounty 1000 \
  --min-work-score 5.0 \
  --expiration-days 30 \
  --fee 10 \
  --rpc http://localhost:9933
```

### Check open problems

```bash
curl -X POST http://localhost:9933 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"marketplace_getOpenProblems","params":[],"id":1}'
```

---

## Step 9 — Explore the Web Wallet

Start the development server:

```bash
cd web-wallet
npm install
npm run dev
```

Open `http://localhost:5173` in your browser. The web wallet connects to the local RPC at `http://localhost:9933`.

---

## Next Steps

| Goal | Resource |
|------|----------|
| Understand the architecture | [docs/ARCHITECTURE.md](ARCHITECTURE.md) |
| Run integration tests | [tests/harness/README.md](../tests/harness/README.md) |
| Contribute code | [CONTRIBUTING.md](../CONTRIBUTING.md) |
| Report a security issue | [SECURITY.md](../SECURITY.md) |
| Common errors | [docs/TROUBLESHOOTING.md](TROUBLESHOOTING.md) |

---

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `RUST_LOG` | `info` | Log level (`trace`, `debug`, `info`, `warn`, `error`) |
| `COINJECT_DATA_DIR` | `./node_data` | Default data directory |
| `COINJECT_RPC_ADDR` | `127.0.0.1:9933` | RPC bind address |
| `COINJECT_P2P_ADDR` | `0.0.0.0:707` | CPP P2P bind address |

Set verbose logging:
```bash
RUST_LOG=coinject_node=debug,coinject_network=trace ./target/release/coinject
```
