# Unidirectional Connectivity & Ghost IP Connections - Full Diagnostic Analysis

**Date:** 2025-12-13  
**Version:** v4.7.58  
**Status:** Active Investigation  
**Issue:** Persistent unidirectional connectivity between Node 1 and Node 2, with ghost IP addresses (169.254.x.x) appearing in connection attempts

---

## Executive Summary

Node 1 (143.110.139.166) can successfully connect OUTBOUND to Node 2 (68.183.205.12) and receive blocks, but Node 2 cannot establish OUTBOUND connections to Node 1. Additionally, connection attempts show "ghost" IP addresses (169.254.x.x) that don't correspond to actual network interfaces. Despite multiple fixes deployed, the core unidirectional connectivity issue persists.

### Current Connection Status
- **Node 1 → Node 2:** ✅ Connected (outbound, receiving blocks)
- **Node 2 → Node 1:** ❌ Failed (timeouts on all outbound dial attempts)
- **Node 2 Incoming Connections:** Only from GCE VM (34.96.47.10), NOT from Node 1
- **Bidirectional GossipSub Mesh:** ❌ Not established (unidirectional prevents mesh)

---

## Issue 1: Ghost IP Addresses (169.254.x.x)

### Symptoms

Connection attempts show IPs in the 169.254.x.x range:
```
Error: Transport([(/ip4/169.254.8.1/tcp/30333/p2p/...), 
                  (/ip4/169.254.169.1/tcp/30333/p2p/...), 
                  (/ip4/169.254.9.1/tcp/30333/p2p/...)])
```

These addresses appear in connection error logs but don't correspond to actual network interfaces.

### Root Cause Analysis

**169.254.0.0/16 is Link-Local Address Space (RFC 3927)**
- These are auto-configured addresses when DHCP fails
- Used for automatic IP assignment on local networks
- Should NOT appear in connection attempts to remote peers
- These are "ghost" IPs that don't exist on the network

**Why They Appear:**

1. **libp2p Address Discovery via Identify Protocol:**
   - When peers exchange identify protocol messages, they advertise their listen addresses
   - If a peer has link-local addresses on its network interfaces, it may advertise them
   - libp2p then tries to connect using these advertised addresses
   - Since 169.254.x.x addresses are link-local, they're not routable across the internet
   - Connection attempts to these addresses always timeout

2. **Docker Network Configuration:**
   - Docker may create link-local addresses on bridge networks
   - These addresses may be discovered by libp2p's address detection
   - Address filtering wasn't catching 169.254.x.x range

3. **Identify Protocol Advertisement:**
   - Peers may be advertising link-local addresses in identify protocol
   - These addresses are then used for connection attempts
   - Should be filtered by `is_private_address()` but wasn't included

### Fix Implemented (v4.7.58)

**Solution:** Added 169.254.0.0/16 filtering to `is_private_address()` function:

```rust
fn is_private_address(addr: &Multiaddr) -> bool {
    let s = addr.to_string();
    s.contains("/ip4/10.") ||           // Docker internal networks
    s.contains("/ip4/172.1") ||         // Docker bridge (172.16-172.31)
    s.contains("/ip4/172.2") ||         // Docker bridge
    s.contains("/ip4/172.3") ||         // Docker bridge  
    s.contains("/ip4/192.168.") ||      // Private networks
    s.contains("/ip4/127.") ||          // Loopback
    s.contains("/ip4/169.254.") ||      // Link-local auto-configuration (RFC 3927) ✅ ADDED
    s.contains("/ip6/::1") ||           // IPv6 loopback
    s.contains("/ip6/fe80")             // IPv6 link-local
}
```

**Status:** ✅ Fixed in v4.7.58, deployed to both nodes

**Expected Impact:** Eliminates ghost IP connection attempts, reduces connection timeouts

---

## Issue 2: Unidirectional Connectivity

### Symptoms

1. **Node 1 connects OUTBOUND to Node 2 successfully:**
   - Handshake completes (~148ms)
   - Node 1 receives blocks from Node 2 (blocks 215, 216, 217, 226+)
   - Node 1 tracks Node 2 as connected peer
   - Connection appears stable from Node 1's perspective

2. **Node 2 does NOT see Node 1's connection:**
   - No incoming connection attempts from 143.110.139.166 logged
   - All incoming connections are from GCE VM (34.96.47.10)
   - Node 2's outbound dials to Node 1 timeout
   - Node 2 never tracks Node 1 as a connected peer

