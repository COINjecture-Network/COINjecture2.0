#!/usr/bin/env bash
# Run api-server + Vite web app for local auth / email signup testing.
# Requires: Rust toolchain (cargo), Node (npm), and api-server/.env configured.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ ! -f "$ROOT/web/coinjecture-evolved-main/.env" ]]; then
  echo "Creating web/coinjecture-evolved-main/.env for local API ..."
  cat > "$ROOT/web/coinjecture-evolved-main/.env" << 'EOF'
VITE_RPC_URL=http://localhost:9933
VITE_API_URL=http://localhost:3030
EOF
fi

cleanup() {
  [[ -n "${API_PID:-}" ]] && kill "$API_PID" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

echo "Starting api-server (http://localhost:3030) ..."
(cd "$ROOT/api-server" && cargo run) &
API_PID=$!

echo "Waiting for API to listen..."
for i in {1..60}; do
  if curl -sf "http://127.0.0.1:3030/health" >/dev/null 2>&1; then
    echo "API is up."
    break
  fi
  sleep 1
  if ! kill -0 "$API_PID" 2>/dev/null; then
    echo "api-server exited early — check api-server/.env and run: cd api-server && cargo run"
    exit 1
  fi
done

echo "Starting Vite (http://localhost:8080) ..."
cd "$ROOT/web/coinjecture-evolved-main"
exec npm run dev
