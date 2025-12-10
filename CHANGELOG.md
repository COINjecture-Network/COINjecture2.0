# Changelog

All notable changes to COINjecture will be documented in this file.

## [4.7.47] - 2025-12-09

### Added
- **Complete Fork Recovery from Genesis**: Automatic recovery from complete chain forks
  - **Problem**: When nodes detected a complete fork (no common ancestor), they would fail to reorganize
  - **Solution**: Implemented automatic recovery that:
    1. Detects complete forks (no common ancestor found)
    2. Requests full chain from genesis (height 0)
    3. Validates the new chain from genesis with full integrity checks
    4. Compares chains by work score and height
    5. Performs complete reorganization from genesis if new chain is better
  - **Features**:
    - `validate_chain_from_genesis()`: Validates entire chain from genesis with work score calculation
    - `get_chain_from_genesis()`: Retrieves current chain from genesis for comparison
    - `reorganize_chain_from_genesis()`: Performs complete reorganization unwinding to genesis
  - **Impact**: Nodes can now automatically recover from complete forks by connecting to the chain with highest work score/height
  - **Files Changed**: `node/src/service.rs` - `attempt_reorganization_if_longer_chain()`, `check_and_reorganize_chain()`

## [4.7.46] - 2025-12-05

### Fixed
- **Database Corruption Detection (Critical)**: Added validation to detect and auto-fix impossibly high block heights
  - **Problem**: Corrupted bytes in the database were being interpreted as valid u64 values (e.g., 358,132,200 blocks)
  - **Root Cause**: No validation existed for block heights read from database
  - **Solution**: Added `MAX_REASONABLE_HEIGHT` constant (10,000,000 blocks) with auto-fix
    - On startup, if stored height exceeds MAX_REASONABLE_HEIGHT, reset to genesis
    - When storing blocks, reject any with impossibly high heights
    - Auto-updates database with corrected values
  - **Files Changed**: `node/src/chain.rs`

## [4.7.45] - 2025-12-05

### Fixed
- **Reorganization with Buffered Blocks**: Improved fork detection for buffered blocks
  - **Problem**: Reorganization wasn't finding the common ancestor properly for buffered blocks from earlier forks
  - **Solution**: Use `find_common_ancestor()` for buffered blocks to handle earlier forks
  - **Impact**: Better detection of fork chains and reorganization candidates
  - **Files Changed**: `node/src/service.rs` - `check_and_reorganize_chain()`

## [4.7.44] - 2025-12-05

### Fixed
- **Mining Race Condition**: Fixed issue where only ONE node could mine successfully
  - **Problem**: There was a `continue` statement that would skip mining entirely when a node received a block from a peer
  - **Root Cause**: This created a feedback loop where the first node to mine would always win - other nodes would receive the block and skip their mining cycle
  - **Solution**: Changed from `continue` (skip mining) to just updating `last_mined_height` and letting all nodes continue to the consensus check
  - **Impact**: All nodes now have fair opportunity to mine, governed by proper peer consensus
  - **Files Changed**: `node/src/service.rs` - mining loop

## [4.7.43] - 2025-01-XX

### Fixed
- **Reorganization Check Only Checking Highest Block**: Fixed reorganization check to examine ALL buffered blocks, not just the highest one
  - Previously only checked the highest buffered block, which might be from a different fork
  - Now checks top 100 buffered blocks to find ANY that connect to the current best chain
  - Builds a set of hashes on the current best chain (walking back up to 1000 blocks)
  - For each buffered block, verifies its previous hash is on the current chain before considering it
  - Walks forward from connection points to find the full chain extent
  - Uses the longest valid chain found for reorganization

### Technical Details
- Modified `check_and_reorganize_chain()` in `node/src/service.rs` to:
  - Build a HashSet of hashes on the current best chain (walking back from best block)
  - Iterate through top 100 buffered blocks (sorted by height descending)
  - Check if each block's previous hash is in the current chain HashSet
  - Walk forward from connection points to find chain extent
  - Select the longest valid chain found

## [4.7.42] - 2025-01-XX

### Fixed
- **Reorganization Check Stopping at Missing Blocks**: Fixed reorganization check to continue scanning past missing blocks
  - Previously stopped at first missing block (e.g., height 1114) and never checked higher heights
  - Now checks buffer for blocks that might connect to current chain
  - Walks back from buffered blocks to find chain connections
  - Continues sequential scan past gaps to find any stored blocks at higher heights
  - Allows reorganization to find stored blocks even when there are gaps in the sequence

