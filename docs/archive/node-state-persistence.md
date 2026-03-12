# Node State Persistence & Redeploy Guide

When we redeployed the droplets without restoring their data directories, each node booted from genesis and the chain reset to height `0`. This guide outlines how to keep state intact across restarts, how to back up or restore a node, and a safe checklist for future upgrades.

---

## 1. Understand What Must Persist

The node stores everything under the path passed to `--data-dir` (default `/app/data`). When mapped via Docker, we mount it from the host (`/root/coinject-data`). The directory contains:

- `chain.db`, `state.db`, and other redb files (blockchain history, account balances, etc.)
- `validator_key.bin` (miner identity)
- `network_key` (libp2p PeerID)
- HuggingFace buffer files, logs, and runtime metadata

**If this directory is missing or empty when the node starts, it creates a brand-new chain at genesis.**

---

## 2. Back Up Before Touching Containers

1. **Stop the container (if needed)**  
   ```bash
   docker stop coinject-node
   ```
2. **Archive the data directory**  
   ```bash
   cd /root
   tar czf coinject-data-$(date +%Y%m%d-%H%M).tgz coinject-data
   ```
3. **Upload to remote storage** (S3/Spaces/etc.)  
   ```bash
   aws s3 cp coinject-data-20251127-0130.tgz s3://coinjecture-backups/
   ```

Do not delete `coinject-data` unless you have a verified backup.

---

## 3. Safe Redeploy Checklist

1. **Confirm data dir exists**  
   ```bash
   ls -al /root/coinject-data
   # expect chain.db, state.db, validator_key.bin, network_key, ...
   ```
2. **Stop container gracefully**  
   ```bash
   docker stop coinject-node
   ```
3. **Update image / binaries** (if needed)  
   ```bash
   docker load -i coinject-node-amd64.tar
   ```
4. **Restart container reusing the same volume**  
   ```bash
   docker run -d --name coinject-node \
     --restart unless-stopped \
     -p 9933:9933 -p 30333:30333 \
     -v /root/coinject-data:/app/data \
     coinject-node:latest \
     --rpc-addr 0.0.0.0:9933 \
     --data-dir /app/data \
     --mine \
     --bootnodes <multiaddr(s)>
   ```
5. **Verify chain continuity**  
   ```bash
   curl -s -X POST http://localhost:9933 \
     -H 'Content-Type: application/json' \
     -d '{"jsonrpc":"2.0","method":"chain_getInfo","params":[],"id":1}'
   ```
   Confirm `best_height` and `best_hash` match expectations.

---

## 4. Restoring From Backup

1. **Stop container** (see above).
2. **Move current data aside** (optional):  
   ```bash
   mv /root/coinject-data /root/coinject-data.bak.$(date +%H%M)
   mkdir /root/coinject-data
   ```
3. **Extract backup**  
   ```bash
   tar xzf coinject-data-YYYYMMDD-HHMM.tgz -C /root
   ```
4. **Restart container using restored dir.**
5. **Check `chain_getInfo`**, ensure heights align.

---

## 5. Hardening Tips

- **Use persistent block storage** (DigitalOcean Volumes, AWS EBS, etc.) mounted at `/root/coinject-data`. This survives droplet reboots/recreates.
- **Avoid `docker rm` unless necessary.** Use `docker stop`/`start` so the container retains its volume configuration.
- **Automate integrity checks:** script that refuses to start Docker if `chain.db` or `validator_key.bin` is missing.
- **Maintain a seed node:** keep at least one authoritative node online so others can resync if needed.
- **Document every deploy:** log image digest, chain height, and backup filename before touching production nodes.

---

Keeping the data directory intact (or restoring it before restart) is the single most important step. Follow this guide each time we rebuild or redeploy, and the chain will continue without resets.

