# Changelog

All notable changes to COINjecture Network B will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [4.6.4] - 2025-01-XX

### Added
- **Full Chain Reorganization (Fork Handling)**
  - Automatic detection of chain forks when nodes receive blocks at the same height with different hashes
  - Complete chain reorganization logic to ensure nodes always follow the longest valid chain
  - State unwinding: Automatically reverses all state changes (transfers, timelocks, escrows, channels, trustlines, swaps, marketplace transactions) when unwinding blocks from a shorter fork
  - State reapplication: Reapplies all transactions from the new longer chain in correct order
  - Common ancestor detection: Efficiently finds the fork point between current chain and competing chain
  - Automatic sync to longest chain: Nodes automatically request and switch to longer chains when detected
  - Fork detection triggers: Status updates and block receipts now trigger reorganization checks

### Changed
- Network event handling now detects forks and triggers full chain requests
- Block processing now includes reorganization checks after processing buffered blocks
- Chain state management now supports finding common ancestors and preparing reorganization paths

### Technical Details
- Added `find_common_ancestor()` to locate fork points between chains
- Added `prepare_reorganization()` to collect blocks for unwinding and reapplication
- Added `reorganize_chain()` to orchestrate the full reorganization process
- Added `unwind_block_transactions()` and `unwind_single_transaction()` for state reversal
- Added `attempt_reorganization_if_longer_chain()` to check for and trigger reorganizations
- Enhanced `NetworkEvent::StatusUpdate` handler to request full chains for fork analysis
- Enhanced `NetworkEvent::BlockReceived` handler to store fork blocks and check for reorganization

## [4.6.3] - 2025-11-20

### Added
- **Comprehensive Block Data Collection for Hugging Face**
  - Enhanced `collect_consensus_block_record` to capture ALL available block data:
    - Complete block header fields (version, height, prev_hash, timestamp, merkle roots, commitment)
    - All PoUW transparency metrics (solve_time_ms, verify_time_ms, time_asymmetry_ratio, solution_quality, complexity_weight, energy_estimate_joules)
    - Full transaction serialization for all transaction types (Transfer, TimeLock, Escrow, Channel, TrustLine, DimensionalPoolSwap, Marketplace)
    - Marketplace data extraction: automatically extracts problem and solution submissions from Marketplace transactions
    - Solution reveal data: complete problem and solution data from block's solution reveal
    - Coinbase transaction details (reward, recipient, height)
    - Calculated metrics: time asymmetry, energy asymmetry, solve/verify energy split, energy efficiency
  - All transaction details are now serialized and included in dataset records
  - Marketplace problems and solutions are extracted and structured separately for easier analysis

### Changed
- Consensus block records now include comprehensive data instead of minimal placeholders
- Energy metrics are calculated from PoUW transparency metrics when available
- Solution quality and work scores are now populated from block header data

## [4.6.2] - 2025-11-20

### Fixed
- **Hugging Face API Integration**
  - Updated to use new commit endpoint (`POST /api/datasets/{repo_id}/commit/main`) instead of deprecated `/upload` endpoint
  - Changed from multipart form data to JSON body with base64-encoded content
  - Fixed API base URL to `https://huggingface.co/api`
  - Changed logging from `println!` to `eprintln!` to ensure messages are captured in logs
- **P2P Network Mesh Formation**
  - Fixed Kademlia bootstrap by adding bootnode address to routing table before dialing
  - Updated gossipsub mesh configuration for small networks:
    - Set `mesh_outbound_min=1`, `mesh_n_low=2`, `mesh_n=2`, `mesh_n_high=4`
    - Satisfies gossipsub inequality: `mesh_outbound_min <= mesh_n_low <= mesh_n <= mesh_n_high`
  - Improved error handling in `broadcast_status` to properly handle gossipsub publish errors
  - Added detailed connection logging for debugging P2P issues

### Changed
- Hugging Face client now uses commit-based API for dataset uploads
- Enhanced network logging for connection establishment and mesh formation
- Improved error messages for gossipsub mesh formation issues

