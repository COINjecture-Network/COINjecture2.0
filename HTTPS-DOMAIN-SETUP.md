# HTTPS Setup with Domain + Let's Encrypt (Recommended)

This guide explains how to set up HTTPS for RPC endpoints using domain names and Let's Encrypt certificates. This is the **recommended approach for production** as it provides trusted certificates with no browser warnings.

## Overview

**How It Works:**
1. You register domain names (e.g., `rpc1.coinjecture.com`)
2. Point DNS A records to your node IP addresses
3. Install a reverse proxy (Caddy or nginx) on each node
4. The reverse proxy automatically obtains Let's Encrypt SSL certificates
5. The proxy handles HTTPS and forwards requests to the RPC server (port 9933)
6. Browsers trust Let's Encrypt certificates (no warnings)

## Prerequisites

### 1. Domain Names

You need domain names for each RPC endpoint. Options:

**Option A: Subdomains of existing domain**
- If you own `coinjecture.com`, create:
  - `rpc1.coinjecture.com` → Droplet 1 (143.110.139.166)
  - `rpc2.coinjecture.com` → Droplet 2 (68.183.205.12)
  - `rpc3.coinjecture.com` → GCE VM (35.184.253.150)

**Option B: Separate domains**
- `coinjecture-rpc1.com` → Droplet 1
- `coinjecture-rpc2.com` → Droplet 2
- `coinjecture-rpc3.com` → GCE VM

**Option C: Single domain with different ports** (not recommended)
- `rpc.coinjecture.com:443` → Droplet 1
- `rpc.coinjecture.com:8443` → Droplet 2
- (Browsers don't like non-standard HTTPS ports)

### 2. DNS Configuration

Create **A records** in your DNS provider pointing to node IPs:

```
Type: A
Name: rpc1
Value: 143.110.139.166
TTL: 300 (5 minutes)

Type: A
Name: rpc2
Value: 68.183.205.12
TTL: 300

Type: A
Name: rpc3
Value: 35.184.253.150
TTL: 300
```

**DNS Propagation:** Changes typically take 5 minutes to 48 hours, but usually within 1 hour.

### 3. Port Requirements

Ensure these ports are open:
- **Port 80 (HTTP)**: Required for Let's Encrypt validation
- **Port 443 (HTTPS)**: Required for RPC access
- **Port 9933 (RPC)**: Must be accessible from localhost (reverse proxy)

## How Let's Encrypt Works

1. **Validation**: Let's Encrypt verifies you control the domain via:
   - **HTTP-01 Challenge**: Places a file at `http://domain/.well-known/acme-challenge/TOKEN`
   - **DNS-01 Challenge**: Requires adding a TXT record (alternative method)

2. **Certificate Issuance**: Once validated, Let's Encrypt issues a certificate valid for 90 days

3. **Auto-Renewal**: Caddy and certbot automatically renew certificates before expiration

4. **Trust**: Browsers trust Let's Encrypt certificates (they're in the root CA store)

## Setup Methods

### Method 1: Caddy (Recommended - Easiest)

**Why Caddy?**
- ✅ Automatic HTTPS (no manual certificate management)
- ✅ Automatic certificate renewal
- ✅ Simple configuration
- ✅ Built-in CORS support
- ✅ Zero-downtime certificate updates

**Setup Process:**

```bash
# For Droplet 1
./scripts/setup-https-rpc-caddy.sh 143.110.139.166 rpc1.coinjecture.com

# For Droplet 2
./scripts/setup-https-rpc-caddy.sh 68.183.205.12 rpc2.coinjecture.com

# For GCE VM (via gcloud)
gcloud compute ssh coinject-node --zone=us-central1-a
# Then run the Caddy setup manually
```

**What the Script Does:**
1. Installs Caddy reverse proxy
2. Creates `/etc/caddy/Caddyfile` with:
   - Domain name
   - Reverse proxy to `127.0.0.1:9933`
   - CORS headers
   - OPTIONS preflight handling
3. Caddy automatically:
   - Obtains Let's Encrypt certificate
   - Configures HTTPS
   - Sets up auto-renewal
4. Starts Caddy service

**Manual Setup:**

