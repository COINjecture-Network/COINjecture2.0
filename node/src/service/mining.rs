// Mining Loop
// PoUW mining coordination and HuggingFace marketplace upload
#![allow(dead_code)]

use super::*;
use tracing::{trace, debug, info, warn, error};

impl CoinjectNode {
    /// Mining loop
    pub(crate) async fn mining_loop(
        miner: Arc<RwLock<Miner>>,
        chain: Arc<ChainState>,
        state: Arc<AccountState>,
        timelock_state: Arc<TimeLockState>,
        escrow_state: Arc<EscrowState>,
        channel_state: Arc<ChannelState>,
        trustline_state: Arc<TrustLineState>,
        dimensional_pool_state: Arc<DimensionalPoolState>,
        marketplace_state: Arc<MarketplaceState>,
        tx_pool: Arc<RwLock<TransactionPool>>,
        network_tx: mpsc::UnboundedSender<NetworkCommand>,
        cpp_network_tx: mpsc::UnboundedSender<coinject_network::cpp::NetworkCommand>,
        hf_sync: Option<Arc<HuggingFaceSync>>,
        peer_count: Arc<RwLock<usize>>,
        best_known_peer_height: Arc<RwLock<u64>>,
        peer_consensus: Arc<PeerConsensus>,
        dev_mode: bool,
    ) {
        // In dev mode, skip waiting for peers and start mining immediately
        if dev_mode {
            info!("dev mode: starting mining immediately, no peer sync required");
        } else {
            // Wait for peer connections and initial chain sync before mining
            info!("waiting for peer connections and chain sync before mining");
        use coinject_core::ETA;
        let mut sync_wait_interval = time::interval(Duration::from_secs(2));
        let mut sync_attempts = 0;
        // MAX_SYNC_WAIT_ATTEMPTS: Network-derived timeout would be ETA * network_median_sync_time
        // For now, using ETA-scaled value: 150 attempts * 2s = 300s, scaled by ETA ≈ 212s effective
        const MAX_SYNC_WAIT_ATTEMPTS: u32 = (150.0 * ETA) as u32; // ETA-scaled sync timeout
        let mut last_height = 0u64;
        let mut stable_height_count = 0u32;
        // STABLE_HEIGHT_THRESHOLD: Dimensionless count, but could be ETA-scaled
        // 3 checks ensures stability without excessive delay
        const STABLE_HEIGHT_THRESHOLD: u32 = 3; // Height must be stable for 3 checks (6 seconds)
        
        loop {
            sync_wait_interval.tick().await;
            sync_attempts += 1;

            let current_peers = *peer_count.read().await;
            let best_height = chain.best_block_height().await;

            // Check if we have peers
            if current_peers > 0 {
                // Check if height is stable (not actively syncing)
                if best_height == last_height {
                    stable_height_count += 1;
                } else {
                    stable_height_count = 0;
                    last_height = best_height;
                }

                // If we're at genesis with peers, start mining after short wait (20 seconds = 10 attempts)
                if best_height == 0 {
                    if sync_attempts >= 10 {
                        // At genesis with peers - time to bootstrap the network!
                        info!(peer_count = current_peers, "at genesis with peers, starting mining to bootstrap network");
                        break;
                    } else if sync_attempts >= 5 {
                        debug!(peer_count = current_peers, attempt = sync_attempts, "connected at genesis, preparing to mine");
                    }
                } else if stable_height_count >= STABLE_HEIGHT_THRESHOLD {
                    // Height is stable - we're either synced or caught up
                    info!(peer_count = current_peers, block_height = best_height, "chain stable, starting mining");
                    break;
                } else {
                    // Height is changing - actively syncing
                    if sync_attempts % 10 == 0 {
                        debug!(block_height = best_height, attempt = sync_attempts, max_attempts = MAX_SYNC_WAIT_ATTEMPTS, "syncing chain");
                    }
                }
            } else {
                // No peers yet
                if sync_attempts % 5 == 0 {
                    debug!(attempt = sync_attempts, max_attempts = MAX_SYNC_WAIT_ATTEMPTS, peer_count = current_peers, "waiting for peers");
                }
            }

            if sync_attempts >= MAX_SYNC_WAIT_ATTEMPTS {
                warn!(elapsed_secs = sync_attempts * 2, block_height = best_height, "sync wait timeout, starting mining anyway");
                break;
            }
        }
        } // end of else block (non-dev mode peer sync)

        // Start mining loop
        info!("starting mining loop");
        let mut last_mined_height = chain.best_block_height().await;
        debug!(block_height = last_mined_height, "mining loop initialized");

        loop {
            // Use blocking sleep to bypass Tokio timer issues
            trace!("mining loop sleeping 5s");
            use std::io::Write;
            let _ = std::io::stderr().flush();

            // Use spawn_blocking with std::thread::sleep
            tokio::task::spawn_blocking(|| {
                std::thread::sleep(Duration::from_secs(5));
            }).await.unwrap();

            trace!("mining loop woke up after blocking sleep");
            let _ = std::io::stderr().flush();

            trace!("mining loop getting chain state");

            let best_height = chain.best_block_height().await;
            trace!(block_height = best_height, "got best height");
            let best_hash = chain.best_block_hash().await;
            trace!(block_hash = ?best_hash, "got best hash");

            // Check if chain advanced since last mining attempt (block received from peer)
            // v4.7.44 FIX: Don't skip mining entirely - just update last_mined_height and continue
            // to the consensus check. This fixes the race condition where only one node could mine.
            if best_height > last_mined_height {
                debug!(prev_height = last_mined_height, block_height = best_height, "chain advanced, block received from peer");
                last_mined_height = best_height;
                // Note: We continue to consensus check below - this allows ALL nodes to potentially mine
                // The consensus check will properly coordinate who should mine
            }

            // SYNC-BEFORE-MINE: Multi-peer consensus check (XRPL-inspired)
            // Requires 5+ peers with 80% agreement before mining
            // SKIP in dev mode - allow solo mining
            if !dev_mode {
                let (should_mine, reason) = peer_consensus.should_mine(best_height).await;
                if !should_mine {
                    debug!(reason = %reason, "mining paused");

                    // Fallback: Also check simple best-peer height (for bootstrap with <5 peers)
                    let peer_best = *best_known_peer_height.read().await;
                    const SYNC_THRESHOLD: u64 = 10;
                    if peer_best > 0 && best_height + SYNC_THRESHOLD < peer_best {
                        let blocks_behind = peer_best - best_height;
                        debug!(blocks_behind = blocks_behind, our_height = best_height, peer_best = peer_best, "behind best peer");
                    }
                    continue; // Skip mining, check again next interval
                }

                // Log consensus diagnostics
                let diagnostics = peer_consensus.diagnostics().await;
                debug!(diagnostics = %diagnostics, "consensus ok");
            } else {
                debug!("dev mode: skipping peer consensus check");
            }

            // Ready to mine!
            debug!(block_height = best_height + 1, "mining block");

            // Select transactions from pool (top 100 by fee)
            let pool = tx_pool.read().await;
            let pool_size = pool.len();
            let transactions = pool.get_top_n(100);
            drop(pool);

            debug!(pool_size = pool_size, tx_count = transactions.len(), "fetching transactions for block");

            // Mine block
            let mut miner_lock = miner.write().await;
            if let Some(block) = miner_lock
                .mine_block(best_hash, best_height + 1, transactions.clone())
                .await
            {
                info!(block_height = block.header.height, block_hash = ?block.header.hash(), "mined new block");
                drop(miner_lock);

                // Update last mined height to prevent immediate re-mining
                last_mined_height = block.header.height;

                // Store block
                if let Err(e) = chain.store_block(&block).await {
                    error!(block_height = block.header.height, error = %e, "failed to store mined block");
                    continue;
                }

                // RUNTIME INTEGRATION: Calculate and save dimensional consensus state
                // τ = block_height / τ_c (dimensionless time progression)
                use coinject_core::{TAU_C, ConsensusState};
                let tau = (block.header.height as f64) / TAU_C;
                let consensus_state = ConsensusState::at_tau(tau);

                if let Err(e) = dimensional_pool_state.save_consensus_state(block.header.height, &consensus_state) {
                    warn!(block_height = block.header.height, error = %e, "failed to save consensus state");
                } else {
                    debug!(
                        tau = consensus_state.tau,
                        magnitude = consensus_state.magnitude,
                        phase = consensus_state.phase,
                        "consensus state saved"
                    );
                }

                // EMPIRICAL MEASUREMENT: Record work score for convergence analysis
                let block_time = if block.header.height > 1 {
                    // Approximate block time from timestamp difference
                    // In full implementation, track previous block timestamp
                    60.0 // Default to ~60s target block time
                } else {
                    0.0
                };

                if let Err(e) = dimensional_pool_state.record_work_score(
                    block.header.height,
                    consensus_state.tau,
                    block.header.work_score,
                    block_time
                ) {
                    warn!(block_height = block.header.height, error = %e, "failed to record work score");
                }

                // EMPIRICAL MEASUREMENT: Update consensus metrics every 50 blocks (after block 50)
                // This provides more frequent updates to see convergence trajectory
                if block.header.height % 50 == 0 && block.header.height >= 50 {
                    // Use adaptive window: smaller early on, larger later
                    let window_size = if block.header.height < 200 {
                        (block.header.height as usize).min(100)
                    } else {
                        300
                    };

                    match dimensional_pool_state.update_consensus_metrics(block.header.height, window_size) {
                        Ok(metrics) => {
                            info!(
                                block_height = block.header.height,
                                measured_eta = metrics.measured_eta,
                                measured_lambda = metrics.measured_lambda,
                                oracle_delta = metrics.measured_oracle_delta,
                                convergence_r2 = metrics.convergence_confidence,
                                sample_size = metrics.sample_size,
                                "empirical consensus metrics"
                            );

                            if let Some(status) = dimensional_pool_state.test_conjecture() {
                                debug!(
                                    eta_converged = status.eta_convergence,
                                    eta_error = (metrics.measured_eta - 0.707107).abs(),
                                    lambda_converged = status.lambda_convergence,
                                    lambda_error = (metrics.measured_lambda - 0.707107).abs(),
                                    oracle_aligned = status.oracle_alignment,
                                    oracle_delta_error = (metrics.measured_oracle_delta - 0.231).abs(),
                                    "conjecture status"
                                );
                            }
                        },
                        Err(e) => {
                            warn!(block_height = block.header.height, error = %e, "failed to update consensus metrics");
                        }
                    }
                }

                // RUNTIME INTEGRATION: Distribute block reward dynamically across dimensional pools
                let block_reward = block.coinbase.reward;
                if let Err(e) = dimensional_pool_state.distribute_block_reward(block_reward, block.header.height) {
                    warn!(block_height = block.header.height, error = %e, "failed to distribute block reward");
                }

                // RUNTIME INTEGRATION: Execute unlock schedules (every 10 blocks to reduce spam)
                if block.header.height % 10 == 0 {
                    if let Err(e) = dimensional_pool_state.execute_unlock_schedules(block.header.height) {
                        warn!(block_height = block.header.height, error = %e, "failed to execute unlock schedules");
                    }
                }

                // RUNTIME INTEGRATION: Distribute yields (every 10 blocks)
                if block.header.height % 10 == 0 {
                    if let Err(e) = dimensional_pool_state.distribute_yields(block.header.height) {
                        warn!(block_height = block.header.height, error = %e, "failed to distribute yields");
                    }
                }

                // Apply block transactions to state
                let applied_txs = match Self::apply_block_transactions(&block, &state, &timelock_state, &escrow_state, &channel_state, &trustline_state, &dimensional_pool_state, &marketplace_state) {
                    Ok(txs) => txs,
                    Err(e) => {
                        error!(block_height = block.header.height, error = %e, "failed to apply mined block transactions");
                        continue;
                    }
                };

                // Remove only successfully applied transactions from pool
                let mut pool = tx_pool.write().await;
                for tx_hash in &applied_txs {
                    pool.remove(tx_hash);
                }
                drop(pool);

                // Broadcast to network
                if let Err(e) = network_tx.send(NetworkCommand::BroadcastBlock(block.clone())) {
                    error!(block_height = block.header.height, error = %e, "failed to broadcast block");
                } else {
                    debug!(block_height = block.header.height, block_hash = ?block.header.hash(), timestamp = block.header.timestamp, "block broadcast via gossip");
                }

                // Update CPP network chain state so it broadcasts correct height to peers
                if let Err(e) = cpp_network_tx.send(coinject_network::cpp::NetworkCommand::UpdateChainState {
                    best_height: block.header.height,
                    best_hash: block.header.hash(),
                }) {
                    warn!(block_height = block.header.height, error = %e, "failed to update cpp chain state");
                }

                // Push consensus block to Hugging Face (inline within mining loop)
                if let Some(ref hf_sync) = hf_sync {
                    debug!(block_height = block.header.height, "uploading mined block to hugging face");
                    match hf_sync.push_consensus_block(&block, true).await {
                        Ok(()) => debug!(block_height = block.header.height, "block queued for hugging face upload"),
                        Err(e) => error!(block_height = block.header.height, error = %e, "hugging face upload error"),
                    }

                    // Upload marketplace transactions from this mined block
                    Self::upload_marketplace_transactions(&block, &marketplace_state, hf_sync);
                }
            } else {
                warn!(block_height = best_height + 1, "mining failed");
            }
        }
    }

