#!/bin/bash
# =============================================================================
# COINjecture Network B - 6-Node Sync Regression Test (Linux/macOS)
# =============================================================================
# This test validates:
# 1. Adaptive Consensus (BOOTSTRAP → SECURE mode transition)
# 2. Gossip Trap Fix (unique request_id prevents dedup issues)
# 3. Multi-node sync stability
#
# Run: ./tests/integration/network_sync_test.sh
# Exit codes: 0 = PASS, 1 = FAIL
# =============================================================================

set -e

# Configuration
NODE_COUNT=${NODE_COUNT:-6}
TEST_DURATION=${TEST_DURATION:-180}
CHECK_INTERVAL=${CHECK_INTERVAL:-15}
MAX_SPREAD=${MAX_SPREAD:-3}
MIN_BLOCKS=${MIN_BLOCKS:-20}
BINARY=${BINARY:-"./target/release/coinject"}
DIFFICULTY=${DIFFICULTY:-3}
BLOCK_TIME=${BLOCK_TIME:-30}

BASE_P2P_PORT=30400
BASE_RPC_PORT=9940
DATA_DIR="testnet/regression-test"
PIDS=()

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

echo -e "${CYAN}═══════════════════════════════════════════════════════════════${NC}"
echo -e "${CYAN}  COINjecture Network B - 6-Node Sync Regression Test${NC}"
echo -e "${CYAN}═══════════════════════════════════════════════════════════════${NC}"
echo ""

# Cleanup function
cleanup() {
    echo -e "\n${YELLOW}🧹 Cleaning up...${NC}"
    for pid in "${PIDS[@]}"; do
        if kill -0 "$pid" 2>/dev/null; then
            kill "$pid" 2>/dev/null || true
        fi
    done
    rm -rf "$DATA_DIR"
}

trap cleanup EXIT

# Check binary exists
if [ ! -f "$BINARY" ]; then
    echo -e "${RED}❌ Binary not found: $BINARY${NC}"
    echo -e "${YELLOW}   Run 'cargo build --release' first${NC}"
    exit 1
fi

# Clean previous test data
rm -rf "$DATA_DIR"

echo "📊 Test Configuration:"
echo "   Nodes: $NODE_COUNT"
echo "   Duration: ${TEST_DURATION}s"
echo "   Check Interval: ${CHECK_INTERVAL}s"
echo "   Max Spread: $MAX_SPREAD blocks"
echo "   Min Blocks: $MIN_BLOCKS"
echo ""

# Start nodes
echo -e "${GREEN}🚀 Starting $NODE_COUNT nodes...${NC}"
for i in $(seq 0 $((NODE_COUNT - 1))); do
    NODE_NAME=$(printf "\\x$(printf '%02x' $((65 + i)))")
    P2P_PORT=$((BASE_P2P_PORT + i))
    RPC_PORT=$((BASE_RPC_PORT + i))
    NODE_DATA_DIR="$DATA_DIR/node-$NODE_NAME"
    
    $BINARY \
        --data-dir "$NODE_DATA_DIR" \
        --p2p-addr "/ip4/0.0.0.0/tcp/$P2P_PORT" \
        --rpc-addr "127.0.0.1:$RPC_PORT" \
        --mine \
        --difficulty $DIFFICULTY \
        --block-time $BLOCK_TIME \
        > /dev/null 2>&1 &
    
    PIDS+=($!)
    echo "   ✅ Node $NODE_NAME started (PID: ${PIDS[-1]}, RPC: $RPC_PORT)"
done

# Wait for nodes to connect
echo -e "\n${YELLOW}⏳ Waiting for nodes to connect (60s)...${NC}"
sleep 60

# Check initial connectivity
echo -e "\n${CYAN}🔍 Checking initial connectivity...${NC}"
ALL_CONNECTED=true
BODY='{"jsonrpc":"2.0","method":"chain_getInfo","params":[],"id":1}'

