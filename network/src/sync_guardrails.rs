// =============================================================================
// Sync DoS Guardrails - Phase 1B Protection
// =============================================================================
// Prevents sync-based DoS attacks by:
// 1. Capping in-flight block requests per peer
// 2. Rate limiting request-response handlers
// 3. Tracking backpressure metrics
// =============================================================================

use libp2p::PeerId;
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

/// Configuration for sync DoS protection
#[derive(Debug, Clone)]
pub struct SyncGuardConfig {
    /// Maximum in-flight block requests per peer
    pub max_inflight_per_peer: usize,
    /// Maximum total in-flight block requests (global)
    pub max_inflight_global: usize,
    /// Request timeout (when to consider a request stale)
    pub request_timeout: Duration,
    /// Rate limit: max requests per peer per window
    pub rate_limit_requests: usize,
    /// Rate limit window duration
    pub rate_limit_window: Duration,
    /// Maximum blocks per single request
    pub max_blocks_per_request: u64,
}

impl Default for SyncGuardConfig {
    fn default() -> Self {
        SyncGuardConfig {
            max_inflight_per_peer: 5,       // Max 5 parallel requests to same peer
            max_inflight_global: 50,        // Max 50 total parallel requests
            request_timeout: Duration::from_secs(120),
            rate_limit_requests: 20,        // Max 20 requests per window
            rate_limit_window: Duration::from_secs(60),
            max_blocks_per_request: 100,    // Max 100 blocks per request
        }
    }
}

/// Tracks a single in-flight request
#[derive(Debug, Clone)]
pub struct InflightRequest {
    pub peer: PeerId,
    pub from_height: u64,
    pub to_height: u64,
    pub sent_at: Instant,
    pub request_id: u64,
}

/// Per-peer rate limiting state
#[derive(Debug, Default)]
pub struct PeerRateLimit {
    /// Request timestamps within the rate limit window
    pub request_times: VecDeque<Instant>,
    /// Timeout count (for backpressure)
    pub timeout_count: u32,
    /// Success count (for backpressure)
    pub success_count: u32,
    /// Average RTT in milliseconds
    pub avg_rtt_ms: f64,
    /// RTT samples for averaging
    rtt_samples: VecDeque<u64>,
}

impl PeerRateLimit {
    const MAX_RTT_SAMPLES: usize = 20;

    pub fn record_rtt(&mut self, rtt_ms: u64) {
        self.rtt_samples.push_back(rtt_ms);
        if self.rtt_samples.len() > Self::MAX_RTT_SAMPLES {
            self.rtt_samples.pop_front();
        }
        // Recalculate average
        if !self.rtt_samples.is_empty() {
            let sum: u64 = self.rtt_samples.iter().sum();
            self.avg_rtt_ms = sum as f64 / self.rtt_samples.len() as f64;
        }
    }
}

/// Backpressure metrics for monitoring
#[derive(Debug, Clone, Default)]
pub struct BackpressureMetrics {
    /// Current in-flight requests per peer
    pub inflight_per_peer: HashMap<PeerId, usize>,
    /// Total in-flight requests
    pub total_inflight: usize,
    /// Total timeouts since start
    pub total_timeouts: u64,
    /// Total retries since start
    pub total_retries: u64,
    /// Chunk size direction changes (oscillation indicator)
    pub chunk_size_changes: u64,
    /// Last chunk size (for detecting oscillation)
    pub last_chunk_size: u64,
    /// Average RTT across all peers
    pub avg_rtt_ms: f64,
}

/// Sync DoS Guardrails - manages rate limiting and inflight tracking
#[derive(Debug)]
pub struct SyncGuardrails {
    config: SyncGuardConfig,
    /// In-flight requests by request_id
    inflight: HashMap<u64, InflightRequest>,
    /// In-flight count per peer
    inflight_per_peer: HashMap<PeerId, usize>,
    /// Rate limiting state per peer
    rate_limits: HashMap<PeerId, PeerRateLimit>,
    /// Metrics for monitoring
    metrics: BackpressureMetrics,
}

impl SyncGuardrails {
    pub fn new(config: SyncGuardConfig) -> Self {
        SyncGuardrails {
            config,
            inflight: HashMap::new(),
            inflight_per_peer: HashMap::new(),
            rate_limits: HashMap::new(),
            metrics: BackpressureMetrics::default(),
        }
    }

