# v4 Genesis Reset — w/√W Tokenomics + Higher Initial Difficulty

Fresh chain **`coinject-network-b-v4`**. No height-gated fork on v3. All mesh hosts must wipe chain volumes at cutover and redeploy together.

## What changes in v4

| Parameter | v3 | v4 |
|-----------|----|----|
| `chain_id` | `coinject-network-b-v3` | **`coinject-network-b-v4`** |
| Emission law | `⌊w·S·K / W⌋` | **`⌊w·S·K / isqrt(W)⌋`** (`K=50`) |
| Header PoW (`--difficulty`) | 4 leading hex zeros | **5** |
| NP bootstrap size | 90 | **110** (`BOOTSTRAP_CURRENT_SIZE`) |

**Block-2 first harvest** (genesis `w=10`, `W_parent=10`): ~**167 BEANS** with `K=50` on w/√W.

**Lifetime target:** ~1 BEANS/block average from genesis through block 100,000 (constant `w=10` per block). See `docs/charts/emission_decay.html` and `scripts/emission_decay_chart.py`.

## Prerequisites (before cutover)

1. **Finish v3 mesh catch-up** or export any v3 data you need (optional snapshot).
2. **Merge v4 code** and wait for CI to publish a GHCR image (`sha-<commit>`).
3. **Agree cutover window** — pause mining on canonical (193) immediately before wipe.
4. Do **not** run destructive resync on v3 until deliberate cutover.

## Mesh hosts

| Role | Host | Path |
|------|------|------|
| Canonical (mining) | `193.203.164.13` | `/opt/coinjecture` |
| Follower | `76.13.101.67` | `/opt/coinjecture-src` |
| Follower | `198.199.81.81` | `/opt/coinjecture` |

## Cutover procedure

### 1. Pin image and confirm env on each host

On every VPS `.env`:

```bash
COINJECT_NODE_IMAGE=ghcr.io/coinjecture-network/coinjecture2.0:sha-<commit>
DIFFICULTY=5   # optional; binary default is 5 for v4
```

Compose files pass `--chain-id coinject-network-b-v4` (no manual override needed if repo is updated).

### 2. Coordinated genesis boot (wipes all chain volumes)

From your laptop (SSH to root on all three hosts):

```bash
export COORDINATED_MESH_GENESIS_CONFIRM=I_UNDERSTAND_WIPE_CHAIN_DATA_ON_ALL_MESH_HOSTS
export COINJECT_NODE_IMAGE=ghcr.io/coinjecture-network/coinjecture2.0:sha-<commit>
# private GHCR: export GHCR_TOKEN=...
./scripts/deployment/coordinated-mesh-genesis-boot.sh
```

This script:

- Stops followers, then canonical
- `docker compose down -v` on all hosts (wipes `bootnode-data` volumes)
- Starts canonical first, waits for RPC
- Starts followers with sync-only overlays
- Waits for stable tip on canonical

**Single-host wipe only** (not for mesh cutover): `scripts/deployment/destructive-chain-resync-remote.sh`.

**Non-destructive redeploy** (same chain): `scripts/deployment/redeploy-remote-mesh-ghcr.sh` — do **not** use for v4 cutover.

### 3. Post-cutover verification

```bash
# Chain id and height
curl -s http://193.203.164.13:9933/ -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","method":"chain_getInfo","params":[],"id":1}' | jq .

# Block 2 coinbase reward ≈ 167 BEANS (167 * 10^12 atoms) with w=10, K=50
curl -s http://193.203.164.13:9933/ -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","method":"chain_getBlockByHeight","params":[2],"id":1}' | jq .

# All three hosts: same best_hash within first ~100 blocks
./scripts/deployment/mesh-fork-watch.sh
```

Expected:

- `chain_id`: `coinject-network-b-v4`
- `best_height` ≥ 2 with mining enabled on canonical after CPP peers connect
- Followers sync to canonical tip (no persistent height gap)

### 4. Re-enable mining on canonical

After followers are up and CPP peers are connected, canonical bootnode mines when `--mine` is set and sync gate allows. Use:

```bash
./scripts/deployment/verify-node-mining-enabled.sh
```

## Rollback

There is no in-place rollback to v3 chain state after volume wipe. Restore from v3 export/snapshot only if you took one before cutover; otherwise v3 history on wiped volumes is gone.

## Related docs

- Emission math: `docs/CONSENSUS_CALCULATIONS.md`, `docs/SECURITY_CALCULATIONS.md`
- Fork recovery (v3): `docs/FORKING_AND_REORG.md`
- Coordinated boot script: `scripts/deployment/coordinated-mesh-genesis-boot.sh`