3. **Connection appears one-way:**
   - Node 1 can send/receive data to/from Node 2
   - Node 2 cannot establish reverse connection
   - GossipSub mesh cannot be established (requires bidirectional connections)

### Critical Discovery: Node 1's Connection Never Reaches Node 2's TCP Layer

**Evidence:**
- Node 2 shows NO incoming connection attempts from 143.110.139.166
- Node 2's TCP port 30333 is listening and accessible (netcat test succeeds)
- All incoming connections to Node 2 are from GCE VM (34.96.47.10)
- Node 1 reports successful connection and receives blocks

**This suggests:**
1. Node 1's connection may be using a different path/mechanism
2. Connection may be established but immediately reset before Node 2 tracks it
3. Connection may be using an intermediate proxy/relay that Node 2 doesn't recognize
4. There may be a connection direction mismatch in libp2p

### Possible Root Causes

#### A. Connection Reset Before Tracking

**Hypothesis:** Node 1's connection reaches Node 2's TCP layer, but is reset during or immediately after handshake, before Node 2 can track it.

**Evidence:**
- Node 1 sees "Connection reset by peer" errors
- Node 1 still receives blocks, suggesting connection exists at some level
- Node 2 never logs the connection

**Possible Causes:**
- Handshake timeout on Node 2's side
- Connection limit reached (too many GCE VM connections)
- Protocol mismatch causing immediate reset
- Connection state corruption

#### B. NAT/Firewall Asymmetry

**Hypothesis:** Node 1 is behind a NAT that allows outbound but blocks inbound connections.

**Evidence:**
- Node 1 can connect outbound successfully
- Node 2 cannot connect inbound to Node 1
- Both nodes are DigitalOcean droplets with public IPs
- Docker bridge mode may be complicating routing

**Possible Causes:**
- Docker NAT interfering with connection tracking
- Port mapping not working correctly for inbound connections
- Firewall rules on DigitalOcean level (not visible via UFW)

#### C. Docker Network Isolation

**Hypothesis:** Docker bridge mode is interfering with connection tracking.

**Evidence:**
- Both nodes run in Docker bridge mode
- Container IPs: 172.17.0.2 (both nodes, different hosts)
- Port 30333 mapped via Docker port mapping
- Connection may be reaching Docker but not the container

**Possible Causes:**
- Docker's NAT may be interfering with connection tracking
- Inbound connections may be reaching Docker but not the container
- Connection state may not be properly synchronized

#### D. libp2p Connection Direction Mismatch

**Hypothesis:** libp2p is not properly recognizing connection direction.

**Evidence:**
- Node 1 establishes connection as OUTBOUND
- Node 2 may not recognize it as INBOUND due to connection state
- libp2p may be treating it as a different connection type

**Possible Causes:**
- Connection state not properly synchronized
- Identify protocol not exchanging correctly
- Connection tracking bug in libp2p

#### E. Observed Address Mismatch (CRITICAL HYPOTHESIS)

**Hypothesis:** Node 2 is trying to connect to Node 1's ephemeral port instead of listen port.

**Evidence from Previous Logs:**
- Identify protocol reports "observed address" (how peer sees us)
- Node 2 may see Node 1 connecting from port 38528 (ephemeral)
- Node 1 listens on port 30333
- Node 2 tries to connect to 38528 instead of 30333

**Root Cause:** libp2p may be using the observed address for reverse connections instead of the advertised listen address.

---

## Issue 3: GCE VM Interference

### Symptoms

- GCE VM (34.96.47.10, PeerId: 12D3KooWFL8uuMmeoWyU46SdX8g2aJEk4Fv5qAr4dZXmZfsGiefa) constantly connects to Node 2
- Multiple simultaneous connection attempts
- Deserialization errors from GCE VM (protocol version mismatch)
- Consumes connection slots and resources

### Impact

- May be preventing Node 1's connections from being established
- Connection backlog may be full of GCE VM attempts
- Resources consumed processing GCE VM handshakes before disconnection
- Deserialization errors create noise in logs

### Fix Implemented (v4.7.57)

**Solution:** Added peer blacklist that disconnects GCE VM peer immediately after handshake:

