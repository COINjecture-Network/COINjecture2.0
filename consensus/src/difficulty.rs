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
const MIN_TARGET_SOLVE_TIME_SECS: f64 = 1.0;   // Minimum 1 second
const MAX_TARGET_SOLVE_TIME_SECS: f64 = 10.0;  // Maximum 10 seconds
const OPTIMAL_SOLVE_TIME_SECS: f64 = 5.0;      // Target 5 seconds (middle of range)

/// Difficulty adjustment window (last N blocks)
const DIFFICULTY_WINDOW: usize = 20;

/// Minimum problem sizes per type (to prevent degenerate cases)
const MIN_PROBLEM_SIZE: usize = 5;

/// Maximum problem sizes per type (to prevent excessive solve times)
const MAX_SUBSET_SUM_SIZE: usize = 50;
const MAX_SAT_VARIABLES: usize = 100;
const MAX_TSP_CITIES: usize = 25;

/// Difficulty adjuster - tracks solve times and adjusts problem size
pub struct DifficultyAdjuster {
    /// Recent solve times (in seconds)
    recent_solve_times: VecDeque<f64>,
    /// Current problem size (dimensionless)
    current_size: usize,
}

impl DifficultyAdjuster {
    pub fn new() -> Self {
        DifficultyAdjuster {
            recent_solve_times: VecDeque::with_capacity(DIFFICULTY_WINDOW),
            current_size: 20, // Start at size 20 (current testnet default)
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

        // Dimensionless ratio: actual_time / target_time
        let time_ratio = avg_solve_time / OPTIMAL_SOLVE_TIME_SECS;

        // If we're too fast, increase size; if too slow, decrease size
        // Use square root to avoid oscillations (smooth adjustment)
        // new_size = current_size × (target / actual)^0.5
        let scale_factor = (1.0 / time_ratio).sqrt();

        let new_size = (self.current_size as f64 * scale_factor).round() as usize;

        // Clamp to reasonable bounds
        let bounded_size = new_size.max(MIN_PROBLEM_SIZE).min(MAX_SUBSET_SUM_SIZE);

        println!("📊 Difficulty Adjustment:");
        println!("   Avg solve time: {:.3}s (target: {:.1}s)", avg_solve_time, OPTIMAL_SOLVE_TIME_SECS);
        println!("   Time ratio: {:.3}x", time_ratio);
        println!("   Scale factor: {:.3}x", scale_factor);
        println!("   Problem size: {} → {}", self.current_size, bounded_size);

        self.current_size = bounded_size;
        bounded_size
    }

    /// Get problem size for specific problem type
    pub fn size_for_problem_type(&self, problem_type: &str) -> usize {
        match problem_type {
            "SubsetSum" => {
                // SubsetSum: n numbers, complexity ~O(2^n)
                self.current_size.min(MAX_SUBSET_SUM_SIZE)
            }
            "SAT" => {
                // SAT: variables, complexity ~O(2^v)
                // SAT grows exponentially, so use slightly smaller size
                ((self.current_size as f64 * 0.8).round() as usize)
                    .max(MIN_PROBLEM_SIZE)
                    .min(MAX_SAT_VARIABLES)
            }
            "TSP" => {
                // TSP: cities, complexity ~O(n!)
                // TSP grows factorially, so use much smaller size
                ((self.current_size as f64 * 0.4).round() as usize)
                    .max(MIN_PROBLEM_SIZE)
                    .min(MAX_TSP_CITIES)
            }
            _ => self.current_size
        }
    }

    /// Get statistics for monitoring
    pub fn stats(&self) -> DifficultyStats {
        let avg_time = self.moving_average_solve_time();
        let min_time = self.recent_solve_times.iter().copied().fold(f64::INFINITY, f64::min);
        let max_time = self.recent_solve_times.iter().copied().fold(0.0, f64::max);

        DifficultyStats {
            current_size: self.current_size,
            avg_solve_time_secs: avg_time,
            min_solve_time_secs: min_time,
            max_solve_time_secs: max_time,
            sample_count: self.recent_solve_times.len(),
            time_ratio: avg_time / OPTIMAL_SOLVE_TIME_SECS,
        }
    }
}

/// Difficulty statistics
#[derive(Debug, Clone)]
pub struct DifficultyStats {
    pub current_size: usize,
    pub avg_solve_time_secs: f64,
    pub min_solve_time_secs: f64,
    pub max_solve_time_secs: f64,
    pub sample_count: usize,
    pub time_ratio: f64,
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
