# Unidirectional Connectivity & Ghost IP Connections - Diagnostic Analysis

**Date:** 2025-12-13  
**Version:** v4.7.58  
**Issue:** Persistent unidirectional connectivity between Node 1 and Node 2, with ghost IP addresses appearing in connection attempts

## Executive Summary

Node 1 (143.110.139.166) can successfully connect OUTBOUND to Node 2 (68.183.205.12) and receive blocks, but Node 2 cannot establish OUTBOUND connections to Node 1. Additionally, connection attempts show "ghost" IP addresses (169.254.x.x) that don't correspond to actual network interfaces. This document provides a comprehensive analysis of the root causes and potential solutions.

## Current Status

### Connection Status
- **Node 1 → Node 2:** ✅ Connected (outbound, receiving blocks)
- **Node 2 → Node 1:** ❌ Failed (timeouts on all outbound dial attempts)
- **Node 2 Incoming Connections:** Only from GCE VM (34.96.47.10), NOT from Node 1
- **Bidirectional GossipSub Mesh:** ❌ Not established (unidirectional prevents mesh)

### Network Configuration
- **Node 1 IP:** 143.110.139.166 (public), 10.46.0.6, 10.120.0.3 (private), 172.17.0.1 (Docker)
- **Node 2 IP:** 68.183.205.12 (public), 10.20.0.5, 10.118.0.2 (private), 172.17.0.1 (Docker)
- **Both nodes:** Running in Docker bridge mode, port 30333 mapped
- **Firewall:** Inactive on both nodes (UFW status: inactive)

## Root Cause Analysis

### Issue 1: Unidirectional Connectivity

#### Symptoms
1. Node 1 connects OUTBOUND to Node 2 successfully
   - Handshake completes (~148ms)
   - Node 1 receives blocks from Node 2
   - Node 1 tracks Node 2 as connected peer

2. Node 2 does NOT see Node 1's connection
   - No incoming connection attempts from 143.110.139.166
   - All incoming connections are from GCE VM (34.96.47.10)
   - Node 2's outbound dials to Node 1 timeout

3. Connection appears one-way
   - Node 1 can send/receive data to/from Node 2
   - Node 2 cannot establish reverse connection

#### Possible Causes

**A. NAT/Firewall Asymmetry**
- Node 1 may be behind a NAT that allows outbound but blocks inbound
- DigitalOcean droplets should have public IPs, but Docker bridge mode may complicate routing
- Port mapping may not be working correctly for inbound connections

**B. Connection Reset Before Tracking**
- Node 1's connection reaches Node 2's TCP layer
- Connection is reset during or immediately after handshake
- Node 2 never sees it as an established connection
- Evidence: Node 1 receives blocks, suggesting connection exists but isn't tracked on Node 2

**C. Docker Network Isolation**
- Both nodes run in Docker bridge mode
- Docker's NAT may be interfering with connection tracking
- Inbound connections may be reaching Docker but not the container

**D. libp2p Connection Direction Mismatch**
- Node 1 establishes connection as OUTBOUND
- Node 2 may not recognize it as INBOUND due to connection state
- libp2p may be treating it as a different connection type

### Issue 2: Ghost IP Addresses (169.254.x.x)

#### Symptoms
Connection attempts show IPs in the 169.254.x.x range:
```
Error: Transport([(/ip4/169.254.8.1/tcp/30333/p2p/...), 
                  (/ip4/169.254.169.1/tcp/30333/p2p/...), 
                  (/ip4/169.254.9.1/tcp/30333/p2p/...)])
```

#### Analysis

**169.254.0.0/16 is Link-Local Address Space (RFC 3927)**
- These are auto-configured addresses when DHCP fails
- Used for automatic IP assignment on local networks
- Should NOT appear in connection attempts to remote peers

**Why They Appear:**
1. **libp2p Address Discovery Bug:**
   - libp2p may be discovering these addresses via identify protocol
   - Peers may be advertising link-local addresses incorrectly
   - Address filtering may not be catching 169.254.x.x range

2. **Docker Network Configuration:**
   - Docker may be creating link-local addresses on bridge networks
   - These addresses may be leaked into libp2p's address discovery
   - Address should be filtered but isn't

3. **Identify Protocol Advertisement:**
   - Peers may be advertising link-local addresses in identify protocol
   - These addresses are then used for connection attempts
   - Should be filtered by `is_private_address()` but 169.254.x.x isn't included

**Current Filtering:**
The `is_private_address()` function filters:
- 10.x.x.x (Docker internal)
- 172.16-172.31.x.x (Docker bridge)
- 192.168.x.x (Private networks)
- 127.x.x.x (Loopback)
- IPv6 loopback and link-local

**Missing:** 169.254.0.0/16 (Link-local auto-configuration)

### Issue 3: GCE VM Interference

#### Symptoms
- GCE VM (34.96.47.10, PeerId: 12D3KooWFL8uuMmeoWyU46SdX8g2aJEk4Fv5qAr4dZXmZfsGiefa) constantly connects to Node 2
- Node 2 now blacklists and disconnects this peer immediately
- GCE VM connections may be consuming connection slots/resources

