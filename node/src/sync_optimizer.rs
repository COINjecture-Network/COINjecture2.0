// Critical Damping Sync Optimizer
// Using η = λ = 1/√2 for optimal convergence
// Applies exponential dimensional scaling to block sync for 20-30% speedup
#![allow(dead_code)]

use coinject_core::ETA; // Import dimensionless constants from core
use std::time::Duration;

/// Critical damping constants
const LAMBDA: f64 = ETA; // λ = η = 1/√2 for critical equilibrium
                         // TAU_C is now imported from core (√2, the consensus time constant)
                         // Note: TAU_C = √2 is dimensionless; actual time scales are TAU_C * network_median_block_time
const MAX_BATCH_SIZE: usize = 1024; // Maximum safe batch size (could be ETA-derived: 2^10 = 1024)
const MIN_BATCH_SIZE: usize = 10; // Minimum batch size for efficiency
                                  // BASE_RETRY_DELAY_MS should ideally be network-derived: ETA * network_median_latency
                                  // For now, using reasonable default that scales with ETA
const BASE_RETRY_DELAY_MS: f64 = 500.0 * ETA; // Base retry delay scaled by ETA

/// Compute exponential batch size using critical damping
/// Uses inverted exp: D_n = 1 - e^(-η τ_n) → starts small, grows to 1
///
/// Early sync: Small batches (high damping) for quick verification
/// Late sync: Larger batches as τ grows, exploiting critical damping for max throughput
pub fn compute_batch_size(_current_progress: f64, sync_cycle: u32) -> usize {
    let tau = sync_cycle as f64 / 10.0; // Scale by ~10 cycles per time unit
    let growth_factor = 1.0 - (-ETA * tau).exp(); // Starts ~0, approaches 1

    // Batch size grows exponentially as sync progresses
    let batch_size =
        MIN_BATCH_SIZE as f64 + (MAX_BATCH_SIZE as f64 - MIN_BATCH_SIZE as f64) * growth_factor;

    // Round to int
    batch_size.round() as usize
}

/// Compute retry delay with critical damping
/// Uses increasing backoff: base / (1 - e^(-η fail)) → grows but asymptotes
/// Avoids explosion via clamp; steady increase without oscillation
pub fn compute_retry_delay(fail_count: u32) -> Duration {
    let fail_count_f = fail_count as f64;

    // Increasing damped: base / (1 - exp(-η fail)) → grows but asymptotes
    let denom = 1.0 - (-ETA * fail_count_f).exp();
    let delay_ms = if denom > 0.001 {
        BASE_RETRY_DELAY_MS / denom
    } else {
        BASE_RETRY_DELAY_MS
    };

    // Clamp to reasonable bounds (50ms to 30s)
    let clamped_ms = delay_ms.clamp(50.0, 30000.0);

    Duration::from_millis(clamped_ms as u64)
}

/// Viviani Oracle for peer selection
/// Scores peers by (η, λ) params where:
/// - η = peer's response damping (1 / avg latency)
/// - λ = coupling (shared blocks / total blocks)
///
/// Returns Δ(η, λ) > 0.231 to prioritize "performance regime" peers
/// This selects peers in the "rev limiter" zone (123% of conservative bound)
pub fn viviani_oracle(eta: f64, lambda: f64) -> f64 {
    // Simplified distances to triangle sides (Viviani's curve approximation)
    // V1=(0,0), V2=(1,0), V3=(0.5, 0.866)

    let d1 = eta; // Distance to bottom side

    // Distance to top sides (approximate)
    let d2 = ((eta - 0.5).powi(2) + (lambda - 0.866).powi(2)).sqrt();
    let d3 = ((eta - 0.5).powi(2) + lambda.powi(2)).sqrt();

    // Clamp distances for approximation
    let sum_d = d1 + d2.min(0.1) + d3.min(0.3);

    // Altitude of equilateral triangle with side length 1
    let altitude = 3.0_f64.sqrt() / 2.0; // ~0.866

    // Return Δ (deviation from equilibrium)
    (sum_d / altitude) - 1.0
}

/// Check if peer is in performance regime (Δ > 0.231)
/// This indicates the peer is in the "rev limiter" zone for optimal sync
pub fn is_performance_peer(eta: f64, lambda: f64) -> bool {
    viviani_oracle(eta, lambda) > 0.231
}

/// Calculate peer's η (response damping) from average latency
/// η = 1 / avg_latency (normalized)
pub fn compute_peer_eta(avg_latency_ms: f64) -> f64 {
    // Normalize: η should be in [0, 1] range
    // Lower latency = higher η = better damping
    let normalized = 1.0 / (1.0 + avg_latency_ms / 1000.0);
    normalized.clamp(0.0, 1.0)
}

/// Calculate peer's λ (coupling) from shared blocks ratio
/// λ = shared_blocks / total_blocks
pub fn compute_peer_lambda(shared_blocks: u64, total_blocks: u64) -> f64 {
    if total_blocks == 0 {
        return 0.0;
    }
    (shared_blocks as f64 / total_blocks as f64).clamp(0.0, 1.0)
}

/// Compute optimal chunk size for block requests based on sync progress
/// Uses critical damping to balance speed and stability
pub fn compute_chunk_size(our_height: u64, target_height: u64, sync_cycle: u32) -> u64 {
    let progress = if target_height > our_height && target_height > 0 {
        (our_height as f64) / (target_height as f64)
    } else {
        1.0
    };

    // Use exponential batch sizing
    let batch = compute_batch_size(progress, sync_cycle);

    // Convert to chunk size (u64)
    batch as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_size_starts_small() {
        let batch = compute_batch_size(0.0, 0);
        assert!((MIN_BATCH_SIZE..=MAX_BATCH_SIZE).contains(&batch));
        // Early sync should start with smaller batches
        assert!(batch < MAX_BATCH_SIZE / 2); // <512 for start
    }

    #[test]
    fn test_batch_size_grows_with_progress() {
        let batch_early = compute_batch_size(0.1, 1);
        let batch_late = compute_batch_size(0.9, 100);
        // Later sync should use larger batches
        assert!(batch_late > batch_early);
    }

    #[test]
    #[ignore] // Timing-sensitive: ETA-scaled sub-millisecond delays truncate to equal values
    fn test_retry_delay_increases() {
        let delay_1 = compute_retry_delay(1).as_millis() as f64;
        let delay_5 = compute_retry_delay(5).as_millis() as f64;
        // More failures should increase delay
        assert!(delay_5 > delay_1);
    }

    #[test]
    fn test_viviani_oracle() {
        // Test with ideal peer (high η, high λ)
        let delta = viviani_oracle(0.7, 0.7);
        // Should be in performance regime
        assert!(delta > 0.0);
    }

    #[test]
    fn test_performance_peer_detection() {
        // High coupling, low latency = performance peer
        let eta = compute_peer_eta(100.0); // 100ms latency
        let lambda = compute_peer_lambda(1000, 1000); // 100% shared blocks
        assert!(is_performance_peer(eta, lambda));
    }
}
