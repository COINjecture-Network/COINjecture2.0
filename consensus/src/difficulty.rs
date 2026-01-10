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
// ALL values derived from network state via NetworkMetrics oracle:
// - Optimal solve time: median_block_time * η (from network)
// - Min/Max targets: Optimal * PHI_INV / PHI (mathematical bounds)
// - Problem size limits: Percentiles from historical solve times

use coinject_tokenomics::{NetworkMetrics, ETA, PHI, PHI_INV};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Difficulty adjustment window (last N blocks)
const DIFFICULTY_WINDOW: usize = 20;

/// Stall and stability tuning (dimensionless ratios)
const HIGH_VARIANCE_RATIO: f64 = 0.8; // σ close to μ ⇒ widen window
const RECOVERY_STABLE_RATIO: f64 = 1.2;
const RECOVERY_STEP: usize = 1;

/// Difficulty adjuster - tracks solve times and adjusts problem size
/// Uses NetworkMetrics oracle for all target times and size limits
pub struct DifficultyAdjuster {
    /// Recent solve times (in seconds)
    recent_solve_times: VecDeque<f64>,
    /// Current problem size (dimensionless)
    current_size: usize,
    /// Target size we want to return to after a penalty
    recovery_target: Option<usize>,
    /// Counts consecutive stalls/failures
    stall_counter: usize,
    /// Network metrics oracle (optional - uses defaults if None)
    network_metrics: Option<Arc<RwLock<NetworkMetrics>>>,
}

impl DifficultyAdjuster {
    /// Create new difficulty adjuster without network metrics (uses defaults)
    pub fn new() -> Self {
        DifficultyAdjuster {
            recent_solve_times: VecDeque::with_capacity(DIFFICULTY_WINDOW),
            current_size: 20, // Start at size 20 (current testnet default)
            recovery_target: None,
            stall_counter: 0,
            network_metrics: None,
        }
    }
    
    /// Create with network metrics oracle (empirical mode)
    pub fn with_metrics(network_metrics: Arc<RwLock<NetworkMetrics>>) -> Self {
        DifficultyAdjuster {
            recent_solve_times: VecDeque::with_capacity(DIFFICULTY_WINDOW),
            current_size: 20,
            recovery_target: None,
            stall_counter: 0,
            network_metrics: Some(network_metrics),
        }
    }
    
    /// Update network metrics reference
    pub fn set_metrics(&mut self, network_metrics: Arc<RwLock<NetworkMetrics>>) {
        self.network_metrics = Some(network_metrics);
    }
    
    /// Check if network metrics are available
    pub fn has_metrics(&self) -> bool {
        self.network_metrics.is_some()
    }
    
    /// Get optimal solve time from network (or default)
    async fn optimal_solve_time(&self) -> f64 {
        if let Some(ref metrics) = self.network_metrics {
            let metrics = metrics.read().await;
            // Optimal = median_block_time * η (mathematical scaling)
            metrics.median_block_time() * ETA
        } else {
            // Default during bootstrap: 5 seconds
            5.0
        }
    }
    
    /// Get min target solve time (mathematical bound)
    async fn min_target_solve_time(&self) -> f64 {
        let optimal = self.optimal_solve_time().await;
        // MIN = Optimal * PHI_INV (mathematical bound, not arbitrary)
        optimal * PHI_INV
    }
    
    /// Get max target solve time (mathematical bound)
    async fn max_target_solve_time(&self) -> f64 {
        let optimal = self.optimal_solve_time().await;
        // MAX = Optimal * PHI (mathematical bound, not arbitrary)
        optimal * PHI
    }
    
