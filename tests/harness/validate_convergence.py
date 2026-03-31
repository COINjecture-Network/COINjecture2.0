#!/usr/bin/env python3
"""
Validation script for COINjecture Network B - runs multiple passes and validates convergence.
"""

import argparse
import os
import shutil
import subprocess
import sys
import time
import json
from pathlib import Path

DATA_DIR = Path(__file__).parent / "validation_data"
HARNESS_SCRIPT = Path(__file__).parent / "local_harness.py"

def kill_nodes():
    """Kill all coinject processes."""
    if sys.platform == 'win32':
        subprocess.run(['taskkill', '/F', '/IM', 'coinject.exe'],
                       capture_output=True, text=True)
    else:
        subprocess.run(['pkill', '-9', 'coinject'],
                       capture_output=True, text=True)
    time.sleep(3)

def clean_data_dir():
    """Remove data directory with retries for locked files."""
    if not DATA_DIR.exists():
        return True

    for attempt in range(3):
        try:
            shutil.rmtree(DATA_DIR)
            return True
        except PermissionError:
            # Kill any remaining processes and wait
            if sys.platform == 'win32':
                subprocess.run(['taskkill', '/F', '/IM', 'coinject.exe'],
                               capture_output=True, text=True)
            time.sleep(2)

    # If we can't delete, try to just delete the log files and node directories
    try:
        for item in DATA_DIR.iterdir():
            if item.is_dir():
                shutil.rmtree(item, ignore_errors=True)
            else:
                try:
                    item.unlink()
                except PermissionError:
                    pass
        return True  # Proceed anyway
    except Exception:
        pass

    print("Warning: Could not fully clean data directory")
    return True  # Proceed anyway - harness will recreate what it needs

def check_convergence():
    """Check if all nodes have converged on the same block."""
    heights = {}
    hashes = {}

    for log_name in ['bootnode', 'node-a', 'node-b', 'node-c']:
        log_path = DATA_DIR / f"{log_name}.log"
        if not log_path.exists():
            return None, None, f"Log file {log_path} not found"

        # Read the log and find the best height and hash
        # Mining nodes: "New best block: height=70 hash=Hash(0082d33564df754d)"
        # Syncing nodes: "(current height: 102)" with hash from status updates
        with open(log_path, 'r', encoding='utf-8', errors='ignore') as f:
            last_best = None
            last_height = 0
            last_hash = None

            for line in f:
                # Check for mining node "New best block" message
                if "New best block:" in line:
                    last_best = line

                # Check for syncing node "current height" from various messages
                if "(current height:" in line:
                    try:
                        height_str = line.split("(current height:")[1].split(")")[0].strip()
                        current_height = int(height_str)
                        if current_height > last_height:
                            last_height = current_height
                    except (IndexError, ValueError):
                        pass

                # Status updates have hash info: "hash=Hash(xxx)"
                if "hash=Hash(" in line and "ours:" in line:
                    try:
                        hash_part = line.split("ours:")[1].split("hash=Hash(")[1].split(")")[0]
                        height_part = line.split("ours:")[1].split()[0]
                        current_height = int(height_part)
                        if current_height > last_height:
                            last_height = current_height
                            last_hash = hash_part
                    except (IndexError, ValueError):
                        pass

        # Prefer "New best block" format if available
        if last_best:
            try:
                parts = last_best.split("height=")[1]
                height = int(parts.split()[0])
                hash_part = last_best.split("hash=Hash(")[1].split(")")[0]
                heights[log_name] = height
                hashes[log_name] = hash_part
            except (IndexError, ValueError):
                return None, None, f"Could not parse log for {log_name}"
        elif last_height > 0 and last_hash:
            heights[log_name] = last_height
            hashes[log_name] = last_hash
        elif last_height > 0:
            # Just got height, no hash - use height only for now
            heights[log_name] = last_height
            hashes[log_name] = "unknown"
        else:
            return None, None, f"No height info found for {log_name}"

    return heights, hashes, None

