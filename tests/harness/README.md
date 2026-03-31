# COINjecture Network B - Multi-Node Test Harness

Phase 1A test infrastructure for validating network stability, sync behavior, and fork recovery.

## Quick Start

```bash
# Install Python dependencies
pip install requests

# Run all scenarios
python tests/harness/scenario_runner.py all

# Run specific scenario
python tests/harness/scenario_runner.py cold-start
python tests/harness/scenario_runner.py join-late
python tests/harness/scenario_runner.py partition-heal
python tests/harness/scenario_runner.py forced-fork
python tests/harness/scenario_runner.py adversarial

# Keep network running after tests (for debugging)
python tests/harness/scenario_runner.py cold-start --keep-running
```

## Scenarios

### 1. Cold Start to Head (`cold-start`)

Starts 7 nodes from genesis and verifies consensus.

**Success Criteria:**
- All nodes reach height >= 20 within 15 minutes
- Height spread <= 3 blocks
- All nodes have >= 5 peers
- No chain forks

### 2. Join-Late Catch-Up (`join-late`)

Lets network mine 50+ blocks, then adds a new node.

**Success Criteria:**
- Late joiner catches up to within 3 blocks of network head
- Catch-up completes within 5 minutes
- No stuck sync state

### 3. Partial Partition + Heal (`partition-heal`)

Partitions 2 nodes, lets both sides mine, then heals.

**Success Criteria:**
- Both partitions make progress during split
- Network converges within 5 minutes after heal
- Final height spread <= 5 blocks
- Longest chain wins (no permanent fork)

### 4. Forced Fork + Recovery (`forced-fork`)

Creates deterministic fork by splitting network 4-3.

**Success Criteria:**
- Both sides mine independently during split
- Network recovers and selects longest chain
- Shorter chain nodes reorg to longer chain
- Max reorg depth tracked and reported

### 5. Adversarial Peer Tests (`adversarial`)

Simulates adversarial conditions: slow peer, high latency, rapid disconnections.

**Success Criteria:**
- Network remains stable under adverse conditions
- Honest nodes maintain sync
- No cascading failures

## Architecture

```
tests/harness/
├── docker-compose.test.yml   # 7-node topology
├── scenario_runner.py        # Test orchestration
├── results/                  # JSON test results
└── README.md
```

## Network Topology

```
                    ┌─────────────┐
                    │   bootnode  │
                    │  (primary)  │
                    └──────┬──────┘
                           │
        ┌──────┬───────┬───┴───┬───────┬──────┐
        │      │       │       │       │      │
     node-a  node-b  node-c  node-d  node-e  node-f
        │      │       │       │       │      │
        └──────┴───────┴───────┴───────┴──────┘
                           │
                    ┌──────┴──────┐
                    │  node-late  │
                    │ (on-demand) │
                    └─────────────┘
```

## Configuration

Environment variables in `docker-compose.test.yml`:

| Variable | Default | Description |
|----------|---------|-------------|
| `DIFFICULTY` | 3 | Mining difficulty |
| `BLOCK_TIME` | 30 | Target block time (seconds) |
| `RUST_LOG` | info | Log level |

## Results

Results are saved to `tests/harness/results/results_YYYYMMDD_HHMMSS.json`:

```json
{
  "timestamp": "20241216_143022",
  "scenarios": [
    {
      "name": "cold_start",
      "passed": true,
      "duration_seconds": 542.3,
      "metrics": {
        "final_heights": {"bootnode": 25, "node-a": 25, ...}
      },
      "assertions": {
        "min_height": true,
        "height_converged": true,
        "all_connected": true,
        "no_fork": true
      }
    }
  ],
  "summary": {
    "total": 5,
    "passed": 5,
    "failed": 0
  }
}
```

## Troubleshooting

### Nodes not connecting

Check Docker network:
```bash
docker network inspect coinject-test-net
```

### View node logs

```bash
docker logs -f coinject-test-bootnode
docker logs -f coinject-test-node-a
```

### Manual network control

```bash
# Start network
docker compose -f tests/harness/docker-compose.test.yml up -d

# Stop network (keep data)
docker compose -f tests/harness/docker-compose.test.yml down

# Stop and remove data
docker compose -f tests/harness/docker-compose.test.yml down -v

# Pause/unpause for partition simulation
docker pause coinject-test-node-f
docker unpause coinject-test-node-f
```
