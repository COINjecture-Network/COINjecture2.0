# Bootstrap Node Setup Guide

This guide explains how to set up and use a P2P bootstrap node for the COINjecture Network B blockchain.

## What is a Bootstrap Node?

A **bootstrap node** (or bootnode) is a well-known peer in the P2P network that other nodes can connect to for initial peer discovery. Bootstrap nodes help new nodes join the network by:

1. Providing an initial connection point
2. Helping with peer discovery through Kademlia DHT
3. Relaying blocks and transactions via GossipSub
4. Broadcasting network status information

## Quick Start

### Step 1: Build the Project

```bash
cargo build --release
```

### Step 2: Start the Bootstrap Node

**Windows (PowerShell):**
```powershell
.\bootstrap-node.ps1
```

**Windows (Batch):**
```cmd
bootstrap-node.bat
```

**Linux/Mac:**
```bash
./target/release/coinject \
  --data-dir ./bootstrap \
  --p2p-addr "/ip4/0.0.0.0/tcp/30333" \
  --rpc-addr "127.0.0.1:9933" \
  --difficulty 3 \
  --block-time 30
```

### Step 3: Note the PeerId

When the bootstrap node starts, look for this line in the output:

```
Network node PeerId: 12D3KooWExamplePeerIdHere...
```

**Copy this PeerId** - you'll need it to construct the bootnode address for other nodes.

## Connecting Other Nodes to the Bootstrap Node

### Multiaddr Format

Bootstrap nodes are specified using libp2p multiaddr format:

```
/ip4/<IP_ADDRESS>/tcp/<PORT>/p2p/<PEER_ID>
```

### Example Multiaddrs

**Local testing (same machine):**
```
/ip4/127.0.0.1/tcp/30333/p2p/12D3KooWExamplePeerIdHere...
```

**LAN (local network):**
```
/ip4/192.168.1.100/tcp/30333/p2p/12D3KooWExamplePeerIdHere...
```

**Remote/Public (internet):**
```
/ip4/203.0.113.50/tcp/30333/p2p/12D3KooWExamplePeerIdHere...
```

### Method 1: Using PowerShell Example Script

1. Edit [connect-to-bootstrap-example.ps1](connect-to-bootstrap-example.ps1)
2. Replace `<PEER_ID>` with your bootstrap node's PeerId
3. If connecting remotely, change `127.0.0.1` to the bootstrap node's IP address
4. Run the script:
   ```powershell
   .\connect-to-bootstrap-example.ps1
   ```

### Method 2: Command Line

```bash
./target/release/coinject \
  --data-dir ./testnet/node2 \
  --p2p-addr "/ip4/0.0.0.0/tcp/30334" \
  --rpc-addr "127.0.0.1:9934" \
  --bootnodes "/ip4/127.0.0.1/tcp/30333/p2p/<PEER_ID>" \
  --mine \
  --difficulty 3 \
  --block-time 30
```

### Multiple Bootstrap Nodes

You can specify multiple bootstrap nodes for redundancy:

```bash
./target/release/coinject \
  --data-dir ./testnet/node2 \
  --p2p-addr "/ip4/0.0.0.0/tcp/30334" \
  --rpc-addr "127.0.0.1:9934" \
  --bootnodes "/ip4/192.168.1.100/tcp/30333/p2p/<PEER_ID_1>" \
  --bootnodes "/ip4/192.168.1.101/tcp/30333/p2p/<PEER_ID_2>" \
  --bootnodes "/ip4/192.168.1.102/tcp/30333/p2p/<PEER_ID_3>" \
  --mine
```

## Network Architecture

### Peer Discovery Methods

COINjecture Network B uses three complementary peer discovery methods:

1. **mDNS (Local Network Discovery)**
   - Automatically discovers peers on the same LAN
   - No configuration needed
   - Enabled by default

2. **Kademlia DHT (Distributed Discovery)**
   - Distributed hash table for peer routing
   - Helps nodes discover each other across the internet
   - Populated via bootstrap nodes

