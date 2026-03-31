#!/bin/bash
# Setup HTTPS for RPC endpoints using nginx and Let's Encrypt
# Usage: ./setup-https-rpc-nginx.sh <node_ip> <domain> <email>
# Example: ./setup-https-rpc-nginx.sh 143.110.139.166 rpc1.coinjecture.com admin@coinjecture.com

set -e

NODE_IP=$1
DOMAIN=$2
EMAIL=${3:-admin@coinjecture.com}

if [ -z "$NODE_IP" ] || [ -z "$DOMAIN" ]; then
    echo "Usage: $0 <node_ip> <domain> [email]"
    echo "Example: $0 143.110.139.166 rpc1.coinjecture.com admin@coinjecture.com"
    exit 1
fi

echo "🔒 Setting up HTTPS for RPC endpoint: $DOMAIN → $NODE_IP:9933"
echo ""

ssh -i ~/.ssh/COINjecture-Key root@$NODE_IP "bash -s" << ENDSSH
set -e

# Install nginx and certbot
if ! command -v nginx &> /dev/null; then
    echo "📦 Installing nginx and certbot..."
    apt-get update
    apt-get install -y nginx certbot python3-certbot-nginx
fi

# Create nginx config for RPC endpoint
cat > /etc/nginx/sites-available/rpc-https << NGINXCONF
server {
    listen 80;
    server_name $DOMAIN;
    
    # Temporary config for Let's Encrypt validation
    location /.well-known/acme-challenge/ {
        root /var/www/html;
    }
    
    location / {
        return 301 https://\$server_name\$request_uri;
    }
}

server {
    listen 443 ssl http2;
    server_name $DOMAIN;

    # SSL configuration (will be updated by certbot)
    ssl_certificate /etc/letsencrypt/live/$DOMAIN/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/$DOMAIN/privkey.pem;
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
NGINXCONF

# Enable site
ln -sf /etc/nginx/sites-available/rpc-https /etc/nginx/sites-enabled/
rm -f /etc/nginx/sites-enabled/default

# Test nginx config
nginx -t

# Start nginx
systemctl start nginx
systemctl enable nginx

# Get SSL certificate
echo "📜 Obtaining SSL certificate from Let's Encrypt..."
certbot --nginx -d $DOMAIN --non-interactive --agree-tos --email $EMAIL --redirect

# Reload nginx
systemctl reload nginx

echo "✅ HTTPS configured for $DOMAIN"
echo "🔗 RPC endpoint: https://$DOMAIN"
ENDSSH

echo ""
echo "✅ HTTPS setup complete!"
echo "🔗 Test: curl https://$DOMAIN -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"method\":\"chain_getInfo\",\"params\":[],\"id\":1}'"

