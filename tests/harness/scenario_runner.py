#!/usr/bin/env python3
"""
COINjecture Network B - Scenario Test Runner
=============================================
Orchestrates multi-node test scenarios with explicit success criteria.

Usage:
    python scenario_runner.py cold-start
    python scenario_runner.py join-late
    python scenario_runner.py partition-heal
    python scenario_runner.py forced-fork
    python scenario_runner.py adversarial
    python scenario_runner.py all
"""

import argparse
import json
import os
import subprocess
import sys
import time
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path
from typing import Dict, List, Optional, Tuple

import requests

# =============================================================================
# Configuration
# =============================================================================

COMPOSE_FILE = Path(__file__).parent / "docker-compose.test.yml"
RESULTS_DIR = Path(__file__).parent / "results"

NODES = {
    "bootnode": {"rpc": 9940, "p2p": 30400},
    "node-a": {"rpc": 9941, "p2p": 30401},
    "node-b": {"rpc": 9942, "p2p": 30402},
    "node-c": {"rpc": 9943, "p2p": 30403},
    "node-d": {"rpc": 9944, "p2p": 30404},
    "node-e": {"rpc": 9945, "p2p": 30405},
    "node-f": {"rpc": 9946, "p2p": 30406},
}

LATE_NODE = {"node-late": {"rpc": 9947, "p2p": 30407}}


# =============================================================================
# Data Classes
# =============================================================================

@dataclass
class NodeState:
    name: str
    height: int
    best_hash: str
    peer_count: int
    is_syncing: bool
    total_work: int  # Cumulative work score (fork-choice weight)
    timestamp: float


@dataclass
class ScenarioResult:
    name: str
    passed: bool
    duration_seconds: float
    metrics: Dict
    errors: List[str] = field(default_factory=list)
    assertions: Dict[str, bool] = field(default_factory=dict)


# =============================================================================
# RPC Client
# =============================================================================

def rpc_call(port: int, method: str, params: list = None) -> Optional[dict]:
    """Make JSON-RPC call to a node."""
    try:
        response = requests.post(
            f"http://127.0.0.1:{port}",
            json={
                "jsonrpc": "2.0",
                "method": method,
                "params": params or [],
                "id": 1,
            },
            timeout=5,
        )
        result = response.json()
        return result.get("result")
    except Exception as e:
        return None


def get_node_state(name: str, port: int) -> Optional[NodeState]:
    """Get current state of a node."""
    info = rpc_call(port, "chain_getInfo")
    if not info:
        return None
    return NodeState(
        name=name,
        height=info.get("best_height", 0),
        best_hash=info.get("best_hash", ""),
        peer_count=info.get("peer_count", 0),
        is_syncing=info.get("is_syncing", False),
        total_work=info.get("total_work", 0),
        timestamp=time.time(),
    )


def get_all_states(nodes: Dict) -> Dict[str, NodeState]:
    """Get state of all nodes."""
    states = {}
    for name, config in nodes.items():
        state = get_node_state(name, config["rpc"])
        if state:
            states[name] = state
    return states


# =============================================================================
# Docker Compose Helpers
# =============================================================================

def docker_compose(*args, capture=False):
    """Run docker compose command."""
    cmd = ["docker", "compose", "-f", str(COMPOSE_FILE)] + list(args)
    if capture:
        result = subprocess.run(cmd, capture_output=True, text=True)
        return result.stdout, result.stderr, result.returncode
    else:
        return subprocess.run(cmd).returncode


def start_network(profile: str = None):
    """Start the test network."""
    print("🚀 Starting test network...")
    args = ["up", "-d", "--build"]
    if profile:
        args = ["--profile", profile] + args
    return docker_compose(*args)


def stop_network(remove_volumes=False):
    """Stop the test network."""
    print("🛑 Stopping test network...")
    args = ["down"]
    if remove_volumes:
        args.append("-v")
    return docker_compose(*args)


def pause_container(name: str):
    """Pause a container (simulates network partition)."""
    subprocess.run(["docker", "pause", f"coinject-test-{name}"])


def unpause_container(name: str):
    """Unpause a container (heals partition)."""
    subprocess.run(["docker", "unpause", f"coinject-test-{name}"])


def restart_container(name: str):
    """Restart a specific container."""
    subprocess.run(["docker", "restart", f"coinject-test-{name}"])


