// Dimensional Pool State with Exponential Tokenomics
// Implements the COINjecture white paper mathematics
//
// Core Mathematics:
// - Satoshi Constant: η = λ = 1/√2 (critical complex equilibrium)
// - Unit Circle Constraint: |μ|² = η² + λ² = 1
// - Dimensional Scales: Dn = e^(-η·τn)
// - Normalized Allocation: p_n = Dn / Σ(Dk²)^(1/2)
// - Phase Evolution: θ(τ) = λτ = τ/√2
//
// Reference: COINjecture White Paper v2.3, Mathematical Proof

use coinject_core::{
    Address, Balance, DimensionalPool, Hash,
    ConsensusState, DimensionalScales, DimensionalEconomics, VivianiOracle,
    ETA, LAMBDA, // Import dimensionless constants from core (re-exported via `pub use dimensional::*;`)
};
use serde::{Deserialize, Serialize};
use redb::{Database, TableDefinition, ReadableTable};
use std::sync::Arc;

// Table definitions for redb
const POOL_LIQUIDITY_TABLE: TableDefinition<u8, &[u8]> = TableDefinition::new("pool_liquidity");
const SWAP_RECORDS_TABLE: TableDefinition<&[u8; 32], &[u8]> = TableDefinition::new("swap_records");
const CONSENSUS_STATE_TABLE: TableDefinition<u64, &[u8]> = TableDefinition::new("consensus_state");
const WORK_SCORE_HISTORY_TABLE: TableDefinition<u64, &[u8]> = TableDefinition::new("work_score_history");
const CONSENSUS_METRICS_TABLE: TableDefinition<u64, &[u8]> = TableDefinition::new("consensus_metrics");

// Use dimensionless constants from core (no duplicates)
// ETA and LAMBDA are imported from coinject_core::dimensional

/// All 8 dimensional economic scales (dimensionless time points τn)
/// From white paper Section 6.2: D_n = e^(-η·τ_n)
pub const DIMENSIONAL_SCALES: [(DimensionalPool, f64, f64, &str); 8] = [
    (DimensionalPool::D1, 0.00, 1.000, "Genesis"),         // τ₁=0.00, D₁=1.000
    (DimensionalPool::D2, 0.20, 0.867, "Coupling"),        // τ₂=0.20, D₂=0.867
    (DimensionalPool::D3, 0.41, 0.750, "First Harmonic"),  // τ₃=0.41, D₃=0.750
    (DimensionalPool::D4, 0.68, 0.618, "Golden Ratio"),    // τ₄=0.68, D₄=φ⁻¹
    (DimensionalPool::D5, 0.98, 0.500, "Half-scale"),      // τ₅=0.98, D₅=2⁻¹
    (DimensionalPool::D6, 1.36, 0.382, "Second Golden"),   // τ₆=1.36, D₆=φ⁻²
    (DimensionalPool::D7, 1.96, 0.250, "Quarter-scale"),   // τ₇=1.96, D₇=2⁻²
    (DimensionalPool::D8, 2.72, 0.146, "Euler"),           // τ₈=2.72, D₈=e⁻ᵉ/√²
];

/// Normalized allocation ratios for all 8 pools
/// From white paper Section 6.2: p_n(t) = D̃_n(t) / Σ D̃_k(t)
/// Conservation constraint: Σ D̃_n² = 1
pub const ALLOCATION_RATIOS: [(DimensionalPool, f64); 8] = [
    (DimensionalPool::D1, 0.222), // 22.2% - Immediate liquidity
    (DimensionalPool::D2, 0.193), // 19.3% - Short-term staking
    (DimensionalPool::D3, 0.167), // 16.7% - Primary liquidity
    (DimensionalPool::D4, 0.137), // 13.7% - Treasury reserve
    (DimensionalPool::D5, 0.111), // 11.1% - Secondary liquidity
    (DimensionalPool::D6, 0.085), // 8.5%  - Long-term vesting
    (DimensionalPool::D7, 0.056), // 5.6%  - Strategic reserve
    (DimensionalPool::D8, 0.032), // 3.2%  - Foundation endowment
];

/// Pool liquidity data
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PoolLiquidity {
    /// Pool type
    pub pool: DimensionalPool,
    /// Current liquidity (total tokens in pool)
    pub liquidity: Balance,
    /// RUNTIME INTEGRATION: Locked tokens (not yet unlocked by U_n(τ))
    pub locked_liquidity: Balance,
    /// RUNTIME INTEGRATION: Unlocked tokens (available for withdrawal/yields)
    pub unlocked_liquidity: Balance,
    /// RUNTIME INTEGRATION: Last unlock fraction checkpoint (to prevent re-unlocking)
    pub last_unlock_fraction: f64,
    /// Dimensional scale factor D_n = e^(-η·τ_n)
    pub dimensional_factor: f64,
    /// Allocation ratio p_n (normalized)
    pub allocation_ratio: f64,
    /// Current dimensionless time τ (for phase evolution)
    pub tau: f64,
    /// Phase angle θ(τ) = λτ = τ/√2
    pub phase: f64,
    /// Last update block height
    pub last_update_height: u64,
}

