#!/usr/bin/env bash
# Pull prebuilt node image from GHCR and restart the **distributed** three-host mesh:
#   - Each VPS: one bootnode (:707 public face, own chain DB, sync-gated --mine)
#   - Canonical (193): bootnode + api-server
#   - Followers (76, 198): bootnode + api-server (local RPC; public API on 193)
#   - node1–node3 are NOT started (mesh-bootnode-only profile)
#   - mesh-node-autorepair on each host
#
# Does NOT wipe chain volumes on redeploy.
#
# Usage:
#   export COINJECT_NODE_IMAGE=ghcr.io/coinjecture-network/coinjecture2.0:sha-f5cf6f5
#   ./scripts/deployment/redeploy-remote-mesh-ghcr.sh
#
set -euo pipefail

COINJECT_NODE_IMAGE="${COINJECT_NODE_IMAGE:-ghcr.io/coinjecture-network/coinjecture2.0:sha-f5cf6f5}"
GHCR_USER="${GHCR_USER:-${GITHUB_ACTOR:-COINjecture-Network}}"
GHCR_TOKEN="${GHCR_TOKEN:-}"

CANONICAL_HOST="${CANONICAL_HOST:-root@193.203.164.13}"
CANONICAL_PATH="${CANONICAL_PATH:-/opt/coinjecture}"
FOLLOWER1_HOST="${FOLLOWER1_HOST:-root@76.13.101.67}"
FOLLOWER1_PATH="${FOLLOWER1_PATH:-/opt/coinjecture-src}"
FOLLOWER2_HOST="${FOLLOWER2_HOST:-root@198.199.81.81}"
FOLLOWER2_PATH="${FOLLOWER2_PATH:-/opt/coinjecture}"

COMPOSE_FILE="${COMPOSE_FILE:-docker-compose.yml}"
SYNC_OVERLAY="${SYNC_COMPOSE_OVERLAY:-docker-compose.sync-follower.yml}"
DISTRIBUTED_OVERLAY="${DISTRIBUTED_OVERLAY:-docker-compose.mesh-bootnode-only.yml}"
BOOTNODE_HEALTH_OVERLAY="${BOOTNODE_HEALTH_COMPOSE_OVERLAY:-docker-compose.bootnode-health-metrics-only.yml}"

MESH_SERVICES="${MESH_SERVICES:-bootnode api-server}"
MESH_PULL="${MESH_PULL:-bootnode}"

MESH_IP_193="${MESH_IP_193:-193.203.164.13}"
MESH_IP_76="${MESH_IP_76:-76.13.101.67}"
MESH_IP_198="${MESH_IP_198:-198.199.81.81}"

AUTOREPAIR_INTERVAL="${AUTOREPAIR_INTERVAL:-300}"

SSH_OPTS=( -o BatchMode=yes -o ConnectTimeout=30 -o StrictHostKeyChecking=accept-new )

remote() {
  local host="$1"
  shift
  ssh "${SSH_OPTS[@]}" "$host" "$@"
}

compose_flags_distributed() {
  echo -f "$COMPOSE_FILE" -f "$SYNC_OVERLAY" -f "$DISTRIBUTED_OVERLAY" -f "$BOOTNODE_HEALTH_OVERLAY"
}

patch_mesh_bootnodes_remote() {
  local host="$1" path="$2" bootnodes="$3"
  remote "$host" bash -s <<REMOTE
set -euo pipefail
cd "$path"
for key in COINJECT_BOOTNODES BOOTNODES; do
  if grep -q "^\${key}=" .env 2>/dev/null; then
    sed -i.bak "s|^\${key}=.*|\${key}=$bootnodes|" .env
  else
    echo "\${key}=$bootnodes" >> .env
  fi
done
grep -E '^(COINJECT_BOOTNODES|BOOTNODES)=' .env
REMOTE
}

patch_local_rpc_url() {
  local host="$1" path="$2"
  remote "$host" bash -s <<REMOTE
set -euo pipefail
cd "$path"
for kv in "NODE_RPC_URL=http://bootnode:9933" "MINING_RPC_URL=http://bootnode:9933"; do
  key="\${kv%%=*}"
  val="\${kv#*=}"
  if grep -q "^\${key}=" .env 2>/dev/null; then
    sed -i.bak "s|^\${key}=.*|\${key}=\${val}|" .env
  else
    echo "\${key}=\${val}" >> .env
  fi
done
grep -E '^(NODE_RPC_URL|MINING_RPC_URL)=' .env
REMOTE
}

install_autorepair() {
  local host="$1" path="$2" role="$3"
  remote "$host" bash -s <<REMOTE
set -euo pipefail
cd "$path"
chmod +x scripts/deployment/mesh-node-autorepair.sh scripts/deployment/install-mesh-autorepair-timer.sh 2>/dev/null || true
MESH_ROLE=$role MESH_FLEET=bootnode-only COMPOSE_DIR=$path REPAIR_INTERVAL=$AUTOREPAIR_INTERVAL \
  ./scripts/deployment/install-mesh-autorepair-timer.sh
REMOTE
}

run_mesh_host() {
  local label="$1" host="$2" path="$3" bootnodes="$4" role="$5"
  echo "=== $label (distributed miner): $host $path ==="
  patch_mesh_bootnodes_remote "$host" "$path" "$bootnodes"
  patch_local_rpc_url "$host" "$path"
  remote "$host" bash -s <<REMOTE
set -euo pipefail
cd "$path"
export COINJECT_NODE_IMAGE="$COINJECT_NODE_IMAGE"
if [[ -n "$GHCR_TOKEN" ]]; then
  echo "$GHCR_TOKEN" | docker login ghcr.io -u "$GHCR_USER" --password-stdin
fi
flags="$(compose_flags_distributed)"
# Retire local-only fleet containers from the old single-host layout.
docker compose \$flags stop node1 node2 node3 2>/dev/null || true
docker rm -f coinject-node1 coinject-node2 coinject-node3 2>/dev/null || true
docker compose \$flags pull $MESH_PULL
docker compose \$flags up -d --no-build --force-recreate --remove-orphans $MESH_SERVICES
docker compose \$flags ps
REMOTE
  install_autorepair "$host" "$path" "$role"
}

echo "COINJECT_NODE_IMAGE=$COINJECT_NODE_IMAGE"
echo "Topology: distributed (one bootnode miner per host; node1–node3 off)"
echo "Autorepair interval=${AUTOREPAIR_INTERVAL}s"
echo ""

run_mesh_host "Canonical" "$CANONICAL_HOST" "$CANONICAL_PATH" "${MESH_IP_76}:707,${MESH_IP_198}:707" canonical
echo ""
run_mesh_host "Follower1" "$FOLLOWER1_HOST" "$FOLLOWER1_PATH" "${MESH_IP_193}:707,${MESH_IP_198}:707" follower
echo ""
run_mesh_host "Follower2" "$FOLLOWER2_HOST" "$FOLLOWER2_PATH" "${MESH_IP_193}:707,${MESH_IP_76}:707" follower
echo ""
echo "Done. Each host: one :707 face + bootnode-data volume + sync-gated mining."
echo "Fork self-healing: coinjecture-mesh-autorepair.service on each host"
echo "  ONE_SHOT=1 ./scripts/deployment/mesh-fork-watch.sh"
