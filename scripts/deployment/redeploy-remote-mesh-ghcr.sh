#!/usr/bin/env bash
# Pull prebuilt node image from GHCR and restart the mesh with **self-healing join-and-mine**:
#   - Canonical: full mining fleet (bootnode + node1–node3 + api), one public mesh face (local-ram)
#   - Followers: bootnode + api by default (scale to full fleet via FOLLOWER_FLEET=full)
#   - Mining is sync-gated in the binary (sync-follower overlay); autorepair aligns local DBs
#   - Installs on-host mesh autorepair systemd unit per host
#
# Does NOT wipe chain volumes on redeploy.
#
# Usage:
#   export COINJECT_NODE_IMAGE=ghcr.io/coinjecture-network/coinjecture2.0:sha-de079ae
#   ./scripts/deployment/redeploy-remote-mesh-ghcr.sh
#
# Scale followers to full miners later:
#   FOLLOWER_FLEET=full ./scripts/deployment/redeploy-remote-mesh-ghcr.sh
#
set -euo pipefail

COINJECT_NODE_IMAGE="${COINJECT_NODE_IMAGE:-ghcr.io/coinjecture-network/coinjecture2.0:sha-0392372}"
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
LOCAL_RAM_OVERLAY="${LOCAL_RAM_COMPOSE_OVERLAY:-docker-compose.local-ram.yml}"
FLEET_OVERLAY="${FLEET_OVERLAY:-docker-compose.mesh-fleet.yml}"
BOOTNODE_ONLY_OVERLAY="${BOOTNODE_ONLY_OVERLAY:-docker-compose.mesh-bootnode-only.yml}"
BOOTNODE_HEALTH_OVERLAY="${BOOTNODE_HEALTH_COMPOSE_OVERLAY:-docker-compose.bootnode-health-metrics-only.yml}"

CANONICAL_FLEET="${CANONICAL_FLEET:-full}"
FOLLOWER_FLEET="${FOLLOWER_FLEET:-bootnode-only}"

CANONICAL_SERVICES="${CANONICAL_SERVICES:-bootnode node1 node2 node3 api-server}"
FOLLOWER_SERVICES_BOOTNODE_ONLY="${FOLLOWER_SERVICES_BOOTNODE_ONLY:-bootnode api-server}"
FOLLOWER_SERVICES_FULL="${FOLLOWER_SERVICES_FULL:-bootnode node1 node2 node3 api-server}"
CANONICAL_PULL="${CANONICAL_PULL:-bootnode node1 node2 node3}"
FOLLOWER_PULL_BOOTNODE_ONLY="${FOLLOWER_PULL_BOOTNODE_ONLY:-bootnode}"
FOLLOWER_PULL_FULL="${FOLLOWER_PULL_FULL:-bootnode node1 node2 node3}"

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

compose_flags_full() {
  echo -f "$COMPOSE_FILE" -f "$SYNC_OVERLAY" -f "$FLEET_OVERLAY" -f "$LOCAL_RAM_OVERLAY" -f "$BOOTNODE_HEALTH_OVERLAY"
}

