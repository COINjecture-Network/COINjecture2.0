// Fork Detection and Chain Reorganization
// Handles chain reorganization, fork detection, and chain comparison
#![allow(dead_code)]

use super::*;
use tracing::{debug, error, info, trace, warn};

impl CoinjectNode {
    /// Check for chain reorganization opportunities
    /// When we have blocks that form a longer valid chain, reorganize to it
    pub(crate) async fn check_and_reorganize_chain(
        chain: &Arc<ChainState>,
        state: &Arc<AccountState>,
        timelock_state: &Arc<TimeLockState>,
        escrow_state: &Arc<EscrowState>,
        channel_state: &Arc<ChannelState>,
        trustline_state: &Arc<TrustLineState>,
        dimensional_pool_state: &Arc<DimensionalPoolState>,
        marketplace_state: &Arc<MarketplaceState>,
        validator: &Arc<BlockValidator>,
        block_buffer: &Arc<RwLock<HashMap<u64, coinject_core::Block>>>,
        network_cmd_tx: Option<&mpsc::UnboundedSender<NetworkCommand>>,
        cpp_network_cmd_tx: Option<&mpsc::UnboundedSender<CppNetworkCommand>>,
        peer_consensus: &Arc<PeerConsensus>,
    ) {
        let current_best_height = chain.best_block_height().await;
        let current_best_hash = chain.best_block_hash().await;

        debug!(block_height = current_best_height, block_hash = ?current_best_hash, "reorganization check");

        // Check if we have blocks in buffer that might form a longer chain
        let buffer = block_buffer.read().await;
        let buffer_info = if buffer.is_empty() {
            trace!("buffer is empty, checking stored blocks only");
            (0, Vec::new())
        } else {
            let heights: Vec<u64> = buffer.keys().take(10).copied().collect();
            trace!(buffer_len = buffer.len(), "buffer has blocks");
            (buffer.keys().max().copied().unwrap_or(0), heights)
        };
        drop(buffer);

        // Find the highest block in buffer
        let max_buffered_height = buffer_info.0;

        // v4.7.45 FIX: Use find_common_ancestor() for buffered blocks to handle earlier forks properly
        // If we have blocks that extend beyond our current best, check if they form a valid chain
        if max_buffered_height > current_best_height {
            // Re-acquire buffer lock to check for chain path
            let buffer = block_buffer.read().await;

            // Find the highest buffered block and use find_common_ancestor to check for forks
            if let Some(highest_block) = buffer.get(&max_buffered_height) {
                let highest_hash = highest_block.header.hash();
                drop(buffer); // Release lock before async call

                // Use find_common_ancestor to properly detect if this is a fork from earlier
                match chain
                    .find_common_ancestor(&highest_hash, max_buffered_height)
                    .await
                {
                    Ok(Some((_common_hash, common_height))) => {
                        if common_height < current_best_height {
                            // This buffered chain forks from before our current best - it's a reorganization candidate
                            let fork_length = max_buffered_height - common_height;
                            let our_length = current_best_height - common_height;
                            warn!(
                                buffered_height = max_buffered_height,
                                fork_height = common_height,
                                fork_length = fork_length,
                                our_length = our_length,
                                "buffered chain forks before current tip"
                            );

                            if fork_length > our_length {
                                warn!(
                                    fork_length = fork_length,
                                    our_length = our_length,
                                    "buffered fork is longer than current chain"
                                );
                            }
                        } else {
                            debug!(
                                common_height = common_height,
                                "buffered blocks connect to current chain"
                            );
                        }
                    }
                    Ok(None) => {
                        // COMPLETE FORK DETECTED: No common ancestor means we're on a completely different chain
                        // This requires a full chain review from genesis OR complete chain replacement
                        warn!(
                            buffered_height = max_buffered_height,
                            "complete fork detected: no common ancestor with buffered chain"
                        );

                        // Check if the buffered chain is longer than ours
                        if max_buffered_height > current_best_height {
                            warn!(
                                fork_height = max_buffered_height,
                                our_height = current_best_height,
                                "fork chain is longer, requesting full chain from best peer"
                            );

                            // Promote the currently buffered alternate tip directly into the
                            // complete-fork reorg path. Waiting for the later stored-block scan
                            // can leave us buffering forever if the alternate branch never looks
                            // like a normal sequential extension of our current best chain.
                            let chain_clone = Arc::clone(chain);
                            let chain_for_requests = Arc::clone(chain);
                            let state_clone = Arc::clone(state);
                            let timelock_clone = Arc::clone(timelock_state);
                            let escrow_clone = Arc::clone(escrow_state);
                            let channel_clone = Arc::clone(channel_state);
                            let trustline_clone = Arc::clone(trustline_state);
                            let dimensional_clone = Arc::clone(dimensional_pool_state);
                            let marketplace_clone = Arc::clone(marketplace_state);
                            let validator_clone = Arc::clone(validator);
                            let block_buffer_clone = Arc::clone(block_buffer);
                            let block_buffer_for_requests = Arc::clone(block_buffer);
                            let candidate_tip_hash = highest_hash;
                            let candidate_tip_height = max_buffered_height;

                            tokio::spawn(async move {
                                match Self::attempt_reorganization_if_longer_chain(
                                    candidate_tip_hash,
                                    candidate_tip_height,
                                    &chain_clone,
                                    &state_clone,
                                    &timelock_clone,
                                    &escrow_clone,
                                    &channel_clone,
                                    &trustline_clone,
                                    &dimensional_clone,
                                    &marketplace_clone,
                                    &validator_clone,
                                    Some(&block_buffer_clone),
                                )
                                .await
                                {
                                    Ok(true) => {
                                        info!(
                                            candidate_tip_height,
                                            candidate_tip_hash = ?candidate_tip_hash,
                                            "buffered complete-fork candidate triggered successful reorganization"
                                        );
                                    }
                                    Ok(false) => {
                                        warn!(
                                            candidate_tip_height,
                                            candidate_tip_hash = ?candidate_tip_hash,
                                            "buffered complete-fork candidate did not pass reorganization checks"
                                        );
                                    }
                                    Err(e) => {
                                        error!(
                                            candidate_tip_height,
                                            candidate_tip_hash = ?candidate_tip_hash,
                                            error = %e,
                                            "buffered complete-fork candidate failed during reorganization attempt"
                                        );
                                    }
                                }
                            });

                            // Use CPP network commands with best peer from peer_consensus
                            if let Some(cpp_tx) = cpp_network_cmd_tx {
                                // Get best peer from peer_consensus (highest height)
                                let active_peers = peer_consensus.active_peers().await;

                                if let Some((peer_id_str, peer_state)) = active_peers
                                    .iter()
                                    .max_by_key(|(_, state)| state.best_height)
                                {
                                    // Parse peer_id from hex string
                                    if let Ok(peer_id_bytes) = hex::decode(peer_id_str) {
                                        if peer_id_bytes.len() == 32 {
                                            let mut peer_id = [0u8; 32];
                                            peer_id.copy_from_slice(&peer_id_bytes[..32]);

                                            // Request one review window around our current tip instead of
                                            // blasting the full chain from genesis. The old behavior could
                                            // enqueue dozens of GetBlocks requests at once and trip the
                                            // bootnode's short-ban rate limiter before we learned anything
                                            // useful about the competing branch.
                                            const CHUNK_SIZE: u64 = 16; // MAX_BLOCKS_PER_RESPONSE
                                            let from_height = current_best_height.saturating_sub(CHUNK_SIZE);
                                            let to_height = max_buffered_height.min(current_best_height + CHUNK_SIZE);
                                            let request_id: u64 = rand::random();

                                            debug!(
                                                chunk_size = CHUNK_SIZE,
                                                from_height,
                                                to_height,
                                                peer_id = &peer_id_str[..8],
                                                peer_height = peer_state.best_height,
                                                "requesting fork review window via cpp"
                                            );

                                            if let Err(e) =
                                                cpp_tx.send(CppNetworkCommand::RequestBlocks {
                                                    peer_id,
                                                    from_height,
                                                    to_height,
                                                    request_id,
                                                })
                                            {
                                                error!(from_height, to_height, error = %e, "failed to request fork review window");
                                            }

                                            // Walk backward toward the first competing ancestor instead
                                            // of pinning recovery to only the current tip height. When
                                            // complete-fork validation reports a missing ancestor, the
                                            // conflicting historical blocks we already buffered tell us
                                            // how far back the alternate branch is known to exist. Ask
                                            // for the window immediately before the earliest buffered
                                            // conflict so we can progressively recover 513, 512, etc.
                                            let earliest_conflicting_height = {
                                                let buffer = block_buffer_for_requests.read().await;
                                                let mut earliest: Option<u64> = None;

                                                for (height, block) in buffer.iter() {
                                                    if *height > current_best_height {
                                                        continue;
                                                    }

                                                    let conflicts_with_local =
                                                        match chain_for_requests.get_block_by_height(*height)
                                                        {
                                                            Ok(Some(existing_block)) => {
                                                                existing_block.header.hash()
                                                                    != block.header.hash()
                                                            }
                                                            Ok(None) => true,
                                                            Err(err) => {
                                                                warn!(
                                                                    block_height = *height,
                                                                    error = %err,
                                                                    "failed to inspect local block while planning complete-fork recovery window"
                                                                );
                                                                true
                                                            }
                                                        };

                                                    if conflicts_with_local {
                                                        earliest = Some(match earliest {
                                                            Some(existing) => existing.min(*height),
                                                            None => *height,
                                                        });
                                                    }
                                                }

                                                earliest
                                            };

                                            let ancestor_probe_height = earliest_conflicting_height
                                                .unwrap_or(current_best_height);

                                            // Backfill multiple earlier windows per cycle so complete-fork
                                            // validation can reach a sufficiently anchored competing branch
                                            // without waiting for one stalled validation round per chunk.
                                            const MAX_RECOVERY_WINDOWS_PER_CYCLE: u64 = 4;
                                            let mut recovery_cursor = ancestor_probe_height;

                                            for recovery_step in 0..MAX_RECOVERY_WINDOWS_PER_CYCLE {
                                                let ancestor_probe_from =
                                                    recovery_cursor.saturating_sub(CHUNK_SIZE);
                                                let ancestor_probe_to =
                                                    recovery_cursor.saturating_sub(1);

                                                if ancestor_probe_from > ancestor_probe_to {
                                                    break;
                                                }

                                                let ancestor_probe_request_id: u64 = rand::random();
                                                debug!(
                                                    ancestor_probe_from,
                                                    ancestor_probe_to,
                                                    recovery_step,
                                                    earliest_conflicting_height,
                                                    peer_id = &peer_id_str[..8],
                                                    "requesting complete-fork ancestor recovery window via cpp"
                                                );
                                                if let Err(e) =
                                                    cpp_tx.send(CppNetworkCommand::RequestBlocks {
                                                        peer_id,
                                                        from_height: ancestor_probe_from,
                                                        to_height: ancestor_probe_to,
                                                        request_id: ancestor_probe_request_id,
                                                    })
                                                {
                                                    error!(
                                                        ancestor_probe_from,
                                                        ancestor_probe_to,
                                                        recovery_step,
                                                        error = %e,
                                                        "failed to request complete-fork ancestor recovery window"
                                                    );
                                                }

                                                // Request the stitch point immediately above each recovery
                                                // window so validation can connect the newly fetched range
                                                // to the already-buffered competing branch.
                                                let fork_point_height = recovery_cursor;
                                                let fork_point_request_id: u64 = rand::random();
                                                debug!(
                                                    fork_point_height,
                                                    recovery_step,
                                                    earliest_conflicting_height,
                                                    peer_id = &peer_id_str[..8],
                                                    "requesting explicit fork-point block via cpp"
                                                );
                                                if let Err(e) =
                                                    cpp_tx.send(CppNetworkCommand::RequestBlocks {
                                                        peer_id,
                                                        from_height: fork_point_height,
                                                        to_height: fork_point_height,
                                                        request_id: fork_point_request_id,
                                                    })
                                                {
                                                    error!(
                                                        fork_point_height,
                                                        recovery_step,
                                                        error = %e,
                                                        "failed to request explicit fork-point block"
                                                    );
                                                }

                                                if ancestor_probe_from == 0 {
                                                    break;
                                                }
                                                recovery_cursor = ancestor_probe_from;
                                            }
                                        } else {
                                            error!(peer_id = %peer_id_str, actual_len = peer_id_bytes.len(), "invalid peer_id length, expected 32 bytes");
                                        }
                                    } else {
                                        error!(peer_id = %peer_id_str, "failed to decode peer_id from hex");
                                    }
                                } else {
                                    warn!("no active peers available, cannot request blocks");
                                }
                            } else {
                                warn!("no cpp network command channel available to request full chain");
                            }
                        } else {
                            debug!(
                                buffered_height = max_buffered_height,
                                our_height = current_best_height,
                                "fork chain is not longer, keeping current chain"
                            );
                        }
                    }
                    Err(e) => {
                        warn!(error = ?e, "error finding common ancestor for buffered blocks");
                    }
                }
            } else {
                drop(buffer);
            }

            // Also try sequential path building for directly connected blocks
            let buffer = block_buffer.read().await;
            let mut chain_path = Vec::new();
            let mut walk_hash = current_best_hash;
            let mut walk_height = current_best_height;

            // Try to find a path through buffered blocks
            while walk_height < max_buffered_height {
                let next_height = walk_height + 1;

                // Look for a block at next_height that connects to walk_hash
                let mut found = false;
                for (height, block) in buffer.iter() {
                    if *height == next_height && block.header.prev_hash == walk_hash {
                        chain_path.push(block.clone());
                        walk_hash = block.header.hash();
                        walk_height = next_height;
                        found = true;
                        break;
                    }
                }

                if !found {
                    // Can't form a complete chain from buffer at this point
                    break;
                }
            }
            drop(buffer);

            // If we found a complete chain path, it will be processed by process_buffered_blocks
            // This check is mainly for detecting forks
        }

        // Check for forks at same height - if we have a block at current height with different hash
        // and it's part of a longer chain, we should reorganize
        {
            let buffer = block_buffer.read().await;
            if let Some(fork_block) = buffer.get(&current_best_height) {
                if fork_block.header.hash() != current_best_hash {
                    // Fork detected - we'd need to request the full chain from the peer
                    // to see if it's longer. This is handled by status update handler.
                    warn!(
                        block_height = current_best_height,
                        "fork block at current height detected in buffer, waiting for full chain"
                    );
                }
            }
        }

        // Also check stored blocks for longer chains (not just buffer)
        // This is critical when we've received and stored blocks from a longer fork
        // Instead of scanning sequentially (which stops at first missing block),
        // scan the buffer for blocks that might connect to our chain, then check if they're stored
        let mut max_stored_height = current_best_height;
        let mut max_stored_hash = current_best_hash;

        // First, check buffer for blocks that might form a chain
        // Look for blocks whose previous hash matches blocks in our current chain
        let buffer = block_buffer.read().await;
        let buffer_heights: Vec<u64> = buffer.keys().copied().collect();
        drop(buffer);

        if !buffer_heights.is_empty() {
            trace!(
                buffered_count = buffer_heights.len(),
                "checking buffered blocks for chain connections"
            );

            // Find the highest block in buffer
            let max_buffered_height = *buffer_heights.iter().max().unwrap_or(&current_best_height);

            // Try to find a chain path from current best to buffered blocks
            // Check ALL blocks in buffer, not just the highest one, to find any that connect
            if max_buffered_height > current_best_height {
                let buffer = block_buffer.read().await;
                let mut best_candidate_height = current_best_height;
                let mut best_candidate_hash = current_best_hash;

                // Iterate through buffered blocks to find ANY that connect to our current best chain
                // Sort by height descending to check highest blocks first
                let mut sorted_heights: Vec<u64> = buffer_heights
                    .iter()
                    .copied()
                    .filter(|&h| h > current_best_height)
                    .collect();
                sorted_heights.sort_by(|a, b| b.cmp(a)); // Descending order

                // Walk back from current best to build a set of hashes that are on our current chain
                let mut current_chain_hashes = std::collections::HashSet::new();
                let mut walk_back_hash = current_best_hash;
                let mut walk_back_height = current_best_height;
                for _ in 0..1000 {
                    // Walk back up to 1000 blocks
                    current_chain_hashes.insert(walk_back_hash);
                    if walk_back_height == 0 {
                        break;
                    }
                    if let Ok(Some(prev_block)) = chain.get_block_by_hash(&walk_back_hash) {
                        walk_back_hash = prev_block.header.prev_hash;
                        walk_back_height -= 1;
                    } else {
                        break;
                    }
                }

                for &check_height in sorted_heights.iter().take(100) {
                    // Limit to top 100 to avoid excessive checks
                    if let Some(block) = buffer.get(&check_height) {
                        // Check if this block's previous hash is on our current best chain
                        if current_chain_hashes.contains(&block.header.prev_hash) {
                            // Found a connection! This block connects to our current best chain
                            // Walk forward from this connection point to see how far the chain extends
                            let mut walk_height = check_height;
                            let mut walk_hash = block.header.hash();
                            let mut valid_chain = true;
                            let mut chain_end_height = check_height;
                            let mut chain_end_hash = walk_hash;

                            // Walk forward to find the end of this chain
                            while valid_chain {
                                // Check if next block exists in buffer or is stored
                                let next_height = walk_height + 1;
                                let mut found_next = false;

                                // Check buffer first
                                if let Some(next_block) = buffer.get(&next_height) {
                                    if next_block.header.prev_hash == walk_hash {
                                        walk_height = next_height;
                                        walk_hash = next_block.header.hash();
                                        chain_end_height = next_height;
                                        chain_end_hash = walk_hash;
                                        found_next = true;
                                    }
                                }

                                // Also check if stored block exists at next height
                                if !found_next {
                                    if let Ok(Some(stored_block)) =
                                        chain.get_block_by_height(next_height)
                                    {
                                        if stored_block.header.prev_hash == walk_hash {
                                            walk_height = next_height;
                                            walk_hash = stored_block.header.hash();
                                            chain_end_height = next_height;
                                            chain_end_hash = walk_hash;
                                            found_next = true;
                                        }
                                    }
                                }

                                if !found_next {
                                    valid_chain = false;
                                }

                                // Limit walk to prevent infinite loops
                                if walk_height > check_height + 1000 {
                                    break;
                                }
                            }

                            // If this chain is longer than our best candidate, use it
                            if chain_end_height > best_candidate_height {
                                best_candidate_height = chain_end_height;
                                best_candidate_hash = chain_end_hash;
                                debug!(
                                    chain_end_height = chain_end_height,
                                    prev_hash = ?block.header.prev_hash,
                                    chain_end_hash = ?chain_end_hash,
                                    "found potential chain connection"
                                );
                            }
                        }
                    }
                }

                if best_candidate_height > current_best_height {
                    max_stored_height = best_candidate_height;
                    max_stored_hash = best_candidate_hash;
                }
            }
        }

        // Also do sequential scan for blocks that are directly connected (no gaps)
        // This handles the case where blocks are stored sequentially
        // Scan up to 1000 blocks ahead, but don't stop at first missing block
        let scan_limit = current_best_height + 1000;
        trace!(
            from_height = current_best_height + 1,
            to_height = scan_limit,
            "scanning stored blocks sequentially"
        );
        for height in (current_best_height + 1)..=scan_limit {
            if let Ok(Some(block)) = chain.get_block_by_height(height) {
                if height <= current_best_height + 10 {
                    trace!(block_height = height, "found block in sequential scan");
                }
                // Verify this block is part of a valid chain by checking its previous block
                if let Ok(Some(prev_block)) = chain.get_block_by_hash(&block.header.prev_hash) {
                    if prev_block.header.height == height - 1 {
                        // Valid chain continuation
                        if height > max_stored_height {
                            max_stored_height = height;
                            max_stored_hash = block.header.hash();
                        }
                    } else {
                        // Chain broken - but don't stop, continue scanning
                        if height <= current_best_height + 10 {
                            warn!(
                                block_height = height,
                                prev_height = prev_block.header.height,
                                "chain broken in sequential scan"
                            );
                        }
                    }
                } else {
                    // Previous block not found - but don't stop, continue scanning
                    if height <= current_best_height + 10 {
                        warn!(block_height = height, prev_hash = ?block.header.prev_hash, "previous block not found in sequential scan");
                    }
                }
            }
            // Don't break on missing blocks - continue scanning to find any stored blocks
        }

        // If we found blocks ahead but with gaps, request missing blocks aggressively
        if max_stored_height > current_best_height + 1 {
            // We have blocks ahead but possibly with gaps
            // Request the full range to fill gaps for reorganization
            let from_height = current_best_height + 1;
            let to_height = max_stored_height;

            debug!(
                from_height = from_height,
                to_height = to_height,
                "requesting missing blocks to complete chain for reorg"
            );

            if let Some(cmd_tx) = network_cmd_tx {
                if let Err(e) = cmd_tx.send(NetworkCommand::RequestBlocks {
                    from_height,
                    to_height,
                }) {
                    error!(from_height = from_height, to_height = to_height, error = %e, "failed to request missing blocks for reorganization");
                }
            }
        }

        // If we found a longer chain in stored blocks, attempt reorganization
        if max_stored_height > current_best_height {
            info!(
                max_stored_height = max_stored_height,
                "found longer chain in stored blocks, attempting reorganization"
            );

            // Check if this chain has no common ancestor (complete fork)
            // If so, we need to validate from genesis
            match chain
                .find_common_ancestor(&max_stored_hash, max_stored_height)
                .await
            {
                Ok(Some((_common_hash, common_height))) => {
                    debug!(common_height = common_height, "found common ancestor");
                    // Normal reorganization with common ancestor
                }
                Ok(None) => {
                    warn!("no common ancestor found, complete fork will be validated from genesis");
                }
                Err(e) => {
                    warn!(error = ?e, "error finding common ancestor");
                }
            }

            let chain_clone = Arc::clone(chain);
            let state_clone = Arc::clone(state);
            let timelock_clone = Arc::clone(timelock_state);
            let escrow_clone = Arc::clone(escrow_state);
            let channel_clone = Arc::clone(channel_state);
            let trustline_clone = Arc::clone(trustline_state);
            let dimensional_clone = Arc::clone(dimensional_pool_state);
            let marketplace_clone = Arc::clone(marketplace_state);
            let validator_clone = Arc::clone(validator);

            tokio::spawn(async move {
                if let Err(e) = Self::attempt_reorganization_if_longer_chain(
                    max_stored_hash,
                    max_stored_height,
                    &chain_clone,
                    &state_clone,
                    &timelock_clone,
                    &escrow_clone,
                    &channel_clone,
                    &trustline_clone,
                    &dimensional_clone,
                    &marketplace_clone,
                    &validator_clone,
                    None,
                )
                .await
                {
                    error!(error = %e, "failed to attempt reorganization for stored blocks");
                }
            });
        }
    }

