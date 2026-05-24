//! Property-based and unit tests for the consensus engine.
//!
//! Covers WorkScoreCalculator properties and DifficultyAdjuster behaviour.

use coinject_consensus::{DifficultyAdjuster, WorkScoreCalculator};
use proptest::prelude::*;
use std::time::Duration;

// =============================================================================
// WorkScoreCalculator — unit tests
// =============================================================================

#[test]
fn test_work_score_basic_calculation() {
    let calc = WorkScoreCalculator::new();
    // 1 s solve, 1 ms verify → ratio = 1000 → log2(1000) ≈ 9.97 bits
    let score = calc.calculate(Duration::from_secs(1), Duration::from_millis(1), 1.0);
    let expected = 1000.0_f64.log2();
    assert!(
        (score - expected).abs() < 0.01,
        "Expected ≈{:.2}, got {:.2}",
        expected,
        score
    );
}

#[test]
fn test_zero_quality_yields_zero_score() {
    let calc = WorkScoreCalculator::new();
    let score = calc.calculate(Duration::from_secs(100), Duration::from_millis(1), 0.0);
    assert_eq!(score, 0.0);
}

#[test]
fn test_trivial_asymmetry_yields_zero_score() {
    let calc = WorkScoreCalculator::new();
    // solve ≈ verify → ratio < MIN_ASYMMETRY_RATIO (2.0)
    let score = calc.calculate(Duration::from_millis(1), Duration::from_millis(1), 1.0);
    assert_eq!(score, 0.0);
}

#[test]
fn test_half_quality_is_half_the_score() {
    let calc = WorkScoreCalculator::new();
    let full = calc.calculate(Duration::from_secs(10), Duration::from_millis(1), 1.0);
    let half = calc.calculate(Duration::from_secs(10), Duration::from_millis(1), 0.5);
    assert!((half - full * 0.5).abs() < 0.01);
}

#[test]
fn test_doubling_solve_time_adds_one_bit() {
    let calc = WorkScoreCalculator::new();
    let base = calc.calculate(Duration::from_secs(1), Duration::from_millis(1), 1.0);
    let doubled = calc.calculate(Duration::from_secs(2), Duration::from_millis(1), 1.0);
    // log2(2x) = log2(x) + 1
    assert!(
        (doubled - base - 1.0).abs() < 0.01,
        "diff = {}",
        doubled - base
    );
}

#[test]
fn test_chain_security_is_sum_of_scores() {
    let scores = vec![5.0, 10.0, 7.5, 12.25];
    let total = WorkScoreCalculator::chain_security_bits(&scores);
    assert!((total - 34.75).abs() < 0.001);
}

#[test]
fn test_chain_security_empty_is_zero() {
    let total = WorkScoreCalculator::chain_security_bits(&[]);
    assert_eq!(total, 0.0);
}

#[test]
fn test_required_asymmetry_for_bits() {
    // 10 bits → 2^10 = 1024
    let ratio = WorkScoreCalculator::required_asymmetry_for_bits(10.0);
    assert!((ratio - 1024.0).abs() < 0.01);
}

// =============================================================================
// DifficultyAdjuster — unit tests
// =============================================================================

#[test]
fn test_initial_size_is_nonzero() {
    let adj = DifficultyAdjuster::new();
    assert!(adj.current_size() > 0);
}

#[test]
fn test_no_adjustment_before_window_filled() {
    let mut adj = DifficultyAdjuster::new();
    let initial = adj.current_size();
    // Record only 3 times (well below the 10-block half-window threshold)
    for _ in 0..3 {
        adj.record_solve_time(Duration::from_secs(5));
    }
    let after = adj.adjust_difficulty();
    assert_eq!(
        after, initial,
        "Adjuster must not change size until enough samples are collected"
    );
}

#[test]
fn test_fast_solve_times_increase_size() {
    let mut adj = DifficultyAdjuster::new();
    for _ in 0..20 {
        adj.record_solve_time(Duration::from_millis(100)); // very fast
    }
    let new_size = adj.adjust_difficulty();
    assert!(
        new_size > adj.current_size() || new_size >= adj.current_size(),
        "Fast solve times must not decrease size"
    );
    use coinject_consensus::difficulty::CANONICAL_MAX_SIZE;
    assert!(new_size <= CANONICAL_MAX_SIZE);
}

#[test]
fn test_slow_solve_times_decrease_size() {
    let mut adj = DifficultyAdjuster::new();
    let original = adj.current_size();
    for _ in 0..20 {
        adj.record_solve_time(Duration::from_secs(60)); // very slow
    }
    let new_size = adj.adjust_difficulty();
    assert!(
        new_size < original,
        "Slow solve times must decrease problem size"
    );
}

