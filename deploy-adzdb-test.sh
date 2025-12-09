#!/bin/bash
# Deploy ADZDB test nodes to DigitalOcean droplets with HuggingFace v4 streaming
# Usage: ./deploy-adzdb-test.sh

set -e

# Configuration
DROPLET1="143.110.139.166"
DROPLET2="68.183.205.12"
# Peer IDs will be discovered dynamically since ADZDB nodes are fresh
SSH_KEY="${SSH_KEY:-$HOME/.ssh/COINjecture-Key}"
IMAGE_TAG="v4.7.46-adzdb"
IMAGE_NAME="coinject-node:$IMAGE_TAG"
CONTAINER_NAME="coinject-adzdb"
DATA_VOLUME="coinject-adzdb-data"
HF_TOKEN="${HF_TOKEN:-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}"
HF_DATASET="COINjecture/NP_Solutions_v4"  # NEW v4 dataset for ADZDB test

echo "════════════════════════════════════════════════════════════════════════"
echo "  🗄️  ADZDB Network Test - 2 Node Deployment"
echo "  📊 Streaming to: $HF_DATASET"
echo "════════════════════════════════════════════════════════════════════════"
echo ""

# Step 1: Build Docker image with ADZDB feature
echo "📦 Building Docker image with ADZDB feature..."
docker build -f Dockerfile.adzdb -t $IMAGE_NAME --platform linux/amd64 .

if [ $? -ne 0 ]; then
    echo "❌ Docker build failed!"
    exit 1
fi
echo "✅ Docker image built: $IMAGE_NAME"
echo ""

# Save Docker image to tar file
echo "📦 Saving Docker image to tar..."
docker save "$IMAGE_NAME" -o /tmp/coinject-adzdb.tar
echo "✅ Image saved: /tmp/coinject-adzdb.tar ($(du -h /tmp/coinject-adzdb.tar | cut -f1))"
echo ""

# Function to deploy to a node
deploy_adzdb_node() {
    local NODE_IP=$1
    local NODE_NAME=$2
    local NODE_PORT=$3
    local BOOTNODES=${4:-""}
    
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "📡 Deploying ADZDB node to $NODE_NAME ($NODE_IP)..."
    echo "   P2P Port: $NODE_PORT"
    if [ -n "$BOOTNODES" ]; then
        echo "   Bootnodes: $BOOTNODES"
    fi
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    
    # Transfer image to remote node
    echo "📤 Transferring image to $NODE_IP..."
    scp -i "$SSH_KEY" /tmp/coinject-adzdb.tar root@$NODE_IP:/tmp/
    
    # Deploy and restart on remote node
    ssh -i "$SSH_KEY" root@$NODE_IP "CONTAINER_NAME='$CONTAINER_NAME' IMAGE_NAME='$IMAGE_NAME' DATA_VOLUME='$DATA_VOLUME' HF_TOKEN='$HF_TOKEN' HF_DATASET='$HF_DATASET' BOOTNODES_ARG='$BOOTNODES' NODE_PORT='$NODE_PORT' bash -s" << 'ENDSSH'
set -e

echo "📥 Loading Docker image..."
docker load -i /tmp/coinject-adzdb.tar
rm -f /tmp/coinject-adzdb.tar

echo "🛑 Stopping existing ADZDB container (if running)..."
docker stop $CONTAINER_NAME 2>/dev/null || true
docker rm $CONTAINER_NAME 2>/dev/null || true

# Stop any process using the port
for port in $NODE_PORT 9934 9091; do
    lsof -ti:$port | xargs kill -9 2>/dev/null || true
    fuser -k $port/tcp 2>/dev/null || true
done
sleep 2

# Clean previous ADZDB data for fresh test
echo "🧹 Cleaning previous ADZDB data volume..."
docker volume rm $DATA_VOLUME 2>/dev/null || true
docker volume create $DATA_VOLUME

echo "🚀 Starting ADZDB node with HuggingFace v4 sync..."
echo "   Dataset: $HF_DATASET"
echo "   Using --use-adzdb flag for file-based storage"

# Build docker run command
if [ -n "$BOOTNODES_ARG" ]; then
    docker run -d \
      --name $CONTAINER_NAME \
      --restart unless-stopped \
      -p $NODE_PORT:$NODE_PORT \
      -p 9934:9933 \
      -p 9091:9090 \
      -v $DATA_VOLUME:/data \
      $IMAGE_NAME \
      --data-dir /data \
      --p2p-addr /ip4/0.0.0.0/tcp/$NODE_PORT \
      --rpc-addr 0.0.0.0:9933 \
      --metrics-addr 0.0.0.0:9090 \
      --mine \
      --use-adzdb \
      --hf-token "$HF_TOKEN" \
      --hf-dataset-name "$HF_DATASET" \
      --bootnodes "$BOOTNODES_ARG" \
      --verbose
else
    docker run -d \
      --name $CONTAINER_NAME \
      --restart unless-stopped \
      -p $NODE_PORT:$NODE_PORT \
      -p 9934:9933 \
      -p 9091:9090 \
      -v $DATA_VOLUME:/data \
      $IMAGE_NAME \
      --data-dir /data \
      --p2p-addr /ip4/0.0.0.0/tcp/$NODE_PORT \
      --rpc-addr 0.0.0.0:9933 \
      --metrics-addr 0.0.0.0:9090 \
      --mine \
      --use-adzdb \
      --hf-token "$HF_TOKEN" \
      --hf-dataset-name "$HF_DATASET" \
      --verbose
fi

echo "⏳ Waiting for container to start..."
sleep 5

if docker ps | grep -q $CONTAINER_NAME; then
    echo "✅ Container started successfully!"
    echo ""
    echo "📋 Container status:"
    docker ps | grep $CONTAINER_NAME
    echo ""
    echo "📜 Recent logs (checking for ADZDB initialization):"
    docker logs --tail 30 $CONTAINER_NAME | grep -E "(ADZDB|adzdb|HuggingFace|PeerId|Network node)" || docker logs --tail 30 $CONTAINER_NAME
else
    echo "❌ Container failed to start!"
    echo "📜 Error logs:"
    docker logs $CONTAINER_NAME
    exit 1
fi
ENDSSH

    if [ $? -eq 0 ]; then
        echo "✅ $NODE_NAME deployment complete!"
        
        # Get the PeerId
        echo "🔍 Retrieving PeerId..."
        PEER_ID=$(ssh -i "$SSH_KEY" root@$NODE_IP "docker logs $CONTAINER_NAME 2>&1 | grep -oP 'PeerId: \K12D3KooW[A-Za-z0-9]+' | head -1" || echo "")
        if [ -n "$PEER_ID" ]; then
            echo "   PeerId: $PEER_ID"
            echo "   Bootnode address: /ip4/$NODE_IP/tcp/$NODE_PORT/p2p/$PEER_ID"
        fi
    else
        echo "❌ $NODE_NAME deployment failed!"
        return 1
    fi
    echo ""
}

