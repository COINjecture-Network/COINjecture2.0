#!/bin/bash
# Build and Deploy COINjecture Docker Image
# Usage: ./build-and-deploy.sh [--build-only] [--no-push] [--skip-gcr]

set -e

# Configuration
DROPLET1="143.110.139.166"
DROPLET2="68.183.205.12"
SSH_KEY="${SSH_KEY:-$HOME/.ssh/coinjecture-key}"
CONTAINER_NAME="coinject-node"
DATA_VOLUME="coinject-data"
HF_TOKEN="${HF_TOKEN:-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}"
HF_DATASET="${HF_DATASET:-COINjecture/v5}"

# Get version info
GIT_COMMIT=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")
BUILD_DATE=$(date +%Y%m%d)
VERSION="${VERSION:-v4.7.47}"

# Image tags
LOCAL_IMAGE="coinjecture-netb:latest"
LOCAL_TAG_COMMIT="coinjecture-netb:${GIT_COMMIT}"
LOCAL_TAG_DATE="coinjecture-netb:${BUILD_DATE}"
GCR_PROJECT="${GCR_PROJECT:-coinjecture}"
GCR_IMAGE="gcr.io/${GCR_PROJECT}/coinject-node"
GCR_TAG_LATEST="${GCR_IMAGE}:latest"
GCR_TAG_VERSION="${GCR_IMAGE}:${VERSION}"
GCR_TAG_COMMIT="${GCR_IMAGE}:${GIT_COMMIT}"

# Flags
BUILD_ONLY=false
NO_PUSH=false
SKIP_GCR=false

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --build-only)
            BUILD_ONLY=true
            shift
            ;;
        --no-push)
            NO_PUSH=true
            shift
            ;;
        --skip-gcr)
            SKIP_GCR=true
            shift
            ;;
        *)
            echo "Unknown option: $1"
            echo "Usage: $0 [--build-only] [--no-push] [--skip-gcr]"
            exit 1
            ;;
    esac
done

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "🚀 COINjecture Docker Build & Deploy"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "📦 Version: $VERSION"
echo "🔖 Commit: $GIT_COMMIT"
echo "📅 Date: $BUILD_DATE"
echo ""

# ============================================================================
# Step 1: Build Docker Image
# ============================================================================
echo "🔨 Step 1: Building Docker image..."
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

docker build \
    --platform linux/amd64 \
    -t "$LOCAL_IMAGE" \
    -t "$LOCAL_TAG_COMMIT" \
    -t "$LOCAL_TAG_DATE" \
    -f Dockerfile \
    .

if [ $? -ne 0 ]; then
    echo "❌ Docker build failed!"
    exit 1
fi

echo "✅ Docker image built successfully!"
echo "   Tags: $LOCAL_IMAGE, $LOCAL_TAG_COMMIT, $LOCAL_TAG_DATE"
echo ""

if [ "$BUILD_ONLY" = true ]; then
    echo "✅ Build complete (--build-only flag set)"
    exit 0
fi

