# COINjecture Frontend - AWS S3 + CloudFront Deployment Guide

This guide covers deploying the COINjecture frontend to AWS S3 with CloudFront CDN for optimal global performance.

## Prerequisites

- AWS Account with appropriate permissions
- AWS CLI installed and configured
- Node.js 18+ and npm installed
- Domain name (optional, for custom domain)

## Quick Start

### 1. Build the Frontend

```bash
cd web/coinjecture-evolved-main
npm install
npm run build
```

This creates a `dist/` directory with optimized static files ready for deployment.

### 2. Configure Environment Variables

Create a `.env.production` file:

```bash
VITE_RPC_URL=https://your-rpc-endpoint.com:9933
VITE_METRICS_URL=https://your-metrics-endpoint.com:9094
```

**Important**: For CloudFront deployment, you'll need to handle CORS properly. The RPC endpoint should:
- Allow requests from your CloudFront domain
- Support CORS headers
- Use HTTPS in production

### 3. Create S3 Bucket

```bash
# Create bucket (replace with your bucket name)
aws s3 mb s3://coinjecture-frontend --region us-east-1

# Enable static website hosting
aws s3 website s3://coinjecture-frontend \
  --index-document index.html \
  --error-document index.html
```

### 4. Upload Build Files

```bash
# Upload all files from dist/ to S3
aws s3 sync dist/ s3://coinjecture-frontend --delete

# Set proper content types
aws s3 cp dist/index.html s3://coinjecture-frontend/index.html \
  --content-type "text/html" --cache-control "no-cache"

aws s3 sync dist/assets s3://coinjecture-frontend/assets \
  --content-type "application/javascript" \
  --cache-control "public, max-age=31536000, immutable"
```

### 5. Create CloudFront Distribution

#### Option A: Using AWS Console

1. Go to CloudFront in AWS Console
2. Create Distribution
3. Configure:
   - **Origin Domain**: Select your S3 bucket
   - **Origin Path**: Leave empty
   - **Viewer Protocol Policy**: Redirect HTTP to HTTPS
   - **Allowed HTTP Methods**: GET, HEAD, OPTIONS
   - **Cache Policy**: CachingOptimized
   - **Default Root Object**: index.html
   - **Error Pages**: 
     - HTTP Error Code: 403, 404
     - Response Page Path: /index.html
     - HTTP Response Code: 200

#### Option B: Using AWS CLI

```bash
# Create CloudFront distribution configuration
cat > cloudfront-config.json <<EOF
{
  "CallerReference": "coinjecture-frontend-$(date +%s)",
  "Comment": "COINjecture Frontend Distribution",
  "DefaultRootObject": "index.html",
  "Origins": {
    "Quantity": 1,
    "Items": [
      {
        "Id": "S3-coinjecture-frontend",
        "DomainName": "coinjecture-frontend.s3.amazonaws.com",
        "S3OriginConfig": {
          "OriginAccessIdentity": ""
        }
      }
    ]
  },
  "DefaultCacheBehavior": {
    "TargetOriginId": "S3-coinjecture-frontend",
    "ViewerProtocolPolicy": "redirect-to-https",
    "AllowedMethods": {
      "Quantity": 3,
      "Items": ["GET", "HEAD", "OPTIONS"],
      "CachedMethods": {
        "Quantity": 2,
        "Items": ["GET", "HEAD"]
      }
    },
    "Compress": true,
    "ForwardedValues": {
      "QueryString": false,
      "Cookies": {
        "Forward": "none"
      }
    },
    "MinTTL": 0,
    "DefaultTTL": 86400,
    "MaxTTL": 31536000
  },
  "CustomErrorResponses": {
    "Quantity": 2,
    "Items": [
      {
        "ErrorCode": 403,
        "ResponsePagePath": "/index.html",
        "ResponseCode": "200",
        "ErrorCachingMinTTL": 300
      },
      {
        "ErrorCode": 404,
        "ResponsePagePath": "/index.html",
        "ResponseCode": "200",
        "ErrorCachingMinTTL": 300
      }
    ]
  },
  "Enabled": true,
  "PriceClass": "PriceClass_100"
}
EOF

# Create distribution
aws cloudfront create-distribution --distribution-config file://cloudfront-config.json
```

### 6. Configure Custom Domain (Optional)

1. Request SSL certificate in AWS Certificate Manager (ACM)
2. Add alternate domain name (CNAME) to CloudFront distribution
3. Update DNS records to point to CloudFront distribution

```bash
# Example DNS record (Route 53)
# Type: CNAME
# Name: coinjecture.com
# Value: d1234567890.cloudfront.net
```

## Deployment Script

