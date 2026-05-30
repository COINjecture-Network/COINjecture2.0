#!/usr/bin/env bash
# Install on-host mesh fork autorepair as a systemd service (runs on the VPS).
#
# Usage (on the VPS as root):
#   MESH_ROLE=follower COMPOSE_DIR=/opt/coinjecture-src \
#     ./scripts/deployment/install-mesh-autorepair-timer.sh
#
set -euo pipefail

MESH_ROLE="${MESH_ROLE:-follower}"
COMPOSE_DIR="${COMPOSE_DIR:-/opt/coinjecture}"
REPAIR_INTERVAL="${REPAIR_INTERVAL:-300}"
SCRIPT_PATH="${SCRIPT_PATH:-$COMPOSE_DIR/scripts/deployment/mesh-node-autorepair.sh}"

if [[ ! -x "$SCRIPT_PATH" && ! -f "$SCRIPT_PATH" ]]; then
  echo "Missing $SCRIPT_PATH — deploy repo to COMPOSE_DIR first."
  exit 1
fi
chmod +x "$SCRIPT_PATH" 2>/dev/null || true

UNIT=/etc/systemd/system/coinjecture-mesh-autorepair.service
cat >"$UNIT" <<EOF
[Unit]
Description=COINjecture mesh fork autorepair ($MESH_ROLE)
After=docker.service
Requires=docker.service

[Service]
Type=simple
Environment=MESH_ROLE=$MESH_ROLE
Environment=MESH_FLEET=${MESH_FLEET:-bootnode-only}
Environment=COMPOSE_DIR=$COMPOSE_DIR
Environment=REPAIR_INTERVAL=$REPAIR_INTERVAL
Environment=GRACE_SECS=120
ExecStart=$SCRIPT_PATH
Restart=always
RestartSec=30

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable --now coinjecture-mesh-autorepair.service
systemctl status coinjecture-mesh-autorepair.service --no-pager || true
echo "Installed coinjecture-mesh-autorepair.service (interval=${REPAIR_INTERVAL}s)"
