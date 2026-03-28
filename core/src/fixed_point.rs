//! Fixed-point integer arithmetic for consensus-critical calculations.
//!
//! All consensus-critical computations (work scores, difficulty, rewards) MUST
//! use deterministic integer arithmetic. IEEE 754 floating-point operations
//! produce platform-specific rounding on ARM vs x86, and extended 80-bit
//! precision on x87, causing consensus splits if used in agreement paths.
//!
//! This module provides a **scale-6 fixed-point** representation where
//! `SCALE = 1_000_000` (one million), so:
//!   - `1.0` → `1_000_000`
//!   - `0.5` → `500_000`
//!   - `3.321928` → `3_321_928`
//!
//! ## Consensus usage
//!
//! Use `log2_ratio()` + `apply_quality()` for deterministic work score
//! computation. These functions are bit-exact across all supported platforms.
//! Only convert to/from f64 at the **display boundary** — never use
//! `from_f64_lossy` / `to_f64` for comparisons or block validation.

/// Fixed-point scale factor: 6 decimal places of precision.
///
/// `1.0 = SCALE`, `1.5 = 1_500_000`, `0.001 = 1_000`.
pub const SCALE: u64 = 1_000_000;

/// Scale factor as u128 (for intermediate multiplication without overflow).
const SCALE_U128: u128 = SCALE as u128;

/// Fixed-point value (u64), scaled by `SCALE`.
///
/// This type is used for all consensus-critical numeric representations.
/// Integer comparison (`a > b`) is valid and correct for ordering.
pub type Fixed64 = u64;

// ---------------------------------------------------------------------------
// Display-only conversions (NOT for consensus logic)
// ---------------------------------------------------------------------------

/// Convert an f64 to `Fixed64` (lossy — for display input only).
///
/// **NEVER use this in consensus-critical comparison paths.**
#[inline]
pub fn from_f64_lossy(v: f64) -> Fixed64 {
    if v <= 0.0 {
        return 0;
    }
    let scaled = v * SCALE as f64;
    if scaled >= u64::MAX as f64 {
        return u64::MAX;
    }
    scaled as u64
}

/// Convert `Fixed64` to f64 (for display / logging only).
///
/// **NEVER use this in consensus-critical comparison paths.**
#[inline]
pub fn to_f64(v: Fixed64) -> f64 {
    v as f64 / SCALE as f64
}

// ---------------------------------------------------------------------------
// Integer square root (Newton's method, deterministic)
// ---------------------------------------------------------------------------

