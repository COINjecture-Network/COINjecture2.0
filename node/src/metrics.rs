// COINjecture WEB4 Metrics - Institutional-grade observability
// Empirical validation of dimensional economics: η = λ = 1/√2
//
// This module exports comprehensive metrics for proving that the mathematical
// theory translates to real-world network behavior.

use lazy_static::lazy_static;
use prometheus::{
    register_counter_vec, register_gauge, register_gauge_vec, register_histogram_vec,
    register_int_counter, register_int_counter_vec, register_int_gauge, register_int_gauge_vec,
    CounterVec, Encoder, Gauge, GaugeVec, HistogramVec, IntCounter, IntCounterVec, IntGauge,
    IntGaugeVec, Registry, TextEncoder,
};

lazy_static! {
    // Global metrics registry
    pub static ref REGISTRY: Registry = Registry::new();

    // === DIMENSIONAL POOL METRICS ===
    // These metrics prove η = λ = 1/√2 through observable decay and coupling

    /// Pool balances by dimension (D₁, D₂, D₃)
    /// Expected ratios: D₂/D₁ ≈ 0.867, D₃/D₂ ≈ 0.865
    pub static ref POOL_BALANCE: GaugeVec = register_gauge_vec!(
        "coinject_pool_balance",
        "Balance in each dimensional pool (tokens)",
        &["dimension"]
    )
    .unwrap();

    /// Dimensional decay rates: D_n = e^(-η·τ_n)
    /// Should converge to η = 1/√2 ≈ 0.7071
    pub static ref DIMENSIONAL_DECAY_RATE: GaugeVec = register_gauge_vec!(
        "coinject_dimensional_decay_rate",
        "Observed decay rate for each dimension",
        &["dimension"]
    )
    .unwrap();

    /// Pool coupling strength between dimensions
    /// Validates critical damping hypothesis
    pub static ref POOL_COUPLING: GaugeVec = register_gauge_vec!(
        "coinject_pool_coupling",
        "Coupling coefficient between dimensional pools",
        &["from_dimension", "to_dimension"]
    )
    .unwrap();

    /// Dimensional scale ratios (theoretical vs observed)
    pub static ref DIMENSIONAL_SCALE: GaugeVec = register_gauge_vec!(
        "coinject_dimensional_scale",
        "Dimensional scale: D_n = exp(-eta * tau_n)",
        &["dimension", "type"]  // type: "theoretical" or "observed"
    )
    .unwrap();

    /// Pool swap volumes by dimension pair
    pub static ref POOL_SWAP_VOLUME: CounterVec = register_counter_vec!(
        "coinject_pool_swap_volume_total",
        "Total swap volume between dimensional pools (tokens)",
        &["from_dimension", "to_dimension"]
    )
    .unwrap();

    /// Pool liquidity depth
    pub static ref POOL_LIQUIDITY: GaugeVec = register_gauge_vec!(
        "coinject_pool_liquidity",
        "Available liquidity in each pool (tokens)",
        &["dimension"]
    )
    .unwrap();

    // === CONSENSUS METRICS ===
    // Validate that consensus dynamics reflect dimensional coupling

    /// Current block height
    pub static ref BLOCK_HEIGHT: IntGauge = register_int_gauge!(
        "coinject_block_height",
        "Current blockchain height"
    )
    .unwrap();

    /// Block production time (seconds between blocks)
    pub static ref BLOCK_TIME: Gauge = register_gauge!(
        "coinject_block_time_seconds",
        "Time between last two blocks (seconds)"
    )
    .unwrap();

    /// Block production histogram
    pub static ref BLOCK_TIME_HISTOGRAM: HistogramVec = register_histogram_vec!(
        "coinject_block_time_histogram_seconds",
        "Distribution of block production times",
        &["validator"],
        vec![0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 60.0]
    )
    .unwrap();

    /// Transaction count per block
    pub static ref BLOCK_TX_COUNT: IntGaugeVec = register_int_gauge_vec!(
        "coinject_block_tx_count",
        "Number of transactions in each block",
        &["block_hash"]
    )
    .unwrap();

    /// Validator set size
    pub static ref VALIDATOR_COUNT: IntGauge = register_int_gauge!(
        "coinject_validator_count",
        "Number of active validators"
    )
    .unwrap();

    /// Consensus rounds per block
    pub static ref CONSENSUS_ROUNDS: HistogramVec = register_histogram_vec!(
        "coinject_consensus_rounds",
        "Number of consensus rounds needed per block",
        &["result"],  // "success" or "timeout"
        vec![1.0, 2.0, 3.0, 5.0, 10.0]
    )
    .unwrap();

    // === PROOF OF USEFUL WORK (PoUW) METRICS ===
    // Validate that computational work translates to network value

    /// Work score distribution
    pub static ref WORK_SCORE: HistogramVec = register_histogram_vec!(
        "coinject_work_score",
        "Distribution of PoUW work scores",
        &["problem_type"],
        vec![1.0, 10.0, 100.0, 1000.0, 10000.0, 100000.0]
    )
    .unwrap();

    /// Problems submitted by type
    pub static ref PROBLEMS_SUBMITTED: IntCounterVec = register_int_counter_vec!(
        "coinject_problems_submitted_total",
        "Total problems submitted to marketplace",
        &["problem_type"]
    )
    .unwrap();

    /// Problems solved by type
    pub static ref PROBLEMS_SOLVED: IntCounterVec = register_int_counter_vec!(
        "coinject_problems_solved_total",
        "Total problems solved in marketplace",
        &["problem_type"]
    )
    .unwrap();

    /// Active marketplace bounties
    pub static ref ACTIVE_BOUNTIES: IntGauge = register_int_gauge!(
        "coinject_active_bounties",
        "Number of open problems with bounties"
    )
    .unwrap();

    /// Total bounty pool value
    pub static ref BOUNTY_POOL: Gauge = register_gauge!(
        "coinject_bounty_pool_tokens",
        "Total value of escrowed bounties (tokens)"
    )
    .unwrap();

    /// Solution verification time
    pub static ref VERIFICATION_TIME: HistogramVec = register_histogram_vec!(
        "coinject_verification_time_seconds",
        "Time to verify problem solutions",
        &["problem_type"],
        vec![0.001, 0.01, 0.1, 1.0, 10.0]
    )
    .unwrap();

    /// Autonomous payout count (WEB4 feature!)
    pub static ref AUTONOMOUS_PAYOUTS: IntCounter = register_int_counter!(
        "coinject_autonomous_payouts_total",
        "Number of autonomous bounty payouts (WEB4 revolution)"
    )
    .unwrap();

    // === NETWORK METRICS ===

    /// Connected peer count
    pub static ref PEER_COUNT: IntGauge = register_int_gauge!(
        "coinject_peer_count",
        "Number of connected peers"
    )
    .unwrap();

    /// Messages sent/received by type
    pub static ref NETWORK_MESSAGES: IntCounterVec = register_int_counter_vec!(
        "coinject_network_messages_total",
        "Network messages by type and direction",
        &["message_type", "direction"]
    )
    .unwrap();

    /// Network bandwidth
    pub static ref NETWORK_BANDWIDTH: CounterVec = register_counter_vec!(
        "coinject_network_bandwidth_bytes_total",
        "Network bandwidth usage",
        &["direction"]  // "inbound" or "outbound"
    )
    .unwrap();

    // === STATE METRICS ===

    /// Total supply
    pub static ref TOTAL_SUPPLY: Gauge = register_gauge!(
        "coinject_total_supply_tokens",
        "Total token supply"
    )
    .unwrap();

    /// Active accounts
    pub static ref ACTIVE_ACCOUNTS: IntGauge = register_int_gauge!(
        "coinject_active_accounts",
        "Number of accounts with non-zero balance"
    )
    .unwrap();

    /// Mempool size
    pub static ref MEMPOOL_SIZE: IntGauge = register_int_gauge!(
        "coinject_mempool_size",
        "Number of pending transactions"
    )
    .unwrap();

    /// Transaction throughput
    pub static ref TX_THROUGHPUT: Gauge = register_gauge!(
        "coinject_tx_throughput_tps",
        "Transactions per second (rolling average)"
    )
    .unwrap();

    // === SATOSHI CONSTANT VALIDATION ===
    // Direct measurements of η = λ = 1/√2 ≈ 0.7071

    /// Measured eta (η) from decay rates
    pub static ref MEASURED_ETA: Gauge = register_gauge!(
        "coinject_measured_eta",
        "Empirically measured η from pool decay rates"
    )
    .unwrap();

    /// Measured lambda (λ) from coupling rates
    pub static ref MEASURED_LAMBDA: Gauge = register_gauge!(
        "coinject_measured_lambda",
        "Empirically measured λ from pool coupling"
    )
    .unwrap();

    /// Unit circle constraint: |μ|² = η² + λ²
    /// Should equal 1.0 if theory is correct
    pub static ref UNIT_CIRCLE_CONSTRAINT: Gauge = register_gauge!(
        "coinject_unit_circle_constraint",
        "Validation of |mu|^2 = eta^2 + lambda^2 = 1"
    )
    .unwrap();

    /// Critical damping coefficient ζ = η/√2
    /// Should equal 1.0 for critical damping
    pub static ref DAMPING_COEFFICIENT: Gauge = register_gauge!(
        "coinject_damping_coefficient",
        "Damping coefficient zeta = eta/sqrt(2)"
    )
    .unwrap();

    // === CONSENSUS STATE METRICS (RUNTIME INTEGRATION) ===
    // Live tracking of τ, |ψ|, and θ from actual blockchain state

    /// Dimensionless time: τ = block_height / τ_c
    pub static ref CONSENSUS_TAU: Gauge = register_gauge!(
        "coinject_consensus_tau",
        "Dimensionless time tau = block_height / tau_c"
    )
    .unwrap();

    /// Complex wavefunction magnitude: |ψ(τ)| = e^(-ητ)
    pub static ref CONSENSUS_MAGNITUDE: Gauge = register_gauge!(
        "coinject_consensus_magnitude",
        "Wavefunction magnitude |psi(tau)| = exp(-eta*tau)"
    )
    .unwrap();

    /// Complex wavefunction phase: θ(τ) = λτ
    pub static ref CONSENSUS_PHASE: Gauge = register_gauge!(
        "coinject_consensus_phase",
        "Wavefunction phase theta(tau) = lambda*tau (radians)"
    )
    .unwrap();
}

