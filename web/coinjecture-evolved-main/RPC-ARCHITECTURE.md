# RPC Architecture: Lock & Key Analysis

## 🔒 THE LOCK (The Problem)

### 1. **Protocol Mismatch (Mixed Content)**
   - **Frontend**: Served over HTTPS via CloudFront
   - **Backend RPC**: HTTP endpoints on droplet IPs (`143.110.139.166:9933`, `68.183.205.12:9933`)
   - **Browser Security**: Blocks HTTPS pages from making HTTP requests (Mixed Content Policy)
   - **Error**: `Mixed Content: The page was loaded over HTTPS, but requested an insecure resource`

### 2. **CORS Restrictions**
   - **RPC Server**: No CORS headers in `rpc/src/server.rs` (Rust backend)
   - **Browser**: Blocks cross-origin requests without proper CORS headers
   - **Origin Mismatch**: CloudFront domain ≠ Droplet IP addresses
   - **Error**: `Access to fetch at 'http://...' from origin 'https://...' has been blocked by CORS policy`

### 3. **CloudFront Limitations**
   - Cannot use IP addresses as custom origins directly
   - Cache behaviors require valid domain-based origins
   - S3 origins don't support forwarding all headers (especially `*` wildcard)

### 4. **Network Architecture Mismatch**
   - Frontend: CloudFront CDN (HTTPS, edge locations worldwide)
   - Backend: Direct IP addresses (HTTP, no domain)
   - No direct bridge between them

## 🔑 THE KEY (The Solution)

### **Lambda@Edge as Protocol & Origin Bridge**

**How it works:**
1. **Interception**: Lambda@Edge intercepts `/api/rpc` requests at CloudFront edge (viewer-request event)
2. **Direct HTTP Call**: Makes direct HTTP request to backend droplet (bypasses CloudFront origin entirely)
3. **CORS Injection**: Adds CORS headers to response before returning to browser
4. **Protocol Translation**: Converts HTTPS request → HTTP backend → HTTPS response

**The Flow:**
```
Browser (HTTPS) 
  → CloudFront Distribution (HTTPS)
    → Lambda@Edge viewer-request (intercepts /api/rpc)
      → Direct HTTP call to droplet:9933 (bypasses origin)
      → Receives JSON-RPC response
      → Injects CORS headers
      → Returns response directly
    ← Response with CORS headers
  ← HTTPS response
← Browser (sees HTTPS response, no mixed content error)
```

**Key Insight:**
- Lambda@Edge `viewer-request` can return a response directly, completely bypassing CloudFront's origin system
- The cache behavior's origin is just a placeholder - Lambda handles the actual request
- This solves both mixed content AND CORS in one layer

## Implementation Details

### Lambda@Edge Function
- **Location**: `lambda-edge-rpc-proxy/index.js`
- **Event Type**: `viewer-request`
- **Functionality**:
  - Parses `target` parameter from query string (`?target=143.110.139.166:9933`)
  - Makes direct HTTP request to target node
  - Adds CORS headers to response
  - Returns response directly to browser

### Frontend RPC Client
- **Location**: `src/lib/rpc-client.ts`
- **Behavior**:
  - Development: Uses Vite proxy (`/api/rpc`)
  - Production HTTPS: Uses CloudFront proxy (`/api/rpc?target=host:port`)
  - Supports multiple nodes with failover

### CloudFront Configuration
- **Cache Behavior**: `/api/rpc` pattern
- **Origin**: S3 bucket (placeholder - Lambda bypasses it)
- **Lambda Association**: `viewer-request` event
- **ForwardedValues**: Minimal (no headers - Lambda handles it)

## Why This Works

1. **Protocol Bridge**: Lambda makes HTTP call from edge location (server-side), browser only sees HTTPS
2. **CORS Solution**: Lambda injects CORS headers, bypassing backend limitations
3. **Origin Bypass**: Lambda returns response directly, origin is irrelevant
4. **Multi-Node Support**: Query parameter allows routing to any node

## Alternative Solutions (Not Used)

1. **Add CORS to RPC Server**: Would solve CORS but not mixed content
2. **HTTPS on Droplets**: Would solve mixed content but requires certificates/domains
3. **CloudFront Custom Origin with Domain**: Would require DNS setup for each node
4. **API Gateway**: More expensive, adds latency

## Current Status

✅ Lambda@Edge function deployed  
✅ CloudFront distribution updated  
✅ Cache behavior configured  
⏳ Distribution deployment in progress (5-15 minutes)

