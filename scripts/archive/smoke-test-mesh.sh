#!/usr/bin/env bash
# =============================================================================
# COINjecture Mesh Network — Smoke Test
# =============================================================================
# Spins up 3 mesh nodes locally, verifies:
# 1. All nodes start and form connections
# 2. Broadcast messages propagate
# 3. Nodes handle disconnect/reconnect
#
# Usage: bash smoke-test-mesh.sh

set -e

BINARY="target/release/examples/mesh_node"
TMPDIR=$(mktemp -d)
LOG_A="$TMPDIR/node_a.log"
LOG_B="$TMPDIR/node_b.log"
LOG_C="$TMPDIR/node_c.log"

PORT_A=19500
PORT_B=19501
PORT_C=19502

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

pass() { echo -e "  ${GREEN}✓ $1${NC}"; }
fail() { echo -e "  ${RED}✗ $1${NC}"; FAILURES=$((FAILURES + 1)); }
info() { echo -e "  ${YELLOW}→ $1${NC}"; }

FAILURES=0

cleanup() {
    info "Cleaning up..."
    kill $PID_A $PID_B $PID_C 2>/dev/null || true
    wait $PID_A $PID_B $PID_C 2>/dev/null || true
    rm -rf "$TMPDIR"
}
trap cleanup EXIT

echo ""
echo "  ╔═══════════════════════════════════════════════╗"
echo "  ║     COINjecture Mesh — Smoke Test             ║"
echo "  ╚═══════════════════════════════════════════════╝"
echo ""

# Check binary exists
if [ ! -f "$BINARY" ]; then
    echo "Building mesh_node..."
    cargo build --release --example mesh_node -p coinject-network
fi

# ─── Test 1: Start seed node ──────────────────────────────────────────────
info "Starting Node A (seed) on port $PORT_A..."
RUST_LOG=info $BINARY --listen "0.0.0.0:$PORT_A" --data-dir "$TMPDIR/data" > "$LOG_A" 2>&1 &
PID_A=$!
sleep 2

if kill -0 $PID_A 2>/dev/null; then
    pass "Node A started (PID $PID_A)"
else
    fail "Node A failed to start"
    cat "$LOG_A"
    exit 1
fi

# ─── Test 2: Start node B connecting to seed ──────────────────────────────
info "Starting Node B on port $PORT_B, seed=$PORT_A..."
RUST_LOG=info $BINARY --listen "0.0.0.0:$PORT_B" --seed "127.0.0.1:$PORT_A" --data-dir "$TMPDIR/data" > "$LOG_B" 2>&1 &
PID_B=$!
sleep 2

if kill -0 $PID_B 2>/dev/null; then
    pass "Node B started (PID $PID_B)"
else
    fail "Node B failed to start"
    cat "$LOG_B"
    exit 1
fi

# ─── Test 3: Start node C connecting to seed ──────────────────────────────
info "Starting Node C on port $PORT_C, seed=$PORT_A..."
RUST_LOG=info $BINARY --listen "0.0.0.0:$PORT_C" --seed "127.0.0.1:$PORT_A" --data-dir "$TMPDIR/data" > "$LOG_C" 2>&1 &
PID_C=$!
sleep 2

if kill -0 $PID_C 2>/dev/null; then
    pass "Node C started (PID $PID_C)"
else
    fail "Node C failed to start"
    cat "$LOG_C"
    exit 1
fi

# ─── Test 4: Verify mesh formation (wait for connections) ─────────────────
info "Waiting for mesh formation (8 seconds for peer exchange)..."
sleep 8

# Check Node A sees peers
PEERS_A=$(grep -c "PEER CONNECTED" "$LOG_A" 2>/dev/null || echo "0")
if [ "$PEERS_A" -ge 2 ]; then
    pass "Node A connected to $PEERS_A peers (expected ≥2)"
else
    fail "Node A connected to only $PEERS_A peers (expected ≥2)"
fi

# Check Node B sees at least the seed
PEERS_B=$(grep -c "PEER CONNECTED" "$LOG_B" 2>/dev/null || echo "0")
if [ "$PEERS_B" -ge 1 ]; then
    pass "Node B connected to $PEERS_B peers (expected ≥1)"
else
    fail "Node B connected to only $PEERS_B peers (expected ≥1)"
fi