/// Initialize all metrics
pub fn init() {
    // Register all metrics with the global registry
    REGISTRY
        .register(Box::new(POOL_BALANCE.clone()))
        .expect("Failed to register pool_balance");
    REGISTRY
        .register(Box::new(DIMENSIONAL_DECAY_RATE.clone()))
        .expect("Failed to register dimensional_decay_rate");
    REGISTRY
        .register(Box::new(POOL_COUPLING.clone()))
        .expect("Failed to register pool_coupling");
    REGISTRY
        .register(Box::new(DIMENSIONAL_SCALE.clone()))
        .expect("Failed to register dimensional_scale");
    REGISTRY
        .register(Box::new(POOL_SWAP_VOLUME.clone()))
        .expect("Failed to register pool_swap_volume");
    REGISTRY
        .register(Box::new(POOL_LIQUIDITY.clone()))
        .expect("Failed to register pool_liquidity");

    REGISTRY
        .register(Box::new(BLOCK_HEIGHT.clone()))
        .expect("Failed to register block_height");
    REGISTRY
        .register(Box::new(BLOCK_TIME.clone()))
        .expect("Failed to register block_time");
    REGISTRY
        .register(Box::new(BLOCK_TIME_HISTOGRAM.clone()))
        .expect("Failed to register block_time_histogram");
    REGISTRY
        .register(Box::new(BLOCK_TX_COUNT.clone()))
        .expect("Failed to register block_tx_count");
    REGISTRY
        .register(Box::new(VALIDATOR_COUNT.clone()))
        .expect("Failed to register validator_count");
    REGISTRY
        .register(Box::new(CONSENSUS_ROUNDS.clone()))
        .expect("Failed to register consensus_rounds");

    REGISTRY
        .register(Box::new(WORK_SCORE.clone()))
        .expect("Failed to register work_score");
    REGISTRY
        .register(Box::new(PROBLEMS_SUBMITTED.clone()))
        .expect("Failed to register problems_submitted");
    REGISTRY
        .register(Box::new(PROBLEMS_SOLVED.clone()))
        .expect("Failed to register problems_solved");
    REGISTRY
        .register(Box::new(ACTIVE_BOUNTIES.clone()))
        .expect("Failed to register active_bounties");
    REGISTRY
        .register(Box::new(BOUNTY_POOL.clone()))
        .expect("Failed to register bounty_pool");
    REGISTRY
        .register(Box::new(VERIFICATION_TIME.clone()))
        .expect("Failed to register verification_time");
    REGISTRY
        .register(Box::new(AUTONOMOUS_PAYOUTS.clone()))
        .expect("Failed to register autonomous_payouts");

    REGISTRY
        .register(Box::new(PEER_COUNT.clone()))
        .expect("Failed to register peer_count");
    REGISTRY
        .register(Box::new(NETWORK_MESSAGES.clone()))
        .expect("Failed to register network_messages");
    REGISTRY
        .register(Box::new(NETWORK_BANDWIDTH.clone()))
        .expect("Failed to register network_bandwidth");

    REGISTRY
        .register(Box::new(TOTAL_SUPPLY.clone()))
        .expect("Failed to register total_supply");
    REGISTRY
        .register(Box::new(ACTIVE_ACCOUNTS.clone()))
        .expect("Failed to register active_accounts");
    REGISTRY
        .register(Box::new(MEMPOOL_SIZE.clone()))
        .expect("Failed to register mempool_size");
    REGISTRY
        .register(Box::new(TX_THROUGHPUT.clone()))
        .expect("Failed to register tx_throughput");

    REGISTRY
        .register(Box::new(MEASURED_ETA.clone()))
        .expect("Failed to register measured_eta");
    REGISTRY
        .register(Box::new(MEASURED_LAMBDA.clone()))
        .expect("Failed to register measured_lambda");
    REGISTRY
        .register(Box::new(UNIT_CIRCLE_CONSTRAINT.clone()))
        .expect("Failed to register unit_circle_constraint");
    REGISTRY
        .register(Box::new(DAMPING_COEFFICIENT.clone()))
        .expect("Failed to register damping_coefficient");

    REGISTRY
        .register(Box::new(CONSENSUS_TAU.clone()))
        .expect("Failed to register consensus_tau");
    REGISTRY
        .register(Box::new(CONSENSUS_MAGNITUDE.clone()))
        .expect("Failed to register consensus_magnitude");
    REGISTRY
        .register(Box::new(CONSENSUS_PHASE.clone()))
        .expect("Failed to register consensus_phase");

    tracing::info!("✓ Prometheus metrics initialized");
}

