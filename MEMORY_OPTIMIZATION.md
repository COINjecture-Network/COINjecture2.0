# Memory Optimization for 1GB Droplets

## Problem
Node 1 exceeded memory on a 1GB droplet, causing OOM (Out of Memory) kills and unresponsiveness.

## Solution

### Memory Limits Applied
- **Container Memory Limit**: 800MB
- **Memory Swap**: 800MB (no swap, prevents thrashing)
- **System Reserve**: 200MB (for OS, Docker daemon, etc.)

### Rationale
1GB droplet specs:
- Total RAM: 1GB
- Disk: 25GB
- Location: SFO2 (San Francisco)

With 800MB limit:
- Prevents OOM kills
- Leaves 200MB for system processes
- No swap to avoid disk I/O thrashing

### Deployment Script Updates
Updated `build-and-deploy.sh` to include:
```bash
--memory=800m \
--memory-swap=800m \
```

### Recommendations

#### Short Term
1. ✅ Memory limits applied (800MB per container)
2. Monitor memory usage: `docker stats coinject-node`
3. Restart Node 1 with new limits

#### Long Term Options

**Option 1: Upgrade Droplet (Recommended)**
- Upgrade to 2GB RAM droplet ($12/month)
- Allows 1.5GB container limit
- More headroom for mining operations

**Option 2: Optimize Code**
- Reduce memory footprint in mining operations
- Implement memory-efficient block storage
- Add memory monitoring and alerts

**Option 3: Separate Mining Node**
- Run mining on larger instance (2GB+)
- Keep full nodes on 1GB droplets
- Archive node on larger instance

### Monitoring

Check memory usage:
```bash
# On droplet
docker stats coinject-node --no-stream
free -h

# Check for OOM kills
dmesg | grep -i "out of memory"
journalctl -k | grep -i "oom"
```

### Current Status
- ✅ Deployment script updated with 800MB limits
- ⏳ Node 1 needs restart with new limits
- ⏳ Node 2 already has limits (if redeployed)

