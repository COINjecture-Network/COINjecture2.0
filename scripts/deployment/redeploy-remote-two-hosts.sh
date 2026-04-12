#!/usr/bin/env bash
# Redeploy on **two** servers over SSH without wiping chain data.
#
# Chain blocks live in Docker *named volumes* (e.g. bootnode-data, node1-data).
# This script only runs: git pull + docker compose up -d --build
# That recreates containers and keeps volumes. Blocks are lost only if someone runs:
#   docker compose down -v
#   docker volume rm …
# Do not use those on production hosts.
#
# Prerequisites on each host:
#   - Repo or deploy bundle at REMOTE_PATH
#   - Docker + compose plugin
#   - Same named volumes as before (never `docker compose down -v`)
#
# Usage:
#   export HOST1=user@node-a.example
#   export HOST2=user@node-b.example
#   export REMOTE_PATH=/opt/COINjecture2.0-main   # path to repo on server
#   export REMOTE_PATH2=/opt/other-path           # optional: HOST2 if different from HOST1
#   ./scripts/deployment/redeploy-remote-two-hosts.sh
#
# Bootnode host with existing chain volume (see docker-compose.bootnode-external-chain.yml):
#   export COMPOSE_EXTRA=docker-compose.bootnode-external-chain.yml
#   export SERVICES="bootnode api-server"   # on host1; host2 may differ
#
set -euo pipefail

HOST1="${HOST1:-}"
HOST2="${HOST2:-}"
REMOTE_PATH="${REMOTE_PATH:-/opt/COINjecture2.0-main}"
REMOTE_PATH2="${REMOTE_PATH2:-$REMOTE_PATH}"
COMPOSE_FILE="${COMPOSE_FILE:-docker-compose.yml}"
COMPOSE_EXTRA="${COMPOSE_EXTRA:-}"
SERVICES="${SERVICES:-bootnode node1 node2}"

if [[ -z "$HOST1" || -z "$HOST2" ]]; then
  echo "Set HOST1 and HOST2 (e.g. root@143.110.139.166) and optionally REMOTE_PATH"
  exit 1
fi

run_remote() {
  local host="$1"
  local rpath="$2"
  ssh -o BatchMode=yes -o StrictHostKeyChecking=accept-new "$host" bash -s <<EOF
set -e
cd "$rpath"
echo ">>> Chain safety: using up -d --build only (no compose down -v; volumes unchanged)"
git pull --ff-only 2>/dev/null || true
# Safe: rebuild images and recreate containers; named volumes keep existing blocks
if [[ -n "$COMPOSE_EXTRA" ]]; then
  docker compose -f "$COMPOSE_FILE" -f "$COMPOSE_EXTRA" up -d --build $SERVICES
  docker compose -f "$COMPOSE_FILE" -f "$COMPOSE_EXTRA" ps
else
  docker compose -f "$COMPOSE_FILE" up -d --build $SERVICES
  docker compose -f "$COMPOSE_FILE" ps
fi
# Right after recreate, host :3030 can RST until the new process binds; /chain/info can
# also exceed a short curl timeout on cold RPC. Only probe /health with retries.
if [[ "$SERVICES" == *api-server* ]]; then
  echo ">>> Probing http://127.0.0.1:3030/health (up to ~30s; ignore transient RST)..."
  succeeded=0
  for _ in \$(seq 1 15); do
    if curl -sfS -m 2 http://127.0.0.1:3030/health >/dev/null 2>&1; then
      succeeded=1
      echo ">>> /health OK"
      break
    fi
    sleep 2
  done
  if [[ "\$succeeded" -ne 1 ]]; then
    echo ">>> WARN: /health not ready in time — check: docker logs coinject-api"
  fi
fi
EOF
}

echo "=== Redeploying $HOST1 (REMOTE_PATH=$REMOTE_PATH) ==="
run_remote "$HOST1" "$REMOTE_PATH"
echo ""
echo "=== Redeploying $HOST2 (REMOTE_PATH2=$REMOTE_PATH2) ==="
run_remote "$HOST2" "$REMOTE_PATH2"
echo ""
echo "Done. If you use different compose topology per host, run commands manually"
echo "on each machine with the correct COMPOSE_FILE and SERVICES."