3. **Manual Bootstrap Nodes (Static Discovery)**
   - Explicitly configured trusted peers
   - Guaranteed initial connection point
   - Useful for cross-network or internet deployments

### When to Use Bootstrap Nodes

**Use bootstrap nodes when:**
- Running nodes across different networks (not the same LAN)
- Setting up a public testnet or mainnet
- mDNS discovery is unreliable or disabled
- You need guaranteed connectivity to specific peers
- Running nodes in cloud environments

**mDNS alone is sufficient when:**
- All nodes are on the same local network
- Testing with multiple nodes on the same machine
- Running a simple local testnet

## Bootstrap Node Best Practices

### 1. Dedicated Hardware/VPS

For production networks:
- Run bootstrap nodes on reliable infrastructure (VPS, cloud server, or dedicated hardware)
- Ensure high uptime (>99%)
- Use static IP addresses or DNS names

### 2. Network Configuration

**Firewall Rules:**
- Allow incoming TCP connections on P2P port (default: 30333)
- Allow outgoing connections for peer discovery

**Port Forwarding (if behind NAT):**
- Forward TCP port 30333 to the bootstrap node
- Configure router to use static internal IP for the node

### 3. Monitoring

Monitor your bootstrap node with:

**RPC endpoint:**
```bash
# Check peer count
curl -X POST http://127.0.0.1:9933 -H "Content-Type: application/json" -d '{
  "jsonrpc": "2.0",
  "method": "chain_getInfo",
  "params": [],
  "id": 1
}'
```

**Expected output includes:**
```json
{
  "peer_count": 5
}
```

### 4. Multiple Bootstrap Nodes

For production networks, run at least **3-5 bootstrap nodes** in different:
- Geographic locations
- Data centers/cloud providers
- Network segments

This provides:
- Redundancy (network remains accessible if one fails)
- Better performance (nodes connect to nearest bootstrap)
- DDoS resistance

## Advanced Configuration

### Bootstrap Node with Mining

Enable mining on the bootstrap node (helps secure the network):

```bash
./target/release/coinject \
  --data-dir ./bootstrap \
  --p2p-addr "/ip4/0.0.0.0/tcp/30333" \
  --rpc-addr "127.0.0.1:9933" \
  --mine \
  --miner-address "your_64_char_hex_address_here" \
  --difficulty 4 \
  --block-time 60
```

### Bootstrap Node Without Mining

Run as a pure relay/discovery node (lower resource usage):

```bash
./target/release/coinject \
  --data-dir ./bootstrap \
  --p2p-addr "/ip4/0.0.0.0/tcp/30333" \
  --rpc-addr "127.0.0.1:9933" \
  --difficulty 3 \
  --block-time 30
```

### Custom P2P Port

If port 30333 is already in use:

**Bootstrap node:**
```bash
--p2p-addr "/ip4/0.0.0.0/tcp/40000"
```

**Other nodes connecting:**
```bash
--bootnodes "/ip4/<IP>/tcp/40000/p2p/<PEER_ID>"
```

## Troubleshooting

### Problem: "Failed to parse bootnode address"

**Cause:** Invalid multiaddr format

**Solution:** Ensure the bootnode address follows the exact format:
```
/ip4/<IP>/tcp/<PORT>/p2p/<PEER_ID>
```

**Common mistakes:**
- Missing `/p2p/` prefix before PeerId
- Incorrect IP format
- Using hostname instead of IP (use IP address)
- Missing components (ip4, tcp, or p2p parts)

### Problem: "Failed to dial bootnode"

**Causes:**
1. Bootstrap node is not running
2. Firewall blocking connections
3. Incorrect IP address or port
4. Network connectivity issues

**Solutions:**
1. Verify bootstrap node is running: `netstat -an | findstr 30333` (Windows)
2. Check firewall rules allow TCP connections on port 30333
3. Test connectivity: `telnet <bootstrap_ip> 30333`
4. Verify IP address is reachable: `ping <bootstrap_ip>`

