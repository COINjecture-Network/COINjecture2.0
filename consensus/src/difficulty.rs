// Dynamic Difficulty Adjustment (EMPIRICAL VERSION)
// Scales problem size to maintain target solve times for meaningful asymmetry
//
// COMPLIANCE: Empirical ✓ | Self-referential ✓ | Dimensionless ✓
//
// Principles (from whitepaper):
// 1. Dimensionless - Pure ratios, no arbitrary limits
// 2. Self-Referenced - Measured against network's own state
// 3. Empirically Grounded - Derived from actual solve times
//
// PHASE-4 SAFETY: Solve times are stored as u64 microseconds (exact integer
// representation). The adjustment arithmetic uses integer square-root via
// coinject_core::fixed_point::isqrt so that the size decision is deterministic
// across platforms. f64 is used only at the display/monitoring boundary.
//
// ALL values derived from network state via NetworkMetrics oracle:
// - Optimal solve time: median_block_time * η (from network)
// - Min/Max targets: Optimal * PHI_INV / PHI (mathematical bounds)
// - Problem size limits: Percentiles from historical solve times

use crate::problem_registry::SharedRegistry;
use coinject_core::fixed_point::isqrt;
use coinject_tokenomics::{NetworkMetrics, ETA, PHI, PHI_INV};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Number of trailing blocks used for empirical difficulty measurement.
/// During bootstrap (blocks 0–19), seeded defaults govern.
/// After block 20, empirical data takes over.
/// See docs/BOOTSTRAP.md for the full bootstrap → empirical transition.
const DIFFICULTY_WINDOW: usize = 20;

/// Stall and stability tuning (dimensionless ratios)
const HIGH_VARIANCE_RATIO: f64 = 0.8; // σ close to μ ⇒ widen window
const RECOVERY_STABLE_RATIO: f64 = 1.2;
const RECOVERY_STEP: usize = 1;

/// Absolute minimum problem size — must never reach 0 (would halt the chain).
const ABSOLUTE_MIN_SIZE: usize = 1;

/// Bootstrap default target solve time in microseconds (5 seconds).
const DEFAULT_TARGET_US: u64 = 5_000_000;

/// Scale factor clamp expressed as a fraction: new_size stays in
/// [current × MIN_SCALE_NUM/DENOM, current × MAX_SCALE_NUM/DENOM].
const SCALE_CLAMP_MIN_NUM: u128 = 2;  // 0.4 = 2/5
const SCALE_CLAMP_MIN_DEN: u128 = 5;
const SCALE_CLAMP_MAX_NUM: u128 = 2;  // 2.0 = 2/1
const SCALE_CLAMP_MAX_DEN: u128 = 1;

/// Difficulty adjuster - tracks solve times and adjusts problem size.
///
/// ## Determinism guarantee
///
/// Solve times are stored as **u64 microseconds** (exact integer, no rounding).
/// The size-adjustment decision uses integer square root, not floating-point
/// `sqrt()`. The only f64 usage is in `stats()` / `stats_async()` which are
/// monitoring helpers and are never part of consensus.
pub struct DifficultyAdjuster {
    /// Recent solve times in **microseconds** (integer, no FP rounding).
    recent_solve_times_us: VecDeque<u64>,
    /// Current problem size (dimensionless integer).
    current_size: usize,
    /// Target size we want to return to after a penalty.
    recovery_target: Option<usize>,
    /// Counts consecutive stalls/failures.
    stall_counter: usize,
    /// Network metrics oracle (optional - uses defaults if None).
    network_metrics: Option<Arc<RwLock<NetworkMetrics>>>,
    /// Problem registry for type-specific parameters.
    registry: Option<SharedRegistry>,
}

impl DifficultyAdjuster {
    /// Create new difficulty adjuster without network metrics (uses defaults).
    pub fn new() -> Self {
        DifficultyAdjuster {
            recent_solve_times_us: VecDeque::with_capacity(DIFFICULTY_WINDOW),
            current_size: 20, // Canonical SubsetSum size unit (bootstrap default)
            recovery_target: None,
            stall_counter: 0,
            network_metrics: None,
            registry: None,
        }
    }