    /// Check if a new request to this peer is allowed
    pub fn can_request(&self, peer: &PeerId) -> bool {
        // Check global cap
        if self.inflight.len() >= self.config.max_inflight_global {
            return false;
        }

        // Check per-peer cap
        let peer_inflight = self.inflight_per_peer.get(peer).copied().unwrap_or(0);
        if peer_inflight >= self.config.max_inflight_per_peer {
            return false;
        }

        // Check rate limit
        if let Some(rate_limit) = self.rate_limits.get(peer) {
            let now = Instant::now();
            let recent_requests = rate_limit.request_times.iter()
                .filter(|&&t| now.duration_since(t) < self.config.rate_limit_window)
                .count();
            if recent_requests >= self.config.rate_limit_requests {
                return false;
            }
        }

        true
    }

    /// Check if a block range request is valid (not too large)
    pub fn validate_request_range(&self, from_height: u64, to_height: u64) -> bool {
        if to_height < from_height {
            return false;
        }
        let block_count = to_height - from_height + 1;
        block_count <= self.config.max_blocks_per_request
    }

    /// Register a new outbound request
    pub fn register_request(
        &mut self,
        request_id: u64,
        peer: PeerId,
        from_height: u64,
        to_height: u64,
    ) {
        let request = InflightRequest {
            peer,
            from_height,
            to_height,
            sent_at: Instant::now(),
            request_id,
        };

        self.inflight.insert(request_id, request);
        *self.inflight_per_peer.entry(peer).or_insert(0) += 1;

        // Update rate limit tracking
        let rate_limit = self.rate_limits.entry(peer).or_default();
        rate_limit.request_times.push_back(Instant::now());

        // Clean old entries from rate limit window
        let cutoff = Instant::now() - self.config.rate_limit_window;
        while rate_limit.request_times.front().map(|&t| t < cutoff).unwrap_or(false) {
            rate_limit.request_times.pop_front();
        }

        // Update metrics
        self.update_metrics();
    }

    /// Mark a request as completed (success or failure)
    pub fn complete_request(&mut self, request_id: u64, success: bool) -> Option<InflightRequest> {
        if let Some(request) = self.inflight.remove(&request_id) {
            // Update per-peer count
            if let Some(count) = self.inflight_per_peer.get_mut(&request.peer) {
                *count = count.saturating_sub(1);
                if *count == 0 {
                    self.inflight_per_peer.remove(&request.peer);
                }
            }

            // Record RTT if success
            if let Some(rate_limit) = self.rate_limits.get_mut(&request.peer) {
                if success {
                    let rtt = request.sent_at.elapsed().as_millis() as u64;
                    rate_limit.record_rtt(rtt);
                    rate_limit.success_count += 1;
                } else {
                    rate_limit.timeout_count += 1;
                    self.metrics.total_timeouts += 1;
                }
            }

            self.update_metrics();
            Some(request)
        } else {
            None
        }
    }

    /// Clean up stale/timed-out requests
    pub fn cleanup_stale_requests(&mut self) -> Vec<InflightRequest> {
        let now = Instant::now();
        let timeout = self.config.request_timeout;

        let stale_ids: Vec<u64> = self.inflight.iter()
            .filter(|(_, req)| now.duration_since(req.sent_at) > timeout)
            .map(|(&id, _)| id)
            .collect();

        let mut stale_requests = Vec::new();
        for id in stale_ids {
            if let Some(req) = self.complete_request(id, false) {
                stale_requests.push(req);
            }
        }

        stale_requests
    }

    /// Record chunk size change (for oscillation detection)
    pub fn record_chunk_size(&mut self, new_size: u64) {
        if self.metrics.last_chunk_size != 0 {
            // Check if direction changed
            let prev_increasing = self.metrics.last_chunk_size < new_size;
            if (prev_increasing && new_size < self.metrics.last_chunk_size) ||
               (!prev_increasing && new_size > self.metrics.last_chunk_size) {
                self.metrics.chunk_size_changes += 1;
            }
        }
        self.metrics.last_chunk_size = new_size;
    }

