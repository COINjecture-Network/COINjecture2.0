#!/usr/bin/env bash
# Coordinated mesh genesis: wipe chain volumes everywhere, pull GHCR image, boot canonical,
# then boot follower VPSs so the canonical bootnode has external CPP peers (mining is gated
# until at least one CPP peer connects). Finally wait for a stable tip on canonical.
#
# Canonical (largest / seed): 193.203.164.13 — followers dial it first via COINJECT_BOOTNODES.
#
# Usage (from your laptop; SSH keys to root on all hosts):
#   export COORDINATED_MESH_GENESIS_CONFIRM=I_UNDERSTAND_WIPE_CHAIN_DATA_ON_ALL_MESH_HOSTS
#   export COINJECT_NODE_IMAGE=ghcr.io/coinjecture-network/coinjecture2.0:latest
#   # optional pin: COINJECT_NODE_IMAGE=ghcr.io/coinjecture-network/coinjecture2.0:sha-4520e7e
#   # private GHCR: export GHCR_TOKEN=...   # optional: GHCR_USER (default GITHUB_ACTOR or COINjecture-Network)
#   ./scripts/deployment/coordinated-mesh-genesis-boot.sh
#
# Optional overrides:
#   CANONICAL_HOST=root@193.203.164.13  CANONICAL_PATH=/opt/coinjecture
#   FOLLOWER1_HOST=root@76.13.101.67  FOLLOWER1_PATH=/opt/coinjecture-src
#   FOLLOWER2_HOST=root@198.199.81.81 FOLLOWER2_PATH=/opt/coinjecture
#   STABLE_IDLE_SECS=45   STABLE_MIN_HEIGHT=1   STABLE_POLL_SECS=15   CANONICAL_WAIT_MAX_SECS=600
#   CANONICAL_RPC_WAIT_MAX_SECS=120   (wait for JSON-RPC after canonical up, before followers)
#   COMPOSE_FILE=docker-compose.yml   FOLLOWER_COMPOSE_EXTRA=docker-compose.follower-no-mine.yml
#
# Image package: https://github.com/COINjecture-Network/COINjecture2.0/pkgs/container/coinjecture2.0
#
set -euo pipefail

CONFIRM="${COORDINATED_MESH_GENESIS_CONFIRM:-}"
if [[ "$CONFIRM" != "I_UNDERSTAND_WIPE_CHAIN_DATA_ON_ALL_MESH_HOSTS" ]]; then
  echo "Refusing: set COORDINATED_MESH_GENESIS_CONFIRM=I_UNDERSTAND_WIPE_CHAIN_DATA_ON_ALL_MESH_HOSTS"
  exit 1
fi

CANONICAL_HOST="${CANONICAL_HOST:-root@193.203.164.13}"
CANONICAL_PATH="${CANONICAL_PATH:-/opt/coinjecture}"
FOLLOWER1_HOST="${FOLLOWER1_HOST:-root@76.13.101.67}"
FOLLOWER1_PATH="${FOLLOWER1_PATH:-/opt/coinjecture-src}"
FOLLOWER2_HOST="${FOLLOWER2_HOST:-root@198.199.81.81}"
FOLLOWER2_PATH="${FOLLOWER2_PATH:-/opt/coinjecture}"
COMPOSE_FILE="${COMPOSE_FILE:-docker-compose.yml}"
FOLLOWER_COMPOSE_EXTRA="${FOLLOWER_COMPOSE_EXTRA:-docker-compose.sync-follower.yml,docker-compose.mesh-bootnode-only.yml,docker-compose.bootnode-health-metrics-only.yml}"

COINJECT_NODE_IMAGE="${COINJECT_NODE_IMAGE:-ghcr.io/coinjecture-network/coinjecture2.0:latest}"
GHCR_USER="${GHCR_USER:-${GITHUB_ACTOR:-COINjecture-Network}}"
GHCR_TOKEN="${GHCR_TOKEN:-}"
NODE_SERVICES="${NODE_SERVICES:-bootnode node1 node2 node3}"
STACK_SERVICES="${STACK_SERVICES:-bootnode node1 node2 node3 api-server}"