```bash
# Install Caddy
apt-get update
apt-get install -y debian-keyring debian-archive-keyring apt-transport-https
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' | gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' | tee /etc/apt/sources.list.d/caddy-stable.list
apt-get update
apt-get install -y caddy

# Create Caddyfile
cat > /etc/caddy/Caddyfile << EOF
rpc1.coinjecture.com {
    reverse_proxy 127.0.0.1:9933 {
        header_up Host {host}
        header_up X-Real-IP {remote}
        header_up X-Forwarded-For {remote_host}
        header_up X-Forwarded-Proto {scheme}
    }
    
    # CORS headers (RPC server also has CORS, but proxy adds extra layer)
    header {
        Access-Control-Allow-Origin "*"
        Access-Control-Allow-Methods "POST, GET, OPTIONS"
        Access-Control-Allow-Headers "Content-Type"
    }
    
    # Handle OPTIONS preflight
    @options {
        method OPTIONS
    }
    handle @options {
        header {
            Access-Control-Allow-Origin "*"
            Access-Control-Allow-Methods "POST, GET, OPTIONS"
            Access-Control-Allow-Headers "Content-Type"
            Access-Control-Max-Age "86400"
        }
        respond 204
    }
}
EOF

# Validate and start
caddy validate --config /etc/caddy/Caddyfile
systemctl start caddy
systemctl enable caddy
```

### Method 2: nginx + certbot

**Why nginx?**
- ✅ More control and customization
- ✅ Industry standard
- ✅ Extensive documentation
- ⚠️ Requires manual certificate renewal setup (though certbot handles it)

**Setup Process:**

```bash
# For Droplet 1
./scripts/setup-https-rpc-nginx.sh 143.110.139.166 rpc1.coinjecture.com admin@coinjecture.com

# For Droplet 2
./scripts/setup-https-rpc-nginx.sh 68.183.205.12 rpc2.coinjecture.com admin@coinjecture.com
```

**What the Script Does:**
1. Installs nginx and certbot
2. Creates nginx config with:
   - HTTP server (port 80) for Let's Encrypt validation
   - HTTPS server (port 443) with reverse proxy
   - CORS headers
   - OPTIONS preflight handling
3. Runs certbot to:
   - Obtain Let's Encrypt certificate
   - Update nginx config with certificate paths
   - Set up auto-renewal
4. Configures systemd timer for auto-renewal

**Certificate Renewal:**

Certbot sets up automatic renewal via systemd timer:
```bash
# Check renewal status
systemctl status certbot.timer

# Test renewal
certbot renew --dry-run
```

## Architecture

```
Browser (HTTPS)
  ↓
Domain (rpc1.coinjecture.com) with Let's Encrypt Certificate
  ↓
Reverse Proxy (Caddy/nginx on port 443)
  ↓
RPC Server (localhost:9933) with CORS enabled
  ↓
COINjecture Node
```

## Frontend Configuration

Once HTTPS is set up, update frontend `.env.production`:

```env
VITE_RPC_URL=https://rpc1.coinjecture.com,https://rpc2.coinjecture.com,https://rpc3.coinjecture.com
```

The RPC client will automatically:
- Use HTTPS URLs directly (no proxy needed)
- Handle CORS (enabled on RPC server)
- Support failover between nodes

## Testing

### 1. Test DNS Resolution

```bash
# Check if DNS is configured
dig rpc1.coinjecture.com +short
# Should return: 143.110.139.166
```

### 2. Test HTTPS Endpoint

```bash
# Test RPC call
curl https://rpc1.coinjecture.com \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"chain_getInfo","params":[],"id":1}'

# Test CORS
curl -X OPTIONS https://rpc1.coinjecture.com \
  -H "Origin: https://coinjecture.com" \
  -H "Access-Control-Request-Method: POST" \
  -v
```

### 3. Test from Browser

Open browser console and test:
```javascript
fetch('https://rpc1.coinjecture.com', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    jsonrpc: '2.0',
    method: 'chain_getInfo',
    params: [],
    id: 1
  })
}).then(r => r.json()).then(console.log)
```

## Benefits

1. **No Browser Warnings**: Let's Encrypt certificates are trusted by all browsers
2. **Free**: Let's Encrypt is free and unlimited
3. **Auto-Renewal**: Certificates automatically renew before expiration
4. **Production-Ready**: Suitable for public-facing services
5. **Professional**: Users see a secure connection (green padlock)
6. **SEO Friendly**: HTTPS is a ranking factor for search engines

