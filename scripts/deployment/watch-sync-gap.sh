#!/usr/bin/env bash
# Poll two JSON-RPC endpoints and print best_height gap until follower is near canonical.
#
# Defaults match the two-host example in README (override with env or args).
#
# Usage:
#   ./scripts/deployment/watch-sync-gap.sh
#   CANONICAL_RPC=http://193.203.164.13:9933/ FOLLOWER_RPC=http://76.13.101.67:9933/ \
#     SYNC_WATCH_INTERVAL=120 SYNC_GAP_OK=10 ./scripts/deployment/watch-sync-gap.sh
#   ./scripts/deployment/watch-sync-gap.sh http://HOST1:9933/ http://HOST2:9933/
#   ONE_SHOT=1 ./scripts/deployment/watch-sync-gap.sh   # print once and exit
#   EXIT_WHEN_CAUGHT_UP=1 ./scripts/deployment/watch-sync-gap.sh   # exit 0 as soon as gap <= SYNC_GAP_OK (genesis ok)
#
# When gap <= SYNC_GAP_OK, the script prints a reminder to switch compose (see README).
# Press Ctrl+C to stop (unless EXIT_WHEN_CAUGHT_UP exited already).
#
set -euo pipefail

CANONICAL_RPC="${CANONICAL_RPC:-http://193.203.164.13:9933/}"
FOLLOWER_RPC="${FOLLOWER_RPC:-http://76.13.101.67:9933/}"
SYNC_WATCH_INTERVAL="${SYNC_WATCH_INTERVAL:-60}"
SYNC_GAP_OK="${SYNC_GAP_OK:-10}"
ONE_SHOT="${ONE_SHOT:-0}"
EXIT_WHEN_CAUGHT_UP="${EXIT_WHEN_CAUGHT_UP:-0}"

if [[ "${1:-}" ]]; then
  CANONICAL_RPC="$1"
fi
if [[ "${2:-}" ]]; then
  FOLLOWER_RPC="$2"
fi

fetch_line() {
  local url="$1"
  curl -sS -m 20 -X POST "$url" -H 'Content-Type: application/json' \
    -d '{"jsonrpc":"2.0","id":1,"method":"chain_getInfo","params":[]}' | python3 -c "
import json, sys
try:
    d = json.load(sys.stdin)
except json.JSONDecodeError:
    print('', '', '', sep='|')
    sys.exit(2)
if d.get('error'):
    print('', '', '', sep='|')
    sys.exit(3)
r = d.get('result') or {}
h = r.get('best_height')
p = r.get('peer_count')
g = r.get('genesis_hash') or ''
print('' if h is None else int(h), '' if p is None else int(p), g, sep='|')
"
}

echo "Watching: canonical=$CANONICAL_RPC follower=$FOLLOWER_RPC interval=${SYNC_WATCH_INTERVAL}s gap_ok=${SYNC_GAP_OK}"
echo "If follower stalls, on that host: docker logs --tail 500 coinject-bootnode"
echo "---"

while true; do
  ts="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
  line_c="$(fetch_line "$CANONICAL_RPC")" || line_c="|||"
  line_f="$(fetch_line "$FOLLOWER_RPC")" || line_f="|||"

  IFS='|' read -r hc pc gc <<<"$line_c"
  IFS='|' read -r hf pf gf <<<"$line_f"

  if [[ -z "$hc" || -z "$hf" ]]; then
    echo "[$ts] RPC error or timeout (canonical_height=${hc:-?} follower_height=${hf:-?})"
  else
    gap=$((hc - hf))
    gen_ok="ok"
    if [[ -n "$gc" && -n "$gf" && "$gc" != "$gf" ]]; then
      gen_ok="MISMATCH genesis — fix bootnodes / chain before trusting height"
    fi
    echo "[$ts] canonical h=$hc peers=${pc:-?} | follower h=$hf peers=${pf:-?} | gap=$gap | genesis $gen_ok"
    if [[ "$gen_ok" == ok && "$gap" -le "$SYNC_GAP_OK" && "$gap" -ge 0 ]]; then
      echo ">>> Gap <= ${SYNC_GAP_OK}: switch to base compose without -v (see README \"Production deployment and chain health\")."
      if [[ "$EXIT_WHEN_CAUGHT_UP" == 1 ]]; then
        exit 0
      fi
    fi
  fi

  if [[ "$ONE_SHOT" == 1 ]]; then
    exit 0
  fi
  sleep "$SYNC_WATCH_INTERVAL"
done
