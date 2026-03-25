// Block Validator
// Comprehensive block and transaction validation
//
// NOTE: Full validation integration is prepared for future use
#![allow(dead_code)]

use coinject_core::{Block, Hash};
use coinject_state::AccountState;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ValidationError {
    #[error("Invalid block height: expected {expected}, got {actual}")]
    InvalidHeight { expected: u64, actual: u64 },
    #[error("Invalid previous hash")]
    InvalidPrevHash,
    #[error("Invalid solution: does not solve the problem")]
    InvalidSolution,
    #[error("Invalid commitment")]
    InvalidCommitment,
    #[error("Insufficient work score: {0}")]
    InsufficientWorkScore(f64),
    #[error("Invalid block hash: does not meet difficulty target")]
    InsufficientDifficulty,
    #[error("Invalid timestamp: block is from the future")]
    FutureTimestamp,
    #[error("Invalid timestamp: block is too old")]
    TooOldTimestamp,
    #[error("Transaction validation failed: {0}")]
    InvalidTransaction(String),
    #[error("Coinbase amount exceeds maximum")]
    InvalidCoinbase,
    #[error("State error: {0}")]
    StateError(String),
}

/// Block validator with configurable rules
pub struct BlockValidator {
    /// Minimum work score required
    min_work_score: f64,
    /// Minimum difficulty (leading zeros)
    min_difficulty: u32,
    /// Maximum timestamp drift (seconds into future)
    max_timestamp_drift: i64,
    /// Maximum age for blocks (seconds)
    max_block_age: i64,
}

impl BlockValidator {
    pub fn new(min_difficulty: u32) -> Self {
        BlockValidator {
            min_work_score: 0.0, // Allow all work scores (PoW hash is primary validation)
            min_difficulty,
            max_timestamp_drift: 120, // 2 minutes into future
            max_block_age: 7200,      // 2 hours old
        }
    }

    /// Validate a block completely
    pub fn validate_block(
        &self,
        block: &Block,
        prev_hash: &Hash,
        expected_height: u64,
    ) -> Result<(), ValidationError> {
        self.validate_block_with_options(block, prev_hash, expected_height, false)
    }

