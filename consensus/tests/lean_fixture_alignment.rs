//! Tier C: alignment between Rust consensus/tokenomics and Lean 4 specs (`lean4/Coinjecture/Fixtures.lean`).
//!
//! Run: `cargo test -p coinject-consensus lean_fixture`
//! Full gate: `./scripts/verify-formal-fixtures.sh`

use coinject_consensus::WorkScoreCalculator;
use coinject_core::fixed_point::{self, apply_quality, log2_ratio, SCALE};
use coinject_tokenomics::rewards::{
    header_work_score_trunc_u128, RewardCalculator, REWARD_EMISSION_MULTIPLIER,
    REWARD_FIXED_POINT_SCALE,
};

/// `Coinjecture.Fixtures.log2TenToOne`
const LOG2_TEN_TO_ONE: u64 = 3_250_000;

/// `Coinjecture.Fixtures.log2FourToOne` = 2 * fpScale
const LOG2_FOUR_TO_ONE: u64 = 2 * SCALE;

#[test]
fn lean_fixture_log2_ratio_ten_to_one() {
    assert_eq!(log2_ratio(10, 1), Some(LOG2_TEN_TO_ONE));
}

#[test]
fn lean_fixture_log2_ratio_four_to_one() {
    assert_eq!(log2_ratio(4, 1), Some(LOG2_FOUR_TO_ONE));
}

#[test]
fn lean_fixture_work_score_deterministic_ten_one() {
    let calc = WorkScoreCalculator::new();
    let score = calc.calculate_deterministic(10, 1, 10_000);
    assert_eq!(score, LOG2_TEN_TO_ONE);
}

#[test]
fn lean_fixture_apply_quality_half() {
    let full = log2_ratio(10, 1).unwrap();
    assert_eq!(apply_quality(full, 5_000), full / 2);
}

#[test]
fn lean_fixture_mint_atoms_16_over_521() {
    let c = RewardCalculator::new();
    let expected = (16 * REWARD_FIXED_POINT_SCALE * REWARD_EMISSION_MULTIPLIER) / 521;
    assert_eq!(c.calculate_block_reward(16.0, 521), expected);
}

#[test]
fn lean_fixture_first_harvest() {
    let c = RewardCalculator::new();
    assert_eq!(
        c.calculate_block_reward(1.0, 1),
        REWARD_FIXED_POINT_SCALE * REWARD_EMISSION_MULTIPLIER
    );
}

#[test]
fn lean_fixture_header_work_trunc() {
    assert_eq!(header_work_score_trunc_u128(16.9), 16);
    assert_eq!(header_work_score_trunc_u128(0.0), 0);
}

#[test]
fn lean_fixture_sub_ms_asymmetry() {
    let calc = WorkScoreCalculator::new();
    let score = calc.calculate_deterministic(324, 3, 10_000);
    assert!(score > 0);
    let bits = fixed_point::to_f64(score);
    let expected = (324.0_f64 / 3.0).log2();
    assert!(
        (bits - expected).abs() < 0.15,
        "expected ~{expected:.2} bits, got {bits:.2}"
    );
}
