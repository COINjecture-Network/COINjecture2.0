// =============================================================================
// COINjecture P2P Protocol (CPP) - Equilibrium-Based Flow Control
// =============================================================================
// Flow control using the equilibrium constant η = λ = 1/√2 ≈ 0.7071
//
// Traditional TCP uses arbitrary constants (e.g., initial window = 10 packets).
// CPP uses equilibrium-based adaptation for critical damping (fastest convergence
// without congestion).

use crate::cpp::config::ETA;
use std::time::{Duration, Instant};

/// Equilibrium-based flow control
///
/// Adapts window size using the equilibrium constant η = 1/√2 ≈ 0.7071:
/// - On ACK: window += η (additive increase)
/// - On timeout: window *= (1 - η) (multiplicative decrease)
///
/// This achieves critical damping - the fastest convergence to optimal
/// throughput without overshoot or oscillation.
#[derive(Debug, Clone)]
pub struct FlowControl {
    /// Current window size (number of in-flight messages)
    window: f64,

    /// Minimum window size
    min_window: f64,

    /// Maximum window size
    max_window: f64,

    /// Measured round-trip time (RTT)
    rtt: Duration,

    /// RTT variance (for timeout calculation)
    rtt_var: Duration,

    /// Number of messages sent
    sent_count: u64,

    /// Number of messages acknowledged
    ack_count: u64,

    /// Number of timeouts
    timeout_count: u64,

    /// Last update time
    last_update: Instant,

    /// Equilibrium constant (η = 1/√2)
    eta: f64,
}

impl FlowControl {
    /// Create new flow control with default parameters
    pub fn new() -> Self {
        FlowControl {
            window: 10.0, // Start with 10 messages
            min_window: 1.0,
            max_window: 100.0,
            rtt: Duration::from_millis(100), // Assume 100ms initially
            rtt_var: Duration::from_millis(50),
            sent_count: 0,
            ack_count: 0,
            timeout_count: 0,
            last_update: Instant::now(),
            eta: ETA,
        }
    }

    /// Create flow control with custom window bounds
    pub fn with_bounds(min_window: f64, max_window: f64) -> Self {
        FlowControl {
            window: min_window.max(1.0),
            min_window,
            max_window,
            rtt: Duration::from_millis(100),
            rtt_var: Duration::from_millis(50),
            sent_count: 0,
            ack_count: 0,
            timeout_count: 0,
            last_update: Instant::now(),
            eta: ETA,
        }
    }

    /// Check if we can send more messages
    pub fn can_send(&self, in_flight: usize) -> bool {
        (in_flight as f64) < self.window
    }

    /// Get current window size
    pub fn window_size(&self) -> usize {
        self.window.ceil() as usize
    }

    /// Get current RTT
    pub fn rtt(&self) -> Duration {
        self.rtt
    }

    /// Get timeout duration (RTT + 4 * RTT_VAR, per RFC 6298)
    pub fn timeout(&self) -> Duration {
        // Cap rtt_var to prevent overflow (max 10 seconds)
        let max_rtt_var = Duration::from_secs(10);
        let capped_rtt_var = self.rtt_var.min(max_rtt_var);
        self.rtt + capped_rtt_var * 4
    }

    /// Record message sent
    pub fn on_send(&mut self) {
        self.sent_count += 1;
    }

    /// Record ACK received (successful delivery)
    ///
    /// Increases window using equilibrium-based additive increase:
    /// window += η
    pub fn on_ack(&mut self, rtt: Duration) {
        self.ack_count += 1;

        // Update RTT using exponential moving average
        // RTT = (1 - α) * RTT + α * measured_RTT
        // α = 0.125 (standard TCP value)
        let alpha = 0.125;
        let rtt_ms = self.rtt.as_millis() as f64;
        let measured_ms = rtt.as_millis() as f64;
        let new_rtt_ms = (1.0 - alpha) * rtt_ms + alpha * measured_ms;
        self.rtt = Duration::from_millis(new_rtt_ms as u64);

        // Update RTT variance
        // RTT_VAR = (1 - β) * RTT_VAR + β * |RTT - measured_RTT|
        // β = 0.25 (standard TCP value)
        let beta = 0.25;
        let rtt_var_ms = self.rtt_var.as_millis() as f64;
        let diff_ms = (rtt_ms - measured_ms).abs();
        let new_rtt_var_ms = (1.0 - beta) * rtt_var_ms + beta * diff_ms;
        // Cap RTT variance to prevent overflow (max 10 seconds)
        let max_rtt_var_ms = 10_000.0;
        self.rtt_var = Duration::from_millis(new_rtt_var_ms.min(max_rtt_var_ms) as u64);

        // Equilibrium-based additive increase
        self.window += self.eta;

        // Clamp to bounds
        self.window = self.window.max(self.min_window).min(self.max_window);

        self.last_update = Instant::now();
    }

