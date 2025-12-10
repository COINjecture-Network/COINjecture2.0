# GCE VM Sync Issue Assessment

**Date:** 2025-12-10  
**Version:** v4.7.47  
**Issue:** GCE VM not receiving blocks from Node 2 despite being connected

## Current Status

### Node Heights
- **Node 1 (143.110.139.166):** Height ~4,838 (syncing, 3,916 blocks behind)
- **Node 2 (68.183.205.12):** Height ~8,754 (mining paused, serving blocks)
- **GCE VM (35.184.253.150):** Height 45 (stuck, waiting for full chain from genesis)

### Connection Status
- **GCE VM:** 3 peers connected (including Node 1 and Node 2)
- **Node 2:** Connection timeouts when trying to dial GCE VM
- **Port 30333:** Open and accessible (nc test succeeded)
- **Firewall:** Rules configured correctly (`coinject-p2p` allows port 30333)

## Root Cause Analysis

### Issue 1: Connection Timeouts
**Problem:** Node 2 is experiencing connection timeouts when trying to dial GCE VM:
```
Error: Transport([(/ip4/35.184.253.150/tcp/30333/p2p/12D3KooWNazjoWWEF8ZX1LczwyJhiUBd5PM31HzkHAFKQarByAzP, Other(Custom { kind: Other, error: Timeout }))])
```

**Possible Causes:**
1. **NAT/Firewall Issues:** GCE VM might be behind NAT, making it difficult for Node 2 to establish outbound connections
2. **Connection Direction:** GCE VM can connect to Node 2, but Node 2 cannot initiate connections to GCE VM
3. **Network Routing:** GCE's network configuration might not allow incoming connections on port 30333 despite firewall rules

### Issue 2: Full Chain Request Not Being Served
**Problem:** GCE VM requested full chain from genesis (0 to 8,557+), but blocks are not arriving.

**Analysis:**
- GCE VM correctly detected complete fork and requested full chain
- Node 2's code handles `BlocksRequested` events and serves blocks via `SendSyncBlock`
- However, Node 2 is only serving blocks to Node 1 (PeerId: `12D3KooWL3Q7KmTocqNGLfyz4X4mhyyPD8b4zx6MBk1qnDAT8FYs`)
- No evidence of blocks being served to GCE VM (PeerId: `12D3KooWNazjoWWEF8ZX1LczwyJhiUBd5PM31HzkHAFKQarByAzP`)

**Possible Causes:**
1. **Gossipsub Routing:** The `GetBlocks` request might not be reaching Node 2 due to gossipsub mesh routing
2. **Peer Not in Mesh:** GCE VM might not be in Node 2's gossipsub mesh, so broadcast messages aren't reaching it
3. **Request Lost:** The request might be getting lost in the network layer before reaching Node 2's handler

### Issue 3: One-Way Connection
**Observation:** GCE VM is receiving blocks from itself (blocks 24-28), suggesting it's in a loop or only connected to itself.

**Evidence:**
- Node 2 logs show: `📥 Received block 28 from PeerId("12D3KooWNazjoWWEF8ZX1LczwyJhiUBd5PM31HzkHAFKQarByAzP")`
- This means GCE VM is sending blocks TO Node 2, but Node 2 isn't sending blocks back

## Technical Details

### Network Protocol Flow
1. GCE VM detects fork → requests full chain (0 to 8557+)
2. Request sent via `NetworkMessage::GetBlocks { from: 0, to: 8557, request_id }`
3. Request published to gossipsub `blocks` topic
4. Node 2 should receive `NetworkEvent::BlocksRequested`
5. Node 2 should serve blocks via `SendSyncBlock` with unique request_id

### Current Implementation
- `BlocksRequested` handler in `service.rs` (line 1534) correctly serves blocks
- Uses `SendSyncBlock` with unique request_id to bypass gossipsub deduplication
- Serves blocks sequentially from `from_height` to `to_height`

## Recommendations

### Immediate Fixes

1. **Force Direct Connection:**
   - Ensure GCE VM can establish bidirectional connections
   - Check if GCE VM needs to initiate connection to Node 2 (not just receive)

2. **Verify Gossipsub Mesh:**
   - Check if GCE VM is in Node 2's gossipsub mesh
   - If not, force mesh subscription or use direct peer-to-peer messaging

3. **Add Request Retry Logic:**
   - If full chain request fails, retry with smaller chunks (e.g., 1000 blocks at a time)
   - Implement exponential backoff for failed requests

4. **Add Connection Diagnostics:**
   - Log when blocks are requested but not served
   - Track which peers are in the gossipsub mesh
   - Monitor connection state for each peer

### Long-term Improvements

1. **Direct Peer Messaging:**
   - For large sync requests (full chain), use direct request-response instead of gossipsub
   - Implement dedicated sync protocol for genesis-to-tip sync

2. **Connection Health Monitoring:**
   - Track connection quality per peer
   - Automatically retry failed connections
   - Detect and report one-way connections

3. **Chunked Sync:**
   - Break large sync requests into smaller chunks
   - Process chunks in parallel from multiple peers
   - Verify chunk integrity before applying

## Code Changes Needed

1. **Add mesh peer tracking** in `network/src/protocol.rs`
2. **Implement direct peer messaging** for large sync requests
3. **Add connection health checks** to detect one-way connections
4. **Implement chunked sync** for full chain requests

## Testing

After fixes:
1. Verify GCE VM can receive blocks from Node 2
2. Test full chain sync from genesis
3. Verify bidirectional connectivity
4. Test with multiple peers serving blocks