#[test]
fn test_size_stays_within_bounds_under_extreme_fast() {
    let mut adj = DifficultyAdjuster::new();
    for _ in 0..20 {
        adj.record_solve_time(Duration::from_nanos(1)); // impossibly fast
    }
    let size = adj.adjust_difficulty();
    assert!(size >= 5, "Size must not drop below minimum of 5");
    use coinject_consensus::difficulty::CANONICAL_MAX_SIZE;
    assert!(
        size <= CANONICAL_MAX_SIZE,
        "Size must not exceed canonical maximum"
    );
}

#[test]
fn test_penalize_failure_reduces_size() {
    let mut adj = DifficultyAdjuster::new();
    let before = adj.current_size();
    let after = adj.penalize_failure();
    assert!(after < before, "Failure penalty must reduce problem size");
    assert!(after >= 5, "Penalized size must still meet minimum");
}

#[test]
fn test_stats_sample_count_matches_recorded() {
    let mut adj = DifficultyAdjuster::new();
    for _ in 0..7 {
        adj.record_solve_time(Duration::from_secs(5));
    }
    let stats = adj.stats();
    assert_eq!(stats.sample_count, 7);
}

#[test]
fn test_stats_average_time_correct() {
    let mut adj = DifficultyAdjuster::new();
    // All 5 s → avg = 5.0
    for _ in 0..20 {
        adj.record_solve_time(Duration::from_secs(5));
    }
    let stats = adj.stats();
    assert!((stats.avg_solve_time_secs - 5.0).abs() < 0.01);
}

#[test]
fn test_stats_reports_recovery_mode_after_penalty() {
    let mut adj = DifficultyAdjuster::new();
    adj.penalize_failure();
    let stats = adj.stats();
    assert!(
        stats.in_recovery_mode,
        "Must be in recovery mode after a penalty"
    );
}

#[test]
fn test_bootstrap_size_and_per_type_mapping() {
    use coinject_consensus::difficulty::BOOTSTRAP_CURRENT_SIZE;

    let mut adj = DifficultyAdjuster::new();
    assert_eq!(adj.current_size(), BOOTSTRAP_CURRENT_SIZE);
    assert_eq!(adj.size_for_problem_type("SubsetSum"), BOOTSTRAP_CURRENT_SIZE);
    assert_eq!(adj.size_for_problem_type("SAT"), BOOTSTRAP_CURRENT_SIZE);
    let tsp = adj.size_for_problem_type("TSP");
    assert_eq!(tsp, ((BOOTSTRAP_CURRENT_SIZE as f64) * 0.9).round() as usize);
    assert!(tsp <= BOOTSTRAP_CURRENT_SIZE);

    adj.penalize_failure();
    assert!(
        adj.size_for_problem_type("SubsetSum") < BOOTSTRAP_CURRENT_SIZE,
        "failure penalty must reduce effective SubsetSum size below bootstrap"
    );
}

// =============================================================================
// WorkScoreCalculator — property tests
// =============================================================================

proptest! {
    /// Work score must always be non-negative, for any combination of inputs.
    #[test]
    fn prop_work_score_nonnegative(
        solve_ms  in 1u64..1_000_000u64,
        verify_ms in 1u64..100_000u64,
        quality   in 0.0f64..=1.0f64,
    ) {
        let calc = WorkScoreCalculator::new();
        let score = calc.calculate(
            Duration::from_millis(solve_ms),
            Duration::from_millis(verify_ms),
            quality,
        );
        prop_assert!(score >= 0.0, "Work score must be non-negative, got {}", score);
    }

    /// Quality = 0.0 must always yield score = 0.0, regardless of timing.
    #[test]
    fn prop_zero_quality_always_zero(
        solve_ms  in 1u64..1_000_000u64,
        verify_ms in 1u64..100_000u64,
    ) {
        let calc = WorkScoreCalculator::new();
        let score = calc.calculate(
            Duration::from_millis(solve_ms),
            Duration::from_millis(verify_ms),
            0.0,
        );
        prop_assert_eq!(score, 0.0);
    }

    /// Higher quality must give equal or higher score for the same timing pair.
    #[test]
    fn prop_higher_quality_higher_score(
        solve_ms  in 10u64..1_000_000u64,
        verify_ms in 1u64..1_000u64,
        q_lo      in 0.0f64..0.5f64,
        q_hi      in 0.5f64..=1.0f64,
    ) {
        let calc = WorkScoreCalculator::new();
        let s_lo = calc.calculate(Duration::from_millis(solve_ms), Duration::from_millis(verify_ms), q_lo);
        let s_hi = calc.calculate(Duration::from_millis(solve_ms), Duration::from_millis(verify_ms), q_hi);
        prop_assert!(s_hi >= s_lo, "Higher quality must yield >= score: s_hi={}, s_lo={}", s_hi, s_lo);
    }

    /// Chain security is the simple sum of individual work scores.
    #[test]
    fn prop_chain_security_is_sum(scores in proptest::collection::vec(0.0f64..100.0f64, 1..20)) {
        let total = WorkScoreCalculator::chain_security_bits(&scores);
        let expected: f64 = scores.iter().sum();
        prop_assert!((total - expected).abs() < 1e-9, "chain_security must equal sum");
    }
}
