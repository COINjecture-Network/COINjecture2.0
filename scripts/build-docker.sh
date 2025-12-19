#!/bin/bash
# Build Docker image for COINjecture CPP Network
# Usage: ./scripts/build-docker.sh [tag]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

TAG="${1:-coinject-cpp:latest}"

echo "=========================================="
echo "🐳 Building COINjecture CPP Docker Image"
echo "=========================================="
echo "Tag: $TAG"
echo ""

# Build Docker image for linux/amd64 (DigitalOcean droplets)
# Use buildx for cross-platform builds (Mac ARM64 -> Linux AMD64)
echo "Building for linux/amd64 platform..."
docker buildx build --platform linux/amd64 -f Dockerfile.cpp -t "$TAG" --load .

echo ""
echo "✅ Docker image built successfully: $TAG"
echo ""
echo "To run locally:"
echo "  docker run -p 707:707 -p 8080:8080 -p 9933:9933 $TAG \\"
echo "    --data-dir /data --cpp-p2p-addr 0.0.0.0:707 --cpp-ws-addr 0.0.0.0:8080"
echo ""
echo "To push to registry:"
echo "  docker tag $TAG <registry>/coinject-cpp:latest"
echo "  docker push <registry>/coinject-cpp:latest"

