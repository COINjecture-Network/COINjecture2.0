# Changelog

All notable changes to COINjecture will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [4.7.2] - 2025-11-25

### Fixed
- **Buffered Block Processing During Sync**
  - Fixed critical bug where `process_buffered_blocks` would stop processing all buffered blocks if `store_block` returned `is_new_best=false`.
  - During sequential sync, blocks are now applied even if `store_block` doesn't immediately update the best chain (due to race conditions or duplicate storage).
  - Added logic to check if a buffered block actually extends the current best chain before applying it, and manually update the best chain if needed.
  - **Critical Fix**: `process_buffered_blocks` is now called immediately after buffering a future block, ensuring buffered blocks are processed as soon as sequential blocks become available.
  - **Missing Block Detection**: When `process_buffered_blocks` doesn't find the next sequential block but has blocks ahead in the buffer, it now automatically requests the missing blocks from the network (e.g., if at height 15 and have block 2000 buffered, it requests blocks 16-115).
  - **Invalid Block Handling**: When a buffered block fails validation due to "Invalid previous hash" (e.g., block was buffered before the previous block was applied), it's now removed from the buffer and will be re-requested with the correct prev_hash.
  - This fixes the issue where Cloud Run and other nodes would get stuck during initial sync, receiving blocks but not applying them.
- **Initial Chain Sync**
  - Validator now skips timestamp age checks (2-hour limit) during initial sync to allow historical blocks.
  - Mining loop now waits for peer connections and chain sync before starting to mine, preventing forks from genesis.
  - New nodes will properly sync with the existing chain instead of creating separate chains.
- **Mining Loop Sync Wait**
  - Fixed issue where nodes would start mining from genesis before completing chain sync.
  - Mining loop now waits up to 5 minutes for chain sync to complete, tracking height stability.
  - Nodes at genesis wait at least 15 seconds for status updates before starting to mine.
  - Height must be stable for 6 seconds (3 checks) before mining begins, ensuring sync completion.
- **P2P Network Improvements**
  - Added gossipsub mesh peer tracking to prevent "InsufficientPeers" errors.
  - Broadcast functions now check mesh status before attempting to publish messages.
  - Improved error handling: "InsufficientPeers" errors are silently ignored (expected when no peers connected).
  - Mining loop now skips mining cycles when chain advances from peer blocks, preventing stale mining.
  - Better mesh peer tracking via gossipsub Subscribed/Unsubscribed events.

### Changed
- **Block Validation**
  - Added `validate_block_with_options()` to allow skipping timestamp age checks during sync.
  - Timestamp age checks are automatically skipped for blocks older than 2 hours (indicating sync scenario).
  - Future timestamp checks are still enforced to prevent invalid blocks.
- **Cloud Run P2P Networking**
  - Updated Cloud Run deployment to use `min-instances=1` to maintain P2P connections (prevents scaling to zero).
  - Added enhanced connection error logging for debugging P2P connectivity issues.
  - Enabled libp2p debug logging in Cloud Run environment variables for better diagnostics.
  - Improved bootnode connection error handling with detailed error messages.

## [4.7.1] - 2025-11-24

### Added
- **Adaptive Difficulty Telemetry**
  - Mining loop now logs per-block difficulty stats (avg / σ / ratio / stall counts) using `DifficultyStats`.
  - Operators get explicit warnings when solve time ratios exceed 2× and guidance to recruit miners.

### Changed
- **Dynamic Difficulty Engine**
  - Introduced stall detection thresholds, high-variance guarding, and recovery mode to prevent two-node stalls.
  - Penalizes failures immediately, caps per-problem-type sizes adaptively, and scales more aggressively when the network lags.
- **Mining Loop Reliability**
  - Mining attempts now retry up to five times with 60s timeout, automatically shrinking problem size between attempts.
  - Ensures `record_solve_time` and `adjust_difficulty` run for every successful block while timeouts feed the penalty path.

### Fixed
- Eliminated conditions where SAT problems remained permanently unsolved by auto-reducing problem size on repeated failures.
- Prevented difficulty oscillations by deferring adjustments when solve-time variance is high.

## [4.0.0] - 2025-11-23

### Added
- **Complete Rust Rewrite**
  - Full rewrite from Python to Rust for production-grade performance
  - Modular architecture with workspace-based crate organization
  - ACID-compliant redb database for state persistence
  - libp2p networking with GossipSub, Kademlia, and mDNS
  - JSON-RPC server with HTTP/WebSocket support
  - Complete CLI wallet with Ed25519 keystore
  - Web frontend with React-based explorer and wallet
  - HuggingFace integration for automatic dataset uploads

- **Proof of Useful Work (PoUW)**
  - NP-complete problem solving (SubsetSum, SAT, TSP)
  - Polynomial-time solution verification
  - Work score calculation with quality metrics
  - Adaptive difficulty adjustment
  - Commit-reveal protocol to prevent grinding attacks

- **Autonomous Marketplace**
  - On-chain problem submission with bounty escrow
  - Automatic solution verification and payout
  - Marketplace state persistence in redb
  - RPC endpoints for problem queries and submission
  - Support for public and private problem submissions

- **Dimensional Tokenomics**
  - Multi-tier liquidity pools (D₁, D₂, D₃)
  - Exponential allocation ratios based on Satoshi constant
  - Unit circle constraint: |μ|² = η² + λ² = 1
  - Critical damping: η = λ = 1/√2