# Check Node C sees at least the seed
PEERS_C=$(grep -c "PEER CONNECTED" "$LOG_C" 2>/dev/null || echo "0")
if [ "$PEERS_C" -ge 1 ]; then
    pass "Node C connected to $PEERS_C peers (expected ≥1)"
else
    fail "Node C connected to only $PEERS_C peers (expected ≥1)"
fi

# ─── Test 5: Verify handshakes are authenticated ─────────────────────────
HANDSHAKES_A=$(grep -c "handshake OK" "$LOG_A" 2>/dev/null || echo "0")
if [ "$HANDSHAKES_A" -ge 1 ]; then
    pass "Node A completed $HANDSHAKES_A authenticated handshakes"
else
    fail "Node A completed no handshakes"
fi

# ─── Test 6: Verify heartbeats are flowing ────────────────────────────────
info "Waiting for heartbeat cycle (12 seconds)..."
sleep 12

HEARTBEATS_B=$(grep -c "Heartbeat" "$LOG_B" 2>/dev/null || echo "0")
if [ "$HEARTBEATS_B" -ge 1 ]; then
    pass "Node B received $HEARTBEATS_B heartbeat messages"
else
    fail "Node B received no heartbeats"
fi

# ─── Test 7: Verify peer exchange is working ──────────────────────────────
PEEREX=$(grep -c "PeerExchange" "$LOG_A" 2>/dev/null || echo "0")
if [ "$PEEREX" -ge 1 ]; then
    pass "Peer exchange messages flowing ($PEEREX seen on Node A)"
else
    DISCOVERED=$(grep -l "discovered via peer exchange" "$LOG_A" "$LOG_B" "$LOG_C" 2>/dev/null | wc -l)
    if [ "$DISCOVERED" -ge 1 ]; then
        pass "Peer discovery via exchange working"
    else
        info "Peer exchange not observed (OK if all connected directly to seed)"
    fi
fi

# ─── Test 8: Kill Node B and verify disconnect detection ──────────────────
info "Killing Node B to test disconnect detection..."
kill $PID_B 2>/dev/null
wait $PID_B 2>/dev/null || true
sleep 3

DISCONNECTS_A=$(grep -c "peer disconnected" "$LOG_A" 2>/dev/null || echo "0")
if [ "$DISCONNECTS_A" -ge 1 ]; then
    pass "Node A detected disconnect ($DISCONNECTS_A events)"
else
    # May take time for heartbeat timeout
    info "Waiting for heartbeat-based disconnect detection (20s)..."
    sleep 20
    DISCONNECTS_A=$(grep -c "peer disconnected\|declared dead" "$LOG_A" 2>/dev/null || echo "0")
    if [ "$DISCONNECTS_A" -ge 1 ]; then
        pass "Node A detected disconnect via heartbeat timeout"
    else
        fail "Node A did not detect Node B disconnect"
    fi
fi

# ─── Test 9: Restart Node B and verify reconnection ──────────────────────
info "Restarting Node B..."
RUST_LOG=info $BINARY --listen "0.0.0.0:$PORT_B" --seed "127.0.0.1:$PORT_A" --data-dir "$TMPDIR/data" > "$LOG_B.2" 2>&1 &
PID_B=$!
sleep 5

RECONNECTS=$(grep -c "PEER CONNECTED" "$LOG_B.2" 2>/dev/null || echo "0")
if [ "$RECONNECTS" -ge 1 ]; then
    pass "Node B reconnected after restart ($RECONNECTS peers)"
else
    fail "Node B failed to reconnect"
fi

# ─── Summary ──────────────────────────────────────────────────────────────
echo ""
echo "  ─────────────────────────────────────────────────"
if [ "$FAILURES" -eq 0 ]; then
    echo -e "  ${GREEN}ALL TESTS PASSED${NC}"
else
    echo -e "  ${RED}$FAILURES TEST(S) FAILED${NC}"
fi
echo "  ─────────────────────────────────────────────────"
echo ""

# Show node IDs from logs
echo "  Node IDs:"
grep "Node ID:" "$LOG_A" 2>/dev/null | head -1 | sed 's/^/    A: /'
grep "Node ID:" "$LOG_B" 2>/dev/null | head -1 | sed 's/^/    B: /'
grep "Node ID:" "$LOG_C" 2>/dev/null | head -1 | sed 's/^/    C: /'
echo ""

exit $FAILURES