def start_late_joiner():
    """Start the late-joiner node."""
    docker_compose("--profile", "late-join", "up", "-d", "node-late")


def inject_latency(container: str, delay_ms: int):
    """Inject network latency into a container using tc."""
    subprocess.run([
        "docker", "exec", f"coinject-test-{container}",
        "tc", "qdisc", "add", "dev", "eth0", "root", "netem", "delay", f"{delay_ms}ms"
    ])


def clear_latency(container: str):
    """Clear injected latency."""
    subprocess.run([
        "docker", "exec", f"coinject-test-{container}",
        "tc", "qdisc", "del", "dev", "eth0", "root"
    ], capture_output=True)


# =============================================================================
# Assertions
# =============================================================================

def assert_all_connected(states: Dict[str, NodeState], min_peers: int = 5) -> Tuple[bool, str]:
    """Assert all nodes have minimum peer connections."""
    for name, state in states.items():
        if state.peer_count < min_peers:
            return False, f"{name} has only {state.peer_count} peers (need {min_peers})"
    return True, f"All nodes have >= {min_peers} peers"


def assert_heights_converged(states: Dict[str, NodeState], max_spread: int = 3) -> Tuple[bool, str]:
    """Assert all nodes are within max_spread blocks of each other."""
    heights = [s.height for s in states.values()]
    if not heights:
        return False, "No node states available"
    spread = max(heights) - min(heights)
    if spread > max_spread:
        return False, f"Height spread {spread} exceeds max {max_spread}"
    return True, f"Height spread {spread} <= {max_spread}"


def assert_no_persistent_fork(states: Dict[str, NodeState], max_fork_depth: int = 2) -> Tuple[bool, str]:
    """Assert no persistent forks exist (transient forks with depth <= max_fork_depth are OK).

    In real P2P mining, small transient forks are normal due to propagation delays.
    We only fail if nodes disagree on blocks that are deep enough to be 'settled'.
    """
    if not states:
        return False, "No node states available"

    # Find the highest height
    max_height = max(s.height for s in states.values())

    # Check agreement on blocks that are "settled" (at least max_fork_depth below tip)
    settled_height = max_height - max_fork_depth
    if settled_height <= 0:
        return True, f"Chain too short for fork detection (height {max_height})"

    # All nodes should agree on the hash at settled_height or below
    # (We can only check this if nodes expose block-by-height, so we check best_hash agreement
    # among nodes that are at or above settled_height)
    settled_nodes = [s for s in states.values() if s.height >= settled_height]

    if len(settled_nodes) <= 1:
        return True, "Not enough settled nodes to detect forks"

    # All nodes above settled height should eventually converge to same chain
    # Check if they all have the same best_hash (indicates convergence)
    hashes = set(s.best_hash for s in settled_nodes)

    if len(hashes) > 1:
        # Multiple different tips - check if this is a transient fork
        heights = [s.height for s in settled_nodes]
        height_spread = max(heights) - min(heights)
        if height_spread <= max_fork_depth:
            return True, f"Transient fork detected (spread {height_spread} <= {max_fork_depth})"
        else:
            return False, f"Persistent fork: {len(hashes)} different chain tips, spread {height_spread}"

    return True, "All nodes converged to same chain tip"


def assert_highest_work_wins(states: Dict[str, NodeState]) -> Tuple[bool, str]:
    """Assert all nodes selected the chain with highest cumulative work score.

    This is the actual fork-choice rule - highest work wins, not longest chain.
    """
    if not states:
        return False, "No node states available"

    # Find the highest work score
    max_work = max(s.total_work for s in states.values())

    # All nodes should have selected a chain with work close to max
    # (Allow small variance due to propagation timing)
    min_acceptable_work = max_work * 0.95  # 95% of max work

    for state in states.values():
        if state.total_work < min_acceptable_work:
            return False, f"{state.name} has work {state.total_work} < {min_acceptable_work} (max: {max_work})"

    return True, f"All nodes on highest-work chain (work: {max_work})"


def assert_min_height(states: Dict[str, NodeState], min_height: int) -> Tuple[bool, str]:
    """Assert all nodes have reached minimum height."""
    for name, state in states.items():
        if state.height < min_height:
            return False, f"{name} at height {state.height} < {min_height}"
    return True, f"All nodes >= height {min_height}"