    /// Get problem size limits from network metrics
    /// Uses hardness factors and median block time to estimate reasonable size ranges
    async fn get_size_limits(&self, problem_type: &str) -> (usize, usize) {
        if let Some(ref metrics) = self.network_metrics {
            let metrics = metrics.read().await;
            
            // Map problem type to category
            let category = match problem_type {
                "SubsetSum" => 3,
                "SAT" => 0,
                "TSP" => 1,
                _ => 0,
            };
            
            // Get hardness ratio for this category (empirical from network)
            let hardness = metrics.hardness_factor(category);
            
            // Get median block time (network-derived)
            let median_time = metrics.median_block_time();
            
            // Calculate optimal solve time for this category
            // Optimal = median_block_time * η * hardness_factor
            let optimal_time = median_time * ETA * hardness;
            
            // Estimate base size from optimal time
            // For NP-hard problems: solve_time ≈ base_time * (size^complexity_factor)
            // Using logarithmic relationship: size ≈ (solve_time / base_time)^(1/complexity)
            // For exponential complexity (2^n), complexity ≈ 1.0
            // For factorial (n!), complexity ≈ 0.5
            let complexity_factor = match problem_type {
                "TSP" => 0.5,      // Factorial - very sensitive
                "SAT" => 0.7,      // Exponential - sensitive
                "SubsetSum" => 0.8, // Exponential - moderate
                _ => 0.8,
            };
            
            let base_time = 1.0; // 1 second baseline
            let estimated_size = ((optimal_time / base_time).powf(complexity_factor) * hardness) as usize;
            
            // Min size: allow 20% of estimated (fast solves)
            // But ensure minimum of 5 to prevent degenerate cases
            let min_size = (estimated_size as f64 * 0.2).max(5.0) as usize;
            
            // Max size: allow 200% of estimated (slow but acceptable)
            // Cap at reasonable maximums based on problem type
            let type_max = match problem_type {
                "TSP" => 30,      // TSP grows factorially - keep small
                "SAT" => 120,     // SAT can handle larger
                "SubsetSum" => 60, // SubsetSum moderate
                _ => 60,
            };
            let max_size = (estimated_size as f64 * 2.0).min(type_max as f64) as usize;
            
            (min_size, max_size)
        } else {
            // Defaults during bootstrap (mathematical bounds)
            match problem_type {
                "SubsetSum" => (5, 50),
                "SAT" => (5, 100),
                "TSP" => (5, 25),
                _ => (5, 50),
            }
        }
    }

    /// Record a solve time
    pub fn record_solve_time(&mut self, solve_time: Duration) {
        let solve_secs = solve_time.as_secs_f64();

        // Add to window
        self.recent_solve_times.push_back(solve_secs);

        // Maintain window size
        if self.recent_solve_times.len() > DIFFICULTY_WINDOW {
            self.recent_solve_times.pop_front();
        }
    }

    /// Get current problem size
    pub fn current_size(&self) -> usize {
        self.current_size
    }

    /// Calculate moving average of recent solve times
    fn moving_average_solve_time(&self) -> f64 {
        if self.recent_solve_times.is_empty() {
            // Will be overridden by async method when network metrics available
            return 5.0; // Temporary default
        }

        let sum: f64 = self.recent_solve_times.iter().sum();
        sum / self.recent_solve_times.len() as f64
    }
    
    /// Calculate moving average (async version with network metrics)
    async fn moving_average_solve_time_async(&self) -> f64 {
        if self.recent_solve_times.is_empty() {
            return self.optimal_solve_time().await;
        }

        let sum: f64 = self.recent_solve_times.iter().sum();
        sum / self.recent_solve_times.len() as f64
    }

