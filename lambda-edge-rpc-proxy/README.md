# RPC Proxy Lambda@Edge Function

This Lambda@Edge function proxies RPC requests from CloudFront to any COINjecture node.

## Features

- ✅ Proxies to any node via `?target=host:port` parameter
- ✅ Adds CORS headers automatically
- ✅ Handles errors gracefully
- ✅ Works with multiple nodes (failover support)

## Deployment

### 1. Create IAM Role for Lambda@Edge

```bash
# Create trust policy
cat > trust-policy.json <<EOF
{
  "Version": "2012-10-17",
  "Statement": [{
    "Effect": "Allow",
    "Principal": {
      "Service": ["lambda.amazonaws.com", "edgelambda.amazonaws.com"]
    },
    "Action": "sts:AssumeRole"
  }]
}
EOF

# Create role
aws iam create-role \
  --role-name lambda-edge-rpc-proxy-role \
  --assume-role-policy-document file://trust-policy.json

# Attach basic execution policy
aws iam attach-role-policy \
  --role-name lambda-edge-rpc-proxy-role \
  --policy-arn arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole
```

### 2. Package and Deploy

```bash
# Package function
zip -r function.zip index.js package.json

# Create function in us-east-1 (REQUIRED for Lambda@Edge)
aws lambda create-function \
  --function-name coinjecture-rpc-proxy \
  --runtime nodejs18.x \
  --role arn:aws:iam::YOUR_ACCOUNT:role/lambda-edge-rpc-proxy-role \
  --handler index.handler \
  --zip-file fileb://function.zip \
  --region us-east-1

# Publish version (REQUIRED for Lambda@Edge)
VERSION=$(aws lambda publish-version \
  --function-name coinjecture-rpc-proxy \
  --region us-east-1 \
  --query 'Version' \
  --output text)

echo "Function version: $VERSION"
```

### 3. Associate with CloudFront

Update your CloudFront distribution to use this function for `/api/rpc` requests. See `RPC-PROXY-SETUP.md` for detailed instructions.

## Usage

The RPC client automatically uses this proxy when served over HTTPS:

```
/api/rpc?target=143.110.139.166:9933
/api/rpc?target=68.183.205.12:9933
```

## Testing

```bash
curl -X POST "https://d1f2zzpbyxllz7.cloudfront.net/api/rpc?target=143.110.139.166:9933" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"chain_getInfo","params":[]}'
```

