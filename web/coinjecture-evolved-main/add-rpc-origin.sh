#!/bin/bash
# Get current config
aws cloudfront get-distribution-config --id E1F9JMDDFH6L9V --output json > /tmp/current-config.json

# Use jq to add RPC origin and behavior (keep CallerReference)
jq '.DistributionConfig.Origins.Items += [{
  "Id": "RPC-Node1",
  "DomainName": "143.110.139.166",
  "CustomOriginConfig": {
    "HTTPPort": 9933,
    "HTTPSPort": 443,
    "OriginProtocolPolicy": "http-only",
    "OriginSslProtocols": {"Quantity": 0, "Items": []},
    "OriginReadTimeout": 30,
    "OriginKeepaliveTimeout": 5
  }
}] | .DistributionConfig.Origins.Quantity = (.DistributionConfig.Origins.Items | length) |
(.DistributionConfig.CacheBehaviors // {"Quantity": 0, "Items": []}) as $behaviors |
.DistributionConfig.CacheBehaviors = ($behaviors | .Items += [{
  "PathPattern": "/api/rpc",
  "TargetOriginId": "RPC-Node1",
  "ViewerProtocolPolicy": "redirect-to-https",
  "AllowedMethods": {
    "Quantity": 7,
    "Items": ["GET", "HEAD", "OPTIONS", "PUT", "POST", "PATCH", "DELETE"],
    "CachedMethods": {"Quantity": 2, "Items": ["GET", "HEAD"]}
  },
  "ForwardedValues": {
    "QueryString": true,
    "Cookies": {"Forward": "none"},
    "Headers": {"Quantity": 1, "Items": ["*"]}
  },
  "MinTTL": 0,
  "DefaultTTL": 0,
  "MaxTTL": 0,
  "Compress": false
}] | .Quantity = (.Items | length)) |
.DistributionConfig' /tmp/current-config.json > /tmp/updated-config.json

ETAG=$(jq -r '.ETag' /tmp/current-config.json)
aws cloudfront update-distribution --id E1F9JMDDFH6L9V --distribution-config file:///tmp/updated-config.json --if-match "$ETAG" --query 'Distribution.{Id:Id,Status:Status}' --output json