## Troubleshooting

### Certificate Not Issued

**Problem**: Let's Encrypt can't validate domain ownership

**Solutions**:
- Verify DNS A record is correct: `dig rpc1.coinjecture.com`
- Ensure port 80 is open (required for HTTP-01 challenge)
- Wait for DNS propagation (can take up to 48 hours)
- Check firewall allows inbound port 80

### Certificate Expired

**Problem**: Certificate expired and not renewed

**Solutions**:
- **Caddy**: Restart service - `systemctl restart caddy`
- **nginx**: Run `certbot renew` manually, then reload nginx
- Check renewal logs: `journalctl -u certbot.timer`

### 502 Bad Gateway

**Problem**: Reverse proxy can't connect to RPC server

**Solutions**:
- Verify RPC server is running: `docker ps | grep coinject-node`
- Check RPC server is listening: `netstat -tlnp | grep 9933`
- Verify reverse proxy config points to `127.0.0.1:9933`

### CORS Errors

**Problem**: Browser blocks requests due to CORS

**Solutions**:
- Verify CORS headers in reverse proxy config
- Verify CORS is enabled in RPC server (v4.7.33+)
- Check browser console for specific CORS error

## Cost Considerations

- **Domain**: $10-15/year (one-time per domain, or use subdomains)
- **Let's Encrypt**: Free
- **Reverse Proxy**: Free (Caddy/nginx are open source)
- **Total**: ~$10-15/year for domain (if you don't already own one)

## Security Considerations

1. **Firewall**: Only expose ports 80 and 443, keep 9933 internal
2. **Rate Limiting**: Consider adding rate limiting to prevent abuse
3. **Access Control**: RPC endpoints are public - consider authentication for sensitive operations
4. **Certificate Security**: Let's Encrypt uses strong encryption (RSA 2048 or ECDSA)

## Comparison: Caddy vs nginx

| Feature | Caddy | nginx |
|---------|-------|-------|
| Auto-HTTPS | ✅ Automatic | ⚠️ Requires certbot |
| Configuration | ✅ Simple | ⚠️ More complex |
| Performance | ✅ Excellent | ✅ Excellent |
| CORS Support | ✅ Built-in | ⚠️ Manual config |
| Learning Curve | ✅ Easy | ⚠️ Steeper |
| Industry Adoption | ⚠️ Newer | ✅ Very common |

**Recommendation**: Use **Caddy** for simplicity, or **nginx** if you need more control.

## Next Steps

1. **Register domains** (or use subdomains)
2. **Configure DNS A records** pointing to node IPs
3. **Wait for DNS propagation** (check with `dig`)
4. **Run setup script** (Caddy or nginx)
5. **Test HTTPS endpoint**
6. **Update frontend** `.env.production` with HTTPS URLs
7. **Deploy frontend** and verify direct RPC access works

## Example: Complete Setup

```bash
# 1. DNS configured (rpc1.coinjecture.com → 143.110.139.166)
# 2. Verify DNS
dig rpc1.coinjecture.com +short
# Output: 143.110.139.166

# 3. Setup HTTPS with Caddy
./scripts/setup-https-rpc-caddy.sh 143.110.139.166 rpc1.coinjecture.com

# 4. Test
curl https://rpc1.coinjecture.com \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"chain_getInfo","params":[],"id":1}'

# 5. Update frontend .env.production
echo "VITE_RPC_URL=https://rpc1.coinjecture.com,https://rpc2.coinjecture.com" > web/coinjecture-evolved-main/.env.production

# 6. Rebuild and deploy frontend
cd web/coinjecture-evolved-main
npm run build
# Deploy dist/ to CloudFront
```

## Summary

**Domain + Let's Encrypt is the recommended approach because:**
- ✅ No browser security warnings
- ✅ Free SSL certificates
- ✅ Automatic renewal
- ✅ Production-ready
- ✅ Professional appearance
- ✅ Works with all browsers

**Requirements:**
- Domain names (subdomains work fine)
- DNS A records pointing to node IPs
- Ports 80 and 443 open
- Reverse proxy (Caddy or nginx)

**Time to Setup:** ~15-30 minutes per node (after DNS is configured)