    /// Attempt chain reorganization when we have a longer valid chain available
    /// This is called when we've received blocks that form a longer chain than our current best
    async fn attempt_reorganization_if_longer_chain(
        new_chain_end_hash: coinject_core::Hash,
        new_chain_end_height: u64,
        chain: &Arc<ChainState>,
        state: &Arc<AccountState>,
        timelock_state: &Arc<TimeLockState>,
        escrow_state: &Arc<EscrowState>,
        channel_state: &Arc<ChannelState>,
        trustline_state: &Arc<TrustLineState>,
        dimensional_pool_state: &Arc<DimensionalPoolState>,
        marketplace_state: &Arc<MarketplaceState>,
        validator: &Arc<BlockValidator>,
        block_buffer: Option<&Arc<RwLock<HashMap<u64, coinject_core::Block>>>>,
    ) -> Result<bool, String> {
        let current_best_height = chain.best_block_height().await;
        let current_best_hash = chain.best_block_hash().await;

        // Only reorganize if new chain is actually longer
        if new_chain_end_height <= current_best_height {
            return Ok(false);
        }

        // COMMON ANCESTOR ANCHORING: Find common ancestor and validate it's anchored
        // This ensures we only reorganize to chains that share a valid common ancestor
        // The common ancestor must be:
        // 1. At least 6 blocks deep (to prevent shallow reorganizations)
        // 2. Valid and stored in our chain
        // 3. Not at genesis (unless absolutely necessary)
        let (_common_hash, common_height) = match chain
            .find_common_ancestor(&new_chain_end_hash, new_chain_end_height)
            .await
            .map_err(|e| format!("Failed to find common ancestor: {}", e))
        {
            Ok(Some((hash, height))) => {
                // Validate common ancestor is anchored (at least 6 blocks deep)
                const MIN_ANCHOR_DEPTH: u64 = 6;
                if height < MIN_ANCHOR_DEPTH {
                    warn!(
                        fork_height = height,
                        min_anchor_depth = MIN_ANCHOR_DEPTH,
                        "common ancestor too shallow, cannot reorganize"
                    );
                    return Ok(false);
                }

                // Verify common ancestor block exists and is valid
                match chain.get_block_by_hash(&hash) {
                    Ok(Some(block)) => {
                        // Verify the block is actually on our current chain
                        let current_best = chain.best_block_height().await;
                        if block.header.height > current_best {
                            warn!(
                                ancestor_height = block.header.height,
                                our_best = current_best,
                                "common ancestor block is ahead of our best"
                            );
                            return Ok(false);
                        }
                        (hash, height)
                    }
                    Ok(None) => {
                        warn!("common ancestor block not found in storage");
                        return Ok(false);
                    }
                    Err(e) => {
                        warn!(error = %e, "error verifying common ancestor");
                        return Ok(false);
                    }
                }
            }
            Ok(None) => {
                // COMPLETE FORK DETECTED: No common ancestor means we're on a completely different chain
                // This requires a full chain review from genesis
                warn!(new_chain_end_height = new_chain_end_height, "complete fork detected: no common ancestor found, requires full chain validation from genesis");
                info!(
                    new_chain_end_height,
                    new_chain_end_hash = ?new_chain_end_hash,
                    current_best_height,
                    current_best_hash = ?current_best_hash,
                    "starting complete-fork validation path"
                );

                // Request full chain from genesis to validate the new chain
                // The caller should have already requested blocks, but we need to ensure we have the full chain
                // For now, we'll attempt to validate what we have and request if needed
                // This will be handled by the reorganization check that triggers this

                // Validate the new chain from genesis
                match Self::validate_chain_from_genesis(
                    &new_chain_end_hash,
                    new_chain_end_height,
                    chain,
                    validator,
                    block_buffer,
                )
                .await
                {
                    Ok((new_chain_blocks, new_chain_work)) => {
                        info!(
                            block_count = new_chain_blocks.len(),
                            total_work = new_chain_work,
                            "new chain validated from genesis"
                        );

                        // Get our current chain from genesis
                        let (our_chain_blocks, our_chain_work) = match Self::get_chain_from_genesis(
                            current_best_hash,
                            current_best_height,
                            chain,
                        )
                        .await
                        {
                            Ok(chain_data) => chain_data,
                            Err(e) => {
                                warn!(error = %e, "failed to get our chain from genesis");
                                return Ok(false);
                            }
                        };

                        info!(
                            our_blocks = our_chain_blocks.len(),
                            our_work = our_chain_work,
                            new_blocks = new_chain_blocks.len(),
                            new_work = new_chain_work,
                            "complete chain comparison"
                        );

                        // Compare by work score first, then height
                        use crate::peer_consensus::WorkScoreCalculator;
                        let comparison =
                            WorkScoreCalculator::compare_chains(our_chain_work, new_chain_work);

                        if comparison <= 0 && new_chain_end_height <= current_best_height {
                            // Our chain has equal or more work and equal or greater height
                            debug!("skipping reorganization: our chain has equal or better work/height");
                            return Ok(false);
                        }

                        // New chain is better - reorganize from genesis
                        warn!(
                            old_blocks = our_chain_blocks.len(),
                            old_work = our_chain_work,
                            new_blocks = new_chain_blocks.len(),
                            new_work = new_chain_work,
                            "reorganizing from genesis"
                        );

                        // Perform complete reorganization (unwind all blocks to genesis, apply new chain)
                        Self::reorganize_chain_from_genesis(
                            our_chain_blocks,
                            new_chain_blocks,
                            chain,
                            state,
                            timelock_state,
                            escrow_state,
                            channel_state,
                            trustline_state,
                            dimensional_pool_state,
                            marketplace_state,
                            validator,
                        )
                        .await?;

                        return Ok(true);
                    }
                    Err(e) => {
                        warn!(
                            error = %e,
                            new_chain_end_height,
                            new_chain_end_hash = ?new_chain_end_hash,
                            "failed to validate new chain from genesis, requesting full chain"
                        );
                        // Return false but the caller should request full chain
                        return Ok(false);
                    }
                }
            }
            Err(e) => return Err(e),
        };

        debug!(fork_height = common_height, old_tip_hash = ?current_best_hash, new_tip_hash = ?new_chain_end_hash, reorg_depth = current_best_height.saturating_sub(common_height), "found anchored common ancestor");

        // Get old chain blocks (from common ancestor to current best, excluding common ancestor)
        let mut old_chain_blocks = Vec::new();
        if common_height < current_best_height {
            for height in (common_height + 1)..=current_best_height {
                match chain.get_block_by_height(height) {
                    Ok(Some(block)) => old_chain_blocks.push(block),
                    Ok(None) => {
                        return Err(format!(
                            "Failed to get old chain block at height {}",
                            height
                        ))
                    }
                    Err(e) => {
                        return Err(format!(
                            "Error getting old chain block at height {}: {}",
                            height, e
                        ))
                    }
                }
            }
            old_chain_blocks.reverse(); // Reverse so we unwind from newest to oldest
        }

        // Get new chain blocks (from common ancestor to new best, excluding common ancestor)
        let mut new_chain_blocks = Vec::new();
        let mut current_hash = new_chain_end_hash;
        let mut current_height = new_chain_end_height;

        // Walk back from new best to common ancestor, collecting blocks
        while current_height > common_height {
            match chain.get_block_by_hash(&current_hash) {
                Ok(Some(block)) => {
                    new_chain_blocks.push(block.clone());
                    current_hash = block.header.prev_hash;
                    current_height -= 1;
                }
                Ok(None) => {
                    return Err(format!(
                        "Failed to get new chain block at height {}",
                        current_height
                    ))
                }
                Err(e) => {
                    return Err(format!(
                        "Error getting new chain block at height {}: {}",
                        current_height, e
                    ))
                }
            }
        }

        // Reverse new_chain_blocks so they're in forward order (common+1 to new_best)
        new_chain_blocks.reverse();

        // CRITICAL: Compare chains by work score, not just length
        // A longer chain might have less total work if blocks have lower work scores
        use crate::peer_consensus::WorkScoreCalculator;

        // Calculate cumulative work scores for both chains
        // Work scores are f64, so we need to convert to u64 for comparison
        let old_chain_work: u64 = old_chain_blocks
            .iter()
            .map(|b| b.header.work_score as u64)
            .sum();
        let new_chain_work: u64 = new_chain_blocks
            .iter()
            .map(|b| b.header.work_score as u64)
            .sum();

        info!(
            old_blocks = old_chain_blocks.len(),
            old_work = old_chain_work,
            new_blocks = new_chain_blocks.len(),
            new_work = new_chain_work,
            "chain comparison"
        );

        // Use work score comparison (with tolerance)
        let comparison = WorkScoreCalculator::compare_chains(old_chain_work, new_chain_work);

        if comparison <= 0 {
            // Our chain has equal or more work, don't reorganize
            debug!(
                comparison = comparison,
                "skipping reorganization: our chain has equal or more work"
            );
            return Ok(false);
        }

        // New chain has more work - proceed with reorganization
        warn!(
            old_tip_hash = ?current_best_hash,
            new_tip_hash = ?new_chain_end_hash,
            reorg_depth = old_chain_blocks.len(),
            old_work = old_chain_work,
            new_work = new_chain_work,
            "chain reorg: switching to heavier chain"
        );

        // Perform reorganization
        Self::reorganize_chain(
            old_chain_blocks,
            new_chain_blocks,
            chain,
            state,
            timelock_state,
            escrow_state,
            channel_state,
            trustline_state,
            dimensional_pool_state,
            marketplace_state,
            validator,
        )
        .await?;

        Ok(true)
    }

