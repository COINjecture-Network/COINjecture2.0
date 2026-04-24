#!/usr/bin/env bash
# Compare chain_getBlock between two JSON-RPC endpoints to find the first height
# in a range where header.prev_hash differs (fork / chain divergence signal).
#
# Usage:
#   ./scripts/compare-fork-blocks.sh
#   HOST1_RPC=http://a:9933/ HOST2_RPC=http://b:9933/ START_HEIGHT=0 END_HEIGHT=200 \
#     ./scripts/compare-fork-blocks.sh
#   ./scripts/compare-fork-blocks.sh http://HOST1:9933/ http://HOST2:9933/ 116150 116200
#
# Requires: curl, python3
set -euo pipefail

HOST1_RPC="${HOST1_RPC:-http://193.203.164.13:9933/}"
HOST2_RPC="${HOST2_RPC:-http://76.13.101.67:9933/}"
START_HEIGHT="${START_HEIGHT:-116150}"
END_HEIGHT="${END_HEIGHT:-116200}"

[[ "${1:-}" ]] && HOST1_RPC="$1"
[[ "${2:-}" ]] && HOST2_RPC="$2"
[[ "${3:-}" ]] && START_HEIGHT="$3"
[[ "${4:-}" ]] && END_HEIGHT="$4"

get_prev_hash_json() {
  local url="$1" height="$2"
  curl -sS -m 20 -X POST "$url" \
    -H 'Content-Type: application/json' \
    -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"chain_getBlock\",\"params\":[$height]}" \
  | python3 -c "
import json, sys
try:
    d = json.load(sys.stdin)
except json.JSONDecodeError:
    print('')
    sys.exit(2)
r = d.get('result')
if not r or 'header' not in r:
    print('')
    sys.exit(3)
ph = r['header'].get('prev_hash')
print(json.dumps(ph, separators=(',', ':')))
"
}

echo "Scanning heights $START_HEIGHT–$END_HEIGHT for prev_hash divergence"
echo "HOST1: $HOST1_RPC"
echo "HOST2: $HOST2_RPC"
echo "---"

first_divergence=""
for h in $(seq "$START_HEIGHT" "$END_HEIGHT"); do
  h1="$(get_prev_hash_json "$HOST1_RPC" "$h" || true)"
  h2="$(get_prev_hash_json "$HOST2_RPC" "$h" || true)"
  if [[ -z "$h1" || -z "$h2" ]]; then
    echo "RPC error at height $h (empty prev_hash payload)"
    continue
  fi
  if [[ "$h1" != "$h2" ]]; then
    echo "DIVERGE at height $h"
    echo "  HOST1 prev_hash: $h1"
    echo "  HOST2 prev_hash: $h2"
    first_divergence="$h"
    break
  else
    echo "ok h=$h"
  fi
done

if [[ -z "$first_divergence" ]]; then
  echo "No divergence in range $START_HEIGHT–$END_HEIGHT. Expand START_HEIGHT/END_HEIGHT."
fi