# Deploy Node 1 first (bootstrap)
deploy_adzdb_node "$DROPLET1" "ADZDB Node 1 (Bootstrap)" "30334"

# Wait for Node 1 to be fully initialized
echo "⏳ Waiting for Node 1 to initialize (10 seconds)..."
sleep 10

# Get Node 1's PeerId
echo "🔍 Getting Node 1 PeerId for bootnode address..."
NODE1_PEER_ID=$(ssh -i "$SSH_KEY" root@$DROPLET1 "docker logs $CONTAINER_NAME 2>&1 | grep -oP 'PeerId: \K12D3KooW[A-Za-z0-9]+' | head -1")

if [ -z "$NODE1_PEER_ID" ]; then
    echo "⚠️  Could not retrieve Node 1 PeerId. Node 2 will use mDNS discovery."
    BOOTNODE1=""
else
    echo "✅ Node 1 PeerId: $NODE1_PEER_ID"
    BOOTNODE1="/ip4/$DROPLET1/tcp/30334/p2p/$NODE1_PEER_ID"
fi

# Deploy Node 2 with Node 1 as bootnode
deploy_adzdb_node "$DROPLET2" "ADZDB Node 2" "30334" "$BOOTNODE1"

# Clean up local tar
rm -f /tmp/coinject-adzdb.tar

echo ""
echo "════════════════════════════════════════════════════════════════════════"
echo "  ✅ ADZDB 2-Node Test Deployment Complete!"
echo "════════════════════════════════════════════════════════════════════════"
echo ""
echo "📊 HuggingFace Dataset: https://huggingface.co/datasets/$HF_DATASET"
echo ""
echo "📋 Monitor commands:"
echo ""
echo "  # View logs for Node 1"
echo "  ssh -i $SSH_KEY root@$DROPLET1 'docker logs -f $CONTAINER_NAME'"
echo ""
echo "  # View logs for Node 2"
echo "  ssh -i $SSH_KEY root@$DROPLET2 'docker logs -f $CONTAINER_NAME'"
echo ""
echo "  # Check ADZDB files on Node 1"
echo "  ssh -i $SSH_KEY root@$DROPLET1 'docker exec $CONTAINER_NAME ls -la /data/adzdb/'"
echo ""
echo "  # RPC endpoints"
echo "  curl -X POST http://$DROPLET1:9934 -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"method\":\"chain_getInfo\",\"params\":[],\"id\":1}'"
echo "  curl -X POST http://$DROPLET2:9934 -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"method\":\"chain_getInfo\",\"params\":[],\"id\":1}'"
echo ""
echo "  # Stop test nodes"
echo "  ssh -i $SSH_KEY root@$DROPLET1 'docker stop $CONTAINER_NAME'"
echo "  ssh -i $SSH_KEY root@$DROPLET2 'docker stop $CONTAINER_NAME'"
echo ""