# ============================================================================
# Step 2: Tag for GCR
# ============================================================================
if [ "$SKIP_GCR" = false ]; then
    echo "🏷️  Step 2: Tagging image for GCR..."
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    
    docker tag "$LOCAL_IMAGE" "$GCR_TAG_LATEST"
    docker tag "$LOCAL_IMAGE" "$GCR_TAG_VERSION"
    docker tag "$LOCAL_IMAGE" "$GCR_TAG_COMMIT"
    
    echo "✅ Images tagged for GCR:"
    echo "   - $GCR_TAG_LATEST"
    echo "   - $GCR_TAG_VERSION"
    echo "   - $GCR_TAG_COMMIT"
    echo ""
    
    # ============================================================================
    # Step 3: Push to GCR
    # ============================================================================
    if [ "$NO_PUSH" = false ]; then
        echo "📤 Step 3: Pushing to Google Container Registry..."
        echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        
        # Check if gcloud is authenticated
        if ! gcloud auth list --filter=status:ACTIVE --format="value(account)" | grep -q .; then
            echo "⚠️  Not authenticated with gcloud!"
            echo "💡 Run: gcloud auth login && gcloud auth configure-docker"
            echo ""
            echo "Skipping GCR push. You can push manually later with:"
            echo "  docker push $GCR_TAG_LATEST"
            echo "  docker push $GCR_TAG_VERSION"
            echo "  docker push $GCR_TAG_COMMIT"
        else
            echo "🔐 Authenticated with gcloud"
            echo "📤 Pushing images to GCR..."
            
            docker push "$GCR_TAG_LATEST" || echo "⚠️  Failed to push latest"
            docker push "$GCR_TAG_VERSION" || echo "⚠️  Failed to push version"
            docker push "$GCR_TAG_COMMIT" || echo "⚠️  Failed to push commit"
            
            echo "✅ Images pushed to GCR!"
            echo ""
        fi
    else
        echo "⏭️  Skipping GCR push (--no-push flag set)"
        echo ""
    fi
else
    echo "⏭️  Skipping GCR steps (--skip-gcr flag set)"
    echo ""
fi

# ============================================================================
# Step 4: Deploy to Droplets
# ============================================================================
echo "🚀 Step 4: Deploying to Droplets..."
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Function to deploy to a node
deploy_to_node() {
    local NODE_IP=$1
    local NODE_NAME=$2
    local BOOTNODES=${3:-""}
    local IMAGE_TO_USE=${4:-$LOCAL_IMAGE}
    
    echo "📡 Deploying to $NODE_NAME ($NODE_IP)..."
    if [ -n "$BOOTNODES" ]; then
        echo "   Bootnodes: $BOOTNODES"
    fi
    echo ""
    
    # Save Docker image to tar file
    echo "📦 Saving Docker image to tar..."
    docker save "$IMAGE_TO_USE" -o /tmp/coinject-node.tar
    
    # Transfer image to remote node
    echo "📤 Transferring image to $NODE_IP..."
    if ! scp -i "$SSH_KEY" -o StrictHostKeyChecking=no /tmp/coinject-node.tar root@$NODE_IP:/tmp/; then
        echo "❌ Failed to transfer image to $NODE_IP"
        rm -f /tmp/coinject-node.tar
        return 1
    fi
    
    # Clean up local tar
    rm -f /tmp/coinject-node.tar
    
    # Deploy and restart on remote node
    ssh -i "$SSH_KEY" -o StrictHostKeyChecking=no root@$NODE_IP "CONTAINER_NAME='$CONTAINER_NAME' IMAGE_NAME='$IMAGE_TO_USE' DATA_VOLUME='$DATA_VOLUME' HF_TOKEN='$HF_TOKEN' HF_DATASET='$HF_DATASET' BOOTNODES_ARG='$BOOTNODES' bash -s" << 'ENDSSH'
set -e

echo "📥 Loading Docker image..."
docker load -i /tmp/coinject-node.tar
rm -f /tmp/coinject-node.tar

echo "🛑 Stopping existing container (if running)..."
docker stop $CONTAINER_NAME 2>/dev/null || true
docker rm $CONTAINER_NAME 2>/dev/null || true

echo "🛑 Stopping any existing coinject processes using ports 30333, 9933, or 9090..."
for port in 30333 9933 9090; do
    lsof -ti:$port | xargs kill -9 2>/dev/null || true
    fuser -k $port/tcp 2>/dev/null || true
done

pkill -9 coinject 2>/dev/null || true
sleep 2

echo "🚀 Starting new container..."
if [ -n "$BOOTNODES_ARG" ]; then
    docker run -d \
      --name $CONTAINER_NAME \
      --restart unless-stopped \
      -p 30333:30333 \
      -p 9933:9933 \
      -p 9090:9090 \
      -v $DATA_VOLUME:/data \
      $IMAGE_NAME \
      --data-dir /data \
      --p2p-addr /ip4/0.0.0.0/tcp/30333 \
      --rpc-addr 0.0.0.0:9933 \
      --metrics-addr 0.0.0.0:9090 \
      --mine \
      --hf-token "$HF_TOKEN" \
      --hf-dataset-name "$HF_DATASET" \
      --bootnodes "$BOOTNODES_ARG"
else
    docker run -d \
      --name $CONTAINER_NAME \
      --restart unless-stopped \
      -p 30333:30333 \
      -p 9933:9933 \
      -p 9090:9090 \
      -v $DATA_VOLUME:/data \
      $IMAGE_NAME \
      --data-dir /data \
      --p2p-addr /ip4/0.0.0.0/tcp/30333 \
      --rpc-addr 0.0.0.0:9933 \
      --metrics-addr 0.0.0.0:9090 \
      --mine \
      --hf-token "$HF_TOKEN" \
      --hf-dataset-name "$HF_DATASET"
fi

echo "⏳ Waiting for container to start..."
sleep 3

if docker ps | grep -q $CONTAINER_NAME; then
    echo "✅ Container started successfully!"
    docker ps | grep $CONTAINER_NAME
    echo ""
    echo "📜 Recent logs:"
    docker logs --tail 20 $CONTAINER_NAME
else
    echo "❌ Container failed to start!"
    docker logs $CONTAINER_NAME
    exit 1
fi
ENDSSH

    if [ $? -eq 0 ]; then
        echo "✅ $NODE_NAME deployment complete!"
    else
        echo "❌ $NODE_NAME deployment failed!"
        return 1
    fi
    echo ""
}

