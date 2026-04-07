#!/usr/bin/env bash
# Redeploy on **two** servers over SSH without wiping chain data.
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
COMPOSE_FILE="${COMPOSE_FILE:-docker-compose.yml}"
COMPOSE_EXTRA="${COMPOSE_EXTRA:-}"
SERVICES="${SERVICES:-bootnode node1 node2}"

if [[ -z "$HOST1" || -z "$HOST2" ]]; then
  echo "Set HOST1 and HOST2 (e.g. root@143.110.139.166) and optionally REMOTE_PATH"
  exit 1
fi

run_remote() {
  local host="$1"
  ssh -o BatchMode=yes -o StrictHostKeyChecking=accept-new "$host" bash -s <<EOF
set -e
cd "$REMOTE_PATH"
git pull --ff-only 2>/dev/null || true
# Safe: rebuild images and recreate containers; volumes persist
if [[ -n "$COMPOSE_EXTRA" ]]; then
  docker compose -f "$COMPOSE_FILE" -f "$COMPOSE_EXTRA" up -d --build $SERVICES
  docker compose -f "$COMPOSE_FILE" -f "$COMPOSE_EXTRA" ps
else
  docker compose -f "$COMPOSE_FILE" up -d --build $SERVICES
  docker compose -f "$COMPOSE_FILE" ps
fi
EOF
}

echo "=== Redeploying $HOST1 ==="
run_remote "$HOST1"
echo ""
echo "=== Redeploying $HOST2 ==="
run_remote "$HOST2"
echo ""
echo "Done. If you use different compose topology per host, run commands manually"
echo "on each machine with the correct COMPOSE_FILE and SERVICES."
