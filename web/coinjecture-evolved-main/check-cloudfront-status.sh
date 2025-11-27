#!/bin/bash
echo "Checking CloudFront distribution status..."
STATUS=$(aws cloudfront get-distribution --id E1F9JMDDFH6L9V --query 'Distribution.Status' --output text 2>/dev/null)
echo "Status: $STATUS"
if [ "$STATUS" = "Deployed" ]; then
  echo "✅ Distribution is deployed and ready!"
  echo "🌐 URL: https://d1f2zzpbyxllz7.cloudfront.net"
else
  echo "⏳ Still deploying... This can take 5-15 minutes."
fi
