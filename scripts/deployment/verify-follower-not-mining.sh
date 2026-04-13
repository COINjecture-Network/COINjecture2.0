#!/usr/bin/env bash
# Prove chain Docker containers are not running with --mine (follower / sync-safe).
# Inspects each container's Config.Cmd JSON for "--mine".
#
# Remote (from laptop):
#   export HOST=root@76.13.101.67
#   ./scripts/deployment/verify-follower-not-mining.sh
#
# On the server:
#   VERIFY_LOCAL=1 ./scripts/deployment/verify-follower-not-mining.sh
#
# Optional:
#   CONTAINERS="coinject-bootnode" VERIFY_LOCAL=1 ./scripts/deployment/verify-follower-not-mining.sh
#
set -euo pipefail

HOST="${HOST:-}"
VERIFY_LOCAL="${VERIFY_LOCAL:-0}"
CONTAINERS="${CONTAINERS:-coinject-bootnode coinject-node1 coinject-node2 coinject-node3}"

check_one() {
  local name="$1"
  if ! docker inspect "$name" &>/dev/null; then
    echo "  (skip) $name — not running"
    return 0
  fi
  local cmd_json
  cmd_json="$(docker inspect "$name" --format '{{json .Config.Cmd}}' 2>/dev/null || echo 'null')"
  echo "  $name Cmd=$cmd_json"
  if echo "$cmd_json" | grep -q '"--mine"'; then
    echo "ERROR: $name has --mine in Cmd — use docker-compose.sync-follower.yml or fix compose command."
    return 1
  fi
}

run_checks() {
  local failed=0
  for c in $CONTAINERS; do
    check_one "$c" || failed=1
  done
  return "$failed"
}

echo "Checking containers: $CONTAINERS"

if [[ "$VERIFY_LOCAL" == 1 ]]; then
  run_checks
else
  if [[ -z "$HOST" ]]; then
    echo "Set HOST (e.g. root@76.13.101.67) or VERIFY_LOCAL=1 on the server."
    exit 1
  fi
  ssh -o BatchMode=yes -o StrictHostKeyChecking=accept-new "$HOST" bash -s <<EOF
set -euo pipefail
CONTAINERS="$CONTAINERS"
check_one() {
  local name="\$1"
  if ! docker inspect "\$name" &>/dev/null; then
    echo "  (skip) \$name — not running"
    return 0
  fi
  local cmd_json
  cmd_json="\$(docker inspect "\$name" --format '{{json .Config.Cmd}}' 2>/dev/null || echo 'null')"
  echo "  \$name Cmd=\$cmd_json"
  if echo "\$cmd_json" | grep -q '"--mine"'; then
    echo "ERROR: \$name has --mine in Cmd — use docker-compose.sync-follower.yml or fix compose command."
    return 1
  fi
}
failed=0
for c in \$CONTAINERS; do
  check_one "\$c" || failed=1
done
exit "\$failed"
EOF
fi

echo "OK: no --mine in Cmd for running containers checked."