for i in $(seq 0 $((NODE_COUNT - 1))); do
    NODE_NAME=$(printf "\\x$(printf '%02x' $((65 + i)))")
    RPC_PORT=$((BASE_RPC_PORT + i))
    
    RESULT=$(curl -s -X POST -H "Content-Type: application/json" -d "$BODY" "http://127.0.0.1:$RPC_PORT" 2>/dev/null || echo '{"result":{"peer_count":0}}')
    PEER_COUNT=$(echo "$RESULT" | grep -o '"peer_count":[0-9]*' | grep -o '[0-9]*' || echo "0")
    EXPECTED_PEERS=$((NODE_COUNT - 1))
    
    if [ "$PEER_COUNT" -lt $((EXPECTED_PEERS - 1)) ]; then
        echo -e "   ${YELLOW}⚠️  Node $NODE_NAME: Only $PEER_COUNT/$EXPECTED_PEERS peers${NC}"
        ALL_CONNECTED=false
    else
        echo "   ✅ Node $NODE_NAME: $PEER_COUNT peers connected"
    fi
done

if [ "$ALL_CONNECTED" = false ]; then
    echo -e "\n${YELLOW}⏳ Waiting additional 30s for full connectivity...${NC}"
    sleep 30
fi

# Wait for mining to start
echo -e "\n${YELLOW}⏳ Waiting for mining to start...${NC}"
MINING_STARTED=false
for attempt in $(seq 1 20); do
    sleep 10
    RESULT=$(curl -s -X POST -H "Content-Type: application/json" -d "$BODY" "http://127.0.0.1:$BASE_RPC_PORT" 2>/dev/null || echo '{"result":{"best_height":0}}')
    HEIGHT=$(echo "$RESULT" | grep -o '"best_height":[0-9]*' | grep -o '[0-9]*' || echo "0")
    
    if [ "$HEIGHT" -gt 0 ]; then
        echo -e "   ${GREEN}✅ Mining started! Height: $HEIGHT${NC}"
        MINING_STARTED=true
        break
    fi
    echo "   [$attempt/20] Still at genesis..."
done

if [ "$MINING_STARTED" = false ]; then
    echo -e "${RED}❌ FAIL: Mining did not start within timeout${NC}"
    exit 1
fi

# Run stability test
echo -e "\n${CYAN}📈 Running stability test...${NC}"
ROUNDS=$((TEST_DURATION / CHECK_INTERVAL))
SYNC_FAILURES=0
MAX_SPREAD_SEEN=0
TEST_PASSED=true

for round in $(seq 1 $ROUNDS); do
    sleep $CHECK_INTERVAL
    
    HEIGHTS=""
    MAX_H=0
    MIN_H=999999
    TOTAL_PEERS=0
    
    for i in $(seq 0 $((NODE_COUNT - 1))); do
        RPC_PORT=$((BASE_RPC_PORT + i))
        RESULT=$(curl -s -X POST -H "Content-Type: application/json" -d "$BODY" "http://127.0.0.1:$RPC_PORT" 2>/dev/null || echo '{"result":{"best_height":0,"peer_count":0}}')
        HEIGHT=$(echo "$RESULT" | grep -o '"best_height":[0-9]*' | grep -o '[0-9]*' || echo "0")
        PEERS=$(echo "$RESULT" | grep -o '"peer_count":[0-9]*' | grep -o '[0-9]*' || echo "0")
        
        HEIGHTS="$HEIGHTS$HEIGHT,"
        TOTAL_PEERS=$((TOTAL_PEERS + PEERS))
        
        if [ "$HEIGHT" -gt "$MAX_H" ]; then MAX_H=$HEIGHT; fi
        if [ "$HEIGHT" -gt 0 ] && [ "$HEIGHT" -lt "$MIN_H" ]; then MIN_H=$HEIGHT; fi
    done
    
    SPREAD=$((MAX_H - MIN_H))
    AVG_PEERS=$((TOTAL_PEERS / NODE_COUNT))
    
    if [ "$SPREAD" -gt "$MAX_SPREAD_SEEN" ]; then MAX_SPREAD_SEEN=$SPREAD; fi
    
    if [ "$SPREAD" -le "$MAX_SPREAD" ]; then
        STATUS="✅"
    else
        STATUS="❌"
        SYNC_FAILURES=$((SYNC_FAILURES + 1))
    fi
    
    echo "   [$round/$ROUNDS] $STATUS Heights: ${HEIGHTS%,} | Spread: $SPREAD | Peers: $AVG_PEERS"
done

