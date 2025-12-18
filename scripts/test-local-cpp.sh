#!/bin/bash
# Local CPP Network Test Script
# Tests bootnode and Node 2 locally before deployment

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${GREEN}🧪 COINjecture CPP Network - Local Test${NC}"
echo "=========================================="
echo ""

# Build binary (prefer debug for faster iteration, use release if available)
BINARY_PATH="./target/debug/coinject"
if [ ! -f "$BINARY_PATH" ]; then
    if [ -f "./target/release/coinject" ]; then
        BINARY_PATH="./target/release/coinject"
        echo -e "${GREEN}✅ Using existing release binary${NC}"
    else
        echo -e "${YELLOW}📦 Building debug binary...${NC}"
        cargo build -p coinject-node
        echo ""
    fi
else
    echo -e "${GREEN}✅ Using debug binary (has latest changes)${NC}"
    echo ""
fi

# Create test data directories
BOOTNODE_DIR="./data/bootnode"
NODE2_DIR="./data/node2"

echo -e "${YELLOW}📁 Creating test data directories...${NC}"
rm -rf "$BOOTNODE_DIR" "$NODE2_DIR"
mkdir -p "$BOOTNODE_DIR" "$NODE2_DIR"
echo ""

# Define ports first
BOOTNODE_P2P_PORT=707
BOOTNODE_WS_PORT=8080
BOOTNODE_RPC_PORT=9933
BOOTNODE_METRICS_PORT=9090

NODE2_P2P_PORT=7071
NODE2_WS_PORT=8081
NODE2_RPC_PORT=9934
NODE2_METRICS_PORT=9091

# Get local IP (for macOS/Linux)
LOCAL_IP=$(ifconfig | grep -Eo 'inet (addr:)?([0-9]*\.){3}[0-9]*' | grep -Eo '([0-9]*\.){3}[0-9]*' | grep -v '127.0.0.1' | head -1)
if [ -z "$LOCAL_IP" ]; then
    LOCAL_IP="127.0.0.1"
fi

BOOTNODE_P2P="0.0.0.0:$BOOTNODE_P2P_PORT"
BOOTNODE_WS="0.0.0.0:$BOOTNODE_WS_PORT"
BOOTNODE_RPC="127.0.0.1:$BOOTNODE_RPC_PORT"

NODE2_P2P="0.0.0.0:$NODE2_P2P_PORT"
NODE2_WS="0.0.0.0:$NODE2_WS_PORT"
NODE2_RPC="127.0.0.1:$NODE2_RPC_PORT"
NODE2_BOOTNODE="$LOCAL_IP:$BOOTNODE_P2P_PORT"

echo -e "${GREEN}🚀 Starting Bootnode...${NC}"
echo "   P2P: $BOOTNODE_P2P"
echo "   WebSocket: $BOOTNODE_WS"
echo "   RPC: $BOOTNODE_RPC"
echo ""

# Start bootnode in background
# Note: --p2p-addr is for libp2p (multiaddr format), --cpp-p2p-addr and --cpp-ws-addr are for CPP protocol
$BINARY_PATH \
    --data-dir "$BOOTNODE_DIR" \
    --node-type full \
    --mine \
    --p2p-addr "/ip4/0.0.0.0/tcp/30333" \
    --cpp-p2p-addr "$BOOTNODE_P2P" \
    --cpp-ws-addr "$BOOTNODE_WS" \
    --rpc-addr "$BOOTNODE_RPC" \
    --metrics-addr "127.0.0.1:9090" \
    --miner-address "0000000000000000000000000000000000000000000000000000000000000001" \
    > bootnode.log 2>&1 &

BOOTNODE_PID=$!
echo "Bootnode PID: $BOOTNODE_PID"
echo ""

# Wait for bootnode to start
echo -e "${YELLOW}⏳ Waiting for bootnode to initialize (10 seconds)...${NC}"
sleep 10

# Check if bootnode is running
if ! kill -0 $BOOTNODE_PID 2>/dev/null; then
    echo -e "${RED}❌ Bootnode failed to start!${NC}"
    echo "Check bootnode.log for errors:"
    tail -20 bootnode.log
    exit 1
fi

echo -e "${GREEN}✅ Bootnode is running${NC}"
echo ""

# Start Node 2
echo -e "${GREEN}🚀 Starting Node 2...${NC}"
echo "   P2P: $NODE2_P2P"
echo "   WebSocket: $NODE2_WS"
echo "   RPC: $NODE2_RPC"
echo "   Bootnode: $NODE2_BOOTNODE"
echo ""

