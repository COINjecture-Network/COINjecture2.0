# COINjecture Deployment Guide

## Prerequisites

| Requirement | Minimum | Notes |
|---|---|---|
| Docker | 24.x+ | `docker --version` |
| Docker Compose | 2.x plugin | `docker compose version` |
| RAM | 512 MB per node | 1 GB recommended for production |
| Disk | 10 GB | Chain data grows over time |
| Port 707 | Open inbound | CPP P2P — peers must reach this |

### Image scanning (recommended before any deployment)

```bash
# Using Docker Scout (requires Docker Hub account)
docker scout cves coinject-node:latest

# Using Trivy (open-source, no account needed)
trivy image coinject-node:latest
```

---

## Environment setup

```bash
# 1. Copy the example env file
cp .env.example .env

# 2. Fill in required values
#    MINER_ADDRESS — 64-char hex (openssl rand -hex 32)
#    HF_TOKEN      — only needed for dataset uploads
#    BOOTNODES     — leave empty for a standalone / genesis node
#    DIFFICULTY    — testnet: 4, production: 8+
nano .env
```

`.env` is excluded from version control via `.gitignore`. Never commit it.

---

## Single-node (development / quick start)

Run a single node in dev mode (auto-mines, no peers required):

```bash
docker compose run --rm -p 9933:9933 -p 9090:9090 \
  coinject-node --dev --data-dir /data --rpc-addr 0.0.0.0:9933 --metrics-addr 0.0.0.0:9090
```

Or build the binary locally and run directly:

```bash
cargo build --release --bin coinject
./target/release/coinject --dev
```

---

## Testnet (4-node local network)

Uses `docker-compose.yml`. Starts a bootnode plus three mining peers.

```bash
# Build the image once
docker compose build

# Start all nodes in the background
docker compose up -d

# Tail logs
docker compose logs -f

# Check health
docker compose ps   # all services should show "(healthy)"

# Stop and remove containers (data volumes are preserved)
docker compose down
```

**Port mapping (host → container):**

| Service | P2P | Metrics/Health | RPC |
|---|---|---|---|
| bootnode | 707 | 9090 | 9933 |
| node1 | 708 | 9091 | 9934 |
| node2 | 709 | 9092 | 9935 |
| node3 | 710 | 9093 | 9936 |

Verify the network is mining:

```bash
curl -s http://localhost:9090/health   # {"status":"healthy"}
curl -s http://localhost:9090/metrics  # Prometheus text format
curl -s -X POST http://localhost:9933  \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","method":"chain_getInfo","params":[],"id":1}'
```

---

## Production deployment (single validator node)

Uses `docker-compose.production.yml`. Key differences from testnet:

- Mining disabled — validator role only
- RPC and metrics bound to `127.0.0.1` — require a reverse proxy for external access
- `restart: always` — survives host reboots
- JSON-file log rotation (50 MB × 5 files)
- Higher resource reservations

### Steps

```bash
# 1. Provision host (Ubuntu 22.04 LTS recommended)
# 2. Install Docker: https://docs.docker.com/engine/install/ubuntu/

# 3. Create chain data directory with correct ownership
sudo mkdir -p /var/lib/coinject
sudo chown 10001:10001 /var/lib/coinject

# 4. Set DATA_DIR in .env
echo "DATA_DIR=/var/lib/coinject" >> .env

# 5. Set BOOTNODES to known seed nodes
echo "BOOTNODES=seed1.coinjecture.com:707,seed2.coinjecture.com:707" >> .env

# 6. Build and start
docker compose -f docker-compose.production.yml build
docker compose -f docker-compose.production.yml up -d

# 7. Monitor
docker compose -f docker-compose.production.yml logs -f
```

### Reverse proxy (Caddy example)

```caddyfile
rpc.coinjecture.com {
    reverse_proxy localhost:9933
}
```

This provides automatic TLS via Let's Encrypt and proxies the JSON-RPC port.

### Firewall rules

```bash
# Allow CPP P2P from any peer
ufw allow 707/tcp

# Allow reverse proxy (80/443 handled by Caddy/nginx)
ufw allow 80/tcp
ufw allow 443/tcp

# Block direct RPC access from internet (proxy handles this)
ufw deny 9933/tcp
ufw deny 9090/tcp
```

---

## Volume permissions

The container runs as uid/gid `10001` (user `coinject`). Named Docker volumes work automatically. For bind-mounted host directories:

```bash
# Set ownership on the host path
sudo chown -R 10001:10001 /path/to/chain-data
```

---

## Upgrading

```bash
# Pull or rebuild the image
docker compose build --no-cache

# Rolling restart (data volumes are preserved)
docker compose up -d --force-recreate
```

---

## Troubleshooting

| Symptom | Check |
|---|---|
| Container exits immediately | `docker compose logs coinject-production` |
| Health check failing | `curl http://localhost:9090/health` from inside the container |
| No peers connecting | Verify port 707 is open inbound (`nmap -p 707 <host>`) |
| High memory usage | Reduce `--difficulty` or check for memory leak via metrics |
| Volume permission denied | Ensure host path is `chown 10001:10001` |
