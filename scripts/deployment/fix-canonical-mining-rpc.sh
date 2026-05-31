#!/usr/bin/env bash
# Restore chain_getMiningWork + wallet transfers on the canonical host (193):
#   - bootnode: P2P seed + RPC failover (no --mine in node1-only-mine overlay)
#   - node1: --mine + mempool used for blocks
#   - api-server: NODE_RPC_URL puts node1 first (tx_submit → miner mempool)
#
# Usage (from laptop):
#   HOST=root@193.203.164.13 ./scripts/deployment/fix-canonical-mining-rpc.sh
#
# On the server:
#   VERIFY_LOCAL=1 ./scripts/deployment/fix-canonical-mining-rpc.sh
#
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
HOST="${HOST:-root@193.203.164.13}"
PATH_ON_HOST="${PATH_ON_HOST:-/opt/coinjecture}"
VERIFY_LOCAL="${VERIFY_LOCAL:-0}"
SYNC_SOURCES="${SYNC_SOURCES:-1}"

COMPOSE="-f docker-compose.yml -f docker-compose.sync-follower.yml -f docker-compose.node1-only-mine.yml -f docker-compose.local-ram.yml -f docker-compose.bootnode-health-metrics-only.yml"

sync_to_host() {
  echo "=== Syncing workspace (excluding .env, target, .git) to $HOST:$PATH_ON_HOST ==="
  rsync -az \
    --exclude '.git' \
    --exclude 'target' \
    --exclude 'node_modules' \
    --exclude '.env' \
    --exclude '*.bak' \
    "$ROOT/" "$HOST:$PATH_ON_HOST/"
}

run_fix() {
  set -euo pipefail
  cd "$PATH_ON_HOST"

  for kv in \
    "NODE_RPC_URL=http://bootnode:9933,http://node1:9933,http://node2:9933,http://node3:9933" \
    "MINING_RPC_URL=http://node1:9933"
  do
    key="${kv%%=*}"
    val="${kv#*=}"
    if grep -q "^${key}=" .env 2>/dev/null; then
      sed -i.bak "s|^${key}=.*|${key}=${val}|" .env
    else
      echo "${key}=${val}" >> .env
    fi
  done

  echo "=== Building api-server (MINING_RPC_URL support) ==="
  docker compose $COMPOSE build api-server

  echo "=== Starting bootnode + node1 + api-server ==="
  docker compose $COMPOSE up -d --no-build bootnode node1
  docker compose $COMPOSE up -d --force-recreate api-server

  echo "=== chain_getMiningWork via node1 (from host network) ==="
  curl -sf --max-time 30 -X POST http://127.0.0.1:9934/ \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc":"2.0","id":1,"method":"chain_getMiningWork","params":[]}' \
    | head -c 400 || echo "(node1 RPC on :9934 not reachable from host — check container)"
  echo ""

  echo "=== chain_getMiningWork via api (localhost:3030/node-rpc) ==="
  curl -sf --max-time 30 -X POST http://127.0.0.1:3030/node-rpc \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc":"2.0","id":1,"method":"chain_getMiningWork","params":[]}' \
    | head -c 400 || echo "(api /node-rpc failed — check logs: docker logs coinject-api)"
  echo ""
}

if [[ "$VERIFY_LOCAL" == 1 ]]; then
  run_fix
else
  if [[ "$SYNC_SOURCES" == 1 ]]; then
    sync_to_host
  fi
  ssh -o BatchMode=yes -o ConnectTimeout=30 -o StrictHostKeyChecking=accept-new "$HOST" \
    "PATH_ON_HOST=$PATH_ON_HOST VERIFY_LOCAL=1 bash -s" < "$0"
fi

echo "Done. Public check: curl -s -X POST https://api.coinjecture.com/node-rpc -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"chain_getMiningWork\",\"params\":[]}' | head -c 300"
