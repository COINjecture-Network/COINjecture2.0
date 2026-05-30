#!/usr/bin/env bash
# On-host fork self-healing for mesh VPSs. Run via cron/systemd (no laptop SSH required).
#
# Works with:
#   - Single bootnode per host (mesh followers at bootstrap)
#   - Full mining fleet (bootnode + node1–node3, join-and-mine / sync-gated)
#
# Detects:
#   - Local bootnode tip hash diverges from remote mesh peers
#   - Any **running** local replica (node1–node3) tip hash diverges from local bootnode
#   - Sync stall patterns in bootnode logs
#
# Actions (automatic):
#   - Soft restart on first detection; after grace period:
#       * Follower (bootnode-only): wipe bootnode-data and resync from mesh
#       * Fleet host: clone bootnode-data → each diverged replica volume; mesh resync if bootnode forked
#
# Usage:
#   MESH_FLEET=bootnode-only MESH_ROLE=follower COMPOSE_DIR=/opt/coinjecture-src \
#     ./scripts/deployment/mesh-node-autorepair.sh
#   MESH_FLEET=full MESH_ROLE=canonical COMPOSE_DIR=/opt/coinjecture \
#     ./scripts/deployment/mesh-node-autorepair.sh
#
set -euo pipefail

MESH_ROLE="${MESH_ROLE:-follower}"       # follower | canonical (logging only)
MESH_FLEET="${MESH_FLEET:-bootnode-only}" # bootnode-only | full
COMPOSE_DIR="${COMPOSE_DIR:-/opt/coinjecture}"
COMPOSE_FILE="${COMPOSE_FILE:-docker-compose.yml}"
SYNC_OVERLAY="${SYNC_COMPOSE_OVERLAY:-docker-compose.sync-follower.yml}"
BOOTNODE_HEALTH_OVERLAY="${BOOTNODE_HEALTH_COMPOSE_OVERLAY:-docker-compose.bootnode-health-metrics-only.yml}"
BOOTNODE_ONLY_OVERLAY="${BOOTNODE_ONLY_OVERLAY:-docker-compose.mesh-bootnode-only.yml}"
FLEET_OVERLAY="${FLEET_OVERLAY:-docker-compose.mesh-fleet.yml}"
LOCAL_RAM="${LOCAL_RAM_OVERLAY:-docker-compose.local-ram.yml}"

REPAIR_INTERVAL="${REPAIR_INTERVAL:-0}"
GRACE_SECS="${GRACE_SECS:-120}"
STALL_LOG_LINES="${STALL_LOG_LINES:-400}"
MESH_PEER_RPC_TIMEOUT="${MESH_PEER_RPC_TIMEOUT:-12}"

STATE_DIR="${STATE_DIR:-/var/lib/coinjecture-mesh-autorepair}"
mkdir -p "$STATE_DIR"

# svc|host_rpc_port|volume_name
REPLICA_SPECS=(
  "node1|9934|coinjecture_node1-data"
  "node2|9935|coinjecture_node2-data"
  "node3|9936|coinjecture_node3-data"
)

log() { echo "[$(date -u +"%Y-%m-%dT%H:%M:%SZ")] mesh-autorepair: $*"; }

compose_flags() {
  local flags=(-f "$COMPOSE_FILE" -f "$SYNC_OVERLAY")
  if [[ "$MESH_FLEET" == "full" ]]; then
    flags+=(-f "$FLEET_OVERLAY" -f "$LOCAL_RAM")
  else
    flags+=(-f "$BOOTNODE_ONLY_OVERLAY")
  fi
  flags+=(-f "$BOOTNODE_HEALTH_OVERLAY")
  printf '%s ' "${flags[@]}"
}

stack_services() {
  if [[ "$MESH_FLEET" == "full" ]]; then
    echo "bootnode node1 node2 node3 api-server"
  else
    echo "bootnode api-server"
  fi
}

rpc_post() {
  local url="$1"
  curl -sf --max-time "$MESH_PEER_RPC_TIMEOUT" -X POST "${url%/}/" \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc":"2.0","method":"chain_getInfo","params":[],"id":1}'
}

hash6() {
  python3 - "$1" <<'PY'
import json, sys
d = json.loads(sys.argv[1])
r = d.get("result") or {}
h = r.get("best_hash")
if isinstance(h, list):
    print("".join(f"{b:02x}" for b in h[:6]))
else:
    print(str(h)[:12])
PY
}

height_of() {
  python3 - "$1" <<'PY'
import json, sys
d = json.loads(sys.argv[1])
print((d.get("result") or {}).get("best_height", 0))
PY
}

container_running() {
  docker ps --format '{{.Names}}' 2>/dev/null | grep -qx "coinject-$1"
}

read_local_bootnode_info() {
  rpc_post "http://127.0.0.1:9933"
}

mesh_peer_urls() {
  local env_file="$COMPOSE_DIR/.env"
  [[ -f "$env_file" ]] || return 0
  local bootnodes
  bootnodes="$(grep -E '^(COINJECT_BOOTNODES|BOOTNODES)=' "$env_file" | head -1 | cut -d= -f2- | tr -d '"'"'" || true)"
  [[ -n "$bootnodes" ]] || return 0
  IFS=',' read -ra peers <<<"$bootnodes"
  for entry in "${peers[@]}"; do
    entry="${entry// /}"
    [[ -n "$entry" ]] || continue
    local host="${entry%%:*}"
    [[ "$host" =~ ^[0-9.]+$ ]] || continue
    echo "http://${host}:9933"
  done
}