```rust
blacklisted_peers: {
    let mut blacklist = HashSet::new();
    if let Ok(blacklisted_peer) = "12D3KooWFL8uuMmeoWyU46SdX8g2aJEk4Fv5qAr4dZXmZfsGiefa".parse::<PeerId>() {
        blacklist.insert(blacklisted_peer);
    }
    blacklist
}
```

**Status:** ✅ Implemented and deployed

**Result:** GCE VM connections are immediately disconnected, freeing up connection slots

---

## Network Configuration Details

### Node 1 (143.110.139.166)
- **Public IP:** 143.110.139.166
- **Private IPs:** 10.46.0.6, 10.120.0.3
- **Docker IP:** 172.17.0.1
- **Container IP:** 172.17.0.2
- **Listen Address:** `/ip4/0.0.0.0/tcp/30333`
- **Port Mapping:** `-p 30333:30333`
- **Firewall:** UFW inactive

### Node 2 (68.183.205.12)
- **Public IP:** 68.183.205.12
- **Private IPs:** 10.20.0.5, 10.118.0.2
- **Docker IP:** 172.17.0.1
- **Container IP:** 172.17.0.2
- **Listen Address:** `/ip4/0.0.0.0/tcp/30333`
- **Port Mapping:** `-p 30333:30333`
- **Firewall:** UFW inactive

### Network Tests

**TCP Connectivity:**
```bash
# From Node 2 to Node 1
$ nc -zv 143.110.139.166 30333
Connection to 143.110.139.166 30333 port [tcp/*] succeeded! ✅
```

**Port Status:**
```bash
# Node 1
$ ss -tlnp | grep 30333
LISTEN 0 4096 0.0.0.0:30333 0.0.0.0:* ✅

# Node 2
$ ss -tlnp | grep 30333
LISTEN 0 4096 0.0.0.0:30333 0.0.0.0:* ✅
```

**Conclusion:** TCP layer is reachable, issue is at libp2p/application layer

---

## Connection Flow Analysis

### Node 1's Connection to Node 2 (OUTBOUND)

