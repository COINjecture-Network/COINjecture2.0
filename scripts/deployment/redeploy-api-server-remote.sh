#!/usr/bin/env bash
# Sync repo sources to canonical host and rebuild api-server (Dockerfile.api needs full workspace).
#
# Usage:
#   HOST=root@193.203.164.13 ./scripts/deployment/redeploy-api-server-remote.sh
#
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
HOST="${HOST:-root@193.203.164.13}"
PATH_ON_HOST="${PATH_ON_HOST:-/opt/coinjecture}"

COMPOSE="-f docker-compose.yml -f docker-compose.sync-follower.yml -f docker-compose.mesh-fleet.yml -f docker-compose.local-ram.yml -f docker-compose.bootnode-health-metrics-only.yml"

echo "=== Syncing workspace (excluding .env, target, .git, lean4) to $HOST:$PATH_ON_HOST ==="
rsync -az \
  --exclude '.git' \
  --exclude 'target' \
  --exclude 'node_modules' \
  --exclude 'lean4' \
  --exclude '.hf_cache' \
  --exclude 'latest-upstream' \
  --exclude '.env' \
  --exclude '*.bak' \
  "$ROOT/" "$HOST:$PATH_ON_HOST/"

ssh -o BatchMode=yes -o ConnectTimeout=30 -o StrictHostKeyChecking=accept-new "$HOST" bash -s <<REMOTE
set -euo pipefail
cd "$PATH_ON_HOST"
echo "=== Building coinject-api:latest (api-only, no node image) ==="
docker build -f Dockerfile.api -t coinject-api:latest .
echo "=== Recreating api-server ==="
docker compose $COMPOSE up -d --no-build --force-recreate api-server
docker compose $COMPOSE ps api-server
echo "=== Smoke: transaction_submit route (node1 first via env) ==="
grep -E '^(NODE_RPC_URL|MINING_RPC_URL)=' .env || true
REMOTE

echo "Done."
