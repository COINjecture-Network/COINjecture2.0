#!/usr/bin/env bash
# Repair public /node-rpc when chain_getMiningWork returns 503 (bootnode restart / stale API pool).
#
# Usage:
#   ./scripts/deployment/repair-canonical-api-rpc.sh
#   HOST=root@193.203.164.13 ./scripts/deployment/repair-canonical-api-rpc.sh
#
set -euo pipefail

HOST="${HOST:-root@193.203.164.13}"
PATH_ON_HOST="${PATH_ON_HOST:-/opt/coinjecture}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SYNC_SOURCES="${SYNC_SOURCES:-1}"
COMPOSE="-f docker-compose.yml -f docker-compose.sync-follower.yml -f docker-compose.mesh-bootnode-only.yml -f docker-compose.bootnode-health-metrics-only.yml"

if [[ "$SYNC_SOURCES" == 1 ]]; then
  echo "=== Syncing full workspace (Dockerfile.api needs workspace Cargo.toml) to $HOST:$PATH_ON_HOST ==="
  rsync -az \
    --exclude '.git' \
    --exclude 'target' \
    --exclude 'node_modules' \
    --exclude '.env' \
    --exclude '*.bak' \
    "$ROOT/" "$HOST:$PATH_ON_HOST/"
fi

ssh -o BatchMode=yes -o ConnectTimeout=30 -o StrictHostKeyChecking=accept-new "$HOST" bash -s <<REMOTE
set -euo pipefail
cd "$PATH_ON_HOST"

for kv in \
  "NODE_RPC_URL=http://bootnode:9933,http://host.docker.internal:9933" \
  "MINING_RPC_URL=http://bootnode:9933,http://host.docker.internal:9933"
do
  key="\${kv%%=*}"
  val="\${kv#*=}"
  if grep -q "^\${key}=" .env 2>/dev/null; then
    sed -i.bak "s|^\${key}=.*|\${key}=\${val}|" .env
  else
    echo "\${key}=\${val}" >> .env
  fi
done
grep -E '^(NODE_RPC_URL|MINING_RPC_URL)=' .env

echo "=== Rebuild api-server (90s mining RPC timeout + host.docker.internal fallback) ==="
docker compose $COMPOSE build api-server

echo "=== Stop legacy node1–node3 (mesh uses bootnode-only) ==="
docker compose $COMPOSE stop node1 node2 node3 2>/dev/null || true
docker rm -f coinject-node1 coinject-node2 coinject-node3 2>/dev/null || true

echo "=== Restart bootnode, then api-server ==="
docker compose $COMPOSE up -d --no-build bootnode
for i in \$(seq 1 60); do
  if docker inspect coinject-bootnode --format '{{.State.Health.Status}}' 2>/dev/null | grep -q healthy; then
    echo "bootnode healthy"
    break
  fi
  sleep 2
done
docker compose $COMPOSE up -d --force-recreate --no-build api-server

echo "=== Verify chain_getMiningWork via local API ==="
curl -sf --max-time 90 -X POST http://127.0.0.1:3030/node-rpc \\
  -H 'Content-Type: application/json' \\
  -d '{"jsonrpc":"2.0","id":1,"method":"chain_getMiningWork","params":[]}' \\
  | head -c 280 || echo "(local /node-rpc failed — check docker logs coinject-api coinject-bootnode)"
echo ""
REMOTE

echo "Public check:"
curl -sf --max-time 90 -X POST https://api.coinjecture.com/node-rpc \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"chain_getMiningWork","params":[]}' \
  | head -c 280 || echo "(public /node-rpc failed)"
echo ""
