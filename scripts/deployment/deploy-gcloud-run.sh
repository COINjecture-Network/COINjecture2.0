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

# Build and push Docker image
echo "📦 Building Docker image..."
docker build --platform linux/amd64 -t "$IMAGE_NAME" .

echo "📤 Pushing image to Google Container Registry..."
docker push "$IMAGE_NAME"

# Construct container args. Cloud Run expects individual args instead of comma string.
CONTAINER_ARGS="--data-dir=/tmp/data,\
--p2p-addr=/ip4/0.0.0.0/tcp/30333,\
--rpc-addr=0.0.0.0:9933,\
--metrics-addr=0.0.0.0:9090,\
--mine,\
--hf-token=$HF_TOKEN,\
--hf-dataset-name=$HF_DATASET"

# Deploy to Cloud Run
# Note: Cloud Run is HTTP-based, so we'll expose the RPC port (9933) as the main port
# P2P networking (30333) may have limitations in Cloud Run
echo "🚀 Deploying to Cloud Run..."
gcloud run deploy "$SERVICE_NAME" \
    --image "$IMAGE_NAME" \
    --platform managed \
    --region "$REGION" \
    --allow-unauthenticated \
    --port 9933 \
    --memory 2Gi \
    --cpu 2 \
    --timeout 3600 \
    --max-instances 1 \
    --min-instances 1 \
    --set-env-vars "RUST_LOG=info" \
    --set-env-vars "HF_TOKEN=$HF_TOKEN" \
    --set-env-vars "HF_DATASET=$HF_DATASET" \
    --command coinject \
    --args="$CONTAINER_ARGS" \
    --service-account default \
    --execution-environment gen2

echo ""
echo "✅ Deployment complete!"
echo ""
echo "📋 Service URL:"
gcloud run services describe "$SERVICE_NAME" --region "$REGION" --format "value(status.url)"
echo ""
echo "📊 View logs:"
echo "   gcloud run logs read $SERVICE_NAME --region $REGION"
echo ""
echo "🛑 Stop service:"
echo "   gcloud run services delete $SERVICE_NAME --region $REGION"