    /// Perform chain reorganization: unwind old chain and apply new chain
    async fn reorganize_chain(
        old_chain_blocks: Vec<coinject_core::Block>,
        new_chain_blocks: Vec<coinject_core::Block>,
        chain: &Arc<ChainState>,
        state: &Arc<AccountState>,
        timelock_state: &Arc<TimeLockState>,
        escrow_state: &Arc<EscrowState>,
        channel_state: &Arc<ChannelState>,
        trustline_state: &Arc<TrustLineState>,
        dimensional_pool_state: &Arc<DimensionalPoolState>,
        marketplace_state: &Arc<MarketplaceState>,
        validator: &Arc<BlockValidator>,
    ) -> Result<(), String> {
        warn!(
            unwind_count = old_chain_blocks.len(),
            apply_count = new_chain_blocks.len(),
            "starting chain reorganization"
        );

        // Step 1: Unwind old chain blocks (in reverse order - newest to oldest)
        for block in old_chain_blocks.iter().rev() {
            debug!(block_height = block.header.height, "unwinding block");
            if let Err(e) = Self::unwind_block_transactions(
                block,
                state,
                timelock_state,
                escrow_state,
                channel_state,
                trustline_state,
                dimensional_pool_state,
                marketplace_state,
            ) {
                return Err(format!(
                    "Failed to unwind block {}: {}",
                    block.header.height, e
                ));
            }

            // Also need to reverse dimensional pool state changes
            // This is complex - for now we log a warning
            if block.header.height > 0 {
                warn!(
                    block_height = block.header.height,
                    "dimensional pool state reversal is approximate"
                );
            }
        }

        // Step 2: Validate new chain
        let mut prev_hash = if let Some(first_block) = new_chain_blocks.first() {
            first_block.header.prev_hash
        } else {
            return Err("New chain is empty".to_string());
        };

        for (idx, block) in new_chain_blocks.iter().enumerate() {
            let _expected_height = if idx == 0 {
                // First block height should be common_ancestor_height + 1
                // We'd need to pass this in, but for now we validate relative to prev_hash
                0 // Will be set properly
            } else {
                new_chain_blocks[idx - 1].header.height + 1
            };

            // Validate block connects to previous
            if block.header.prev_hash != prev_hash {
                return Err(format!(
                    "New chain block {} doesn't connect to previous (prev_hash mismatch)",
                    block.header.height
                ));
            }

            // Validate block (skip timestamp age check during chain reorganization/sync)
            match validator.validate_block_with_options(
                block,
                &prev_hash,
                block.header.height,
                true,
            ) {
                Ok(()) => {
                    prev_hash = block.header.hash();
                }
                Err(e) => {
                    return Err(format!(
                        "New chain block {} validation failed: {}",
                        block.header.height, e
                    ));
                }
            }
        }

        // Step 3: Apply new chain blocks
        for block in &new_chain_blocks {
            debug!(
                block_height = block.header.height,
                "applying new chain block"
            );

            // Store block
            chain
                .store_block(block)
                .await
                .map_err(|e| format!("Failed to store block {}: {}", block.header.height, e))?;

            // Apply transactions
            Self::apply_block_transactions(
                block,
                state,
                timelock_state,
                escrow_state,
                channel_state,
                trustline_state,
                dimensional_pool_state,
                marketplace_state,
            )?;

            // Update consensus state
            use coinject_core::{ConsensusState, TAU_C};
            let tau = (block.header.height as f64) / TAU_C;
            let consensus_state = ConsensusState::at_tau(tau);
            dimensional_pool_state
                .save_consensus_state(block.header.height, &consensus_state)
                .map_err(|e| format!("Failed to save consensus state: {}", e))?;
        }

        // Step 4: Update best chain
        if let Some(last_block) = new_chain_blocks.last() {
            chain
                .update_best_chain(last_block.header.hash(), last_block.header.height)
                .await
                .map_err(|e| format!("Failed to update best chain: {}", e))?;
        }

        info!("chain reorganization complete");
        Ok(())
    }