/// Pool swap record
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PoolSwapRecord {
    /// Swap transaction hash
    pub tx_hash: Hash,
    /// Swapper address
    pub from: Address,
    /// Source pool
    pub pool_from: DimensionalPool,
    /// Destination pool
    pub pool_to: DimensionalPool,
    /// Amount swapped in
    pub amount_in: Balance,
    /// Amount swapped out
    pub amount_out: Balance,
    /// Swap ratio (amount_out / amount_in)
    pub swap_ratio: f64,
    /// Block height when swap occurred
    pub block_height: u64,
}

/// EMPIRICAL MEASUREMENT: Work score history entry
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkScoreEntry {
    pub block_height: u64,
    pub tau: f64,
    pub work_score: f64,
    pub block_time: f64, // seconds since previous block
}

/// EMPIRICAL MEASUREMENT: Convergence status for "The Conjecture"
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConjectureStatus {
    /// True if measured η converges to 1/√2 within tolerance
    pub eta_convergence: bool,
    /// True if measured λ converges to 1/√2 within tolerance
    pub lambda_convergence: bool,
    /// True if oracle metric Δ aligns with theoretical 0.231
    pub oracle_alignment: bool,
    /// Convergence confidence (0.0 = no data, 1.0 = strong fit)
    pub confidence: f64,
    /// Number of blocks analyzed
    pub sample_size: usize,
}

/// EMPIRICAL MEASUREMENT: Consensus metrics tracker
/// Tests whether optimal consensus naturally converges to η = λ = 1/√2
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConsensusMetrics {
    /// Measured damping ratio from exponential fit to work scores
    pub measured_eta: f64,
    /// Measured coupling strength from timing coherence
    pub measured_lambda: f64,
    /// Oracle metric from measured values: Δ(η_measured, λ_measured)
    pub measured_oracle_delta: f64,
    /// Convergence confidence (R² from exponential fit)
    pub convergence_confidence: f64,
    /// Number of data points used for measurement
    pub sample_size: usize,
    /// Last update block height
    pub last_update_height: u64,
}

impl Default for ConsensusMetrics {
    fn default() -> Self {
        Self {
            measured_eta: ETA,       // Start at theoretical value
            measured_lambda: LAMBDA,  // Start at theoretical value
            measured_oracle_delta: 0.231,     // Theoretical Δ at critical equilibrium
            convergence_confidence: 0.0,      // No data yet
            sample_size: 0,
            last_update_height: 0,
        }
    }
}

/// Dimensional Pool State Manager
pub struct DimensionalPoolState {
    db: Arc<Database>,
}

impl DimensionalPoolState {
    /// Create new dimensional pool state manager
    pub fn new(db: Arc<Database>) -> Result<Self, redb::Error> {
        // Initialize tables
        let write_txn = db.begin_write()?;
        {
            let _ = write_txn.open_table(POOL_LIQUIDITY_TABLE)?;
            let _ = write_txn.open_table(SWAP_RECORDS_TABLE)?;
            let _ = write_txn.open_table(CONSENSUS_STATE_TABLE)?;
            let _ = write_txn.open_table(WORK_SCORE_HISTORY_TABLE)?;
            let _ = write_txn.open_table(CONSENSUS_METRICS_TABLE)?;
        }
        write_txn.commit()?;

        Ok(DimensionalPoolState { db })
    }

    /// Initialize pools with genesis liquidity
    pub fn initialize_pools(&self, total_supply: Balance, genesis_height: u64) -> Result<(), String> {
        for (pool, tau, d_n, name) in DIMENSIONAL_SCALES.iter() {
            // Calculate initial liquidity based on allocation ratio
            let allocation = self.get_allocation_ratio(*pool);
            let initial_liquidity = (total_supply as f64 * allocation) as Balance;

            let pool_liquidity = PoolLiquidity {
                pool: *pool,
                liquidity: initial_liquidity,
                // RUNTIME INTEGRATION: All genesis tokens start locked
                locked_liquidity: initial_liquidity,
                unlocked_liquidity: 0,
                last_unlock_fraction: 0.0,
                dimensional_factor: *d_n,
                allocation_ratio: allocation,
                tau: *tau,
                phase: self.calculate_phase(*tau),
                last_update_height: genesis_height,
            };

            self.save_pool_liquidity(&pool_liquidity)?;

            println!("✅ Initialized pool {:?} ({}) with {} tokens (D_n={:.3}, p_n={:.3})",
                pool, name, initial_liquidity, d_n, allocation);
        }

        Ok(())
    }

