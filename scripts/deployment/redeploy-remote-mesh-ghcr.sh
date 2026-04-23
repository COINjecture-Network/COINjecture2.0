#!/usr/bin/env bash
# Pull prebuilt node image from GHCR and restart P2P stack on all three mesh hosts.
# Does NOT run `docker compose down -v` — chain volumes are preserved.
#
# Image package: https://github.com/COINjecture-Network/COINjecture2.0/pkgs/container/coinjecture2.0
#
# Prerequisites:
#   - SSH access (BatchMode) to each host as root (or set CANONICAL_HOST / FOLLOWER*_HOST)
#   - Repo + `.env` at REMOTE_PATH on each host (COINJECT_BOOTNODES, keys, etc.)
#   - Private GHCR: export GHCR_TOKEN (PAT read:packages) — script logs in once per host
#
# Usage:
#   export COINJECT_NODE_IMAGE=ghcr.io/coinjecture-network/coinjecture2.0:latest
#   # optional pin (see package page "Recent tagged image versions"):
#   # export COINJECT_NODE_IMAGE=ghcr.io/coinjecture-network/coinjecture2.0:sha-be7d59e
#   ./scripts/deployment/redeploy-remote-mesh-ghcr.sh
#
set -euo pipefail

COINJECT_NODE_IMAGE="${COINJECT_NODE_IMAGE:-ghcr.io/coinjecture-network/coinjecture2.0:latest}"
GHCR_USER="${GHCR_USER:-${GITHUB_ACTOR:-COINjecture-Network}}"
GHCR_TOKEN="${GHCR_TOKEN:-}"

CANONICAL_HOST="${CANONICAL_HOST:-root@193.203.164.13}"
CANONICAL_PATH="${CANONICAL_PATH:-/opt/coinjecture}"
FOLLOWER1_HOST="${FOLLOWER1_HOST:-root@76.13.101.67}"
FOLLOWER1_PATH="${FOLLOWER1_PATH:-/opt/coinjecture-src}"
FOLLOWER2_HOST="${FOLLOWER2_HOST:-root@198.199.81.81}"
FOLLOWER2_PATH="${FOLLOWER2_PATH:-/opt/coinjecture}"

COMPOSE_FILE="${COMPOSE_FILE:-docker-compose.yml}"
FOLLOWER_COMPOSE_EXTRA="${FOLLOWER_COMPOSE_EXTRA:-docker-compose.follower-no-mine.yml}"
NODE_SERVICES="${NODE_SERVICES:-bootnode node1 node2 node3}"
STACK_SERVICES="${STACK_SERVICES:-bootnode node1 node2 node3 api-server}"

SSH_OPTS=( -o BatchMode=yes -o ConnectTimeout=30 -o StrictHostKeyChecking=accept-new )

remote() {
  local host="$1"
  shift
  ssh "${SSH_OPTS[@]}" "$host" "$@"
}

run_canonical() {
  local host="$1" path="$2"
  echo "=== Canonical: $host $path ==="
  remote "$host" bash -s <<REMOTE
set -euo pipefail
cd "$path"
export COINJECT_NODE_IMAGE="$COINJECT_NODE_IMAGE"
if [[ -n "$GHCR_TOKEN" ]]; then
  echo "$GHCR_TOKEN" | docker login ghcr.io -u "$GHCR_USER" --password-stdin
fi
docker compose -f "$COMPOSE_FILE" pull $NODE_SERVICES
docker compose -f "$COMPOSE_FILE" up -d --no-build $STACK_SERVICES
docker compose -f "$COMPOSE_FILE" ps
REMOTE
}

run_follower() {
  local host="$1" path="$2"
  echo "=== Follower: $host $path ==="
  remote "$host" bash -s <<REMOTE
set -euo pipefail
cd "$path"
export COINJECT_NODE_IMAGE="$COINJECT_NODE_IMAGE"
if [[ -n "$GHCR_TOKEN" ]]; then
  echo "$GHCR_TOKEN" | docker login ghcr.io -u "$GHCR_USER" --password-stdin
fi
docker compose -f "$COMPOSE_FILE" -f "$FOLLOWER_COMPOSE_EXTRA" pull $NODE_SERVICES
docker compose -f "$COMPOSE_FILE" -f "$FOLLOWER_COMPOSE_EXTRA" up -d --no-build $STACK_SERVICES
docker compose -f "$COMPOSE_FILE" -f "$FOLLOWER_COMPOSE_EXTRA" ps
REMOTE
}

echo "COINJECT_NODE_IMAGE=$COINJECT_NODE_IMAGE"
echo ""

run_canonical "$CANONICAL_HOST" "$CANONICAL_PATH"
echo ""
run_follower "$FOLLOWER1_HOST" "$FOLLOWER1_PATH"
echo ""
run_follower "$FOLLOWER2_HOST" "$FOLLOWER2_PATH"
echo ""
echo "Done. Verify: curl -sS http://193.203.164.13:9933/ -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"method\":\"chain_getInfo\",\"params\":[],\"id\":1}'"