    /// Validate a chain from genesis block
    /// Returns (chain_blocks, total_work_score) if valid
    async fn validate_chain_from_genesis(
        end_hash: &coinject_core::Hash,
        end_height: u64,
        chain: &Arc<ChainState>,
        validator: &Arc<BlockValidator>,
        block_buffer: Option<&Arc<RwLock<HashMap<u64, coinject_core::Block>>>>,
    ) -> Result<(Vec<coinject_core::Block>, u64), String> {
        info!(end_height, end_hash = ?end_hash, "validating candidate chain from genesis");
        let genesis_hash = chain.genesis_hash();
        let mut chain_blocks = Vec::new();
        let mut current_hash = *end_hash;
        let mut current_height = end_height;
        let mut total_work: u64 = 0;

        // Walk back from end to genesis, collecting blocks
        while current_height > 0 {
            match Self::get_candidate_block_by_hash(chain, block_buffer, &current_hash).await {
                Ok(Some(block)) => {
                    // Validate block connects properly
                    if block.header.height != current_height {
                        return Err(format!(
                            "Block height mismatch: expected {}, got {}",
                            current_height, block.header.height
                        ));
                    }

                    // Add work score
                    total_work += block.header.work_score as u64;

                    chain_blocks.push(block.clone());
                    current_hash = block.header.prev_hash;
                    current_height -= 1;
                }
                Ok(None) => {
                    warn!(
                        missing_height = current_height,
                        missing_hash = ?current_hash,
                        "candidate chain validation missing block"
                    );
                    return Err(format!(
                        "Missing block at height {} (hash: {:?})",
                        current_height, current_hash
                    ));
                }
                Err(e) => {
                    return Err(format!(
                        "Error getting block at height {}: {}",
                        current_height, e
                    ));
                }
            }
        }

        // Verify we reached genesis
        if current_hash != genesis_hash {
            return Err(format!(
                "Chain doesn't connect to genesis. Expected {:?}, got {:?}",
                genesis_hash, current_hash
            ));
        }

        // Get genesis block
        match chain.get_block_by_hash(&genesis_hash) {
            Ok(Some(genesis_block)) => {
                if genesis_block.header.height != 0 {
                    return Err("Genesis block has wrong height".to_string());
                }
                total_work += genesis_block.header.work_score as u64;
                chain_blocks.push(genesis_block);
            }
            Ok(None) => {
                return Err("Genesis block not found".to_string());
            }
            Err(e) => {
                return Err(format!("Error getting genesis block: {}", e));
            }
        }

        // Reverse so blocks are in forward order (genesis to end)
        chain_blocks.reverse();

        // Validate chain integrity: each block must connect to previous
        for i in 1..chain_blocks.len() {
            let prev_block = &chain_blocks[i - 1];
            let curr_block = &chain_blocks[i];

            if curr_block.header.prev_hash != prev_block.header.hash() {
                return Err(format!("Chain integrity violation at height {}: prev_hash doesn't match previous block hash", 
                    curr_block.header.height));
            }

            if curr_block.header.height != prev_block.header.height + 1 {
                return Err(format!(
                    "Chain height gap at height {}: expected {}, got {}",
                    curr_block.header.height,
                    prev_block.header.height + 1,
                    curr_block.header.height
                ));
            }
        }

        // Validate all blocks (except genesis, which is assumed valid)
        for i in 1..chain_blocks.len() {
            let block = &chain_blocks[i];
            let prev_hash = chain_blocks[i - 1].header.hash();

            // Validate block (skip timestamp age check during chain validation)
            if let Err(e) =
                validator.validate_block_with_options(block, &prev_hash, block.header.height, true)
            {
                return Err(format!(
                    "Block {} validation failed: {}",
                    block.header.height, e
                ));
            }
        }

        info!(
            block_count = chain_blocks.len(),
            total_work,
            end_height,
            "candidate chain validated from genesis"
        );
        Ok((chain_blocks, total_work))
    }