Create `deploy.sh` for automated deployment:

```bash
#!/bin/bash
set -e

echo "Building frontend..."
npm run build

echo "Uploading to S3..."
aws s3 sync dist/ s3://coinjecture-frontend --delete

echo "Invalidating CloudFront cache..."
DISTRIBUTION_ID=$(aws cloudfront list-distributions \
  --query "DistributionList.Items[?Comment=='COINjecture Frontend Distribution'].Id" \
  --output text)

if [ ! -z "$DISTRIBUTION_ID" ]; then
  aws cloudfront create-invalidation \
    --distribution-id $DISTRIBUTION_ID \
    --paths "/*"
  echo "Cache invalidation created for distribution: $DISTRIBUTION_ID"
else
  echo "Warning: Could not find CloudFront distribution"
fi

echo "Deployment complete!"
```

Make it executable:
```bash
chmod +x deploy.sh
```

## CORS Configuration

If your RPC endpoint is on a different domain, ensure CORS is properly configured:

### On Your RPC Server

Add CORS headers to allow requests from your CloudFront domain:

```rust
// Example for Actix-web
HttpResponse::Ok()
    .header("Access-Control-Allow-Origin", "https://coinjecture.com")
    .header("Access-Control-Allow-Methods", "POST, GET, OPTIONS")
    .header("Access-Control-Allow-Headers", "Content-Type")
    .json(response)
```

### CloudFront Behavior

Create a custom cache behavior for API requests:

```bash
# If using API Gateway or separate API endpoint
# Add custom origin for API with CORS support
```

## Environment-Specific Builds

For different environments, use different build commands:

```bash
# Development
npm run build

# Staging
VITE_RPC_URL=https://staging-api.coinjecture.com:9933 npm run build

# Production
VITE_RPC_URL=https://api.coinjecture.com:9933 npm run build
```

## Monitoring

### CloudWatch Metrics

Monitor CloudFront distribution:
- Requests
- Bytes downloaded
- Error rates (4xx, 5xx)
- Cache hit ratio

### Set up Alarms

```bash
aws cloudwatch put-metric-alarm \
  --alarm-name coinjecture-frontend-errors \
  --alarm-description "Alert on high error rate" \
  --metric-name 4xxErrorRate \
  --namespace AWS/CloudFront \
  --statistic Average \
  --period 300 \
  --threshold 5.0 \
  --comparison-operator GreaterThanThreshold
```

## Security Best Practices

1. **Enable WAF**: Add AWS WAF to protect against common attacks
2. **HTTPS Only**: Enforce HTTPS redirects
3. **Security Headers**: Add security headers via CloudFront response headers policy
4. **Bucket Policy**: Restrict S3 bucket access to CloudFront only

### Example Bucket Policy

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Sid": "AllowCloudFrontServicePrincipal",
      "Effect": "Allow",
      "Principal": {
        "Service": "cloudfront.amazonaws.com"
      },
      "Action": "s3:GetObject",
      "Resource": "arn:aws:s3:::coinjecture-frontend/*",
      "Condition": {
        "StringEquals": {
          "AWS:SourceArn": "arn:aws:cloudfront::ACCOUNT_ID:distribution/DISTRIBUTION_ID"
        }
      }
    }
  ]
}
```

## Troubleshooting

### Issue: Blank page after deployment

**Solution**: Check that `index.html` is set as default root object and error pages are configured.

### Issue: API calls failing with CORS errors

**Solution**: 
1. Verify RPC endpoint CORS configuration
2. Check CloudFront origin settings
3. Ensure environment variable `VITE_RPC_URL` is set correctly

### Issue: Stale content after update

**Solution**: 
1. Invalidate CloudFront cache: `aws cloudfront create-invalidation --distribution-id DIST_ID --paths "/*"`
2. Check cache headers in S3 upload

## Cost Optimization

- **Price Class**: Use `PriceClass_100` for US/Europe only (cheaper)
- **Compression**: Enable CloudFront compression
- **Cache**: Optimize cache headers for static assets
- **S3 Lifecycle**: Move old files to Glacier if needed

## Next Steps

1. Set up CI/CD pipeline (GitHub Actions, GitLab CI, etc.)
2. Configure custom domain with SSL
3. Set up monitoring and alerts
4. Enable CloudFront access logs
5. Configure WAF rules

## Integration with coinjecture.com

Once CloudFront is set up:

1. Point `coinjecture.com` DNS to CloudFront distribution
2. Update environment variables to use production RPC endpoint
3. Rebuild and deploy
4. Test all functionality

The frontend will be accessible at `https://coinjecture.com` with global CDN acceleration.

