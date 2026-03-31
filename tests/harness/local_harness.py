#!/usr/bin/env python3
"""
COINjecture Network B - Local Test Harness
==========================================
Runs multiple nodes locally without Docker for testing.
"""

import argparse
import os
import shutil
import signal
import subprocess
import sys
import time
from pathlib import Path

# Path to the built binary
BINARY = Path(__file__).parent.parent.parent / "target" / "release" / "coinject.exe"
DATA_DIR = Path(__file__).parent / "validation_data"

NODES = {
    "bootnode": {"rpc": 9940, "p2p": 30400},
    "node-a": {"rpc": 9941, "p2p": 30401},
    "node-b": {"rpc": 9942, "p2p": 30402},
    "node-c": {"rpc": 9943, "p2p": 30403},
    "node-d": {"rpc": 9944, "p2p": 30404},
    "node-e": {"rpc": 9945, "p2p": 30405},
    "node-f": {"rpc": 9946, "p2p": 30406},
}

processes = {}

def cleanup_data():
    """Remove all node data directories."""
    if DATA_DIR.exists():
        try:
            shutil.rmtree(DATA_DIR)
        except PermissionError as e:
            # Windows may have files locked - try killing coinject processes first
            print(f"Warning: Could not clean data directory (files may be locked): {e}")
            if sys.platform == 'win32':
                import subprocess
                subprocess.run(['taskkill', '/F', '/IM', 'coinject.exe'],
                              capture_output=True, text=True)
                time.sleep(2)
                try:
                    shutil.rmtree(DATA_DIR)
                except PermissionError:
                    print("Still could not clean - proceeding anyway")
    DATA_DIR.mkdir(parents=True, exist_ok=True)

def start_bootnode():
    """Start the bootnode."""
    name = "bootnode"
    config = NODES[name]
    data_path = DATA_DIR / name
    data_path.mkdir(parents=True, exist_ok=True)

    cmd = [
        str(BINARY),
        "--mine",
        "--data-dir", str(data_path),
        "--p2p-addr", f"/ip4/0.0.0.0/tcp/{config['p2p']}",
        "--rpc-addr", f"0.0.0.0:{config['rpc']}",
        "--difficulty", "2",
        "--block-time", "10",
    ]

    log_file = open(DATA_DIR / f"{name}.log", "w")
    proc = subprocess.Popen(cmd, stdout=log_file, stderr=subprocess.STDOUT)
    processes[name] = (proc, log_file)
    print(f"Started {name} (PID: {proc.pid})")
    return proc

def get_bootnode_peer_id():
    """Get bootnode peer ID from its log."""
    time.sleep(5)
    log_path = DATA_DIR / "bootnode.log"
    if log_path.exists():
        with open(log_path, "r", encoding="utf-8", errors="ignore") as f:
            for line in f:
                if "Network node PeerId:" in line:
                    # Extract PeerID: 12D3KooW...
                    parts = line.split("Network node PeerId:")
                    if len(parts) > 1:
                        peer_id = parts[1].strip()
                        return peer_id
    return None

def start_peer(name: str, bootnode_addr: str, mine: bool = False):
    """Start a peer node."""
    config = NODES[name]
    data_path = DATA_DIR / name
    data_path.mkdir(parents=True, exist_ok=True)

    cmd = [
        str(BINARY),
        "--data-dir", str(data_path),
        "--p2p-addr", f"/ip4/0.0.0.0/tcp/{config['p2p']}",
        "--rpc-addr", f"0.0.0.0:{config['rpc']}",
        "--bootnodes", bootnode_addr,
        "--difficulty", "2",
        "--block-time", "10",
    ]
    if mine:
        cmd.insert(1, "--mine")

    log_file = open(DATA_DIR / f"{name}.log", "w")
    proc = subprocess.Popen(cmd, stdout=log_file, stderr=subprocess.STDOUT)
    processes[name] = (proc, log_file)
    print(f"Started {name} (PID: {proc.pid})")
    return proc

def stop_all():
    """Stop all running nodes."""
    for name, (proc, log_file) in processes.items():
        print(f"Stopping {name}...")
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()
        log_file.close()
    processes.clear()

def signal_handler(sig, frame):
    print("\nShutting down...")
    stop_all()
    sys.exit(0)

def main():
    parser = argparse.ArgumentParser(description="Local test harness")
    parser.add_argument("--nodes", type=int, default=3, help="Number of peer nodes to start (1-6)")
    parser.add_argument("--clean", action="store_true", help="Clean data directories before starting")
    parser.add_argument("--peer-mine", action="store_true", help="Enable mining on peer nodes (default: only bootnode mines)")
    args = parser.parse_args()

    signal.signal(signal.SIGINT, signal_handler)

    if not BINARY.exists():
        print(f"Error: Binary not found at {BINARY}")
        print("Run: cargo build --release --bin coinject")
        sys.exit(1)

    if args.clean:
        cleanup_data()
    else:
        DATA_DIR.mkdir(parents=True, exist_ok=True)

    # Start bootnode
    print("Starting bootnode...")
    start_bootnode()

    # Wait for bootnode to initialize and get its peer ID
    print("Waiting for bootnode to initialize...")
    peer_id = None
    for _ in range(30):
        peer_id = get_bootnode_peer_id()
        if peer_id:
            break
        time.sleep(1)

    if not peer_id:
        print("Error: Could not get bootnode peer ID")
        stop_all()
        sys.exit(1)

    bootnode_addr = f"/ip4/127.0.0.1/tcp/30400/p2p/{peer_id}"
    print(f"Bootnode address: {bootnode_addr}")

    # Start peer nodes
    peer_names = ["node-a", "node-b", "node-c", "node-d", "node-e", "node-f"][:args.nodes]
    for name in peer_names:
        start_peer(name, bootnode_addr, mine=args.peer_mine)
        time.sleep(1)

    print(f"\n{len(peer_names) + 1} nodes started. Press Ctrl+C to stop.")
    print(f"Logs in: {DATA_DIR}")
    print("\nRPC ports:")
    print(f"  bootnode: http://localhost:9940")
    for name in peer_names:
        print(f"  {name}: http://localhost:{NODES[name]['rpc']}")

    # Keep running
    while True:
        time.sleep(10)
        # Check if nodes are still running
        for name, (proc, _) in list(processes.items()):
            if proc.poll() is not None:
                print(f"Warning: {name} has exited with code {proc.returncode}")

if __name__ == "__main__":
    main()
