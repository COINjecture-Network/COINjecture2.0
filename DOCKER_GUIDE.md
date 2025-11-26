# COINjecture Docker Multi-Node Guide

This guide explains how to run a multi-node COINjecture blockchain network using Docker for testing the decentralized network.

## Prerequisites

- Docker Desktop (Windows) or Docker Engine (Linux/Mac)
- Docker Compose
- At least 4GB RAM and 10GB disk space

## Quick Start

### 1. Build the Docker Image

Build the Linux binary in a Docker container:

```bash
docker compose build
```

This will:
- Use Rust 1.75 to compile the project
- Create an optimized release binary
- Package it in a slim Debian container
- Take approximately 5-10 minutes on first build

### 2. Start the Multi-Node Network

Start all 4 nodes (1 bootnode + 3 peers):

```bash
docker compose up -d
```

### 3. View Node Logs

View logs from all nodes:
```bash
docker compose logs -f
```

View logs from a specific node:
```bash
docker compose logs -f bootnode
docker compose logs -f node1
docker compose logs -f node2
docker compose logs -f node3
```

### 4. Check Network Status

Check if nodes are connecting:
```bash
# Check bootnode logs for peer connections
docker compose logs bootnode | grep -i "peer"

# Check node1 sync status
docker compose logs node1 | grep -i "height"
```

### 5. Stop the Network

Stop all nodes:
```bash
docker compose down
```

Stop and remove all data volumes:
```bash
docker compose down -v
```

## Network Architecture

### Services

- **bootnode** (Port 30333/9933): First node that other nodes connect to
- **node1** (Port 30334/9934): Mining peer connected to bootnode
- **node2** (Port 30335/9935): Mining peer connected to bootnode
- **node3** (Port 30336/9936): Mining peer connected to bootnode

### Volumes

Each node has its own persistent data volume:
- `bootnode-data`: Bootnode blockchain data
- `node1-data`: Node 1 blockchain data
- `node2-data`: Node 2 blockchain data
- `node3-data`: Node 3 blockchain data

### Network

All nodes run on a shared bridge network `coinject-network` for P2P communication.

## Testing Decentralization

### Verify Nodes are Mining

```bash
# Check if all nodes are mining blocks
docker compose logs | grep "Mined block"
```

### Verify Block Sync

```bash
# Check if nodes are synchronizing blockchain
docker compose logs | grep -i "sync\|block"
```

### Test Network Resilience

Stop the bootnode and verify other nodes continue:
```bash
docker compose stop bootnode
docker compose logs node1 node2 node3 -f
```

The network should continue operating as nodes maintain P2P connections with each other.

## Troubleshooting

### Build Failures

If the build fails:
1. Check Docker has enough resources (4GB+ RAM)
2. Clear Docker cache: `docker system prune -a`
3. Rebuild: `docker compose build --no-cache`

### Connection Issues

If nodes can't connect:
1. Check network: `docker network ls`
2. Inspect container IPs: `docker compose ps`
3. Check bootnode is healthy: `docker compose ps bootnode`

### Performance Issues

If nodes run slowly:
1. Increase Docker resource limits in Docker Desktop settings
2. Reduce number of nodes in docker-compose.yml
3. Disable mining on some nodes (remove `--mine` flag)

## Advanced Configuration

### Modify Node Settings

Edit `docker-compose.yml` to change node parameters:

```yaml
command: >
  --mine
  --data-dir /data
  --p2p-addr /ip4/0.0.0.0/tcp/30333
  --rpc-addr 0.0.0.0:9933
  # Add more flags here
```

### Add More Nodes

Copy a node service in `docker-compose.yml`:

```yaml
  node4:
    build: .
    ports:
      - "30337:30337"
      - "9937:9937"
    volumes:
      - node4-data:/data
    command: >
      --mine
      --data-dir /data
      --p2p-addr /ip4/0.0.0.0/tcp/30337
      --rpc-addr 0.0.0.0:9937
      --bootnodes /ip4/bootnode/tcp/30333
    networks:
      - coinject-network
```

### Enable ADZDB in Docker

Edit environment variables in `docker-compose.yml`:

```yaml
environment:
  - RUST_LOG=info
  - ADZDB_ENABLED=true  # Enable ADZDB
  - ADZDB_BATCH_INTERVAL=5000
  - HF_TOKEN=your_token_here
```

## Monitoring

### Resource Usage

Monitor container resources:
```bash
docker stats
```

### Network Traffic

Monitor network activity:
```bash
docker compose exec bootnode ss -tuln
```

### Blockchain State

Access node via RPC:
```bash
curl http://localhost:9933/status
```

## Clean Up

### Remove All Containers and Volumes

```bash
docker compose down -v
```

### Remove Docker Image

```bash
docker rmi coinject:latest
```

## Support

For issues or questions:
- Check logs: `docker compose logs`
- GitHub Issues: https://github.com/beanapologist/COINjecture-NetB-Updates/issues
- Discord: [Your Discord Link]
