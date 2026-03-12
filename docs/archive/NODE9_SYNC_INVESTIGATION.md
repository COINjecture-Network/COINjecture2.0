# Node 9 Sync Issue Investigation Report

**Date**: December 10, 2025  
**Node**: 143.110.139.166 (Node 9)  
**Status**: 🔴 **STUCK ON FORK**

---

## Problem Summary

Node 9 is **stuck at height 935** and cannot progress to block 936. The node is receiving block 936 repeatedly from peers, but validation consistently fails with **"Invalid previous hash"**. The reorganization logic detects a fork but cannot find a common ancestor to reconnect.

---

## Current State

### Node 9 Status
- **Current Height**: 935
- **Current Hash**: `00009d772b89a2fc`
- **Peer Height**: 4,511+ blocks
- **Blocks Behind**: 3,576 blocks
- **Status**: Mining paused (waiting for sync)
- **Runtime**: ~1h 38m

### Node 2 Status (Reference)
- **Current Height**: 4,481+ blocks
- **Status**: ✅ Synced and operational
- **Same Network**: Yes (connected to same peers)

---

## Root Cause Analysis

### 1. Chain Divergence at Height 935

**Evidence**:
- Node 9 has block 935 with hash `00009d772b89a2fc`
- Node 9 successfully validated and stored block 935
- All received block 936 instances fail validation: "Invalid previous hash"
- Reorganization check: "Buffered blocks have no common ancestor with current chain"

**Interpretation**:
- Node 9's block 935 hash (`00009d772b89a2fc`) does **NOT match** what the network expects
- Peers are sending block 936 with a `prev_hash` that doesn't match Node 9's block 935
- This indicates Node 9 is on a **different fork** than the network

### 2. Fork Detection

**Log Evidence**:
```
⚠️  Fork detected! Peer is ahead (height 4511) and we're on a fork. Requesting full chain for reorganization...
🔍 Reorganization check: Buffered blocks at height 4507 have no common ancestor with current chain
```

**What This Means**:
- Node 9 has 3,571+ blocks buffered (heights 1024-4507)
- None of these buffered blocks connect to Node 9's current chain
- The reorganization logic is scanning stored blocks from 936-1935 but finding gaps
- **No common ancestor found** between Node 9's chain and peer chains

### 3. Block Validation Failure

**Pattern**:
```
📥 Received block 936 from PeerId(...) (sync_block: true)
❌ Block validation failed: Invalid previous hash
```

**Repeated**: Block 936 is received and rejected repeatedly (dozens of times)

**What This Means**:
- Block 936's `prev_hash` field doesn't match Node 9's block 935 hash
- This is the **symptom**, not the cause
- The **root cause** is that Node 9's block 935 is on a different fork

---

## Possible Causes

### Hypothesis 1: Database Corruption (Most Likely)
- Node 9's `chain.db` may have corrupted block 935
- Block 935 was stored with incorrect hash
- Subsequent blocks cannot connect

**Evidence**:
- Database files last modified: Dec 10 00:25 (recent)
- Node started around 00:25 (same time)
- Block 935 was accepted but may have wrong hash stored

### Hypothesis 2: Fork at Earlier Height
- Node 9 forked at some height < 935
- Block 935 is valid on Node 9's fork but not on main chain
- Network consensus is on different fork

**Evidence**:
- Reorganization check scanning from 936-1935 suggests earlier divergence
- No common ancestor found in buffered blocks

### Hypothesis 3: Genesis/Initialization Issue
- Node 9 may have started with different genesis block
- Or loaded from different chain state
- Chain divergence from genesis

**Evidence**:
- Genesis hash: `Hash(4a80254b4a48e867)` (matches expected)
- But chain state may have diverged later

---

## Diagnostic Commands Run

1. ✅ Process status - Node 9 running, consuming resources
2. ✅ Port status - RPC/P2P ports listening
3. ✅ Log analysis - Found validation failures and fork detection
4. ✅ Database status - Files exist and are recent
5. ✅ Chain state - Stuck at height 935, hash `00009d772b89a2fc`
6. ✅ Reorganization attempts - Active but failing

---

## Recommended Solutions

### Solution 1: Reset Node 9 Chain Database (Recommended)

**Action**: Delete chain database and resync from network

**Steps**:
```bash
# Stop Node 9
kill 1099900

# Backup current state (optional)
cp -r /opt/coinjecture/node9-data /opt/coinjecture/node9-data-backup

# Remove chain database (keep state.db for account balances)
rm /opt/coinjecture/node9-data/chain.db

# Restart Node 9
# It will resync from peers
```

**Risk**: Low - State database preserved (accounts, balances)
**Time**: Depends on network sync speed (~hours for 4,500 blocks)

### Solution 2: Copy Chain from Node 2

**Action**: Copy chain database from working Node 2

**Steps**:
```bash
# Stop Node 9
kill 1099900

# Copy chain from Node 2
cp /opt/coinjecture/node2-data/chain.db /opt/coinjecture/node9-data/chain.db

# Restart Node 9
```

**Risk**: Medium - May have compatibility issues if nodes use different configs
**Time**: Fast (copy operation)

### Solution 3: Full Reset (Nuclear Option)

**Action**: Delete all Node 9 data and resync from scratch

**Steps**:
```bash
# Stop Node 9
kill 1099900

# Backup everything
mv /opt/coinjecture/node9-data /opt/coinjecture/node9-data-backup-$(date +%Y%m%d)

# Restart Node 9 (will create fresh databases)
```

**Risk**: High - Loses all local state
**Time**: Longest (full resync from genesis)

---

## Immediate Actions

### Check Block 935 Hash Comparison

**Command to run**:
```bash
# On Node 2 (working node)
# Check what hash Node 2 has at height 935

# On Node 9 (stuck node)  
# Compare with Node 9's hash: 00009d772b89a2fc
```

**Expected Result**:
- If hashes match → Different issue (block 936 problem)
- If hashes differ → Confirms fork (Solution 1 needed)

### Check Reorganization Progress

**Monitor**:
```bash
tail -f /opt/coinjecture/node9.log | grep -E 'reorganization|common ancestor|Fork'
```

**What to Look For**:
- "Found common ancestor" → Reorganization may succeed
- "No common ancestor" → Reset needed (Solution 1)

---

## Prevention

### Recommendations

1. **Regular Chain Validation**: Add periodic chain integrity checks
2. **Backup Strategy**: Regular backups of chain.db before major updates
3. **Fork Detection**: Improve reorganization logic to handle earlier forks
4. **Monitoring**: Alert when nodes fall behind or detect forks
5. **Database Integrity**: Add checksums or validation for stored blocks

---

## Next Steps

1. **Immediate**: Compare block 935 hashes between Node 2 and Node 9
2. **Short-term**: Implement Solution 1 (reset chain database)
3. **Medium-term**: Improve fork detection and reorganization logic
4. **Long-term**: Add chain validation and monitoring

---

## Status

🔴 **CRITICAL**: Node 9 is on a fork and cannot sync. Manual intervention required.

**Recommended Action**: Reset chain database (Solution 1) to allow resync from network.

