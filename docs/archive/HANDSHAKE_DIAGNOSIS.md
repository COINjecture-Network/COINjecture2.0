# Handshake Failure Diagnosis

## Summary
Node 1 can connect OUTBOUND to Node 2, but Node 2 cannot connect OUTBOUND to Node 1. The connection is **unidirectional**.

## Key Findings

### 1. Connection Direction
- **Node 1 → Node 2**: ✅ SUCCESS (OUTBOUND)
  - Connection established with 158ms handshake
  - Identify protocol works both ways
  - Gossipsub subscriptions successful
  - Node 1 receives blocks and status from Node 2

- **Node 2 → Node 1**: ❌ FAILURE (OUTBOUND)
  - Repeated timeout errors
  - Never establishes connection

- **Node 2 INBOUND**: ✅ Multiple connections established
  - But these are from the GCE VM (12D3KooWFL8uuMmeoWyU46SdX8g2aJEk4Fv5qAr4dZXmZfsGiefa)
  - NOT from Node 1

### 2. Connection Reset Issue
Node 1 shows:
```
❌ OUTGOING CONNECTION FAILED [TRANSPORT_ERROR]
   Error: Connection reset by peer
```
This happens AFTER a successful connection establishment, suggesting:
- Connection is established
- Then immediately reset by Node 2
- Or connection is unstable and keeps resetting

### 3. Deserialization Errors on Node 2
```
Failed to deserialize network message: invalid value: integer '386047657', expected variant index 0 <= i < 6
```

**Critical**: This suggests:
- Node 2 is receiving messages (so connection exists at some level)
- But the messages are malformed or from a different protocol version
- The `NetworkMessage` enum has 6 variants (0-5), but received index >= 6
- This could be a **protocol version mismatch** or **serialization bug**

### 4. Status Messages
- Node 1 receives status from Node 2 (height 182)
- Node 2 does NOT show receiving status from Node 1
- This confirms unidirectional communication

### 5. Genesis Hash
- Both nodes have same genesis hash: `Hash(4a80254b4a48e867)` ✅
- Genesis hash validation is NOT the issue (handshake fails before Status message)

## Root Cause Analysis

### Critical Finding: Node 2's INBOUND Connections Are NOT From Node 1
- **Node 2's INBOUND connections are from GCE VM** (`12D3KooWFL8uuMmeoWyU46SdX8g2aJEk4Fv5qAr4dZXmZfsGiefa`)
- **Node 2 NEVER sees Node 1's peer ID in INBOUND connections**
- This means Node 1's OUTBOUND connection to Node 2 is either:
  1. Not completing the handshake (fails before ConnectionEstablished)
  2. Being reset immediately after establishment
  3. Not being recognized by Node 2 as an INBOUND connection

### Primary Hypothesis: Unidirectional Connection with Immediate Reset
1. **Node 1 → Node 2 (OUTBOUND)**: Connection established, handshake completes
2. **Connection immediately reset**: Node 1 sees "Connection reset by peer"
3. **Node 2 never recognizes the connection**: No INBOUND connection logged from Node 1
4. **Node 2 → Node 1 (OUTBOUND)**: Always times out (can't establish connection)
5. **Result**: Unidirectional communication - Node 1 can send to Node 2, but Node 2 can't respond

### Secondary Hypothesis: Deserialization Errors from Wrong Peer
- Deserialization errors on Node 2 are likely from the **GCE VM**, not Node 1
- The GCE VM might be running an older protocol version
- Node 2 can't deserialize messages from GCE VM → errors
- But Node 2 thinks it's connected (to GCE VM), so it doesn't try harder to connect to Node 1

### Tertiary Hypothesis: NAT/Firewall Issue
- Node 1 can initiate connections (outbound works)
- Node 2 cannot initiate connections to Node 1 (timeout)
- This suggests Node 1's firewall or NAT is blocking inbound connections from Node 2
- But Node 1's outbound connections work, so it's not a complete firewall block

## Next Steps for Diagnosis

### Completed ✅
1. ✅ Added detailed deserialization logging with peer ID and hex dump
2. ✅ Verified Node 2's INBOUND connections are from GCE VM, not Node 1
3. ✅ Confirmed Node 1 can connect OUTBOUND to Node 2, but connection gets reset
4. ✅ Confirmed Node 2 cannot connect OUTBOUND to Node 1 (timeout)

### Remaining Tasks
1. **Deploy enhanced logging** to see which peer is sending malformed messages
2. **Check if Node 1's connection to Node 2 is actually completing**: The reset might be happening during handshake
3. **Verify if Node 2 is receiving any messages from Node 1**: Check if messages are being received but not logged
4. **Check connection state on both nodes**: Use RPC to check actual connection status
5. **Investigate why Node 1's OUTBOUND connection resets**: Check if it's a protocol mismatch or connection issue

## Immediate Actions

1. **Deploy the enhanced deserialization logging** to identify which peer is sending malformed messages
2. **Check connection state via RPC** on both nodes to see actual peer connections
3. **Monitor logs in real-time** when Node 1 connects to Node 2 to see the exact sequence of events
4. **Check if there's a connection limit** that's causing Node 2 to reject Node 1's connection