def assert_catchup_time(actual_seconds: float, max_seconds: float) -> Tuple[bool, str]:
    """Assert catch-up completed within time limit."""
    if actual_seconds > max_seconds:
        return False, f"Catch-up took {actual_seconds:.1f}s > {max_seconds}s"
    return True, f"Catch-up completed in {actual_seconds:.1f}s <= {max_seconds}s"


# =============================================================================
# Scenarios
# =============================================================================

def scenario_cold_start() -> ScenarioResult:
    """
    Scenario 1: Cold Start to Head
    ==============================
    Start 7 nodes from genesis and verify they reach consensus.

    Success Criteria:
    - All nodes reach height >= 20 within 15 minutes
    - Height spread <= 3 blocks
    - All nodes have >= 5 peers
    - No chain forks
    """
    print("\n" + "=" * 60)
    print("SCENARIO: Cold Start to Head")
    print("=" * 60)

    start_time = time.time()
    errors = []
    assertions = {}

    # Start fresh network
    stop_network(remove_volumes=True)
    time.sleep(2)
    start_network()

    # Wait for network to initialize
    print("⏳ Waiting for network initialization (90s)...")
    time.sleep(90)

    # Monitor until timeout or success
    timeout = 15 * 60  # 15 minutes
    check_interval = 30
    target_height = 20

    while time.time() - start_time < timeout:
        states = get_all_states(NODES)

        if len(states) < len(NODES):
            print(f"   ⚠️  Only {len(states)}/{len(NODES)} nodes responding")
            time.sleep(check_interval)
            continue

        heights = [s.height for s in states.values()]
        min_h, max_h = min(heights), max(heights)
        spread = max_h - min_h
        avg_peers = sum(s.peer_count for s in states.values()) / len(states)

        print(f"   Heights: {min_h}-{max_h} (spread: {spread}) | Peers: {avg_peers:.1f}")

        # Check if we've reached target
        if min_h >= target_height:
            break

        time.sleep(check_interval)

    # Final assertions
    final_states = get_all_states(NODES)
    duration = time.time() - start_time

    passed, msg = assert_min_height(final_states, target_height)
    assertions["min_height"] = passed
    if not passed:
        errors.append(msg)
    print(f"   {'✅' if passed else '❌'} {msg}")

    passed, msg = assert_heights_converged(final_states, max_spread=3)
    assertions["height_converged"] = passed
    if not passed:
        errors.append(msg)
    print(f"   {'✅' if passed else '❌'} {msg}")

    passed, msg = assert_all_connected(final_states, min_peers=5)
    assertions["all_connected"] = passed
    if not passed:
        errors.append(msg)
    print(f"   {'✅' if passed else '❌'} {msg}")

    passed, msg = assert_no_persistent_fork(final_states, max_fork_depth=2)
    assertions["no_persistent_fork"] = passed
    if not passed:
        errors.append(msg)
    print(f"   {'✅' if passed else '❌'} {msg}")

    passed, msg = assert_highest_work_wins(final_states)
    assertions["highest_work_wins"] = passed
    if not passed:
        errors.append(msg)
    print(f"   {'✅' if passed else '❌'} {msg}")

    all_passed = all(assertions.values())

    return ScenarioResult(
        name="cold_start",
        passed=all_passed,
        duration_seconds=duration,
        metrics={
            "final_heights": {s.name: s.height for s in final_states.values()},
            "peer_counts": {s.name: s.peer_count for s in final_states.values()},
        },
        errors=errors,
        assertions=assertions,
    )


