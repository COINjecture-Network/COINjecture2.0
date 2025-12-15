# Unidirectional Connectivity Fix - v4.7.10

## Diagnosis Summary

### Root Cause
The unidirectional connectivity issue was caused by **three compounding factors**:

1. **No address filtering**: The identify protocol handler blindly added ALL addresses from peers to kademlia, including:
   - `169.254.x.x` link-local addresses from Docker/VM interfaces
   - Private RFC1918 addresses not routable between cloud droplets
   - Addresses with ephemeral source ports from Docker NAT (e.g., `/tcp/54321` instead of `/tcp/30333`)

2. **No external address advertisement**: Without `swarm.add_external_address()`, nodes behind Docker NAT advertised their internal container IP/port instead of the public-facing address.

3. **Stray node pollution**: Unintended background nodes poisoned the DHT and identify "observed address" consensus with conflicting/invalid addresses.

### Critical Code Locations Fixed

| Location | Issue | Fix |
|----------|-------|-----|
| `protocol.rs:666-680` | Identify handler added ALL addresses | Now filters via `filter_multiaddrs_with_logging()` |
| `protocol.rs:644-655` | mDNS handler added addresses without filtering | Now validates via `validate_multiaddr()` |
| `protocol.rs:270-275` | No external address support | Now calls `swarm.add_external_address()` if configured |
| `protocol.rs:438-492` | Dial collision issues | Now uses `DialOpts` with `PeerCondition::Disconnected` |
| `config.rs` | No `--external-addr` flag | Added `--external-addr` and `--allow-private-addrs` flags |

---

## Changes Made

### New File: `network/src/addr_filter.rs`
Address filtering module that rejects:
- Link-local: `169.254.0.0/16`
- Loopback: `127.0.0.0/8`
- Unspecified: `0.0.0.0/8`
- Private RFC1918: `10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`
- Docker bridge: `172.17.0.0/16`
- Multicast/Broadcast
- Ephemeral ports (>32767) when expected port is 30333

### New CLI Flags

```bash
--external-addr /ip4/<PUBLIC_IP>/tcp/30333  # Advertised address
--allow-private-addrs                        # For local/LAN testing
```

### Instrumentation Logs (grep-able)

| Prefix | Purpose |
|--------|---------|
| `[ADDR]` | External address setup |
| `[ADDR_FILTER]` | Address rejection reasons |
| `[IDENTIFY]` | Identify protocol events |
| `[MDNS]` | mDNS discovery events |
| `[CONN]` | Connection established/closed |
| `[DIAL]` | Dialing attempts |
| `[DIAL_ERR]` | Dial failures with details |
| `[CONN_ERR]` | Connection errors |
| `[LISTEN]` | Listen address events |
| `[BOOT]` | Bootnode connection attempts |
| `[RETRY]` | Bootnode retry attempts |

---

## Test Plan

### A) Sanity Cleanup

#### 1. Confirm no unintended nodes running (laptop/local)
```bash
# Windows (PowerShell)
Get-Process | Where-Object { $_.ProcessName -like "*coinject*" }
tasklist | findstr coinject

# Linux/Mac
ps aux | grep coinject
pgrep -la coinject
```

#### 2. Confirm no unintended nodes on droplets
```bash
# On each droplet
ssh root@143.110.139.166 "ps aux | grep coinject"
ssh root@68.183.205.12 "ps aux | grep coinject"

# Kill all existing nodes
ssh root@143.110.139.166 "pkill -9 coinject; sleep 2; ps aux | grep coinject"
ssh root@68.183.205.12 "pkill -9 coinject; sleep 2; ps aux | grep coinject"
```

#### 3. Verify port 30333 is free
```bash
# On each droplet
ssh root@143.110.139.166 "ss -tlnp | grep 30333"
ssh root@68.183.205.12 "ss -tlnp | grep 30333"
```

#### 4. Clear peerstore caches
```bash
# Delete network_key to get fresh PeerId (optional, only if needed)
ssh root@143.110.139.166 "rm -f /root/COINjecture1337-NETB-main/node-data/network_key"
ssh root@68.183.205.12 "rm -f /root/COINjecture1337-NETB-main/node-data/network_key"

# Or just clear the chain/state for fresh start
ssh root@143.110.139.166 "rm -rf /root/COINjecture1337-NETB-main/node-data/*.db"
ssh root@68.183.205.12 "rm -rf /root/COINjecture1337-NETB-main/node-data/*.db"
```

---

### B) Build and Deploy

```bash
# Build release binary
cargo build --release

# Copy to droplets
scp target/release/coinject root@143.110.139.166:/root/COINjecture1337-NETB-main/target/release/
scp target/release/coinject root@68.183.205.12:/root/COINjecture1337-NETB-main/target/release/
```

---

### C) Test: Docker Bridge Mode (original failing config)

#### Start Node 1 (Bootstrap)
```bash
ssh root@143.110.139.166 << 'EOF'
cd /root/COINjecture1337-NETB-main
pkill -9 coinject || true
sleep 2

# Docker bridge mode with external-addr fix
docker run -d --name node1 \
  -p 30333:30333 \
  -v $(pwd)/node-data:/data \
  --entrypoint /coinject \
  <your-image> \
    --data-dir /data \
    --p2p-addr /ip4/0.0.0.0/tcp/30333 \
    --external-addr /ip4/143.110.139.166/tcp/30333 \
    --rpc-addr 0.0.0.0:9933 \
    --difficulty 3 \
    --block-time 30 \
    --enable-faucet

# OR without Docker (recommended for testing):
nohup ./target/release/coinject \
  --data-dir ./node-data \
  --p2p-addr /ip4/0.0.0.0/tcp/30333 \
  --external-addr /ip4/143.110.139.166/tcp/30333 \
  --rpc-addr 0.0.0.0:9933 \
  --difficulty 3 \
  --block-time 30 \
  --enable-faucet \
  > /root/bootstrap.log 2>&1 &

sleep 3
grep PeerId /root/bootstrap.log | head -1
EOF
```

