#!/usr/bin/env bash
# Poll until follower is within SYNC_GAP_OK of canonical, then run the guarded mining switch over SSH.
#
# Prerequisites:
#   export HOST=root@76.13.101.67
#   export REMOTE_PATH=/opt/coinjecture-src
# Optional RPC overrides (defaults: production HOST1 / HOST2):
#   CANONICAL_RPC FOLLOWER_RPC
# Polling:
#   SYNC_WATCH_INTERVAL (default 60), SYNC_GAP_OK (default 10)
#
# Usage:
#   export HOST=root@76.13.101.67 REMOTE_PATH=/opt/coinjecture-src
#   ./scripts/deployment/wait-for-sync-then-mining.sh
#   ./scripts/deployment/wait-for-sync-then-mining.sh http://CANON:9933/ http://FOLLOW:9933/
#
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
export SYNC_WATCH_INTERVAL="${SYNC_WATCH_INTERVAL:-60}"
export SYNC_GAP_OK="${SYNC_GAP_OK:-10}"
export EXIT_WHEN_CAUGHT_UP=1

if [[ "${1:-}" || "${2:-}" ]]; then
  "$ROOT/scripts/deployment/watch-sync-gap.sh" "$@"
else
  "$ROOT/scripts/deployment/watch-sync-gap.sh"
fi

echo ""
echo ">>> Caught up — running switch-to-mining-after-sync-remote.sh"
exec "$ROOT/scripts/deployment/switch-to-mining-after-sync-remote.sh"
