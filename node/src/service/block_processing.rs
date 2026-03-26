// Block Processing
// Transaction application, block unwinding, and buffered block processing
#![allow(dead_code, clippy::too_many_arguments)]

use super::*;
use tracing::{debug, info, warn, error};

impl CoinjectNode {
    /// Process buffered blocks sequentially
    pub(crate) async fn process_buffered_blocks(
        chain: &Arc<ChainState>,
        state: &Arc<AccountState>,
        timelock_state: &Arc<TimeLockState>,
        escrow_state: &Arc<EscrowState>,
        channel_state: &Arc<ChannelState>,
        trustline_state: &Arc<TrustLineState>,
        dimensional_pool_state: &Arc<DimensionalPoolState>,
        marketplace_state: &Arc<MarketplaceState>,
        validator: &Arc<BlockValidator>,
        tx_pool: &Arc<RwLock<TransactionPool>>,
        block_buffer: &Arc<RwLock<HashMap<u64, coinject_core::Block>>>,
        hf_sync: &Option<Arc<HuggingFaceSync>>,
        network_tx: Option<&mpsc::UnboundedSender<NetworkCommand>>,
    ) {
        loop {
            let best_height = chain.best_block_height().await;
            let next_height = best_height + 1;

            // Check if we have the next sequential block in buffer
            let block_opt = {
                let mut buffer = block_buffer.write().await;
                buffer.remove(&next_height)
            };

            match block_opt {
                Some(block) => {
                    debug!(block_height = next_height, "processing buffered block");

                    let best_hash = chain.best_block_hash().await;

                    // Validate the buffered block (skip timestamp age check for historical blocks during sync)
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as i64;
                    let block_age = now - block.header.timestamp;
                    let skip_age_check = block_age > 7200; // 2 hours

                    // FORK HANDLING FIX: Check if buffered block extends our current chain or is a fork block
                    let extends_best_chain = block.header.prev_hash == best_hash;

                    let validation_result = if extends_best_chain {
                        // Normal case: block extends our best chain
                        validator.validate_block_with_options(&block, &best_hash, next_height, skip_age_check)
                    } else {
                        // Fork case: check if we have the parent block
                        match chain.has_block(&block.header.prev_hash) {
                            Ok(true) => {
                                // We have the parent - this is a valid sidechain block
                                warn!(
                                    block_height = block.header.height,
                                    prev_hash = &block.header.prev_hash.to_string()[..16],
                                    our_tip = &best_hash.to_string()[..16],
                                    "buffered fork block detected"
                                );

                                // Validate against its declared parent (not best_hash)
                                validator.validate_block_with_options(&block, &block.header.prev_hash, next_height, skip_age_check)
                            }
                            Ok(false) => {
                                // Parent missing - this is an orphan block from a fork chain
                                // We can't process it without its parent. Don't re-add to buffer
                                // (that would cause infinite loop). The block will be re-sent by peers
                                // during normal sync or gossip if we need it later.
                                warn!(
                                    block_height = block.header.height,
                                    prev_hash = &block.header.prev_hash.to_string()[..16],
                                    "orphan block discarded: parent not found"
                                );

                                // Request missing blocks from peers to help sync
                                if let Some(net_tx) = network_tx {
                                    let _ = net_tx.send(NetworkCommand::RequestBlocks {
                                        from_height: next_height,
                                        to_height: next_height + 10,
                                    });
                                }
                                break; // Exit the loop - can't process more without syncing
                            }
                            Err(e) => {
                                error!(error = %e, "error checking for parent block");
                                continue;
                            }
                        }
                    };

                    match validation_result {
                        Ok(()) => {
                            // Store and apply
                            // During sequential sync, we're processing blocks one by one starting from best_height + 1.
                            // If validation passed (prev_hash matches best_hash and height is sequential), the block
                            // should extend the best chain. However, store_block might return false due to race conditions
                            // or if the block was already stored. In this case, we should still apply it if it extends
                            // our current best chain.
                            match chain.store_block(&block).await {
                                Ok(is_new_best) => {
                                    // Check if this block extends our current best chain
                                    // Since we validated prev_hash == best_hash and height == best_height + 1,
                                    // this block should extend the chain. If is_new_best is false, it might be due
                                    // to a race condition, so we check if it actually extends the chain.
                                    let current_best = chain.best_block_height().await;
                                    let current_best_hash = chain.best_block_hash().await;
                                    
                                    // Block extends chain if: it's the next height AND prev_hash matches current best
                                    let extends_chain = block.header.height == current_best + 1 && block.header.prev_hash == current_best_hash;
                                    
                                    if is_new_best || extends_chain {
                                        // RUNTIME INTEGRATION: Save consensus state for buffered blocks
                                        use coinject_core::{TAU_C, ConsensusState};
                                        let tau = (block.header.height as f64) / TAU_C;
                                        let consensus_state = ConsensusState::at_tau(tau);

                                        if let Err(e) = dimensional_pool_state.save_consensus_state(block.header.height, &consensus_state) {
                                            warn!(block_height = block.header.height, error = %e, "failed to save consensus state");
                                        }

                                        match Self::apply_block_transactions(&block, state, timelock_state, escrow_state, channel_state, trustline_state, dimensional_pool_state, marketplace_state) {
                                            Ok(applied_txs) => {
                                                info!(block_height = next_height, tau = tau, "buffered block applied to chain");

                                                // If store_block didn't update best chain, manually update it
                                                if !is_new_best && extends_chain {
                                                    if let Err(e) = chain.update_best_chain(block.header.hash(), block.header.height).await {
                                                        warn!(block_height = block.header.height, error = %e, "failed to update best chain after buffered block");
                                                    } else {
                                                        info!(block_height = block.header.height, prev_best = current_best, "best chain updated");
                                                    }
                                                }

                                                // Remove only successfully applied transactions from pool
                                                let mut pool = tx_pool.write().await;
                                                for tx_hash in &applied_txs {
                                                    pool.remove(tx_hash);
                                                }
                                                drop(pool);

                                                // Push consensus block to Hugging Face (fire-and-forget)
                                                if let Some(ref hf_sync) = hf_sync {
                                                    let hf_sync_clone = Arc::clone(hf_sync);
                                                    let block_clone = block.clone();
                                                    tokio::spawn(async move {
                                                        if let Err(e) = hf_sync_clone.push_consensus_block(&block_clone, false).await {
                                                            warn!(error = %e, "failed to push consensus block to hugging face");
                                                        }
                                                    });
                                                }

                                                // Continue loop to check for next sequential block
                                            }
                                            Err(e) => {
                                                error!(block_height = next_height, error = %e, "failed to apply buffered block transactions");
                                                break;
                                            }
                                        }
                                    } else {
                                        // Block doesn't extend our chain - might be a fork, duplicate, or out of order
                                        // Skip it and continue processing (don't break, as there might be other sequential blocks)
                                        warn!(block_height = next_height, best_height = current_best, "buffered block does not extend best chain, skipping");
                                        // Continue loop to check for next sequential block
                                    }
                                }
                                Err(e) => {
                                    error!(block_height = next_height, error = %e, "failed to store buffered block");
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            warn!(block_height = next_height, error = %e, "buffered block validation failed");
                            // If validation failed due to invalid prev_hash, the block might have been
                            // buffered before the previous block was applied. Remove it from buffer
                            // so it can be re-received with the correct prev_hash.
                            if e.to_string().contains("Invalid previous hash") {
                                debug!(block_height = next_height, "removing invalid buffered block, will be re-requested");
                                // Block already removed from buffer above, so we can continue
                            }
                            // Don't break - continue to check for next sequential block
                            // The invalid block has been removed, so next iteration will skip it
                            continue;
                        }
                    }
                }
                None => {
                    // No more sequential blocks in buffer
                    // Check if we have blocks ahead in the buffer - if so, request missing blocks
                    let buffer = block_buffer.read().await;
                    if let Some(&max_buffered_height) = buffer.keys().max() {
                        if max_buffered_height > next_height {
                            // We have blocks ahead but missing the next one - request missing blocks
                            // CRITICAL FIX: Request blocks ONE AT A TIME for missing sequential blocks
                            // This ensures we get the exact block we need, even if it doesn't exist on some peers
                            let request_from = next_height;
                            // Request only the next missing block first, then expand if needed
                            let request_to = next_height;
                            
                            warn!(block_height = next_height, max_buffered = max_buffered_height, "missing block, requesting from peers");
                            
                            drop(buffer);
                            
                            // Request missing block ONE AT A TIME if network_tx is available
                            if let Some(network_tx) = network_tx {
                                if let Err(e) = network_tx.send(NetworkCommand::RequestBlocks {
                                    from_height: request_from,
                                    to_height: request_to,
                                }) {
                                    error!(error = %e, "failed to request missing block");
                                }
                            }
                            
                            // Break and wait for block to arrive
                            // Will retry on next call to process_buffered_blocks
                            break;
                        } else {
                            // No blocks ahead in buffer - we're caught up or waiting
                            break;
                        }
                    } else {
                        // Buffer is empty - no blocks to process
                        break;
                    }
                }
            }
        }
    }

    /// Apply block transactions to account state
    /// Returns a vector of successfully applied transaction hashes
    pub(crate) fn apply_block_transactions(
        block: &coinject_core::Block,
        state: &Arc<AccountState>,
        timelock_state: &Arc<TimeLockState>,
        escrow_state: &Arc<EscrowState>,
        channel_state: &Arc<ChannelState>,
        trustline_state: &Arc<TrustLineState>,
        dimensional_pool_state: &Arc<DimensionalPoolState>,
        marketplace_state: &Arc<MarketplaceState>,
    ) -> Result<Vec<coinject_core::Hash>, String> {
        // Apply coinbase reward
        let miner = block.header.miner;
        let reward = block.coinbase.reward;
        let current_balance = state.get_balance(&miner);
        state.set_balance(&miner, current_balance + reward)
            .map_err(|e| format!("Failed to set miner balance: {}", e))?;

        let mut applied_txs = Vec::new();
        let block_height = block.header.height;

        // Apply regular transactions
        for tx in &block.transactions {
            // Apply the transaction
            match Self::apply_single_transaction(tx, state, timelock_state, escrow_state, channel_state, trustline_state, dimensional_pool_state, marketplace_state, block_height) {
                Ok(()) => {
                    applied_txs.push(tx.hash());
                }
                Err(e) => {
                    warn!(tx_hash = ?tx.hash(), error = %e, "skipping transaction");
                    continue; // Skip this transaction and continue with the rest
                }
            }
        }

        if applied_txs.len() < block.transactions.len() {
            debug!(
                applied = applied_txs.len(),
                total = block.transactions.len(),
                "partial transaction application"
            );
        }

        Ok(applied_txs)
    }

    /// Unwind block transactions (reverse apply_block_transactions)
    /// Used for chain reorganization
    pub(crate) fn unwind_block_transactions(
        block: &coinject_core::Block,
        state: &Arc<AccountState>,
        timelock_state: &Arc<TimeLockState>,
        escrow_state: &Arc<EscrowState>,
        channel_state: &Arc<ChannelState>,
        trustline_state: &Arc<TrustLineState>,
        dimensional_pool_state: &Arc<DimensionalPoolState>,
        marketplace_state: &Arc<MarketplaceState>,
    ) -> Result<(), String> {
        let block_height = block.header.height;

        // Unwind transactions in reverse order
        for tx in block.transactions.iter().rev() {
            if let Err(e) = Self::unwind_single_transaction(tx, state, timelock_state, escrow_state, channel_state, trustline_state, dimensional_pool_state, marketplace_state, block_height) {
                warn!(tx_hash = ?tx.hash(), error = %e, "failed to unwind transaction");
                // Continue unwinding other transactions even if one fails
            }
        }

        // Unwind coinbase reward
        let miner = block.header.miner;
        let reward = block.coinbase.reward;
        let current_balance = state.get_balance(&miner);
        if current_balance >= reward {
            state.set_balance(&miner, current_balance - reward)
                .map_err(|e| format!("Failed to unwind miner reward: {}", e))?;
        } else {
            // Miner balance insufficient - this shouldn't happen but handle gracefully
            warn!(balance = current_balance, reward = reward, "miner balance below reward, setting to zero");
            state.set_balance(&miner, 0)
                .map_err(|e| format!("Failed to set miner balance: {}", e))?;
        }

        Ok(())
    }

    /// Unwind a single transaction (reverse apply_single_transaction)
    fn unwind_single_transaction(
        tx: &coinject_core::Transaction,
        state: &Arc<AccountState>,
        timelock_state: &Arc<TimeLockState>,
        escrow_state: &Arc<EscrowState>,
        channel_state: &Arc<ChannelState>,
        _trustline_state: &Arc<TrustLineState>,
        _dimensional_pool_state: &Arc<DimensionalPoolState>,
        _marketplace_state: &Arc<MarketplaceState>,
        _block_height: u64,
    ) -> Result<(), String> {
        use coinject_core::{EscrowType, ChannelType};
        use coinject_state::EscrowStatus;

        match tx {
            coinject_core::Transaction::Transfer(transfer_tx) => {
                // Reverse: credit sender, debit recipient, decrement nonce
                let sender_balance = state.get_balance(&transfer_tx.from);
                state.set_balance(&transfer_tx.from, sender_balance + transfer_tx.amount + transfer_tx.fee)
                    .map_err(|e| format!("Failed to unwind sender balance: {}", e))?;
                
                let current_nonce = state.get_nonce(&transfer_tx.from);
                if current_nonce > 0 {
                    state.set_nonce(&transfer_tx.from, current_nonce - 1)
                        .map_err(|e| format!("Failed to unwind sender nonce: {}", e))?;
                }

                let recipient_balance = state.get_balance(&transfer_tx.to);
                if recipient_balance >= transfer_tx.amount {
                    state.set_balance(&transfer_tx.to, recipient_balance - transfer_tx.amount)
                        .map_err(|e| format!("Failed to unwind recipient balance: {}", e))?;
                } else {
                    // Recipient balance insufficient - set to 0
                    state.set_balance(&transfer_tx.to, 0)
                        .map_err(|e| format!("Failed to set recipient balance: {}", e))?;
                }

                Ok(())
            }

            coinject_core::Transaction::TimeLock(timelock_tx) => {
                // Reverse: credit sender, remove timelock, decrement nonce
                let sender_balance = state.get_balance(&timelock_tx.from);
                state.set_balance(&timelock_tx.from, sender_balance + timelock_tx.amount + timelock_tx.fee)
                    .map_err(|e| format!("Failed to unwind sender balance: {}", e))?;
                
                let current_nonce = state.get_nonce(&timelock_tx.from);
                if current_nonce > 0 {
                    state.set_nonce(&timelock_tx.from, current_nonce - 1)
                        .map_err(|e| format!("Failed to unwind sender nonce: {}", e))?;
                }

                // Remove timelock if it exists
                let _ = timelock_state.remove_timelock(&tx.hash());
                Ok(())
            }

            coinject_core::Transaction::Escrow(escrow_tx) => {
                match &escrow_tx.escrow_type {
                    EscrowType::Create { .. } => {
                        // Reverse: credit sender, remove escrow, decrement nonce
                        let sender_balance = state.get_balance(&escrow_tx.from);
                        // We need to get the escrow to know the amount
                        if let Some(escrow) = escrow_state.get_escrow(&escrow_tx.escrow_id) {
                            state.set_balance(&escrow_tx.from, sender_balance + escrow.amount + escrow_tx.fee)
                                .map_err(|e| format!("Failed to unwind sender balance: {}", e))?;
                            
                            let current_nonce = state.get_nonce(&escrow_tx.from);
                            if current_nonce > 0 {
                                state.set_nonce(&escrow_tx.from, current_nonce - 1)
                                    .map_err(|e| format!("Failed to unwind sender nonce: {}", e))?;
                            }

                            // Remove escrow - note: perfect reversal requires delete method
                            // For now, we mark it as an approximate reversal
                            warn!("escrow deletion requires delete_escrow method, state may be approximate");
                        }
                        Ok(())
                    }

                    EscrowType::Release => {
                        // Reverse: debit recipient, restore escrow to active
                        if let Some(escrow) = escrow_state.get_escrow(&escrow_tx.escrow_id) {
                            let recipient_balance = state.get_balance(&escrow.recipient);
                            if recipient_balance >= escrow.amount {
                                state.set_balance(&escrow.recipient, recipient_balance - escrow.amount)
                                    .map_err(|e| format!("Failed to unwind recipient balance: {}", e))?;
                            } else {
                                state.set_balance(&escrow.recipient, 0)
                                    .map_err(|e| format!("Failed to set recipient balance: {}", e))?;
                            }

                            // Restore escrow to active
                            escrow_state.update_escrow_status(&escrow_tx.escrow_id, EscrowStatus::Active, None)?;
                        }
                        Ok(())
                    }

                    EscrowType::Refund => {
                        // Reverse: debit sender, restore escrow to active
                        if let Some(escrow) = escrow_state.get_escrow(&escrow_tx.escrow_id) {
                            let sender_balance = state.get_balance(&escrow.sender);
                            if sender_balance >= escrow.amount {
                                state.set_balance(&escrow.sender, sender_balance - escrow.amount)
                                    .map_err(|e| format!("Failed to unwind sender balance: {}", e))?;
                            } else {
                                state.set_balance(&escrow.sender, 0)
                                    .map_err(|e| format!("Failed to set sender balance: {}", e))?;
                            }

                            // Restore escrow to active
                            escrow_state.update_escrow_status(&escrow_tx.escrow_id, EscrowStatus::Active, None)?;
                        }
                        Ok(())
                    }
                }
            }

            coinject_core::Transaction::Channel(channel_tx) => {
                match &channel_tx.channel_type {
                    ChannelType::Open { participant_a, deposit_a, deposit_b, .. } => {
                        // Reverse: credit initiator, remove channel, decrement nonce
                        let initiator_deposit = if &channel_tx.from == participant_a { *deposit_a } else { *deposit_b };
                        let initiator_balance = state.get_balance(&channel_tx.from);
                        state.set_balance(&channel_tx.from, initiator_balance + initiator_deposit + channel_tx.fee)
                            .map_err(|e| format!("Failed to unwind initiator balance: {}", e))?;
                        
                        let current_nonce = state.get_nonce(&channel_tx.from);
                        if current_nonce > 0 {
                            state.set_nonce(&channel_tx.from, current_nonce - 1)
                                .map_err(|e| format!("Failed to unwind initiator nonce: {}", e))?;
                        }

                        // Remove channel - note: perfect reversal requires delete method
                        warn!("channel deletion requires delete_channel method, state may be approximate");
                        Ok(())
                    }

                    ChannelType::Update { .. } => {
                        // Channel updates are state changes, hard to reverse perfectly
                        // For now, just log - in practice, we'd need to track previous state
                        warn!("cannot perfectly reverse channel update, state may be inconsistent");
                        Ok(())
                    }

                    ChannelType::CooperativeClose { final_balance_a, final_balance_b } => {
                        // Reverse: debit both participants, restore channel
                        if let Some(channel) = channel_state.get_channel(&channel_tx.channel_id) {
                            let balance_a = state.get_balance(&channel.participant_a);
                            if balance_a >= *final_balance_a {
                                state.set_balance(&channel.participant_a, balance_a - *final_balance_a)
                                    .map_err(|e| format!("Failed to unwind participant A balance: {}", e))?;
                            } else {
                                state.set_balance(&channel.participant_a, 0)
                                    .map_err(|e| format!("Failed to set participant A balance: {}", e))?;
                            }

                            let balance_b = state.get_balance(&channel.participant_b);
                            if balance_b >= *final_balance_b {
                                state.set_balance(&channel.participant_b, balance_b - *final_balance_b)
                                    .map_err(|e| format!("Failed to unwind participant B balance: {}", e))?;
                            } else {
                                state.set_balance(&channel.participant_b, 0)
                                    .map_err(|e| format!("Failed to set participant B balance: {}", e))?;
                            }

                            // Restore channel to open (approximate - we don't have exact previous state)
                            // This is a limitation - we'd need to store channel history
                            warn!("channel state restoration is approximate");
                        }
                        Ok(())
                    }

                    ChannelType::UnilateralClose { .. } => {
                        // Reverse dispute - restore channel state
                        // This is complex and approximate
                        warn!("cannot perfectly reverse unilateral close, state may be inconsistent");
                        Ok(())
                    }
                }
            }

            coinject_core::Transaction::TrustLine(trustline_tx) => {
                // Reverse: credit fee, decrement nonce, reverse trustline operation
                let sender_balance = state.get_balance(&trustline_tx.from);
                state.set_balance(&trustline_tx.from, sender_balance + trustline_tx.fee)
                    .map_err(|e| format!("Failed to unwind sender balance: {}", e))?;
                
                let current_nonce = state.get_nonce(&trustline_tx.from);
                if current_nonce > 0 {
                    state.set_nonce(&trustline_tx.from, current_nonce - 1)
                        .map_err(|e| format!("Failed to unwind sender nonce: {}", e))?;
                }

                use coinject_core::TrustLineType;
                match &trustline_tx.trustline_type {
                    TrustLineType::Create { .. } => {
                        // Remove trustline - note: perfect reversal requires delete method
                        warn!("trustline deletion requires delete_trustline method, state may be approximate");
                    }
                    TrustLineType::UpdateLimits { .. } | TrustLineType::Freeze | TrustLineType::EvolvePhase { .. } => {
                        // These are state changes - hard to reverse perfectly
                        // In practice, we'd need to store previous state
                        warn!("trustline state reversal is approximate");
                    }
                    TrustLineType::Close => {
                        // Restore trustline - this is complex, would need previous state
                        warn!("cannot perfectly reverse trustline close");
                    }
                }
                Ok(())
            }

            coinject_core::Transaction::DimensionalPoolSwap(pool_swap_tx) => {
                // Reverse: credit fee, decrement nonce, reverse swap
                let sender_balance = state.get_balance(&pool_swap_tx.from);
                state.set_balance(&pool_swap_tx.from, sender_balance + pool_swap_tx.fee)
                    .map_err(|e| format!("Failed to unwind sender balance: {}", e))?;
                
                let current_nonce = state.get_nonce(&pool_swap_tx.from);
                if current_nonce > 0 {
                    state.set_nonce(&pool_swap_tx.from, current_nonce - 1)
                        .map_err(|e| format!("Failed to unwind sender nonce: {}", e))?;
                }

                // Reverse swap - this is complex and may not be perfectly reversible
                // We'd need to track swap history
                warn!("dimensional pool swap reversal is approximate");
                Ok(())
            }

            coinject_core::Transaction::Marketplace(marketplace_tx) => {
                use coinject_core::MarketplaceOperation;
                
                // Reverse: credit fee, decrement nonce
                let sender_balance = state.get_balance(&marketplace_tx.from);
                state.set_balance(&marketplace_tx.from, sender_balance + marketplace_tx.fee)
                    .map_err(|e| format!("Failed to unwind sender balance: {}", e))?;
                
                let current_nonce = state.get_nonce(&marketplace_tx.from);
                if current_nonce > 0 {
                    state.set_nonce(&marketplace_tx.from, current_nonce - 1)
                        .map_err(|e| format!("Failed to unwind sender nonce: {}", e))?;
                }

                match &marketplace_tx.operation {
                    MarketplaceOperation::SubmitProblem { bounty, .. } => {
                        // Reverse: credit bounty back, remove problem
                        state.set_balance(&marketplace_tx.from, sender_balance + marketplace_tx.fee + bounty)
                            .map_err(|e| format!("Failed to unwind problem submission: {}", e))?;
                        // Remove problem - would need problem_id
                        warn!("problem removal requires problem_id tracking");
                    }
                    MarketplaceOperation::SubmitSolution { .. } => {
                        // Reverse: remove solution, potentially reverse auto-payout
                        // This is complex - we'd need to track if bounty was paid
                        warn!("solution reversal is approximate");
                    }
                    MarketplaceOperation::ClaimBounty { .. } => {
                        // Reverse: debit solver, restore bounty to escrow
                        // Would need to track who received the bounty
                        warn!("bounty claim reversal requires tracking");
                    }
                    MarketplaceOperation::CancelProblem { .. } => {
                        // Reverse: debit refund, restore problem
                        // Would need to track refund amount
                        warn!("problem cancellation reversal requires tracking");
                    }
                }
                Ok(())
            }
        }
    }

    /// Apply a single transaction to state
    fn apply_single_transaction(
        tx: &coinject_core::Transaction,
        state: &Arc<AccountState>,
        timelock_state: &Arc<TimeLockState>,
        escrow_state: &Arc<EscrowState>,
        channel_state: &Arc<ChannelState>,
        trustline_state: &Arc<TrustLineState>,
        dimensional_pool_state: &Arc<DimensionalPoolState>,
        marketplace_state: &Arc<MarketplaceState>,
        block_height: u64,
    ) -> Result<(), String> {
        use coinject_core::{EscrowType, ChannelType};
        use coinject_state::{Escrow, EscrowStatus, TimeLock, Channel, ChannelStatus};

        // Pattern match on transaction type to maintain economic mathematics
        match tx {
            coinject_core::Transaction::Transfer(transfer_tx) => {
                // Validate sender has sufficient balance
                let sender_balance = state.get_balance(&transfer_tx.from);
                if sender_balance < transfer_tx.amount + transfer_tx.fee {
                    return Err(format!("Insufficient balance: has {}, needs {}",
                        sender_balance, transfer_tx.amount + transfer_tx.fee));
                }

                // Deduct from sender
                state.set_balance(&transfer_tx.from, sender_balance - transfer_tx.amount - transfer_tx.fee)
                    .map_err(|e| format!("Failed to set sender balance: {}", e))?;
                state.set_nonce(&transfer_tx.from, transfer_tx.nonce + 1)
                    .map_err(|e| format!("Failed to set sender nonce: {}", e))?;

                // Credit recipient
                let recipient_balance = state.get_balance(&transfer_tx.to);
                state.set_balance(&transfer_tx.to, recipient_balance + transfer_tx.amount)
                    .map_err(|e| format!("Failed to set recipient balance: {}", e))?;

                Ok(())
            }

            coinject_core::Transaction::TimeLock(timelock_tx) => {
                // Validate sender has sufficient balance
                let sender_balance = state.get_balance(&timelock_tx.from);
                if sender_balance < timelock_tx.amount + timelock_tx.fee {
                    return Err(format!("Insufficient balance for timelock: has {}, needs {}",
                        sender_balance, timelock_tx.amount + timelock_tx.fee));
                }

                // Deduct from sender
                state.set_balance(&timelock_tx.from, sender_balance - timelock_tx.amount - timelock_tx.fee)
                    .map_err(|e| format!("Failed to set sender balance: {}", e))?;
                state.set_nonce(&timelock_tx.from, timelock_tx.nonce + 1)
                    .map_err(|e| format!("Failed to set sender nonce: {}", e))?;

                // Create timelock entry
                let timelock = TimeLock {
                    tx_hash: tx.hash(),
                    from: timelock_tx.from,
                    recipient: timelock_tx.recipient,
                    amount: timelock_tx.amount,
                    unlock_time: timelock_tx.unlock_time,
                    created_at_height: block_height,
                };

                timelock_state.add_timelock(timelock)?;
                Ok(())
            }

            coinject_core::Transaction::Escrow(escrow_tx) => {
                match &escrow_tx.escrow_type {
                    EscrowType::Create { recipient, arbiter, amount, timeout, conditions_hash } => {
                        // Validate sender has sufficient balance
                        let sender_balance = state.get_balance(&escrow_tx.from);
                        if sender_balance < amount + escrow_tx.fee {
                            return Err(format!("Insufficient balance for escrow: has {}, needs {}",
                                sender_balance, amount + escrow_tx.fee));
                        }

                        // Deduct from sender
                        state.set_balance(&escrow_tx.from, sender_balance - amount - escrow_tx.fee)
                            .map_err(|e| format!("Failed to set sender balance: {}", e))?;
                        state.set_nonce(&escrow_tx.from, escrow_tx.nonce + 1)
                            .map_err(|e| format!("Failed to set sender nonce: {}", e))?;

                        // Create escrow entry
                        let escrow = Escrow {
                            escrow_id: escrow_tx.escrow_id,
                            sender: escrow_tx.from,
                            recipient: *recipient,
                            arbiter: *arbiter,
                            amount: *amount,
                            timeout: *timeout,
                            conditions_hash: *conditions_hash,
                            status: EscrowStatus::Active,
                            created_at_height: block_height,
                            resolved_at_height: None,
                        };

                        escrow_state.create_escrow(escrow)?;
                        Ok(())
                    }

                    EscrowType::Release => {
                        let escrow = escrow_state.get_escrow(&escrow_tx.escrow_id)
                            .ok_or("Escrow not found".to_string())?;

                        if !escrow_state.can_release(&escrow_tx.escrow_id, &escrow_tx.from) {
                            return Err("Not authorized to release escrow".to_string());
                        }

                        // Credit recipient
                        let recipient_balance = state.get_balance(&escrow.recipient);
                        state.set_balance(&escrow.recipient, recipient_balance + escrow.amount)
                            .map_err(|e| format!("Failed to set recipient balance: {}", e))?;

                        // Update escrow status
                        escrow_state.update_escrow_status(&escrow_tx.escrow_id, EscrowStatus::Released, Some(block_height))?;
                        Ok(())
                    }

                    EscrowType::Refund => {
                        let escrow = escrow_state.get_escrow(&escrow_tx.escrow_id)
                            .ok_or("Escrow not found".to_string())?;

                        if !escrow_state.can_refund(&escrow_tx.escrow_id, &escrow_tx.from) {
                            return Err("Not authorized to refund escrow".to_string());
                        }

                        // Credit sender (refund)
                        let sender_balance = state.get_balance(&escrow.sender);
                        state.set_balance(&escrow.sender, sender_balance + escrow.amount)
                            .map_err(|e| format!("Failed to set sender balance: {}", e))?;

                        // Update escrow status
                        escrow_state.update_escrow_status(&escrow_tx.escrow_id, EscrowStatus::Refunded, Some(block_height))?;
                        Ok(())
                    }
                }
            }

            coinject_core::Transaction::Channel(channel_tx) => {
                match &channel_tx.channel_type {
                    ChannelType::Open { participant_a, participant_b, deposit_a, deposit_b, timeout } => {
                        // Validate initiator has sufficient balance for their deposit
                        let initiator_balance = state.get_balance(&channel_tx.from);
                        let initiator_deposit = if &channel_tx.from == participant_a { *deposit_a } else { *deposit_b };

                        if initiator_balance < initiator_deposit + channel_tx.fee {
                            return Err(format!("Insufficient balance for channel: has {}, needs {}",
                                initiator_balance, initiator_deposit + channel_tx.fee));
                        }

                        // Deduct initiator's deposit
                        state.set_balance(&channel_tx.from, initiator_balance - initiator_deposit - channel_tx.fee)
                            .map_err(|e| format!("Failed to set initiator balance: {}", e))?;
                        state.set_nonce(&channel_tx.from, channel_tx.nonce + 1)
                            .map_err(|e| format!("Failed to set initiator nonce: {}", e))?;

                        // Create channel entry
                        let channel = Channel {
                            channel_id: channel_tx.channel_id,
                            participant_a: *participant_a,
                            participant_b: *participant_b,
                            deposit_a: *deposit_a,
                            deposit_b: *deposit_b,
                            balance_a: *deposit_a,
                            balance_b: *deposit_b,
                            sequence: 0,
                            dispute_timeout: *timeout,
                            status: ChannelStatus::Open,
                            opened_at_height: block_height,
                            closed_at_height: None,
                            dispute_started_at: None,
                        };

                        channel_state.open_channel(channel)?;
                        Ok(())
                    }

                    ChannelType::Update { sequence, balance_a, balance_b } => {
                        channel_state.update_channel_state(&channel_tx.channel_id, *sequence, *balance_a, *balance_b)?;
                        Ok(())
                    }

                    ChannelType::CooperativeClose { final_balance_a, final_balance_b } => {
                        let channel = channel_state.get_channel(&channel_tx.channel_id)
                            .ok_or("Channel not found".to_string())?;

                        // Credit both participants
                        let balance_a = state.get_balance(&channel.participant_a);
                        state.set_balance(&channel.participant_a, balance_a + final_balance_a)
                            .map_err(|e| format!("Failed to set participant A balance: {}", e))?;

                        let balance_b = state.get_balance(&channel.participant_b);
                        state.set_balance(&channel.participant_b, balance_b + final_balance_b)
                            .map_err(|e| format!("Failed to set participant B balance: {}", e))?;

                        // Close channel
                        channel_state.close_cooperative(&channel_tx.channel_id, *final_balance_a, *final_balance_b, block_height)?;
                        Ok(())
                    }

                    ChannelType::UnilateralClose { sequence, balance_a, balance_b, .. } => {
                        let _channel = channel_state.get_channel(&channel_tx.channel_id)
                            .ok_or("Channel not found".to_string())?;

                        // Start dispute
                        channel_state.start_dispute(&channel_tx.channel_id, *sequence, *balance_a, *balance_b)?;
                        Ok(())
                    }
                }
            }

            coinject_core::Transaction::TrustLine(trustline_tx) => {
                use coinject_core::TrustLineType;
                use coinject_state::{TrustLine, TrustLineStatus};

                // TrustLine transactions: dimensional economics with exponential decay
                // Validate sender has sufficient balance for fee
                let sender_balance = state.get_balance(&trustline_tx.from);
                if sender_balance < trustline_tx.fee {
                    return Err(format!("Insufficient balance for trustline fee: has {}, needs {}",
                        sender_balance, trustline_tx.fee));
                }

                // Deduct fee from sender and increment nonce
                state.set_balance(&trustline_tx.from, sender_balance - trustline_tx.fee)
                    .map_err(|e| format!("Failed to set sender balance: {}", e))?;
                state.set_nonce(&trustline_tx.from, trustline_tx.nonce + 1)
                    .map_err(|e| format!("Failed to set sender nonce: {}", e))?;

                // Apply trustline state operations based on transaction type
                match &trustline_tx.trustline_type {
                    TrustLineType::Create {
                        account_b,
                        limit_a_to_b,
                        limit_b_to_a,
                        quality_in,
                        quality_out,
                        ripple_enabled,
                        dimensional_scale,
                    } => {
                        // Create new bilateral trustline with dimensional economics
                        let mut trustline = TrustLine {
                            trustline_id: trustline_tx.trustline_id,
                            account_a: trustline_tx.from,
                            account_b: *account_b,
                            limit_a_to_b: *limit_a_to_b,
                            limit_b_to_a: *limit_b_to_a,
                            balance: 0,
                            quality_in: *quality_in,
                            quality_out: *quality_out,
                            ripple_enabled: *ripple_enabled,
                            dimensional_scale: *dimensional_scale,
                            tau: 0.0,
                            viviani_delta: 0.0,
                            status: TrustLineStatus::Active,
                            created_at_height: block_height,
                            modified_at_height: block_height,
                        };

                        // Initialize Viviani oracle metrics
                        trustline.update_viviani_oracle();

                        trustline_state.create_trustline(trustline)
                            .map_err(|e| format!("Failed to create trustline: {}", e))?;

                        Ok(())
                    }

                    TrustLineType::UpdateLimits { limit_a_to_b, limit_b_to_a } => {
                        // Update credit limits on existing trustline
                        let trustline = trustline_state.get_trustline(&trustline_tx.trustline_id)
                            .ok_or_else(|| "TrustLine not found".to_string())?;

                        // Verify sender is authorized (must be account_a or account_b)
                        if !trustline.is_participant(&trustline_tx.from) {
                            return Err("Not authorized to update trustline".to_string());
                        }

                        // Update limits via state manager (handles dimensional recalibration)
                        trustline_state.update_limits(
                            &trustline_tx.trustline_id,
                            *limit_a_to_b,
                            *limit_b_to_a,
                            block_height,
                        )?;

                        Ok(())
                    }

                    TrustLineType::Freeze => {
                        // Freeze trustline (prevents further transfers)
                        let trustline = trustline_state.get_trustline(&trustline_tx.trustline_id)
                            .ok_or_else(|| "TrustLine not found".to_string())?;

                        // Verify sender is authorized
                        if !trustline.is_participant(&trustline_tx.from) {
                            return Err("Not authorized to freeze trustline".to_string());
                        }

                        trustline_state.freeze_trustline(&trustline_tx.trustline_id, block_height)?;
                        Ok(())
                    }

                    TrustLineType::Close => {
                        // Close trustline (requires zero balance)
                        let trustline = trustline_state.get_trustline(&trustline_tx.trustline_id)
                            .ok_or_else(|| "TrustLine not found".to_string())?;

                        // Verify sender is authorized
                        if !trustline.is_participant(&trustline_tx.from) {
                            return Err("Not authorized to close trustline".to_string());
                        }

                        // close_trustline already validates zero balance internally
                        trustline_state.close_trustline(&trustline_tx.trustline_id, block_height)?;
                        Ok(())
                    }

                    TrustLineType::EvolvePhase { delta_tau } => {
                        // Evolve phase parameter: θ(τ) = λτ = τ/√2
                        let trustline = trustline_state.get_trustline(&trustline_tx.trustline_id)
                            .ok_or_else(|| "TrustLine not found".to_string())?;

                        // Verify sender is authorized
                        if !trustline.is_participant(&trustline_tx.from) {
                            return Err("Not authorized to evolve trustline phase".to_string());
                        }

                        // Evolve phase via state manager (handles oracle update)
                        trustline_state.evolve_trustline_phase(
                            &trustline_tx.trustline_id,
                            *delta_tau,
                            block_height,
                        )?;

                        Ok(())
                    }
                }
            }

            coinject_core::Transaction::DimensionalPoolSwap(pool_swap_tx) => {
                // Dimensional pool swap: exponential tokenomics with adaptive ratios
                // Validate sender has sufficient balance for fee
                let sender_balance = state.get_balance(&pool_swap_tx.from);
                if sender_balance < pool_swap_tx.fee {
                    return Err(format!("Insufficient balance for pool swap fee: has {}, needs {}",
                        sender_balance, pool_swap_tx.fee));
                }

                // Deduct fee from sender and increment nonce
                state.set_balance(&pool_swap_tx.from, sender_balance - pool_swap_tx.fee)
                    .map_err(|e| format!("Failed to set sender balance: {}", e))?;
                state.set_nonce(&pool_swap_tx.from, pool_swap_tx.nonce + 1)
                    .map_err(|e| format!("Failed to set sender nonce: {}", e))?;

                // Execute dimensional pool swap with exponential scaling
                let amount_out = dimensional_pool_state.execute_swap(
                    pool_swap_tx.pool_from,
                    pool_swap_tx.pool_to,
                    pool_swap_tx.amount_in,
                    pool_swap_tx.min_amount_out,
                    block_height,
                )?;

                // Record the swap transaction
                dimensional_pool_state.record_swap(
                    tx.hash(),
                    pool_swap_tx.from,
                    pool_swap_tx.pool_from,
                    pool_swap_tx.pool_to,
                    pool_swap_tx.amount_in,
                    amount_out,
                    block_height,
                )?;

                Ok(())
            }

            coinject_core::Transaction::Marketplace(marketplace_tx) => {
                // PoUW Marketplace transaction processing
                use coinject_core::MarketplaceOperation;

                // Validate sender has sufficient balance for fee
                let sender_balance = state.get_balance(&marketplace_tx.from);

                match &marketplace_tx.operation {
                    MarketplaceOperation::SubmitProblem { problem, bounty, min_work_score, expiration_days } => {
                        // Need fee + bounty for escrow
                        let total_needed = marketplace_tx.fee + bounty;
                        if sender_balance < total_needed {
                            return Err(format!("Insufficient balance for problem submission: has {}, needs {}",
                                sender_balance, total_needed));
                        }

                        // Deduct fee + bounty (bounty goes to escrow)
                        state.set_balance(&marketplace_tx.from, sender_balance - total_needed)
                            .map_err(|e| format!("Failed to set sender balance: {}", e))?;

                        // Submit problem to marketplace state
                        let problem_id = marketplace_state.submit_problem(
                            coinject_core::SubmissionMode::Public { problem: problem.clone() },
                            marketplace_tx.from,
                            *bounty,
                            *min_work_score,
                            *expiration_days,
                        ).map_err(|e| format!("Failed to submit problem: {}", e))?;

                        info!(problem_id = ?problem_id, bounty = bounty, "problem submitted to marketplace");
                    }
                    MarketplaceOperation::SubmitSolution { problem_id, solution } => {
                        // AUTONOMOUS BOUNTY PAYOUT
                        // When a valid solution is submitted, automatically claim and payout the bounty
                        // This makes the marketplace truly self-executing - no manual claim needed!

                        // Just need fee
                        if sender_balance < marketplace_tx.fee {
                            return Err(format!("Insufficient balance for marketplace fee: has {}, needs {}",
                                sender_balance, marketplace_tx.fee));
                        }

                        // Deduct fee
                        state.set_balance(&marketplace_tx.from, sender_balance - marketplace_tx.fee)
                            .map_err(|e| format!("Failed to set sender balance: {}", e))?;

                        // Submit solution to marketplace state (verifies and marks as solved)
                        marketplace_state.submit_solution(*problem_id, marketplace_tx.from, solution.clone())
                            .map_err(|e| format!("Failed to submit solution: {}", e))?;

                        // AUTONOMOUS PAYOUT: Immediately claim and release bounty to solver
                        let (solver, bounty) = marketplace_state.claim_bounty(*problem_id)
                            .map_err(|e| format!("Failed to auto-claim bounty: {}", e))?;

                        // Credit bounty to solver atomically in the same block
                        let solver_balance = state.get_balance(&solver);
                        state.set_balance(&solver, solver_balance + bounty)
                            .map_err(|e| format!("Failed to credit bounty to solver: {}", e))?;

                        info!(bounty = bounty, solver = ?solver, "solution accepted, bounty auto-paid");
                    }
                    MarketplaceOperation::ClaimBounty { problem_id } => {
                        // Just need fee
                        if sender_balance < marketplace_tx.fee {
                            return Err(format!("Insufficient balance for marketplace fee: has {}, needs {}",
                                sender_balance, marketplace_tx.fee));
                        }

                        // Deduct fee
                        state.set_balance(&marketplace_tx.from, sender_balance - marketplace_tx.fee)
                            .map_err(|e| format!("Failed to set sender balance: {}", e))?;

                        // Claim bounty from marketplace state
                        let (solver, bounty) = marketplace_state.claim_bounty(*problem_id)
                            .map_err(|e| format!("Failed to claim bounty: {}", e))?;

                        // Credit bounty to solver
                        let solver_balance = state.get_balance(&solver);
                        state.set_balance(&solver, solver_balance + bounty)
                            .map_err(|e| format!("Failed to credit bounty to solver: {}", e))?;

                        info!(bounty = bounty, solver = ?solver, "bounty claimed");
                    }
                    MarketplaceOperation::CancelProblem { problem_id } => {
                        // Just need fee
                        if sender_balance < marketplace_tx.fee {
                            return Err(format!("Insufficient balance for marketplace fee: has {}, needs {}",
                                sender_balance, marketplace_tx.fee));
                        }

                        // Deduct fee
                        state.set_balance(&marketplace_tx.from, sender_balance - marketplace_tx.fee)
                            .map_err(|e| format!("Failed to set sender balance: {}", e))?;

                        // Cancel problem and refund bounty
                        let bounty = marketplace_state.cancel_problem(*problem_id, marketplace_tx.from)
                            .map_err(|e| format!("Failed to cancel problem: {}", e))?;

                        // Refund bounty to submitter
                        let submitter_balance = state.get_balance(&marketplace_tx.from);
                        state.set_balance(&marketplace_tx.from, submitter_balance + bounty)
                            .map_err(|e| format!("Failed to refund bounty to submitter: {}", e))?;

                        info!(bounty = bounty, "problem cancelled, bounty refunded");
                    }
                }

                // Increment nonce
                state.set_nonce(&marketplace_tx.from, marketplace_tx.nonce + 1)
                    .map_err(|e| format!("Failed to set sender nonce: {}", e))?;

                Ok(())
            }
        }
    }
}
