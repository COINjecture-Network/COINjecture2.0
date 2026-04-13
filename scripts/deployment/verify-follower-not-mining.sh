#!/usr/bin/env bash
# Deprecated: `docker-compose.sync-follower.yml` now keeps `--mine` on all node services;
# catch-up is handled inside the node (sync wait + `peer_consensus.should_mine()`).
# Use verify-node-mining-enabled.sh instead.
set -euo pipefail
exec "$(cd "$(dirname "$0")" && pwd)/verify-node-mining-enabled.sh" "$@"