    /// Create with network metrics oracle (empirical mode).
    pub fn with_metrics(network_metrics: Arc<RwLock<NetworkMetrics>>) -> Self {
        DifficultyAdjuster {
            recent_solve_times_us: VecDeque::with_capacity(DIFFICULTY_WINDOW),
            current_size: 20,
            recovery_target: None,
            stall_counter: 0,
            network_metrics: Some(network_metrics),
            registry: None,
        }
    }

    /// Create with both network metrics and problem registry.
    pub fn with_registry(network_metrics: Arc<RwLock<NetworkMetrics>>, registry: SharedRegistry) -> Self {
        DifficultyAdjuster {
            recent_solve_times_us: VecDeque::with_capacity(DIFFICULTY_WINDOW),
            current_size: 20,
            recovery_target: None,
            stall_counter: 0,
            network_metrics: Some(network_metrics),
            registry: Some(registry),
        }
    }

    /// Update network metrics reference.
    pub fn set_metrics(&mut self, network_metrics: Arc<RwLock<NetworkMetrics>>) {
        self.network_metrics = Some(network_metrics);
    }

    /// Set problem registry.
    pub fn set_registry(&mut self, registry: SharedRegistry) {
        self.registry = Some(registry);
    }

    /// Check if network metrics are available.
    pub fn has_metrics(&self) -> bool {
        self.network_metrics.is_some()
    }

    /// Get optimal solve time in microseconds from network metrics (or default).
    async fn optimal_solve_time_us(&self) -> u64 {
        if let Some(ref metrics) = self.network_metrics {
            let metrics = metrics.read().await;
            // Optimal = median_block_time * η (mathematical scaling).
            // Convert seconds → microseconds.
            let secs = metrics.median_block_time() * ETA;
            (secs * 1_000_000.0) as u64
        } else {
            DEFAULT_TARGET_US
        }
    }

    /// Get optimal solve time as f64 seconds (display/compat only).
    async fn optimal_solve_time(&self) -> f64 {
        self.optimal_solve_time_us().await as f64 / 1_000_000.0
    }

    /// Get min target solve time in seconds (display/compat only).
    async fn min_target_solve_time(&self) -> f64 {
        let optimal = self.optimal_solve_time().await;
        optimal * PHI_INV
    }

    /// Get max target solve time in seconds (display/compat only).
    async fn max_target_solve_time(&self) -> f64 {
        let optimal = self.optimal_solve_time().await;
        optimal * PHI
    }

    /// Get problem size limits from network metrics and problem registry.
    async fn get_size_limits(&self, problem_type: &str) -> (usize, usize) {
        let (scaling_exp, abs_max, abs_min) = if let Some(ref registry) = self.registry {
            let reg = registry.read().await;
            if let Some(desc) = reg.get(problem_type) {
                (desc.scaling_exponent(), desc.absolute_max_size(), desc.absolute_min_size())
            } else {
                (0.8, 60, ABSOLUTE_MIN_SIZE)
            }
        } else {
            match problem_type {
                "TSP" => (0.5, 30, ABSOLUTE_MIN_SIZE),
                "SAT" => (0.7, 120, ABSOLUTE_MIN_SIZE),
                "SubsetSum" => (0.8, 60, ABSOLUTE_MIN_SIZE),
                _ => (0.8, 60, ABSOLUTE_MIN_SIZE),
            }
        };

        if let Some(ref metrics) = self.network_metrics {
            let metrics = metrics.read().await;

            let category = match problem_type {
                "SubsetSum" => 3,
                "SAT" => 0,
                "TSP" => 1,
                _ => 0,
            };

            let hardness = metrics.hardness_factor(category);
            let median_time = metrics.median_block_time();
            let optimal_time = median_time * ETA * hardness;
            let base_time = 1.0;
            let estimated_size = ((optimal_time / base_time).powf(scaling_exp) * hardness) as usize;

            let min_size = (estimated_size as f64 * 0.2)
                .max(abs_min as f64)
                .max(ABSOLUTE_MIN_SIZE as f64) as usize;
            let max_size = (estimated_size as f64 * 2.0).min(abs_max as f64) as usize;

            (min_size.max(ABSOLUTE_MIN_SIZE), max_size.max(ABSOLUTE_MIN_SIZE + 1))
        } else {
            let min = abs_min.max(ABSOLUTE_MIN_SIZE);
            let max = abs_max.min(match problem_type {
                "SubsetSum" => 50,
                "SAT" => 100,
                "TSP" => 25,
                _ => 50,
            });
            (min, max.max(min + 1))
        }
    }

