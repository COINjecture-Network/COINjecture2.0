#!/bin/bash
set -e

echo "=== COINjecture CI Smoke Test ==="

echo "1. Building Docker images..."
docker-compose build

echo "2. Starting 4-node testnet..."
docker-compose up -d

echo "3. Waiting for health checks (60s)..."
sleep 60

echo "4. Checking health endpoints..."
for port in 9090 9091 9092 9093; do
  STATUS=$(curl -sf http://localhost:$port/health || echo "FAIL")
  echo "   Port $port: $STATUS"
  if [ "$STATUS" = "FAIL" ]; then
    echo "   FAILED: Node on metrics port $port is not healthy"
    docker-compose logs
    docker-compose down -v
    exit 1
  fi
done

echo "5. Checking RPC endpoints..."
for port in 9933 9934 9935 9936; do
  RESULT=$(curl -sf -X POST http://localhost:$port \
    -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","method":"chain_getInfo","params":[],"id":1}' || echo "FAIL")
  echo "   RPC port $port: $(echo $RESULT | head -c 100)"
done

echo "6. Tearing down..."
docker-compose down -v

echo "=== Smoke test passed ==="