/// Export metrics in Prometheus text format
pub fn export() -> Result<String, prometheus::Error> {
    let encoder = TextEncoder::new();
    let metric_families = REGISTRY.gather();
    let mut buffer = vec![];
    encoder.encode(&metric_families, &mut buffer)?;
    String::from_utf8(buffer).map_err(|e| prometheus::Error::Msg(e.to_string()))
}

/// Update dimensional pool metrics
pub fn update_pool_metrics(dimension: u8, balance: u128, liquidity: u128) {
    let dim_label = format!("D{}", dimension);
    POOL_BALANCE
        .with_label_values(&[&dim_label])
        .set(balance as f64);
    POOL_LIQUIDITY
        .with_label_values(&[&dim_label])
        .set(liquidity as f64);
}

/// Record pool swap
pub fn record_pool_swap(from_dimension: u8, to_dimension: u8, amount: u128) {
    let from_label = format!("D{}", from_dimension);
    let to_label = format!("D{}", to_dimension);
    POOL_SWAP_VOLUME
        .with_label_values(&[&from_label, &to_label])
        .inc_by(amount as f64);
}

/// Update dimensional scale measurements
pub fn update_dimensional_scales(
    dimension: u8,
    theoretical: f64,
    observed: f64,
    decay_rate: f64,
) {
    let dim_label = format!("D{}", dimension);

    DIMENSIONAL_SCALE
        .with_label_values(&[&dim_label, "theoretical"])
        .set(theoretical);
    DIMENSIONAL_SCALE
        .with_label_values(&[&dim_label, "observed"])
        .set(observed);
    DIMENSIONAL_DECAY_RATE
        .with_label_values(&[&dim_label])
        .set(decay_rate);
}

