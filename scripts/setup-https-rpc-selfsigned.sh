#!/bin/bash
# Setup HTTPS for RPC endpoints using self-signed certificates (no domain needed)
# Usage: ./setup-https-rpc-selfsigned.sh <node_ip>
# Example: ./setup-https-rpc-selfsigned.sh 143.110.139.166
#
# WARNING: Self-signed certificates will show browser security warnings.
# Users must manually accept the certificate. Not recommended for production.

set -e

NODE_IP=$1

if [ -z "$NODE_IP" ]; then
    echo "Usage: $0 <node_ip>"
    echo "Example: $0 143.110.139.166"
    exit 1
fi

echo "🔒 Setting up HTTPS with self-signed certificate for: $NODE_IP"
echo "⚠️  WARNING: Browsers will show security warnings for self-signed certificates"
echo ""

ssh -i ~/.ssh/COINjecture-Key root@$NODE_IP "bash -s" << ENDSSH
set -e

# Install nginx
if ! command -v nginx &> /dev/null; then
    echo "📦 Installing nginx..."
    apt-get update
    apt-get install -y nginx openssl
fi

# Create SSL directory
mkdir -p /etc/nginx/ssl

# Generate self-signed certificate
if [ ! -f /etc/nginx/ssl/rpc.crt ]; then
    echo "📜 Generating self-signed certificate..."
    openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
        -keyout /etc/nginx/ssl/rpc.key \
        -out /etc/nginx/ssl/rpc.crt \
        -subj "/C=US/ST=State/L=City/O=COINjecture/CN=$NODE_IP" \
        -addext "subjectAltName=IP:$NODE_IP"
fi

# Create nginx config
cat > /etc/nginx/sites-available/rpc-https-selfsigned << NGINXCONF
server {
    listen 443 ssl http2;
    server_name $NODE_IP;

    # Self-signed SSL certificate
    ssl_certificate /etc/nginx/ssl/rpc.crt;
    ssl_certificate_key /etc/nginx/ssl/rpc.key;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;

    # CORS headers
    add_header 'Access-Control-Allow-Origin' '*' always;
    add_header 'Access-Control-Allow-Methods' 'POST, GET, OPTIONS' always;
    add_header 'Access-Control-Allow-Headers' 'Content-Type' always;

    # Proxy to RPC server
    location / {
        proxy_pass http://127.0.0.1:9933;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
        
        # CORS preflight
        if (\$request_method = 'OPTIONS') {
            add_header 'Access-Control-Allow-Origin' '*';
            add_header 'Access-Control-Allow-Methods' 'POST, GET, OPTIONS';
            add_header 'Access-Control-Allow-Headers' 'Content-Type';
            add_header 'Access-Control-Max-Age' 86400;
            add_header 'Content-Type' 'text/plain';
            add_header 'Content-Length' 0;
            return 204;
        }
    }
}

# Redirect HTTP to HTTPS
server {
    listen 80;
    server_name $NODE_IP;
    return 301 https://\$server_name\$request_uri;
}
NGINXCONF

# Enable site
ln -sf /etc/nginx/sites-available/rpc-https-selfsigned /etc/nginx/sites-enabled/
rm -f /etc/nginx/sites-enabled/default

# Test nginx config
nginx -t

# Start/reload nginx
systemctl start nginx
systemctl enable nginx
systemctl reload nginx

echo "✅ HTTPS configured with self-signed certificate"
echo "🔗 RPC endpoint: https://$NODE_IP"
echo ""
echo "⚠️  IMPORTANT: Browsers will show 'Not Secure' warnings"
echo "   Users must click 'Advanced' → 'Proceed to site' to accept certificate"
ENDSSH

echo ""
echo "✅ HTTPS setup complete (self-signed)"
echo "⚠️  Note: Browsers will show security warnings"
echo "🔗 Test: curl -k https://$NODE_IP -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"method\":\"chain_getInfo\",\"params\":[],\"id\":1}'"