compose_flags_bootnode_only() {
  echo -f "$COMPOSE_FILE" -f "$SYNC_OVERLAY" -f "$BOOTNODE_ONLY_OVERLAY" -f "$BOOTNODE_HEALTH_OVERLAY"
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

patch_canonical_node_rpc_url() {
  local host="$1" path="$2"
  remote "$host" bash -s <<REMOTE
set -euo pipefail
cd "$path"
rpc="http://bootnode:9933,http://node1:9933,http://node2:9933,http://node3:9933"
if grep -q '^NODE_RPC_URL=' .env 2>/dev/null; then
  sed -i.bak "s|^NODE_RPC_URL=.*|NODE_RPC_URL=\$rpc|" .env
else
  echo "NODE_RPC_URL=\$rpc" >> .env
fi
grep '^NODE_RPC_URL=' .env
REMOTE
}

install_autorepair() {
  local host="$1" path="$2" role="$3" fleet="$4"
  remote "$host" bash -s <<REMOTE
set -euo pipefail
cd "$path"
chmod +x scripts/deployment/mesh-node-autorepair.sh scripts/deployment/install-mesh-autorepair-timer.sh 2>/dev/null || true
MESH_ROLE=$role MESH_FLEET=$fleet COMPOSE_DIR=$path REPAIR_INTERVAL=$AUTOREPAIR_INTERVAL \
  ./scripts/deployment/install-mesh-autorepair-timer.sh
REMOTE
}

run_canonical() {
  local host="$1" path="$2"
  echo "=== Canonical (join-and-mine fleet): $host $path ==="
  patch_mesh_bootnodes_remote "$host" "$path" "${MESH_IP_76}:707,${MESH_IP_198}:707"
  patch_canonical_node_rpc_url "$host" "$path"
  remote "$host" bash -s <<REMOTE
set -euo pipefail
cd "$path"
export COINJECT_NODE_IMAGE="$COINJECT_NODE_IMAGE"
if [[ -n "$GHCR_TOKEN" ]]; then
  echo "$GHCR_TOKEN" | docker login ghcr.io -u "$GHCR_USER" --password-stdin
fi
docker compose $(compose_flags_full) pull $CANONICAL_PULL
docker compose $(compose_flags_full) up -d --no-build --force-recreate --remove-orphans $CANONICAL_SERVICES
docker compose $(compose_flags_full) ps
REMOTE
  install_autorepair "$host" "$path" canonical full
}

run_mesh_host() {
  local label="$1" host="$2" path="$3" bootnodes="$4"
  local fleet="$FOLLOWER_FLEET"
  local services pull flags
  if [[ "$fleet" == "full" ]]; then
    services="$FOLLOWER_SERVICES_FULL"
    pull="$FOLLOWER_PULL_FULL"
    flags="$(compose_flags_full)"
    echo "=== $label (full mining fleet): $host $path ==="
  else
    services="$FOLLOWER_SERVICES_BOOTNODE_ONLY"
    pull="$FOLLOWER_PULL_BOOTNODE_ONLY"
    flags="$(compose_flags_bootnode_only)"
    echo "=== $label (bootnode mesh face): $host $path ==="
  fi
  patch_mesh_bootnodes_remote "$host" "$path" "$bootnodes"
  remote "$host" bash -s <<REMOTE
set -euo pipefail
cd "$path"
export COINJECT_NODE_IMAGE="$COINJECT_NODE_IMAGE"
if [[ -n "$GHCR_TOKEN" ]]; then
  echo "$GHCR_TOKEN" | docker login ghcr.io -u "$GHCR_USER" --password-stdin
fi
docker compose $flags pull $pull
docker compose $flags up -d --no-build --remove-orphans $services
docker compose $flags ps
REMOTE
  install_autorepair "$host" "$path" follower "$fleet"
}

echo "COINJECT_NODE_IMAGE=$COINJECT_NODE_IMAGE"
echo "Canonical: full fleet (bootnode+node1–3 mine, sync-gated)"
echo "Followers: fleet=$FOLLOWER_FLEET (set FOLLOWER_FLEET=full to add miners)"
echo "Autorepair interval=${AUTOREPAIR_INTERVAL}s"
echo ""

run_canonical "$CANONICAL_HOST" "$CANONICAL_PATH"
echo ""
run_mesh_host "Follower1" "$FOLLOWER1_HOST" "$FOLLOWER1_PATH" "${MESH_IP_193}:707,${MESH_IP_198}:707"
echo ""
run_mesh_host "Follower2" "$FOLLOWER2_HOST" "$FOLLOWER2_PATH" "${MESH_IP_193}:707,${MESH_IP_76}:707"
echo ""
echo "Done. Fork self-healing: coinjecture-mesh-autorepair.service on each host"
echo "  ONE_SHOT=1 ./scripts/deployment/mesh-fork-watch.sh"
