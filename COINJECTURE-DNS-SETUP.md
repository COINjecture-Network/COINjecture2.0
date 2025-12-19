# DNS Setup for coinjecture.com RPC Endpoints

This guide shows exactly how to configure DNS records for your `coinjecture.com` domain to enable HTTPS RPC access.

## DNS Records to Add

Add these **A records** in your DNS provider (wherever `coinjecture.com` is managed):

### Record 1: rpc1.coinjecture.com

```
Type: A
Name: rpc1
Value: 143.110.139.166
TTL: 300 (or Auto)
```

**Full domain:** `rpc1.coinjecture.com` → `143.110.139.166`

### Record 2: rpc2.coinjecture.com

```
Type: A
Name: rpc2
Value: 68.183.205.12
TTL: 300 (or Auto)
```

**Full domain:** `rpc2.coinjecture.com` → `68.183.205.12`

### Record 3: rpc3.coinjecture.com

```
Type: A
Name: rpc3
Value: 35.184.253.150
TTL: 300 (or Auto)
```

**Full domain:** `rpc3.coinjecture.com` → `35.184.253.150`

## Common DNS Providers

### Cloudflare
1. Go to DNS → Records
2. Click "Add record"
3. Type: A
4. Name: `rpc1` (or `rpc2`, `rpc3`)
5. IPv4 address: `143.110.139.166` (or corresponding IP)
6. Proxy status: DNS only (gray cloud)
7. TTL: Auto
8. Save

### AWS Route 53
1. Go to Hosted zones → coinjecture.com
2. Click "Create record"
3. Record name: `rpc1` (or `rpc2`, `rpc3`)
4. Record type: A
5. Value: `143.110.139.166` (or corresponding IP)
6. TTL: 300
7. Create record

### Google Domains / Namecheap / GoDaddy
1. Go to DNS Management
2. Add A record:
   - Host: `rpc1` (or `rpc2`, `rpc3`)
   - Type: A
   - Value: `143.110.139.166` (or corresponding IP)
   - TTL: 300
3. Save

## Verify DNS Configuration

After adding records, verify with:

```bash
# Check each subdomain
dig rpc1.coinjecture.com +short
# Expected: 143.110.139.166

dig rpc2.coinjecture.com +short
# Expected: 68.183.205.12

dig rpc3.coinjecture.com +short
# Expected: 35.184.253.150
```

**DNS Propagation:** Changes typically take 5 minutes to 1 hour, but can take up to 48 hours.

## After DNS is Configured

Once DNS records are active, run:

```bash
./scripts/setup-https-coinjecture.sh
```

This will automatically set up HTTPS for all three RPC endpoints using Caddy.

## Alternative: Manual Setup

If you prefer to set up each node individually:

```bash
# Droplet 1
./scripts/setup-https-rpc-caddy.sh 143.110.139.166 rpc1.coinjecture.com

# Droplet 2
./scripts/setup-https-rpc-caddy.sh 68.183.205.12 rpc2.coinjecture.com

# GCE VM
gcloud compute ssh coinject-node --zone=us-central1-a
# Then follow Caddy setup instructions
```

## Frontend Configuration

After HTTPS is set up, update `web/coinjecture-evolved-main/.env.production`:

```env
VITE_RPC_URL=https://rpc1.coinjecture.com,https://rpc2.coinjecture.com,https://rpc3.coinjecture.com
```

## Testing

Once DNS and HTTPS are configured:

```bash
# Test each endpoint
curl https://rpc1.coinjecture.com \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"chain_getInfo","params":[],"id":1}'
```

You should get a JSON response with chain information (no certificate warnings).