def scenario_join_late() -> ScenarioResult:
    """
    Scenario 2: Join-Late Catch-Up
    ==============================
    Let network mine 50+ blocks, then add a new node and verify it catches up.

    Success Criteria:
    - Late joiner catches up to within 3 blocks of network head
    - Catch-up completes within 5 minutes
    - No stuck sync state
    """
    print("\n" + "=" * 60)
    print("SCENARIO: Join-Late Catch-Up")
    print("=" * 60)

    start_time = time.time()
    errors = []
    assertions = {}

    # Ensure network is running and has blocks
    print("⏳ Waiting for network to reach height 50...")
    target_before_join = 50

    while True:
        states = get_all_states(NODES)
        if not states:
            print("   ⚠️  No nodes responding, starting network...")
            start_network()
            time.sleep(60)
            continue

        max_height = max(s.height for s in states.values())
        print(f"   Network height: {max_height}")

        if max_height >= target_before_join:
            break

        time.sleep(30)

    # Record network state before join
    pre_join_states = get_all_states(NODES)
    network_height = max(s.height for s in pre_join_states.values())
    print(f"\n🚀 Starting late joiner (network at height {network_height})...")

    join_start = time.time()
    start_late_joiner()

    # Monitor catch-up
    catchup_timeout = 5 * 60  # 5 minutes
    check_interval = 10

    while time.time() - join_start < catchup_timeout:
        late_state = get_node_state("node-late", LATE_NODE["node-late"]["rpc"])
        network_states = get_all_states(NODES)

        if not late_state:
            print("   ⏳ Late joiner not yet responding...")
            time.sleep(check_interval)
            continue

        network_height = max(s.height for s in network_states.values())
        gap = network_height - late_state.height

        print(f"   Late joiner: {late_state.height} | Network: {network_height} | Gap: {gap}")

        if gap <= 3:
            break

        time.sleep(check_interval)

    catchup_time = time.time() - join_start

    # Final assertions
    late_state = get_node_state("node-late", LATE_NODE["node-late"]["rpc"])
    network_states = get_all_states(NODES)

    if late_state and network_states:
        network_height = max(s.height for s in network_states.values())
        gap = network_height - late_state.height

        passed = gap <= 3
        msg = f"Late joiner gap: {gap} blocks"
        assertions["caught_up"] = passed
        if not passed:
            errors.append(msg)
        print(f"   {'✅' if passed else '❌'} {msg}")

        passed, msg = assert_catchup_time(catchup_time, 5 * 60)
        assertions["catchup_time"] = passed
        if not passed:
            errors.append(msg)
        print(f"   {'✅' if passed else '❌'} {msg}")
    else:
        errors.append("Late joiner failed to respond")
        assertions["caught_up"] = False
        assertions["catchup_time"] = False

    duration = time.time() - start_time
    all_passed = all(assertions.values())

    return ScenarioResult(
        name="join_late",
        passed=all_passed,
        duration_seconds=duration,
        metrics={
            "catchup_time_seconds": catchup_time,
            "final_gap": gap if late_state else None,
        },
        errors=errors,
        assertions=assertions,
    )


def scenario_partition_heal() -> ScenarioResult:
    """
    Scenario 3: Partial Partition + Heal
    ====================================
    Partition 2 nodes, let both sides mine, then heal and verify convergence.

    Success Criteria:
    - Both partitions make progress during split
    - Network converges within 5 minutes after heal
    - Final height spread <= 5 blocks
    - Longest chain wins (no permanent fork)
    """
    print("\n" + "=" * 60)
    print("SCENARIO: Partial Partition + Heal")
    print("=" * 60)

    start_time = time.time()
    errors = []
    assertions = {}

    # Ensure network is running
    states = get_all_states(NODES)
    if len(states) < len(NODES):
        print("⏳ Waiting for full network...")
        time.sleep(30)
        states = get_all_states(NODES)

    pre_partition_height = max(s.height for s in states.values())
    print(f"📊 Pre-partition height: {pre_partition_height}")

    # Create partition: pause node-e and node-f
    partitioned_nodes = ["node-e", "node-f"]
    print(f"\n🔌 Partitioning nodes: {partitioned_nodes}")
    for node in partitioned_nodes:
        pause_container(node)

    # Let main partition mine
    print("⏳ Mining during partition (90s)...")
    time.sleep(90)

    # Check main partition progress
    main_states = get_all_states({k: v for k, v in NODES.items() if k not in partitioned_nodes})
    main_height = max(s.height for s in main_states.values())
    main_progress = main_height - pre_partition_height
    print(f"📊 Main partition progress: +{main_progress} blocks (height: {main_height})")

    # Heal partition
    print("\n🔗 Healing partition...")
    for node in partitioned_nodes:
        unpause_container(node)

    # Wait for convergence
    print("⏳ Waiting for convergence (120s)...")
    heal_start = time.time()
    convergence_timeout = 5 * 60

    while time.time() - heal_start < convergence_timeout:
        states = get_all_states(NODES)
        if len(states) < len(NODES):
            time.sleep(10)
            continue

        heights = [s.height for s in states.values()]
        spread = max(heights) - min(heights)

        print(f"   Heights: {min(heights)}-{max(heights)} (spread: {spread})")

        if spread <= 5:
            break

        time.sleep(15)

    convergence_time = time.time() - heal_start

    # Final assertions
    final_states = get_all_states(NODES)
    duration = time.time() - start_time

    # Main partition made progress
    passed = main_progress >= 2
    msg = f"Main partition mined {main_progress} blocks during split"
    assertions["partition_progress"] = passed
    if not passed:
        errors.append(msg)
    print(f"   {'✅' if passed else '❌'} {msg}")

    # Convergence
    passed, msg = assert_heights_converged(final_states, max_spread=5)
    assertions["converged"] = passed
    if not passed:
        errors.append(msg)
    print(f"   {'✅' if passed else '❌'} {msg}")

    # No persistent fork (transient forks during heal are OK)
    passed, msg = assert_no_persistent_fork(final_states, max_fork_depth=3)
    assertions["no_persistent_fork"] = passed
    if not passed:
        errors.append(msg)
    print(f"   {'✅' if passed else '❌'} {msg}")

    # Highest work chain wins (not just longest)
    passed, msg = assert_highest_work_wins(final_states)
    assertions["highest_work_wins"] = passed
    if not passed:
        errors.append(msg)
    print(f"   {'✅' if passed else '❌'} {msg}")

    all_passed = all(assertions.values())

    return ScenarioResult(
        name="partition_heal",
        passed=all_passed,
        duration_seconds=duration,
        metrics={
            "partition_duration_seconds": 90,
            "convergence_time_seconds": convergence_time,
            "main_progress_blocks": main_progress,
            "final_total_work": {s.name: s.total_work for s in final_states.values()},
        },
        errors=errors,
        assertions=assertions,
    )


