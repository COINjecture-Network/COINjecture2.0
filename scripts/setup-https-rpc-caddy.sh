#!/bin/bash
# Setup HTTPS for RPC endpoints using Caddy (simpler, auto-HTTPS)
# Usage: ./setup-https-rpc-caddy.sh <node_ip> <domain>
# Example: ./setup-https-rpc-caddy.sh 143.110.139.166 rpc1.coinjecture.com

set -e

NODE_IP=$1
DOMAIN=$2

if [ -z "$NODE_IP" ] || [ -z "$DOMAIN" ]; then
    echo "Usage: $0 <node_ip> <domain>"
    echo "Example: $0 143.110.139.166 rpc1.coinjecture.com"
    exit 1
fi

echo "🔒 Setting up HTTPS for RPC endpoint: $DOMAIN → $NODE_IP:9933"
echo ""

ssh -i ~/.ssh/COINjecture-Key root@$NODE_IP "bash -s" << ENDSSH
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
$DOMAIN {
    reverse_proxy 127.0.0.1:9933 {
        header_up Host {host}
        header_up X-Real-IP {remote}
        header_up X-Forwarded-For {remote_host}
        header_up X-Forwarded-Proto {scheme}
    }
    
    # CORS headers
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
CADDYFILE

# Test Caddyfile
caddy validate --config /etc/caddy/Caddyfile

# Reload Caddy
systemctl reload caddy || systemctl start caddy
systemctl enable caddy

echo "✅ Caddy configured for $DOMAIN"
echo "🔗 RPC endpoint: https://$DOMAIN"
echo ""
echo "📋 Note: Ensure DNS A record points $DOMAIN to this server's IP"
ENDSSH

echo ""
echo "✅ HTTPS setup complete!"
echo "🔗 Test: curl https://$DOMAIN -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"method\":\"chain_getInfo\",\"params\":[],\"id\":1}'"