#### Impact
- May be preventing Node 1's connections from being established
- Connection backlog may be full of GCE VM attempts
- Resources consumed processing GCE VM handshakes before disconnection

## Technical Details

### Connection Flow Analysis

**Node 1's Connection to Node 2:**
1. Node 1 dials: `/ip4/68.183.205.12/tcp/30333/p2p/12D3KooWQwpXp7NJG9gMVJMFH7oBfYQizbtPAB3RfRqxyvQ5WZfv`
2. TCP connection established
3. Noise handshake completes (~148ms)
4. Yamux negotiation completes
5. Connection established (OUTBOUND from Node 1's perspective)
6. Node 1 receives blocks from Node 2
7. **BUT:** Node 2 never sees this connection as INBOUND

**Node 2's Connection Attempts to Node 1:**
1. Node 2 dials: `/ip4/143.110.139.166/tcp/30333/p2p/12D3KooWL3Q7KmTocqNGLfyz4X4mhyyPD8b4zx6MBk1qnDAT8FYs`
2. TCP connection may or may not be established
3. Noise handshake times out
4. Connection fails with timeout error
5. Retry loop continues

### Network Layer Investigation

**TCP Connectivity Test:**
- `netcat` from Node 2 to Node 1's port 30333: ✅ SUCCESS
- This confirms TCP layer is reachable
- Issue is at libp2p/application layer, not network layer

**Docker Port Mapping:**
- Both nodes: Port 30333 mapped correctly
- Docker bridge mode: May be interfering with connection tracking
- Container IPs: 172.17.0.2 (both nodes, different hosts)

**Firewall Status:**
- Node 1: UFW inactive
- Node 2: UFW inactive
- No firewall rules blocking connections

### libp2p Behavior

**Connection Establishment:**
- libp2p uses Noise for encryption handshake
- Yamux for stream multiplexing
- Identify protocol for peer information exchange

**Address Discovery:**
- libp2p discovers addresses via:
  1. Identify protocol (peer advertises its addresses)
  2. External address detection (NAT traversal)
  3. mDNS (local network only)
  4. Kademlia DHT

**Observed Address:**
- Identify protocol reports "observed address" (how peer sees us)
- This may differ from our actual listen address
- Can cause connection issues if wrong address is used

## Implemented Fixes

### Fix 1: Peer Blacklisting (v4.7.57)
- **Status:** ✅ Implemented and deployed
- **Functionality:** Node 2 now blacklists GCE VM peer
- **Result:** GCE VM connections are immediately disconnected
- **Impact:** Frees up connection slots, but doesn't fix Node 1 ↔ Node 2 connectivity

### Fix 2: Incoming Connection Tracking (v4.7.56)
- **Status:** ✅ Implemented and deployed
- **Functionality:** Track incoming connection attempts to prevent simultaneous dials
- **Result:** Should prevent race conditions, but Node 2 never sees Node 1's incoming connections
- **Impact:** Limited - doesn't address root cause

### Fix 3: Bootnode Connection Recognition (v4.7.55)
- **Status:** ✅ Implemented and deployed
- **Functionality:** Check both internal tracking and swarm's connected peers
- **Result:** Should recognize inbound connections, but Node 2 doesn't see Node 1's connection
- **Impact:** Limited - doesn't address root cause

## Recommended Solutions

### Solution 1: Fix Link-Local Address Filtering (HIGH PRIORITY)

**Problem:** 169.254.x.x addresses are not filtered, causing connection attempts to non-existent addresses.

**Fix:** Update `is_private_address()` to include 169.254.0.0/16:

```rust
fn is_private_address(addr: &Multiaddr) -> bool {
    let s = addr.to_string();
    s.contains("/ip4/10.") ||           // Docker internal networks
    s.contains("/ip4/172.1") ||         // Docker bridge (172.16-172.31)
    s.contains("/ip4/172.2") ||         // Docker bridge
    s.contains("/ip4/172.3") ||         // Docker bridge  
    s.contains("/ip4/192.168.") ||      // Private networks
    s.contains("/ip4/127.") ||          // Loopback
    s.contains("/ip4/169.254.") ||      // Link-local auto-configuration (RFC 3927) - ADD THIS
    s.contains("/ip6/::1") ||           // IPv6 loopback
    s.contains("/ip6/fe80")             // IPv6 link-local
}
```

**Expected Impact:** Eliminates ghost IP connection attempts, reduces connection timeouts.

### Solution 2: Force Public Address Advertisement (MEDIUM PRIORITY)

**Problem:** Nodes may not be advertising their public IPs correctly.

**Fix:** Explicitly add public IPs as external addresses:

```rust
// In NetworkService::new, after swarm creation
let public_ip = "143.110.139.166"; // Node 1's public IP
let public_addr: Multiaddr = format!("/ip4/{}/tcp/30333", public_ip).parse()?;
swarm.add_external_address(public_addr);
```

**Expected Impact:** Ensures peers use correct public IPs for connections.

### Solution 3: Docker Host Network Mode (MEDIUM PRIORITY)

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

**Expected Impact:** May resolve connection tracking issues, but requires testing.

### Solution 4: Enhanced Connection Logging (LOW PRIORITY)

**Problem:** Insufficient visibility into connection establishment process.

**Fix:** Add detailed logging at each stage:
- TCP connection establishment
- Noise handshake start/completion
- Yamux negotiation
- Identify protocol exchange
- Connection direction detection

**Expected Impact:** Better diagnostics, but doesn't fix the issue.

### Solution 5: Connection State Verification (MEDIUM PRIORITY)

**Problem:** Node 1's connection exists but Node 2 doesn't track it.

**Fix:** Add periodic connection state verification:
- Query swarm for all active connections
- Compare with internal peer tracking
- Log discrepancies
- Attempt to reconcile state

**Expected Impact:** May reveal why connections aren't being tracked.

## Diagnostic Commands

### Check Active Connections
```bash
# Node 1
ssh root@143.110.139.166 'ss -tnp | grep 30333'

# Node 2
ssh root@68.183.205.12 'ss -tnp | grep 30333'
```

### Check Docker Network
```bash
# Node 1
ssh root@143.110.139.166 'docker inspect coinject-node | grep -A 10 NetworkSettings'

# Node 2
ssh root@68.183.205.12 'docker inspect coinject-node | grep -A 10 NetworkSettings'
```

### Monitor Connection Attempts
```bash
# Node 2 - Watch for incoming connections
ssh root@68.183.205.12 'docker logs -f coinject-node 2>&1 | grep -E "INCOMING|143.110.139.166"'
```

### Test TCP Connectivity
```bash
# From Node 2 to Node 1
ssh root@68.183.205.12 'timeout 5 nc -zv 143.110.139.166 30333'
```

## Next Steps

1. **Immediate:** Fix link-local address filtering (Solution 1)
2. **Short-term:** Force public address advertisement (Solution 2)
3. **Medium-term:** Consider Docker host network mode (Solution 3)
4. **Long-term:** Implement connection state verification (Solution 5)

## Critical Discovery: Observed Address Mismatch

### Node 1's Observed Address
When Node 1 connects to Node 2, Node 2 reports via identify protocol:
```
Observed addr (how they see us): /ip4/143.110.139.166/tcp/38528
```

**Key Finding:** Node 2 sees Node 1's connection coming from port **38528**, not port **30333**!

This suggests:
1. Node 1 is connecting from an ephemeral port (38528)
2. Node 2 may be trying to connect back using this ephemeral port
3. But Node 1 isn't listening on port 38528 - it's listening on 30333
4. This creates a connection mismatch

### The Real Problem
- **Node 1 listens on:** `/ip4/0.0.0.0/tcp/30333`
- **Node 1 connects from:** `/ip4/143.110.139.166/tcp/38528` (ephemeral port)
- **Node 2 tries to connect to:** `/ip4/143.110.139.166/tcp/38528` (wrong port!)
- **Result:** Connection fails because Node 1 isn't listening on 38528

### Why This Happens
libp2p's identify protocol reports the "observed address" - the address the peer sees us connecting from. This is typically an ephemeral port used for outbound connections. However, when Node 2 tries to connect back, it should use Node 1's **listen address** (30333), not the observed address (38528).

**Root Cause:** libp2p may be using the observed address for reverse connections instead of the advertised listen address.

## Conclusion

The unidirectional connectivity issue is complex and involves multiple factors:

1. **Ghost IP Addresses (169.254.x.x):** ✅ FIXED in v4.7.58 - Added link-local address filtering
2. **Observed Address Mismatch:** Node 2 is trying to connect to Node 1's ephemeral port instead of listen port
3. **Docker Network Configuration:** May be interfering with connection tracking
4. **libp2p Connection State:** Not properly synchronized between nodes
5. **GCE VM Interference:** ✅ MITIGATED in v4.7.57 - Blacklist implemented

### Immediate Actions Required

1. **Fix Observed Address Usage:**
   - Ensure reverse connections use advertised listen addresses, not observed addresses
   - Filter observed addresses that don't match listen addresses
   - Prefer listen addresses over observed addresses for connection attempts

2. **Verify Port Mapping:**
   - Ensure Docker port mapping is correct
   - Verify Node 1 is actually listening on port 30333
   - Check if ephemeral ports are being used incorrectly

3. **Test Direct TCP Connection:**
   - Verify Node 2 can connect to Node 1's port 30333 directly
   - If TCP works but libp2p doesn't, issue is in libp2p configuration

## References

- RFC 3927: Link-Local Addresses
- libp2p Documentation: Connection Establishment
- libp2p Documentation: Identify Protocol and Observed Addresses
- Docker Networking: Bridge Mode
- DigitalOcean Networking: Public IP Configuration