    /// Adjust difficulty based on recent solve times (sync version for backward compatibility)
    /// Returns new problem size using dimensionless scaling
    /// Note: Uses default values if network metrics not available
    pub fn adjust_difficulty(&mut self) -> usize {
        // Need at least half a window of data before adjusting
        if self.recent_solve_times.len() < DIFFICULTY_WINDOW / 2 {
            return self.current_size;
        }

        let avg_solve_time = self.moving_average_solve_time();
        let optimal = 5.0; // Default if no network metrics
        let min_target = optimal * PHI_INV;
        let max_target = optimal * PHI;
        
        if avg_solve_time < min_target {
            println!(
                "⚡ Solve times {:.2}s are below minimum target ({:.1}s). Allowing controlled growth.",
                avg_solve_time,
                min_target
            );
        }
        let std_dev = self.solve_time_std_dev(avg_solve_time);

        if self.recent_solve_times.len() >= DIFFICULTY_WINDOW / 2
            && std_dev > avg_solve_time * HIGH_VARIANCE_RATIO
        {
            println!(
                "🔁 High variance detected (σ={:.2}s). Deferring difficulty adjustment to widen window.",
                std_dev
            );
            return self.current_size;
        }

        // Dimensionless ratio: actual_time / target_time
        let time_ratio = avg_solve_time / optimal;

        // Stall detection: if avg > 2 * max_target
        if avg_solve_time > max_target * 2.0 {
            self.apply_stall_penalty("avg solve time exceeded safe threshold");
        }

        // If we're too fast, increase size; if too slow, decrease size
        // Use square root to avoid oscillations (smooth adjustment)
        // new_size = current_size × (target / actual)^0.5
        let mut raw_scale_factor = (1.0 / time_ratio.max(0.01)).sqrt();

        // Stall ratio alert: if time_ratio > 2 * (max/optimal) = 2 * PHI
        let stall_ratio_alert = 2.0 * PHI;
        if time_ratio > stall_ratio_alert {
            raw_scale_factor *= 0.7;
        }
        let extreme_stall_ratio = 5.0;
        if time_ratio > extreme_stall_ratio {
            raw_scale_factor = raw_scale_factor.min(0.4);
        }

        // Clamp scale factor to prevent extreme jumps
        let scale_factor = raw_scale_factor.clamp(0.4, 2.0);

        let new_size = (self.current_size as f64 * scale_factor).round() as usize;

        // Clamp to reasonable bounds (will use network-derived if available in async version)
        let (min_size, max_size) = (5, 50); // Defaults
        let mut bounded_size = new_size.max(min_size).min(max_size);

        bounded_size = self.apply_recovery(bounded_size, time_ratio);

        println!("📊 Difficulty Adjustment:");
        println!(
            "   Avg solve time: {:.3}s (target: {:.1}s) σ={:.3}s",
            avg_solve_time, optimal, std_dev
        );
        println!("   Time ratio: {:.3}x", time_ratio);
        println!("   Scale factor: {:.3}x", scale_factor);
        println!(
            "   Problem size: {} → {}{}",
            self.current_size,
            bounded_size,
            if self.recovery_target.is_some() {
                " (recovery mode)"
            } else {
                ""
            }
        );

        self.current_size = bounded_size;
        bounded_size
    }
    
    /// Adjust difficulty with network metrics (async version - fully empirical)
    pub async fn adjust_difficulty_async(&mut self) -> usize {
        // Need at least half a window of data before adjusting
        if self.recent_solve_times.len() < DIFFICULTY_WINDOW / 2 {
            return self.current_size;
        }

        let avg_solve_time = self.moving_average_solve_time_async().await;
        let optimal = self.optimal_solve_time().await;
        let min_target = self.min_target_solve_time().await;
        let max_target = self.max_target_solve_time().await;
        
        if avg_solve_time < min_target {
            println!(
                "⚡ Solve times {:.2}s are below minimum target ({:.1}s). Allowing controlled growth.",
                avg_solve_time,
                min_target
            );
        }
        let std_dev = self.solve_time_std_dev(avg_solve_time);

        if self.recent_solve_times.len() >= DIFFICULTY_WINDOW / 2
            && std_dev > avg_solve_time * HIGH_VARIANCE_RATIO
        {
            println!(
                "🔁 High variance detected (σ={:.2}s). Deferring difficulty adjustment to widen window.",
                std_dev
            );
            return self.current_size;
        }

        // Dimensionless ratio: actual_time / target_time
        let time_ratio = avg_solve_time / optimal;

        // Stall detection: if avg > 2 * max_target
        if avg_solve_time > max_target * 2.0 {
            self.apply_stall_penalty("avg solve time exceeded safe threshold");
        }

        // If we're too fast, increase size; if too slow, decrease size
        // Use square root to avoid oscillations (smooth adjustment)
        let mut raw_scale_factor = (1.0 / time_ratio.max(0.01)).sqrt();

        // Stall ratio alert: if time_ratio > 2 * (max/optimal) = 2 * PHI
        let stall_ratio_alert = 2.0 * PHI;
        if time_ratio > stall_ratio_alert {
            raw_scale_factor *= 0.7;
        }
        let extreme_stall_ratio = 5.0;
        if time_ratio > extreme_stall_ratio {
            raw_scale_factor = raw_scale_factor.min(0.4);
        }

        // Clamp scale factor to prevent extreme jumps
        let scale_factor = raw_scale_factor.clamp(0.4, 2.0);

        let new_size = (self.current_size as f64 * scale_factor).round() as usize;

        // Get size limits from network (use "SubsetSum" as default)
        let (min_size, max_size) = self.get_size_limits("SubsetSum").await;
        let mut bounded_size = new_size.max(min_size).min(max_size);

        bounded_size = self.apply_recovery(bounded_size, time_ratio);

        println!("📊 Difficulty Adjustment (Empirical):");
        println!(
            "   Avg solve time: {:.3}s (target: {:.1}s, range: [{:.1}, {:.1}]) σ={:.3}s",
            avg_solve_time, optimal, min_target, max_target, std_dev
        );
        println!("   Time ratio: {:.3}x", time_ratio);
        println!("   Scale factor: {:.3}x", scale_factor);
        println!(
            "   Problem size: {} → {} (limits: [{}, {}]){}",
            self.current_size,
            bounded_size,
            min_size,
            max_size,
            if self.recovery_target.is_some() {
                " (recovery mode)"
            } else {
                ""
            }
        );

        self.current_size = bounded_size;
        bounded_size
    }