### Known Issues
- **Gossipsub Mesh Formation**: Mesh may not form immediately after connection establishment
  - TCP/libp2p connections are established successfully
  - Gossipsub mesh formation is asynchronous and occurs during heartbeats (1s intervals)
  - "InsufficientPeers" errors may persist until mesh fully forms
  - **Workaround**: Wait 30-60 seconds after node startup for mesh to stabilize
- **Node Stability**: Node2 may not start consistently
  - **Workaround**: Manually restart node2 if it fails to start
- **Hugging Face Uploads**: Data may not appear in dataset immediately
  - Uploads are buffered (10 records) before flushing
  - Requires successful mesh formation for blocks to be processed
  - **Workaround**: Ensure both nodes are connected and mesh is formed before expecting uploads

## [4.6.1] - 2025-11-19

### Fixed
- **Web Wallet cryptography**
  - Switched to the ESM-friendly `@noble/curves/ed25519.js` entry point and unified key generation via `ed25519.keygen()` to match the on-chain address format.
  - Explicitly disables `zip215` during signature verification so browser-side checks align with the Rust validator.
- **Privacy commitment utilities**
  - Passes the underlying `ArrayBuffer` into `crypto.subtle.digest` to prevent `DataCloneError`/Safari failures.
  - Adds defensive checks for optional `ProblemType` variants when estimating complexity to avoid undefined access.

## [4.6.0] - 2025-11-19

### Added
- Privacy-preserving marketplace with ZK commitment scheme
- Public and private problem submission modes (`SubmissionMode::Public` and `SubmissionMode::Private`)
- Marketplace export crate stub for future problem/solution repository
- Web wallet components for marketplace interactions

### Fixed
- **Critical Security Fix**: Epoch salt now correctly derived from parent block hash (`prev_hash`) instead of block height
  - Prevents pre-mining attacks where miners could compute problems before parent block exists
  - Ensures commit-mine-reveal protocol: `commitment = H(problem_params || parent_hash || H(solution))`
- Gossipsub mesh configuration optimized for small networks
  - Set `mesh_outbound_min=1`, `mesh_n_low=1`, `mesh_n=2`, `mesh_n_high=3`
  - Changed validation mode to `Permissive` for small network testing
  - Resolves "InsufficientPeers" errors in 2-node networks
- Fixed compilation error in `submit_problem` call to use `SubmissionMode::Public` wrapper
- Stale block prevention: Mining loop now checks if chain advanced before storing newly mined blocks

### Changed
- Updated network protocol to support small network deployments
- Enhanced block validation to use parent hash for epoch salt verification

### Technical Details
- **Epoch Salt Fix**: Changed from `Hash::new(&height.to_le_bytes())` to `prev_hash` in both `consensus/src/miner.rs` and `node/src/validator.rs`
- **Network Config**: Updated `network/src/protocol.rs` with optimized gossipsub parameters for 2-node networks
- **Marketplace**: Added support for both public (full problem visible) and private (commitment-only) problem submissions

## [4.5.0] - Previous Release

### Added
- Initial Network B implementation
- NP-hard problem solving (SubsetSum, SAT, TSP)
- Proof-of-Useful-Work (PoUW) consensus
- Dimensional tokenomics (η = 1/√2)
- P2P networking with libp2p gossipsub
- Redb database for blockchain and state storage

---

[4.6.4]: https://github.com/beanapologist/COINjecture-NetB-Updates/releases/tag/v4.6.4
[4.6.3]: https://github.com/beanapologist/COINjecture-NetB-Updates/releases/tag/v4.6.3
[4.6.2]: https://github.com/beanapologist/COINjecture-NetB-Updates/releases/tag/v4.6.2
[4.6.1]: https://github.com/beanapologist/COINjecture-NetB-Updates/releases/tag/v4.6.1
[4.6.0]: https://github.com/beanapologist/COINjecture-NetB-Updates/releases/tag/v4.6.0
[4.5.0]: https://github.com/beanapologist/COINjecture-NetB-Updates/releases/tag/v4.5.0

