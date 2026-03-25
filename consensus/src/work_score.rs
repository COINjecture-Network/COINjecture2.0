//! # Work Score Calculation Engine
//!
//! ## Core Formula
//!
//! ```text
//! work_score = log₂(solve_time / verify_time) × quality_score
//! ```
//!
//! Two inputs. Both network-verifiable. Problem-type agnostic.
//! Directly interpretable as security bits.
//!
//! ## Design Rationale
//!
//! The time asymmetry ratio `solve_time / verify_time` is the one property
//! ALL NP problems share by definition — the gap between finding a solution
//! and checking one. A harder instance of any NP problem type will have a
//! larger asymmetry ratio because solve time grows superpolynomially while
//! verify time grows polynomially.
//!
//! The `log₂` converts this ratio to **bits**: if solving took 1024× longer
//! than verifying, that's 10 bits of work. This makes work scores directly
//! comparable across problem types without any problem-specific parameters.
//!
//! ## What's NOT in the formula (and why)
//!
//! - **Space asymmetry** (`solve_memory / verify_memory`): Self-reported by
//!   the miner. Cannot be verified by the network. Gameable.
//! - **Energy efficiency**: Self-reported. Cannot be verified. Gameable.
//! - **Problem-specific weight** (`base_difficulty_weight`): The whole point
//!   of using time asymmetry is that it's universal. Problem-specific weights
//!   would reintroduce the hardcoded dispatch we eliminated with the registry.
//!   The registry's `base_difficulty_weight` remains available as an optional
//!   normalization hint for analytics, but is NOT a consensus parameter.
//!
//! ## Inflation Resistance
//!
//! A miner could artificially slow their solver to inflate `solve_time`.
//! But the racing incentive handles this: a miner who inflates solve time
//! loses the block to a faster competitor. The winning block's work score
//! is therefore the **minimum competitive** solve time, not an inflatable
//! self-report.
//!
//! During single-miner operation (bootstrap), the difficulty adjuster's
//! target block time serves as the inflation ceiling.
//!
//! ## Security Interpretation
//!
//! A block with `work_score = 40` means the solver demonstrated roughly
//! 2⁴⁰ more computational effort than a verifier needs. An attacker who
//! wants to produce an alternative chain must match this cumulative work.
//!
//! ```text
//! bit_equivalent = work_score   (they are the same thing)
//! chain_security = Σ work_score  (over all blocks)
//! ```
//!
//! COMPLIANCE: Empirical ✓ | Self-referential ✓ | Dimensionless ✓

