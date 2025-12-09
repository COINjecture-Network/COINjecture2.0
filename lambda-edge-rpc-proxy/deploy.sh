#!/bin/bash
# Deploy Lambda@Edge function for RPC proxy

set -e

FUNCTION_NAME="coinjecture-rpc-proxy"
REGION="us-east-1"  # Lambda@Edge must be in us-east-1
CLOUDFRONT_DIST_ID="${CLOUDFRONT_DIST_ID:-E1F9JMDDFH6L9V}"  # Default distribution ID

echo "📦 Packaging Lambda@Edge function..."

# Package the function
cd "$(dirname "$0")"
zip -r function.zip index.js package.json 2>&1 | tail -5

echo ""
echo "📤 Updating Lambda function..."

# Check if function exists
if aws lambda get-function --function-name "$FUNCTION_NAME" --region "$REGION" &>/dev/null; then
    echo "✅ Function exists, updating code..."
    aws lambda update-function-code \
        --function-name "$FUNCTION_NAME" \
        --zip-file fileb://function.zip \
        --region "$REGION" \
        --output json | jq -r '.FunctionArn'
    
    echo "⏳ Waiting for function update to complete..."
    aws lambda wait function-updated \
        --function-name "$FUNCTION_NAME" \
        --region "$REGION"
else
    echo "❌ Function does not exist. Please create it first:"
    echo ""
    echo "aws lambda create-function \\"
    echo "  --function-name $FUNCTION_NAME \\"
    echo "  --runtime nodejs18.x \\"
    echo "  --role arn:aws:iam::YOUR_ACCOUNT:role/lambda-edge-rpc-proxy-role \\"
    echo "  --handler index.handler \\"
    echo "  --zip-file fileb://function.zip \\"
    echo "  --region $REGION"
    echo ""
    exit 1
fi

echo ""
echo "📝 Publishing new version (required for Lambda@Edge)..."

# Publish new version
VERSION=$(aws lambda publish-version \
    --function-name "$FUNCTION_NAME" \
    --region "$REGION" \
    --query 'Version' \
    --output text)

FUNCTION_ARN=$(aws lambda get-function \
    --function-name "$FUNCTION_NAME" \
    --region "$REGION" \
    --query 'Configuration.FunctionArn' \
    --output text)

VERSIONED_ARN="${FUNCTION_ARN}:${VERSION}"

echo "✅ Published version: $VERSION"
echo "📋 Function ARN: $VERSIONED_ARN"

echo ""
echo "📋 Next steps:"
echo "1. Update CloudFront distribution $CLOUDFRONT_DIST_ID to use version $VERSION"
echo "2. The Lambda@Edge association should use ARN: $VERSIONED_ARN"
echo ""
echo "To update CloudFront manually:"
echo "  aws cloudfront get-distribution-config --id $CLOUDFRONT_DIST_ID > dist-config.json"
echo "  # Edit dist-config.json to update LambdaFunctionARN to $VERSIONED_ARN"
echo "  aws cloudfront update-distribution --id $CLOUDFRONT_DIST_ID --if-match ETAG --distribution-config file://dist-config.json"
echo ""
echo "Or use AWS Console: CloudFront > Distributions > $CLOUDFRONT_DIST_ID > Behaviors > Edit > Lambda@Edge"

# Cleanup
rm -f function.zip

echo ""
echo "✅ Deployment complete! Version $VERSION is ready."
echo "⚠️  Note: CloudFront distribution updates can take 5-15 minutes to propagate."