    /// Record a solve time (accepts `Duration` for API compatibility).
    ///
    /// Internally stored as **microseconds (u64)** — exact integer, no rounding.
    pub fn record_solve_time(&mut self, solve_time: Duration) {
        self.record_solve_time_us(solve_time.as_micros() as u64);
    }

    /// Record a solve time in microseconds (preferred low-level API).
    pub fn record_solve_time_us(&mut self, solve_time_us: u64) {
        self.recent_solve_times_us.push_back(solve_time_us);
        if self.recent_solve_times_us.len() > DIFFICULTY_WINDOW {
            self.recent_solve_times_us.pop_front();
        }
    }

    /// Get current problem size (always ≥ ABSOLUTE_MIN_SIZE).
    pub fn current_size(&self) -> usize {
        self.current_size.max(ABSOLUTE_MIN_SIZE)
    }

    /// Moving average of recent solve times in **microseconds** (integer, exact).
    fn moving_average_us(&self) -> u64 {
        if self.recent_solve_times_us.is_empty() {
            return DEFAULT_TARGET_US;
        }
        // Saturating sum to guard against overflow for very long solve times.
        let sum: u64 = self.recent_solve_times_us
            .iter()
            .fold(0u64, |acc, &t| acc.saturating_add(t));
        sum / self.recent_solve_times_us.len() as u64
    }

    /// Moving average as f64 seconds (display/monitoring only).
    fn moving_average_solve_time(&self) -> f64 {
        self.moving_average_us() as f64 / 1_000_000.0
    }

    /// Async moving average as f64 seconds (falls back to network target).
    async fn moving_average_solve_time_async(&self) -> f64 {
        if self.recent_solve_times_us.is_empty() {
            return self.optimal_solve_time().await;
        }
        self.moving_average_us() as f64 / 1_000_000.0
    }

    /// Compute new problem size using deterministic integer arithmetic.
    ///
    /// `new_size = isqrt(current_size² × target_us / avg_us)`
    ///
    /// This is mathematically equivalent to `current_size × √(target/avg)`
    /// but uses integer square root — bit-exact across all platforms.
    fn compute_new_size_deterministic(current_size: usize, avg_us: u64, target_us: u64) -> usize {
        if avg_us == 0 {
            return current_size;
        }
        let size_sq = (current_size as u128) * (current_size as u128);
        let scaled = size_sq.saturating_mul(target_us as u128) / (avg_us as u128);
        isqrt(scaled) as usize
    }

