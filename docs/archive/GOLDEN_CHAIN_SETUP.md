# Golden Chain Setup - Establishing Canonical Chain

## Overview

Following the whitepaper's critical equilibrium principles (Appendix D.6), we establish a "golden chain" by having ONE node (the golden full node) build initial history first. This creates a chain with non-zero cumulative work scores, giving it higher weight for other nodes to converge on.

## Step 1: Establish the Golden Chain with Solo Mining

### Current Configuration
- **Golden Node**: Node 1 (143.110.139.166) - PRIMARY BOOTNODE
- **Ports**: P2P: 30333, RPC: 9933, Metrics: 9090
- **Mining**: ENABLED (only on Node 1)

### Golden Node Setup (Node 1)
Node 1 is already configured as the golden node:
- ✅ Mining enabled (`--mine` flag)
- ✅ Primary bootnode
- ✅ Deterministic block 1 timestamp (prevents forks)

### Verify Golden Node is Building Chain
```bash
# Check Node 1 is mining and building blocks
curl -X POST -H 'Content-Type: application/json' \
  --data '{"jsonrpc":"2.0","method":"chain_getInfo","params":[],"id":1}' \
  http://143.110.139.166:9933

# Monitor mining activity
ssh root@143.110.139.166 "docker logs -f coinject-node | grep -E 'Mining|Mined|block'"
```

**Expected**: Node 1 should be actively mining blocks, showing:
- Increasing heights
- Commitments (H(problem params || salt || H(solution)))
- Solution reveals
- Work scores validating via polynomial-time checks (NP asymmetry)

This builds |ψ(t)| decaying exponentially (proof eq. 13), giving it higher "weight" for others to converge on.

## Step 2: Reset and Sync Other Nodes

### Node 2 (Full Node - No Mining)

Node 2 should:
1. **Stop and reset chain data** (but keep genesis):
   ```bash
   ssh root@68.183.205.12 "docker stop coinject-node && docker rm coinject-node"
   ssh root@68.183.205.12 "rm -rf /var/lib/docker/volumes/coinject-data/_data/chain.db"
   # Keep state.db for account balances, but reset chain
   ```

2. **Restart as full node** (no mining):
   ```bash
   docker run -d \
     --name coinject-node \
     --restart unless-stopped \
     -p 30333:30333 \
     -p 9933:9933 \
     -p 9090:9090 \
     -v coinject-data:/data \
     coinjecture-netb:latest \
     --data-dir /data \
     --p2p-addr /ip4/0.0.0.0/tcp/30333 \
     --rpc-addr 0.0.0.0:9933 \
     --metrics-addr 0.0.0.0:9090 \
     --hf-token "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" \
     --hf-dataset-name "COINjecture/v5" \
     --bootnodes "/ip4/143.110.139.166/tcp/30333/p2p/12D3KooWL3Q7KmTocqNGLfyz4X4mhyyPD8b4zx6MBk1qnDAT8FYs"
   ```

   **Note**: No `--mine` flag - Node 2 syncs from Node 1

### GCE VM (Archive Node - No Mining)

GCE VM should:
1. **Reset chain data** (if needed)
2. **Start as archive node**:
   ```bash
   docker run -d \
     --name coinject-node \
     --restart unless-stopped \
     -p 30333:30333 \
     -p 9933:9933 \
     -p 9090:9090 \
     -v coinject-data:/data \
     gcr.io/coinjecture/coinject-node:v4.7.48-amd64 \
     --data-dir /data \
     --node-type archive \
     --p2p-addr /ip4/0.0.0.0/tcp/30333 \
     --rpc-addr 0.0.0.0:9933 \
     --metrics-addr 0.0.0.0:9090 \
     --hf-token "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" \
     --hf-dataset-name "COINjecture/v5" \
     --bootnodes "/ip4/143.110.139.166/tcp/30333/p2p/12D3KooWL3Q7KmTocqNGLfyz4X4mhyyPD8b4zx6MBk1qnDAT8FYs" \
     --bootnodes "/ip4/68.183.205.12/tcp/30333/p2p/12D3KooWQwpXp7NJG9gMVJMFH7oBfYQizbtPAB3RfRqxyvQ5WZfv"
   ```

## Step 3: Verify Convergence Using Protocol Rules

### Monitor All Nodes

**Check Connections:**
```bash
# Node 1 (Golden) - should show peers connecting
ssh root@143.110.139.166 "docker logs coinject-node | grep -E 'peer|connected|dialing' | tail -10"

# Node 2 - should show connection to Node 1
ssh root@68.183.205.12 "docker logs coinject-node | grep -E 'peer|connected|dialing|bootnode' | tail -10"
```

**Check Sync Status:**
```bash
# Compare heights - should converge
curl -X POST -H 'Content-Type: application/json' \
  --data '{"jsonrpc":"2.0","method":"chain_getInfo","params":[],"id":1}' \
  http://143.110.139.166:9933 | grep best_height

curl -X POST -H 'Content-Type: application/json' \
  --data '{"jsonrpc":"2.0","method":"chain_getInfo","params":[],"id":1}' \
  http://68.183.205.12:9933 | grep best_height
```

**Expected Behavior:**
- ✅ Node 2 connects to Node 1 (golden node)
- ✅ Node 2 syncs blocks from Node 1
- ✅ Node 2 verifies solutions (fast O(n) per Appendix A)
- ✅ Node 2 computes cumulative work (proof section 5 normalization)
- ✅ Node 2 reorganizes to golden chain if its work > local (tiebreaker per whitepaper section 8)
- ✅ No "fork detected" after initial sync
- ✅ Oracle metric (Appendix D.5, ∆≈0.231 at equilibrium) implies optimal regime once synced

### Verify Work Score Accumulation

The golden chain should have:
- **Non-zero cumulative work**: Sum of all block work scores
- **Self-referenced**: Work scores measured against network's own state
- **Empirical**: Derived from actual solve times, not assumptions

## Step 4: Scale Up Safely

Once nodes are synced:

1. **Add more persistent peers** as nodes join
2. **For testnet**: Submit user problems (Appendix C) via bounties to test aggregation (ANY/BEST modes)
3. **Monitor convergence**: Nodes should converge exponentially fast (time constant τ=√2 ≈1.414 units, proof section 8.2)

## Troubleshooting

### If Nodes Don't Converge

1. **Check epoch salt**: Derived from parent hash, prevents pre-mining per section 5
2. **Verify work score calculation**: Ensure all nodes use same formula from Appendix B
3. **Check fork choice logic**: Implement as per proof Theorem 13 for perturbation resilience
   - Compare chains by cumulative ∑ work_scores
   - Normalized as in proof section 5

### Code Locations

- **Fork choice**: `node/src/service.rs` - `attempt_reorganization_if_longer_chain()`
- **Work score**: `consensus/src/work_score.rs`
- **Chain comparison**: `node/src/service.rs` - `check_and_reorganize_chain()`

## Current Status

- ✅ Node 1: Golden node (mining enabled)
- ✅ Node 2: Full node (mining disabled, syncing from Node 1)
- ⏳ GCE VM: Archive node (configured, needs deployment)

