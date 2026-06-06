#!/usr/bin/env bash
# Wipe chain Docker state (optional) and deploy a COINjecture **mesh peer** on a fresh Ubuntu droplet.
# Dials existing CPP peers via COINJECT_BOOTNODES (port **707**).
#
# Default stack: **bootnode + api-server** with **sync-follower** overlay (mesh bootnodes + `--mine`).
# api-server needs Supabase — use DEPLOY_BOOTNODE_ONLY=1 for **bootnode only** (pure P2P + RPC 9933).
#
# Usage:
#   export HOST=root@164.90.173.188
#   export WIPE_CONFIRM=I_WIPE_DROPLET_CHAIN_DATA
#   export MESH_BOOTNODES=193.203.164.13:707,76.13.101.67:707
#   export DEPLOY_BOOTNODE_ONLY=1
#   export SSH_IDENTITY="$HOME/.ssh/id_ed25519"   # private key that matches the key on the droplet
#   ./scripts/deployment/bootstrap-digitalocean-mesh-node.sh
#
# With api-server:
#   export SUPABASE_URL=... SUPABASE_ANON_KEY=... SUPABASE_JWT_SECRET=...
#   unset DEPLOY_BOOTNODE_ONLY   # or =0
#   ./scripts/deployment/bootstrap-digitalocean-mesh-node.sh
#
# DigitalOcean: inbound **TCP 707** (CPP), **9933** (JSON-RPC), **3030** (API), **9090** (optional).
#
set -euo pipefail

HOST="${HOST:-}"
REMOTE_PATH="${REMOTE_PATH:-/opt/coinjecture}"
REPO_URL="${REPO_URL:-https://github.com/COINjecture-Network/COINjecture2.0.git}"
GIT_REF="${GIT_REF:-main}"
WIPE_CONFIRM="${WIPE_CONFIRM:-}"
MESH_BOOTNODES="${MESH_BOOTNODES:-193.203.164.13:707,76.13.101.67:707}"
DEPLOY_BOOTNODE_ONLY="${DEPLOY_BOOTNODE_ONLY:-0}"
SSH_IDENTITY="${SSH_IDENTITY:-}"

if [[ -z "$HOST" ]]; then
  echo "Set HOST=root@<droplet-ip>"
  exit 1
fi

if [[ "$WIPE_CONFIRM" != "I_WIPE_DROPLET_CHAIN_DATA" ]]; then
  echo "Refusing: export WIPE_CONFIRM=I_WIPE_DROPLET_CHAIN_DATA to remove $REMOTE_PATH and Docker chain volumes."
  exit 1
fi

if [[ "$DEPLOY_BOOTNODE_ONLY" != "1" ]]; then
  if [[ -z "${SUPABASE_URL:-}" || -z "${SUPABASE_ANON_KEY:-}" || -z "${SUPABASE_JWT_SECRET:-}" ]]; then
    echo "Set Supabase env vars for api-server, or DEPLOY_BOOTNODE_ONLY=1 for bootnode only."
    exit 1
  fi
fi

echo "=== Bootstrap mesh node ==="
echo "HOST=$HOST REMOTE_PATH=$REMOTE_PATH"
echo "MESH_BOOTNODES=$MESH_BOOTNODES DEPLOY_BOOTNODE_ONLY=$DEPLOY_BOOTNODE_ONLY"
echo ""

ssh_args=( -o BatchMode=yes -o StrictHostKeyChecking=accept-new )
if [[ -n "$SSH_IDENTITY" ]]; then
  if [[ ! -f "$SSH_IDENTITY" ]]; then
    echo "SSH_IDENTITY file not found: $SSH_IDENTITY"
    exit 1
  fi
  ssh_args+=( -i "$SSH_IDENTITY" -o IdentitiesOnly=yes )
fi

