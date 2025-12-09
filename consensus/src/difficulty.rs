// Dynamic Difficulty Adjustment
// Scales problem size to maintain target solve times for meaningful asymmetry
//
// Principles (from whitepaper):
// 1. Dimensionless - Pure ratios, no arbitrary limits
// 2. Self-Referenced - Measured against network's own state
// 3. Empirically Grounded - Derived from actual solve times

use std::collections::VecDeque;
use std::time::Duration;

/// Target solve time range for meaningful asymmetry
const MIN_TARGET_SOLVE_TIME_SECS: f64 = 1.0; // Minimum 1 second
const MAX_TARGET_SOLVE_TIME_SECS: f64 = 10.0; // Maximum 10 seconds
const OPTIMAL_SOLVE_TIME_SECS: f64 = 5.0; // Target 5 seconds (middle of range)

/// Difficulty adjustment window (last N blocks)
const DIFFICULTY_WINDOW: usize = 20;

/// Minimum problem sizes per type (to prevent degenerate cases)
const MIN_PROBLEM_SIZE: usize = 5;

/// Maximum problem sizes per type (to prevent excessive solve times)
const MAX_SUBSET_SUM_SIZE: usize = 50;
const MAX_SAT_VARIABLES: usize = 100;
const MAX_TSP_CITIES: usize = 25;

/// Stall and stability tuning
const STALL_RATIO_ALERT: f64 =
    (MAX_TARGET_SOLVE_TIME_SECS * 2.0) / OPTIMAL_SOLVE_TIME_SECS; // Avg > 20s
const EXTREME_STALL_RATIO: f64 = 5.0; // Severe stalls
const HIGH_VARIANCE_RATIO: f64 = 0.8; // σ close to μ ⇒ widen window
const RECOVERY_STABLE_RATIO: f64 = 1.2;
const RECOVERY_STEP: usize = 1;

/// Difficulty adjuster - tracks solve times and adjusts problem size
pub struct DifficultyAdjuster {
    /// Recent solve times (in seconds)
    recent_solve_times: VecDeque<f64>,
    /// Current problem size (dimensionless)
    current_size: usize,
    /// Target size we want to return to after a penalty
    recovery_target: Option<usize>,
    /// Counts consecutive stalls/failures
    stall_counter: usize,
}

impl DifficultyAdjuster {
    pub fn new() -> Self {
        DifficultyAdjuster {
            recent_solve_times: VecDeque::with_capacity(DIFFICULTY_WINDOW),
            current_size: 20, // Start at size 20 (current testnet default)
            recovery_target: None,
            stall_counter: 0,
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
            return OPTIMAL_SOLVE_TIME_SECS; // Default if no data
        }

        let sum: f64 = self.recent_solve_times.iter().sum();
        sum / self.recent_solve_times.len() as f64
    }

    /// Adjust difficulty based on recent solve times
    /// Returns new problem size using dimensionless scaling
    pub fn adjust_difficulty(&mut self) -> usize {
        // Need at least half a window of data before adjusting
        if self.recent_solve_times.len() < DIFFICULTY_WINDOW / 2 {
            return self.current_size;
        }

        let avg_solve_time = self.moving_average_solve_time();
        if avg_solve_time < MIN_TARGET_SOLVE_TIME_SECS {
            println!(
                "⚡ Solve times {:.2}s are below minimum target ({:.1}s). Allowing controlled growth.",
                avg_solve_time,
                MIN_TARGET_SOLVE_TIME_SECS
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
        let time_ratio = avg_solve_time / OPTIMAL_SOLVE_TIME_SECS;

        if avg_solve_time > MAX_TARGET_SOLVE_TIME_SECS * 2.0 {
            self.apply_stall_penalty("avg solve time exceeded safe threshold");
        }

        // If we're too fast, increase size; if too slow, decrease size
        // Use square root to avoid oscillations (smooth adjustment)
        // new_size = current_size × (target / actual)^0.5
        let mut raw_scale_factor = (1.0 / time_ratio.max(0.01)).sqrt();

        if time_ratio > STALL_RATIO_ALERT {
            raw_scale_factor *= 0.7;
        }
        if time_ratio > EXTREME_STALL_RATIO {
            raw_scale_factor = raw_scale_factor.min(0.4);
        }

        // Clamp scale factor to prevent extreme jumps
        let scale_factor = raw_scale_factor.clamp(0.4, 2.0);

        let new_size = (self.current_size as f64 * scale_factor).round() as usize;

        // Clamp to reasonable bounds
        let subset_cap = self.dynamic_cap(MAX_SUBSET_SUM_SIZE, 1.0);
        let mut bounded_size = new_size.max(MIN_PROBLEM_SIZE).min(subset_cap);

        bounded_size = self.apply_recovery(bounded_size, time_ratio);

        println!("📊 Difficulty Adjustment:");
        println!(
            "   Avg solve time: {:.3}s (target: {:.1}s) σ={:.3}s",
            avg_solve_time, OPTIMAL_SOLVE_TIME_SECS, std_dev
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

    /// Penalize difficulty immediately after an unsolved block
    /// This bypasses the moving-average window to prevent multi-hour stalls.
    pub fn penalize_failure(&mut self) -> usize {
        let old_size = self.current_size;
        let reduced = (((self.current_size as f64) * 0.85).round() as usize).max(MIN_PROBLEM_SIZE);
        println!(
            "⚠️  Mining failure penalty: {} → {}",
            old_size, reduced
        );
        self.recovery_target = Some(self.recovery_target.unwrap_or(old_size));
        self.current_size = reduced;
        self.stall_counter = (self.stall_counter + 1).min(20);
        reduced
    }

    /// Get problem size for specific problem type
    pub fn size_for_problem_type(&self, problem_type: &str) -> usize {
        match problem_type {
            "SubsetSum" => {
                // SubsetSum: n numbers, complexity ~O(2^n)
                self.current_size.min(self.dynamic_cap(MAX_SUBSET_SUM_SIZE, 1.0))
            }
            "SAT" => {
                // SAT grows exponentially, so keep it tighter
                let sat_cap = self.dynamic_cap(MAX_SAT_VARIABLES, 0.9);
                ((self.current_size as f64 * 0.75).round() as usize)
                    .max(MIN_PROBLEM_SIZE)
                    .min(sat_cap)
            }
            "TSP" => {
                // TSP grows factorially, so enforce very small caps
                let tsp_cap = self.dynamic_cap(MAX_TSP_CITIES, 0.5);
                ((self.current_size as f64 * 0.35).round() as usize)
                    .max(MIN_PROBLEM_SIZE)
                    .min(tsp_cap)
            }
            _ => self.current_size,
        }
    }

    /// Get statistics for monitoring
    pub fn stats(&self) -> DifficultyStats {
        let avg_time = self.moving_average_solve_time();
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
            time_ratio: avg_time / OPTIMAL_SOLVE_TIME_SECS,
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

    fn dynamic_cap(&self, base_max: usize, hardness: f64) -> usize {
        let stall_factor = 1.0 - (self.stall_counter as f64 * 0.05).min(0.5);
        let adaptive = (base_max as f64 * hardness * stall_factor).round() as usize;
        adaptive.max(MIN_PROBLEM_SIZE)
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
        let reduced = (((self.current_size as f64) * 0.7).round() as usize).max(MIN_PROBLEM_SIZE);
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

        // Should be clamped to max
        assert!(new_size <= MAX_SUBSET_SUM_SIZE, "Size should be clamped to maximum");
    }
}
