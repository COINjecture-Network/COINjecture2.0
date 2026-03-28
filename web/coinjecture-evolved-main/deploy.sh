#!/bin/bash
set -e

# Configuration (production: coinjecture.com + www — CloudFront E2INLKPSADEUYX)
S3_BUCKET="${S3_BUCKET:-coinjecture.com}"
CLOUDFRONT_DIST_ID="${CLOUDFRONT_DIST_ID:-E2INLKPSADEUYX}"
REGION="${AWS_REGION:-us-east-1}"

echo "🚀 Starting deployment..."

# Check if .env.production exists
if [ ! -f .env.production ]; then
  echo "⚠️  Warning: .env.production not found. Using default environment variables."
fi

# Build the frontend
echo "📦 Building frontend..."
npm run build

if [ ! -d "dist" ]; then
  echo "❌ Build failed: dist/ directory not found"
  exit 1
fi

# Upload to S3
echo "☁️  Uploading to S3 bucket: $S3_BUCKET..."
aws s3 sync dist/ "s3://$S3_BUCKET" \
  --region "$REGION" \
  --delete \
  --exclude "*.map" \
  --cache-control "public, max-age=31536000, immutable" \
  --exclude "index.html"

# Upload index.html with no-cache
aws s3 cp dist/index.html "s3://$S3_BUCKET/index.html" \
  --region "$REGION" \
  --content-type "text/html" \
  --cache-control "no-cache, no-store, must-revalidate"

# Set proper content types for assets
echo "📝 Setting content types..."
aws s3 sync dist/assets "s3://$S3_BUCKET/assets" \
  --region "$REGION" \
  --exclude "*" \
  --include "*.js" \
  --content-type "application/javascript" \
  --cache-control "public, max-age=31536000, immutable"

aws s3 sync dist/assets "s3://$S3_BUCKET/assets" \
  --region "$REGION" \
  --exclude "*" \
  --include "*.css" \
  --content-type "text/css" \
  --cache-control "public, max-age=31536000, immutable"

# Invalidate CloudFront cache if distribution ID is provided
if [ ! -z "$CLOUDFRONT_DIST_ID" ]; then
  echo "🔄 Invalidating CloudFront cache..."
  INVALIDATION_ID=$(aws cloudfront create-invalidation \
    --distribution-id "$CLOUDFRONT_DIST_ID" \
    --paths "/*" \
    --query 'Invalidation.Id' \
    --output text)
  
  echo "✅ Cache invalidation created: $INVALIDATION_ID"
  echo "⏳ This may take a few minutes to complete..."
else
  echo "⚠️  CLOUDFRONT_DIST_ID not set. Skipping cache invalidation."
  echo "   Set CLOUDFRONT_DIST_ID environment variable to enable cache invalidation."
fi

echo "✅ Deployment complete!"
echo ""
echo "📊 Summary:"
echo "   S3 Bucket: $S3_BUCKET"
if [ ! -z "$CLOUDFRONT_DIST_ID" ]; then
  echo "   CloudFront Distribution: $CLOUDFRONT_DIST_ID"
fi
echo ""
echo "🌐 Your frontend should be live shortly!"

