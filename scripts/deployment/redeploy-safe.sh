#!/usr/bin/env bash
# Safe redeploy: new images/containers, **same chain data** (named Docker volumes).
#
# NEVER run:  docker compose down -v     # -v deletes volumes → loses blocks
# OK:         docker compose up -d --build
#
# Usage (from repo root):
#   ./scripts/deployment/redeploy-safe.sh                    # backend only, default compose + services
#   COMPOSE_FILE=docker-compose.production.yml ./scripts/deployment/redeploy-safe.sh
#   COMPOSE_EXTRA=docker-compose.bootnode-external-chain.yml \  # VPS bootnode: fixed chain volume
#     SERVICES="bootnode api-server" ./scripts/deployment/redeploy-safe.sh
#   SERVICES="bootnode node1 node2" ./scripts/deployment/redeploy-safe.sh
#   WITH_FRONTEND=1 ./scripts/deployment/redeploy-safe.sh   # also npm build (no AWS; upload separately)
#
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

COMPOSE_FILE="${COMPOSE_FILE:-docker-compose.yml}"
# Default: bootnode + two peers (adjust if you only run two containers)
SERVICES="${SERVICES:-bootnode node1 node2}"
WITH_FRONTEND="${WITH_FRONTEND:-0}"

echo "=========================================="
echo "COINjecture safe redeploy (keeps volumes)"
echo "=========================================="
echo "Compose file: $COMPOSE_FILE"
if [[ -n "$COMPOSE_EXTRA" ]]; then
  echo "Compose extra: $COMPOSE_EXTRA"
fi
echo "Services:     $SERVICES"
echo ""

if [[ "${1:-}" == "--help" ]] || [[ "${1:-}" == "-h" ]]; then
  echo "Chain DB lives in named volumes (e.g. bootnode-data, node1-data). They survive"
  echo "image rebuilds as long as you do NOT pass -v to docker compose down."
  exit 0
fi

if ! command -v docker >/dev/null 2>&1; then
  echo "ERROR: docker not found. Install Docker / Docker Compose plugin."
  exit 1
fi

echo "[1/2] Backend: build + recreate containers (volumes unchanged)..."
# shellcheck disable=SC2086
docker compose "${compose_args[@]}" up -d --build $SERVICES

echo ""
echo "[2/2] Backend containers updated."
if [[ "$WITH_FRONTEND" == "1" ]]; then
  echo "Building frontend (dist/) — upload with web/coinjecture-evolved-main/deploy.sh + AWS creds..."
  (cd "$ROOT/web/coinjecture-evolved-main" && npm ci 2>/dev/null || npm install)
  (cd "$ROOT/web/coinjecture-evolved-main" && npm run build)
  echo "Frontend dist ready at web/coinjecture-evolved-main/dist/"
else
  echo "Skipping frontend build (set WITH_FRONTEND=1 to run npm run build)."
fi

echo ""
echo "Done. Verify height: curl -s http://localhost:9933 -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"method\":\"chain_getInfo\",\"params\":[],\"id\":1}'"
echo "(Adjust host/port per node.)"