### Technical Details
- Modified `check_and_reorganize_chain()` in `node/src/service.rs` to:
  - First check buffer for blocks that might connect to current chain
  - Walk back from highest buffered block to see if it connects
  - Continue sequential scan past missing blocks (don't stop at first gap)
  - Scan up to 1000 blocks ahead to find any stored blocks

## [4.7.41] - 2025-01-XX

### Added
- **Periodic Reorganization Check**: Added 60-second periodic task to proactively check for reorganization opportunities
  - Runs every minute regardless of block processing
  - Ensures reorganization is checked even when blocks fail validation
  - Spawned alongside other periodic tasks (status broadcast, metrics)

- **Aggressive Missing Block Requests**: Reorganization check now requests missing blocks when gaps are detected
  - When stored blocks are found ahead but with gaps, requests full range immediately
  - Enables reorganization to proceed once gaps are filled
  - Uses network command sender passed to reorganization check

### Changed
- **Increased Reorganization Scan Limit**: Increased from 500 to 1000 blocks to handle very large forks
- **Reorganization Check Signature**: Now accepts optional `network_cmd_tx` parameter to enable block requests
- **Frontend RPC Client**: Updated to use HTTPS domains directly instead of CloudFront proxy
  - Maps IP addresses to HTTPS domains: `143.110.139.166` → `https://rpc1.coinjecture.com`
  - Removed CloudFront `/api/rpc` proxy dependency
  - Validates all URLs are HTTPS in production

### Fixed
- **Reorganization Not Triggering**: Fixed by adding periodic checks and aggressive missing block requests
- **Frontend 502 Bad Gateway**: Fixed by updating RPC client to use HTTPS domains directly

### Technical Details
- Modified `node/src/service.rs` to:
  - Add periodic reorganization check task (60-second interval)
  - Pass `network_cmd_tx` to `check_and_reorganize_chain()` function
  - Request missing blocks when gaps detected in stored blocks
  - Increase scan limit from 500 to 1000 blocks
- Modified `web/coinjecture-evolved-main/src/lib/rpc-client.ts` to:
  - Use HTTPS domains directly in production
  - Map IP addresses to HTTPS domains automatically
  - Remove CloudFront proxy fallback

## [4.7.40] - 2025-01-XX

### Changed
- **Increased Sync Threshold**: Increased from 100 to 500 blocks to allow storing blocks from forks for reorganization
- **Increased Reorganization Scan Limit**: Increased from 200 to 500 blocks to handle larger forks
- **Added Reorganization Check Logging**: Added logging when reorganization check is triggered after processing blocks

### Fixed
- **Reorganization Check Trigger**: Added reorganization check trigger after storing fork blocks to ensure reorganization can find stored blocks even if they're not connected yet

### Known Issues
- **Chain Reorganization Not Triggering**: Despite implementing work score-based reorganization and common ancestor anchoring, reorganization is not triggering when nodes are on different forks. Current diagnosis:
  - Blocks are being received and stored from forks
  - Reorganization check is not being called frequently enough
  - Blocks failing validation ("Invalid previous hash") prevent sequential processing
  - Gap between nodes continues to widen (currently 500+ blocks)
  - Need to investigate: periodic reorganization checks, better fork block storage, or alternative reorganization trigger mechanisms

- **Frontend 502 Bad Gateway Errors**: CloudFront is returning 502 errors when submitting blocks via RPC:
  - Error: `POST https://d1f2zzpbyxllz7.cloudfront.net/api/rpc 502 (Bad Gateway)`
  - This suggests CloudFront proxy is still being used despite HTTPS domain setup
  - Frontend may need cache invalidation or RPC client update to use direct HTTPS domains
  - Lambda@Edge was removed but CloudFront distribution may still be configured incorrectly

### Technical Details
- Modified `node/src/service.rs` to:
  - Increase sync threshold from 100 to 500 blocks
  - Increase reorganization scan limit from 200 to 500 blocks
  - Add reorganization check trigger after storing fork blocks
  - Add logging for reorganization check triggers

## [4.7.39] - 2025-01-XX

### Added
- **Work Score-Based Chain Reorganization**
  - Chain reorganization now compares chains by cumulative work score, not just length
  - Prevents reorganization to longer chains with less total work
  - Uses `WorkScoreCalculator::compare_chains()` with 0.5% tolerance
  - Logs work score comparison for debugging: old chain work vs new chain work

- **Common Ancestor Anchoring Logic**
  - Reorganization requires common ancestor to be at least 6 blocks deep
  - Validates common ancestor block exists and is on current chain
  - Prevents shallow reorganizations that could destabilize the network
  - Verifies common ancestor block is stored and valid before proceeding

### Changed
- `attempt_reorganization_if_longer_chain()` now compares work scores before reorganizing
- Reorganization check no longer ignores errors (proper error handling)
- Work score calculation converts `f64` to `u64` for comparison

### Fixed
- Compilation errors: Fixed work score type mismatch (`f64` vs `u64`)
- Reorganization check return type handling

### Technical Details
- Modified `node/src/service.rs::attempt_reorganization_if_longer_chain()` to:
  - Calculate cumulative work scores for both old and new chains
  - Use `WorkScoreCalculator::compare_chains()` to determine which chain has more work
  - Only reorganize if new chain has significantly more work (>0.5% tolerance)
  - Validate common ancestor is anchored (min 6 blocks deep) and exists in storage

## [4.7.37] - 2025-12-03

### Fixed
- **Reorganization Debug Logging**: Added detailed logging to diagnose why reorganization isn't triggering
  - Added logging when chain is broken during reorganization check
  - Added logging when previous blocks are missing
  - Added logging when no blocks are found at expected heights
  - This will help identify why `check_and_reorganize_chain` isn't finding longer chains

### Changed
- **Frontend RPC Client**: Updated to use HTTPS domains directly instead of CloudFront proxy
  - Maps HTTP IP addresses to HTTPS domains:
    - `143.110.139.166` → `https://rpc1.coinjecture.com`
    - `68.183.205.12` → `https://rpc2.coinjecture.com`
    - `35.184.253.150` → `https://rpc3.coinjecture.com`
  - Removed CloudFront proxy fallback (Lambda@Edge was removed)
  - Direct HTTPS access with CORS (already enabled on RPC servers)
  - Fixes 405 Method Not Allowed errors from CloudFront

### Technical Details
- Modified `node/src/service.rs` to add debug logging in `check_and_reorganize_chain`
- Modified `web/coinjecture-evolved-main/src/lib/rpc-client.ts` to use HTTPS domains

## [4.7.36] - 2025-12-03

### Fixed
- **Enhanced Fork Detection with Multiple Indicators**: Improved fork detection to trigger even when peer's best block isn't stored yet
  - **Problem**: Previous logic only checked if we had peer's best block stored, returning false if not available
  - **Solution**: Added multiple fork indicators:
    1. If we have peer's best block, verify if it connects to our chain (existing logic)
    2. If we're missing sequential blocks AND have blocks buffered ahead (>10 blocks), likely a fork
    3. If peer is significantly ahead (>50 blocks), more likely a fork
  - **Impact**: Fork detection now triggers when missing blocks prevent processing, even without peer's best block
  - **Missing Blocks**: When fork indicators are detected, requests full chain from genesis, ensuring no missing blocks

### Technical Details
- Modified `NetworkEvent::StatusUpdate` handler to check multiple fork indicators
- Checks buffer for missing sequential blocks and blocks buffered ahead
- Triggers fork detection when: missing_next && has_blocks_ahead && peer_significantly_ahead
- Requests full chain (0 to peer's best height) when fork is detected

## [4.7.35] - 2025-12-03

### Fixed
- **Improved Fork Detection When Peer is Ahead**: Enhanced fork detection logic to properly identify forks when a peer is ahead
  - **Problem**: Previous logic only checked if we had a different block at our height, but didn't verify if the peer's chain diverges from ours
  - **Solution**: Now walks back from peer's best block to check if it connects to our current chain
  - **Impact**: Fork detection now correctly triggers when peer is ahead on a different fork, ensuring full chain is requested for reorganization
  - **Missing Blocks**: When fork is detected, requests full chain from genesis, ensuring no blocks are missing for reorganization

### Technical Details
- Modified `NetworkEvent::StatusUpdate` handler to walk back from peer's best block
- Checks if peer's chain connects to our current chain by traversing backwards
- If peer's chain diverges, requests full chain from genesis to peer's best height
- This ensures all blocks are available for reorganization, preventing missing block issues

## [4.7.34] - 2025-12-03

### Fixed
- **Chain Reorganization Triggering**: Fixed critical issues preventing chain reorganization from triggering
  - **Fork Detection When Peer is Ahead**: Now detects forks when a peer is ahead and requests full chain for reorganization
  - **Reorganization After Receiving Blocks**: Added comprehensive reorganization check after processing blocks
  - **Enhanced check_and_reorganize_chain**: Now checks both buffer AND stored blocks for longer chains
  - **Stored Block Scanning**: Scans stored blocks up to 200 blocks ahead to find longest valid chain
  - Reorganization now triggers automatically when:
    - Peer is ahead and we're on a fork (requests full chain)
    - We receive blocks from a longer chain (checks after processing)
    - We have stored blocks that form a longer chain (scans stored blocks)

### Technical Details
- Modified `NetworkEvent::StatusUpdate` handler to detect forks when peer is ahead
- Enhanced `check_and_reorganize_chain()` to scan stored blocks, not just buffer
- Added reorganization check after `process_buffered_blocks()` completes
- Improved fork detection logic to request full chain when fork is detected

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [4.7.33] - 2025-12-03

### Added
- **CORS Support for RPC Server**: Added CORS headers to enable direct browser access to RPC endpoints
  - **Implementation**: Added `tower-http` CORS middleware to JSON-RPC server
  - **Configuration**: Allows all origins, methods, and headers (permissive for development/testnet)
  - **Impact**: Enables direct RPC access from HTTPS frontends without Lambda@Edge proxy
  - **Files Changed**:
    - `rpc/Cargo.toml`: Added `tower` and `tower-http` dependencies
    - `rpc/src/server.rs`: Added CORS middleware to server builder
  - **Next Steps**: Set up HTTPS/TLS for RPC endpoints to enable full direct access from HTTPS frontends

- **Block Submission Handler**: Enabled RPC block submission for web client mining
  - **Implementation**: Created `block_submission_handler` that validates, stores, and broadcasts blocks
  - **Flow**: RPC request → validation → storage → transaction application → network broadcast
  - **Impact**: Web CLI can now submit mined blocks directly via `chain_submitBlock` RPC method
  - **Files Changed**: `node/src/service.rs`: Added block submission handler with async task execution

### Fixed
- **Lambda@Edge Timeout**: Reduced timeout from 10s to 4.5s to comply with Lambda@Edge viewer-request 5s limit
- **Lambda@Edge Body Truncation**: Added detection and proper error handling for requests >1MB
- **Lambda@Edge POST Validation**: Added explicit POST method validation with proper error messages

## [4.7.32] - 2025-12-02

### Fixed
- **Sequential Block Request Chunks**: Reduced block request chunk size to improve sync reliability
  - **Problem**: Blocks were requested in large chunks (100 blocks), causing out-of-order delivery via gossipsub
  - **Solution**: Reduced chunk size to 20 blocks during initial sync (height < 1000), 50 blocks after
  - **Impact**: Smaller chunks reduce out-of-order delivery and "Invalid previous hash" errors
  - **Missing Block Requests**: Reduced missing block request range from 99 to 19 blocks ahead
  - **Single-Block Requests**: Missing sequential blocks are now requested one at a time for precise delivery

- **Block Serving Continuity**: Improved block serving to continue even when some blocks are missing
  - **Problem**: Block serving would stop at first missing block, preventing other blocks from being served
  - **Solution**: Changed block serving logic to continue serving available blocks even if some are missing
  - **Impact**: Nodes can receive blocks that exist even if some blocks in the range are missing

## [4.7.31] - 2025-12-02

### Fixed
- **Sync Block Threshold Bypass**: Fixed critical issue where explicitly requested sync blocks were being ignored
  - **Problem**: Sync blocks (explicitly requested via `SyncBlock` message) were being filtered by sync threshold check
  - **Root Cause**: `SyncBlock` messages were converted to `BlockReceived` events without distinguishing them from broadcast blocks
  - **Solution**: Added `is_sync_block: bool` flag to `NetworkEvent::BlockReceived` to distinguish sync blocks
    - Sync blocks now bypass the 100-block sync threshold check
    - Allows nodes to receive requested blocks even if they're far ahead during catch-up sync
  - **Impact**: Nodes can now catch up from large height differences by requesting and receiving blocks sequentially
  - **Files Changed**: 
    - `network/src/protocol.rs`: Added `is_sync_block` flag to `BlockReceived` event
    - `node/src/service.rs`: Modified sync threshold check to skip for sync blocks

## [4.7.30] - 2025-12-02

### Fixed
- **Peer Count Tracking Bug**: Fixed critical bug where mining was paused due to incorrect peer count
  - **Problem**: Mining loop was reading from a shadowed `peer_count` variable that was never updated
  - **Root Cause**: Duplicate `Arc<RwLock<u32>>` creation in `node/src/service.rs` at line 307
  - **Solution**: Removed duplicate `peer_count` creation, ensuring mining loop uses the same `Arc<RwLock>` as network service
  - **Impact**: Mining now correctly detects peer count and resumes when sufficient peers are connected
  - **Deployment**: Fix deployed to all nodes (v4.7.30)

## [4.7.29] - 2025-12-02

### Fixed
- **Block Request/Response Mechanism**: Fixed critical sync issue where nodes weren't receiving requested blocks during initial sync
  - **Root Cause**: Block requests were broadcast via gossipsub, and responses were also broadcast, causing unreliable delivery
  - **Solution**: Implemented direct peer-to-peer block sending for sync responses
    - Added `NetworkCommand::SendBlockToPeer` for direct peer communication
    - Added `NetworkService::send_block_to_peer()` method that ensures target peer is in gossipsub mesh before sending
    - Modified `BlocksRequested` handler to use `SendBlockToPeer` instead of `BroadcastBlock` for sync responses
    - Blocks are now sent directly to requesting peers, ensuring reliable delivery during sync
  - **Impact**: Nodes can now reliably sync by requesting specific block ranges and receiving them directly from peers
  - **Deployment**: Fix deployed to GCE VM and verified working (nodes receiving requested blocks 466-565)

- **Sync Performance**: Added sync threshold filter to prevent buffer buildup during initial sync
  - Blocks more than 100 blocks ahead of expected height are now ignored during sync (when node height < 1000)
  - Prevents buffer from filling with invalid blocks that can't be validated yet
  - Reduces "Invalid previous hash" validation errors during sync

- **Peer Count Bug**: Fixed double-counting issue where peer count was incremented/decremented in both network and service layers
  - Removed redundant peer count updates from service layer
  - Peer count now only updated by network layer (`network/src/protocol.rs`)

### Changed
- **Network Protocol**: Enhanced block request/response for better sync reliability
  - Re-exported `PeerId` from network crate for use in node crate
  - Improved block serving logic to ensure requesting peers receive blocks

### Resolved
- **Chain Fork Issue**: Resolved by clearing all node data and restarting from genesis
  - All nodes (Droplet 1, Droplet 2, GCE VM) reset to height 0
  - Fresh chain started from genesis block
  - Hugging Face dataset (NP_Solutions_v3) will be populated as new blocks are mined
  - No more chain gaps or fork issues - clean slate

## [4.7.16] - 2025-11-29

### Added
- **INSTITUTIONAL-GRADE METRICS v3.0**: Comprehensive HuggingFace data collection for academic research
  - **Block Identity Metrics**: `block_hash`, `prev_block_hash` for chain linkage tracing
  - **Timing Metrics**: `solve_time_us`, `verify_time_us` (microsecond precision), `block_time_seconds`, `mining_attempts`
  - **Memory Metrics**: `solve_memory_bytes`, `verify_memory_bytes`, `peak_memory_bytes`
  - **Network Metrics**: `peer_count`, `propagation_time_ms`, `sync_lag_blocks`
  - **Mining Metrics**: `difficulty_target`, `nonce`, `hash_rate_estimate`
  - **Chain Metrics**: `chain_work`, `transaction_count`, `block_size_bytes`
  - **Economic Metrics**: `block_reward`, `total_fees`, `pool_distributions` (per-pool token allocation)
  - **Hardware Metrics**: `cpu_model`, `cpu_cores`, `cpu_threads`, `ram_total_bytes`, `os_info`
  - **Provenance Metrics**: `node_version`, `node_id` (PeerId), `data_version: "v3.0"`
  
- **NetworkContext**: New struct for passing network state to metrics collection
- **HardwareContext**: Automatic hardware detection on node startup
- **push_consensus_block_with_context()**: Enhanced API for full context metrics

### Changed
- DatasetRecord upgraded from v2.0 to v3.0 with 30+ new fields
- All metrics now include hardware provenance (CPU model, cores, RAM)
- Data provenance tracking now includes node identity and software version

### Nuclear Reset
- **Complete chain reset performed** due to unfixable chain corruption (blocks 17-18 hash mismatch)
- Both DigitalOcean droplets reset to fresh genesis
- New canonical chain started from block 0
- All nodes now mining on unified fresh chain
- HuggingFace streaming verified operational

## [4.7.9] - 2025-11-28

### Fixed
- **Network Fork Unification**: Resolved critical issue where Node 1 and Node 2 were mining on separate forks
  - Identified root cause: Node 1 running old code without `SyncBlock` message type
  - Built and deployed v4.7.8 Docker image with `SyncBlock` fix to Node 1 (143.110.139.166)
  - Copied chain data from Node 2 to Node 1 to unify the blockchain
  - All nodes now share same genesis hash (`4a80254b...`)

### Infrastructure
- **Network Unification Deployment**
  - Cloned v4.7.8 from `github.com/beanapologist/COINjecture-NetB-Updates`
  - Compiled new Docker image with historical block sync fix
  - Deployed to DigitalOcean Node 1
  - Synchronized chain state across all nodes
  - Verified 5 nodes with 4+ peers each on unified network

### Status
- **Before**: 2 separate fork networks, Cloud Run stuck at block 18
- **After**: 1 unified network, all nodes syncing properly
- Both DigitalOcean nodes mining and streaming to HuggingFace
- Cloud Run nodes syncing via `SyncBlock` protocol

## [4.7.8] - 2025-11-27

### Fixed
- **Lambda@Edge RPC Proxy 503 Errors**: Fixed 503 Service Unavailable errors by adding comprehensive error handling
  - Added try-catch blocks around entire handler function
  - Added error handling for promise rejections
  - All errors now return proper JSON-RPC 2.0 error responses instead of causing 503s
  - Deployed as Lambda version 4 with enhanced error handling
  - CloudFront distribution updated to use version 4

- **Lambda@Edge RPC Proxy 502 Errors**: Improved error handling and logging for CloudFront RPC proxy
  - Added comprehensive error logging with request IDs for debugging
  - Increased timeout from 5s to 10s to handle slower RPC responses
  - Added proper OPTIONS preflight handling
  - Improved error messages with detailed error codes and target information
  - Added response stream error handling
  - Better body encoding handling (base64 and utf8)
  - All errors now return proper JSON-RPC 2.0 error format with diagnostic data

- **Header Hashing Mismatch**: Fixed JSON field ordering issue causing browser-mined blocks to show `leading_zeros=0`
  - **Root cause**: `serializeBlockForRpc()` used spread operator `...block.header` followed by field overrides, causing JavaScript to reorder fields
  - **Impact**: Client calculated hash with correct field order, but RPC submission reordered fields, breaking server-side hash validation
  - **Fix**: Explicitly construct header object with all fields in exact Rust struct order (matching `BlockHeader` in `core/src/block.rs`)
  - Field order now matches Rust: `version, height, prev_hash, timestamp, transactions_root, solutions_root, commitment, work_score, miner, nonce, solve_time_us, verify_time_us, time_asymmetry_ratio, solution_quality, complexity_weight, energy_estimate_joules`
  - Enhanced logging to diagnose client vs. server JSON serialization differences
    - Added detailed logging of exact JSON bytes on server side (first 200 bytes)
    - Added client-side logging of JSON bytes and object structure when debug mode enabled
    - Server now logs both JSON string and byte array representation for comparison
- **Historical Block Sync**: Fixed Cloud Run and other nodes getting stuck during initial sync due to gossipsub message deduplication
  - Added `SyncBlock` message type with unique `request_id` to bypass gossipsub deduplication for historical blocks
  - Implemented `send_sync_block()` method in `NetworkService` to send blocks with unique identifiers
  - Updated `BlocksRequested` handler to use `SendSyncBlock` command instead of `BroadcastBlock` for sync responses
  - Each sync block now includes a unique request_id (timestamp + height) to ensure gossipsub treats them as distinct messages
  - This allows nodes to receive historical blocks even if they've seen them before (e.g., during previous sync attempts)

### Changed
- **Cloud Run Deployment**: Updated deployment script to use a fresh data directory per deploy
  - Each Cloud Run deployment now uses `/tmp/data-{timestamp}` to ensure clean state
  - This prevents sync issues from stale buffered blocks and ensures nodes start from genesis when needed
  - Added logging to indicate clean state initialization

### Technical Details
- Modified `NetworkMessage` enum to include `SyncBlock { block: Block, request_id: u64 }`
- Added `NetworkCommand::SendSyncBlock { block, request_id }` for internal command routing
- Updated `handle_gossipsub_message()` to process `SyncBlock` messages and emit `BlockReceived` events
- Sync blocks are still sent via gossipsub but with unique message IDs that prevent deduplication

## [4.7.7] - 2025-11-27

### Added
- **JSON-Serialized Commitment Support**: Added server-side support for JSON-serialized commitments to enable client-side mining from web browsers
  - New `Commitment::create_from_json()` method that uses `serde_json::to_vec()` instead of `bincode::serialize()`
  - Updated `Commitment::verify()` to try both bincode and JSON serialization formats
  - This allows web-based miners to submit blocks without needing Rust/bincode compatibility
  - **Benefits**: Makes user submission much easier - users can mine from any web browser without installing Rust toolchain

### Fixed
- **Client-Side Mining Commitment Creation**: Updated commitment creation to use JSON serialization matching server-side `create_from_json()` method
  - Client now uses `JSON.stringify()` which matches server's `serde_json::to_vec()`
  - Commitment verification now accepts both bincode (server-side) and JSON (client-side) formats
  - Blocks submitted from web browsers should now pass validation and receive rewards

### Known Issues / Ongoing Work
- **Chain data persistence on droplets**: Redeploying without restoring `/root/coinject-data` wipes the chain and PeerID. Added a persistence guide (`docs/node-state-persistence.md`), but production deploys still require manual backup/restore until we automate snapshots.

## [4.7.6] - 2025-11-27

### Fixed
- **Client-Side Mining Block Serialization**
  - Fixed Hash and Address serialization to use byte arrays `[u8; 32]` instead of hex strings
  - Fixed coinbase transaction field name from `recipient` to `to` to match Rust `CoinbaseTransaction`
  - Fixed balance type from string to number for u128 serialization
  - Added automatic block serialization in RPC client to ensure correct format
  - Fixed hash extraction to handle byte arrays, hex strings, and object formats

- **RPC Block Submission Reward Application**
  - Fixed bug where blocks not accepted as "new best" would skip transaction application
  - Now applies coinbase rewards even for fork blocks (will reorganize if needed)
  - Improved logging to distinguish between new best blocks and fork blocks
  - Fixed "Duplicate" broadcast errors being treated as failures

- **Frontend Mining Feedback**
  - Added balance check after block submission to show reward status
  - Displays reward amount and current balance after successful submission
  - Better error handling and user feedback for mining operations

### Changed
- **Block Submission Flow**
  - RPC-submitted blocks now always apply transactions, even if not immediately the new best
  - Improved error messages to distinguish between storage failures and fork blocks
  - Broadcast errors for duplicate blocks are now silently ignored (expected behavior)

## [4.7.5] - 2025-11-27

### Added
- **Client-Side Mining via Web Terminal**
  - Full client-side mining implementation in TypeScript/JavaScript
  - Deterministic problem generation from `prev_hash + height` (matches server-side)
  - Problem solvers for SubsetSum (DP), SAT (brute force), and TSP (nearest neighbor)
  - Commitment creation using blake3 hashing with epoch salt
  - Header mining with nonce search to meet difficulty target
  - Complete block construction and submission via RPC
  - Terminal command `mine submit` now performs full mining workflow
  - Miners can now mine and submit blocks directly from the web interface

- **RPC Block Submission API**
  - New RPC method `chain_submitBlock(block)` for submitting mined blocks
  - Block validation, storage, and network broadcasting
  - Integrated with network event loop for proper block processing
  - Supports web-based miners submitting blocks via RPC

### Changed
- **Terminal Mining Commands**
  - `mine submit` command now performs actual mining instead of showing instructions
  - Real-time mining progress and block submission feedback
  - Automatic problem generation, solving, commitment creation, and header mining

### Technical Details
- Client-side mining uses seeded RNG (LCG) for deterministic problem generation
- Problem solvers match server-side implementations for compatibility
- Commitment protocol prevents pre-mining attacks using epoch salt (prev_hash)
- Header mining searches for nonce meeting difficulty (N leading zeros in hash)
- Full block structure with PoUW metrics, work score, and solution reveal

## [4.7.4] - 2025-11-25

### Added
- **Bootnode Connection Retry Logic**
  - Automatic retry mechanism for bootnode connections that drop or fail
  - Tracks bootnode addresses and their associated PeerIDs for reconnection
  - Retries disconnected bootnodes every 10 seconds with exponential backoff protection
  - Handles ephemeral connections common in serverless environments (e.g., Cloud Run)
  - Detects bootnode disconnections and automatically attempts to reconnect
  - Logs bootnode connection status and retry attempts for debugging
  - This fixes the issue where Cloud Run nodes would connect but then immediately disconnect, preventing chain sync

### Changed
- **Connection Management**
  - Enhanced connection event handling to track bootnode connections separately
  - Improved error handling for outgoing connection failures with automatic retry
  - Network service now maintains persistent bootnode connection attempts
- **Hugging Face Flush Cadence**
  - Increased unified dataset flush window from 50 to 600 unique blocks (~10 minutes at current solve rate) to reduce commit spam.
  - Fallback timer now checks every 10 minutes and only forces a flush if >600 blocks accumulated without hitting the primary threshold.
  - Keeps per-block telemetry intact while letting Hugging Face ingestion breathe and batch metadata for downstream analysts.

### Known Issues
- **Cloud Run ↔ Node 2 Connectivity**
  - Cloud Run is currently dialling both droplet bootnodes (Node 1 + Node 2) using IP-only multiaddrs.
  - Kademlia has not yet discovered Node 2’s PeerID, so Cloud Run is only peering with Node 1.
  - We are monitoring the DHT bootstrap to confirm Node 2 is added once its PeerID is advertised.
- **Cloud Run Block Sync Stall**
  - Even after a clean redeploy, Cloud Run buffers thousands of blocks (up to height ~10.6k) but never applies them.
  - Logs show repeated “Missing block 15” requests, which means heights 1‑14 never make it across before higher blocks arrive.
  - Action item: investigate why early blocks aren’t being served (possible request deduplication) so the buffered backlog can flush and sync can progress.

## [4.7.3] - 2025-11-25

### Fixed
- **Kademlia DHT Bootstrap**
  - Added automatic Kademlia bootstrap when peers are identified and when connections are established.
  - Kademlia DHT now actively queries for peers after connecting to bootnodes, enabling automatic peer discovery.
  - This allows nodes (like Cloud Run) to discover additional peers (e.g., Node 2) through P2P discovery without requiring explicit bootnode configuration.
  - Added event handling for Kademlia bootstrap completion and GetClosestPeers queries.

### Changed
- **HuggingFace Flush Strategy**
  - Changed from record-based flushing (every 10 records) to block-based flushing (every 50 blocks by default).
  - This allows data to accumulate and be processed in larger batches, reducing API calls and improving data processing efficiency.
  - Added `flush_interval_blocks` configuration option (default: 50 blocks).
  - Block tracking ensures unique blocks are counted correctly, even when multiple records exist per block.
  - Fallback periodic flush (every 5 minutes) ensures data is flushed even if block-based flushing doesn't trigger during sync.

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

[4.7.6]: https://github.com/beanapologist/COINjecture-NetB-Updates/releases/tag/v4.7.6
[4.7.5]: https://github.com/beanapologist/COINjecture-NetB-Updates/releases/tag/v4.7.5
[4.7.4]: https://github.com/beanapologist/COINjecture-NetB-Updates/releases/tag/v4.7.4
[4.7.3]: https://github.com/beanapologist/COINjecture-NetB-Updates/releases/tag/v4.7.3
[4.6.4]: https://github.com/beanapologist/COINjecture-NetB-Updates/releases/tag/v4.6.4
[4.6.3]: https://github.com/beanapologist/COINjecture-NetB-Updates/releases/tag/v4.6.3
[4.6.2]: https://github.com/beanapologist/COINjecture-NetB-Updates/releases/tag/v4.6.2
[4.6.1]: https://github.com/beanapologist/COINjecture-NetB-Updates/releases/tag/v4.6.1
[4.6.0]: https://github.com/beanapologist/COINjecture-NetB-Updates/releases/tag/v4.6.0
[4.5.0]: https://github.com/beanapologist/COINjecture-NetB-Updates/releases/tag/v4.5.0