/// Integer square root: returns `floor(√n)`.
///
/// Uses Newton's method; converges in O(log n) iterations.
/// Deterministic and exact on all platforms.
#[inline]
pub fn isqrt(n: u128) -> u128 {
    if n == 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

// ---------------------------------------------------------------------------
// Arithmetic helpers
// ---------------------------------------------------------------------------

/// Fixed-point multiplication: `(a × b) / SCALE`.
///
/// Uses 128-bit intermediate to prevent overflow.
/// Returns `None` if the result overflows `u64`.
#[inline]
pub fn mul(a: Fixed64, b: Fixed64) -> Option<Fixed64> {
    let product = (a as u128).checked_mul(b as u128)?;
    let result = product / SCALE_U128;
    if result > u64::MAX as u128 {
        None
    } else {
        Some(result as u64)
    }
}

/// Fixed-point multiplication, saturating on overflow.
#[inline]
pub fn mul_sat(a: Fixed64, b: Fixed64) -> Fixed64 {
    let product = (a as u128).saturating_mul(b as u128);
    let result = product / SCALE_U128;
    result.min(u64::MAX as u128) as u64
}

// ---------------------------------------------------------------------------
// Logarithm (consensus-critical)
// ---------------------------------------------------------------------------

/// Compute `log₂(numerator / denominator) × SCALE` using integer arithmetic.
///
/// This is the deterministic, platform-independent implementation of the
/// work score formula. All arithmetic is exact 128-bit integer math with
/// no floating-point at any stage.
///
/// ## Algorithm
///
/// 1. Scale `numerator` left by 2³² to preserve fractional bits in integer
///    division: `ratio_fp = (numerator << 32) / denominator`.
/// 2. `total_bits = floor(log₂(ratio_fp))` via leading-zeros counting.
/// 3. `floor_k = total_bits − 32` gives `floor(log₂(ratio))`.
/// 4. Normalized mantissa `m = ratio_fp >> (total_bits − 32)` lies in
///    `[2³², 2³³)`, representing the significand in `[1, 2)`.
/// 5. Fractional part via linear approximation:
///    `frac ≈ (m − 2³²) / 2³² × SCALE`
///    (linear interpolation; max error < 0.086 bits).
///
/// ## Returns
///
/// `Some(score)` where `score = floor_k × SCALE + frac`, or `None` if the
/// ratio is ≤ 1 (logarithm would be non-positive).
pub fn log2_ratio(numerator: u64, denominator: u64) -> Option<Fixed64> {
    if denominator == 0 || numerator <= denominator {
        return None;
    }

    // Shift numerator left by 32 bits before dividing, preserving fractional info.
    const SHIFT: u32 = 32;
    let ratio_fp: u128 = ((numerator as u128) << SHIFT) / (denominator as u128);

    if ratio_fp == 0 {
        return None;
    }

    // floor(log₂(ratio_fp)) = 127 − leading_zeros(ratio_fp)
    let total_bits = 127u32.saturating_sub(ratio_fp.leading_zeros());

    // floor(log₂(ratio)) = total_bits − SHIFT
    if total_bits < SHIFT {
        return None; // ratio < 1
    }
    let floor_k = (total_bits - SHIFT) as u64;

    // Normalized mantissa in [2^SHIFT, 2^(SHIFT+1)):
    //   ratio_fp >> (total_bits − SHIFT)
    let shift_amount = total_bits - SHIFT;
    let mantissa = (ratio_fp >> shift_amount) as u64;

    // Fractional part: (mantissa − 2^SHIFT) / 2^SHIFT × SCALE
    // This is linear interpolation of log₂(x) in [1, 2).
    let mantissa_frac = mantissa.saturating_sub(1u64 << SHIFT);
    let frac = ((mantissa_frac as u128 * SCALE_U128) >> SHIFT) as u64;

    Some(floor_k * SCALE + frac)
}

// ---------------------------------------------------------------------------
// Quality scaling
// ---------------------------------------------------------------------------

/// Apply a quality factor (basis points, 0–10_000) to a work score.
///
/// `quality_bps = 10_000` → perfect quality (×1.0), no scaling.
/// `quality_bps = 5_000`  → half quality (×0.5).
/// `quality_bps = 0`      → rejected solution, returns 0.
///
/// Uses 128-bit intermediate to avoid overflow.
#[inline]
pub fn apply_quality(score: Fixed64, quality_bps: u16) -> Fixed64 {
    if quality_bps == 0 {
        return 0;
    }
    if quality_bps >= 10_000 {
        return score;
    }
    ((score as u128 * quality_bps as u128) / 10_000) as u64
}

/// Convert a floating-point quality score `[0.0, 1.0]` to basis points `[0, 10_000]`.
///
/// For use at input boundaries only. Do not use the result in storage
/// that is hashed into block headers.
#[inline]
pub fn quality_f64_to_bps(quality: f64) -> u16 {
    if quality <= 0.0 {
        return 0;
    }
    if quality >= 1.0 {
        return 10_000;
    }
    (quality * 10_000.0) as u16
}

// ---------------------------------------------------------------------------
// Chain-security helpers
// ---------------------------------------------------------------------------

/// Compute cumulative chain security in fixed-point bits.
///
/// `Σ work_scores` over all blocks. An attacker must reproduce this many
/// bit-equivalent operations to forge an alternative chain.
#[inline]
pub fn chain_security(scores: &[Fixed64]) -> u128 {
    scores.iter().map(|&s| s as u128).sum()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_isqrt_perfect_squares() {
        assert_eq!(isqrt(0), 0);
        assert_eq!(isqrt(1), 1);
        assert_eq!(isqrt(4), 2);
        assert_eq!(isqrt(9), 3);
        assert_eq!(isqrt(100), 10);
        assert_eq!(isqrt(1024), 32);
    }

    #[test]
    fn test_isqrt_non_perfect() {
        assert_eq!(isqrt(2), 1);   // floor(√2) = 1
        assert_eq!(isqrt(3), 1);
        assert_eq!(isqrt(5), 2);
        assert_eq!(isqrt(10), 3);  // floor(√10) = 3
        assert_eq!(isqrt(99), 9);  // floor(√99) = 9
    }

    #[test]
    fn test_log2_ratio_exact_powers() {
        // log₂(2/1) = 1.0 → SCALE
        let score = log2_ratio(2, 1).unwrap();
        assert_eq!(score / SCALE, 1);

        // log₂(4/1) = 2.0 → 2*SCALE
        let score = log2_ratio(4, 1).unwrap();
        assert_eq!(score / SCALE, 2);

        // log₂(1024/1) = 10.0 → 10*SCALE
        let score = log2_ratio(1024, 1).unwrap();
        assert_eq!(score / SCALE, 10);
    }

    #[test]
    fn test_log2_ratio_returns_none_for_ratio_leq_one() {
        assert!(log2_ratio(1, 1).is_none());
        assert!(log2_ratio(1, 2).is_none());
        assert!(log2_ratio(0, 1).is_none());
        assert!(log2_ratio(999, 1000).is_none());
    }

    #[test]
    fn test_log2_ratio_denominator_zero() {
        assert!(log2_ratio(100, 0).is_none());
    }

    #[test]
    fn test_log2_ratio_deterministic() {
        // Identical inputs must always produce identical outputs.
        let a = log2_ratio(10_000_000, 1_000);
        let b = log2_ratio(10_000_000, 1_000);
        assert_eq!(a, b);
    }

    #[test]
    fn test_log2_ratio_approx_10_to_1() {
        // log₂(10) ≈ 3.32193
        let score = log2_ratio(10, 1).unwrap();
        let bits = to_f64(score);
        assert!(bits > 3.0 && bits < 3.5, "log₂(10) ≈ 3.32, got {:.4}", bits);
    }

    #[test]
    fn test_log2_ratio_large_values() {
        // solve=10s=10_000_000μs, verify=1ms=1_000μs  → ratio=10_000
        // log₂(10_000) ≈ 13.29
        let score = log2_ratio(10_000_000, 1_000).unwrap();
        let bits = to_f64(score);
        assert!(bits > 13.0 && bits < 13.5, "log₂(10000) ≈ 13.29, got {:.4}", bits);
    }

    #[test]
    fn test_apply_quality_full() {
        let score = 5_000_000u64;
        assert_eq!(apply_quality(score, 10_000), 5_000_000);
    }

    #[test]
    fn test_apply_quality_half() {
        let score = 5_000_000u64;
        assert_eq!(apply_quality(score, 5_000), 2_500_000);
    }

    #[test]
    fn test_apply_quality_zero_returns_zero() {
        assert_eq!(apply_quality(999_999, 0), 0);
    }

    #[test]
    fn test_apply_quality_over_10000_clamped() {
        let score = 5_000_000u64;
        // 10_001 should be treated as full quality
        assert_eq!(apply_quality(score, 10_001), 5_000_000);
    }

    #[test]
    fn test_from_f64_to_f64_roundtrip() {
        let original = 3.14159f64;
        let fixed = from_f64_lossy(original);
        let back = to_f64(fixed);
        assert!((back - original).abs() < 0.000_002, "roundtrip error: {}", (back - original).abs());
    }

    #[test]
    fn test_mul_basic() {
        // 2.0 × 3.0 = 6.0
        let two = 2 * SCALE;
        let three = 3 * SCALE;
        let six = mul(two, three).unwrap();
        assert_eq!(six, 6 * SCALE);
    }

    #[test]
    fn test_mul_fractional() {
        // 1.5 × 2.0 = 3.0
        let one_half = 3 * SCALE / 2; // 1.5
        let two = 2 * SCALE;
        let result = mul(one_half, two).unwrap();
        assert_eq!(result, 3 * SCALE);
    }

    #[test]
    fn test_quality_f64_to_bps() {
        assert_eq!(quality_f64_to_bps(0.0), 0);
        assert_eq!(quality_f64_to_bps(1.0), 10_000);
        assert_eq!(quality_f64_to_bps(0.5), 5_000);
        assert_eq!(quality_f64_to_bps(-1.0), 0);
        assert_eq!(quality_f64_to_bps(2.0), 10_000);
    }

    #[test]
    fn test_chain_security_additive() {
        let scores = vec![10 * SCALE, 12 * SCALE, 11 * SCALE];
        let total = chain_security(&scores);
        assert_eq!(total, 33 * SCALE as u128);
    }
}