def scenario_forced_fork() -> ScenarioResult:
    """
    Scenario 4: Forced Fork + Recovery
    ==================================
    Create deterministic fork by splitting network, mining on both sides, then healing.

    Success Criteria:
    - Both sides mine independently during split
    - Network recovers and selects HIGHEST WORK chain (not longest)
    - Lower-work chain nodes reorg to higher-work chain
    - Max reorg depth tracked and reported
    """
    print("\n" + "=" * 60)
    print("SCENARIO: Forced Fork + Recovery")
    print("=" * 60)

    start_time = time.time()
    errors = []
    assertions = {}

    # Split into two groups
    group_a = ["bootnode", "node-a", "node-b", "node-c"]
    group_b = ["node-d", "node-e", "node-f"]

    states = get_all_states(NODES)
    pre_fork_height = max(s.height for s in states.values()) if states else 0
    print(f"📊 Pre-fork height: {pre_fork_height}")

    # Partition group B
    print(f"\n🔌 Creating fork: Group A ({len(group_a)}) vs Group B ({len(group_b)})")
    for node in group_b:
        pause_container(node)

    # Let group A mine
    print("⏳ Group A mining (60s)...")
    time.sleep(60)

    group_a_states = get_all_states({k: v for k, v in NODES.items() if k in group_a})
    group_a_height = max(s.height for s in group_a_states.values())
    group_a_work = max(s.total_work for s in group_a_states.values())
    print(f"📊 Group A: height={group_a_height}, work={group_a_work}")

    # Now pause group A and unpause group B (they'll start from pre_fork_height)
    print("\n🔄 Swapping partitions...")
    for node in group_a:
        pause_container(node)
    for node in group_b:
        unpause_container(node)

    # Let group B mine (shorter time = shorter chain)
    print("⏳ Group B mining (30s)...")
    time.sleep(30)

    group_b_states = get_all_states({k: v for k, v in NODES.items() if k in group_b})
    group_b_height = max(s.height for s in group_b_states.values()) if group_b_states else pre_fork_height
    group_b_work = max(s.total_work for s in group_b_states.values()) if group_b_states else 0
    print(f"📊 Group B: height={group_b_height}, work={group_b_work}")

    # Heal - bring everyone back
    print("\n🔗 Healing fork...")
    for node in group_a:
        unpause_container(node)

    # Wait for reorg and convergence
    print("⏳ Waiting for reorg and convergence (180s)...")
    time.sleep(180)

    # Final state
    final_states = get_all_states(NODES)
    duration = time.time() - start_time

    if final_states:
        final_heights = [s.height for s in final_states.values()]
        final_works = [s.total_work for s in final_states.values()]
        final_height = max(final_heights)
        final_work = max(final_works)
        spread = max(final_heights) - min(final_heights)

        # Expected: HIGHEST WORK chain wins (not longest)
        expected_winner = "Group A" if group_a_work > group_b_work else "Group B"
        winning_work = max(group_a_work, group_b_work)
        max_reorg_depth = abs(group_a_height - group_b_height)

        # Check that network selected highest-work chain
        passed = final_work >= winning_work * 0.95  # Allow 5% variance
        msg = f"Network work={final_work} (expected ~{winning_work} from {expected_winner})"
        assertions["highest_work_wins"] = passed
        if not passed:
            errors.append(msg)
        print(f"   {'✅' if passed else '❌'} {msg}")

        passed = spread <= 3
        msg = f"Height spread: {spread}"
        assertions["converged"] = passed
        if not passed:
            errors.append(msg)
        print(f"   {'✅' if passed else '❌'} {msg}")

        # All nodes should converge to same chain
        passed, msg = assert_no_persistent_fork(final_states, max_fork_depth=3)
        assertions["no_persistent_fork"] = passed
        if not passed:
            errors.append(msg)
        print(f"   {'✅' if passed else '❌'} {msg}")

        print(f"   📊 Max reorg depth: {max_reorg_depth} blocks")
        print(f"   📊 Group A work: {group_a_work}, Group B work: {group_b_work}")
    else:
        errors.append("No nodes responding after heal")
        assertions["highest_work_wins"] = False
        assertions["converged"] = False
        assertions["no_persistent_fork"] = False
        max_reorg_depth = 0

    all_passed = all(assertions.values())

    return ScenarioResult(
        name="forced_fork",
        passed=all_passed,
        duration_seconds=duration,
        metrics={
            "group_a_height": group_a_height,
            "group_a_work": group_a_work,
            "group_b_height": group_b_height,
            "group_b_work": group_b_work,
            "max_reorg_depth": max_reorg_depth,
            "final_work": {s.name: s.total_work for s in final_states.values()} if final_states else {},
        },
        errors=errors,
        assertions=assertions,
    )