# Deploy to both droplets
# Current peer IDs (verified from running nodes)
DROPLET1_PEER_ID="${DROPLET1_PEER_ID:-12D3KooWL3Q7KmTocqNGLfyz4X4mhyyPD8b4zx6MBk1qnDAT8FYs}"
DROPLET2_PEER_ID="${DROPLET2_PEER_ID:-12D3KooWQwpXp7NJG9gMVJMFH7oBfYQizbtPAB3RfRqxyvQ5WZfv}"

BOOTNODE2="/ip4/$DROPLET2/tcp/30333/p2p/$DROPLET2_PEER_ID"
deploy_to_node "$DROPLET1" "Droplet 1" "$BOOTNODE2" "$LOCAL_IMAGE"

BOOTNODE1="/ip4/$DROPLET1/tcp/30333/p2p/$DROPLET1_PEER_ID"
deploy_to_node "$DROPLET2" "Droplet 2" "$BOOTNODE1" "$LOCAL_IMAGE"

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "✅ Deployment Complete!"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "📋 Container Management Commands:"
echo ""
echo "  # View logs"
echo "  ssh -i $SSH_KEY root@$DROPLET1 'docker logs -f $CONTAINER_NAME'"
echo "  ssh -i $SSH_KEY root@$DROPLET2 'docker logs -f $CONTAINER_NAME'"
echo ""
echo "  # Restart containers"
echo "  ssh -i $SSH_KEY root@$DROPLET1 'docker restart $CONTAINER_NAME'"
echo "  ssh -i $SSH_KEY root@$DROPLET2 'docker restart $CONTAINER_NAME'"
echo ""
echo "  # View container status"
echo "  ssh -i $SSH_KEY root@$DROPLET1 'docker ps | grep $CONTAINER_NAME'"
echo "  ssh -i $SSH_KEY root@$DROPLET2 'docker ps | grep $CONTAINER_NAME'"
echo ""
echo "📦 GCR Images (if pushed):"
if [ "$SKIP_GCR" = false ] && [ "$NO_PUSH" = false ]; then
    echo "  - $GCR_TAG_LATEST"
    echo "  - $GCR_TAG_VERSION"
    echo "  - $GCR_TAG_COMMIT"
    echo ""
    echo "💡 To deploy to GCE VM, use:"
    echo "  gcloud compute ssh <vm-name> --zone=<zone> --command='docker pull $GCR_TAG_LATEST'"
fi
echo ""