#### Start Node 2 with bootnode
```bash
# Get Node 1's PeerId first
NODE1_PEERID=$(ssh root@143.110.139.166 "grep 'Network node PeerId' /root/bootstrap.log | head -1 | awk '{print \$NF}'")
echo "Node 1 PeerId: $NODE1_PEERID"

ssh root@68.183.205.12 << EOF
cd /root/COINjecture1337-NETB-main
pkill -9 coinject || true
sleep 2

nohup ./target/release/coinject \
  --data-dir ./node-data \
  --p2p-addr /ip4/0.0.0.0/tcp/30333 \
  --external-addr /ip4/68.183.205.12/tcp/30333 \
  --rpc-addr 0.0.0.0:9933 \
  --bootnodes /ip4/143.110.139.166/tcp/30333/p2p/$NODE1_PEERID \
  --difficulty 3 \
  --block-time 30 \
  --enable-faucet \
  > /root/node.log 2>&1 &
EOF
```

---

### D) Test: Docker Host Networking (eliminates NAT variable)

```bash
# Node 1
docker run -d --name node1 --network host \
  -v $(pwd)/node-data:/data \
  --entrypoint /coinject <image> \
    --data-dir /data \
    --p2p-addr /ip4/0.0.0.0/tcp/30333 \
    --rpc-addr 0.0.0.0:9933

# Node 2 (with bootnode)
docker run -d --name node2 --network host \
  -v $(pwd)/node-data:/data \
  --entrypoint /coinject <image> \
    --data-dir /data \
    --p2p-addr /ip4/0.0.0.0/tcp/30333 \
    --rpc-addr 0.0.0.0:9933 \
    --bootnodes /ip4/143.110.139.166/tcp/30333/p2p/<NODE1_PEERID>
```

---

### E) Verification Commands

#### Check address filtering is working
```bash
# On Node 1 - should see [ADDR_FILTER] rejection logs
ssh root@143.110.139.166 "grep '\[ADDR_FILTER\]' /root/bootstrap.log"

# Should see NO rejections for valid public addresses
ssh root@143.110.139.166 "grep '\[IDENTIFY\].*Accepted' /root/bootstrap.log"
```

#### Check external address is advertised
```bash
ssh root@143.110.139.166 "grep '\[LISTEN\].*External addr' /root/bootstrap.log"
# Expected: [LISTEN]   External addr (advertised): /ip4/143.110.139.166/tcp/30333/p2p/...
```

#### Check bidirectional connectivity
```bash
# Node 2 should show Node 1 as connected
ssh root@68.183.205.12 "grep '\[CONN\].*Established' /root/node.log"

# Node 1 should show Node 2 as connected (BOTH directions)
ssh root@143.110.139.166 "grep '\[CONN\].*Established' /root/bootstrap.log"

# Check gossipsub mesh formed on both
ssh root@143.110.139.166 "grep 'subscribed to topic' /root/bootstrap.log"
ssh root@68.183.205.12 "grep 'subscribed to topic' /root/node.log"
```

#### Check no dial attempts to bad addresses
```bash
# Should see NO attempts to 169.254.x.x
ssh root@143.110.139.166 "grep '169.254' /root/bootstrap.log"
ssh root@68.183.205.12 "grep '169.254' /root/node.log"

# Should see NO attempts to ephemeral ports
ssh root@143.110.139.166 "grep '\[ADDR_FILTER\].*ephemeral' /root/bootstrap.log"
```

#### Check for connection resets (should be reduced)
```bash
ssh root@143.110.139.166 "grep -c 'connection reset' /root/bootstrap.log"
ssh root@68.183.205.12 "grep -c 'connection reset' /root/node.log"
```

---

### F) Success Criteria

| Criterion | How to Verify |
|-----------|---------------|
| Node 1 appears in Node 2's peers | `grep '[CONN].*Established' node.log` shows Node 1's PeerId |
| Node 2 appears in Node 1's peers | `grep '[CONN].*Established' bootstrap.log` shows Node 2's PeerId |
| Node 2 can dial Node 1 at public addr | `grep '[BOOT].*Dial initiated' node.log` succeeds |
| Gossipsub mesh is bidirectional | Both nodes show `Peer X subscribed to topic` for each other |
| No dial to 169.254.x.x | `grep '169.254' *.log` returns nothing |
| No dial to ephemeral ports | `grep 'ephemeral' *.log` shows only rejections, not attempts |
| Reduced connection resets | Reset count lower than before fix |

---

## Quick Grep Reference

```bash
# All address filtering decisions
grep '\[ADDR' /root/*.log

# All connection events
grep '\[CONN' /root/*.log

# All dial events (including failures)
grep '\[DIAL' /root/*.log

# Identify protocol activity
grep '\[IDENTIFY\]' /root/*.log

# Bootnode connections
grep '\[BOOT\]' /root/*.log

# Any errors
grep -E '\[.*_ERR\]|error|Error|ERROR' /root/*.log
```

---

## Rollback

If needed, restore previous behavior:
```bash
git checkout network/src/protocol.rs node/src/config.rs node/src/service.rs
rm network/src/addr_filter.rs
cargo build --release
```

---

## Version History

- **v4.7.10**: Address filtering, external-addr flag, dial collision prevention, instrumentation
- **v4.7.6**: Connection state checks, duplicate dial prevention
- **v4.7.5**: mesh_n_low fix (2 -> 1) for 2-node networks