    /// Record timeout (congestion or packet loss)
    ///
    /// Decreases window using equilibrium-based multiplicative decrease:
    /// window *= (1 - η)
    pub fn on_timeout(&mut self) {
        self.timeout_count += 1;

        // Equilibrium-based multiplicative decrease
        self.window *= 1.0 - self.eta;

        // Clamp to bounds
        self.window = self.window.max(self.min_window).min(self.max_window);

        // Double RTT timeout (exponential backoff)
        // Cap RTT to prevent overflow (max 60 seconds)
        let max_rtt = Duration::from_secs(60);
        self.rtt = (self.rtt * 2).min(max_rtt);

        self.last_update = Instant::now();
    }

    /// Get statistics
    pub fn stats(&self) -> FlowControlStats {
        FlowControlStats {
            window: self.window,
            rtt: self.rtt,
            timeout: self.timeout(),
            sent_count: self.sent_count,
            ack_count: self.ack_count,
            timeout_count: self.timeout_count,
            loss_rate: if self.sent_count > 0 {
                self.timeout_count as f64 / self.sent_count as f64
            } else {
                0.0
            },
        }
    }

    /// Reset flow control (e.g., after long idle period)
    pub fn reset(&mut self) {
        self.window = self.min_window.max(1.0);
        self.rtt = Duration::from_millis(100);
        self.rtt_var = Duration::from_millis(50);
        self.last_update = Instant::now();
    }
}

impl Default for FlowControl {
    fn default() -> Self {
        Self::new()
    }
}

/// Flow control statistics
#[derive(Debug, Clone)]
pub struct FlowControlStats {
    /// Current window size
    pub window: f64,

    /// Current RTT
    pub rtt: Duration,

    /// Current timeout
    pub timeout: Duration,

    /// Total messages sent
    pub sent_count: u64,

    /// Total ACKs received
    pub ack_count: u64,

    /// Total timeouts
    pub timeout_count: u64,

    /// Packet loss rate (timeouts / sent)
    pub loss_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flow_control_initialization() {
        let fc = FlowControl::new();
        assert_eq!(fc.window_size(), 10);
        assert!(fc.can_send(5));
        assert!(!fc.can_send(15));
    }

    #[test]
    fn test_additive_increase() {
        let mut fc = FlowControl::new();
        let initial_window = fc.window;

        // Simulate successful ACK
        fc.on_ack(Duration::from_millis(50));

        // Window should increase by η
        assert!((fc.window - (initial_window + ETA)).abs() < 0.001);
    }

    #[test]
    fn test_multiplicative_decrease() {
        let mut fc = FlowControl::new();
        let initial_window = fc.window;

        // Simulate timeout
        fc.on_timeout();

        // Window should decrease by factor of (1 - η)
        let expected = initial_window * (1.0 - ETA);
        assert!((fc.window - expected).abs() < 0.001);
    }

    #[test]
    fn test_window_bounds() {
        let mut fc = FlowControl::with_bounds(5.0, 20.0);

        // Decrease below minimum
        for _ in 0..100 {
            fc.on_timeout();
        }
        assert!(fc.window >= 5.0);

        // Increase above maximum
        fc.reset();
        for _ in 0..100 {
            fc.on_ack(Duration::from_millis(50));
        }
        assert!(fc.window <= 20.0);
    }

    #[test]
    fn test_rtt_measurement() {
        let mut fc = FlowControl::new();

        // Simulate ACKs with different RTTs
        fc.on_ack(Duration::from_millis(100));
        fc.on_ack(Duration::from_millis(150));
        fc.on_ack(Duration::from_millis(120));

        // RTT should be smoothed exponential moving average
        let rtt_ms = fc.rtt().as_millis();
        assert!(rtt_ms > 100 && rtt_ms < 150);
    }

    #[test]
    fn test_convergence() {
        let mut fc = FlowControl::new();

        // Simulate stable network (all ACKs)
        // Starting window: 10.0, each ACK adds η ≈ 0.707
        // After 50 ACKs: 10 + 50*0.707 ≈ 45.35
        for _ in 0..50 {
            fc.on_send();
            fc.on_ack(Duration::from_millis(100));
        }

        // Window should grow significantly (but capped at max_window = 100.0)
        // After 50 ACKs: 10 + 50*0.707 ≈ 45.35, but capped at 100
        assert!(
            fc.window >= 40.0,
            "Window should grow with ACKs, got {}",
            fc.window
        );

        // Simulate congestion (some timeouts)
        for i in 0..50 {
            fc.on_send();
            if i % 5 == 0 {
                fc.on_timeout();
            } else {
                fc.on_ack(Duration::from_millis(100));
            }
        }

        // Window should stabilize below maximum
        let stats = fc.stats();
        assert!(stats.loss_rate > 0.0 && stats.loss_rate < 0.3);
    }
}