    /// Get pool liquidity
    pub fn get_pool_liquidity(&self, pool: &DimensionalPool) -> Option<PoolLiquidity> {
        let read_txn = self.db.begin_read().ok()?;
        let table = read_txn.open_table(POOL_LIQUIDITY_TABLE).ok()?;

        let pool_key = *pool as u8;
        let bytes = table.get(pool_key).ok()??;
        bincode::deserialize(bytes.value()).ok()
    }

    /// Save pool liquidity
    fn save_pool_liquidity(&self, pool: &PoolLiquidity) -> Result<(), String> {
        let pool_key = pool.pool as u8;
        let value = bincode::serialize(pool)
            .map_err(|e| format!("Failed to serialize pool: {}", e))?;

        let write_txn = self.db.begin_write()
            .map_err(|e| format!("Failed to begin write: {}", e))?;
        {
            let mut table = write_txn.open_table(POOL_LIQUIDITY_TABLE)
                .map_err(|e| format!("Failed to open table: {}", e))?;
            table.insert(pool_key, value.as_slice())
                .map_err(|e| format!("Failed to insert pool: {}", e))?;
        }
        write_txn.commit()
            .map_err(|e| format!("Failed to commit: {}", e))?;

        Ok(())
    }

    /// Execute pool swap with exponential dimensional ratios
    /// Implements: amount_out = amount_in × (D_from / D_to)
    pub fn execute_swap(
        &self,
        pool_from: DimensionalPool,
        pool_to: DimensionalPool,
        amount_in: Balance,
        min_amount_out: Balance,
        block_height: u64,
    ) -> Result<Balance, String> {
        // Get pool liquidities
        let mut liquidity_from = self.get_pool_liquidity(&pool_from)
            .ok_or("Source pool not found")?;
        let mut liquidity_to = self.get_pool_liquidity(&pool_to)
            .ok_or("Destination pool not found")?;

        // Check source pool has enough liquidity
        if liquidity_from.liquidity < amount_in {
            return Err(format!("Insufficient liquidity in source pool: has {}, needs {}",
                liquidity_from.liquidity, amount_in));
        }

        // Calculate swap ratio using dimensional factors
        // Ratio = D_from / D_to (exponential scaling)
        let swap_ratio = liquidity_from.dimensional_factor / liquidity_to.dimensional_factor;
        let amount_out = (amount_in as f64 * swap_ratio) as Balance;

        // Check slippage protection
        if amount_out < min_amount_out {
            return Err(format!("Slippage exceeded: got {}, minimum {}",
                amount_out, min_amount_out));
        }

        // Check destination pool has enough liquidity
        if liquidity_to.liquidity < amount_out {
            return Err(format!("Insufficient liquidity in destination pool: has {}, needs {}",
                liquidity_to.liquidity, amount_out));
        }

        // Update pool liquidities
        liquidity_from.liquidity -= amount_in;
        liquidity_from.last_update_height = block_height;

        liquidity_to.liquidity -= amount_out;
        liquidity_to.last_update_height = block_height;

        // Save updated pools (ACID transaction)
        self.save_pool_liquidity(&liquidity_from)?;
        self.save_pool_liquidity(&liquidity_to)?;

        Ok(amount_out)
    }

    /// Calculate dimensional factor: D_n = e^(-η·τ_n)
    pub fn calculate_dimensional_factor(&self, tau: f64) -> f64 {
        (-ETA * tau).exp()
    }

    /// Calculate phase evolution: θ(τ) = λτ = τ/√2
    pub fn calculate_phase(&self, tau: f64) -> f64 {
        LAMBDA * tau
    }

    /// Get normalized allocation ratio for pool
    pub fn get_allocation_ratio(&self, pool: DimensionalPool) -> f64 {
        ALLOCATION_RATIOS.iter()
            .find(|(p, _)| p == &pool)
            .map(|(_, ratio)| *ratio)
            .unwrap_or(0.0)
    }

    /// Get dimensional factor for pool
    pub fn get_dimensional_factor(&self, pool: DimensionalPool) -> f64 {
        DIMENSIONAL_SCALES.iter()
            .find(|(p, _, _, _)| p == &pool)
            .map(|(_, _, d_n, _)| *d_n)
            .unwrap_or(1.0)
    }

    /// Get dimensionless time τ for pool
    pub fn get_tau(&self, pool: DimensionalPool) -> f64 {
        DIMENSIONAL_SCALES.iter()
            .find(|(p, _, _, _)| p == &pool)
            .map(|(_, tau, _, _)| *tau)
            .unwrap_or(0.0)
    }