    async fn get_candidate_block_by_hash(
        chain: &Arc<ChainState>,
        block_buffer: Option<&Arc<RwLock<HashMap<u64, coinject_core::Block>>>>,
        target_hash: &coinject_core::Hash,
    ) -> Result<Option<coinject_core::Block>, String> {
        match chain.get_block_by_hash(target_hash) {
            Ok(Some(block)) => return Ok(Some(block)),
            Ok(None) => {}
            Err(e) => return Err(format!("Error getting block by hash {:?}: {}", target_hash, e)),
        }

        if let Some(buffer) = block_buffer {
            let buffered = {
                let buffer = buffer.read().await;
                buffer
                    .values()
                    .find(|block| block.header.hash() == *target_hash)
                    .cloned()
            };

            if buffered.is_some() {
                debug!(hash = ?target_hash, "resolved candidate block from fork buffer");
            }

            return Ok(buffered);
        }

        Ok(None)
    }

    /// Get our current chain from genesis
    /// Returns (chain_blocks, total_work_score)
    async fn get_chain_from_genesis(
        best_hash: coinject_core::Hash,
        best_height: u64,
        chain: &Arc<ChainState>,
    ) -> Result<(Vec<coinject_core::Block>, u64), String> {
        let genesis_hash = chain.genesis_hash();
        let mut chain_blocks = Vec::new();
        let mut current_hash = best_hash;
        let mut current_height = best_height;
        let mut total_work: u64 = 0;

        // Walk back from best to genesis, collecting blocks
        while current_height > 0 {
            match chain.get_block_by_height(current_height) {
                Ok(Some(block)) => {
                    if block.header.hash() != current_hash {
                        return Err(format!("Block hash mismatch at height {}", current_height));
                    }
                    total_work += block.header.work_score as u64;
                    chain_blocks.push(block.clone());
                    current_hash = block.header.prev_hash;
                    current_height -= 1;
                }
                Ok(None) => {
                    return Err(format!("Missing block at height {}", current_height));
                }
                Err(e) => {
                    return Err(format!(
                        "Error getting block at height {}: {}",
                        current_height, e
                    ));
                }
            }
        }

        // Get genesis block
        match chain.get_block_by_height(0) {
            Ok(Some(genesis_block)) => {
                if genesis_block.header.hash() != genesis_hash {
                    return Err("Genesis block hash mismatch".to_string());
                }
                total_work += genesis_block.header.work_score as u64;
                chain_blocks.push(genesis_block);
            }
            Ok(None) => {
                return Err("Genesis block not found".to_string());
            }
            Err(e) => {
                return Err(format!("Error getting genesis block: {}", e));
            }
        }

        // Reverse so blocks are in forward order (genesis to best)
        chain_blocks.reverse();

        Ok((chain_blocks, total_work))
    }

