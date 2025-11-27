#!/bin/bash
# Deploy COINjecture Node to Google Cloud Run
# Usage: ./deploy-gcloud-run.sh [service-name] [region]
# Example: ./deploy-gcloud-run.sh coinject-node us-central1

set -e

# Configuration
PROJECT_ID="${GCP_PROJECT_ID:-coinjecture}"
SERVICE_NAME="${1:-coinject-node}"
REGION="${2:-us-central1}"
IMAGE_NAME="gcr.io/${PROJECT_ID}/${SERVICE_NAME}:latest"
HF_TOKEN="${HF_TOKEN:-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}"
HF_DATASET="${HF_DATASET:-COINjecture/NP_Solutions}"

echo "🚀 Deploying COINjecture Node to Google Cloud Run"
echo "📦 Project: $PROJECT_ID"
echo "🌍 Region: $REGION"
echo "📝 Service: $SERVICE_NAME"
echo ""

# Check if gcloud is installed
if ! command -v gcloud &> /dev/null; then
    echo "❌ gcloud CLI not found!"
    echo "💡 Install from: https://cloud.google.com/sdk/docs/install"
    exit 1
fi

# Check if Docker is installed
if ! command -v docker &> /dev/null; then
    echo "❌ Docker not found!"
    exit 1
fi

# Set the project
echo "🔧 Setting GCP project..."
gcloud config set project "$PROJECT_ID"

# Enable required APIs
echo "🔧 Enabling required APIs..."
gcloud services enable cloudbuild.googleapis.com \
    run.googleapis.com \
    containerregistry.googleapis.com \
    artifactregistry.googleapis.com

# Use existing Docker image or build if not found
if docker images coinject-node:latest --format "{{.Repository}}:{{.Tag}}" | grep -q "coinject-node:latest"; then
    echo "📦 Using existing Docker image (coinject-node:latest)..."
    docker tag coinject-node:latest "$IMAGE_NAME"
else
    echo "📦 Building Docker image (coinject-node:latest not found)..."
    docker build --platform linux/amd64 -t coinject-node:latest .
    docker tag coinject-node:latest "$IMAGE_NAME"
fi

echo "📤 Pushing image to Google Container Registry..."
docker push "$IMAGE_NAME"

# Bootnodes: Connect to existing droplet nodes
# Note: Using IP addresses only (without PeerID) so libp2p discovers PeerID automatically
# This makes bootnodes resilient to PeerID changes after node restarts
# Node 2 will be discovered via Kademlia DHT through Node 1
DROPLET1="143.110.139.166"
DROPLET2="68.183.205.12"
# Use IP addresses only - libp2p will discover PeerIDs on connection
BOOTNODE1="/ip4/$DROPLET1/tcp/30333"
BOOTNODE2="/ip4/$DROPLET2/tcp/30333"

# Construct container args. Cloud Run splits comma-separated values into separate args.
# Use a unique data dir per deploy so Cloud Run starts from a clean state
# This ensures a clean sync from genesis
DATA_DIR="/tmp/data-$(date +%s)"
echo "📁 Using Cloud Run data dir: $DATA_DIR (clean state - will sync from genesis)"

# Note: Mining disabled for Cloud Run (RPC-only node to save costs)
CONTAINER_ARGS="--data-dir=$DATA_DIR,\
--p2p-addr=/ip4/0.0.0.0/tcp/30333,\
--rpc-addr=0.0.0.0:9933,\
--metrics-addr=0.0.0.0:9090,\
--bootnodes=$BOOTNODE1,\
--bootnodes=$BOOTNODE2,\
--hf-token=$HF_TOKEN,\
--hf-dataset-name=$HF_DATASET"

# Deploy to Cloud Run
# Note: Cloud Run is HTTP-based, but we can enable P2P with proper configuration
# For P2P to work, we need min-instances=1 to keep connections alive
# P2P networking (30333) uses outbound TCP connections which Cloud Run supports
echo "🚀 Deploying to Cloud Run (P2P-enabled: min-instances=1, 1 CPU, 1Gi RAM)..."
gcloud run deploy "$SERVICE_NAME" \
    --image "$IMAGE_NAME" \
    --platform managed \
    --region "$REGION" \
    --allow-unauthenticated \
    --port 9933 \
    --memory 1Gi \
    --cpu 1 \
    --timeout 3600 \
    --max-instances 1 \
    --min-instances 1 \
    --set-env-vars "RUST_LOG=info,libp2p=debug" \
    --set-env-vars "HF_TOKEN=$HF_TOKEN" \
    --set-env-vars "HF_DATASET=$HF_DATASET" \
    --command coinject \
    --args="$CONTAINER_ARGS" \
    --execution-environment gen2 \
    --cpu-boost \
    --cpu-throttling

echo ""
echo "✅ Deployment complete!"
echo ""
echo "🌐 P2P Networking Configuration:"
echo "   • min-instances=1 (keeps instance running to maintain P2P connections)"
echo "   • Outbound TCP connections enabled (for P2P to droplets)"
echo "   • libp2p debug logging enabled (check logs for connection details)"
echo ""
echo "💰 Cost Notes:"
echo "   • min-instances=1 means ~\$0.10/hour base cost (instance always running)"
echo "   • 1 CPU, 1Gi memory (reduced from 2 CPU/2Gi)"
echo "   • max-instances=1 (prevents scaling costs)"
echo ""
echo "💡 To reduce costs: Set min-instances=0 (but P2P connections will drop when idle)"
echo "💡 For RPC-only node: Remove '--mine' from CONTAINER_ARGS"
echo ""
echo "📋 Service URL:"
gcloud run services describe "$SERVICE_NAME" --region "$REGION" --format "value(status.url)"
echo ""
echo "📊 View logs:"
echo "   gcloud run logs read $SERVICE_NAME --region $REGION"
echo ""
echo "🛑 Stop service:"
echo "   gcloud run services delete $SERVICE_NAME --region $REGION"

