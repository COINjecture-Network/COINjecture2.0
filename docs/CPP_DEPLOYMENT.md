# COINjecture CPP Network Deployment Guide

## Overview

This guide covers deploying the COINjecture P2P Protocol (CPP) network to DigitalOcean droplets.

## Architecture

- **Bootnode**: Primary node that other nodes connect to
  - P2P Port: 707 (CPP protocol)
  - WebSocket Port: 8080 (Light client mining)
  - RPC Port: 9933 (JSON-RPC)
  - Metrics Port: 9090 (Prometheus)

- **Node 2**: Secondary node connecting to bootnode
  - P2P Port: 7071 (CPP protocol)
  - WebSocket Port: 8081 (Light client mining)
  - RPC Port: 9934 (JSON-RPC)
  - Metrics Port: 9091 (Prometheus)

## Prerequisites

1. Two DigitalOcean droplets with:
   - Ubuntu 22.04 LTS
   - At least 2GB RAM
   - Root SSH access
   - Firewall configured to allow:
     - Port 707 (bootnode P2P)
     - Port 8080 (bootnode WebSocket)
     - Port 9933 (bootnode RPC)
     - Port 9090 (bootnode metrics)
     - Port 7071 (Node 2 P2P)
     - Port 8081 (Node 2 WebSocket)
     - Port 9934 (Node 2 RPC)
     - Port 9091 (Node 2 metrics)

2. Local machine with:
   - Rust toolchain installed
   - SSH access to droplets
   - `cargo` and `rustc` in PATH

## Local Testing

Before deploying to production, test locally:

```bash
# Run local test script
./scripts/test-local-cpp.sh
```

This will:
1. Build the release binary
2. Start bootnode on ports 707 (P2P) and 8080 (WebSocket)
3. Start Node 2 on ports 7071 (P2P) and 8081 (WebSocket)
4. Test two-node synchronization
5. Test WebSocket connectivity

The script will keep both nodes running until you press Ctrl+C.

### Manual Local Testing

```bash
# Terminal 1: Bootnode
./target/release/coinject-node \
    --data-dir ./data/bootnode \
    --node-type full \
    --mine \
    --p2p-addr "0.0.0.0:707" \
    --rpc-addr "127.0.0.1:9933" \
    --metrics-addr "127.0.0.1:9090" \
    --miner-address "0000000000000000000000000000000000000000000000000000000000000001"

# Terminal 2: Node 2
./target/release/coinject-node \
    --data-dir ./data/node2 \
    --node-type full \
    --mine \
    --p2p-addr "0.0.0.0:7071" \
    --rpc-addr "127.0.0.1:9934" \
    --metrics-addr "127.0.0.1:9091" \
    --bootnodes "127.0.0.1:707" \
    --miner-address "0000000000000000000000000000000000000000000000000000000000000002"
```

## DigitalOcean Deployment

### Step 1: Configure Environment Variables

```bash
export BOOTNODE_IP="143.110.139.166"  # Replace with your bootnode IP
export NODE2_IP="143.110.139.167"      # Replace with your Node 2 IP
export SSH_USER="root"                 # Or your SSH user
export SSH_KEY="~/.ssh/id_rsa"         # Path to your SSH key
```

### Step 2: Deploy

```bash
./scripts/deploy-cpp-droplets.sh
```

This script will:
1. Build the release binary locally
2. Create deployment package
3. Deploy to bootnode droplet
4. Deploy to Node 2 droplet
5. Install systemd services
6. Start both nodes

### Step 3: Verify Deployment

Check bootnode status:
```bash
ssh root@$BOOTNODE_IP 'systemctl status coinject-bootnode'
```

Check Node 2 status:
```bash
ssh root@$NODE2_IP 'systemctl status coinject-node2'
```

Monitor logs:
```bash
# Bootnode logs
ssh root@$BOOTNODE_IP 'journalctl -u coinject-bootnode -f'

# Node 2 logs
ssh root@$NODE2_IP 'journalctl -u coinject-node2 -f'
```

## Testing Deployment

### Test P2P Connection

```bash
# From Node 2, check if it connected to bootnode
ssh root@$NODE2_IP 'journalctl -u coinject-node2 | grep -i "peer\|connect"'
```

### Test WebSocket Connection

```bash
# Test bootnode WebSocket
curl -i -N \
    -H "Connection: Upgrade" \
    -H "Upgrade: websocket" \
    -H "Sec-WebSocket-Version: 13" \
    -H "Sec-WebSocket-Key: test" \
    "http://$BOOTNODE_IP:8080"
```

### Test RPC

```bash
# Get bootnode chain height
curl -X POST \
    -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","method":"chain_getHeight","params":[],"id":1}' \
    "http://$BOOTNODE_IP:9933"

# Get Node 2 chain height
curl -X POST \
    -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","method":"chain_getHeight","params":[],"id":1}' \
    "http://$NODE2_IP:9934"
```

### Test Metrics

```bash
# Bootnode metrics
curl "http://$BOOTNODE_IP:9090/metrics"

# Node 2 metrics
curl "http://$NODE2_IP:9091/metrics"
```

## Monitoring

### Key Metrics to Monitor

1. **Peer Count**: Number of connected peers
2. **Chain Height**: Current blockchain height
3. **Block Propagation Time**: Time for blocks to propagate
4. **Network Events**: Peer connections/disconnections
5. **Error Rate**: Protocol errors, timeouts

### Prometheus Queries

```promql
# Peer count
coinject_network_peers

# Chain height
coinject_chain_height

# Block propagation time
coinject_block_propagation_time_seconds

# Network errors
rate(coinject_network_errors_total[5m])
```

## Troubleshooting

### Node Won't Start

1. Check logs: `journalctl -u coinject-bootnode -n 100`
2. Verify ports are open: `netstat -tulpn | grep -E '707|8080|9933'`
3. Check disk space: `df -h`
4. Verify binary permissions: `ls -la /opt/coinject/coinject-node`

### Nodes Not Connecting

1. Verify firewall rules allow ports 707 and 7071
2. Check bootnode is listening: `netstat -tulpn | grep 707`
3. Verify Node 2 bootnode config: Check `--bootnodes` argument
4. Check network connectivity: `ping $BOOTNODE_IP`

### WebSocket Not Working

1. Verify port 8080/8081 are open
2. Check WebSocket is enabled in config
3. Test with curl (see Testing section)
4. Check browser console for errors (if testing from browser)

## Post-Deployment Checklist

- [ ] Both nodes are running (`systemctl status`)
- [ ] Nodes are connected (check logs for peer connections)
- [ ] Blocks are syncing (compare chain heights)
- [ ] WebSocket is accessible (test connection)
- [ ] RPC is responding (test JSON-RPC calls)
- [ ] Metrics are being collected (check Prometheus endpoint)
- [ ] Firewall rules are configured correctly
- [ ] Systemd services are enabled (auto-start on reboot)

## Empirical Data Collection

After deployment, collect data on:

1. **Equilibrium Convergence**: Monitor flow control window sizes
2. **η ≈ λ ≈ 0.7071**: Verify equilibrium constant in routing
3. **Block Propagation**: Measure time for blocks to reach all nodes
4. **Peer Selection**: Monitor reputation-based peer selection
5. **Network Stability**: Track connection uptime and error rates

## Next Steps

1. Deploy additional nodes to test larger network
2. Implement monitoring dashboard (Grafana)
3. Collect empirical data for research paper
4. Optimize based on production metrics
5. Scale to more nodes as needed

