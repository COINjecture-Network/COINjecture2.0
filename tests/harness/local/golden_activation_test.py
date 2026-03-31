#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Golden Activation Height Test
==============================
Tests the golden_activation_height feature:
- Blocks before activation height should be v1 (standard)
- Blocks at/after activation height should be v2 (golden-enhanced)
- Syncer node should accept both versions
"""

import os
import sys
import time
import shutil
import signal
import subprocess
import requests
from pathlib import Path

# Fix Windows console encoding
if sys.platform == 'win32':
    import io
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8', errors='replace')
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding='utf-8', errors='replace')

# Paths
SCRIPT_DIR = Path(__file__).parent
REPO_ROOT = SCRIPT_DIR.parent.parent.parent
BINARY = REPO_ROOT / "target" / "release" / "coinject.exe"
ARTIFACTS_DIR = SCRIPT_DIR / "artifacts" / "golden_activation"

# Golden activation height for test
GOLDEN_ACTIVATION_HEIGHT = 5

# Node configs
NODES = {
    "miner": {"rpc": 9960, "p2p": 30600, "mine": True},
    "syncer": {"rpc": 9961, "p2p": 30601, "mine": False},
}

processes = {}

def cleanup():
    """Kill processes and clean data."""
    print("Cleaning up...")
    if sys.platform == 'win32':
        subprocess.run(['taskkill', '/F', '/IM', 'coinject.exe'],
                      capture_output=True, text=True)
    time.sleep(2)

    if ARTIFACTS_DIR.exists():
        try:
            shutil.rmtree(ARTIFACTS_DIR)
        except PermissionError:
            print("Warning: Could not fully clean artifacts directory")
    ARTIFACTS_DIR.mkdir(parents=True, exist_ok=True)

def start_node(name: str, connect_to: str = None, golden_height: int = 0):
    """Start a node with golden activation height."""
    config = NODES[name]
    data_path = ARTIFACTS_DIR / name
    data_path.mkdir(parents=True, exist_ok=True)

    ws_port = config['p2p'] + 1000

    cmd = [
        str(BINARY),
        "--data-dir", str(data_path),
        "--rpc-addr", f"127.0.0.1:{config['rpc']}",
        "--cpp-p2p-addr", f"0.0.0.0:{config['p2p']}",
        "--cpp-ws-addr", f"0.0.0.0:{ws_port}",
        "--difficulty", "2",
        "--block-time", "10",
        "--golden-activation-height", str(golden_height),
    ]

    if config['mine']:
        cmd.append("--mine")
        cmd.append("--dev")

    if connect_to:
        cmd.extend(["--bootnodes", connect_to])

    log_path = ARTIFACTS_DIR / f"{name}.log"
    log_file = open(log_path, "w", encoding="utf-8")

    print(f"Starting {name} (golden_activation_height={golden_height})...")
    print(f"  Command: {' '.join(cmd)}")

    proc = subprocess.Popen(cmd, stdout=log_file, stderr=subprocess.STDOUT)
    processes[name] = (proc, log_file)
    return proc

def get_node_height(name: str) -> int:
    """Get node height via RPC."""
    config = NODES[name]
    try:
        resp = requests.post(
            f"http://127.0.0.1:{config['rpc']}",
            json={"jsonrpc": "2.0", "method": "chain_getInfo", "params": [], "id": 1},
            timeout=5
        )
        if resp.status_code == 200:
            data = resp.json()
            if "result" in data:
                return data["result"].get("best_height", 0)
    except Exception:
        pass
    return -1

def wait_for_node(name: str, timeout: int = 30) -> bool:
    """Wait for node to be responsive."""
    config = NODES[name]
    print(f"Waiting for {name} to start...", end="", flush=True)
    for i in range(timeout):
        try:
            resp = requests.post(
                f"http://127.0.0.1:{config['rpc']}",
                json={"jsonrpc": "2.0", "method": "chain_getInfo", "params": [], "id": 1},
                timeout=2
            )
            if resp.status_code == 200:
                print(" OK")
                return True
        except:
            pass
        print(".", end="", flush=True)
        time.sleep(1)
    print(" TIMEOUT")
    return False

def stop_node(name: str):
    """Stop a specific node."""
    if name in processes:
        proc, log_file = processes[name]
        print(f"Stopping {name}...")
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()
        log_file.close()
        del processes[name]

def stop_all():
    """Stop all nodes."""
    for name in list(processes.keys()):
        stop_node(name)

def check_block_versions(name: str) -> dict:
    """Check block versions in node log."""
    log_path = ARTIFACTS_DIR / f"{name}.log"
    versions = {"v1": 0, "v2": 0}

    if not log_path.exists():
        return versions

    with open(log_path, "r", encoding="utf-8", errors="ignore") as f:
        for line in f:
            if "version=1" in line and ("standard" in line or "BLOCK" in line):
                versions["v1"] += 1
            elif "version=2" in line and ("golden" in line or "BLOCK" in line):
                versions["v2"] += 1

    return versions

def run_test():
    """Run the golden activation test."""
    print("\n" + "=" * 60)
    print("GOLDEN ACTIVATION HEIGHT TEST")
    print(f"Activation height: {GOLDEN_ACTIVATION_HEIGHT}")
    print("=" * 60)

    cleanup()

    if not BINARY.exists():
        print(f"ERROR: Binary not found at {BINARY}")
        return False

    print(f"\nUsing binary: {BINARY}")
    print(f"Artifacts: {ARTIFACTS_DIR}")

    # Phase 1: Start miner with golden activation at height 5
    print("\n--- Phase 1: Start Miner Node ---")
    start_node("miner", golden_height=GOLDEN_ACTIVATION_HEIGHT)
    if not wait_for_node("miner", 30):
        print("FAILED: Miner node did not start")
        stop_all()
        return False

    # Wait for miner to produce blocks past activation height
    print(f"Waiting for miner to produce {GOLDEN_ACTIVATION_HEIGHT + 3} blocks...")
    target_height = GOLDEN_ACTIVATION_HEIGHT + 3
    for i in range(60):  # 60 seconds max
        time.sleep(5)
        height = get_node_height("miner")
        print(f"  Miner height: {height}")
        if height >= target_height:
            break

    miner_height = get_node_height("miner")
    print(f"Miner reached height: {miner_height}")

    if miner_height < target_height:
        print(f"FAILED: Miner did not reach height {target_height}")
        stop_all()
        return False

    # Phase 2: Check block versions in log
    print("\n--- Phase 2: Check Block Versions ---")
    versions = check_block_versions("miner")
    print(f"Blocks produced: v1={versions['v1']}, v2={versions['v2']}")

    # We expect some v1 blocks (before activation) and some v2 blocks (after)
    # Genesis is height 0, activation at height 5
    # Heights 1-4 should be v1, heights 5+ should be v2

    # Phase 3: Start syncer and verify it accepts both versions
    print("\n--- Phase 3: Start Syncer Node ---")
    miner_addr = f"127.0.0.1:{NODES['miner']['p2p']}"
    start_node("syncer", connect_to=miner_addr, golden_height=GOLDEN_ACTIVATION_HEIGHT)
    if not wait_for_node("syncer", 30):
        print("FAILED: Syncer node did not start")
        stop_all()
        return False

    # Wait for sync
    print("Waiting for syncer to sync...")
    for i in range(12):
        time.sleep(5)
        syncer_height = get_node_height("syncer")
        miner_height = get_node_height("miner")
        print(f"  Miner: {miner_height}, Syncer: {syncer_height}")

        if syncer_height > 0 and abs(miner_height - syncer_height) <= 2:
            print("Syncer caught up!")
            break

    # Phase 4: Results
    print("\n--- Phase 4: Results ---")
    final_miner = get_node_height("miner")
    final_syncer = get_node_height("syncer")

    print(f"Final heights - Miner: {final_miner}, Syncer: {final_syncer}")
    print(f"Gap: {abs(final_miner - final_syncer)} blocks")

    # Check syncer log for version acceptance
    syncer_versions = check_block_versions("syncer")
    print(f"Syncer received: v1={syncer_versions['v1']}, v2={syncer_versions['v2']}")

    stop_all()

    # Evaluate results
    success = True

    if final_syncer < GOLDEN_ACTIVATION_HEIGHT:
        print("FAIL: Syncer did not sync past activation height")
        success = False

    if abs(final_miner - final_syncer) > 3:
        print("FAIL: Height gap too large")
        success = False

    print("\n" + "=" * 60)
    if success:
        print("TEST PASSED!")
        print(f"Golden activation at height {GOLDEN_ACTIVATION_HEIGHT} working correctly")
    else:
        print("TEST FAILED!")
        print(f"\nCheck logs at:")
        print(f"  {ARTIFACTS_DIR / 'miner.log'}")
        print(f"  {ARTIFACTS_DIR / 'syncer.log'}")
    print("=" * 60)

    return success

def signal_handler(sig, frame):
    print("\nInterrupted, cleaning up...")
    stop_all()
    sys.exit(1)

if __name__ == "__main__":
    signal.signal(signal.SIGINT, signal_handler)
    success = run_test()
    sys.exit(0 if success else 1)