    /// Record a retry
    pub fn record_retry(&mut self) {
        self.metrics.total_retries += 1;
    }

    /// Get current backpressure metrics
    pub fn get_metrics(&self) -> BackpressureMetrics {
        self.metrics.clone()
    }

    /// Get number of in-flight requests for a peer
    pub fn peer_inflight_count(&self, peer: &PeerId) -> usize {
        self.inflight_per_peer.get(peer).copied().unwrap_or(0)
    }

    /// Get total in-flight requests
    pub fn total_inflight(&self) -> usize {
        self.inflight.len()
    }

    /// Get peer's average RTT
    pub fn peer_avg_rtt(&self, peer: &PeerId) -> Option<f64> {
        self.rate_limits.get(peer).map(|rl| rl.avg_rtt_ms)
    }

    /// Check if peer has high failure rate (for backoff decisions)
    pub fn peer_has_high_failure_rate(&self, peer: &PeerId) -> bool {
        if let Some(rate_limit) = self.rate_limits.get(peer) {
            let total = rate_limit.success_count + rate_limit.timeout_count;
            if total >= 5 {
                let failure_rate = rate_limit.timeout_count as f64 / total as f64;
                return failure_rate > 0.5; // More than 50% failures
            }
        }
        false
    }

    fn update_metrics(&mut self) {
        self.metrics.inflight_per_peer = self.inflight_per_peer.clone();
        self.metrics.total_inflight = self.inflight.len();

        // Calculate average RTT across all peers
        let rtts: Vec<f64> = self.rate_limits.values()
            .filter(|rl| rl.avg_rtt_ms > 0.0)
            .map(|rl| rl.avg_rtt_ms)
            .collect();

        if !rtts.is_empty() {
            self.metrics.avg_rtt_ms = rtts.iter().sum::<f64>() / rtts.len() as f64;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_peer_id(n: u8) -> PeerId {
        let keypair = libp2p::identity::Keypair::generate_ed25519();
        keypair.public().to_peer_id()
    }

    #[test]
    fn test_inflight_cap_per_peer() {
        let config = SyncGuardConfig {
            max_inflight_per_peer: 2,
            max_inflight_global: 10,
            ..Default::default()
        };
        let mut guardrails = SyncGuardrails::new(config);
        let peer = make_peer_id(1);

        // Should allow first 2 requests
        assert!(guardrails.can_request(&peer));
        guardrails.register_request(1, peer, 0, 10);
        assert!(guardrails.can_request(&peer));
        guardrails.register_request(2, peer, 11, 20);

        // Third request should be blocked
        assert!(!guardrails.can_request(&peer));

        // Complete one, should allow again
        guardrails.complete_request(1, true);
        assert!(guardrails.can_request(&peer));
    }

    #[test]
    fn test_global_inflight_cap() {
        let config = SyncGuardConfig {
            max_inflight_per_peer: 100,
            max_inflight_global: 3,
            ..Default::default()
        };
        let mut guardrails = SyncGuardrails::new(config);

        for i in 0..3 {
            let peer = make_peer_id(i);
            assert!(guardrails.can_request(&peer));
            guardrails.register_request(i as u64, peer, 0, 10);
        }

        // Fourth request should be blocked even for new peer
        let peer = make_peer_id(10);
        assert!(!guardrails.can_request(&peer));
    }

    #[test]
    fn test_request_range_validation() {
        let config = SyncGuardConfig {
            max_blocks_per_request: 100,
            ..Default::default()
        };
        let guardrails = SyncGuardrails::new(config);

        assert!(guardrails.validate_request_range(0, 99));   // 100 blocks OK
        assert!(!guardrails.validate_request_range(0, 100)); // 101 blocks not OK
        assert!(!guardrails.validate_request_range(100, 50)); // Invalid range
    }

    #[test]
    fn test_rtt_tracking() {
        let config = SyncGuardConfig::default();
        let mut guardrails = SyncGuardrails::new(config);
        let peer = make_peer_id(1);

        guardrails.register_request(1, peer, 0, 10);

        // Simulate some time passing
        std::thread::sleep(std::time::Duration::from_millis(10));

        guardrails.complete_request(1, true);

        // Should have recorded RTT
        assert!(guardrails.peer_avg_rtt(&peer).is_some());
    }
}
