#!/bin/bash
# Setup HTTPS for all COINjecture RPC endpoints using coinjecture.com subdomains
# This script sets up HTTPS for all three nodes

set -e

echo "🔒 Setting up HTTPS for coinjecture.com RPC endpoints"
echo ""
echo "This will configure:"
echo "  - rpc1.coinjecture.com → Droplet 1 (143.110.139.166)"
echo "  - rpc2.coinjecture.com → Droplet 2 (68.183.205.12)"
echo "  - rpc3.coinjecture.com → GCE VM (35.184.253.150)"
echo ""
echo "⚠️  IMPORTANT: Ensure DNS A records are configured first!"
echo "   Check with: dig rpc1.coinjecture.com +short"
echo ""
read -p "Continue? (y/n) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    exit 1
fi

# Check DNS first
echo ""
echo "=== Checking DNS Configuration ==="
RPC1_IP=$(dig +short rpc1.coinjecture.com | tail -1)
RPC2_IP=$(dig +short rpc2.coinjecture.com | tail -1)
RPC3_IP=$(dig +short rpc3.coinjecture.com | tail -1)

if [ "$RPC1_IP" != "143.110.139.166" ]; then
    echo "⚠️  WARNING: rpc1.coinjecture.com resolves to $RPC1_IP (expected 143.110.139.166)"
    echo "   Please configure DNS A record first"
    exit 1
fi

if [ "$RPC2_IP" != "68.183.205.12" ]; then
    echo "⚠️  WARNING: rpc2.coinjecture.com resolves to $RPC2_IP (expected 68.183.205.12)"
    echo "   Please configure DNS A record first"
    exit 1
fi

if [ "$RPC3_IP" != "35.184.253.150" ]; then
    echo "⚠️  WARNING: rpc3.coinjecture.com resolves to $RPC3_IP (expected 35.184.253.150)"
    echo "   Please configure DNS A record first"
    exit 1
fi

echo "✅ DNS configured correctly"
echo ""

# Setup Droplet 1
echo "=== Setting up Droplet 1 (rpc1.coinjecture.com) ==="
./scripts/setup-https-rpc-caddy.sh 143.110.139.166 rpc1.coinjecture.com
echo ""

# Setup Droplet 2
echo "=== Setting up Droplet 2 (rpc2.coinjecture.com) ==="
./scripts/setup-https-rpc-caddy.sh 68.183.205.12 rpc2.coinjecture.com
echo ""

# Setup GCE VM
echo "=== Setting up GCE VM (rpc3.coinjecture.com) ==="
GCE_VM="coinject-node"
ZONE="us-central1-a"
gcloud compute ssh $GCE_VM --zone=$ZONE --command="bash -s" << 'ENDSSH'
set -e

# Install Caddy
if ! command -v caddy &> /dev/null; then
    echo "📦 Installing Caddy..."
    apt-get update
    apt-get install -y debian-keyring debian-archive-keyring apt-transport-https
    curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' | gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
    curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' | tee /etc/apt/sources.list.d/caddy-stable.list
    apt-get update
    apt-get install -y caddy
fi

# Create Caddyfile
cat > /etc/caddy/Caddyfile << CADDYFILE
rpc3.coinjecture.com {
    reverse_proxy 127.0.0.1:9933 {
        header_up Host {host}
        header_up X-Real-IP {remote}
        header_up X-Forwarded-For {remote_host}
        header_up X-Forwarded-Proto {scheme}
    }
    
    header {
        Access-Control-Allow-Origin "*"
        Access-Control-Allow-Methods "POST, GET, OPTIONS"
        Access-Control-Allow-Headers "Content-Type"
    }
    
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
CADDYFILE

# Validate and start
caddy validate --config /etc/caddy/Caddyfile
systemctl reload caddy || systemctl start caddy
systemctl enable caddy

echo "✅ Caddy configured for rpc3.coinjecture.com"
ENDSSH

echo ""
echo "=== Testing HTTPS Endpoints ==="
echo ""
echo "Testing rpc1.coinjecture.com..."
curl -s https://rpc1.coinjecture.com \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"chain_getInfo","params":[],"id":1}' | python3 -c "import sys, json; d=json.load(sys.stdin); print('✅ Working' if 'result' in d else '❌ Error')" 2>&1 || echo "⚠️  Not ready yet (DNS may still be propagating)"

echo ""
echo "Testing rpc2.coinjecture.com..."
curl -s https://rpc2.coinjecture.com \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"chain_getInfo","params":[],"id":1}' | python3 -c "import sys, json; d=json.load(sys.stdin); print('✅ Working' if 'result' in d else '❌ Error')" 2>&1 || echo "⚠️  Not ready yet (DNS may still be propagating)"

echo ""
echo "Testing rpc3.coinjecture.com..."
curl -s https://rpc3.coinjecture.com \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"chain_getInfo","params":[],"id":1}' | python3 -c "import sys, json; d=json.load(sys.stdin); print('✅ Working' if 'result' in d else '❌ Error')" 2>&1 || echo "⚠️  Not ready yet (DNS may still be propagating)"

echo ""
echo "=== Setup Complete ==="
echo ""
echo "✅ HTTPS configured for all RPC endpoints"
echo ""
echo "📋 Frontend Configuration:"
echo "   Update .env.production with:"
echo "   VITE_RPC_URL=https://rpc1.coinjecture.com,https://rpc2.coinjecture.com,https://rpc3.coinjecture.com"
echo ""
echo "🔗 RPC Endpoints:"
echo "   - https://rpc1.coinjecture.com"
echo "   - https://rpc2.coinjecture.com"
echo "   - https://rpc3.coinjecture.com"



