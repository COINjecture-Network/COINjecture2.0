#!/bin/bash
# Setup script for RPC proxy Lambda@Edge function

echo "📦 Creating Lambda@Edge function for RPC proxy..."

# Create deployment package
cd lambda-edge-rpc-proxy
zip -r function.zip index.js 2>&1 | tail -5

echo ""
echo "✅ Lambda function code prepared"
echo ""
echo "📝 Next steps:"
echo "1. Create IAM role for Lambda@Edge (if not exists)"
echo "2. Create Lambda function in us-east-1:"
echo "   aws lambda create-function \\"
echo "     --function-name coinjecture-rpc-proxy \\"
echo "     --runtime nodejs18.x \\"
echo "     --role arn:aws:iam::ACCOUNT:role/lambda-edge-role \\"
echo "     --handler index.handler \\"
echo "     --zip-file fileb://function.zip \\"
echo "     --region us-east-1"
echo ""
echo "3. Publish version:"
echo "   aws lambda publish-version --function-name coinjecture-rpc-proxy --region us-east-1"
echo ""
echo "4. Associate with CloudFront distribution E1F9JMDDFH6L9V"
echo ""
echo "See RPC-PROXY-SETUP.md for detailed instructions"