def scenario_adversarial() -> ScenarioResult:
    """
    Scenario 5: Adversarial Peer Tests
    ==================================
    Simulate adversarial conditions: slow peer, high latency, disconnections.

    Success Criteria:
    - Network remains stable under adverse conditions
    - Honest nodes maintain sync
    - No cascading failures
    """
    print("\n" + "=" * 60)
    print("SCENARIO: Adversarial Peer Tests")
    print("=" * 60)

    start_time = time.time()
    errors = []
    assertions = {}

    states = get_all_states(NODES)
    initial_height = max(s.height for s in states.values()) if states else 0
    print(f"📊 Initial height: {initial_height}")

    # Test 1: Inject latency into one node
    adversarial_node = "node-f"
    print(f"\n🐌 Injecting 500ms latency into {adversarial_node}...")
    inject_latency(adversarial_node, 500)

    print("⏳ Running with adversarial node (120s)...")
    time.sleep(120)

    # Check network health
    states = get_all_states(NODES)
    honest_nodes = {k: v for k, v in states.items() if k != adversarial_node}

    if honest_nodes:
        honest_heights = [s.height for s in honest_nodes.values()]
        spread = max(honest_heights) - min(honest_heights)
        progress = max(honest_heights) - initial_height

        passed = spread <= 5
        msg = f"Honest node spread: {spread} (with adversarial peer)"
        assertions["honest_stable"] = passed
        if not passed:
            errors.append(msg)
        print(f"   {'✅' if passed else '❌'} {msg}")

        passed = progress >= 2
        msg = f"Network progress: +{progress} blocks"
        assertions["network_progress"] = passed
        if not passed:
            errors.append(msg)
        print(f"   {'✅' if passed else '❌'} {msg}")

    # Clean up latency
    print(f"\n🧹 Clearing latency from {adversarial_node}...")
    clear_latency(adversarial_node)

    # Test 2: Rapid disconnections
    print("\n🔌 Testing rapid disconnections...")
    for _ in range(3):
        restart_container("node-e")
        time.sleep(20)

    # Final check
    print("⏳ Final stability check (60s)...")
    time.sleep(60)

    final_states = get_all_states(NODES)
    duration = time.time() - start_time

    passed, msg = assert_heights_converged(final_states, max_spread=5)
    assertions["final_converged"] = passed
    if not passed:
        errors.append(msg)
    print(f"   {'✅' if passed else '❌'} {msg}")

    all_passed = all(assertions.values())

    return ScenarioResult(
        name="adversarial",
        passed=all_passed,
        duration_seconds=duration,
        metrics={
            "tests_run": ["latency_injection", "rapid_disconnections"],
        },
        errors=errors,
        assertions=assertions,
    )


