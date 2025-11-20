# Current Issues and Status

Last Updated: 2025-11-20

## Critical Issues

### 1. Gossipsub Mesh Formation Not Completing
**Status**: Investigating  
**Severity**: High  
**Impact**: Nodes cannot broadcast messages, causing "InsufficientPeers" errors

**Symptoms**:
- TCP/libp2p connections establish successfully
- Peers are added to gossipsub as explicit peers
- Mesh does not form, causing all broadcasts to fail with "InsufficientPeers"

**Root Cause Analysis**:
- Gossipsub mesh formation is asynchronous and occurs during heartbeats
- Current configuration: `mesh_outbound_min=1`, `mesh_n_low=2`, `mesh_n=2`, `mesh_n_high=4`
- With only 2 peers, `mesh_n_low=2` may be too high - mesh needs at least 2 peers but we only have 2 total
- Both peers must be subscribed to the same topics for mesh to form

**Attempted Fixes**:
1. ✅ Added bootnode address to Kademlia routing table before dialing
2. ✅ Configured gossipsub mesh parameters for small networks
3. ✅ Improved error handling in broadcast_status
4. ⚠️ Mesh still not forming after 60+ seconds

**Next Steps**:
- [ ] Verify both nodes are subscribed to the same topics (check chain_id matches)
- [ ] Consider lowering `mesh_n_low` to 1 (but must satisfy inequality)
- [ ] Add explicit mesh formation logging to track when peers join mesh
- [ ] Test with 3+ nodes to see if mesh forms with more peers

---

## Medium Priority Issues

### 2. Hugging Face Dataset Uploads Not Appearing
**Status**: Partially Fixed  
**Severity**: Medium  
**Impact**: Consensus block data not visible in Hugging Face dataset

**Symptoms**:
- Nodes are configured with Hugging Face credentials
- Blocks are being mined/validated
- No data appears in https://huggingface.co/datasets/COINjecture/NP_Solutions

**Root Cause Analysis**:
- API endpoint updated to commit endpoint (✅ Fixed)
- Upload method changed to JSON with base64 encoding (✅ Fixed)
- Logging changed to eprintln! for better visibility (✅ Fixed)
- **Remaining Issue**: Requires successful mesh formation for blocks to be processed and uploaded

**Attempted Fixes**:
1. ✅ Updated to new commit API endpoint
2. ✅ Fixed multipart form data to JSON body
3. ✅ Changed logging to eprintln!
4. ⚠️ Still blocked by mesh formation issue

**Next Steps**:
- [ ] Verify uploads work once mesh is formed
- [ ] Test manual upload to verify API credentials and endpoint
- [ ] Add explicit upload success/failure logging

---

### 3. Node2 Startup Inconsistency
**Status**: Monitoring  
**Severity**: Medium  
**Impact**: Second node may not start automatically

**Symptoms**:
- Node2 process not found when checking status
- Requires manual restart

**Root Cause Analysis**:
- May be related to bootnode PeerId changing on bootstrap node restart
- Script may fail silently

**Next Steps**:
- [ ] Add startup logging to node2 script
- [ ] Implement automatic PeerId detection/update
- [ ] Add health check monitoring

---

## Low Priority Issues

### 4. Mining Disabled During Troubleshooting
**Status**: Intentional  
**Severity**: Low  
**Impact**: No new blocks being generated

**Note**: Mining was intentionally disabled to troubleshoot P2P connection issues. Should be re-enabled once mesh formation is resolved.

---

## Configuration Notes

### Current Gossipsub Configuration
```rust
mesh_outbound_min(1)  // Minimum outbound peers in mesh
mesh_n_low(2)         // Minimum mesh size before trying to add more
mesh_n(2)             // Desired mesh size (for 2-peer network)
mesh_n_high(4)        // Maximum mesh size before pruning
```

**Issue**: With only 2 peers total, `mesh_n_low=2` means both peers must be in the mesh. This may prevent initial mesh formation if one peer hasn't joined yet.

**Potential Fix**: Lower to `mesh_n_low=1`, but must ensure `mesh_outbound_min <= mesh_n_low` (currently 1 <= 1 would work).

### Network Configuration
- **Bootstrap Node**: 143.110.139.166:30333
- **Node2**: 68.183.205.12:30333
- **Chain ID**: coinject-network-b (default)
- **Topics**: `coinject-network-b/blocks`, `coinject-network-b/transactions`, `coinject-network-b/status`

---

## Testing Checklist

- [ ] Verify both nodes start successfully
- [ ] Confirm TCP connection established (check logs for "Connection established")
- [ ] Verify both nodes subscribe to same topics (check chain_id)
- [ ] Wait 60 seconds for mesh formation
- [ ] Check for "InsufficientPeers" errors (should stop after mesh forms)
- [ ] Verify status broadcasts succeed
- [ ] Test block broadcasting
- [ ] Verify Hugging Face uploads appear in dataset
- [ ] Re-enable mining and verify blocks propagate

---

## Related Files

- `network/src/protocol.rs` - P2P network and gossipsub configuration
- `huggingface/src/client.rs` - Hugging Face API client
- `node/src/service.rs` - Node service and block processing
- `node/src/config.rs` - Node configuration