def check_errors():
    """Check for 'Invalid previous hash' errors in logs."""
    error_count = 0
    for log_name in ['bootnode', 'node-a', 'node-b', 'node-c']:
        log_path = DATA_DIR / f"{log_name}.log"
        if log_path.exists():
            with open(log_path, 'r', encoding='utf-8', errors='ignore') as f:
                for line in f:
                    if "Invalid previous hash" in line:
                        error_count += 1
    return error_count

def run_single_pass(pass_num, wait_time=90, min_height=30, peer_mine=False):
    """Run a single test pass and validate convergence."""
    print(f"\n{'='*60}")
    print(f"PASS {pass_num} - Starting...")
    print(f"{'='*60}")

    # Kill any existing nodes and clean data
    kill_nodes()
    if not clean_data_dir():
        print("[FAIL] Could not clean data directory - skipping pass")
        return False

    # Start harness
    mode = "all mining" if peer_mine else "single miner"
    print(f"Starting harness ({mode})...")
    cmd = [sys.executable, str(HARNESS_SCRIPT), '--nodes', '4']
    if peer_mine:
        cmd.append('--peer-mine')
    proc = subprocess.Popen(
        cmd,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True
    )

    # Wait for nodes to mine and sync
    print(f"Waiting {wait_time} seconds for mining and sync...")
    time.sleep(wait_time)

    # Check convergence
    heights, hashes, error = check_convergence()

    if error:
        print(f"[FAIL] {error}")
        kill_nodes()
        return False

    # Check if all heights are the same and above minimum
    unique_heights = set(heights.values())
    unique_hashes = set(hashes.values())

    max_height = max(heights.values())

    print(f"\nResults:")
    for name in ['bootnode', 'node-a', 'node-b', 'node-c']:
        print(f"  {name}: height={heights[name]}, hash={hashes[name][:12]}...")

    # Check for errors
    error_count = check_errors()
    print(f"\n'Invalid previous hash' errors: {error_count}")

    # Validation
    passed = True

    if len(unique_hashes) != 1:
        print(f"[FAIL] Nodes have different hashes! {unique_hashes}")
        passed = False
    else:
        print(f"[PASS] All nodes converged on same hash")

    if max_height < min_height:
        print(f"[FAIL] Max height {max_height} is below minimum {min_height}")
        passed = False
    else:
        print(f"[PASS] Height {max_height} >= minimum {min_height}")

    if error_count > 0:
        print(f"[FAIL] {error_count} 'Invalid previous hash' errors found")
        passed = False
    else:
        print(f"[PASS] No 'Invalid previous hash' errors")

    # Kill nodes for cleanup
    kill_nodes()

    return passed

def main():
    parser = argparse.ArgumentParser(description="Run validation passes")
    parser.add_argument("--passes", type=int, default=10, help="Number of passes to run")
    parser.add_argument("--wait", type=int, default=90, help="Seconds to wait per pass")
    parser.add_argument("--min-height", type=int, default=30, help="Minimum height for pass")
    parser.add_argument("--peer-mine", action="store_true", help="Enable mining on peer nodes")
    args = parser.parse_args()

    mode = "all nodes mining" if args.peer_mine else "single miner (bootnode only)"
    print(f"Running {args.passes} validation passes...")
    print(f"Mining mode: {mode}")
    print(f"Wait time per pass: {args.wait} seconds")
    print(f"Minimum height required: {args.min_height}")

    passed = 0
    failed = 0

    for i in range(1, args.passes + 1):
        if run_single_pass(i, args.wait, args.min_height, args.peer_mine):
            passed += 1
            print(f"\n[SUCCESS] PASS {i}/{args.passes} SUCCEEDED")
        else:
            failed += 1
            print(f"\n[FAILURE] PASS {i}/{args.passes} FAILED")
            # Continue to next pass even on failure

    print(f"\n{'='*60}")
    print(f"FINAL RESULTS: {passed}/{args.passes} passes succeeded")
    print(f"{'='*60}")

    if failed > 0:
        sys.exit(1)

if __name__ == "__main__":
    main()