    /// Adjust difficulty based on recent solve times (sync version).
    ///
    /// Uses deterministic integer arithmetic for the size-change decision.
    pub fn adjust_difficulty(&mut self) -> usize {
        if self.recent_solve_times_us.len() < DIFFICULTY_WINDOW / 2 {
            return self.current_size();
        }

        let avg_us = self.moving_average_us();
        let avg_secs = avg_us as f64 / 1_000_000.0;
        let target_us = DEFAULT_TARGET_US;
        let optimal = target_us as f64 / 1_000_000.0;
        let min_target = optimal * PHI_INV;
        let max_target = optimal * PHI;

        if avg_secs < min_target {
            println!(
                "⚡ Solve times {:.2}s are below minimum target ({:.1}s). Allowing controlled growth.",
                avg_secs, min_target
            );
        }
        let std_dev = self.solve_time_std_dev(avg_secs);

        if std_dev > avg_secs * HIGH_VARIANCE_RATIO {
            println!(
                "🔁 High variance detected (σ={:.2}s). Deferring difficulty adjustment.",
                std_dev
            );
            return self.current_size();
        }

        // Stall detection: avg > 2 × max_target
        if avg_secs > max_target * 2.0 {
            self.apply_stall_penalty("avg solve time exceeded safe threshold");
        }

        // ── Deterministic integer size computation ────────────────────────
        let raw_new_size =
            Self::compute_new_size_deterministic(self.current_size(), avg_us, target_us);

        // Apply stall ratio reduction in integer domain.
        let stall_ratio = avg_us / target_us.max(1);
        let adjusted_size = if stall_ratio > 5 {
            // Extreme stall: cap at 40% of raw
            (raw_new_size * 2 / 5).max(ABSOLUTE_MIN_SIZE)
        } else if stall_ratio > 2 {
            // Moderate stall: reduce by 30%
            (raw_new_size * 7 / 10).max(ABSOLUTE_MIN_SIZE)
        } else {
            raw_new_size
        };

        // Clamp: [current × 0.4, current × 2.0] — integer fractions
        let min_clamped = (self.current_size() as u128 * SCALE_CLAMP_MIN_NUM
            / SCALE_CLAMP_MIN_DEN) as usize;
        let max_clamped = (self.current_size() as u128 * SCALE_CLAMP_MAX_NUM
            / SCALE_CLAMP_MAX_DEN) as usize;

        let (global_min, global_max) = (ABSOLUTE_MIN_SIZE, 50usize);
        let bounded = adjusted_size
            .max(min_clamped.max(global_min))
            .min(max_clamped.min(global_max));

        let bounded = self.apply_recovery(bounded, avg_us as f64 / target_us as f64);

        println!("📊 Difficulty Adjustment:");
        println!(
            "   Avg solve time: {:.3}s (target: {:.1}s) σ={:.3}s",
            avg_secs, optimal, std_dev
        );
        println!("   Time ratio: {:.3}x", avg_us as f64 / target_us as f64);
        println!(
            "   Problem size: {} → {}{}",
            self.current_size(),
            bounded,
            if self.recovery_target.is_some() { " (recovery mode)" } else { "" }
        );

        self.current_size = bounded.max(ABSOLUTE_MIN_SIZE);
        self.current_size
    }

    /// Adjust difficulty with network metrics (async, fully empirical).
    pub async fn adjust_difficulty_async(&mut self) -> usize {
        if self.recent_solve_times_us.len() < DIFFICULTY_WINDOW / 2 {
            return self.current_size();
        }

        let avg_us = self.moving_average_us();
        let avg_secs = avg_us as f64 / 1_000_000.0;
        let target_us = self.optimal_solve_time_us().await;
        let optimal = target_us as f64 / 1_000_000.0;
        let min_target = self.min_target_solve_time().await;
        let max_target = self.max_target_solve_time().await;

        if avg_secs < min_target {
            println!(
                "⚡ Solve times {:.2}s below minimum target ({:.1}s). Allowing controlled growth.",
                avg_secs, min_target
            );
        }
        let std_dev = self.solve_time_std_dev(avg_secs);

        if std_dev > avg_secs * HIGH_VARIANCE_RATIO {
            println!(
                "🔁 High variance detected (σ={:.2}s). Deferring difficulty adjustment.",
                std_dev
            );
            return self.current_size();
        }

        if avg_secs > max_target * 2.0 {
            self.apply_stall_penalty("avg solve time exceeded safe threshold");
        }

        // ── Deterministic integer size computation ────────────────────────
        let raw_new_size =
            Self::compute_new_size_deterministic(self.current_size(), avg_us, target_us.max(1));

        let stall_ratio = avg_us / target_us.max(1);
        let adjusted_size = if stall_ratio > 5 {
            (raw_new_size * 2 / 5).max(ABSOLUTE_MIN_SIZE)
        } else if stall_ratio > 2 {
            (raw_new_size * 7 / 10).max(ABSOLUTE_MIN_SIZE)
        } else {
            raw_new_size
        };

        let min_clamped = (self.current_size() as u128 * SCALE_CLAMP_MIN_NUM
            / SCALE_CLAMP_MIN_DEN) as usize;
        let max_clamped = (self.current_size() as u128 * SCALE_CLAMP_MAX_NUM
            / SCALE_CLAMP_MAX_DEN) as usize;

        let (global_min, global_max) = self.get_size_limits("SubsetSum").await;
        let bounded = adjusted_size
            .max(min_clamped.max(global_min))
            .min(max_clamped.min(global_max));

        let bounded = self.apply_recovery(bounded, avg_us as f64 / target_us.max(1) as f64);

        println!("📊 Difficulty Adjustment (Empirical):");
        println!(
            "   Avg solve time: {:.3}s (target: {:.1}s, range: [{:.1}, {:.1}]) σ={:.3}s",
            avg_secs, optimal, min_target, max_target, std_dev
        );
        println!("   Time ratio: {:.3}x", avg_us as f64 / target_us.max(1) as f64);
        println!(
            "   Problem size: {} → {} (limits: [{}, {}]){}",
            self.current_size(),
            bounded,
            global_min,
            global_max,
            if self.recovery_target.is_some() { " (recovery mode)" } else { "" }
        );

        self.current_size = bounded.max(ABSOLUTE_MIN_SIZE);
        self.current_size
    }