STABLE_IDLE_SECS="${STABLE_IDLE_SECS:-45}"
STABLE_MIN_HEIGHT="${STABLE_MIN_HEIGHT:-1}"
STABLE_POLL_SECS="${STABLE_POLL_SECS:-15}"
CANONICAL_WAIT_MAX_SECS="${CANONICAL_WAIT_MAX_SECS:-600}"
CANONICAL_RPC_WAIT_MAX_SECS="${CANONICAL_RPC_WAIT_MAX_SECS:-120}"

SSH_OPTS=( -o BatchMode=yes -o ConnectTimeout=30 -o StrictHostKeyChecking=accept-new )

remote() {
  local host="$1"
  shift
  ssh "${SSH_OPTS[@]}" "$host" "$@"
}

follower_compose_flags() {
  local flags=(-f "$COMPOSE_FILE")
  local IFS=,
  local f
  for f in $FOLLOWER_COMPOSE_EXTRA; do
    f="${f// /}"
    [[ -n "$f" ]] && flags+=(-f "$f")
  done
  printf '%s ' "${flags[@]}"
}

echo "=== Coordinated mesh genesis boot ==="
echo "COINJECT_NODE_IMAGE=$COINJECT_NODE_IMAGE"
echo "Canonical: $CANONICAL_HOST $CANONICAL_PATH"
echo "Followers: $FOLLOWER1_HOST ; $FOLLOWER2_HOST"
echo ""

echo ">>> 1) Stop followers (reduce fork traffic)"
remote "$FOLLOWER1_HOST" "cd '$FOLLOWER1_PATH' && docker compose $(follower_compose_flags) down --remove-orphans"
remote "$FOLLOWER2_HOST" "cd '$FOLLOWER2_PATH' && docker compose $(follower_compose_flags) down --remove-orphans"

echo ">>> 2) Stop canonical"
remote "$CANONICAL_HOST" "cd '$CANONICAL_PATH' && docker compose -f '$COMPOSE_FILE' down --remove-orphans"

echo ">>> 3) Wipe named chain volumes on all hosts (docker compose down -v)"
for spec in "$FOLLOWER1_HOST|$FOLLOWER1_PATH|follower" "$FOLLOWER2_HOST|$FOLLOWER2_PATH|follower" "$CANONICAL_HOST|$CANONICAL_PATH|canonical"; do
  host="${spec%%|*}"
  rest="${spec#*|}"
  path="${rest%%|*}"
  kind="${rest##*|}"
  echo "    down -v: $host $path ($kind)"
  if [[ "$kind" == "follower" ]]; then
    remote "$host" "cd '$path' && docker compose $(follower_compose_flags) down -v --remove-orphans"
  else
    remote "$host" "cd '$path' && docker compose -f '$COMPOSE_FILE' down -v --remove-orphans"
  fi
done

echo ">>> 4) Pull node image + start ONLY canonical stack ($STACK_SERVICES)"
remote "$CANONICAL_HOST" bash -s <<REMOTE
set -euo pipefail
cd "$CANONICAL_PATH"
export COINJECT_NODE_IMAGE="$COINJECT_NODE_IMAGE"
if [[ -n "$GHCR_TOKEN" ]]; then
  echo "$GHCR_TOKEN" | docker login ghcr.io -u "$GHCR_USER" --password-stdin
fi
docker compose -f "$COMPOSE_FILE" pull $NODE_SERVICES
docker compose -f "$COMPOSE_FILE" up -d --no-build $STACK_SERVICES
REMOTE

echo ">>> 5) Wait for canonical JSON-RPC (chain_getInfo; max ${CANONICAL_RPC_WAIT_MAX_SECS}s)"
rpc_deadline=$((SECONDS + CANONICAL_RPC_WAIT_MAX_SECS))
while [[ $SECONDS -lt $rpc_deadline ]]; do
  out="$(remote "$CANONICAL_HOST" "curl -sfS -m 20 -X POST http://127.0.0.1:9933/ -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"method\":\"chain_getInfo\",\"params\":[],\"id\":1}'" || true)"
  h="$(echo "$out" | python3 -c "import json,sys; r=json.load(sys.stdin); print(r.get('result',{}).get('best_height',''))" 2>/dev/null || echo "")"
  if [[ -n "$h" && "$h" =~ ^[0-9]+$ ]]; then
    echo ">>> Canonical RPC up (best_height=$h)"
    break
  fi
  echo "    (RPC not ready yet, retry in 5s)"
  sleep 5