log_stall_detected() {
  docker logs coinject-bootnode 2>&1 | tail -n "$STALL_LOG_LINES" | grep -qE \
    'sync batch made no progress|sync extension not on peer heavy chain|historical sync block conflicts'
}

bootnode_diverged_from_mesh() {
  local local_json peer_url peer_json lh ph local_h
  local_json="$(read_local_bootnode_info 2>/dev/null)" || return 1
  local_h="$(height_of "$local_json")"
  lh="$(hash6 "$local_json")"
  [[ "$local_h" -gt 0 ]] || return 1

  local agree=0 total=0
  while IFS= read -r peer_url; do
    [[ -n "$peer_url" ]] || continue
    peer_json="$(rpc_post "$peer_url" 2>/dev/null)" || continue
    ph="$(height_of "$peer_json")"
    if (( ph >= local_h - 2 && ph <= local_h + 2 )); then
      total=$((total + 1))
      [[ "$(hash6 "$peer_json")" == "$lh" ]] && agree=$((agree + 1))
    fi
  done < <(mesh_peer_urls)

  (( total >= 1 && agree == 0 ))
}

replica_diverged_from_bootnode() {
  local svc="$1" port="$2"
  container_running "$svc" || return 1
  local b r
  b="$(read_local_bootnode_info 2>/dev/null)" || return 1
  r="$(rpc_post "http://127.0.0.1:${port}" 2>/dev/null)" || return 1
  [[ "$(hash6 "$b")" != "$(hash6 "$r")" ]]
}

any_replica_diverged() {
  [[ "$MESH_FLEET" == "full" ]] || return 1
  local spec svc port vol
  for spec in "${REPLICA_SPECS[@]}"; do
    IFS='|' read -r svc port vol <<<"$spec"
    if replica_diverged_from_bootnode "$svc" "$port"; then
      return 0
    fi
  done
  return 1
}

clone_bootnode_to_volume() {
  local svc="$1" vol="$2"
  log "cloning bootnode-data → ${vol} (service ${svc})"
  docker compose $(compose_flags) stop "$svc" 2>/dev/null || true
  docker run --rm \
    -v coinjecture_bootnode-data:/src:ro \
    -v "${vol}:/dst" \
    alpine:3.20 sh -c 'rm -rf /dst/* /dst/.[!.]* 2>/dev/null; cp -a /src/. /dst/'
  docker compose $(compose_flags) up -d --no-build "$svc"
}

align_all_diverged_replicas() {
  local spec svc port vol fixed=0
  for spec in "${REPLICA_SPECS[@]}"; do
    IFS='|' read -r svc port vol <<<"$spec"
    if replica_diverged_from_bootnode "$svc" "$port"; then
      clone_bootnode_to_volume "$svc" "$vol"
      fixed=$((fixed + 1))
    fi
  done
  log "aligned ${fixed} replica(s) from bootnode"
}

bootnode_resync_wipe() {
  log "wiping bootnode-data and resyncing from mesh"
  docker compose $(compose_flags) stop $(stack_services) 2>/dev/null || true
  docker volume rm -f coinjecture_bootnode-data 2>/dev/null || true
  docker compose $(compose_flags) up -d --no-build $(stack_services)
}

soft_restart_stack() {
  log "soft restart (fleet=${MESH_FLEET})"
  if [[ "$MESH_FLEET" == "full" ]]; then
    docker compose $(compose_flags) restart bootnode node1 node2 node3 2>/dev/null || \
      docker compose $(compose_flags) restart bootnode
  else
    docker compose $(compose_flags) restart bootnode
  fi
}

needs_repair() {
  bootnode_diverged_from_mesh && return 0
  log_stall_detected && return 0
  any_replica_diverged && return 0
  return 1
}

hard_recovery() {
  if bootnode_diverged_from_mesh; then
    bootnode_resync_wipe
    if [[ "$MESH_FLEET" == "full" ]]; then
      sleep 30
      align_all_diverged_replicas
    fi
    return 0
  fi
  if [[ "$MESH_FLEET" == "full" ]] && any_replica_diverged; then
    align_all_diverged_replicas
    return 0
  fi
  if log_stall_detected; then
    soft_restart_stack
  fi
}

repair_once() {
  cd "$COMPOSE_DIR"

  if ! needs_repair; then
    rm -f "$STATE_DIR/suspect_since"
    return 0
  fi

  # Fast path: internal replica drift without mesh fork — align immediately (no grace).
  if [[ "$MESH_FLEET" == "full" ]] && any_replica_diverged && ! bootnode_diverged_from_mesh; then
    align_all_diverged_replicas
    return 0
  fi

  local now suspect_since
  now="$(date +%s)"
  if [[ ! -f "$STATE_DIR/suspect_since" ]]; then
    echo "$now" >"$STATE_DIR/suspect_since"
    soft_restart_stack
    log "suspect fork/stall — soft restart (grace ${GRACE_SECS}s)"
    return 0
  fi

  suspect_since="$(cat "$STATE_DIR/suspect_since")"
  if (( now - suspect_since < GRACE_SECS )); then
    log "grace period ($((now - suspect_since))/${GRACE_SECS}s)"
    return 0
  fi

  hard_recovery
  rm -f "$STATE_DIR/suspect_since"
  log "hard recovery completed"
}

if [[ "$REPAIR_INTERVAL" -gt 0 ]]; then
  log "loop interval=${REPAIR_INTERVAL}s role=$MESH_ROLE fleet=$MESH_FLEET dir=$COMPOSE_DIR"
  while true; do
    repair_once || log "repair_once failed (will retry)"
    sleep "$REPAIR_INTERVAL"
  done
else
  repair_once
fi