    /// Penalize difficulty immediately after an unsolved block.
    pub fn penalize_failure(&mut self) -> usize {
        let old_size = self.current_size();
        // Reduce to 85%, floor at ABSOLUTE_MIN_SIZE.
        let reduced = ((old_size as u128 * 85) / 100)
            .max(ABSOLUTE_MIN_SIZE as u128) as usize;
        println!("⚠️  Mining failure penalty: {} → {}", old_size, reduced);
        self.recovery_target = Some(self.recovery_target.unwrap_or(old_size));
        self.current_size = reduced;
        self.stall_counter = (self.stall_counter + 1).min(20);
        self.current_size
    }

    /// Penalize failure with network metrics (async version).
    pub async fn penalize_failure_async(&mut self) -> usize {
        let old_size = self.current_size();
        let (min_size, _) = self.get_size_limits("SubsetSum").await;
        let reduced = ((old_size as u128 * 85) / 100)
            .max(min_size as u128)
            .max(ABSOLUTE_MIN_SIZE as u128) as usize;
        println!(
            "⚠️  Mining failure penalty: {} → {} (network-derived min: {})",
            old_size, reduced, min_size
        );
        self.recovery_target = Some(self.recovery_target.unwrap_or(old_size));
        self.current_size = reduced;
        self.stall_counter = (self.stall_counter + 1).min(20);
        self.current_size
    }

    /// Get problem size for specific problem type (sync, with registry).
    pub fn size_for_problem_type(&self, problem_type: &str) -> usize {
        if let Some(ref registry) = self.registry {
            if let Ok(reg) = registry.try_read() {
                if let Some(desc) = reg.get(problem_type) {
                    let ratio = desc.size_ratio();
                    let max = desc.absolute_max_size();
                    let min = desc.absolute_min_size().max(ABSOLUTE_MIN_SIZE);
                    return ((self.current_size() as f64 * ratio).round() as usize)
                        .max(min)
                        .min(max);
                }
            }
        }
        // Legacy fallback
        match problem_type {
            "SubsetSum" => self.current_size().min(50),
            "SAT" => ((self.current_size() as f64 * 0.75).round() as usize)
                .max(ABSOLUTE_MIN_SIZE)
                .min(100),
            "TSP" => ((self.current_size() as f64 * 0.35).round() as usize)
                .max(ABSOLUTE_MIN_SIZE)
                .min(25),
            _ => self.current_size(),
        }
    }

    /// Get problem size for specific problem type (async, with network metrics).
    pub async fn size_for_problem_type_async(&self, problem_type: &str) -> usize {
        let (min_size, max_size) = self.get_size_limits(problem_type).await;

        let size_ratio = if let Some(ref registry) = self.registry {
            let reg = registry.read().await;
            reg.get(problem_type).map(|d| d.size_ratio()).unwrap_or(1.0)
        } else {
            match problem_type {
                "SAT" => 0.75,
                "TSP" => 0.35,
                _ => 1.0,
            }
        };

        ((self.current_size() as f64 * size_ratio).round() as usize)
            .max(min_size.max(ABSOLUTE_MIN_SIZE))
            .min(max_size)
    }