done
if [[ $SECONDS -ge $rpc_deadline ]]; then
  echo "WARN: canonical RPC not ready within ${CANONICAL_RPC_WAIT_MAX_SECS}s — starting followers anyway"
fi

echo ">>> 6) Pull + start follower stacks (sync/RPC only — overlay: $FOLLOWER_COMPOSE_EXTRA)"
remote "$FOLLOWER1_HOST" bash -s <<REMOTE
set -euo pipefail
cd "$FOLLOWER1_PATH"
export COINJECT_NODE_IMAGE="$COINJECT_NODE_IMAGE"
if [[ -n "$GHCR_TOKEN" ]]; then
  echo "$GHCR_TOKEN" | docker login ghcr.io -u "$GHCR_USER" --password-stdin
fi
docker compose $(follower_compose_flags) pull $NODE_SERVICES
docker compose $(follower_compose_flags) up -d --no-build $STACK_SERVICES
REMOTE
remote "$FOLLOWER2_HOST" bash -s <<REMOTE
set -euo pipefail
cd "$FOLLOWER2_PATH"
export COINJECT_NODE_IMAGE="$COINJECT_NODE_IMAGE"
if [[ -n "$GHCR_TOKEN" ]]; then
  echo "$GHCR_TOKEN" | docker login ghcr.io -u "$GHCR_USER" --password-stdin
fi
docker compose $(follower_compose_flags) pull $NODE_SERVICES
docker compose $(follower_compose_flags) up -d --no-build $STACK_SERVICES
REMOTE

echo ">>> 7) Wait for stable tip on canonical (best_height >= $STABLE_MIN_HEIGHT, unchanged for ~${STABLE_IDLE_SECS}s; max ${CANONICAL_WAIT_MAX_SECS}s)"
deadline=$((SECONDS + CANONICAL_WAIT_MAX_SECS))
last_h=""
stable_since=""
while [[ $SECONDS -lt $deadline ]]; do
  out="$(remote "$CANONICAL_HOST" "curl -sfS -m 20 -X POST http://127.0.0.1:9933/ -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"method\":\"chain_getInfo\",\"params\":[],\"id\":1}'" || true)"
  h="$(echo "$out" | python3 -c "import json,sys; r=json.load(sys.stdin); print(r.get('result',{}).get('best_height',''))" 2>/dev/null || echo "")"
  if [[ -z "$h" || ! "$h" =~ ^[0-9]+$ ]]; then
    echo "    (RPC hiccup, retry in ${STABLE_POLL_SECS}s)"
    sleep "$STABLE_POLL_SECS"
    continue
  fi
  if [[ "$h" -lt "$STABLE_MIN_HEIGHT" ]]; then
    echo "    height=$h (waiting for min $STABLE_MIN_HEIGHT — followers must reach :707)"
    last_h=""
    stable_since=""
    sleep "$STABLE_POLL_SECS"
    continue
  fi
  if [[ "$h" == "$last_h" ]]; then
    if [[ -z "$stable_since" ]]; then
      stable_since=$SECONDS
    fi
    elapsed=$((SECONDS - stable_since))
    echo "    height=$h stable_for=${elapsed}s / target_idle=${STABLE_IDLE_SECS}s"
    if [[ "$elapsed" -ge "$STABLE_IDLE_SECS" ]]; then
      echo ">>> Stable tip on canonical: height=$h"
      break
    fi
  else
    echo "    height=$h (advancing)"
    last_h="$h"
    stable_since=""
  fi
  sleep "$STABLE_POLL_SECS"
done
if [[ $SECONDS -ge $deadline ]]; then
  echo "WARN: timed out waiting for stable tip — verify P2P :707 and bootnode logs"
fi

echo ""
echo "=== Done ==="
echo "Check: curl -s http://193.203.164.13:9933/ -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"method\":\"chain_getInfo\",\"params\":[],\"id\":1}' | jq .result.best_height"
