# Multi-Node RPC Configuration

The frontend RPC client now supports connecting to multiple blockchain nodes for improved reliability and load balancing.

## Features

- **Failover**: Automatically tries the next node if one fails
- **Load Balancing**: Round-robin distribution across nodes
- **Best Chain Selection**: For `chain_getInfo`, queries all nodes and uses the one with the highest block height
- **Backward Compatible**: Still works with a single RPC URL

## Configuration

### Single Node (Default)

```bash
# .env file
VITE_RPC_URL=http://localhost:9933
```

### Multiple Nodes

```bash
# .env file - comma-separated list
VITE_RPC_URL=http://143.110.139.166:9933,http://68.183.205.12:9933
```

## How It Works

### Read Operations (Failover)
- Tries each node in order until one succeeds
- If all nodes fail, returns an error with details from all attempts
- Rotates to the next node for load balancing

### Chain Info (Parallel Query)
- Queries all nodes in parallel
- Returns the chain info from the node with the highest block height
- Ensures you always see the most up-to-date chain state

### Write Operations (Failover)
- Transaction submissions use failover
- First successful submission is returned
- If all nodes fail, error is returned

## Example Usage

```typescript
import { rpcClient } from '@/lib/rpc-client';

// This will try all configured nodes
const chainInfo = await rpcClient.getChainInfo();

// This will use failover
const balance = await rpcClient.getBalance(address);
```

## Current Node Endpoints

- **Node 1**: `http://143.110.139.166:9933`
- **Node 2**: `http://68.183.205.12:9933`

## Testing

To test the multi-node setup:

1. Create a `.env` file with both nodes:
   ```
   VITE_RPC_URL=http://143.110.139.166:9933,http://68.183.205.12:9933
   ```

2. Start the dev server:
   ```bash
   npm run dev
   ```

3. The frontend will automatically use both nodes with failover and best-chain selection.