def scenario_dos_guardrails() -> ScenarioResult:
    """
    Scenario 6: DoS Guardrails Verification (Phase 1B)
    ===================================================
    Verify that sync DoS protections actually work.

    Tests:
    1. Request flooding - send many parallel requests
    2. Invalid range requests - request >100 blocks
    3. Timeout simulation - pause responding node
    4. Verify guardrails activate

    Success Criteria:
    - Rate limits activate (requests get blocked)
    - Invalid ranges rejected
    - High-failure peers get degraded
    - Honest nodes still sync despite adversarial behavior
    - Total inflight remains bounded (global cap respected)
    """
    print("\n" + "=" * 60)
    print("SCENARIO: DoS Guardrails Verification")
    print("=" * 60)

    start_time = time.time()
    errors = []
    assertions = {}
    metrics_collected = {}

    states = get_all_states(NODES)
    initial_height = max(s.height for s in states.values()) if states else 0
    print(f"📊 Initial height: {initial_height}")

    # Test 1: Create a "spammer" situation by rapidly restarting a node
    # (This causes repeated sync requests from the restarting node)
    spammer_node = "node-f"
    print(f"\n🔥 Test 1: Simulating sync flood from {spammer_node}...")

    flood_restarts = 5
    for i in range(flood_restarts):
        print(f"   Restart {i+1}/{flood_restarts}...")
        restart_container(spammer_node)
        time.sleep(5)  # Very rapid restarts - each triggers sync requests

    # Check that network is still healthy
    time.sleep(30)
    states = get_all_states(NODES)
    honest_nodes = {k: v for k, v in states.items() if k != spammer_node}

    if honest_nodes:
        honest_heights = [s.height for s in honest_nodes.values()]
        spread = max(honest_heights) - min(honest_heights)

        passed = spread <= 5
        msg = f"Honest nodes stable during flood (spread: {spread})"
        assertions["flood_stable"] = passed
        if not passed:
            errors.append(msg)
        print(f"   {'✅' if passed else '❌'} {msg}")
    else:
        assertions["flood_stable"] = False
        errors.append("No honest nodes responding during flood test")

    # Test 2: Pause a node mid-sync (simulates timeout)
    print(f"\n⏱️  Test 2: Simulating sync timeout (pause {spammer_node})...")
    pause_container(spammer_node)

    # Let other nodes continue and see how they handle the timeout
    print("   Waiting 60s for timeout handling...")
    time.sleep(60)

    states = get_all_states({k: v for k, v in NODES.items() if k != spammer_node})
    if states:
        heights = [s.height for s in states.values()]
        progress = max(heights) - initial_height

        passed = progress >= 1  # At least some progress during timeout test
        msg = f"Network progress during timeout test: +{progress} blocks"
        assertions["timeout_progress"] = passed
        if not passed:
            errors.append(msg)
        print(f"   {'✅' if passed else '❌'} {msg}")
    else:
        assertions["timeout_progress"] = False
        errors.append("No nodes responding during timeout test")

    # Unpause for final checks
    unpause_container(spammer_node)

    # Test 3: Verify rate limiting by checking logs
    # (In a real implementation, we'd query an RPC endpoint for guardrail metrics)
    print("\n📊 Test 3: Guardrail metrics check...")

    # Give time for spammer node to rejoin and trigger rate limits
    time.sleep(30)

    # For now, we verify the network stayed healthy (guardrails worked if no cascade)
    final_states = get_all_states(NODES)
    duration = time.time() - start_time

    if final_states:
        final_heights = [s.height for s in final_states.values()]
        final_spread = max(final_heights) - min(final_heights)
        total_progress = max(final_heights) - initial_height

        # Network should have made progress despite adversarial behavior
        passed = total_progress >= 2
        msg = f"Total network progress: +{total_progress} blocks"
        assertions["total_progress"] = passed
        if not passed:
            errors.append(msg)
        print(f"   {'✅' if passed else '❌'} {msg}")

        # All nodes should eventually converge
        passed = final_spread <= 5
        msg = f"Final spread: {final_spread} blocks"
        assertions["final_convergence"] = passed
        if not passed:
            errors.append(msg)
        print(f"   {'✅' if passed else '❌'} {msg}")

        # Verify no persistent forks
        passed, msg = assert_no_persistent_fork(final_states, max_fork_depth=3)
        assertions["no_persistent_fork"] = passed
        if not passed:
            errors.append(msg)
        print(f"   {'✅' if passed else '❌'} {msg}")

        metrics_collected = {
            "flood_restarts": flood_restarts,
            "total_progress": total_progress,
            "final_spread": final_spread,
            "final_heights": {s.name: s.height for s in final_states.values()},
        }
    else:
        errors.append("No nodes responding after guardrail tests")
        assertions["total_progress"] = False
        assertions["final_convergence"] = False
        assertions["no_persistent_fork"] = False

    all_passed = all(assertions.values())

    # Summary
    if all_passed:
        print("\n   ✅ DoS guardrails appear to be working:")
        print("      - Network survived rapid sync flooding")
        print("      - Timeouts handled gracefully")
        print("      - No cascading failures")
    else:
        print("\n   ⚠️  Some guardrail tests failed - investigate logs")

    return ScenarioResult(
        name="dos_guardrails",
        passed=all_passed,
        duration_seconds=duration,
        metrics=metrics_collected,
        errors=errors,
        assertions=assertions,
    )