    /// Upload marketplace transactions from a block to Hugging Face
    fn upload_marketplace_transactions(
        block: &coinject_core::Block,
        marketplace_state: &Arc<MarketplaceState>,
        hf_sync: &Arc<HuggingFaceSync>,
    ) {
        use coinject_core::{Transaction, MarketplaceOperation};

        // Scan block for marketplace transactions
        for tx in &block.transactions {
            if let Transaction::Marketplace(marketplace_tx) = tx {
                match &marketplace_tx.operation {
                    MarketplaceOperation::SubmitProblem { problem, .. } => {
                        // Calculate problem_id from problem data (same as marketplace state does)
                        let problem_id = match bincode::serialize(problem) {
                            Ok(problem_data) => coinject_core::Hash::new(&problem_data),
                            Err(e) => {
                                error!(error = %e, "failed to serialize problem for hash");
                                return;
                            }
                        };

                        // Retrieve the submission from marketplace state
                        let marketplace_clone = Arc::clone(marketplace_state);
                        let hf_clone = Arc::clone(hf_sync);
                        let block_height = block.header.height;

                        tokio::spawn(async move {
                            match marketplace_clone.get_problem(&problem_id) {
                                Ok(Some(submission)) => {
                                    debug!(problem_id = ?problem_id, "uploading problem submission to hugging face");
                                    if let Err(e) = hf_clone.push_problem_submission(&submission, block_height).await {
                                        error!(problem_id = ?problem_id, error = %e, "failed to upload problem submission");
                                    } else {
                                        debug!(problem_id = ?problem_id, "problem submission uploaded");
                                    }
                                }
                                Ok(None) => {
                                    warn!(problem_id = ?problem_id, "problem not found in marketplace state");
                                }
                                Err(e) => {
                                    error!(problem_id = ?problem_id, error = %e, "failed to retrieve problem");
                                }
                            }
                        });
                    }
                    MarketplaceOperation::SubmitSolution { problem_id, .. } => {
                        // Retrieve the updated submission (now has solution) from marketplace state
                        let marketplace_clone = Arc::clone(marketplace_state);
                        let hf_clone = Arc::clone(hf_sync);
                        let problem_id = *problem_id;
                        let block_height = block.header.height;

                        tokio::spawn(async move {
                            match marketplace_clone.get_problem(&problem_id) {
                                Ok(Some(submission)) => {
                                    debug!(problem_id = ?problem_id, "uploading solution submission to hugging face");

                                    // For now, use estimated timing (we'll refine this later with actual measurements)
                                    // Estimate based on problem complexity
                                    let solve_time = std::time::Duration::from_secs((submission.min_work_score * 10.0) as u64);
                                    let verify_time = std::time::Duration::from_millis(100);
                                    let solve_memory = 1024 * 1024; // 1 MB estimate
                                    let verify_memory = 512 * 1024; // 512 KB estimate

                                    if let Err(e) = hf_clone.push_solution_submission(
                                        &submission,
                                        block_height,
                                        solve_time,
                                        verify_time,
                                        solve_memory,
                                        verify_memory,
                                    ).await {
                                        error!(problem_id = ?problem_id, error = %e, "failed to upload solution submission");
                                    } else {
                                        debug!(problem_id = ?problem_id, "solution submission uploaded");
                                    }
                                }
                                Ok(None) => {
                                    warn!(problem_id = ?problem_id, "problem not found in marketplace state");
                                }
                                Err(e) => {
                                    error!(problem_id = ?problem_id, error = %e, "failed to retrieve problem");
                                }
                            }
                        });
                    }
                    _ => {
                        // ClaimBounty and CancelProblem don't need uploads
                    }
                }
            }
        }
    }
}