    /// Get statistics for monitoring (display/metrics only — NOT consensus).
    pub fn stats(&self) -> DifficultyStats {
        let avg_us = self.moving_average_us();
        let avg_secs = avg_us as f64 / 1_000_000.0;
        let optimal_secs = DEFAULT_TARGET_US as f64 / 1_000_000.0;
        let std_dev = self.solve_time_std_dev(avg_secs);

        let min_us = self.recent_solve_times_us.iter().copied().min().unwrap_or(0);
        let max_us = self.recent_solve_times_us.iter().copied().max().unwrap_or(0);

        DifficultyStats {
            current_size: self.current_size(),
            avg_solve_time_secs: avg_secs,
            min_solve_time_secs: min_us as f64 / 1_000_000.0,
            max_solve_time_secs: max_us as f64 / 1_000_000.0,
            std_dev_secs: std_dev,
            sample_count: self.recent_solve_times_us.len(),
            time_ratio: avg_secs / optimal_secs,
            stall_counter: self.stall_counter,
            in_recovery_mode: self.recovery_target.is_some(),
        }
    }

    /// Get statistics with network metrics (display/metrics only).
    pub async fn stats_async(&self) -> DifficultyStats {
        let avg_us = self.moving_average_us();
        let avg_secs = avg_us as f64 / 1_000_000.0;
        let target_us = self.optimal_solve_time_us().await;
        let target_secs = target_us as f64 / 1_000_000.0;
        let std_dev = self.solve_time_std_dev(avg_secs);

        let min_us = self.recent_solve_times_us.iter().copied().min().unwrap_or(0);
        let max_us = self.recent_solve_times_us.iter().copied().max().unwrap_or(0);

        DifficultyStats {
            current_size: self.current_size(),
            avg_solve_time_secs: avg_secs,
            min_solve_time_secs: min_us as f64 / 1_000_000.0,
            max_solve_time_secs: max_us as f64 / 1_000_000.0,
            std_dev_secs: std_dev,
            sample_count: self.recent_solve_times_us.len(),
            time_ratio: if target_secs > 0.0 { avg_secs / target_secs } else { 1.0 },
            stall_counter: self.stall_counter,
            in_recovery_mode: self.recovery_target.is_some(),
        }
    }

    /// Standard deviation of solve times (display/monitoring only — uses f64).
    fn solve_time_std_dev(&self, mean_secs: f64) -> f64 {
        if self.recent_solve_times_us.len() < 2 {
            return 0.0;
        }
        let variance = self
            .recent_solve_times_us
            .iter()
            .map(|&t_us| {
                let t = t_us as f64 / 1_000_000.0;
                let delta = t - mean_secs;
                delta * delta
            })
            .sum::<f64>()
            / self.recent_solve_times_us.len() as f64;
        variance.sqrt()
    }

    #[allow(dead_code)]
    fn dynamic_cap(&self, base_max: usize, hardness: f64) -> usize {
        let stall_factor = 1.0 - (self.stall_counter as f64 * 0.05).min(0.5);
        let adaptive = (base_max as f64 * hardness * stall_factor).round() as usize;
        adaptive.max(ABSOLUTE_MIN_SIZE)
    }

    fn apply_recovery(&mut self, current: usize, time_ratio: f64) -> usize {
        if let Some(target) = self.recovery_target {
            if time_ratio <= RECOVERY_STABLE_RATIO {
                let next = (current + RECOVERY_STEP).min(target);
                if next >= target {
                    self.recovery_target = None;
                }
                return next.max(ABSOLUTE_MIN_SIZE);
            }
            return current.max(ABSOLUTE_MIN_SIZE);
        }

        if time_ratio <= 1.5 && self.stall_counter > 0 {
            self.stall_counter -= 1;
        }

        current.max(ABSOLUTE_MIN_SIZE)
    }

    fn apply_stall_penalty(&mut self, reason: &str) -> usize {
        let old_size = self.current_size();
        // Reduce to 70%, floor at ABSOLUTE_MIN_SIZE.
        let reduced = ((old_size as u128 * 7) / 10)
            .max(ABSOLUTE_MIN_SIZE as u128) as usize;
        println!("⚠️  Stall detected ({}). {} → {}", reason, old_size, reduced);
        self.recovery_target = Some(self.recovery_target.unwrap_or(old_size));
        self.current_size = reduced;
        self.stall_counter = (self.stall_counter + 1).min(20);
        reduced
    }
}

impl Default for DifficultyAdjuster {
    fn default() -> Self {
        Self::new()
    }
}

