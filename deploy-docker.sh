#!/bin/bash
# Deploy Docker image to nodes and restart services
# Usage: ./deploy-docker.sh

set -e

# Configuration
DROPLET1="143.110.139.166"
DROPLET2="68.183.205.12"
# Peer IDs (discovered from network_getInfo RPC)
DROPLET1_PEER_ID="12D3KooWJp1LjiE8sLN4kN9JpqZJWGvrMZR2eH4x2Trs3AdYdPwx"
DROPLET2_PEER_ID="12D3KooWLnKgJxYo1pxMCXxfJsGt1o9j5iExBepUmaMV4NtvWAWQ"
SSH_KEY="${SSH_KEY:-$HOME/.ssh/COINjecture-Key}"
IMAGE_NAME="gcr.io/coinjecture/coinject-node:v4.7.41"
CONTAINER_NAME="coinject-node"
DATA_VOLUME="coinject-data"
HF_TOKEN="${HF_TOKEN:-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}"
HF_DATASET="${HF_DATASET:-COINjecture/NP_Solutions_v3}"

echo "🚀 Deploying Docker image to nodes..."
echo "📦 Image: $IMAGE_NAME"
echo ""

# Check if image exists locally, pull if needed
if ! docker image inspect "$IMAGE_NAME" &>/dev/null; then
    echo "📥 Pulling image from GCR (AMD64 platform)..."
    if ! docker pull --platform linux/amd64 "$IMAGE_NAME"; then
        echo "❌ Failed to pull image from GCR!"
        echo "💡 Make sure you're authenticated: gcloud auth configure-docker"
        exit 1
    fi
    echo "✅ Image pulled: $IMAGE_NAME"
else
    echo "✅ Image found locally: $IMAGE_NAME"
fi
echo ""

# Function to deploy to a node
deploy_to_node() {
    local NODE_IP=$1
    local NODE_NAME=$2
    local BOOTNODES=${3:-""}  # Optional bootnode addresses
    
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "📡 Deploying to $NODE_NAME ($NODE_IP)..."
    if [ -n "$BOOTNODES" ]; then
        echo "   Bootnodes: $BOOTNODES"
    fi
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    
    # Save Docker image to tar file
    echo "📦 Saving Docker image to tar..."
    docker save "$IMAGE_NAME" -o /tmp/coinject-node.tar
    
    # Transfer image to remote node
    echo "📤 Transferring image to $NODE_IP..."
    scp -i "$SSH_KEY" /tmp/coinject-node.tar root@$NODE_IP:/tmp/
    
    # Clean up local tar
    rm -f /tmp/coinject-node.tar
    
    # Deploy and restart on remote node
    # Pass variables as environment variables to SSH session
    ssh -i "$SSH_KEY" root@$NODE_IP "CONTAINER_NAME='$CONTAINER_NAME' IMAGE_NAME='$IMAGE_NAME' DATA_VOLUME='$DATA_VOLUME' HF_TOKEN='$HF_TOKEN' HF_DATASET='$HF_DATASET' BOOTNODES_ARG='$BOOTNODES' bash -s" << 'ENDSSH'
set -e

# Check and install Docker if needed
if ! command -v docker &> /dev/null; then
    echo "🐳 Docker not found. Installing Docker..."
    
    # Remove any existing Docker repository configuration
    rm -f /etc/apt/sources.list.d/docker.list
    rm -f /etc/apt/keyrings/docker.gpg
    
    apt-get update
    apt-get install -y ca-certificates curl gnupg lsb-release
    
    # Force Ubuntu - we know these are Ubuntu 24.04 systems
    OS=ubuntu
    VERSION=noble
    
    echo "Installing Docker for \$OS \$VERSION"
    
    install -m 0755 -d /etc/apt/keyrings
    curl -fsSL https://download.docker.com/linux/\$OS/gpg | gpg --dearmor -o /etc/apt/keyrings/docker.gpg
    chmod a+r /etc/apt/keyrings/docker.gpg
    echo "deb [arch=\$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/\$OS \$VERSION stable" | tee /etc/apt/sources.list.d/docker.list > /dev/null
    apt-get update
    apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
    echo "✅ Docker installed!"
fi

echo "📥 Loading Docker image..."
docker load -i /tmp/coinject-node.tar
rm -f /tmp/coinject-node.tar

echo "🛑 Stopping existing container (if running)..."
docker stop $CONTAINER_NAME 2>/dev/null || true
docker rm $CONTAINER_NAME 2>/dev/null || true

echo "🛑 Stopping any existing coinject processes using ports 30333, 9933, or 9090..."
# Kill processes using the ports
for port in 30333 9933 9090; do
    lsof -ti:$port | xargs kill -9 2>/dev/null || true
    fuser -k $port/tcp 2>/dev/null || true
done

# Also kill any coinject processes
pkill -9 coinject 2>/dev/null || true

# Wait a moment for ports to be released
sleep 2

echo "🚀 Starting new container with HuggingFace sync..."
echo "   Dataset: $HF_DATASET"

# Build docker run command
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
    echo "📋 Container status:"
    docker ps | grep $CONTAINER_NAME
    echo ""
    echo "📜 Recent logs:"
    docker logs --tail 20 $CONTAINER_NAME
else
    echo "❌ Container failed to start!"
    echo "📜 Error logs:"
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

# Deploy to both nodes with mutual bootnodes (including Peer IDs for reliability)
# Node 1: Connect to Node 2 as bootnode
BOOTNODE2="/ip4/$DROPLET2/tcp/30333/p2p/$DROPLET2_PEER_ID"
deploy_to_node "$DROPLET1" "Node 1" "$BOOTNODE2"

# Node 2: Use Node 1 as bootnode
BOOTNODE1="/ip4/$DROPLET1/tcp/30333/p2p/$DROPLET1_PEER_ID"
deploy_to_node "$DROPLET2" "Node 2" "$BOOTNODE1"

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "✅ Deployment complete!"
echo ""
echo "📋 Container management commands:"
echo ""
echo "  # View logs"
echo "  ssh root@$DROPLET1 'docker logs -f $CONTAINER_NAME'"
echo "  ssh root@$DROPLET2 'docker logs -f $CONTAINER_NAME'"
echo ""
echo "  # Restart container"
echo "  ssh root@$DROPLET1 'docker restart $CONTAINER_NAME'"
echo "  ssh root@$DROPLET2 'docker restart $CONTAINER_NAME'"
echo ""
echo "  # Stop container"
echo "  ssh root@$DROPLET1 'docker stop $CONTAINER_NAME'"
echo "  ssh root@$DROPLET2 'docker stop $CONTAINER_NAME'"
echo ""
echo "  # View container status"
echo "  ssh root@$DROPLET1 'docker ps | grep $CONTAINER_NAME'"
echo "  ssh root@$DROPLET2 'docker ps | grep $CONTAINER_NAME'"
echo ""