- **Advanced Transaction Types**
  - Transfer transactions
  - Dimensional pool swaps
  - Time-locked balances
  - Multi-party escrow
  - Payment channels
  - TrustLine protocol (XRPL-inspired)

- **Infrastructure**
  - Full chain reorganization (fork handling)
  - State unwinding and reapplication
  - Common ancestor detection
  - Automatic sync to longest chain
  - Prometheus metrics integration
  - Docker deployment support

### Changed
- Complete rewrite from Python to Rust
- Database migration from Sled to redb for ACID compliance
- Network protocol upgraded to libp2p
- RPC interface standardized to JSON-RPC 2.0
- Architecture refactored to modular workspace structure

### Technical Details
- **Language**: Rust 1.70+
- **Database**: redb 2.1 (ACID-compliant)
- **Networking**: libp2p 0.54 (GossipSub, Kademlia, mDNS)
- **RPC**: jsonrpsee 0.24 (HTTP/WebSocket)
- **Cryptography**: Ed25519-dalek 2.1, Blake3, SHA2/SHA3
- **Build**: Cargo workspace with 11 crates

---

## [4.7.0] - 2025-11-23

### Added
- **Enhanced Web Frontend for CloudFront Deployment**
  - Complete RPC client integration with all 25+ blockchain endpoints
  - Real-time marketplace data fetching and display
  - Live chain information and network metrics
  - Production-ready build configuration optimized for AWS S3/CloudFront
  - Automated deployment scripts with CloudFront cache invalidation
  - Comprehensive deployment documentation (DEPLOYMENT.md, CLOUDFRONT-SETUP.md)
  - Environment variable support for RPC and metrics endpoints
  - TypeScript types matching Rust structs for type safety

### Enhanced
- **Frontend Components**
  - MarketplaceSection: Real-time problem listings with auto-refresh
  - MetricsSection: Live chain info and marketplace statistics
  - RPC Client: Full integration with all endpoints (account, chain, transaction, marketplace, timelock, escrow, channel, faucet)
  - Build optimization: Code splitting, minification, CloudFront-ready static assets

### Changed
- Frontend now uses actual RPC API endpoints instead of mock data
- Data structures updated to match Rust implementation exactly
- Marketplace stats now show `total_bounty_pool`, `expired_problems`, `cancelled_problems`
- Problem info uses `submitted_at` and `expires_at` timestamps (i64)
- Chain info includes `chain_id`, `best_hash`, `genesis_hash`, `peer_count`

### Technical Details
- **RPC Client**: `web/coinjecture-evolved-main/src/lib/rpc-client.ts` - Complete API client with TypeScript types
- **Deployment**: Automated `deploy.sh` script for S3/CloudFront deployment
- **Documentation**: API-INTEGRATION.md, DEPLOYMENT.md, CLOUDFRONT-SETUP.md, README-DEPLOYMENT.md
- **Build Config**: Vite optimized for production with code splitting and minification
- **React Query**: Auto-refresh every 10-30 seconds for live data

## [4.6.5] - 2025-11-23

### Added
- **Unified HuggingFace Dataset**
  - Consolidated all problem types (SubsetSum, SAT, TSP, Custom) into single continuous dataset: `COINjecture/NP_Solutions`
  - Unified buffer system that flushes all problem types together when threshold (10 records) is reached
  - Enhanced dataset schema with data provenance fields (`metrics_source`, `measurement_confidence`, `data_version`)
  - Updated HuggingFace README with comprehensive documentation and organized data fields table
- **Docker Deployment Improvements**
  - Multi-stage Dockerfile for optimized `linux/amd64` builds
  - Automated deployment script with bootnode configuration support
  - Container health checks and automatic restart policies
  - Deployment verification scripts

### Fixed
- **Schema Serialization**
  - Fixed u128 bounty serialization: Now serialized as string to avoid JSON precision loss (JSON integers only safe up to 2^53)
  - Added custom serialization functions: `serialize_u128_as_string` and `deserialize_u128_from_string`
- **P2P Network Connectivity**
  - Added bootnode configuration to deployment script
  - Node 2 now automatically connects to Node 1 as bootnode: `/ip4/143.110.139.166/tcp/30333`
  - Fixed peer discovery and network synchronization between nodes
- **HuggingFace Integration**
  - Fixed unified dataset buffer logic to count total records across all problem types
  - Improved logging for unified mode operations
  - Enhanced error handling for dataset uploads

### Changed
- HuggingFace client now uses unified dataset approach instead of type-specific datasets
- Buffer flush logic: Flushes when total records across all types >= 10 (instead of per-type buffers)
- Deployment script now configures bootnodes automatically for multi-node setups
- Updated timing precision: Changed from milliseconds to microseconds (`solve_time_us`, `verify_time_us`)

### Technical Details
- **Unified Dataset**: Modified `huggingface/src/client.rs` to combine all problem types into single dataset
- **Schema Fix**: Added custom serde serialization for u128 fields in `DatasetRecord`
- **Bootnode Config**: Updated `deploy-docker.sh` to accept and pass bootnode addresses to containers
- **README**: Updated HuggingFace README with unified dataset documentation and reorganized schema table

## [4.6.4] - 2025-11-22

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

