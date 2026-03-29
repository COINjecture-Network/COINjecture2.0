#!/usr/bin/env bash
# Build the COINjecture P2P node image (root Dockerfile → coinject binary).
#
# Usage:
#   ./scripts/build-docker.sh                    # tag coinject-node:latest, native platform
#   ./scripts/build-docker.sh myregistry/coinject-node:v1
#   HOSTINGER=1 ./scripts/build-docker.sh        # force linux/amd64 (VPS)
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

TAG="${1:-coinject-node:latest}"

echo "=========================================="
echo "COINjecture node Docker image"
echo "=========================================="
echo "Tag:     $TAG"
echo "Context: $PROJECT_ROOT"
echo ""

if [[ "${HOSTINGER:-}" == "1" ]] || [[ "${TARGETPLATFORM:-}" == "linux/amd64" ]]; then
  echo "Building for linux/amd64 (Hostinger / typical VPS)…"
  docker buildx build \
    --platform linux/amd64 \
    -f Dockerfile \
    -t "$TAG" \
    --load \
    .
else
  echo "Building for native platform (set HOSTINGER=1 on Apple Silicon before push to amd64 VPS)…"
  docker build -f Dockerfile -t "$TAG" .
fi

echo ""
echo "Image ready: $TAG"
echo ""
echo "Smoke-test (matches docker-compose bootnode command shape):"
echo "  docker run --rm -p 707:707 -p 9933:9933 -p 9090:9090 $TAG \\"
echo "    --mine --data-dir /data --cpp-p2p-addr 0.0.0.0:707 \\"
echo "    --metrics-addr 0.0.0.0:9090 --rpc-addr 0.0.0.0:9933"
echo ""
echo "Export for scp to Hostinger (no registry):"
echo "  docker save $TAG | gzip > coinject-node-image.tar.gz"
echo "  # on server: gunzip -c coinject-node-image.tar.gz | docker load"
echo ""
echo "Push to Docker Hub / GHCR:"
echo "  docker tag $TAG <registry>/<user>/coinject-node:latest"
echo "  docker push <registry>/<user>/coinject-node:latest"