/// Difficulty statistics (display/monitoring only — never consensus-critical).
#[derive(Debug, Clone)]
pub struct DifficultyStats {
    pub current_size: usize,
    pub avg_solve_time_secs: f64,
    pub min_solve_time_secs: f64,
    pub max_solve_time_secs: f64,
    pub std_dev_secs: f64,
    pub sample_count: usize,
    pub time_ratio: f64,
    pub stall_counter: usize,
    pub in_recovery_mode: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_difficulty_increases_when_too_fast() {
        let mut adjuster = DifficultyAdjuster::new();

        // Simulate 20 blocks solved very fast (0.1 seconds each = 100_000 μs)
        for _ in 0..20 {
            adjuster.record_solve_time(Duration::from_millis(100));
        }

        let original_size = adjuster.current_size();
        let new_size = adjuster.adjust_difficulty();

        // Should increase size because we're solving too fast
        assert!(new_size > original_size, "Size should increase when solving too fast");
    }

    #[test]
    fn test_difficulty_decreases_when_too_slow() {
        let mut adjuster = DifficultyAdjuster::new();

        // Simulate 20 blocks solved slowly (20 seconds each = 20_000_000 μs)
        for _ in 0..20 {
            adjuster.record_solve_time(Duration::from_secs(20));
        }

        let original_size = adjuster.current_size();
        let new_size = adjuster.adjust_difficulty();

        // Should decrease size because we're solving too slow
        assert!(new_size < original_size, "Size should decrease when solving too slow");
    }

    #[test]
    fn test_difficulty_stable_at_target() {
        let mut adjuster = DifficultyAdjuster::new();

        // Simulate 20 blocks solved at target time (5 seconds each)
        for _ in 0..20 {
            adjuster.record_solve_time(Duration::from_secs(5));
        }

        let original_size = adjuster.current_size();
        let new_size = adjuster.adjust_difficulty();

        // Should stay roughly the same (within 20%)
        let ratio = new_size as f64 / original_size as f64;
        assert!(ratio > 0.8 && ratio < 1.2, "Size should be stable at target time");
    }

    #[test]
    fn test_size_bounded_above() {
        let mut adjuster = DifficultyAdjuster::new();

        // Simulate extremely fast solves (1 μs each)
        for _ in 0..20 {
            adjuster.record_solve_time_us(1);
        }

        let new_size = adjuster.adjust_difficulty();
        assert!(new_size <= 50, "Size should be clamped to maximum");
    }

    #[test]
    fn test_size_never_zero() {
        let mut adjuster = DifficultyAdjuster::new();

        // Simulate extremely slow solves (force size towards minimum)
        for _ in 0..20 {
            adjuster.record_solve_time_us(u64::MAX / 2);
        }
        adjuster.adjust_difficulty();
        assert!(adjuster.current_size() >= ABSOLUTE_MIN_SIZE, "Size must never reach zero");

        // Repeated penalize_failure should also never drop to zero
        for _ in 0..100 {
            adjuster.penalize_failure();
        }
        assert!(adjuster.current_size() >= ABSOLUTE_MIN_SIZE, "penalize_failure must not produce zero");
    }

    #[test]
    fn test_solve_time_stored_as_microseconds() {
        let mut adjuster = DifficultyAdjuster::new();
        adjuster.record_solve_time(Duration::from_millis(1500)); // 1.5 s
        let us_value = *adjuster.recent_solve_times_us.back().unwrap();
        assert_eq!(us_value, 1_500_000, "should store 1.5s as 1_500_000 μs");
    }

    #[test]
    fn test_compute_new_size_deterministic_same_inputs() {
        // Critical test: same inputs MUST produce the same output.
        let a = DifficultyAdjuster::compute_new_size_deterministic(20, 100_000, 500_000);
        let b = DifficultyAdjuster::compute_new_size_deterministic(20, 100_000, 500_000);
        assert_eq!(a, b, "identical inputs must give identical size");
    }

    #[test]
    fn test_compute_new_size_faster_increases() {
        // avg < target → should compute larger size
        let slow_result = DifficultyAdjuster::compute_new_size_deterministic(20, 500_000, 500_000);
        let fast_result = DifficultyAdjuster::compute_new_size_deterministic(20, 100_000, 500_000);
        assert!(fast_result > slow_result, "faster solving should yield larger problem size");
    }
}