/// Update Satoshi constant measurements
pub fn update_satoshi_constants(eta: f64, lambda: f64) {
    MEASURED_ETA.set(eta);
    MEASURED_LAMBDA.set(lambda);

    // Validate unit circle constraint: |μ|² = η² + λ²
    let unit_circle = eta * eta + lambda * lambda;
    UNIT_CIRCLE_CONSTRAINT.set(unit_circle);

    // Calculate damping coefficient: ζ = η/√2
    let damping = eta / std::f64::consts::SQRT_2;
    DAMPING_COEFFICIENT.set(damping);
}

/// Record work score
pub fn record_work_score(problem_type: &str, score: f64) {
    WORK_SCORE
        .with_label_values(&[problem_type])
        .observe(score);
}

/// Record problem submission
pub fn record_problem_submitted(problem_type: &str) {
    PROBLEMS_SUBMITTED.with_label_values(&[problem_type]).inc();
}

/// Record problem solved
pub fn record_problem_solved(problem_type: &str) {
    PROBLEMS_SOLVED.with_label_values(&[problem_type]).inc();
}

/// Record autonomous payout (WEB4!)
pub fn record_autonomous_payout() {
    AUTONOMOUS_PAYOUTS.inc();
}

/// Update block metrics
pub fn update_block_metrics(height: u64, tx_count: usize, block_time: f64, block_hash: &str) {
    BLOCK_HEIGHT.set(height as i64);
    BLOCK_TIME.set(block_time);
    BLOCK_TX_COUNT
        .with_label_values(&[block_hash])
        .set(tx_count as i64);
}

/// Update network metrics
pub fn update_network_metrics(peer_count: usize) {
    PEER_COUNT.set(peer_count as i64);
}

/// Update state metrics
pub fn update_state_metrics(
    total_supply: u128,
    active_accounts: usize,
    mempool_size: usize,
    tps: f64,
) {
    TOTAL_SUPPLY.set(total_supply as f64);
    ACTIVE_ACCOUNTS.set(active_accounts as i64);
    MEMPOOL_SIZE.set(mempool_size as i64);
    TX_THROUGHPUT.set(tps);
}
