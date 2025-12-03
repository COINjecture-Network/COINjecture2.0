#!/bin/bash
# Configure CloudFront to allow POST requests to /api/rpc without Lambda@Edge
# This script updates the cache behavior to allow POST and adds CORS headers via Response Headers Policy

set -e

DISTRIBUTION_ID="${CLOUDFRONT_DIST_ID:-E1F9JMDDFH6L9V}"
# Default to first node - client will handle failover to other nodes
RPC_NODE_IP="${RPC_NODE_IP:-143.110.139.166}"
RPC_NODE_PORT="${RPC_NODE_PORT:-9933}"

echo "Configuring CloudFront distribution $DISTRIBUTION_ID for RPC proxy..."
echo "Note: CloudFront will proxy /api/rpc to $RPC_NODE_IP:$RPC_NODE_PORT (default node)"
echo "      The RPC client handles failover to other nodes automatically."

# Step 1: Get current distribution config
echo "Fetching current distribution config..."
aws cloudfront get-distribution-config --id "$DISTRIBUTION_ID" --output json > /tmp/current-config.json

# Step 2: Create or update Response Headers Policy for CORS
echo "Creating Response Headers Policy for CORS..."
POLICY_NAME="coinjecture-rpc-cors-policy"

# Check if policy already exists using jq
EXISTING_POLICY=$(aws cloudfront list-response-headers-policies --output json 2>/dev/null | \
  jq -r ".ResponseHeadersPolicyList.Items[] | select(.Name == \"$POLICY_NAME\") | .Id" | head -1)

if [ -z "$EXISTING_POLICY" ] || [ "$EXISTING_POLICY" == "null" ]; then
  echo "Creating new Response Headers Policy..."
  POLICY_ID=$(aws cloudfront create-response-headers-policy \
    --response-headers-policy-config '{
      "Name": "'$POLICY_NAME'",
      "Comment": "CORS headers for RPC API",
      "CorsConfig": {
        "AccessControlAllowOrigins": {
          "Quantity": 1,
          "Items": ["*"]
        },
        "AccessControlAllowHeaders": {
          "Quantity": 1,
          "Items": ["*"]
        },
        "AccessControlAllowMethods": {
          "Quantity": 3,
          "Items": ["GET", "POST", "OPTIONS"]
        },
        "AccessControlAllowCredentials": false,
        "AccessControlExposeHeaders": {
          "Quantity": 0,
          "Items": []
        },
        "AccessControlMaxAgeSec": 86400,
        "OriginOverride": false
      }
    }' \
    --query 'ResponseHeadersPolicy.Id' \
    --output text 2>&1)
  
  # Check if creation failed due to existing policy
  if echo "$POLICY_ID" | grep -q "ResponseHeadersPolicyAlreadyExists"; then
    echo "Policy already exists, fetching ID..."
    POLICY_ID=$(aws cloudfront list-response-headers-policies --output json 2>/dev/null | \
      jq -r ".ResponseHeadersPolicyList.Items[] | select(.Name == \"$POLICY_NAME\") | .Id" | head -1)
    echo "Using existing Response Headers Policy: $POLICY_ID"
  else
    echo "Created Response Headers Policy: $POLICY_ID"
  fi
else
  POLICY_ID="$EXISTING_POLICY"
  echo "Using existing Response Headers Policy: $POLICY_ID"
fi

# Step 3: Update distribution config to add/update /api/rpc cache behavior
echo "Updating cache behavior for /api/rpc..."

# Use jq to update the distribution config
jq --arg policy_id "$POLICY_ID" --arg node_ip "$RPC_NODE_IP" --arg node_port "$RPC_NODE_PORT" '
  .DistributionConfig.Origins.Items = (.DistributionConfig.Origins.Items // []) |
  .DistributionConfig.Origins.Items |= (
    if any(.Id == "RPC-Origin") then .
    else . + [{
      "Id": "RPC-Origin",
      "DomainName": $node_ip,
      "CustomOriginConfig": {
        "HTTPPort": ($node_port | tonumber),
        "HTTPSPort": 443,
        "OriginProtocolPolicy": "http-only",
        "OriginSslProtocols": {
          "Quantity": 0,
          "Items": []
        },
        "OriginReadTimeout": 30,
        "OriginKeepaliveTimeout": 5
      }
    }]
    end
  ) |
  .DistributionConfig.Origins.Quantity = (.DistributionConfig.Origins.Items | length) |
  .DistributionConfig.CacheBehaviors.Items = (.DistributionConfig.CacheBehaviors.Items // []) |
  .DistributionConfig.CacheBehaviors.Items |= (
    map(select(.PathPattern != "/api/rpc")) + [{
      "PathPattern": "/api/rpc",
      "TargetOriginId": "RPC-Origin",
      "ViewerProtocolPolicy": "redirect-to-https",
      "AllowedMethods": {
        "Quantity": 7,
        "Items": ["GET", "HEAD", "OPTIONS", "PUT", "POST", "PATCH", "DELETE"],
        "CachedMethods": {
          "Quantity": 2,
          "Items": ["GET", "HEAD"]
        }
      },
      "CachePolicyId": "4135ea2d-6df8-44a3-9df3-4b5a84be11ad",
      "OriginRequestPolicyId": "216adef6-5c04-47e4-88d2-d96b42e00000",
      "ResponseHeadersPolicyId": $policy_id,
      "Compress": false,
      "SmoothStreaming": false,
      "TrustedSigners": {
        "Enabled": false,
        "Quantity": 0
      },
      "TrustedKeyGroups": {
        "Enabled": false,
        "Quantity": 0
      }
    }]
  ) |
  .DistributionConfig.CacheBehaviors.Quantity = (.DistributionConfig.CacheBehaviors.Items | length)
' /tmp/current-config.json > /tmp/updated-config.json

# Step 4: Update the distribution
ETAG=$(jq -r '.ETag' /tmp/current-config.json)
echo "Updating CloudFront distribution (ETag: $ETAG)..."

aws cloudfront update-distribution \
  --id "$DISTRIBUTION_ID" \
  --distribution-config file:///tmp/updated-config.json \
  --if-match "$ETAG" \
  --query 'Distribution.{Id:Id,Status:Status,LastModifiedTime:LastModifiedTime}' \
  --output json

echo ""
echo "✅ CloudFront distribution updated successfully!"
echo ""
echo "⚠️  IMPORTANT: CloudFront distribution updates can take 15-30 minutes to deploy."
echo "   Check status with: aws cloudfront get-distribution --id $DISTRIBUTION_ID --query 'Distribution.Status'"
echo ""
echo "The /api/rpc cache behavior now:"
echo "  - Allows POST, GET, OPTIONS methods"
echo "  - Proxies to $RPC_NODE_IP:$RPC_NODE_PORT (default node)"
echo "  - Adds CORS headers via Response Headers Policy"
echo "  - Disables caching for POST requests"
echo ""
echo "Multi-Node Support:"
echo "  - The RPC client will try direct HTTPS endpoints first (if configured)"
echo "  - Falls back to CloudFront /api/rpc proxy for HTTP endpoints"
echo "  - Client handles failover automatically across all configured nodes"

