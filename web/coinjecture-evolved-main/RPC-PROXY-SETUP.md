# RPC Proxy Setup for CloudFront

This document explains how to set up RPC proxying through CloudFront to connect to any node.

## Problem

When the frontend is served over HTTPS via CloudFront, it cannot make direct HTTP requests to RPC endpoints due to:
1. **Mixed Content**: Browsers block HTTP requests from HTTPS pages
2. **CORS**: Cross-origin requests require proper headers
3. **CloudFront Limitations**: IP addresses cannot be used as origins

## Solution: Lambda@Edge Proxy

Use a Lambda@Edge function to proxy `/api/rpc` requests to any RPC endpoint.

## Setup Steps

### 1. Create Lambda@Edge Function

```bash
cd lambda-edge-rpc-proxy
zip -r function.zip index.js package.json
```

### 2. Create Lambda Function in us-east-1

```bash
aws lambda create-function \
  --function-name coinjecture-rpc-proxy \
  --runtime nodejs18.x \
  --role arn:aws:iam::YOUR_ACCOUNT:role/lambda-edge-execution-role \
  --handler index.handler \
  --zip-file fileb://function.zip \
  --region us-east-1
```

**Important**: Lambda@Edge functions must be created in `us-east-1`.

### 3. Publish Function Version

```bash
aws lambda publish-version \
  --function-name coinjecture-rpc-proxy \
  --region us-east-1
```

### 4. Associate with CloudFront Distribution

```bash
# Get the function ARN from the publish-version output
FUNCTION_ARN="arn:aws:lambda:us-east-1:ACCOUNT:function:coinjecture-rpc-proxy:VERSION"

# Update CloudFront distribution to use the function
# This requires updating the distribution config to add the Lambda@Edge association
```

### 5. Update CloudFront Distribution

Add the Lambda@Edge function to the distribution's cache behavior for `/api/rpc`:

```json
{
  "PathPattern": "/api/rpc",
  "LambdaFunctionAssociations": {
    "Quantity": 1,
    "Items": [{
      "LambdaFunctionARN": "arn:aws:lambda:us-east-1:ACCOUNT:function:coinjecture-rpc-proxy:VERSION",
      "EventType": "viewer-request"
    }]
  }
}
```

## Alternative: Simple CORS Proxy Service

If Lambda@Edge is too complex, you can use a simple CORS proxy:

1. Deploy a simple Node.js proxy service (e.g., on EC2, ECS, or Lambda)
2. Update RPC client to use the proxy URL
3. Proxy forwards requests to target RPC endpoints

## RPC Client Configuration

The RPC client automatically detects HTTPS and uses `/api/rpc` with target parameter:

```typescript
// Automatically uses /api/rpc?target=143.110.139.166:9933 in production HTTPS
const urls = ['/api/rpc?target=143.110.139.166:9933', '/api/rpc?target=68.183.205.12:9933'];
```

## Testing

After setup, test with:

```bash
curl -X POST https://d1f2zzpbyxllz7.cloudfront.net/api/rpc?target=143.110.139.166:9933 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"chain_getInfo","params":[]}'
```

## Troubleshooting

- **502 Bad Gateway**: Lambda function error - check CloudWatch logs
- **403 Forbidden**: Lambda permissions issue
- **Timeout**: Increase Lambda timeout (max 5 seconds for viewer-request)
- **CORS errors**: Ensure Lambda adds CORS headers