**Observed Flow:**
1. Node 1 dials: `/ip4/68.183.205.12/tcp/30333/p2p/12D3KooWQwpXp7NJG9gMVJMFH7oBfYQizbtPAB3RfRqxyvQ5WZfv`
2. TCP connection established ✅
3. Noise handshake completes (~148ms) ✅
4. Yamux negotiation completes ✅
5. Connection established (OUTBOUND from Node 1's perspective) ✅
6. Identify protocol exchange ✅
7. Node 1 receives blocks from Node 2 ✅
8. **BUT:** Node 2 never sees this connection as INBOUND ❌

**Critical Gap:** Step 8 - Node 2 should see this as an INBOUND connection but doesn't.

### Node 2's Connection Attempts to Node 1 (OUTBOUND)

**Observed Flow:**
1. Node 2 dials: `/ip4/143.110.139.166/tcp/30333/p2p/12D3KooWL3Q7KmTocqNGLfyz4X4mhyyPD8b4zx6MBk1qnDAT8FYs`
2. TCP connection may or may not be established (unclear)
3. Noise handshake times out ❌
4. Connection fails with timeout error ❌
5. Retry loop continues ❌

**Critical Gap:** Step 3 - Handshake times out, preventing connection establishment.

---

## Implemented Fixes Summary

### Fix 1: Link-Local Address Filtering (v4.7.58) ✅
- **Status:** Implemented and deployed
- **Functionality:** Filters 169.254.0.0/16 addresses
- **Result:** Eliminates ghost IP connection attempts
- **Impact:** Reduces connection timeouts, but doesn't fix unidirectional issue

### Fix 2: Peer Blacklisting (v4.7.57) ✅
- **Status:** Implemented and deployed
- **Functionality:** Node 2 blacklists GCE VM peer
- **Result:** GCE VM connections are immediately disconnected
- **Impact:** Frees up connection slots, but doesn't fix Node 1 ↔ Node 2 connectivity

### Fix 3: Incoming Connection Tracking (v4.7.56) ✅
- **Status:** Implemented and deployed
- **Functionality:** Track incoming connection attempts to prevent simultaneous dials
- **Result:** Should prevent race conditions, but Node 2 never sees Node 1's incoming connections
- **Impact:** Limited - doesn't address root cause

### Fix 4: Bootnode Connection Recognition (v4.7.55) ✅
- **Status:** Implemented and deployed
- **Functionality:** Check both internal tracking and swarm's connected peers
- **Result:** Should recognize inbound connections, but Node 2 doesn't see Node 1's connection
- **Impact:** Limited - doesn't address root cause

### Fix 5: Relay Address Discovery (v4.7.54) ✅
- **Status:** Implemented and deployed
- **Functionality:** Extract and use relay addresses for fallback connections
- **Result:** Relay addresses are discovered but no relay server available
- **Impact:** Limited - requires relay server to be effective

---

## Diagnostic Findings

### Finding 1: Node 1 Receives Blocks from Node 2

**Evidence:**
```
📥 Received block 226 from PeerId("12D3KooWQwpXp7NJG9gMVJMFH7oBfYQizbtPAB3RfRqxyvQ5WZfv")
```

**Implication:**
- Connection exists at some level (Node 1 can receive data)
- GossipSub is working in one direction
- Connection is not completely broken

### Finding 2: Node 2 Never Sees Node 1's IP in Incoming Connections

**Evidence:**
- All incoming connections to Node 2 are from 34.96.47.10 (GCE VM)
- No incoming connections from 143.110.139.166 (Node 1)
- Node 2's logs show no connection attempts from Node 1's IP

**Implication:**
- Node 1's connection may be using a different mechanism
- Connection may be established but not tracked as INBOUND
- Connection may be reset before Node 2 can log it

### Finding 3: Ghost IP Addresses Still Appear (Pre-Fix)

**Evidence:**
```
Error: Transport([(/ip4/169.254.8.1/tcp/30333/p2p/...), 
                  (/ip4/169.254.169.1/tcp/30333/p2p/...), 
                  (/ip4/169.254.9.1/tcp/30333/p2p/...)])
```

**Implication:**
- libp2p is discovering/advertising link-local addresses
- These addresses are being used for connection attempts
- Fix in v4.7.58 should eliminate these

### Finding 4: Connection Resets on Node 1

**Evidence:**
```
❌ OUTGOING CONNECTION FAILED [TRANSPORT_ERROR]
   Error: Connection reset by peer
```

**Implication:**
- Connection is established but then reset
- Reset may be happening on Node 2's side
- Reset may be due to protocol mismatch or connection limit

### Finding 5: Deserialization Errors from GCE VM

**Evidence:**
```
❌ Failed to deserialize NetworkMessage from peer PeerId("12D3KooWFL8uuMmeoWyU46SdX8g2aJEk4Fv5qAr4dZXmZfsGiefa")
   invalid value: integer '416407758', expected variant index 0 <= i < 6
```

**Implication:**
- GCE VM is running different protocol version
- Messages are malformed or incompatible
- Blacklist fix should eliminate these errors

---

## Recommended Solutions

### Solution 1: Fix Observed Address Usage (HIGH PRIORITY)

**Problem:** Node 2 may be trying to connect to Node 1's ephemeral port instead of listen port.

**Fix:** Ensure reverse connections use advertised listen addresses, not observed addresses:

```rust
// In identify protocol handler, prefer listen addresses over observed addresses
// Filter observed addresses that don't match listen addresses
// Use listen addresses for connection attempts
```

**Expected Impact:** Node 2 will connect to correct port (30333) instead of ephemeral port

### Solution 2: Enhanced Connection State Logging (MEDIUM PRIORITY)

**Problem:** Insufficient visibility into connection establishment process.

**Fix:** Add detailed logging at each stage:
- TCP connection establishment (before libp2p)
- Noise handshake start/completion
- Yamux negotiation
- Identify protocol exchange
- Connection direction detection
- Connection state synchronization

**Expected Impact:** Better diagnostics to identify where connection fails

### Solution 3: Connection State Verification (MEDIUM PRIORITY)

**Problem:** Node 1's connection exists but Node 2 doesn't track it.

**Fix:** Add periodic connection state verification:
- Query swarm for all active connections
- Compare with internal peer tracking
- Log discrepancies
- Attempt to reconcile state

**Expected Impact:** May reveal why connections aren't being tracked

### Solution 4: Docker Host Network Mode (LOW PRIORITY)

**Problem:** Docker bridge mode may be interfering with connection tracking.

**Fix:** Run containers with `--network=host`:

```bash
docker run --network=host ...
```

**Trade-offs:**
- ✅ Eliminates Docker NAT issues
- ✅ Direct access to host network
- ⚠️ Port conflicts possible if other services use 30333
- ⚠️ Less container isolation

**Expected Impact:** May resolve connection tracking issues, but requires testing

### Solution 5: Force Public Address Advertisement (MEDIUM PRIORITY)

**Problem:** Nodes may not be advertising their public IPs correctly.

**Fix:** Explicitly add public IPs as external addresses:

```rust
// In NetworkService::new, after swarm creation
let public_ip = "143.110.139.166"; // Node 1's public IP
let public_addr: Multiaddr = format!("/ip4/{}/tcp/30333", public_ip).parse()?;
swarm.add_external_address(public_addr);
```

**Expected Impact:** Ensures peers use correct public IPs for connections

---

## Diagnostic Commands

### Check Active TCP Connections
```bash
# Node 1
ssh root@143.110.139.166 'ss -tnp | grep 30333'

# Node 2
ssh root@68.183.205.12 'ss -tnp | grep 30333'
```

### Check Docker Network Configuration
```bash
# Node 1
ssh root@143.110.139.166 'docker inspect coinject-node | grep -A 10 NetworkSettings'

# Node 2
ssh root@68.183.205.12 'docker inspect coinject-node | grep -A 10 NetworkSettings'
```

### Monitor Connection Attempts in Real-Time
```bash
# Node 2 - Watch for incoming connections from Node 1
ssh root@68.183.205.12 'docker logs -f coinject-node 2>&1 | grep -E "INCOMING|143.110.139.166"'

# Node 1 - Watch for connection attempts to Node 2
ssh root@143.110.139.166 'docker logs -f coinject-node 2>&1 | grep -E "DIALING|68.183.205.12"'
```

### Test TCP Connectivity
```bash
# From Node 2 to Node 1
ssh root@68.183.205.12 'timeout 5 nc -zv 143.110.139.166 30333'

# From Node 1 to Node 2
ssh root@143.110.139.166 'timeout 5 nc -zv 68.183.205.12 30333'
```

### Check Connection State via RPC
```bash
# Node 1 - Get connected peers
curl -X POST -H "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","method":"get_peers","id":1}' \
  http://143.110.139.166:8545

# Node 2 - Get connected peers
curl -X POST -H "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","method":"get_peers","id":1}' \
  http://68.183.205.12:8545
```

---

## Next Steps

### Immediate Actions
1. ✅ **Fix link-local address filtering** (v4.7.58) - COMPLETED
2. ✅ **Blacklist GCE VM peer** (v4.7.57) - COMPLETED
3. **Investigate observed address usage** - Verify if Node 2 is using wrong port
4. **Add enhanced connection logging** - Track connection at each stage
5. **Verify connection state** - Compare swarm state with internal tracking

### Short-Term Actions
1. **Force public address advertisement** - Ensure correct IPs are used
2. **Implement connection state verification** - Periodic reconciliation
3. **Test Docker host network mode** - Eliminate Docker NAT issues

### Long-Term Actions
1. **Consider QUIC transport** - Better NAT traversal
2. **Implement relay server** - For nodes behind restrictive NATs
3. **Add connection health monitoring** - Proactive detection of issues

---

## Conclusion

The unidirectional connectivity issue is complex and involves multiple factors:

1. **Ghost IP Addresses (169.254.x.x):** ✅ FIXED in v4.7.58 - Added link-local address filtering
2. **Observed Address Mismatch:** Likely cause - Node 2 trying to connect to ephemeral port instead of listen port
3. **Docker Network Configuration:** May be interfering with connection tracking
4. **libp2p Connection State:** Not properly synchronized between nodes
5. **GCE VM Interference:** ✅ MITIGATED in v4.7.57 - Blacklist implemented

**Primary Hypothesis:** Node 1's connection to Node 2 is established but Node 2 doesn't recognize it as an INBOUND connection, possibly due to connection state mismatch or immediate reset. Node 2's attempts to connect to Node 1 fail because it may be using the wrong port (ephemeral vs listen) or the connection is being blocked/reset.

**Recommended Next Step:** Investigate and fix observed address usage to ensure Node 2 connects to Node 1's correct listen port (30333) instead of any ephemeral port.

---

## References

- RFC 3927: Link-Local Addresses (169.254.0.0/16)
- libp2p Documentation: Connection Establishment
- libp2p Documentation: Identify Protocol and Observed Addresses
- Docker Networking: Bridge Mode
- DigitalOcean Networking: Public IP Configuration
- libp2p Noise Handshake Protocol
- GossipSub Mesh Requirements

