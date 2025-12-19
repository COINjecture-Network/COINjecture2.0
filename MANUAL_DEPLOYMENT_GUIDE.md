# Manual Deployment Guide - Bug Fix Docker Image

## Current Status
- **Bug Fix Image**: `coinject-node:debug-fork-detection` (built locally)
- **Target Droplets**: 
  - Node 1: 143.110.139.166
  - Node 2: 68.183.205.12
- **Issue**: SSH connections timing out due to high system load

## Option 1: Deploy via DigitalOcean Console (Recommended)

### Step 1: Access Droplet Console
1. Go to https://cloud.digitalocean.com/droplets
2. Click on each droplet (143.110.139.166 and 68.183.205.12)
3. Click "Access" → "Launch Droplet Console"
4. This provides browser-based terminal access (bypasses SSH timeout)

### Step 2: Transfer Docker Image

On your local machine, save the image:
```bash
cd "/Users/sarahmarin/Downloads/COINjecture1337-NETB-main 5"
docker save coinject-node:debug-fork-detection -o /tmp/coinject-node.tar
```

Then upload to a file sharing service or use DigitalOcean Spaces:
```bash
# Option A: Upload to temporary file sharing (e.g., transfer.sh)
curl --upload-file /tmp/coinject-node.tar https://transfer.sh/coinject-node.tar

# Option B: Use DigitalOcean Spaces (if configured)
# doctl spaces upload coinject-node.tar s3://your-space/coinject-node.tar
```

### Step 3: On Each Droplet (via Console)

```bash
# Download the image (replace URL with actual upload location)
cd /tmp
# If using transfer.sh:
curl -L https://transfer.sh/XXXXX/coinject-node.tar -o coinject-node.tar

# Load the image
docker load -i /tmp/coinject-node.tar

# Stop existing container
docker stop coinject-node 2>/dev/null || true
docker rm coinject-node 2>/dev/null || true

# Free up ports
for port in 30333 9933 9090; do
    lsof -ti:$port | xargs kill -9 2>/dev/null || true
    fuser -k $port/tcp 2>/dev/null || true
done
pkill -9 coinject 2>/dev/null || true
sleep 2

# Start new container with bug fix image
# For Node 1 (143.110.139.166):
docker run -d \
  --name coinject-node \
  --restart unless-stopped \
  -p 30333:30333 \
  -p 9933:9933 \
  -p 9090:9090 \
  -v coinject-data:/data \
  -e DATA_DIR=/data \
  coinject-node:debug-fork-detection \
  --data-dir /data \
  --p2p-addr /ip4/0.0.0.0/tcp/30333 \
  --rpc-addr 0.0.0.0:9933 \
  --metrics-addr 0.0.0.0:9090 \
  --mine \
  --hf-token "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" \
  --hf-dataset-name "COINjecture/NP_Solutions_v3" \
  --bootnodes "/ip4/68.183.205.12/tcp/30333/p2p/12D3KooWLnKgJxYo1pxMCXxfJsGt1o9j5iExBepUmaMV4NtvWAWQ"

# For Node 2 (68.183.205.12):
docker run -d \
  --name coinject-node \
  --restart unless-stopped \
  -p 30333:30333 \
  -p 9933:9933 \
  -p 9090:9090 \
  -v coinject-data:/data \
  -e DATA_DIR=/data \
  coinject-node:debug-fork-detection \
  --data-dir /data \
  --p2p-addr /ip4/0.0.0.0/tcp/30333 \
  --rpc-addr 0.0.0.0:9933 \
  --metrics-addr 0.0.0.0:9090 \
  --mine \
  --hf-token "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" \
  --hf-dataset-name "COINjecture/NP_Solutions_v3" \
  --bootnodes "/ip4/143.110.139.166/tcp/30333/p2p/12D3KooWJp1LjiE8sLN4kN9JpqZJWGvrMZR2eH4x2Trs3AdYdPwx"

# Verify deployment
docker ps | grep coinject-node
docker logs --tail 20 coinject-node
```

## Option 2: Wait and Retry Automated Deployment

Once SSH access is restored (system load decreases), run:

```bash
cd "/Users/sarahmarin/Downloads/COINjecture1337-NETB-main 5"
./deploy-docker.sh
```

The script has been updated with longer SSH timeouts (30-60 seconds) to handle high system load.

## Option 3: Build Image on Droplet

If you can access the droplet console, you can build the image directly:

```bash
# On droplet, clone or upload source code
# Then build:
docker build -f Dockerfile.adzdb -t coinject-node:debug-fork-detection .

# Then follow Step 3 above to deploy
```

## Verification

After deployment, verify the nodes are running:

```bash
# Check container status
docker ps | grep coinject-node

# Check logs
docker logs --tail 50 coinject-node

# Check RPC endpoint
curl http://143.110.139.166:9933 -X POST -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"network_getInfo","id":1}'
```

## Troubleshooting

If deployment fails:
1. Check system load: `uptime` or `top`
2. Check Docker: `docker ps -a`
3. Check ports: `netstat -tulpn | grep -E '30333|9933|9090'`
4. Check logs: `docker logs coinject-node`