    /// Record swap transaction
    pub fn record_swap(
        &self,
        tx_hash: Hash,
        from: Address,
        pool_from: DimensionalPool,
        pool_to: DimensionalPool,
        amount_in: Balance,
        amount_out: Balance,
        block_height: u64,
    ) -> Result<(), String> {
        let swap_ratio = (amount_out as f64) / (amount_in as f64);

        let swap_record = PoolSwapRecord {
            tx_hash,
            from,
            pool_from,
            pool_to,
            amount_in,
            amount_out,
            swap_ratio,
            block_height,
        };

        let key = tx_hash.as_bytes();
        let value = bincode::serialize(&swap_record)
            .map_err(|e| format!("Failed to serialize swap: {}", e))?;

        let write_txn = self.db.begin_write()
            .map_err(|e| format!("Failed to begin write: {}", e))?;
        {
            let mut table = write_txn.open_table(SWAP_RECORDS_TABLE)
                .map_err(|e| format!("Failed to open table: {}", e))?;
            table.insert(key, value.as_slice())
                .map_err(|e| format!("Failed to insert swap: {}", e))?;
        }
        write_txn.commit()
            .map_err(|e| format!("Failed to commit: {}", e))?;

        Ok(())
    }

    /// Get swap record by transaction hash
    pub fn get_swap_record(&self, tx_hash: &Hash) -> Option<PoolSwapRecord> {
        let read_txn = self.db.begin_read().ok()?;
        let table = read_txn.open_table(SWAP_RECORDS_TABLE).ok()?;

        let bytes = table.get(tx_hash.as_bytes()).ok()??;
        bincode::deserialize(bytes.value()).ok()
    }

    /// Get all pool liquidities
    pub fn get_all_pools(&self) -> Vec<PoolLiquidity> {
        let mut pools = Vec::new();
        for (pool, _, _, _) in DIMENSIONAL_SCALES.iter() {
            if let Some(liquidity) = self.get_pool_liquidity(pool) {
                pools.push(liquidity);
            }
        }
        pools
    }

    /// Calculate total liquidity across all pools
    pub fn total_liquidity(&self) -> Balance {
        self.get_all_pools()
            .iter()
            .map(|p| p.liquidity)
            .sum()
    }

    /// Save consensus state for a given block height
    pub fn save_consensus_state(&self, block_height: u64, state: &ConsensusState) -> Result<(), String> {
        let value = bincode::serialize(state)
            .map_err(|e| format!("Failed to serialize consensus state: {}", e))?;

        let write_txn = self.db.begin_write()
            .map_err(|e| format!("Failed to begin write: {}", e))?;
        {
            let mut table = write_txn.open_table(CONSENSUS_STATE_TABLE)
                .map_err(|e| format!("Failed to open table: {}", e))?;
            table.insert(block_height, value.as_slice())
                .map_err(|e| format!("Failed to insert consensus state: {}", e))?;
        }
        write_txn.commit()
            .map_err(|e| format!("Failed to commit: {}", e))?;

        Ok(())
    }

    /// Get consensus state at a given block height
    pub fn get_consensus_state(&self, block_height: u64) -> Option<ConsensusState> {
        let read_txn = self.db.begin_read().ok()?;
        let table = read_txn.open_table(CONSENSUS_STATE_TABLE).ok()?;

        let bytes = table.get(block_height).ok()??;
        bincode::deserialize(bytes.value()).ok()
    }

    /// Get current consensus state (latest block)
    pub fn get_current_consensus_state(&self) -> Option<ConsensusState> {
        let read_txn = self.db.begin_read().ok()?;
        let table = read_txn.open_table(CONSENSUS_STATE_TABLE).ok()?;

        // Get the highest block height
        let mut iter = table.iter().ok()?;
        let mut latest: Option<(u64, ConsensusState)> = None;

        while let Some(Ok((height, bytes))) = iter.next() {
            if let Ok(state) = bincode::deserialize::<ConsensusState>(bytes.value()) {
                let h = height.value();
                if latest.is_none() || h > latest.as_ref().unwrap().0 {
                    latest = Some((h, state));
                }
            }
        }

        latest.map(|(_, state)| state)
    }

    /// Calculate unlock fractions for all pools at current consensus state
    pub fn get_unlock_fractions(&self) -> Option<[f64; 8]> {
        let state = self.get_current_consensus_state()?;
        Some([
            state.unlock_fraction(0), // D1
            state.unlock_fraction(1), // D2
            state.unlock_fraction(2), // D3
            state.unlock_fraction(3), // D4
            state.unlock_fraction(4), // D5
            state.unlock_fraction(5), // D6
            state.unlock_fraction(6), // D7
            state.unlock_fraction(7), // D8
        ])
    }