    /// Perform complete chain reorganization from genesis
    /// Unwinds all blocks to genesis and applies new chain from genesis
    async fn reorganize_chain_from_genesis(
        old_chain_blocks: Vec<coinject_core::Block>,
        new_chain_blocks: Vec<coinject_core::Block>,
        chain: &Arc<ChainState>,
        state: &Arc<AccountState>,
        timelock_state: &Arc<TimeLockState>,
        escrow_state: &Arc<EscrowState>,
        channel_state: &Arc<ChannelState>,
        trustline_state: &Arc<TrustLineState>,
        dimensional_pool_state: &Arc<DimensionalPoolState>,
        marketplace_state: &Arc<MarketplaceState>,
        _validator: &Arc<BlockValidator>,
    ) -> Result<(), String> {
        warn!(
            unwind_count = old_chain_blocks.len(),
            apply_count = new_chain_blocks.len(),
            "starting complete reorganization from genesis"
        );

        // Verify both chains start from genesis
        if old_chain_blocks.is_empty() || new_chain_blocks.is_empty() {
            return Err("Chain is empty".to_string());
        }

        let genesis_hash = chain.genesis_hash();
        if old_chain_blocks[0].header.hash() != genesis_hash
            || new_chain_blocks[0].header.hash() != genesis_hash
        {
            return Err("Chains don't start from genesis".to_string());
        }

        // Step 1: Unwind all old chain blocks (except genesis) in reverse order
        // Start from the last block (highest height) and work backwards
        // FIX: Skip genesis (first element) before reversing, not after
        // old_chain_blocks is [genesis, block1, ..., tip]
        // [1..] skips genesis, then .rev() gives us [tip, ..., block1] (no genesis)
        for block in old_chain_blocks[1..].iter().rev() {
            debug!(block_height = block.header.height, "unwinding block");
            if let Err(e) = Self::unwind_block_transactions(
                block,
                state,
                timelock_state,
                escrow_state,
                channel_state,
                trustline_state,
                dimensional_pool_state,
                marketplace_state,
            ) {
                return Err(format!(
                    "Failed to unwind block {}: {}",
                    block.header.height, e
                ));
            }
        }
        info!("finished unwinding old chain for complete reorganization");

        // Step 2: Validate new chain integrity (already validated in validate_chain_from_genesis)
        // But verify genesis matches
        if new_chain_blocks[0].header.hash() != genesis_hash {
            return Err("New chain doesn't start from correct genesis".to_string());
        }

        // Step 3: Apply new chain blocks (skip genesis, it's already applied)
        for block in new_chain_blocks.iter().skip(1) {
            // Skip genesis
            debug!(
                block_height = block.header.height,
                "applying new chain block"
            );

            // Store block
            chain
                .store_block(block)
                .await
                .map_err(|e| format!("Failed to store block {}: {}", block.header.height, e))?;

            // Apply transactions
            Self::apply_block_transactions(
                block,
                state,
                timelock_state,
                escrow_state,
                channel_state,
                trustline_state,
                dimensional_pool_state,
                marketplace_state,
            )?;

            // Update consensus state
            use coinject_core::{ConsensusState, TAU_C};
            let tau = (block.header.height as f64) / TAU_C;
            let consensus_state = ConsensusState::at_tau(tau);
            dimensional_pool_state
                .save_consensus_state(block.header.height, &consensus_state)
                .map_err(|e| format!("Failed to save consensus state: {}", e))?;
        }
        info!("finished applying new chain blocks for complete reorganization");

        // Step 4: Update best chain
        if let Some(last_block) = new_chain_blocks.last() {
            chain
                .update_best_chain(last_block.header.hash(), last_block.header.height)
                .await
                .map_err(|e| format!("Failed to update best chain: {}", e))?;
        }

        info!("complete reorganization from genesis finished");

        Ok(())
    }
}
