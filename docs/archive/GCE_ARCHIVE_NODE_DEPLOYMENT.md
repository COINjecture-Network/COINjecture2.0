# GCE Archive Node Deployment

## Configuration Summary

The GCE VM is now configured to run as an **Archive Node** with the following settings:

### Archive Node Characteristics:
- **Full History Storage**: Stores complete blockchain history (unlimited blocks)
- **No Mining**: Does not produce blocks (`--mine` flag removed)
- **Full Sync**: Can perform complete chain synchronization
- **Block Serving**: Can serve blocks and headers to other nodes
- **High Capacity**: Supports up to 100 inbound peers and 50 outbound peers

### Deployment Configuration:

```bash
--node-type archive          # Archive node mode
--data-dir /data            # Data directory
--p2p-addr /ip4/0.0.0.0/tcp/30333
--rpc-addr 0.0.0.0:9933
--metrics-addr 0.0.0.0:9090
--hf-token <token>
--hf-dataset-name COINjecture/v5
--bootnodes <droplet1> <droplet2>
```

### Current Issue:
SSH connection to GCE VM is timing out. This is likely a firewall/network configuration issue.

## Manual Deployment Options

### Option 1: Fix Firewall Rules
```bash
# Allow SSH from your IP
gcloud compute firewall-rules create allow-ssh-coinject \
  --allow tcp:22 \
  --source-ranges <YOUR_IP>/32 \
  --target-tags coinject-node

# Or allow SSH from anywhere (less secure)
gcloud compute firewall-rules create allow-ssh-coinject \
  --allow tcp:22 \
  --source-ranges 0.0.0.0/0 \
  --target-tags coinject-node
```

### Option 2: Use GCE Console
1. Go to GCE Console: https://console.cloud.google.com/compute/instances
2. Click on `coinject-node` VM
3. Click "SSH" button (opens browser-based SSH)
4. Run the deployment commands manually

### Option 3: Deploy via Startup Script
Create a startup script that automatically deploys the archive node on VM boot.

## Manual Deployment Commands

Once SSH access is available, run these commands on the GCE VM:

```bash
# Pull latest image
docker pull gcr.io/coinjecture/coinject-node:v4.7.48-amd64

# Stop existing container
docker stop coinject-node 2>/dev/null || true
docker rm coinject-node 2>/dev/null || true

# Start Archive Node
docker run -d \
  --name coinject-node \
  --restart unless-stopped \
  -p 30333:30333 \
  -p 9933:9933 \
  -p 9090:9090 \
  -v coinject-data:/data \
  gcr.io/coinjecture/coinject-node:v4.7.48-amd64 \
  --data-dir /data \
  --node-type archive \
  --p2p-addr /ip4/0.0.0.0/tcp/30333 \
  --rpc-addr 0.0.0.0:9933 \
  --metrics-addr 0.0.0.0:9090 \
  --hf-token "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" \
  --hf-dataset-name "COINjecture/v5" \
  --bootnodes "/ip4/143.110.139.166/tcp/30333/p2p/12D3KooWL3Q7KmTocqNGLfyz4X4mhyyPD8b4zx6MBk1qnDAT8FYs" \
  --bootnodes "/ip4/68.183.205.12/tcp/30333/p2p/12D3KooWQwpXp7NJG9gMVJMFH7oBfYQizbtPAB3RfRqxyvQ5WZfv"
```

## Verification

After deployment, verify the archive node is running:

```bash
# Check container status
docker ps | grep coinject-node

# Check logs
docker logs -f coinject-node

# Check RPC
curl -X POST -H 'Content-Type: application/json' \
  --data '{"jsonrpc":"2.0","method":"chain_getInfo","params":[],"id":1}' \
  http://35.184.253.150:9933
```

## Archive Node Benefits

1. **Complete History**: Stores all blocks from genesis
2. **High Availability**: Can serve historical data to other nodes
3. **No Mining Overhead**: Focuses resources on storage and serving
4. **Network Support**: Can handle many peer connections
5. **Data Preservation**: Critical for network resilience and historical queries

