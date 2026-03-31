# Troubleshooting Guide

Common issues and solutions for COINjecture 2.0 development and operation.

---

## Build Issues

### `error: rustup could not choose a version of cargo to run`

**Cause**: Wrong Rust version or missing toolchain.

```bash
# Fix: Update rustup and install the required toolchain
rustup update stable
rustup default stable
rustc --version  # should be 1.88+
```

---

### `error[E0658]: use of unstable library feature`

**Cause**: Minimum Rust version not met.

```bash
# Fix: Ensure you're on Rust 1.88+
rustup update
cargo build
```

---

### Build fails with linker errors on Windows

**Cause**: Missing C++ build tools.

```
LINK : fatal error LNK1181: cannot open input file 'kernel32.lib'
```

**Fix**: Install [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with the "Desktop development with C++" workload selected, then restart your terminal.

---

### `error: failed to run custom build command for ring`

**Cause**: Missing C compiler or outdated `ring` crate.

```bash
# On Windows: Install Visual Studio Build Tools (see above)
# On Linux:
sudo apt-get install build-essential
# On macOS:
xcode-select --install
```

---

## Node Startup Issues

### `Error: Address already in use (os error 98)`

**Cause**: Port 707 (CPP) or 9933 (RPC) already in use.

```bash
# Find and kill the process using the port
netstat -tlnp | grep 707        # Linux
netstat -ano | findstr :707     # Windows

# Or use a different port
./target/release/coinject --cpp-p2p-addr 0.0.0.0:708 --rpc-addr 127.0.0.1:9934
```

---

### `ADZDB path is not a directory`

**Cause**: Previous crash left a corrupted data directory.

```bash
# Remove and let the node recreate it
rm -rf ./node_data
./target/release/coinject --data-dir ./node_data
```

---

### `Error: Database already exists`

**Cause**: Trying to create a new database where one already exists.

```bash
# Option 1: Use the existing database (correct behavior)
# The node automatically opens existing databases — this error
# only appears if the code path explicitly calls Database::create()
# on an existing path.

# Option 2: Fresh start
rm -rf ./node_data && mkdir ./node_data
```

---

### Node starts but shows `height: 0` forever

**Cause**: Not connected to any peers, or no miner is running.

```bash
# Check peer count
curl -s http://localhost:9933 \
  -d '{"jsonrpc":"2.0","method":"net_peerCount","params":[],"id":1}'

# Start with explicit peer
./target/release/coinject --peer 127.0.0.1:707

# Or start mining yourself
./target/release/coinject --mine --miner-address <YOUR_ADDRESS>
```

---

## Networking Issues

### Nodes don't discover each other (Docker testnet)

**Cause**: Docker network not initialized or containers not healthy.

```bash
# Check container status
docker-compose ps

# Check bootnode health
docker-compose exec bootnode curl -s http://localhost:9090/health

# View network logs
docker-compose logs bootnode | grep -i "peer\|connect\|cpp"

# Restart the network
docker-compose down && docker-compose up -d --build
```

---

### `Connection refused` on RPC calls

**Cause**: Node RPC server not started or wrong port.

```bash
# Check if node is running
ps aux | grep coinject

# Check which port RPC is on (look for --rpc-addr in startup logs)
./target/release/coinject --rpc-addr 127.0.0.1:9933

# Test with curl
curl -s http://localhost:9933 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"chain_getHeight","params":[],"id":1}'
```

---

### CPP peers connect then immediately disconnect

**Cause**: Genesis hash mismatch between nodes.

All nodes on the same network must have the same genesis block. If you're running different builds with different genesis seeds, peers will reject each other after handshake.

```bash
# Check genesis hash
curl -s http://localhost:9933 \
  -d '{"jsonrpc":"2.0","method":"chain_getBlock","params":[0],"id":1}' \
  | python3 -m json.tool

# Both nodes must show identical genesis hash
```

---

## Consensus & Mining Issues

### Mining produces blocks but work score is very low

**Cause**: Problem type has low difficulty weight or solution quality is 0.

Check the problem registry:
```bash
curl -s http://localhost:9933 \
  -d '{"jsonrpc":"2.0","method":"consensus_getProblemRegistry","params":[],"id":1}'
```

Use larger problems for higher work scores:
- SubsetSum: increase `numbers` length (weight = log₂(n))
- SAT: increase `variables` and `clauses` (weight = vars × log₂(clauses))
- TSP: increase `cities` (weight = cities²)

---

### `Block validation failed: Invalid block height`

**Cause**: Submitting an old block or block from a different chain branch.

The node expects `new_height = current_height + 1`. If you're submitting via RPC, fetch the current height first:

```bash
HEIGHT=$(curl -s http://localhost:9933 \
  -d '{"jsonrpc":"2.0","method":"chain_getHeight","params":[],"id":1}' \
  | python3 -c "import sys,json; print(json.load(sys.stdin)['result'])")
echo "Current height: $HEIGHT"
```

---

### Difficulty keeps rising but no blocks are mined

**Cause**: Block time target vs actual solve time mismatch.

The difficulty adjuster targets `--block-time` seconds (default 60s). If problems are too hard:

```bash
# Use easier problems (fewer numbers in SubsetSum)
# Or lower the minimum work score requirement for marketplace problems
# Or increase block time
./target/release/coinject --mine --block-time 300  # 5 minute blocks
```

---

## State & Transaction Issues

### `Transaction fee too low` or `Transaction rejected`

**Cause**: Fee below the current fee market minimum.

```bash
# Check current minimum fee
curl -s http://localhost:9933 \
  -d '{"jsonrpc":"2.0","method":"mempool_getMinFee","params":[],"id":1}'
```

---

### `Insufficient balance`

**Cause**: Account balance is too low for amount + fee.

```bash
# Check balance
curl -s http://localhost:9933 \
  -d '{"jsonrpc":"2.0","method":"account_getBalance","params":["<ADDRESS>"],"id":1}'

# Request testnet funds
curl -X POST http://localhost:9933 \
  -d '{"jsonrpc":"2.0","method":"faucet_request","params":["<ADDRESS>"],"id":1}'
```

---

### `Nonce too low` or `Nonce already used`

**Cause**: Submitting a transaction with a stale nonce.

```bash
# Get current nonce
curl -s http://localhost:9933 \
  -d '{"jsonrpc":"2.0","method":"account_getNonce","params":["<ADDRESS>"],"id":1}'
```

Always use `current_nonce + 1` for the next transaction.

---

### Marketplace problem not appearing in `getOpenProblems`

**Cause**: The problem transaction hasn't been included in a block yet (still in mempool), or expiration has passed.

```bash
# Check mempool
curl -s http://localhost:9933 \
  -d '{"jsonrpc":"2.0","method":"mempool_getPendingCount","params":[],"id":1}'

# Check if a miner is running
curl -s http://localhost:9933 \
  -d '{"jsonrpc":"2.0","method":"mining_isMining","params":[],"id":1}'
```

---

## Test Failures

### Tests fail with `database already exists`

**Cause**: Parallel tests sharing the same temp directory.

This was fixed in Phase 8 — all tests use `tempfile::tempdir()`. If you're still seeing this:

```bash
# Clean all test artifacts and retry
cargo clean
cargo test --workspace
```

---

### Property tests fail intermittently

**Cause**: Proptest found a regression with a randomly generated input.

```bash
# Run with more iterations to reproduce consistently
PROPTEST_CASES=10000 cargo test -p coinject-consensus property

# If reproduced, the failing case is saved in proptest-regressions/
cat proptest-regressions/*.txt
```

---

### `thread 'tokio-runtime-worker' panicked at 'called \`Option::unwrap()\` on a \`None\` value'`

**Cause**: Race condition in async test setup. Ensure all async tests use `#[tokio::test]`:

```rust
#[tokio::test]
async fn my_test() {
    // ...
}
```

---

## Docker Issues

### `docker-compose up` fails with `port is already allocated`

**Cause**: Ports 9090-9093 or 707 already in use.

```bash
# Find what's using the port (Linux)
sudo lsof -i :9090

# Stop the conflicting process or change ports in docker-compose.yml
```

---

### Docker build fails at `cargo build --release`

**Cause**: Out of memory during compilation. Rust release builds are memory-intensive.

```bash
# Increase Docker memory limit to at least 4GB in Docker Desktop settings

# Or use a pre-built image
docker pull quigles1337/coinject:latest
```

---

### Node containers are healthy but show no peers

**Cause**: The bootnode takes ~5 seconds to start before peers try to connect.

```bash
# Wait 10 seconds after starting, then check
sleep 10
curl http://localhost:9091/health

# Should show peers > 0
```

---

## Web Wallet Issues

### `Failed to fetch` errors in web wallet

**Cause**: CORS or the local node isn't running.

```bash
# Start a local node with CORS enabled
./target/release/coinject \
  --rpc-addr 127.0.0.1:9933 \
  --rpc-cors-allow-all
```

---

### `Invalid private key` on wallet import

**Cause**: Key format mismatch. Keys must be 64 hex characters (32 bytes).

```bash
# Generate a valid key for testing
./target/release/coinject-wallet account new --name test
./target/release/coinject-wallet account export test --format hex
```

---

## Getting More Help

1. **Increase log verbosity**: `RUST_LOG=debug ./target/release/coinject`
2. **Check existing issues**: GitHub Issues page
3. **Run a single test with output**: `cargo test test_name -- --nocapture`
4. **Check compile errors in detail**: `cargo check 2>&1 | head -50`

For security-related issues, see [SECURITY.md](../SECURITY.md).