### Problem: Nodes not discovering each other

**Diagnostic steps:**

1. **Check PeerId is correct:**
   - Restart bootstrap node, copy exact PeerId from console output
   - Verify no extra characters or spaces

2. **Verify network connectivity:**
   ```bash
   # From the connecting node's machine
   telnet <bootstrap_ip> 30333
   ```

3. **Check node logs for connection messages:**
   - Look for "Connection established with peer"
   - Look for "mDNS discovered peer"
   - Look for "Kademlia routing updated"

4. **Test with localhost first:**
   - Start bootstrap node
   - Start second node on same machine using `127.0.0.1`
   - If this works, issue is network/firewall related

### Problem: Bootstrap node PeerId changes on restart

**Cause:** libp2p generates a new keypair on each startup by default

**Impact:** This is normal behavior. You'll need to update the `--bootnodes` parameter whenever the bootstrap node restarts.

**Workaround for production:** In the future, you could modify the code to persist the libp2p keypair to disk (similar to how node data is persisted).

## Example: 3-Node Testnet with Bootstrap

### Terminal 1: Bootstrap Node
```powershell
.\bootstrap-node.ps1
```

**Output:**
```
Network node PeerId: 12D3KooWAbc123...
```

### Terminal 2: Node 2 (Miner)
```bash
./target/release/coinject \
  --data-dir ./testnet/node2 \
  --p2p-addr "/ip4/0.0.0.0/tcp/30334" \
  --rpc-addr "127.0.0.1:9934" \
  --bootnodes "/ip4/127.0.0.1/tcp/30333/p2p/12D3KooWAbc123..." \
  --mine \
  --difficulty 3 \
  --block-time 30
```

### Terminal 3: Node 3 (Miner)
```bash
./target/release/coinject \
  --data-dir ./testnet/node3 \
  --p2p-addr "/ip4/0.0.0.0/tcp/30335" \
  --rpc-addr "127.0.0.1:9935" \
  --bootnodes "/ip4/127.0.0.1/tcp/30333/p2p/12D3KooWAbc123..." \
  --mine \
  --difficulty 3 \
  --block-time 30
```

### Expected Output (on all nodes):

```
Connection established with peer: 12D3KooW...
Identified peer: 12D3KooW... - protocol: /coinject/1.0.0
```

## Security Considerations

### 1. DDoS Protection

Bootstrap nodes are prime targets for DDoS attacks. Mitigation strategies:
- Use cloud providers with DDoS protection (Cloudflare, AWS Shield)
- Implement rate limiting at network/firewall level
- Run multiple bootstrap nodes across different networks

### 2. Trust Model

**Bootstrap nodes can:**
- Help discover other peers
- Relay blocks and transactions
- Provide initial network connection

**Bootstrap nodes cannot:**
- Modify blockchain data (protected by PoW consensus)
- Forge transactions (protected by cryptographic signatures)
- Double-spend or violate protocol rules (validated by all nodes)

**Recommendation:** Run your own bootstrap node(s) if you need guaranteed trustworthy initial peers.

### 3. Network Isolation

For private/permissioned networks:
- Only provide bootnode addresses to authorized participants
- Use firewall rules to restrict connections
- Disable mDNS if you don't want automatic local discovery

## Resources

- **libp2p Multiaddr Documentation:** https://docs.libp2p.io/concepts/addressing/
- **COINjecture Network B README:** [README.md](README.md)
- **Testnet Quick Start:** [TESTNET_QUICKSTART.md](TESTNET_QUICKSTART.md)
- **Transaction Testing:** [TRANSACTION_TEST_GUIDE.md](TRANSACTION_TEST_GUIDE.md)

## Support

For issues or questions:
- Check the [TESTNET_QUICKSTART.md](TESTNET_QUICKSTART.md) troubleshooting section
- Review network logs for connection errors
- Verify firewall and network configuration
- Test with local (`127.0.0.1`) connections first

---

**Version:** 4.5.0
**Last Updated:** January 2025
**License:** MIT
