#!/usr/bin/env bash
# Destructive chain resync on a single remote host: wipe node data volumes, recreate containers.
#
# KEEPS on the server:
#   - Git checkout and docker-compose.yml
#   - .env (secrets / NODE_RPC_URL / bootnodes — never touched by this script)
#   - Docker images (reused; optional --build refreshes binaries)
#
# REMOVES (irrecoverable without backups):
#   - Named volumes: bootnode-data, node1-data, node2-data, node3-data (all declared in compose)
#   - Containers for the services you stop with `docker compose down -v`
#
# After wipe, nodes re-sync from P2P peers using your .env bootnode list (must reach canonical network).
# Recommended: bring the stack up with docker-compose.sync-follower.yml (no --mine) until best_height
# matches peers, then `compose down` and `up` with only docker-compose.yml to re-enable mining — see README.
#
# Usage (from your laptop; requires SSH key):
#   export DESTRUCTIVE_CHAIN_RESYNC_CONFIRM=I_UNDERSTAND_WIPE_CHAIN_DATA
#   export HOST=root@76.13.101.67
#   export REMOTE_PATH=/opt/coinjecture-src
#   export SERVICES="bootnode node1 node2 api-server"   # adjust if your host runs fewer/more
#   ./scripts/deployment/destructive-chain-resync-remote.sh
#
set -euo pipefail

HOST="${HOST:-}"
REMOTE_PATH="${REMOTE_PATH:-}"
COMPOSE_FILE="${COMPOSE_FILE:-docker-compose.yml}"
SERVICES="${SERVICES:-bootnode node1 node2 api-server}"
CONFIRM="${DESTRUCTIVE_CHAIN_RESYNC_CONFIRM:-}"

if [[ "$CONFIRM" != "I_UNDERSTAND_WIPE_CHAIN_DATA" ]]; then
  echo "Refusing to run: set DESTRUCTIVE_CHAIN_RESYNC_CONFIRM=I_UNDERSTAND_WIPE_CHAIN_DATA"
  exit 1
fi

if [[ -z "$HOST" || -z "$REMOTE_PATH" ]]; then
  echo "Set HOST (e.g. root@76.13.101.67) and REMOTE_PATH (e.g. /opt/coinjecture-src)"
  exit 1
fi

echo "=== Destructive chain resync ==="
echo "HOST=$HOST REMOTE_PATH=$REMOTE_PATH"
echo "SERVICES=$SERVICES"
echo ""

ssh -o BatchMode=yes -o StrictHostKeyChecking=accept-new "$HOST" bash -s <<EOF
set -euo pipefail
cd "$REMOTE_PATH"
echo ">>> Before: compose ps + volumes"
docker compose -f "$COMPOSE_FILE" ps -a 2>/dev/null || true
docker volume ls --filter "label=com.docker.compose.project" -q 2>/dev/null | head -20 || true

echo ">>> docker compose down -v (removes named chain volumes for this project)"
docker compose -f "$COMPOSE_FILE" down -v --remove-orphans

echo ">>> docker compose up -d --build $SERVICES"
docker compose -f "$COMPOSE_FILE" up -d --build $SERVICES

echo ">>> After: compose ps"
docker compose -f "$COMPOSE_FILE" ps

echo ">>> Wait for bootnode JSON-RPC (up to ~3 min)"
ok=0
for i in \$(seq 1 40); do
  if curl -sfS -m 5 -X POST http://127.0.0.1:9933/ \\
      -H 'Content-Type: application/json' \\
      -d '{"jsonrpc":"2.0","method":"chain_getInfo","params":[],"id":1}' \\
      | grep -q best_height; then
    ok=1
    echo ">>> chain_getInfo OK (attempt \$i)"
    break
  fi
  sleep 5
done
if [[ "\$ok" -ne 1 ]]; then
  echo ">>> WARN: chain_getInfo not ready — check: docker logs coinject-bootnode"
  exit 0
fi

echo ">>> chain_getInfo:"
curl -sfS -m 10 -X POST http://127.0.0.1:9933/ \\
  -H 'Content-Type: application/json' \\
  -d '{"jsonrpc":"2.0","method":"chain_getInfo","params":[],"id":1}'
echo ""
EOF

echo ""
echo "=== Done ==="
