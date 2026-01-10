#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
P4 Partition-Heal Test with ADZDB
=================================
Simple 2-node test to verify ADZDB chain and state sync works correctly.
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
ARTIFACTS_DIR = SCRIPT_DIR / "artifacts" / "P4_adzdb"

# Node configs
NODES = {
    "miner": {"rpc": 9950, "p2p": 30500, "mine": True},
    "syncer": {"rpc": 9951, "p2p": 30501, "mine": False},
}

processes = {}

def cleanup():
    """Kill processes and clean data."""
    print("Cleaning up...")
    # Kill any existing coinject processes
    if sys.platform == 'win32':
        subprocess.run(['taskkill', '/F', '/IM', 'coinject.exe'],
                      capture_output=True, text=True)
    time.sleep(2)

    # Clean artifacts
    if ARTIFACTS_DIR.exists():
        try:
            shutil.rmtree(ARTIFACTS_DIR)
        except PermissionError:
            print("Warning: Could not fully clean artifacts directory")
    ARTIFACTS_DIR.mkdir(parents=True, exist_ok=True)

def start_node(name: str, connect_to: str = None):
    """Start a node."""
    config = NODES[name]
    data_path = ARTIFACTS_DIR / name
    data_path.mkdir(parents=True, exist_ok=True)

    # Calculate WebSocket port offset from P2P port
    ws_port = config['p2p'] + 1000  # e.g., 30500 -> 31500

    cmd = [
        str(BINARY),
        "--data-dir", str(data_path),
        "--rpc-addr", f"127.0.0.1:{config['rpc']}",
        "--cpp-p2p-addr", f"0.0.0.0:{config['p2p']}",  # CPP P2P port
        "--cpp-ws-addr", f"0.0.0.0:{ws_port}",  # CPP WebSocket port
        "--difficulty", "2",
        "--block-time", "10",
    ]

    if config['mine']:
        cmd.append("--mine")
        cmd.append("--dev")  # Allow mining without peers

    if connect_to:
        # CPP format: IP:port (e.g., 127.0.0.1:30500)
        cmd.extend(["--bootnodes", connect_to])

    log_path = ARTIFACTS_DIR / f"{name}.log"
    log_file = open(log_path, "w", encoding="utf-8")

    print(f"Starting {name}...")
    print(f"  Command: {' '.join(cmd)}")

    proc = subprocess.Popen(cmd, stdout=log_file, stderr=subprocess.STDOUT)
    processes[name] = (proc, log_file)
    return proc

def get_cpp_peer_id(name: str) -> str:
    """Extract CPP PeerId from node log."""
    log_path = ARTIFACTS_DIR / f"{name}.log"
    if not log_path.exists():
        return None

    with open(log_path, "r", encoding="utf-8", errors="ignore") as f:
        for line in f:
            if "CPP PeerId:" in line:
                parts = line.split("CPP PeerId:")
                if len(parts) > 1:
                    return parts[1].strip()
    return None

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
    except Exception as e:
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

def check_logs_for_errors(name: str) -> list:
    """Check node logs for errors."""
    log_path = ARTIFACTS_DIR / f"{name}.log"
    errors = []
    if log_path.exists():
        with open(log_path, "r", encoding="utf-8", errors="ignore") as f:
            for line in f:
                if "Error:" in line or "error:" in line:
                    errors.append(line.strip())
    return errors

