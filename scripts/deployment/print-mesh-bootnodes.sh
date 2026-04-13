#!/usr/bin/env bash
# Print COINJECT_BOOTNODES / BOOTNODES lines for .env (comma list of mesh peers **excluding** self).
#
# Usage:
#   SELF_IP=198.199.81.81 PEERS=193.203.164.13,76.13.101.67,198.199.81.81 ./scripts/deployment/print-mesh-bootnodes.sh
#
set -euo pipefail

SELF_IP="${SELF_IP:-}"
PEERS="${PEERS:-}"

if [[ -z "$SELF_IP" || -z "$PEERS" ]]; then
  echo "Set SELF_IP (this host public IPv4) and PEERS (comma-separated IPv4s of all mesh members, may include self)."
  echo "Example: SELF_IP=198.199.81.81 PEERS=193.203.164.13,76.13.101.67,198.199.81.81 $0"
  exit 1
fi

list=()
IFS=',' read -r -a raw <<<"$PEERS"
for ip in "${raw[@]}"; do
  ip="${ip// /}"
  [[ -z "$ip" ]] && continue
  if [[ "$ip" == "$SELF_IP" ]]; then
    continue
  fi
  list+=("${ip}:707")
done

if [[ ${#list[@]} -eq 0 ]]; then
  echo "ERROR: after removing self, no peers left. Check SELF_IP and PEERS."
  exit 1
fi

joined=$(IFS=,; echo "${list[*]}")
echo "COINJECT_BOOTNODES=$joined"
echo "BOOTNODES=$joined"