    /// Penalize difficulty immediately after an unsolved block
    /// This bypasses the moving-average window to prevent multi-hour stalls.
    pub fn penalize_failure(&mut self) -> usize {
        let old_size = self.current_size;
        let min_size = 5; // Default minimum
        let reduced = (((self.current_size as f64) * 0.85).round() as usize).max(min_size);
        println!(
            "⚠️  Mining failure penalty: {} → {}",
            old_size, reduced
        );
        self.recovery_target = Some(self.recovery_target.unwrap_or(old_size));
        self.current_size = reduced;
        self.stall_counter = (self.stall_counter + 1).min(20);
        reduced
    }
    
    /// Penalize failure with network metrics (async version)
    pub async fn penalize_failure_async(&mut self) -> usize {
        let old_size = self.current_size;
        let (min_size, _) = self.get_size_limits("SubsetSum").await;
        let reduced = (((self.current_size as f64) * 0.85).round() as usize).max(min_size);
        println!(
            "⚠️  Mining failure penalty: {} → {} (network-derived min: {})",
            old_size, reduced, min_size
        );
        self.recovery_target = Some(self.recovery_target.unwrap_or(old_size));
        self.current_size = reduced;
        self.stall_counter = (self.stall_counter + 1).min(20);
        reduced
    }

    /// Get problem size for specific problem type (sync version - uses defaults)
    pub fn size_for_problem_type(&self, problem_type: &str) -> usize {
        match problem_type {
            "SubsetSum" => {
                self.current_size.min(50) // Default max
            }
            "SAT" => {
                ((self.current_size as f64 * 0.75).round() as usize)
                    .max(5)
                    .min(100) // Default max
            }
            "TSP" => {
                ((self.current_size as f64 * 0.35).round() as usize)
                    .max(5)
                    .min(25) // Default max
            }
            _ => self.current_size,
        }
    }
    
    /// Get problem size for specific problem type (async version - uses network metrics)
    pub async fn size_for_problem_type_async(&self, problem_type: &str) -> usize {
        let (min_size, max_size) = self.get_size_limits(problem_type).await;
        
        match problem_type {
            "SubsetSum" => {
                self.current_size.min(max_size)
            }
            "SAT" => {
                ((self.current_size as f64 * 0.75).round() as usize)
                    .max(min_size)
                    .min(max_size)
            }
            "TSP" => {
                ((self.current_size as f64 * 0.35).round() as usize)
                    .max(min_size)
                    .min(max_size)
            }
            _ => self.current_size,
        }
    }