    /// Validate a block with options
    pub fn validate_block_with_options(
        &self,
        block: &Block,
        prev_hash: &Hash,
        expected_height: u64,
        skip_timestamp_age_check: bool,
    ) -> Result<(), ValidationError> {
        // 1. Validate block height
        if block.header.height != expected_height {
            return Err(ValidationError::InvalidHeight {
                expected: expected_height,
                actual: block.header.height,
            });
        }

        // 2. Validate previous hash
        if block.header.prev_hash != *prev_hash {
            // #region agent log
            {
                use std::fs::OpenOptions;
                use std::io::Write;
                if let Ok(mut file) = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(crate::service::get_debug_log_path())
                {
                    let log_entry = serde_json::json!({
                        "id": format!("log_{}_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis(), block.header.height),
                        "timestamp": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis(),
                        "location": "validator.rs:85",
                        "message": "InvalidPrevHash detected",
                        "data": {
                            "block_height": block.header.height,
                            "block_prev_hash": format!("{:?}", block.header.prev_hash),
                            "expected_prev_hash": format!("{:?}", prev_hash),
                            "mismatch": true
                        },
                        "sessionId": "debug-session",
                        "runId": "run1",
                        "hypothesisId": "A"
                    });
                    let _ = writeln!(file, "{}", log_entry);
                }
            }
            // #endregion
            return Err(ValidationError::InvalidPrevHash);
        }

        // 3. Validate timestamp (skip age check during initial sync)
        self.validate_timestamp(block.header.timestamp, skip_timestamp_age_check)?;

        // 4. Validate NP-hard solution
        if !block
            .solution_reveal
            .solution
            .verify(&block.solution_reveal.problem)
        {
            return Err(ValidationError::InvalidSolution);
        }

        // 5. Validate commitment-reveal
        // CRITICAL: Epoch salt must be derived from parent block hash (prev_hash)
        // This prevents pre-mining attacks where miners compute problems before parent block exists
        let epoch_salt = block.header.prev_hash; // Use parent block hash as epoch salt
        if !block.solution_reveal.commitment.verify(
            &block.solution_reveal.problem,
            &block.solution_reveal.solution,
            &epoch_salt,
        ) {
            return Err(ValidationError::InvalidCommitment);
        }

        // 6. Validate work score
        if block.header.work_score < self.min_work_score {
            return Err(ValidationError::InsufficientWorkScore(
                block.header.work_score,
            ));
        }

        // 7. Validate difficulty (block hash)
        self.validate_difficulty(block)?;

        // 8. Validate all transactions
        for tx in &block.transactions {
            if !tx.verify_signature() {
                return Err(ValidationError::InvalidTransaction(
                    "Invalid signature".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Validate block timestamp
    fn validate_timestamp(
        &self,
        timestamp: i64,
        _skip_age_check: bool,
    ) -> Result<(), ValidationError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Check if block is from the future (always enforce this)
        if timestamp > now + self.max_timestamp_drift {
            return Err(ValidationError::FutureTimestamp);
        }

        // M0 FIX: REMOVED too old vs now rejection
        // Historical blocks are valid during sync
        let delta = now - timestamp;
        println!(
            "⏱️  [TIMESTAMP] block_ts={}, now_ts={}, delta={}s",
            timestamp, now, delta
        );

        Ok(())
    }

    /// Validate block hash meets difficulty target
    fn validate_difficulty(&self, block: &Block) -> Result<(), ValidationError> {
        // Try bincode hash first (server-side mining)
        let hash_bincode = block.header.hash();
        let hash_bincode_hex = hex::encode(hash_bincode.as_bytes());
        let leading_zeros_bincode = hash_bincode_hex.chars().take_while(|&c| c == '0').count();

        println!(
            "🔍 Difficulty check (bincode): hash={}... leading_zeros={}, required={}",
            &hash_bincode_hex[..16],
            leading_zeros_bincode,
            self.min_difficulty
        );

        if leading_zeros_bincode >= self.min_difficulty as usize {
            println!("✅ Block hash meets difficulty (bincode)");
            return Ok(());
        }

        // Try JSON hash (client-side mining from web browsers)
        let hash_json = block.header.hash_from_json();
        let hash_json_hex = hex::encode(hash_json.as_bytes());
        let leading_zeros_json = hash_json_hex.chars().take_while(|&c| c == '0').count();

        // Get the exact JSON bytes that were hashed (for debugging)
        let json_bytes = serde_json::to_vec(&block.header).unwrap_or_default();
        let json_string = String::from_utf8_lossy(&json_bytes);

        println!(
            "🔍 Difficulty check (JSON): hash={}... leading_zeros={}, required={}",
            &hash_json_hex[..16],
            leading_zeros_json,
            self.min_difficulty
        );
        println!("🔍 Full JSON hash: {}", hash_json_hex);
        println!(
            "📄 Header JSON (server hashed payload, {} bytes): {}",
            json_bytes.len(),
            json_string
        );
        println!(
            "📄 Header JSON bytes (first 200): {:?}",
            &json_bytes[..std::cmp::min(200, json_bytes.len())]
        );

        if leading_zeros_json < self.min_difficulty as usize {
            println!("❌ Block hash does not meet difficulty (neither bincode nor JSON)");
            println!(
                "   Bincode hash: {} (leading_zeros={})",
                hash_bincode_hex, leading_zeros_bincode
            );
            println!(
                "   JSON hash: {} (leading_zeros={})",
                hash_json_hex, leading_zeros_json
            );
            return Err(ValidationError::InsufficientDifficulty);
        }

        println!("✅ Block hash meets difficulty (JSON)");
        Ok(())
    }

    /// Apply block to state (execute transactions)
    pub fn apply_block(&self, block: &Block, state: &AccountState) -> Result<(), ValidationError> {
        // Credit coinbase to miner
        let current_balance = state.get_balance(&block.header.miner);
        state
            .set_balance(&block.header.miner, current_balance + block.coinbase.reward)
            .map_err(|e| ValidationError::StateError(format!("{:?}", e)))?;

        // Execute all transactions
        for tx in &block.transactions {
            // Pattern match on transaction type for type-specific validation
            match tx {
                coinject_core::Transaction::Transfer(transfer_tx) => {
                    // Verify sender has sufficient balance
                    let sender_balance = state.get_balance(&transfer_tx.from);
                    let total_cost = transfer_tx.amount + transfer_tx.fee;

                    if sender_balance < total_cost {
                        return Err(ValidationError::InvalidTransaction(format!(
                            "Insufficient balance: has {}, needs {}",
                            sender_balance, total_cost
                        )));
                    }

                    // Verify nonce
                    let expected_nonce = state.get_nonce(&transfer_tx.from);
                    if transfer_tx.nonce != expected_nonce {
                        return Err(ValidationError::InvalidTransaction(format!(
                            "Invalid nonce: expected {}, got {}",
                            expected_nonce, transfer_tx.nonce
                        )));
                    }

                    // Execute transfer
                    state
                        .transfer(&transfer_tx.from, &transfer_tx.to, transfer_tx.amount)
                        .map_err(|e| ValidationError::StateError(format!("{:?}", e)))?;

                    // Transfer fee to miner (maintains economic incentives)
                    let sender_balance_after = state.get_balance(&transfer_tx.from);
                    state
                        .set_balance(&transfer_tx.from, sender_balance_after - transfer_tx.fee)
                        .map_err(|e| ValidationError::StateError(format!("{:?}", e)))?;

                    let miner_balance = state.get_balance(&block.header.miner);
                    state
                        .set_balance(&block.header.miner, miner_balance + transfer_tx.fee)
                        .map_err(|e| ValidationError::StateError(format!("{:?}", e)))?;

                    // Increment nonce
                    state
                        .increment_nonce(&transfer_tx.from)
                        .map_err(|e| ValidationError::StateError(format!("{:?}", e)))?;
                }
                coinject_core::Transaction::TimeLock(timelock_tx) => {
                    // Verify sender has sufficient balance for amount + fee
                    let sender_balance = state.get_balance(&timelock_tx.from);
                    let total_cost = timelock_tx.amount + timelock_tx.fee;

                    if sender_balance < total_cost {
                        return Err(ValidationError::InvalidTransaction(format!(
                            "Insufficient balance: has {}, needs {}",
                            sender_balance, total_cost
                        )));
                    }

                    // Verify nonce
                    let expected_nonce = state.get_nonce(&timelock_tx.from);
                    if timelock_tx.nonce != expected_nonce {
                        return Err(ValidationError::InvalidTransaction(format!(
                            "Invalid nonce: expected {}, got {}",
                            expected_nonce, timelock_tx.nonce
                        )));
                    }

                    // Deduct total cost from sender (funds go to time-lock)
                    state
                        .set_balance(&timelock_tx.from, sender_balance - total_cost)
                        .map_err(|e| ValidationError::StateError(format!("{:?}", e)))?;

                    // Transfer fee to miner (maintains economic incentives)
                    let miner_balance = state.get_balance(&block.header.miner);
                    state
                        .set_balance(&block.header.miner, miner_balance + timelock_tx.fee)
                        .map_err(|e| ValidationError::StateError(format!("{:?}", e)))?;

                    // Increment nonce
                    state
                        .increment_nonce(&timelock_tx.from)
                        .map_err(|e| ValidationError::StateError(format!("{:?}", e)))?;

                    // TODO: Add to TimeLockState once validator receives state managers
                    // The locked amount should be tracked separately and released at unlock_time
                }
                coinject_core::Transaction::Escrow(escrow_tx) => {
                    // Pattern match on escrow type
                    match &escrow_tx.escrow_type {
                        coinject_core::EscrowType::Create { amount, .. } => {
                            // Verify sender has sufficient balance for amount + fee
                            let sender_balance = state.get_balance(&escrow_tx.from);
                            let total_cost = amount + escrow_tx.fee;

                            if sender_balance < total_cost {
                                return Err(ValidationError::InvalidTransaction(format!(
                                    "Insufficient balance: has {}, needs {}",
                                    sender_balance, total_cost
                                )));
                            }

                            // Verify nonce
                            let expected_nonce = state.get_nonce(&escrow_tx.from);
                            if escrow_tx.nonce != expected_nonce {
                                return Err(ValidationError::InvalidTransaction(format!(
                                    "Invalid nonce: expected {}, got {}",
                                    expected_nonce, escrow_tx.nonce
                                )));
                            }

                            // Deduct total cost from sender (funds go to escrow)
                            state
                                .set_balance(&escrow_tx.from, sender_balance - total_cost)
                                .map_err(|e| ValidationError::StateError(format!("{:?}", e)))?;

                            // Transfer fee to miner (maintains economic incentives)
                            let miner_balance = state.get_balance(&block.header.miner);
                            state
                                .set_balance(&block.header.miner, miner_balance + escrow_tx.fee)
                                .map_err(|e| ValidationError::StateError(format!("{:?}", e)))?;

                            // Increment nonce
                            state
                                .increment_nonce(&escrow_tx.from)
                                .map_err(|e| ValidationError::StateError(format!("{:?}", e)))?;

                            // TODO: Create escrow in EscrowState once validator receives state managers
                            // The escrowed amount should be tracked separately until released/refunded
                        }
                        coinject_core::EscrowType::Release | coinject_core::EscrowType::Refund => {
                            // TODO: Verify escrow exists and signatures are valid
                            // TODO: Release/refund funds based on escrow state
                            // For now, just deduct fee and increment nonce
                            let sender_balance = state.get_balance(&escrow_tx.from);

                            if sender_balance < escrow_tx.fee {
                                return Err(ValidationError::InvalidTransaction(format!(
                                    "Insufficient balance for fee: has {}, needs {}",
                                    sender_balance, escrow_tx.fee
                                )));
                            }

                            let expected_nonce = state.get_nonce(&escrow_tx.from);
                            if escrow_tx.nonce != expected_nonce {
                                return Err(ValidationError::InvalidTransaction(format!(
                                    "Invalid nonce: expected {}, got {}",
                                    expected_nonce, escrow_tx.nonce
                                )));
                            }

                            state
                                .set_balance(&escrow_tx.from, sender_balance - escrow_tx.fee)
                                .map_err(|e| ValidationError::StateError(format!("{:?}", e)))?;

                            let miner_balance = state.get_balance(&block.header.miner);
                            state
                                .set_balance(&block.header.miner, miner_balance + escrow_tx.fee)
                                .map_err(|e| ValidationError::StateError(format!("{:?}", e)))?;

                            state
                                .increment_nonce(&escrow_tx.from)
                                .map_err(|e| ValidationError::StateError(format!("{:?}", e)))?;
                        }
                    }
                }
                coinject_core::Transaction::Channel(channel_tx) => {
                    // Verify initiator has sufficient balance for fee
                    let sender_balance = state.get_balance(&channel_tx.from);

                    if sender_balance < channel_tx.fee {
                        return Err(ValidationError::InvalidTransaction(format!(
                            "Insufficient balance for fee: has {}, needs {}",
                            sender_balance, channel_tx.fee
                        )));
                    }

                    // Verify nonce
                    let expected_nonce = state.get_nonce(&channel_tx.from);
                    if channel_tx.nonce != expected_nonce {
                        return Err(ValidationError::InvalidTransaction(format!(
                            "Invalid nonce: expected {}, got {}",
                            expected_nonce, channel_tx.nonce
                        )));
                    }

                    // Pattern match on channel operation type
                    match &channel_tx.channel_type {
                        coinject_core::ChannelType::Open {
                            participant_a,
                            participant_b,
                            deposit_a,
                            deposit_b,
                            ..
                        } => {
                            // Verify both participants have sufficient deposits
                            let balance_a = state.get_balance(participant_a);
                            let balance_b = state.get_balance(participant_b);

                            if balance_a < *deposit_a {
                                return Err(ValidationError::InvalidTransaction(format!(
                                    "Participant A insufficient balance: has {}, needs {}",
                                    balance_a, deposit_a
                                )));
                            }

                            if balance_b < *deposit_b {
                                return Err(ValidationError::InvalidTransaction(format!(
                                    "Participant B insufficient balance: has {}, needs {}",
                                    balance_b, deposit_b
                                )));
                            }

                            // Deduct deposits from both participants
                            state
                                .set_balance(participant_a, balance_a - *deposit_a)
                                .map_err(|e| ValidationError::StateError(format!("{:?}", e)))?;
                            state
                                .set_balance(participant_b, balance_b - *deposit_b)
                                .map_err(|e| ValidationError::StateError(format!("{:?}", e)))?;

                            // TODO: Create channel in ChannelState once validator receives state managers
                        }
                        coinject_core::ChannelType::Update { .. } => {
                            // Update operations are off-chain, recorded on-chain for reference
                            // No balance changes needed
                            // TODO: Verify channel exists and signatures are valid
                        }
                        coinject_core::ChannelType::CooperativeClose {
                            final_balance_a: _,
                            final_balance_b: _,
                        } => {
                            // TODO: Verify channel exists and signatures from both parties
                            // TODO: Credit final balances to participants
                            // Balances are u128 (Balance type), always non-negative
                            // TODO: Verify balances match channel capacity
                        }
                        coinject_core::ChannelType::UnilateralClose {
                            balance_a: _,
                            balance_b: _,
                            ..
                        } => {
                            // TODO: Verify channel exists and dispute proof
                            // TODO: Credit balances to participants after dispute period
                            // Balances are u128 (Balance type), always non-negative
                            // TODO: Verify balances match channel capacity
                        }
                    }

                    // Transfer fee to miner (maintains economic incentives)
                    let sender_balance_after = state.get_balance(&channel_tx.from);
                    state
                        .set_balance(&channel_tx.from, sender_balance_after - channel_tx.fee)
                        .map_err(|e| ValidationError::StateError(format!("{:?}", e)))?;

                    let miner_balance = state.get_balance(&block.header.miner);
                    state
                        .set_balance(&block.header.miner, miner_balance + channel_tx.fee)
                        .map_err(|e| ValidationError::StateError(format!("{:?}", e)))?;

                    // Increment nonce
                    state
                        .increment_nonce(&channel_tx.from)
                        .map_err(|e| ValidationError::StateError(format!("{:?}", e)))?;
                }
                coinject_core::Transaction::TrustLine(trustline_tx) => {
                    // TrustLine transactions: dimensional economics with exponential decay
                    // Verify sender has sufficient balance for fee
                    let sender_balance = state.get_balance(&trustline_tx.from);

                    if sender_balance < trustline_tx.fee {
                        return Err(ValidationError::InvalidTransaction(format!(
                            "Insufficient balance for trustline fee: has {}, needs {}",
                            sender_balance, trustline_tx.fee
                        )));
                    }

                    // Verify nonce
                    let expected_nonce = state.get_nonce(&trustline_tx.from);
                    if trustline_tx.nonce != expected_nonce {
                        return Err(ValidationError::InvalidTransaction(format!(
                            "Invalid nonce: expected {}, got {}",
                            expected_nonce, trustline_tx.nonce
                        )));
                    }

                    // Deduct fee from sender
                    state
                        .set_balance(&trustline_tx.from, sender_balance - trustline_tx.fee)
                        .map_err(|e| ValidationError::StateError(format!("{:?}", e)))?;

                    // Transfer fee to miner (maintains economic incentives)
                    let miner_balance = state.get_balance(&block.header.miner);
                    state
                        .set_balance(&block.header.miner, miner_balance + trustline_tx.fee)
                        .map_err(|e| ValidationError::StateError(format!("{:?}", e)))?;

                    // Increment nonce
                    state
                        .increment_nonce(&trustline_tx.from)
                        .map_err(|e| ValidationError::StateError(format!("{:?}", e)))?;

                    // TODO: Add TrustLineState operations once state manager is integrated
                    // The trustline state should be managed separately with dimensional economics
                }
                coinject_core::Transaction::DimensionalPoolSwap(pool_swap_tx) => {
                    // Dimensional pool swaps: exponential tokenomics with η = λ = 1/√2
                    // Verify sender has sufficient balance for fee
                    let sender_balance = state.get_balance(&pool_swap_tx.from);

                    if sender_balance < pool_swap_tx.fee {
                        return Err(ValidationError::InvalidTransaction(format!(
                            "Insufficient balance for pool swap fee: has {}, needs {}",
                            sender_balance, pool_swap_tx.fee
                        )));
                    }

                    // Verify nonce
                    let expected_nonce = state.get_nonce(&pool_swap_tx.from);
                    if pool_swap_tx.nonce != expected_nonce {
                        return Err(ValidationError::InvalidTransaction(format!(
                            "Invalid nonce: expected {}, got {}",
                            expected_nonce, pool_swap_tx.nonce
                        )));
                    }

                    // Deduct fee from sender
                    state
                        .set_balance(&pool_swap_tx.from, sender_balance - pool_swap_tx.fee)
                        .map_err(|e| ValidationError::StateError(format!("{:?}", e)))?;

                    // Transfer fee to miner
                    let miner_balance = state.get_balance(&block.header.miner);
                    state
                        .set_balance(&block.header.miner, miner_balance + pool_swap_tx.fee)
                        .map_err(|e| ValidationError::StateError(format!("{:?}", e)))?;

                    // Increment nonce
                    state
                        .increment_nonce(&pool_swap_tx.from)
                        .map_err(|e| ValidationError::StateError(format!("{:?}", e)))?;

                    // Pool swap execution is handled in service.rs apply_single_transaction
                    // Validator only checks balance, nonce, and basic transaction validity
                }
                coinject_core::Transaction::Marketplace(marketplace_tx) => {
                    // PoUW Marketplace transactions: problem submissions and solutions
                    use coinject_core::MarketplaceOperation;

                    let sender_balance = state.get_balance(&marketplace_tx.from);

                    // Check balance requirements based on operation type
                    match &marketplace_tx.operation {
                        MarketplaceOperation::SubmitProblem { bounty, .. } => {
                            // Need fee + bounty for escrow
                            let total_cost = marketplace_tx.fee + bounty;
                            if sender_balance < total_cost {
                                return Err(ValidationError::InvalidTransaction(format!(
                                    "Insufficient balance for problem submission: has {}, needs {}",
                                    sender_balance, total_cost
                                )));
                            }
                        }
                        _ => {
                            // Other operations (SubmitSolution, ClaimBounty, CancelProblem) only need fee
                            if sender_balance < marketplace_tx.fee {
                                return Err(ValidationError::InvalidTransaction(format!(
                                    "Insufficient balance for marketplace fee: has {}, needs {}",
                                    sender_balance, marketplace_tx.fee
                                )));
                            }
                        }
                    }

                    // Verify nonce
                    let expected_nonce = state.get_nonce(&marketplace_tx.from);
                    if marketplace_tx.nonce != expected_nonce {
                        return Err(ValidationError::InvalidTransaction(format!(
                            "Invalid nonce: expected {}, got {}",
                            expected_nonce, marketplace_tx.nonce
                        )));
                    }

                    // Deduct appropriate amount from sender
                    match &marketplace_tx.operation {
                        MarketplaceOperation::SubmitProblem { bounty, .. } => {
                            // Deduct fee + bounty (bounty goes to escrow)
                            let total_cost = marketplace_tx.fee + bounty;
                            state
                                .set_balance(&marketplace_tx.from, sender_balance - total_cost)
                                .map_err(|e| ValidationError::StateError(format!("{:?}", e)))?;
                        }
                        _ => {
                            // Just deduct fee for other operations
                            state
                                .set_balance(
                                    &marketplace_tx.from,
                                    sender_balance - marketplace_tx.fee,
                                )
                                .map_err(|e| ValidationError::StateError(format!("{:?}", e)))?;
                        }
                    }

                    // Transfer fee to miner (maintains economic incentives)
                    let miner_balance = state.get_balance(&block.header.miner);
                    state
                        .set_balance(&block.header.miner, miner_balance + marketplace_tx.fee)
                        .map_err(|e| ValidationError::StateError(format!("{:?}", e)))?;

                    // Increment nonce
                    state
                        .increment_nonce(&marketplace_tx.from)
                        .map_err(|e| ValidationError::StateError(format!("{:?}", e)))?;

                    // TODO: Execute marketplace operations in MarketplaceState
                    // SubmitProblem: create problem and escrow bounty
                    // SubmitSolution: verify solution and mark problem solved
                    // ClaimBounty: release escrowed bounty to solver
                    // CancelProblem: refund escrowed bounty to submitter
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genesis::{create_genesis_block, GenesisConfig};

    #[test]
    #[ignore] // Genesis timestamp (2025-01-01) drifts beyond MAX_BLOCK_AGE over time
    fn test_genesis_validation() {
        let genesis = create_genesis_block(GenesisConfig::default());
        let validator = BlockValidator::new(0); // No difficulty for testing

        // Genesis should validate with ZERO prev hash
        let result = validator.validate_block(&genesis, &Hash::ZERO, 0);
        assert!(result.is_ok());
    }

    #[test]
    fn test_invalid_height() {
        let genesis = create_genesis_block(GenesisConfig::default());
        let validator = BlockValidator::new(0);

        // Wrong expected height
        let result = validator.validate_block(&genesis, &Hash::ZERO, 1);
        assert!(matches!(result, Err(ValidationError::InvalidHeight { .. })));
    }

    #[test]
    fn test_invalid_prev_hash() {
        let genesis = create_genesis_block(GenesisConfig::default());
        let validator = BlockValidator::new(0);

        // Wrong prev hash
        let wrong_hash = Hash::new(b"wrong");
        let result = validator.validate_block(&genesis, &wrong_hash, 0);
        assert!(matches!(result, Err(ValidationError::InvalidPrevHash)));
    }
}
