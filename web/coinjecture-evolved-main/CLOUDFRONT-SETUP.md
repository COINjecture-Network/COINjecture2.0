# CloudFront Integration Guide for coinjecture.com

This guide covers integrating the COINjecture frontend with CloudFront and pointing it to `coinjecture.com`.

## Overview

The frontend is now optimized for static hosting on AWS S3 with CloudFront CDN. This provides:
- Global CDN acceleration
- HTTPS/SSL support
- Custom domain integration
- Automatic cache invalidation
- Cost-effective scaling

## Architecture

```
User → CloudFront (coinjecture.com) → S3 Bucket (Static Files)
                                    ↓
                              RPC Endpoint (API calls)
```

## Step-by-Step Setup

### 1. Build and Test Locally

```bash
cd web/coinjecture-evolved-main
npm install
npm run dev
```

Verify everything works at `http://localhost:8080`

### 2. Configure Production Environment

Create `.env.production`:

```env
VITE_RPC_URL=https://api.coinjecture.com:9933
VITE_METRICS_URL=https://metrics.coinjecture.com:9094
```

**Important**: Replace with your actual RPC endpoint URLs.

### 3. Create S3 Bucket

```bash
# Create bucket
aws s3 mb s3://coinjecture-frontend-prod --region us-east-1

# Enable static website hosting
aws s3 website s3://coinjecture-frontend-prod \
  --index-document index.html \
  --error-document index.html
```

### 4. Request SSL Certificate

For `coinjecture.com` and `www.coinjecture.com`:

```bash
# Request certificate in us-east-1 (required for CloudFront)
aws acm request-certificate \
  --domain-name coinjecture.com \
  --subject-alternative-names www.coinjecture.com \
  --validation-method DNS \
  --region us-east-1
```

Follow DNS validation instructions to verify domain ownership.

### 5. Create CloudFront Distribution

#### Using AWS Console:

1. Go to CloudFront → Create Distribution
2. **Origin Settings**:
   - Origin Domain: `coinjecture-frontend-prod.s3.amazonaws.com`
   - Origin Path: (leave empty)
   - Origin Access: Use OAC (Origin Access Control)
3. **Default Cache Behavior**:
   - Viewer Protocol Policy: Redirect HTTP to HTTPS
   - Allowed HTTP Methods: GET, HEAD, OPTIONS
   - Cache Policy: CachingOptimized
   - Compress Objects: Yes
4. **Settings**:
   - Alternate Domain Names (CNAMEs): `coinjecture.com`, `www.coinjecture.com`
   - SSL Certificate: Select your ACM certificate
   - Default Root Object: `index.html`
   - Custom Error Responses:
     - 403 → `/index.html` (200)
     - 404 → `/index.html` (200)
5. Create Distribution

#### Using AWS CLI:

```bash
# First, create OAC (Origin Access Control)
aws cloudfront create-origin-access-control \
  --origin-access-control-config Name=coinjecture-oac,OriginAccessControlOriginType=s3,SigningBehavior=always,SigningProtocol=sigv4

# Get OAC ID from response, then create distribution
# (See DEPLOYMENT.md for full JSON configuration)
```

### 6. Update DNS Records

Point your domain to CloudFront:

**Route 53 (if using AWS)**:
```bash
# Get CloudFront distribution domain
DIST_DOMAIN=$(aws cloudfront list-distributions \
  --query "DistributionList.Items[?Comment=='COINjecture Frontend'].DomainName" \
  --output text)

# Create alias record
aws route53 change-resource-record-sets \
  --hosted-zone-id YOUR_ZONE_ID \
  --change-batch '{
    "Changes": [{
      "Action": "UPSERT",
      "ResourceRecordSet": {
        "Name": "coinjecture.com",
        "Type": "A",
        "AliasTarget": {
          "HostedZoneId": "Z2FDTNDATAQYW2",
          "DNSName": "'$DIST_DOMAIN'",
          "EvaluateTargetHealth": false
        }
      }
    }]
  }'
```

**Other DNS Providers**:
- Create CNAME record: `coinjecture.com` → `d1234567890abc.cloudfront.net`
- Or A record with CloudFront IPs (not recommended)

### 7. Configure S3 Bucket Policy

Restrict bucket access to CloudFront only:

```bash
# Get OAC ARN
OAC_ARN=$(aws cloudfront list-origin-access-controls \
  --query "OriginAccessControlList.Items[?Name=='coinjecture-oac'].Id" \
  --output text)

# Create bucket policy
cat > bucket-policy.json <<EOF
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
      "Resource": "arn:aws:s3:::coinjecture-frontend-prod/*",
      "Condition": {
        "StringEquals": {
          "AWS:SourceArn": "arn:aws:cloudfront::ACCOUNT_ID:distribution/DISTRIBUTION_ID"
        }
      }
    }
  ]
}
EOF

aws s3api put-bucket-policy \
  --bucket coinjecture-frontend-prod \
  --policy file://bucket-policy.json
```