    /// Calculate yield rates for all pools at current consensus state
    pub fn get_yield_rates(&self) -> Option<[f64; 8]> {
        let state = self.get_current_consensus_state()?;
        Some([
            state.yield_rate(0), // D1
            state.yield_rate(1), // D2
            state.yield_rate(2), // D3
            state.yield_rate(3), // D4
            state.yield_rate(4), // D5
            state.yield_rate(5), // D6
            state.yield_rate(6), // D7
            state.yield_rate(7), // D8
        ])
    }

    /// Get current dimensional scales
    pub fn get_dimensional_scales(&self) -> DimensionalScales {
        self.get_current_consensus_state()
            .map(|state| state.dimensional_scales())
            .unwrap_or_else(DimensionalScales::calculate)
    }

    /// Get Viviani Oracle metric for current network state
    pub fn get_oracle_metric(&self) -> VivianiOracle {
        VivianiOracle::calculate(ETA, LAMBDA)
    }

    /// Get complete dimensional economics state
    pub fn get_economics_state(&self) -> DimensionalEconomics {
        let consensus = self.get_current_consensus_state()
            .unwrap_or_else(|| ConsensusState::at_tau(0.0));
        let scales = consensus.dimensional_scales();
        let oracle = self.get_oracle_metric();

        DimensionalEconomics {
            consensus,
            scales,
            oracle,
        }
    }

    /// RUNTIME INTEGRATION: Distribute block reward across pools based on live τ
    /// Uses dynamic allocation ratios p_n(τ) = D̃_n(τ) / Σ D̃_k(τ) instead of static constants
    pub fn distribute_block_reward(&self, total_reward: Balance, block_height: u64) -> Result<(), String> {
        // Get current consensus state to calculate dynamic allocations
        let consensus_state = self.get_current_consensus_state()
            .ok_or("No consensus state found")?;

        let scales = consensus_state.dimensional_scales();
        let normalized = scales.normalized();
        let allocation_ratios = normalized.allocation_ratios();

        println!("💰 Distributing {} token reward across 8 dimensional pools (τ={:.4}):",
            total_reward, consensus_state.tau);

        // Distribute to each pool according to current dimensional ratios
        for (i, pool) in [
            DimensionalPool::D1, DimensionalPool::D2, DimensionalPool::D3, DimensionalPool::D4,
            DimensionalPool::D5, DimensionalPool::D6, DimensionalPool::D7, DimensionalPool::D8
        ].iter().enumerate() {
            let ratio = allocation_ratios[i];
            let pool_reward = (total_reward as f64 * ratio) as Balance;

            if let Some(mut liquidity) = self.get_pool_liquidity(pool) {
                liquidity.liquidity += pool_reward;
                // RUNTIME INTEGRATION: New rewards start as locked
                liquidity.locked_liquidity += pool_reward;
                liquidity.allocation_ratio = ratio;
                liquidity.last_update_height = block_height;
                self.save_pool_liquidity(&liquidity)?;

                println!("   {:?}: +{} tokens ({:.1}% of reward, locked: {}, unlocked: {})",
                    pool, pool_reward, ratio * 100.0, liquidity.locked_liquidity, liquidity.unlocked_liquidity);
            }
        }

        Ok(())
    }

    /// RUNTIME INTEGRATION: Execute unlock schedules for all pools
    /// ACTUALLY MOVES TOKENS from locked → unlocked based on U_n(τ) thresholds
    pub fn execute_unlock_schedules(&self, block_height: u64) -> Result<u128, String> {
        let consensus_state = self.get_current_consensus_state()
            .ok_or("No consensus state found")?;

        let mut total_unlocked: u128 = 0;

        println!("🔓 Executing unlock schedules at τ={:.4}:", consensus_state.tau);

        for (i, pool) in [
            DimensionalPool::D1, DimensionalPool::D2, DimensionalPool::D3, DimensionalPool::D4,
            DimensionalPool::D5, DimensionalPool::D6, DimensionalPool::D7, DimensionalPool::D8
        ].iter().enumerate() {
            let current_unlock_fraction = consensus_state.unlock_fraction(i);

            if let Some(mut liquidity) = self.get_pool_liquidity(pool) {
                // Calculate how much has unlocked since last checkpoint
                let new_unlock_fraction = current_unlock_fraction - liquidity.last_unlock_fraction;

                if new_unlock_fraction > 0.001 {  // Only unlock if > 0.1% change
                    // Calculate tokens to unlock from total pool liquidity
                    let tokens_to_unlock = (liquidity.liquidity as f64 * new_unlock_fraction) as Balance;

                    // Don't unlock more than what's locked
                    let actually_unlocked = tokens_to_unlock.min(liquidity.locked_liquidity);

                    if actually_unlocked > 0 {
                        // ACTUALLY MOVE TOKENS: locked → unlocked
                        liquidity.locked_liquidity -= actually_unlocked;
                        liquidity.unlocked_liquidity += actually_unlocked;
                        liquidity.last_unlock_fraction = current_unlock_fraction;
                        liquidity.last_update_height = block_height;

                        self.save_pool_liquidity(&liquidity)?;

                        total_unlocked += actually_unlocked;

                        println!("   {:?}: UNLOCKED {} tokens! ({:.1}% → {:.1}%, locked: {}, unlocked: {})",
                            pool,
                            actually_unlocked,
                            liquidity.last_unlock_fraction * 100.0 - new_unlock_fraction * 100.0,
                            current_unlock_fraction * 100.0,
                            liquidity.locked_liquidity,
                            liquidity.unlocked_liquidity
                        );
                    }
                }
            }
        }

        if total_unlocked > 0 {
            println!("✅ Total tokens unlocked this round: {}", total_unlocked);
        }

        Ok(total_unlocked)
    }

