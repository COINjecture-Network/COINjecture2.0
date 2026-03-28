# COINjecture CPP Testnet — Quick Start

## Prerequisites

- Rust 1.88+ (`rustup update`)
- Or Docker (for containerized testnet)

---

## Option 1: Docker Testnet (Easiest)

```bash
# Build and start 4-node testnet (1 bootnode + 3 peers)
docker-compose up -d --build

# Check health
curl http://localhost:9090/health   # bootnode
curl http://localhost:9091/health   # node1
curl http://localhost:9092/health   # node2
curl http://localhost:9093/health   # node3

# View logs
docker-compose logs -f bootnode
docker-compose logs -f node1

# Stop
docker-compose down
```

### Docker Port Mapping

| Service  | CPP P2P | Metrics/Health | RPC  |
|----------|---------|----------------|------|
| bootnode | 707     | 9090           | 9933 |
| node1    | 708     | 9091           | 9934 |
| node2    | 709     | 9092           | 9935 |
| node3    | 710     | 9093           | 9936 |

---

## Option 2: Local Binary

### Build

```bash
cargo build --release --bin coinject
```

### Start Bootnode

```bash
./target/release/coinject \
  --mine \
  --data-dir ./data/bootnode \
  --cpp-p2p-addr 0.0.0.0:707 \
  --metrics-addr 0.0.0.0:9090 \
  --rpc-addr 0.0.0.0:9933 \
  --difficulty 4 \
  --block-time 60
```

### Start Peer Node

```bash
./target/release/coinject \
  --mine \
  --data-dir ./data/node1 \
  --cpp-p2p-addr 0.0.0.0:708 \
  --metrics-addr 0.0.0.0:9091 \
  --rpc-addr 0.0.0.0:9934 \
  --bootnodes 127.0.0.1:707 \
  --difficulty 4 \
  --block-time 60
```

---

## Verify It Works

### Health Check

```bash
curl http://localhost:9090/health
# {"status":"healthy"}
```

### Get Chain Info

```bash
curl -X POST http://localhost:9933 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"chain_getInfo","params":[],"id":1}'
```

### Get Balance

```bash
curl -X POST http://localhost:9933 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"account_getBalance","params":["0000000000000000000000000000000000000000000000000000000000000001"],"id":1}'
```

### Prometheus Metrics

```bash
curl http://localhost:9090/metrics
```

---

## Run Tests

```bash
# All workspace tests
cargo test --all

# Network integration tests (8/8 passing)
cargo test -p coinject-network -- --test-threads=1

# Testnet integration tests specifically
cargo test -p coinject-network --test testnet_integration -- --test-threads=1

# Node config tests
cargo test -p coinject-node
```

---

## Network Architecture

```
CPP Protocol (port 707)
├── Wire: COIN magic | v1 | type | len | payload | blake3
├── Fanout: ⌈√n × η⌉ where η = 1/√2
├── Routing: EquilibriumRouter with murmuration (Reynolds rules)
├── Handshake: Hello → HelloAck (genesis hash validated)
└── 17 message types with dimensional priority
```

---

## Troubleshooting

**Port 707 already in use:**
```bash
# Check what's using it
lsof -i :707        # Linux/Mac
netstat -ano | findstr :707  # Windows
```

**Node won't connect to bootnode:**
- Verify bootnode is running and listening on the correct address
- Check firewall rules allow TCP on port 707
- Ensure both nodes use the same `--chain-id`

**No blocks being mined:**
- NP-hard problems can take time. Wait 60+ seconds
- Lower difficulty: `--difficulty 2`

**Docker build fails:**
- Ensure Docker has enough memory (4GB+ recommended)
- Check proxy settings: `docker info | grep -i proxy`