### 8. Deploy Frontend

```bash
# Set environment variables
export S3_BUCKET=coinjecture-frontend-prod
export CLOUDFRONT_DIST_ID=YOUR_DISTRIBUTION_ID
export VITE_RPC_URL=https://api.coinjecture.com:9933

# Deploy
./deploy.sh
```

### 9. Verify Deployment

1. Wait for CloudFront distribution to deploy (15-30 minutes)
2. Test `https://coinjecture.com`
3. Check browser console for any errors
4. Verify RPC calls are working
5. Test all routes (/, /marketplace, /metrics, etc.)

## CORS Configuration

Ensure your RPC endpoint allows requests from `coinjecture.com`:

```rust
// Example CORS configuration
HttpResponse::Ok()
    .header("Access-Control-Allow-Origin", "https://coinjecture.com")
    .header("Access-Control-Allow-Methods", "POST, GET, OPTIONS")
    .header("Access-Control-Allow-Headers", "Content-Type")
    .header("Access-Control-Max-Age", "86400")
    .json(response)
```

## Monitoring

### CloudWatch Metrics

Monitor:
- Requests per second
- Error rates (4xx, 5xx)
- Cache hit ratio
- Data transfer

### Set Up Alarms

```bash
# High error rate alarm
aws cloudwatch put-metric-alarm \
  --alarm-name coinjecture-frontend-errors \
  --alarm-description "Alert on high error rate" \
  --metric-name 4xxErrorRate \
  --namespace AWS/CloudFront \
  --statistic Average \
  --period 300 \
  --threshold 5.0 \
  --comparison-operator GreaterThanThreshold \
  --evaluation-periods 2
```

## Cache Invalidation

After deploying updates:

```bash
./deploy.sh  # Automatically invalidates cache

# Or manually:
aws cloudfront create-invalidation \
  --distribution-id YOUR_DIST_ID \
  --paths "/*"
```

## Security Hardening

### 1. Enable WAF

```bash
# Create WAF web ACL
aws wafv2 create-web-acl \
  --scope CLOUDFRONT \
  --default-action Allow={} \
  --rules file://waf-rules.json \
  --name coinjecture-waf \
  --region us-east-1
```

### 2. Security Headers

Create CloudFront Response Headers Policy:

```json
{
  "ResponseHeadersPolicyConfig": {
    "Name": "coinjecture-security-headers",
    "SecurityHeadersConfig": {
      "StrictTransportSecurity": {
        "Override": true,
        "AccessControlMaxAgeSec": 31536000,
        "IncludeSubdomains": true
      },
      "ContentTypeOptions": {
        "Override": true
      },
      "FrameOptions": {
        "Override": true,
        "FrameOption": "DENY"
      },
      "XSSProtection": {
        "Override": true,
        "ModeBlock": true,
        "Protection": true
      }
    }
  }
}
```

### 3. Restrict S3 Access

Only allow CloudFront to access S3 (already configured in step 7).

## Troubleshooting

### Issue: Blank page

**Check**:
- CloudFront error pages configured correctly
- S3 bucket has `index.html`
- Distribution is deployed (not "In Progress")

### Issue: CORS errors

**Check**:
- RPC endpoint CORS headers
- `VITE_RPC_URL` environment variable
- Browser console for specific error

### Issue: Stale content

**Solution**:
```bash
./deploy.sh  # Invalidates cache automatically
```

### Issue: SSL certificate errors

**Check**:
- Certificate is validated in ACM
- Certificate is in `us-east-1` region
- CNAME records are correct

## Cost Optimization

1. **Price Class**: Use `PriceClass_100` (US, Canada, Europe only)
2. **Compression**: Enable CloudFront compression
3. **Cache**: Optimize cache headers
4. **Monitoring**: Set up billing alerts

## Next Steps

1. ✅ Frontend deployed to CloudFront
2. ✅ Domain pointing to CloudFront
3. ⏳ Set up CI/CD pipeline
4. ⏳ Configure monitoring and alerts
5. ⏳ Enable WAF protection
6. ⏳ Set up backup/rollback procedures

## Support

For issues:
1. Check CloudFront distribution status
2. Review CloudWatch logs
3. Verify S3 bucket contents
4. Test RPC endpoint directly
5. Check browser console for errors

---

**Status**: Ready for CloudFront deployment
**Last Updated**: 2025-01-XX

