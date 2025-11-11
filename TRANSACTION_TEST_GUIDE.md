# Transaction Broadcasting Test Guide

This guide demonstrates institutional-grade transaction broadcasting and inclusion testing for Network B.

## Prerequisites

You've already created wallet accounts:
- **Alice**: `3d49786cd6249a3689ed38fbd033c7871974d7ea596af8c9e5d3657022adc4bf`
- **Bob**: `9de41d9ba4506045e3490e98ffe383471fbe78932a8554701e80c7ab73055507`
- **Faucet**: `e0e2547212b9273e006f7731340c38c0cb6c81fa5b5bd7c385212d76adf0afcd`

## Step-by-Step Test Procedure

### Step 1: Clean Environment

1. **Close** all running PowerShell node windows (press Ctrl+C in each)
2. Run cleanup:
   ```powershell
   .\clean-testnet.bat
   ```

### Step 2: Start Faucet Node

This node will mine with the faucet address, earning rewards we can use for testing.

```powershell
.\test-faucet-node.ps1
```

Wait for it to mine **at least 3 blocks** (~90 seconds). You'll see:
```
🎉 Mined new block 1!
🎉 Mined new block 2!
🎉 Mined new block 3!
```

### Step 3: Start Node 2 (in a new PowerShell window)

```powershell
.\test-node2.ps1
```

Watch for:
- Peer discovery messages
- Status sync messages
- Block synchronization

### Step 4: Check Faucet Balance

```powershell
.\target\release\coinject-wallet.exe --rpc http://127.0.0.1:9933 account balance e0e2547212b9273e006f7731340c38c0cb6c81fa5b5bd7c385212d76adf0afcd
```

You should see mining rewards (varies based on work done).

### Step 5: Send Transaction (Faucet → Alice)

```powershell
.\target\release\coinject-wallet.exe --rpc http://127.0.0.1:9933 transaction send --from faucet --to 3d49786cd6249a3689ed38fbd033c7871974d7ea596af8c9e5d3657022adc4bf --amount 1000 --fee 10
```

### Step 6: Watch Transaction Propagation

In **both node windows**, you should see:
```
📨 Received transaction <hash> from <PeerId>
✅ Added transaction <hash> to pool
```

### Step 7: Wait for Block Inclusion

Within ~30 seconds, you'll see:
```
⛏️  Mining block X...
   Including 1 transactions
🎉 Mined new block X!
```

### Step 8: Verify Transaction Included

Both nodes should show:
```
📥 Received block X from <PeerId>
✅ Block X accepted and applied to chain
```

### Step 9: Check Alice's Balance

```powershell
.\target\release\coinject-wallet.exe --rpc http://127.0.0.1:9933 account balance 3d49786cd6249a3689ed38fbd033c7871974d7ea596af8c9e5d3657022adc4bf
```

Should show: **1000 tokens**

### Step 10: Send Another Transaction (Alice → Bob)

```powershell
.\target\release\coinject-wallet.exe --rpc http://127.0.0.1:9933 transaction send --from alice --to 9de41d9ba4506045e3490e98ffe383471fbe78932a8554701e80c7ab73055507 --amount 500 --fee 5
```

### Step 11: Verify Final Balances

**Bob's balance:**
```powershell
.\target\release\coinject-wallet.exe --rpc http://127.0.0.1:9933 account balance 9de41d9ba4506045e3490e98ffe383471fbe78932a8554701e80c7ab73055507
```
Should show: **500 tokens**

**Alice's balance:**
```powershell
.\target\release\coinject-wallet.exe --rpc http://127.0.0.1:9933 account balance 3d49786cd6249a3689ed38fbd033c7871974d7ea596af8c9e5d3657022adc4bf
```
Should show: **495 tokens** (1000 - 500 - 5 fee)

## Success Criteria

✅ Transaction broadcasts to all connected peers
✅ Transactions enter the mempool on all nodes
✅ Transactions get included in mined blocks
✅ Balances update correctly on all nodes
✅ Nonces increment properly (replay protection)
✅ Fees are deducted correctly

## Architecture Proven

This test demonstrates:
- **P2P Transaction Gossip**: Same infrastructure as block broadcasting
- **Sequential Block Buffering**: Handles out-of-order message delivery
- **State Synchronization**: All nodes converge to same state
- **Mempool Management**: Transactions removed after inclusion
- **Account State**: Balances and nonces managed correctly

## Next Steps

This completes the institutional-grade P2P testnet validation! The network is ready for:
- Backend integration with COINjecture
- Additional features (smart contracts, staking, etc.)
- Mainnet deployment preparation
