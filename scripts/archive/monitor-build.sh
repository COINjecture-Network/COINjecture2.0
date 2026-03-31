#!/bin/bash
# Monitor cargo build progress on remote server

SERVER="143.110.139.166"
KEY="~/.ssh/COINjecture-Key"

echo "=== Build Progress Monitor ==="
echo "Press Ctrl+C to stop monitoring"
echo ""

while true; do
    clear
    echo "=== Build Status - $(date '+%H:%M:%S') ==="
    echo ""
    
    # Check if build is running
    BUILD_PID=$(ssh -i $KEY root@$SERVER 'ps aux | grep "[c]argo build --release" | awk "{print \$2}" | head -1')
    if [ -z "$BUILD_PID" ]; then
        echo "❌ Build process not found"
        echo ""
        echo "Checking if build completed..."
        ssh -i $KEY root@$SERVER 'tail -n 5 /root/COINjecture1337-NETB-main/target/release/coinject 2>/dev/null && echo "✅ Binary exists" || echo "❌ Binary not found"'
        break
    fi
    
    echo "✅ Build running (PID: $BUILD_PID)"
    echo ""
    
    # Show current rustc processes
    echo "📦 Currently compiling:"
    ssh -i $KEY root@$SERVER 'ps aux | grep "[r]ustc" | grep -o "crate-name [^ ]*" | sed "s/crate-name /  - /" | head -5'
    echo ""
    
    # Show CPU usage
    echo "💻 CPU Usage:"
    ssh -i $KEY root@$SERVER 'ps aux | grep "[r]ustc" | awk "{sum+=\$3} END {print \"  Total: \" sum \"%\"}"'
    echo ""
    
    # Count compiled crates
    echo "📊 Progress:"
    COMPILED=$(ssh -i $KEY root@$SERVER 'ls -1 /root/COINjecture1337-NETB-main/target/release/deps/*.rlib 2>/dev/null | wc -l')
    echo "  Compiled crates: $COMPILED"
    
    # Estimate time (rough: ~200-300 crates total, ~2-3 min per crate with codegen-units=1)
    if [ "$COMPILED" -gt 0 ]; then
        REMAINING=$((300 - COMPILED))
        EST_MIN=$((REMAINING * 2 / 60))
        echo "  Estimated remaining: ~$EST_MIN minutes (rough estimate)"
    fi
    
    echo ""
    echo "Refreshing in 5 seconds... (Ctrl+C to stop)"
    sleep 5
done
