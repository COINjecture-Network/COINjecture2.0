# Genesis Fork Fix - "One Node = One Chain" Problem

## Root Cause Analysis

**The Problem:** Every node boots from the same genesis file → generates block 1 with different hashes → every node thinks ITS chain is canonical → they all fork into separate universes.

### Why This Happens

1. **All Nodes Mining at Genesis** (PRIMARY ISSUE)
   - All nodes start with `--mine` flag enabled
   - Each node independently generates block 1
   - Even with deterministic genesis, block 1 has:
     - Different timestamps (`SystemTime::now()` - line 686 in `consensus/src/miner.rs`)
     - Different miner addresses (if not explicitly set)
     - Different nonces (from header mining)
   - Result: Different block hashes → immediate fork

2. **Non-Deterministic Timestamp**
   - Block timestamp uses `SystemTime::now()` which is unique per node
   - Even if only one node mines, timestamp should be deterministic for block 1

3. **Multiple Validators at Genesis**
   - No mechanism to prevent multiple nodes from mining simultaneously at genesis

## The Fix

### Fix 1: Only ONE Node Mines at Genesis (CRITICAL)

**Solution:** Only the primary bootnode (Node 1) should mine at genesis. All other nodes must start as non-mining full nodes.

**Implementation:**
- Node 1 (143.110.139.166): Keep `--mine` enabled
- Node 2 (68.183.205.12): Remove `--mine` flag (or add `--no-mine`)
- GCE VM: Remove `--mine` flag

**Why:** This ensures only one node produces block 1, creating a single canonical chain that all other nodes can sync to.

### Fix 2: Deterministic Block 1 Timestamp (IMPORTANT)

**Current Code (line 686-689 in `consensus/src/miner.rs`):**
```rust
let timestamp = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap()
    .as_secs() as i64;
```

**Fix:** For block 1 (height 1), use a deterministic timestamp:
- Option A: `genesis_timestamp + 1` (if genesis has timestamp)
- Option B: Fixed offset from genesis (e.g., `1735689601` = Jan 1, 2025 00:00:01 UTC)
- Option C: `max(parent_timestamp + 1, current_time)` (ensures monotonicity)

**Why:** Even if only one node mines, deterministic timestamp ensures block 1 hash is consistent across all nodes.

### Fix 3: Mining Wait Logic Enhancement

**Current Behavior:** Nodes wait for peers, but if multiple nodes start simultaneously at genesis, they all start mining after 10 attempts (20 seconds).

**Fix:** Add a "genesis mining lock" mechanism:
- Only the first node to reach genesis with peers should mine
- Other nodes should wait longer (e.g., 60 seconds) to ensure they sync block 1 first
- Or: Only mine if local height > 0 (never mine at genesis if other nodes exist)

## Implementation Steps

### Step 1: Update Deployment Scripts

**File: `build-and-deploy.sh`**
- Node 1: Keep `--mine`
- Node 2: Remove `--mine` or add conditional logic

**File: `deploy-gce.sh`**
- Remove `--mine` flag

### Step 2: Fix Timestamp Logic

**File: `consensus/src/miner.rs`**
- Modify `mine_block()` to use deterministic timestamp for height 1
- Use `max(parent_timestamp + 1, current_time)` for subsequent blocks

### Step 3: Add Mining Guard

**File: `node/src/service.rs`**
- Enhance mining loop to check if we're at genesis with peers
- If so, wait longer (60s) or skip mining entirely if another node is ahead

## Testing

1. **Test Single Miner at Genesis:**
   - Start Node 1 with `--mine`
   - Start Node 2 without `--mine`
   - Verify: Node 2 syncs block 1 from Node 1
   - Verify: Both nodes have same block 1 hash

2. **Test Deterministic Timestamp:**
   - Restart Node 1 multiple times
   - Verify: Block 1 hash is identical each time (if timestamp is deterministic)

3. **Test Multi-Node Sync:**
   - Start Node 1 (mining)
   - Wait for block 1
   - Start Node 2 (non-mining)
   - Start GCE VM (non-mining)
   - Verify: All nodes converge to same chain

## Nuclear Option (If All Else Fails)

If nodes are already forked, reset all non-primary nodes:

```bash
# On Node 2 and GCE VM:
docker stop coinject-node
rm -rf /data/chain.db /data/state.db
docker start coinject-node  # Without --mine flag
```

This forces them to resync from Node 1's canonical chain.

## Prevention

1. **Documentation:** Add clear docs that only ONE node should mine at genesis
2. **Config Validation:** Add warning if multiple nodes have `--mine` and no existing chain
3. **Genesis Mining Lock:** Implement automatic lock mechanism in code