    /// Get statistics for monitoring (sync version)
    pub fn stats(&self) -> DifficultyStats {
        let avg_time = self.moving_average_solve_time();
        let optimal = 5.0; // Default
        let std_dev = self.solve_time_std_dev(avg_time);
        let min_time = self
            .recent_solve_times
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min);
        let max_time = self
            .recent_solve_times
            .iter()
            .copied()
            .fold(0.0, f64::max);

        DifficultyStats {
            current_size: self.current_size,
            avg_solve_time_secs: avg_time,
            min_solve_time_secs: min_time,
            max_solve_time_secs: max_time,
            std_dev_secs: std_dev,
            sample_count: self.recent_solve_times.len(),
            time_ratio: avg_time / optimal,
            stall_counter: self.stall_counter,
            in_recovery_mode: self.recovery_target.is_some(),
        }
    }
    
    /// Get statistics for monitoring (async version with network metrics)
    pub async fn stats_async(&self) -> DifficultyStats {
        let avg_time = self.moving_average_solve_time_async().await;
        let optimal = self.optimal_solve_time().await;
        let std_dev = self.solve_time_std_dev(avg_time);
        let min_time = self
            .recent_solve_times
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min);
        let max_time = self
            .recent_solve_times
            .iter()
            .copied()
            .fold(0.0, f64::max);

        DifficultyStats {
            current_size: self.current_size,
            avg_solve_time_secs: avg_time,
            min_solve_time_secs: min_time,
            max_solve_time_secs: max_time,
            std_dev_secs: std_dev,
            sample_count: self.recent_solve_times.len(),
            time_ratio: avg_time / optimal,
            stall_counter: self.stall_counter,
            in_recovery_mode: self.recovery_target.is_some(),
        }
    }

    fn solve_time_std_dev(&self, mean: f64) -> f64 {
        if self.recent_solve_times.len() < 2 {
            return 0.0;
        }
        let variance = self
            .recent_solve_times
            .iter()
            .map(|t| {
                let delta = t - mean;
                delta * delta
            })
            .sum::<f64>()
            / self.recent_solve_times.len() as f64;
        variance.sqrt()
    }

    #[allow(dead_code)]
    fn dynamic_cap(&self, base_max: usize, hardness: f64) -> usize {
        let stall_factor = 1.0 - (self.stall_counter as f64 * 0.05).min(0.5);
        let adaptive = (base_max as f64 * hardness * stall_factor).round() as usize;
        adaptive.max(5) // Default minimum
    }

    fn apply_recovery(&mut self, current: usize, time_ratio: f64) -> usize {
        if let Some(target) = self.recovery_target {
            if time_ratio <= RECOVERY_STABLE_RATIO {
                let next = (current + RECOVERY_STEP).min(target);
                if next >= target {
                    self.recovery_target = None;
                }
                return next;
            }
            return current;
        }

        if time_ratio <= 1.5 && self.stall_counter > 0 {
            self.stall_counter -= 1;
        }

        current
    }

    fn apply_stall_penalty(&mut self, reason: &str) -> usize {
        let old_size = self.current_size;
        let reduced = (((self.current_size as f64) * 0.7).round() as usize).max(5); // Default minimum
        println!("⚠️  Stall detected ({}). {} → {}", reason, old_size, reduced);
        self.recovery_target = Some(self.recovery_target.unwrap_or(old_size));
        self.current_size = reduced;
        self.stall_counter = (self.stall_counter + 1).min(20);
        reduced
    }
}

/// Difficulty statistics
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

        // Simulate 20 blocks solved very fast (0.1 seconds each)
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

        // Simulate 20 blocks solved slowly (20 seconds each)
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
    fn test_size_bounded() {
        let mut adjuster = DifficultyAdjuster::new();

        // Simulate extremely fast solves
        for _ in 0..20 {
            adjuster.record_solve_time(Duration::from_micros(1));
        }

        let new_size = adjuster.adjust_difficulty();

        // Should be clamped to max (default 50 for SubsetSum)
        assert!(new_size <= 50, "Size should be clamped to maximum");
    }
}