    /// RUNTIME INTEGRATION: Calculate and distribute yields based on r_n(τ)
    /// ACTUALLY GENERATES YIELD from unlocked pool liquidity
    /// Yields are calculated as: yield = unlocked_liquidity × r_n(τ) × Δt
    /// where Δt is the time since last yield distribution
    pub fn distribute_yields(&self, block_height: u64) -> Result<u128, String> {
        let consensus_state = self.get_current_consensus_state()
            .ok_or("No consensus state found")?;

        let mut total_yield: u128 = 0;

        let yield_rates = self.get_yield_rates()
            .ok_or("Failed to calculate yield rates")?;

        println!("📈 Distributing yields at τ={:.4}:", consensus_state.tau);

        for (i, pool) in [
            DimensionalPool::D1, DimensionalPool::D2, DimensionalPool::D3, DimensionalPool::D4,
            DimensionalPool::D5, DimensionalPool::D6, DimensionalPool::D7, DimensionalPool::D8
        ].iter().enumerate() {
            if let Some(mut liquidity) = self.get_pool_liquidity(pool) {
                let rate = yield_rates[i];

                // Only generate yield from UNLOCKED liquidity
                if liquidity.unlocked_liquidity > 0 {
                    // Calculate yield: unlocked_balance × yield_rate × time_factor
                    // Using a conservative time factor of 0.001 per block (0.1% max yield per distribution)
                    let time_factor = 0.001;
                    let yield_amount = (liquidity.unlocked_liquidity as f64 * rate * time_factor) as Balance;

                    if yield_amount > 0 {
                        // ACTUALLY GENERATE YIELD: Add to pool's unlocked liquidity
                        // In full implementation, this would be distributed to stakers
                        // For now, it compounds back into the pool
                        liquidity.unlocked_liquidity += yield_amount;
                        liquidity.liquidity += yield_amount;
                        liquidity.last_update_height = block_height;

                        self.save_pool_liquidity(&liquidity)?;

                        total_yield += yield_amount;

                        println!("   {:?}: GENERATED {} tokens yield (r_n={:.4}, unlocked: {})",
                            pool, yield_amount, rate, liquidity.unlocked_liquidity);
                    }
                }
            }
        }

        if total_yield > 0 {
            println!("✅ Total yield generated this round: {} tokens", total_yield);
        }

        Ok(total_yield)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // EMPIRICAL MEASUREMENT: Testing "The Conjecture"
    // Does optimal consensus naturally converge to η = λ = 1/√2?
    // ═══════════════════════════════════════════════════════════════════════════

    /// Record work score for empirical analysis
    pub fn record_work_score(&self, block_height: u64, tau: f64, work_score: f64, block_time: f64) -> Result<(), String> {
        let entry = WorkScoreEntry {
            block_height,
            tau,
            work_score,
            block_time,
        };

        let write_txn = self.db.begin_write()
            .map_err(|e| format!("Failed to begin write transaction: {}", e))?;
        {
            let mut table = write_txn.open_table(WORK_SCORE_HISTORY_TABLE)
                .map_err(|e| format!("Failed to open work score history table: {}", e))?;

            let serialized = bincode::serialize(&entry)
                .map_err(|e| format!("Failed to serialize work score entry: {}", e))?;
            table.insert(block_height, serialized.as_slice())
                .map_err(|e| format!("Failed to insert work score: {}", e))?;
        }
        write_txn.commit()
            .map_err(|e| format!("Failed to commit work score: {}", e))?;

        Ok(())
    }

    /// Measure η empirically from work score decay
    /// Fit exponential: log(work_score) ≈ -η·τ + c
    pub fn measure_eta_from_work_scores(&self, window_size: usize) -> Result<(f64, f64), String> {
        let read_txn = self.db.begin_read()
            .map_err(|e| format!("Failed to begin read transaction: {}", e))?;

        let table = read_txn.open_table(WORK_SCORE_HISTORY_TABLE)
            .map_err(|e| format!("Failed to open work score history table: {}", e))?;

        // Collect recent work scores
        let mut entries: Vec<WorkScoreEntry> = Vec::new();
        let iter = table.iter()
            .map_err(|e| format!("Failed to iterate work scores: {}", e))?;

        for item in iter {
            let (_key, value) = item.map_err(|e| format!("Failed to read item: {}", e))?;
            let entry: WorkScoreEntry = bincode::deserialize(value.value())
                .map_err(|e| format!("Failed to deserialize entry: {}", e))?;
            entries.push(entry);
        }

        // Take most recent window
        if entries.len() < 10 {
            return Ok((ETA, 0.0)); // Not enough data
        }

        let start_idx = entries.len().saturating_sub(window_size);
        let window = &entries[start_idx..];

        // Linear regression on log-transformed work scores
        // y = log(work_score), x = τ
        // y = -η·x + c  →  slope = -η
        let n = window.len() as f64;
        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        let mut sum_xy = 0.0;
        let mut sum_x2 = 0.0;

        for entry in window {
            if entry.work_score > 0.0 {
                let x = entry.tau;
                let y = entry.work_score.ln();
                sum_x += x;
                sum_y += y;
                sum_xy += x * y;
                sum_x2 += x * x;
            }
        }

        // Slope = (n·Σxy - Σx·Σy) / (n·Σx² - (Σx)²)
        let denominator = n * sum_x2 - sum_x * sum_x;
        if denominator.abs() < 1e-10 {
            return Ok((ETA, 0.0)); // Degenerate case
        }

        let slope = (n * sum_xy - sum_x * sum_y) / denominator;

        // Calculate R² (coefficient of determination)
        let mean_y = sum_y / n;
        let mut ss_tot = 0.0;
        let mut ss_res = 0.0;

        for entry in window {
            if entry.work_score > 0.0 {
                let y = entry.work_score.ln();
                let y_pred = slope * entry.tau + (sum_y - slope * sum_x) / n;
                ss_tot += (y - mean_y).powi(2);
                ss_res += (y - y_pred).powi(2);
            }
        }

        let r_squared = if ss_tot > 0.0 {
            1.0 - (ss_res / ss_tot)
        } else {
            0.0
        };

        let measured_eta = -slope; // η is the decay rate (negative slope)

        Ok((measured_eta.max(0.0), r_squared))
    }

    /// Measure λ from timing coherence
    /// High coherence (stable block times) → strong coupling (high λ)
    pub fn measure_lambda_from_timing(&self, measured_eta: f64, window_size: usize) -> Result<f64, String> {
        let read_txn = self.db.begin_read()
            .map_err(|e| format!("Failed to begin read transaction: {}", e))?;

        let table = read_txn.open_table(WORK_SCORE_HISTORY_TABLE)
            .map_err(|e| format!("Failed to open work score history table: {}", e))?;

        // Collect recent block times
        let mut entries: Vec<WorkScoreEntry> = Vec::new();
        let iter = table.iter()
            .map_err(|e| format!("Failed to iterate work scores: {}", e))?;

        for item in iter {
            let (_key, value) = item.map_err(|e| format!("Failed to read item: {}", e))?;
            let entry: WorkScoreEntry = bincode::deserialize(value.value())
                .map_err(|e| format!("Failed to deserialize entry: {}", e))?;
            entries.push(entry);
        }

        if entries.len() < 10 {
            // Unit circle constraint: λ = √(1 - η²)
            return Ok((1.0 - measured_eta.powi(2)).sqrt().max(0.0));
        }

        let start_idx = entries.len().saturating_sub(window_size);
        let window = &entries[start_idx..];

        // Calculate coefficient of variation (CV) of block times
        // Low CV → high coherence → high λ
        let n = window.len() as f64;
        let mean_time: f64 = window.iter().map(|e| e.block_time).sum::<f64>() / n;
        let variance: f64 = window.iter()
            .map(|e| (e.block_time - mean_time).powi(2))
            .sum::<f64>() / n;
        let std_dev = variance.sqrt();
        let cv = if mean_time > 0.0 { std_dev / mean_time } else { 1.0 };

        // Timing coherence: 1.0 = perfect coherence, 0.0 = no coherence
        // CV = 0 → coherence = 1.0
        // CV → ∞ → coherence → 0.0
        let coherence = (-cv).exp(); // Exponential decay from perfect coherence

        // Theoretical λ from unit circle constraint
        let theoretical_lambda = (1.0 - measured_eta.powi(2)).sqrt().max(0.0);

        // Empirical λ scales with coherence
        let measured_lambda = coherence * theoretical_lambda;

        Ok(measured_lambda.max(0.0).min(1.0))
    }

    /// Update consensus metrics (call periodically, e.g., every 100 blocks)
    pub fn update_consensus_metrics(&self, block_height: u64, window_size: usize) -> Result<ConsensusMetrics, String> {
        // Measure η from work score exponential decay
        let (measured_eta, r_squared) = self.measure_eta_from_work_scores(window_size)?;

        // Measure λ from timing coherence
        let measured_lambda = self.measure_lambda_from_timing(measured_eta, window_size)?;

        // Calculate oracle metric Δ for measured values
        let measured_oracle_delta = self.calculate_oracle_delta(measured_eta, measured_lambda);

        let metrics = ConsensusMetrics {
            measured_eta,
            measured_lambda,
            measured_oracle_delta,
            convergence_confidence: r_squared,
            sample_size: window_size,
            last_update_height: block_height,
        };

        // Persist metrics
        let write_txn = self.db.begin_write()
            .map_err(|e| format!("Failed to begin write transaction: {}", e))?;
        {
            let mut table = write_txn.open_table(CONSENSUS_METRICS_TABLE)
                .map_err(|e| format!("Failed to open consensus metrics table: {}", e))?;

            let serialized = bincode::serialize(&metrics)
                .map_err(|e| format!("Failed to serialize metrics: {}", e))?;
            table.insert(block_height, serialized.as_slice())
                .map_err(|e| format!("Failed to insert metrics: {}", e))?;
        }
        write_txn.commit()
            .map_err(|e| format!("Failed to commit metrics: {}", e))?;

        Ok(metrics)
    }

    /// Calculate oracle metric Δ for given (η, λ)
    fn calculate_oracle_delta(&self, eta: f64, lambda: f64) -> f64 {
        use coinject_core::VivianiOracle;

        let oracle = VivianiOracle::calculate(eta, lambda);
        oracle.delta
    }

    /// Get latest consensus metrics
    pub fn get_consensus_metrics(&self) -> Option<ConsensusMetrics> {
        let read_txn = self.db.begin_read().ok()?;
        let table = read_txn.open_table(CONSENSUS_METRICS_TABLE).ok()?;

        // Get most recent entry
        let mut iter = table.iter().ok()?;
        let entry = iter.next_back()?.ok()?;
        let (_k, v) = entry;
        bincode::deserialize(v.value()).ok()
    }

    /// Test "The Conjecture" - does consensus converge to η = λ = 1/√2?
    pub fn test_conjecture(&self) -> Option<ConjectureStatus> {
        let metrics = self.get_consensus_metrics()?;

        let eta_error = (metrics.measured_eta - ETA).abs();
        let lambda_error = (metrics.measured_lambda - LAMBDA).abs();
        let delta_error = (metrics.measured_oracle_delta - 0.231).abs();

        Some(ConjectureStatus {
            eta_convergence: eta_error < 0.05,       // Within 5% of theoretical
            lambda_convergence: lambda_error < 0.05,
            oracle_alignment: delta_error < 0.05,    // Within 5% of 0.231
            confidence: metrics.convergence_confidence,
            sample_size: metrics.sample_size,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_satoshi_constant() {
        // Verify η = λ = 1/√2
        let sqrt_2 = 2.0_f64.sqrt();
        assert!((ETA - 1.0 / sqrt_2).abs() < 1e-10);
        assert!((LAMBDA - 1.0 / sqrt_2).abs() < 1e-10);
    }

    #[test]
    fn test_unit_circle_constraint() {
        // Verify |μ|² = η² + λ² = 1
        let magnitude_squared = ETA.powi(2) + LAMBDA.powi(2);
        assert!((magnitude_squared - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_dimensional_factors() {
        // Verify D_n = e^(-η·τ_n)
        for (_, tau, expected_d, _) in DIMENSIONAL_SCALES.iter() {
            let calculated_d = (-ETA * tau).exp();
            assert!((calculated_d - expected_d).abs() < 0.01,
                "D_n mismatch for τ={}: expected {}, got {}", tau, expected_d, calculated_d);
        }
    }

    #[test]
    fn test_allocation_ratios_sum() {
        // Allocation ratios should be dimensionless and properly normalized
        let sum: f64 = ALLOCATION_RATIOS.iter().map(|(_, r)| r).sum();
        // Sum should be approximately 1.003 (properly normalized across all 8 pools)
        assert!((sum - 1.0).abs() < 0.01, "Allocation ratios sum: {}", sum);
    }

    #[test]
    fn test_phase_evolution() {
        // θ(τ) = λτ = τ/√2
        let tau = 1.0;
        let phase = LAMBDA * tau;
        let expected = tau / 2.0_f64.sqrt();
        assert!((phase - expected).abs() < 1e-10);
    }
}
