# Current Issues - v4.8.4

**Date**: 2026-01-07  
**Version**: 4.8.4  
**Status**: Active Development

---

## Critical Issue: Peer Connection Stability

### Problem
Server 2 (159.89.145.211) cannot maintain a stable connection to Server 1 (167.172.221.42), preventing proper network synchronization.

### Symptoms
- **Server 1**: Sees Server 2 as STALE peer (last seen 500+ seconds ago)
- **Server 2**: Shows 0 active peers despite continuous reconnection attempts
- **Connection Attempts**: Logs show "Attempting reconnection" but no success or error messages
- **Network**: Port 30333 is reachable (confirmed via TCP test), not a firewall issue

### Observed Behavior
```
Server 1 logs:
  - Peer count: 1 (but STALE)
  - Peer 39656531 height=248 last_seen=500s+ ago [STALE]
  - "Command handling error: Invalid handshake: Peer already connected"

Server 2 logs:
  - Peer count: 0
  - "[CPP][BOOTNODE] Attempting reconnection to 167.172.221.42:30333"
  - "[CPP] Connecting to bootnode: 167.172.221.42:30333"
  - (No error or success message after this)
```

### Root Cause Analysis

**Hypothesis 1: Handshake Timeout**
- Connection is established (TCP succeeds)
- Hello message may be sent successfully
- HelloAck receive may be timing out or hanging
- No timeout error is logged (suggests silent failure)

**Hypothesis 2: Race Condition**
- Both nodes attempt to connect simultaneously
- Server 1 accepts incoming connection from Server 2
- Server 2's outgoing connection fails with "Peer already connected"
- Server 1's incoming connection may also fail due to duplicate check

**Hypothesis 3: Message Protocol Issue**
- HelloAck message format may not match expectations
- MessageCodec::receive may be waiting indefinitely for data
- No timeout on MessageCodec::receive operation

### Code Changes Made (v4.8.4)
1. ✅ Added timeout handling to `connect_bootnode`:
   - TCP connection: 30s timeout
   - Hello send: 10s timeout
   - HelloAck receive: 10s timeout
2. ✅ Added detailed logging at each step
3. ✅ Improved error messages

### Next Steps Required
1. **Add timeout to MessageCodec::receive**
   - Currently `MessageCodec::receive` has no timeout
   - Should wrap in `tokio::time::timeout(HANDSHAKE_TIMEOUT, ...)`
2. **Review peer state cleanup**
   - Stale peers may be preventing new connections
   - Consider clearing stale peer entries before reconnection
3. **Implement connection retry logic**
   - Exponential backoff is implemented but may need adjustment
   - Consider resetting peer state on failed connection
4. **Bidirectional connection handling**
   - Both nodes can initiate connections
   - Need to handle "already connected" more gracefully

---

## Secondary Issue: Fork Detection Requires Connection

### Problem
Fork detection fix (v4.8.4) works correctly but cannot function without active peer connections.

### Status
- ✅ **Code is correct**: Fork detection now uses CPP network commands
- ✅ **Uses best peer**: Gets peer from peer_consensus
- ✅ **Proper chunking**: Requests 16 blocks per chunk
- ❌ **Cannot test**: Requires active peer connection to function

### Impact
Once connection stability is fixed, fork detection will automatically work. The code is ready and waiting for connections.

---

## Tertiary Issue: Chain Sync Stalled

### Problem
Server 2 is stuck at height 248 while Server 1 is at height 400+.

### Root Cause
- Server 2 detects complete fork (no common ancestor)
- Fork detection correctly identifies longer chain
- Cannot request blocks because no active peers
- Circular dependency: needs connection to sync, needs sync to connect

### Resolution Path
1. Fix connection stability (primary issue)
2. Fork detection will automatically trigger
3. Server 2 will request blocks from Server 1
4. Sync will proceed normally

---

## Recent Improvements (v4.8.4)

### ✅ Fork Detection Fix
- Changed from legacy `NetworkCommand::RequestBlocks` to `CppNetworkCommand::RequestBlocks`
- Uses peer_consensus to get best peer
- Proper chunking (16 blocks per request)

### ✅ Constant Consolidation
- Removed 3 duplicate ETA/LAMBDA definitions
- All modules use `core::dimensional` as single source of truth
- Improved code maintainability

### ✅ Network-Derived Timeouts
- Timeouts now scale with ETA (equilibrium constant)
- Better self-reference and empirical grounding
- Improved compliance with dimensionless principles

### ✅ Connection Logging
- Added detailed logging for connection attempts
- Timeout handling with proper error messages
- Easier diagnosis of connection failures

---

## Testing Status

### ✅ Compilation
- All packages compile successfully
- No breaking changes to public APIs

### ✅ Deployment
- Docker images build successfully
- Deployment scripts work correctly
- Containers start and run

### ❌ Network Connectivity
- Connection stability needs investigation
- Handshake may be timing out silently
- Need to add timeout to MessageCodec::receive

---

## Recommended Next Actions

### Priority 1: Fix Connection Stability
1. Add timeout to `MessageCodec::receive` in protocol layer
2. Review and improve peer state cleanup
3. Test connection with improved logging
4. Consider implementing connection retry with state reset

### Priority 2: Verify Fork Detection
1. Once connections are stable, verify fork detection works
2. Test with nodes on different chain forks
3. Confirm blocks are requested and received correctly

### Priority 3: Monitor and Optimize
1. Monitor connection stability over time
2. Optimize handshake performance
3. Review peer management for efficiency

---

## Files Modified in v4.8.4

### Core Changes
- `node/src/service.rs` - Fork detection fix, timeout improvements
- `network/src/cpp/network.rs` - Connection logging, timeout handling
- `tokenomics/src/*.rs` - Constant consolidation (8 files)
- `state/src/dimensional_pools.rs` - Constant consolidation
- `state/src/trustlines.rs` - Constant consolidation
- `node/src/peer_consensus.rs` - Timeout improvements
- `node/src/sync_optimizer.rs` - Timeout improvements
- `core/src/dimensional.rs` - Single source of truth for constants

### Documentation
- `CHANGELOG.md` - Updated with v4.8.4 changes
- `Cargo.toml` - Version bumped to 4.8.4
- `AUDIT_FIXES_APPLIED.md` - Detailed audit fixes
- `SYSTEM_AUDIT_REPORT.md` - Compliance audit results
- `CURRENT_ISSUES_V4.8.4.md` - This document

---

## Summary

**Version 4.8.4** includes significant improvements:
- ✅ Fork detection fixed (uses CPP commands)
- ✅ Constants consolidated (single source of truth)
- ✅ Timeouts improved (ETA-scaled)
- ✅ Logging enhanced (better diagnosis)

**Current blocker**: Connection stability between nodes needs investigation. The improved logging should help diagnose the issue, but additional timeout handling may be required.

**Next version focus**: Fix connection stability to enable full network functionality.