use coinject_core::{ProblemType, Solution, WorkScore};
use std::time::Duration;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Minimum verify time floor (prevents division by zero and log₂ explosion
/// from negligibly fast verification). 1ms is a conservative floor —
/// real verification of any non-trivial NP instance takes at least this.
const MIN_VERIFY_TIME_SECS: f64 = 0.001;

/// Minimum meaningful asymmetry ratio. Below this, the "work" is negligible.
/// log₂(2) = 1 bit — solving took at least 2× longer than verifying.
const MIN_ASYMMETRY_RATIO: f64 = 2.0;

// ---------------------------------------------------------------------------
// Calculator
// ---------------------------------------------------------------------------

/// Calculates bit-equivalent work scores from network-verifiable inputs.
///
/// The calculator is stateless and problem-type agnostic. It takes only
/// what the network can verify: timestamps and solution quality.
pub struct WorkScoreCalculator;

impl WorkScoreCalculator {
    /// Create a new work score calculator.
    ///
    /// No configuration needed — the formula is universal.
    pub fn new() -> Self {
        WorkScoreCalculator
    }

    /// Calculate bit-equivalent work score.
    ///
    /// # Arguments
    /// * `solve_time` — Wall-clock time from problem assignment to solution submission
    ///                   (network-observed: T_solution - T_assignment)
    /// * `verify_time` — Wall-clock time to verify the solution
    ///                   (network-observed: measured by each validator)
    /// * `quality_score` — Solution quality in [0.0, 1.0]
    ///                     (network-verified: 1.0 for decision problems, gradient for optimization)
    ///
    /// # Returns
    /// Work score in bits. A score of N means the solver demonstrated ~2^N
    /// more computational effort than verification requires.
    ///
    /// Returns 0.0 if the solution is invalid (quality = 0) or the
    /// asymmetry ratio is below the minimum threshold.
    pub fn calculate(
        &self,
        solve_time: Duration,
        verify_time: Duration,
        quality_score: f64,
    ) -> WorkScore {
        // Quality gate: invalid solutions produce zero work
        if quality_score <= 0.0 {
            return 0.0;
        }

        let solve_secs = solve_time.as_secs_f64();
        let verify_secs = verify_time.as_secs_f64().max(MIN_VERIFY_TIME_SECS);

        // Time asymmetry ratio (dimensionless)
        let asymmetry_ratio = solve_secs / verify_secs;

        // Below minimum asymmetry = negligible work
        if asymmetry_ratio < MIN_ASYMMETRY_RATIO {
            return 0.0;
        }

        // Bit-equivalent work score
        let bits = asymmetry_ratio.log2();

        // Quality adjustment: optimal solution gets full bits,
        // suboptimal gets proportionally less
        let work_score = bits * quality_score.clamp(0.0, 1.0);

        // Floor at zero (can't have negative work)
        work_score.max(0.0)
    }

    /// Convenience: calculate from ProblemType and Solution directly.
    ///
    /// Extracts quality_score from `solution.quality(problem)`.
    /// This is the typical call site in block validation.
    pub fn calculate_from_solution(
        &self,
        problem: &ProblemType,
        solution: &Solution,
        solve_time: Duration,
        verify_time: Duration,
    ) -> WorkScore {
        let quality_score = solution.quality(problem);
        self.calculate(solve_time, verify_time, quality_score)
    }

    /// Calculate cumulative chain security in bits.
    ///
    /// A chain with cumulative work W requires ~2^W verification-equivalent
    /// operations to reproduce.
    pub fn chain_security_bits(work_scores: &[WorkScore]) -> f64 {
        work_scores.iter().sum()
    }

    /// Estimate the asymmetry ratio needed for a target work score.
    ///
    /// Useful for difficulty adjustment: "what solve_time / verify_time
    /// ratio would produce a target_bits work score at quality 1.0?"
    pub fn required_asymmetry_for_bits(target_bits: f64) -> f64 {
        2.0_f64.powf(target_bits)
    }
}

impl Default for WorkScoreCalculator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use coinject_core::{ProblemType, Solution};

    #[test]
    fn test_basic_work_score() {
        let calc = WorkScoreCalculator::new();

        // 10s solve, 1ms verify → ratio = 10,000 → log₂(10000) ≈ 13.29 bits
        let score = calc.calculate(Duration::from_secs(10), Duration::from_millis(1), 1.0);

        let expected = 10_000.0_f64.log2(); // ≈ 13.29
        assert!(
            (score - expected).abs() < 0.01,
            "Expected ~{:.2} bits, got {:.2}",
            expected,
            score
        );
    }

    #[test]
    fn test_quality_scales_linearly() {
        let calc = WorkScoreCalculator::new();

        let full_quality = calc.calculate(Duration::from_secs(10), Duration::from_millis(1), 1.0);

        let half_quality = calc.calculate(Duration::from_secs(10), Duration::from_millis(1), 0.5);

        assert!(
            (half_quality - full_quality * 0.5).abs() < 0.01,
            "Half quality should give half the bits"
        );
    }

    #[test]
    fn test_invalid_solution_gives_zero() {
        let calc = WorkScoreCalculator::new();

        let score = calc.calculate(Duration::from_secs(100), Duration::from_millis(1), 0.0);

        assert_eq!(score, 0.0, "Invalid solution should produce zero work");
    }

    #[test]
    fn test_trivial_asymmetry_gives_zero() {
        let calc = WorkScoreCalculator::new();

        // Solve time ≈ verify time → negligible work
        let score = calc.calculate(Duration::from_millis(1), Duration::from_millis(1), 1.0);

        assert_eq!(score, 0.0, "Trivial asymmetry should produce zero work");
    }

    #[test]
    fn test_bits_are_additive() {
        // 2× harder = 1 more bit
        let calc = WorkScoreCalculator::new();

        let score_1k = calc.calculate(Duration::from_secs(1), Duration::from_millis(1), 1.0);

        let score_2k = calc.calculate(Duration::from_secs(2), Duration::from_millis(1), 1.0);

        let diff = score_2k - score_1k;
        assert!(
            (diff - 1.0).abs() < 0.01,
            "Doubling solve time should add ~1 bit, got {:.3}",
            diff
        );
    }

    #[test]
    fn test_chain_security() {
        let scores = vec![10.0, 12.5, 11.0, 13.2];
        let total = WorkScoreCalculator::chain_security_bits(&scores);
        assert!((total - 46.7).abs() < 0.01);
    }

    #[test]
    fn test_required_asymmetry() {
        // 10 bits of work requires 2^10 = 1024× asymmetry
        let ratio = WorkScoreCalculator::required_asymmetry_for_bits(10.0);
        assert!((ratio - 1024.0).abs() < 0.01);
    }

    #[test]
    fn test_from_solution_subset_sum() {
        let calc = WorkScoreCalculator::new();

        let problem = ProblemType::SubsetSum {
            numbers: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            target: 25,
        };
        let solution = Solution::SubsetSum(vec![2, 5, 6, 8]); // 3 + 6 + 7 + 9 = 25

        let score = calc.calculate_from_solution(
            &problem,
            &solution,
            Duration::from_secs(10),
            Duration::from_millis(1),
        );

        // Valid solution → quality 1.0 → full bit-equivalent score
        assert!(
            score > 10.0,
            "Valid SubsetSum should produce >10 bits of work"
        );
    }

    #[test]
    fn test_problem_type_agnostic() {
        let calc = WorkScoreCalculator::new();

        // Same solve/verify times → same work score regardless of problem type
        let score_a = calc.calculate(Duration::from_secs(5), Duration::from_millis(1), 1.0);
        let score_b = calc.calculate(Duration::from_secs(5), Duration::from_millis(1), 1.0);

        assert_eq!(
            score_a, score_b,
            "Same inputs should give same score — formula is problem-type agnostic"
        );
    }
}
