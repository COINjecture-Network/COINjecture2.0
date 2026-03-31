#!/bin/bash
set -euo pipefail

echo "COINjecture 2.0 — Starting Production Stack"
echo "================================================"

if [ ! -f .env.production ]; then
    echo "No .env.production found. Copying from example..."
    cp .env.production.example .env.production
    echo "Please edit .env.production with your actual values."
    exit 1
fi

export $(grep -v '^#' .env.production | xargs)

echo "Building containers..."
docker compose -f docker-compose.production.yml build

echo "Starting stack..."
docker compose -f docker-compose.production.yml up -d

echo ""
echo "Stack is running!"
echo ""
echo "  API Server:  http://localhost:3030/health"
echo "  Node RPC:    http://localhost:9933"
echo "  Grafana:     http://localhost:3001 (admin / ${GRAFANA_PASSWORD:-coinjecture})"
echo "  Prometheus:  http://localhost:9091"
echo ""
echo "  View logs:   docker compose -f docker-compose.production.yml logs -f"
echo "  Stop:        docker compose -f docker-compose.production.yml down"