ssh "${ssh_args[@]}" "$HOST" \
  REMOTE_PATH="$REMOTE_PATH" \
  REPO_URL="$REPO_URL" \
  GIT_REF="$GIT_REF" \
  MESH_BOOTNODES="$MESH_BOOTNODES" \
  DEPLOY_BOOTNODE_ONLY="$DEPLOY_BOOTNODE_ONLY" \
  SUPABASE_URL="${SUPABASE_URL:-}" \
  SUPABASE_ANON_KEY="${SUPABASE_ANON_KEY:-}" \
  SUPABASE_JWT_SECRET="${SUPABASE_JWT_SECRET:-}" \
  SUPABASE_SERVICE_ROLE_KEY="${SUPABASE_SERVICE_ROLE_KEY:-}" \
  bash -s <<'REMOTE'
set -euxo pipefail
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq ca-certificates curl git jq openssl

if ! command -v docker >/dev/null 2>&1; then
  curl -fsSL https://get.docker.com | sh
fi

if [[ -d "$REMOTE_PATH" ]]; then
  cd "$REMOTE_PATH" || exit 1
  if [[ -f docker-compose.yml ]]; then
    docker compose -f docker-compose.yml -f docker-compose.sync-follower.yml down -v --remove-orphans 2>/dev/null || true
    docker compose -f docker-compose.yml down -v --remove-orphans 2>/dev/null || true
  fi
  cd /
  rm -rf "$REMOTE_PATH"
fi

mkdir -p "$(dirname "$REMOTE_PATH")"
git clone --depth 1 --branch "$GIT_REF" "$REPO_URL" "$REMOTE_PATH"
cd "$REMOTE_PATH"

MINER="$(openssl rand -hex 32)"
{
  echo "RUST_LOG=info"
  echo "RUST_BACKTRACE=0"
  echo "MINER_ADDRESS=$MINER"
  echo "COINJECT_BOOTNODES=$MESH_BOOTNODES"
  echo "BOOTNODES=$MESH_BOOTNODES"
  echo "HUGGINGFACE_TOKEN="
  echo "HF_TOKEN="
  echo "HF_DATASET_NAME=COINjecture/NP-Solutions"
  echo "RPC_MAX_BODY_BYTES=67108864"
  echo "RPC_TIMEOUT_SECS=300"
  if [[ "$DEPLOY_BOOTNODE_ONLY" != "1" ]]; then
    echo "SUPABASE_URL=$SUPABASE_URL"
    echo "SUPABASE_ANON_KEY=$SUPABASE_ANON_KEY"
    echo "SUPABASE_JWT_SECRET=$SUPABASE_JWT_SECRET"
    echo "SUPABASE_SERVICE_ROLE_KEY=$SUPABASE_SERVICE_ROLE_KEY"
  fi
} > .env

if [[ "$DEPLOY_BOOTNODE_ONLY" == "1" ]]; then
  docker compose -f docker-compose.yml -f docker-compose.sync-follower.yml up -d --build bootnode
else
  docker compose -f docker-compose.yml -f docker-compose.sync-follower.yml up -d --build bootnode api-server
fi

docker compose -f docker-compose.yml -f docker-compose.sync-follower.yml ps

ok=0
for i in $(seq 1 50); do
  if curl -sfS -m 10 -X POST http://127.0.0.1:9933/ \
      -H 'Content-Type: application/json' \
      -d '{"jsonrpc":"2.0","method":"chain_getInfo","params":[],"id":1}' | jq -e .result.best_height >/dev/null 2>&1; then
    ok=1
    break
  fi
  sleep 5
done
if [[ "$ok" -eq 1 ]]; then
  curl -sfS -m 15 -X POST http://127.0.0.1:9933/ \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc":"2.0","method":"chain_getInfo","params":[],"id":1}' | jq .
else
  echo "WARN: RPC not ready — check: docker logs coinject-bootnode"
fi
REMOTE

echo ""
echo "=== Done ==="
echo "Open DO Cloud Firewall: TCP 707 from peers (or 0.0.0.0/0 for test), 9933, 3030 if API."
echo "Add this node to the mesh: append ${HOST#*@}:707 to COINJECT_BOOTNODES on other hosts' .env then restart compose."