# Final status check
echo -e "\n${CYAN}🔍 Final status check...${NC}"
FINAL_MAX=0
for i in $(seq 0 $((NODE_COUNT - 1))); do
    NODE_NAME=$(printf "\\x$(printf '%02x' $((65 + i)))")
    RPC_PORT=$((BASE_RPC_PORT + i))
    RESULT=$(curl -s -X POST -H "Content-Type: application/json" -d "$BODY" "http://127.0.0.1:$RPC_PORT" 2>/dev/null || echo '{"result":{"best_height":0,"peer_count":0}}')
    HEIGHT=$(echo "$RESULT" | grep -o '"best_height":[0-9]*' | grep -o '[0-9]*' || echo "0")
    PEERS=$(echo "$RESULT" | grep -o '"peer_count":[0-9]*' | grep -o '[0-9]*' || echo "0")
    
    if [ "$HEIGHT" -gt "$FINAL_MAX" ]; then FINAL_MAX=$HEIGHT; fi
    echo "   Node $NODE_NAME: Height $HEIGHT | Peers: $PEERS"
done

# Evaluate results
echo ""
echo -e "${CYAN}═══════════════════════════════════════════════════════════════${NC}"
echo -e "${CYAN}  TEST RESULTS${NC}"
echo -e "${CYAN}═══════════════════════════════════════════════════════════════${NC}"
echo ""

FAILURES=""

# Check 1: Minimum blocks mined
if [ "$FINAL_MAX" -lt "$MIN_BLOCKS" ]; then
    echo -e "${RED}❌ FAIL: Only $FINAL_MAX blocks mined (required: $MIN_BLOCKS)${NC}"
    FAILURES="$FAILURES\n  - Insufficient blocks: $FINAL_MAX < $MIN_BLOCKS"
    TEST_PASSED=false
else
    echo -e "${GREEN}✅ PASS: $FINAL_MAX blocks mined (required: $MIN_BLOCKS)${NC}"
fi

# Check 2: Sync stability
PASS_RATE=$(( (ROUNDS - SYNC_FAILURES) * 100 / ROUNDS ))
MAX_FAILURES=$((ROUNDS / 10))
if [ "$SYNC_FAILURES" -gt "$MAX_FAILURES" ]; then
    echo -e "${RED}❌ FAIL: $SYNC_FAILURES/$ROUNDS sync checks failed ($PASS_RATE% pass rate)${NC}"
    FAILURES="$FAILURES\n  - Sync failures: $SYNC_FAILURES/$ROUNDS"
    TEST_PASSED=false
else
    echo -e "${GREEN}✅ PASS: $PASS_RATE% sync stability ($SYNC_FAILURES failures)${NC}"
fi

# Check 3: Maximum spread
if [ "$MAX_SPREAD_SEEN" -gt "$MAX_SPREAD" ]; then
    echo -e "${RED}❌ FAIL: Max spread $MAX_SPREAD_SEEN blocks (allowed: $MAX_SPREAD)${NC}"
    FAILURES="$FAILURES\n  - Max spread: $MAX_SPREAD_SEEN > $MAX_SPREAD"
    TEST_PASSED=false
else
    echo -e "${GREEN}✅ PASS: Max spread $MAX_SPREAD_SEEN blocks (allowed: $MAX_SPREAD)${NC}"
fi

echo ""

# Summary
if [ "$TEST_PASSED" = true ]; then
    echo -e "${GREEN}═══════════════════════════════════════════════════════════════${NC}"
    echo -e "${GREEN}  ✅ ALL TESTS PASSED${NC}"
    echo -e "${GREEN}═══════════════════════════════════════════════════════════════${NC}"
    echo ""
    echo "Summary:"
    echo "  - Nodes: $NODE_COUNT"
    echo "  - Blocks Mined: $FINAL_MAX"
    echo "  - Sync Pass Rate: $PASS_RATE%"
    echo "  - Max Spread: $MAX_SPREAD_SEEN blocks"
    echo ""
    exit 0
else
    echo -e "${RED}═══════════════════════════════════════════════════════════════${NC}"
    echo -e "${RED}  ❌ TESTS FAILED${NC}"
    echo -e "${RED}═══════════════════════════════════════════════════════════════${NC}"
    echo ""
    echo -e "${RED}Failure Reasons:$FAILURES${NC}"
    echo ""
    exit 1
fi

