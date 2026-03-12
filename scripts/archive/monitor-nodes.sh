#!/bin/bash
# Monitor nodes for: peer connections, longest chain, problem types
# Usage: ./monitor-nodes.sh

set -e

DROPLET1="143.110.139.166"
DROPLET2="68.183.205.12"
SSH_KEY="${SSH_KEY:-~/.ssh/COINjecture-Key}"

echo "🔍 Monitoring COINjecture Nodes"
echo "   Node 1: $DROPLET1"
echo "   Node 2: $DROPLET2"
echo ""
echo "Monitoring for:"
echo "  1. Peer connection (Peer IDs)"
echo "  2. Longest chain (block heights)"
echo "  3. Problem types (SubsetSum, SAT, TSP)"
echo ""
echo "Press Ctrl+C to stop"
echo ""

while true; do
    clear
    echo "=== $(date +%H:%M:%S) ==="
    echo ""
    
    # Get Peer IDs
    echo "📡 Peer IDs:"
    PEER1=$(ssh -i "$SSH_KEY" root@"$DROPLET1" "grep 'Network node PeerId:' /root/node.log 2>/dev/null | tail -1 | grep -oE '12D3KooW[0-9A-Za-z]+' || echo 'Not found'")
    PEER2=$(ssh -i "$SSH_KEY" root@"$DROPLET2" "grep 'Network node PeerId:' /root/node.log 2>/dev/null | tail -1 | grep -oE '12D3KooW[0-9A-Za-z]+' || echo 'Not found'")
    echo "   Node 1: $PEER1"
    echo "   Node 2: $PEER2"
    echo ""
    
    # Check peer connections
    echo "🔗 Peer Connections:"
    CONN1=$(ssh -i "$SSH_KEY" root@"$DROPLET1" "grep -c 'Connection established with peer' /root/node.log 2>/dev/null || echo '0'")
    CONN2=$(ssh -i "$SSH_KEY" root@"$DROPLET2" "grep -c 'Connection established with peer' /root/node.log 2>/dev/null || echo '0'")
    echo "   Node 1 connections: $CONN1"
    echo "   Node 2 connections: $CONN2"
    
    # Get latest connection info
    LATEST_CONN1=$(ssh -i "$SSH_KEY" root@"$DROPLET1" "grep 'Connection established with peer' /root/node.log 2>/dev/null | tail -1 | grep -oE '12D3KooW[0-9A-Za-z]+' || echo 'None'")
    LATEST_CONN2=$(ssh -i "$SSH_KEY" root@"$DROPLET2" "grep 'Connection established with peer' /root/node.log 2>/dev/null | tail -1 | grep -oE '12D3KooW[0-9A-Za-z]+' || echo 'None'")
    echo "   Node 1 latest peer: $LATEST_CONN1"
    echo "   Node 2 latest peer: $LATEST_CONN2"
    echo ""
    
    # Check chain heights
    echo "⛓️  Chain Heights:"
    HEIGHT1=$(ssh -i "$SSH_KEY" root@"$DROPLET1" "grep 'Best height:' /root/node.log 2>/dev/null | tail -1 | grep -oE '[0-9]+' || echo '0'")
    HEIGHT2=$(ssh -i "$SSH_KEY" root@"$DROPLET2" "grep 'Best height:' /root/node.log 2>/dev/null | tail -1 | grep -oE '[0-9]+' || echo '0'")
    echo "   Node 1: $HEIGHT1"
    echo "   Node 2: $HEIGHT2"
    
    if [ "$HEIGHT1" != "0" ] && [ "$HEIGHT2" != "0" ]; then
        if [ "$HEIGHT1" -gt "$HEIGHT2" ]; then
            echo "   ⚠️  Node 1 is ahead by $((HEIGHT1 - HEIGHT2)) blocks"
        elif [ "$HEIGHT2" -gt "$HEIGHT1" ]; then
            echo "   ⚠️  Node 2 is ahead by $((HEIGHT2 - HEIGHT1)) blocks"
        else
            echo "   ✅ Chains are synchronized"
        fi
    fi
    echo ""
    
    # Check problem types
    echo "🧩 Problem Types (last 10 blocks):"
    PROBLEMS1=$(ssh -i "$SSH_KEY" root@"$DROPLET1" "grep 'Generated problem:' /root/node.log 2>/dev/null | tail -10 | grep -oE 'SubsetSum|SAT|TSP' | sort | uniq -c || echo 'No problems found'")
    PROBLEMS2=$(ssh -i "$SSH_KEY" root@"$DROPLET2" "grep 'Generated problem:' /root/node.log 2>/dev/null | tail -10 | grep -oE 'SubsetSum|SAT|TSP' | sort | uniq -c || echo 'No problems found'")
    echo "   Node 1:"
    echo "$PROBLEMS1" | sed 's/^/      /'
    echo "   Node 2:"
    echo "$PROBLEMS2" | sed 's/^/      /'
    echo ""
    
    # Check for all three types
    ALL_TYPES1=$(echo "$PROBLEMS1" | grep -cE 'SubsetSum|SAT|TSP' || echo "0")
    ALL_TYPES2=$(echo "$PROBLEMS2" | grep -cE 'SubsetSum|SAT|TSP' || echo "0")
    
    if [ "$ALL_TYPES1" -ge "3" ]; then
        echo "   ✅ Node 1 has all 3 problem types"
    else
        echo "   ⚠️  Node 1 missing some problem types (found: $ALL_TYPES1)"
    fi
    
    if [ "$ALL_TYPES2" -ge "3" ]; then
        echo "   ✅ Node 2 has all 3 problem types"
    else
        echo "   ⚠️  Node 2 missing some problem types (found: $ALL_TYPES2)"
    fi
    echo ""
    
    # Check if nodes are running
    echo "🖥️  Node Status:"
    RUNNING1=$(ssh -i "$SSH_KEY" root@"$DROPLET1" "ps aux | grep -c '[c]oinject' || echo '0'")
    RUNNING2=$(ssh -i "$SSH_KEY" root@"$DROPLET2" "ps aux | grep -c '[c]oinject' || echo '0'")
    echo "   Node 1: $([ "$RUNNING1" -gt "0" ] && echo '✅ Running' || echo '❌ Stopped')"
    echo "   Node 2: $([ "$RUNNING2" -gt "0" ] && echo '✅ Running' || echo '❌ Stopped')"
    echo ""
    
    sleep 10
done


