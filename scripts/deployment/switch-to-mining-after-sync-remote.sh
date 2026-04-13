#!/usr/bin/env bash
# Stop sync-follower overlay and start the base compose stack with mining enabled.
# Intended to run **only after** the follower tip is within SYNC_GAP_OK blocks of canonical.
#
# Usage (from your laptop; requires SSH key):
#   export HOST=root@76.13.101.67
#   export REMOTE_PATH=/opt/coinjecture-src
#   export CANONICAL_RPC=http://193.203.164.13:9933/
#   export FOLLOWER_RPC=http://76.13.101.67:9933/
#   ./scripts/deployment/switch-to-mining-after-sync-remote.sh
#
# Optional:
#   SYNC_GAP_OK=25 MINING_SERVICES="bootnode node1 node2 api-server" ./scripts/deployment/switch-to-mining-after-sync-remote.sh
#   SWITCH_TO_MINING_FORCE=1 ...   # skip gap check (dangerous if still far behind)
#
set -euo pipefail

HOST="${HOST:-}"
REMOTE_PATH="${REMOTE_PATH:-}"
CANONICAL_RPC="${CANONICAL_RPC:-http://193.203.164.13:9933/}"
FOLLOWER_RPC="${FOLLOWER_RPC:-http://76.13.101.67:9933/}"
SYNC_GAP_OK="${SYNC_GAP_OK:-10}"
SWITCH_TO_MINING_FORCE="${SWITCH_TO_MINING_FORCE:-0}"
COMPOSE_FILE="${COMPOSE_FILE:-docker-compose.yml}"
SYNC_OVERLAY="${SYNC_COMPOSE_OVERLAY:-docker-compose.sync-follower.yml}"
# All chain roles + API (match docker-compose service names). Omit services you do not run.
MINING_SERVICES="${MINING_SERVICES:-bootnode node1 node2 node3 api-server}"

if [[ -z "$HOST" || -z "$REMOTE_PATH" ]]; then
  echo "Set HOST (e.g. root@76.13.101.67) and REMOTE_PATH (e.g. /opt/coinjecture-src)"
  exit 1
fi

fetch_info() {
  local url="$1"
  curl -sS -m 25 -X POST "$url" -H 'Content-Type: application/json' \
    -d '{"jsonrpc":"2.0","id":1,"method":"chain_getInfo","params":[]}' | python3 -c "
import json, sys
d = json.load(sys.stdin)
if d.get('error'):
    sys.exit(2)
r = d.get('result') or {}
print(r.get('best_height', ''), r.get('genesis_hash', ''), sep='|')
"
}

if [[ "$SWITCH_TO_MINING_FORCE" != "1" ]]; then
  IFS='|' read -r hc gc <<<"$(fetch_info "$CANONICAL_RPC")" || true
  IFS='|' read -r hf gf <<<"$(fetch_info "$FOLLOWER_RPC")" || true
  if [[ -z "$hc" || -z "$hf" ]]; then
    echo "Could not read chain_getInfo from canonical or follower RPC. Fix URLs or retry."
    exit 2
  fi
  if [[ -n "$gc" && -n "$gf" && "$gc" != "$gf" ]]; then
    echo "genesis_hash mismatch — do not enable mining on the wrong chain."
    exit 2
  fi
  gap=$((hc - hf))
  if (( gap < 0 )); then
    echo "Follower ($hf) is ahead of canonical ($hc) — investigate before switching."
    exit 2
  fi
  if (( gap > SYNC_GAP_OK )); then
    echo "Refusing: gap=$gap blocks (canonical=$hc follower=$hf). Max allowed SYNC_GAP_OK=$SYNC_GAP_OK."
    echo "Wait for sync, or set SWITCH_TO_MINING_FORCE=1 (not recommended when far behind)."
    exit 3
  fi
  echo "OK: gap=$gap <= $SYNC_GAP_OK (canonical=$hc follower=$hf), genesis matches."
else
  echo "WARN: SWITCH_TO_MINING_FORCE=1 — skipping RPC gap/genesis checks."
fi

echo ">>> $HOST: cd $REMOTE_PATH — down sync overlay (no -v), up mining stack: $MINING_SERVICES"
ssh -o BatchMode=yes -o StrictHostKeyChecking=accept-new "$HOST" bash -s <<EOF
set -eux
cd "$REMOTE_PATH"
docker compose -f "$COMPOSE_FILE" -f "$SYNC_OVERLAY" down
docker compose -f "$COMPOSE_FILE" up -d --build $MINING_SERVICES
docker compose -f "$COMPOSE_FILE" ps
EOF

echo "Done. Verify: curl ... chain_getInfo on follower; check docker logs for mining."
