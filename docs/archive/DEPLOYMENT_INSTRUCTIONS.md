# Deployment Instructions for Fork Detection Debugging

This guide will help you rebuild the Docker image with fork detection instrumentation and deploy to your droplets.

## Step 1: Rebuild Docker Image

### Option A: Build Locally and Push
```bash
# Build the Docker image
docker build -t coinject-node:debug .

# Tag for your registry (if using one)
# docker tag coinject-node:debug your-registry/coinject-node:debug
# docker push your-registry/coinject-node:debug
```

### Option B: Build on Remote Droplet (Recommended)
The `deploy-v5-droplet.ps1` script builds on the remote server, which is faster and avoids cross-compilation issues.

## Step 2: Deploy to Droplets

### Using PowerShell Script (Windows)
```powershell
.\deploy-v5-droplet.ps1 -DropletIP "143.110.139.166" -P2PPort 30333 -RPCPort 9933
```

### Using Bash Script (Linux/Mac)
```bash
./deploy-docker.sh
```

**Note:** Both scripts have been updated to include `DATA_DIR=/data` environment variable, which ensures debug logs are written to `/data/debug.log`.

## Step 3: Run Nodes and Reproduce Issue

After deployment, the nodes will start automatically. Monitor them for false positive fork detection:

```bash
# View logs on Node 1
ssh root@143.110.139.166 "docker logs -f coinject-v5"

# View logs on Node 2  
ssh root@68.183.205.12 "docker logs -f coinject-v5"
```

Look for messages like:
- `⚠️  Fork detected! Peer is ahead...`
- `🔍 Fork detection Indicator 1...`
- `🔍 Fork detection Indicator 2...`

## Step 4: Retrieve Debug Log Files

Once you've reproduced the false positive fork detection, retrieve the debug logs:

```bash
# From Node 1
scp root@143.110.139.166:/var/lib/docker/volumes/coinject-v5-data/_data/debug.log ./node1-debug.log

# From Node 2
scp root@68.183.205.12:/var/lib/docker/volumes/coinject-v5-data/_data/debug.log ./node2-debug.log
```

**Alternative:** If the log is in the container's /data directory:
```bash
# Copy from container
ssh root@143.110.139.166 "docker cp coinject-v5:/data/debug.log ./node1-debug.log"
scp root@143.110.139.166:./node1-debug.log ./

ssh root@68.183.205.12 "docker cp coinject-v5:/data/debug.log ./node2-debug.log"
scp root@68.183.205.12:./node2-debug.log ./
```

## Step 5: Analyze Logs

The debug logs are in NDJSON format (one JSON object per line). Each entry contains:
- `hypothesisId`: Which hypothesis this log relates to (A, B, C, D, E, F, G)
- `message`: What event occurred
- `data`: Detailed information about the event

Key log messages to look for:
- **Hypothesis G**: Fork detection indicators (Indicator 1 and Indicator 2)
- **Hypothesis A**: Block validation failures (prev_hash mismatches)
- **Hypothesis B**: Common ancestor search results
- **Hypothesis C**: Fork detection in StatusUpdate handler

## Quick Commands Reference

```bash
# Check if debug log exists
ssh root@143.110.139.166 "docker exec coinject-v5 ls -lh /data/debug.log"

# View last 50 lines of debug log
ssh root@143.110.139.166 "docker exec coinject-v5 tail -50 /data/debug.log"

# Monitor debug log in real-time
ssh root@143.110.139.166 "docker exec coinject-v5 tail -f /data/debug.log"

# Get log file size
ssh root@143.110.139.166 "docker exec coinject-v5 stat -c%s /data/debug.log"
```

## Troubleshooting

If the debug log file is not being created:
1. Check that `DATA_DIR=/data` is set in the container environment
2. Verify the container has write permissions to `/data`
3. Check container logs for any errors: `docker logs coinject-v5`

If you need to manually set the log path:
```bash
# Set custom log path via environment variable
docker run ... -e DEBUG_LOG_PATH=/data/debug.log ...
```