# Start Node 2 - CPP bootnodes are handled via config parsing
# For CPP testing, we pass bootnode in a format CPP can extract IP:PORT from
# Option 1: If we have a valid PeerId, use full multiaddr (both libp2p and CPP work)
# Option 2: If no PeerId, pass as IP:PORT directly (CPP will use it, libp2p will skip)
if [ -n "$BOOTNODE_PEER_ID" ] && [ "$BOOTNODE_PEER_ID" != "PLACEHOLDER" ] && [ "${#BOOTNODE_PEER_ID}" -gt 20 ]; then
    # Use full multiaddr if we have a valid PeerId
    BOOTNODE_MULTIADDR="/ip4/$LOCAL_IP/tcp/$BOOTNODE_P2P_PORT/p2p/$BOOTNODE_PEER_ID"
    BOOTNODE_ARGS="--bootnodes $BOOTNODE_MULTIADDR"
    echo "Using bootnode multiaddr: $BOOTNODE_MULTIADDR"
else
    # For CPP-only testing, pass as IP:PORT format
    # CPP will extract it, libp2p will skip (no error)
    BOOTNODE_IP_PORT="$LOCAL_IP:$BOOTNODE_P2P_PORT"
    BOOTNODE_ARGS="--bootnodes $BOOTNODE_IP_PORT"
    echo -e "${YELLOW}⚠️  Using IP:PORT format for CPP bootnode: $BOOTNODE_IP_PORT${NC}"
    echo "  (libp2p will skip, CPP will connect)"
fi

$BINARY_PATH \
    --data-dir "$NODE2_DIR" \
    --node-type full \
    --mine \
    --p2p-addr "/ip4/0.0.0.0/tcp/30334" \
    --cpp-p2p-addr "$NODE2_P2P" \
    --cpp-ws-addr "$NODE2_WS" \
    --rpc-addr "$NODE2_RPC" \
    --metrics-addr "127.0.0.1:9091" \
    $BOOTNODE_ARGS \
    --miner-address "0000000000000000000000000000000000000000000000000000000000000002" \
    > node2.log 2>&1 &

NODE2_PID=$!
echo "Node 2 PID: $NODE2_PID"
echo ""

# Wait for Node 2 to start
echo -e "${YELLOW}⏳ Waiting for Node 2 to initialize (10 seconds)...${NC}"
sleep 10

# Check if Node 2 is running
if ! kill -0 $NODE2_PID 2>/dev/null; then
    echo -e "${RED}❌ Node 2 failed to start!${NC}"
    echo "Check node2.log for errors:"
    tail -20 node2.log
    kill $BOOTNODE_PID 2>/dev/null || true
    exit 1
fi

echo -e "${GREEN}✅ Node 2 is running${NC}"
echo ""

# Test sync
echo -e "${YELLOW}🔄 Testing two-node sync (30 seconds)...${NC}"
sleep 30

# Check logs for sync activity
echo -e "${YELLOW}📊 Checking sync status...${NC}"
echo ""
echo "Bootnode log (last 10 lines):"
tail -10 bootnode.log
echo ""
echo "Node 2 log (last 10 lines):"
tail -10 node2.log
echo ""

# Test WebSocket connection
echo -e "${YELLOW}🌐 Testing WebSocket connection...${NC}"
WS_URL="ws://127.0.0.1:8080"
echo "Connecting to $WS_URL..."

# Use curl to test WebSocket (if available)
if command -v curl &> /dev/null; then
    curl -i -N \
        -H "Connection: Upgrade" \
        -H "Upgrade: websocket" \
        -H "Sec-WebSocket-Version: 13" \
        -H "Sec-WebSocket-Key: test" \
        "$WS_URL" 2>&1 | head -5 || echo "WebSocket test skipped (curl may not support WS)"
else
    echo "curl not available, skipping WebSocket test"
fi
echo ""

# Summary
echo -e "${GREEN}=========================================="
echo "✅ Local Test Complete"
echo "==========================================${NC}"
echo ""
echo "Nodes running:"
echo "  Bootnode: PID $BOOTNODE_PID (P2P: $BOOTNODE_P2P, WS: $BOOTNODE_WS, RPC: $BOOTNODE_RPC)"
echo "  Node 2:   PID $NODE2_PID (P2P: $NODE2_P2P, WS: $NODE2_WS, RPC: $NODE2_RPC)"
echo ""
echo "Logs:"
echo "  bootnode.log"
echo "  node2.log"
echo ""
echo "To stop nodes:"
echo "  kill $BOOTNODE_PID $NODE2_PID"
echo ""
echo "To monitor:"
echo "  tail -f bootnode.log"
echo "  tail -f node2.log"
echo ""
echo -e "${YELLOW}Press Ctrl+C to stop all nodes...${NC}"

# Trap to cleanup on exit
trap "echo ''; echo -e '${YELLOW}🛑 Stopping nodes...${NC}'; kill $BOOTNODE_PID $NODE2_PID 2>/dev/null || true; exit" INT TERM

# Keep script running
wait

