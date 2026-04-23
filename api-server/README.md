# COINjecture API server

Rust (Axum) service behind `https://api.coinjecture.com`: auth, marketplace, **`GET /chain/info`**, **`POST /node-rpc`** (JSON-RPC proxy), indexer hooks, etc.

## Local run

From repo root (workspace member):

```bash
cd api-server
cp .env.example .env   # fill SUPABASE_* and NODE_RPC_URL
cargo run
```

Health: `curl -s http://127.0.0.1:3030/health`

## Production redeploy (api.coinjecture.com)

Redeploy must run **on the host that serves the API** (or in your CI that builds/pushes the image). This repo cannot SSH into your VPS.

### Option A — Docker (`docker-compose.production.yml`)

From **repository root** (context must include the full workspace for `Dockerfile.api`):

```bash
git pull
# Ensure .env.production (or shell env) has NODE_RPC_URL, SUPABASE_*, INDEXER_* as needed
docker compose -f docker-compose.production.yml build api-server
docker compose -f docker-compose.production.yml up -d api-server
```

Or use the helper (set `SERVICES` / `COMPOSE_FILE` as needed):

```bash
COMPOSE_FILE=docker-compose.production.yml SERVICES=api-server ./scripts/deployment/redeploy-safe.sh
```

### Option B — systemd binary (`coinjecture-api.service`)

Example unit: `scripts/deployment/hostinger-vps/coinjecture-api.service`.

```bash
cd /opt/coinjecture   # or your deploy dir
git pull
cargo build --release -p coinjecture-api-server
sudo install -m 755 target/release/coinjecture-api /opt/coinjecture/coinjecture-api
sudo systemctl restart coinjecture-api
sudo systemctl status coinjecture-api --no-pager
```

### Verify after deploy

```bash
curl -sS 'https://api.coinjecture.com/health'
curl -sS 'https://api.coinjecture.com/chain/info' -H 'Accept: application/json' | head -c 400
```

JSON-RPC (should be fast on **repeated** `chain_getInfo` while cache is warm):

```bash
curl -sS 'https://api.coinjecture.com/node-rpc' \
  -H 'Content-Type: application/json' \
  -H 'Origin: https://coinjecture.com' \
  --data-raw '{"jsonrpc":"2.0","id":1,"method":"chain_getInfo","params":[]}'
```

## `chain_getInfo` and `/node-rpc` latency

`POST /node-rpc` for `chain_getInfo` with empty `params` is served from the **same in-memory cache** as `GET /chain/info` when fresh (short TTL in `EventBroadcaster`), warmed on startup and by the node poller. Expect **sub‑100 ms** for repeated calls while the cache is valid.

Upstream JSON-RPC uses a **short read deadline** for `chain_getInfo` / `network_getInfo` (see `node_rpc.rs` `http_light`) and far fewer transport retries than before, so a wedged `NODE_RPC_URL` fails in **seconds to tens of seconds**, not tens of minutes.

See also: `scripts/deployment/hostinger-vps/README.txt` for VPS/DNS/TLS notes.
