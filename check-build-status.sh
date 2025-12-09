#!/bin/bash
# Quick build status check

DROPLET1_IP="143.110.139.166"
SSH_KEY="~/.ssh/COINjecture-Key"

echo "Checking build status on node 1..."

# Try to get build status with a short timeout
ssh -i "$SSH_KEY" -o ConnectTimeout=3 -o ServerAliveInterval=5 root@"$DROPLET1_IP" << 'EOF' 2>&1 | head -20
    cd /root/COINjecture1337-NETB-main
    
    # Check if binary exists
    if [ -f target/fast-release/coinject ]; then
        echo "✅ BUILD COMPLETE!"
        ls -lh target/fast-release/coinject
        exit 0
    fi
    
    # Check if build is still running
    if pgrep -f "cargo build.*coinject-node" > /dev/null; then
        echo "⏳ Build still running..."
        ps aux | grep -E "cargo|rustc" | grep -v grep | wc -l | xargs echo "Active processes:"
    else
        echo "❌ No build process found - may have completed or failed"
        if [ -d target/fast-release ]; then
            echo "Build directory exists, checking for errors..."
            ls -la target/fast-release/ | head -5
        fi
    fi
EOF