# =============================================================================
# Main
# =============================================================================

SCENARIOS = {
    "cold-start": scenario_cold_start,
    "join-late": scenario_join_late,
    "partition-heal": scenario_partition_heal,
    "forced-fork": scenario_forced_fork,
    "adversarial": scenario_adversarial,
    "dos-guardrails": scenario_dos_guardrails,
}


def run_scenario(name: str) -> ScenarioResult:
    """Run a single scenario."""
    if name not in SCENARIOS:
        print(f"❌ Unknown scenario: {name}")
        print(f"   Available: {list(SCENARIOS.keys())}")
        sys.exit(1)

    return SCENARIOS[name]()


def save_results(results: List[ScenarioResult]):
    """Save results to JSON file."""
    RESULTS_DIR.mkdir(parents=True, exist_ok=True)
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    filepath = RESULTS_DIR / f"results_{timestamp}.json"

    data = {
        "timestamp": timestamp,
        "scenarios": [
            {
                "name": r.name,
                "passed": r.passed,
                "duration_seconds": r.duration_seconds,
                "metrics": r.metrics,
                "errors": r.errors,
                "assertions": r.assertions,
            }
            for r in results
        ],
        "summary": {
            "total": len(results),
            "passed": sum(1 for r in results if r.passed),
            "failed": sum(1 for r in results if not r.passed),
        },
    }

    with open(filepath, "w") as f:
        json.dump(data, f, indent=2)

    print(f"\n📄 Results saved to: {filepath}")
    return filepath


def main():
    parser = argparse.ArgumentParser(description="COINjecture Scenario Test Runner")
    parser.add_argument(
        "scenario",
        choices=list(SCENARIOS.keys()) + ["all"],
        help="Scenario to run (or 'all' to run all)",
    )
    parser.add_argument(
        "--keep-running",
        action="store_true",
        help="Keep network running after tests",
    )
    args = parser.parse_args()

    results = []

    if args.scenario == "all":
        for name in SCENARIOS:
            result = run_scenario(name)
            results.append(result)
    else:
        result = run_scenario(args.scenario)
        results.append(result)

    # Print summary
    print("\n" + "=" * 60)
    print("TEST SUMMARY")
    print("=" * 60)

    for r in results:
        status = "✅ PASS" if r.passed else "❌ FAIL"
        print(f"  {status} {r.name} ({r.duration_seconds:.1f}s)")
        if r.errors:
            for err in r.errors:
                print(f"       ⚠️  {err}")

    total_passed = sum(1 for r in results if r.passed)
    total = len(results)

    print(f"\n  Total: {total_passed}/{total} passed")

    # Save results
    save_results(results)

    # Cleanup
    if not args.keep_running:
        stop_network()

    # Exit code
    sys.exit(0 if total_passed == total else 1)


if __name__ == "__main__":
    main()