def run_test():
    """Run the P4 partition-heal test."""
    print("\n" + "=" * 60)
    print("P4 PARTITION-HEAL TEST (ADZDB)")
    print("=" * 60)

    cleanup()

    # Check binary exists
    if not BINARY.exists():
        print(f"ERROR: Binary not found at {BINARY}")
        return False

    print(f"\nUsing binary: {BINARY}")
    print(f"Artifacts: {ARTIFACTS_DIR}")

    # Phase 1: Start miner node
    print("\n--- Phase 1: Start Miner Node ---")
    start_node("miner")
    if not wait_for_node("miner", 30):
        print("FAILED: Miner node did not start")
        stop_all()
        return False

    # Wait for some blocks to be mined
    print("Letting miner produce blocks for 30s...")
    time.sleep(30)

    miner_height = get_node_height("miner")
    print(f"Miner height: {miner_height}")

    if miner_height < 2:
        print("FAILED: Miner did not produce blocks")
        stop_all()
        return False

    # Get miner's CPP peer address for syncer to connect
    miner_peer_id = get_cpp_peer_id("miner")
    if not miner_peer_id:
        print("FAILED: Could not get miner's CPP PeerId")
        stop_all()
        return False

    # CPP bootnode format is just IP:port (peer discovery happens via handshake)
    miner_addr = f"127.0.0.1:{NODES['miner']['p2p']}"
    print(f"Miner CPP address: {miner_addr}")

    # Phase 2: Start syncer node and let it sync
    print("\n--- Phase 2: Start Syncer Node ---")
    start_node("syncer", connect_to=miner_addr)
    if not wait_for_node("syncer", 30):
        print("FAILED: Syncer node did not start")
        stop_all()
        return False

    # Wait for sync
    print("Waiting for syncer to sync (60s)...")
    for i in range(12):
        time.sleep(5)
        syncer_height = get_node_height("syncer")
        miner_height = get_node_height("miner")
        print(f"  Miner: {miner_height}, Syncer: {syncer_height}")

        if syncer_height > 0 and abs(miner_height - syncer_height) <= 2:
            print("Syncer caught up!")
            break

    # Phase 3: Simulate partition (stop syncer)
    print("\n--- Phase 3: Simulate Partition ---")
    pre_partition_miner = get_node_height("miner")
    pre_partition_syncer = get_node_height("syncer")
    print(f"Pre-partition heights - Miner: {pre_partition_miner}, Syncer: {pre_partition_syncer}")

    stop_node("syncer")
    print("Syncer stopped (simulating partition)")

    # Let miner continue for a bit
    print("Miner continues for 30s during partition...")
    time.sleep(30)

    during_partition_miner = get_node_height("miner")
    print(f"Miner height during partition: {during_partition_miner}")

    # Phase 4: Heal partition (restart syncer)
    print("\n--- Phase 4: Heal Partition ---")
    start_node("syncer", connect_to=miner_addr)
    if not wait_for_node("syncer", 30):
        print("FAILED: Syncer did not restart")
        stop_all()
        return False

    # Wait for re-sync
    print("Waiting for syncer to re-sync (60s)...")
    converged = False
    for i in range(12):
        time.sleep(5)
        syncer_height = get_node_height("syncer")
        miner_height = get_node_height("miner")
        print(f"  Miner: {miner_height}, Syncer: {syncer_height}")

        if syncer_height > pre_partition_syncer and abs(miner_height - syncer_height) <= 2:
            print("Network converged!")
            converged = True
            break

    # Phase 5: Check results
    print("\n--- Phase 5: Results ---")
    final_miner = get_node_height("miner")
    final_syncer = get_node_height("syncer")

    print(f"Final heights - Miner: {final_miner}, Syncer: {final_syncer}")
    print(f"Gap: {abs(final_miner - final_syncer)} blocks")

    # Check for errors in logs
    miner_errors = check_logs_for_errors("miner")
    syncer_errors = check_logs_for_errors("syncer")

    # Filter out known non-critical errors
    critical_miner = [e for e in miner_errors if "DatabaseAlreadyOpen" not in e]
    critical_syncer = [e for e in syncer_errors if "DatabaseAlreadyOpen" not in e]

    success = True

    if final_miner < 5:
        print("FAIL: Miner did not produce enough blocks")
        success = False

    if final_syncer < pre_partition_syncer:
        print("FAIL: Syncer lost blocks during partition heal")
        success = False

    if not converged:
        print("FAIL: Network did not converge")
        success = False

    if abs(final_miner - final_syncer) > 3:
        print("FAIL: Height gap too large after convergence")
        success = False

    # Check for DatabaseAlreadyOpen errors (the bug we fixed)
    db_errors = [e for e in miner_errors + syncer_errors if "DatabaseAlreadyOpen" in e]
    if db_errors:
        print(f"FAIL: DatabaseAlreadyOpen errors detected ({len(db_errors)})")
        for e in db_errors[:3]:
            print(f"  {e}")
        success = False

    stop_all()

    print("\n" + "=" * 60)
    if success:
        print("TEST PASSED!")
    else:
        print("TEST FAILED!")
        print("\nCheck logs at:")
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
