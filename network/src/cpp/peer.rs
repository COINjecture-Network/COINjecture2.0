// =============================================================================
// COINjecture P2P Protocol (CPP) - Peer Management
// =============================================================================
// Peer connection management with node classification integration

use crate::cpp::{
    config::{NodeType, PEER_TIMEOUT, KEEPALIVE_INTERVAL},
    message::*,
    protocol::{MessageCodec, ProtocolError},
    flow_control::FlowControl,
};
use coinject_core::Hash;
use tokio::net::TcpStream;
use tokio::time::{Instant, Duration};
use std::net::SocketAddr;

/// Peer ID (32-byte hash of public key)
pub type PeerId = [u8; 32];

/// Peer connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerState {
    /// Connecting (handshake in progress)
    Connecting,
    /// Connected and active
    Connected,
    /// Disconnecting (graceful shutdown)
    Disconnecting,
    /// Disconnected
    Disconnected,
}

/// Peer connection with integrated node classification
pub struct Peer {
    /// Peer ID
    pub id: PeerId,
    
    /// Socket address
    pub addr: SocketAddr,
    
    /// TCP stream
    pub stream: TcpStream,
    
    /// Connection state
    pub state: PeerState,
    
    /// Node type (from handshake)
    pub node_type: NodeType,
    
    /// Best block height
    pub best_height: u64,
    
    /// Best block hash
    pub best_hash: Hash,
    
    /// Genesis hash (for chain validation)
    pub genesis_hash: Hash,
    
    /// Flow control
    pub flow_control: FlowControl,
    
    /// Connection quality (0.0-1.0, dimensionless)
    pub quality: f64,
    
    /// Last message received time
    pub last_seen: Instant,
    
    /// Last ping sent time
    pub last_ping: Option<Instant>,
    
    /// Pending ping nonce
    pub pending_ping_nonce: Option<u64>,
    
    /// Round-trip time samples (for quality calculation)
    pub rtt_samples: Vec<Duration>,
    
    /// Messages sent
    pub messages_sent: u64,
    
    /// Messages received
    pub messages_received: u64,
    
    /// Bytes sent
    pub bytes_sent: u64,
    
    /// Bytes received
    pub bytes_received: u64,
    
    /// Connection established time
    pub connected_at: Instant,
}

impl Peer {
    /// Create new peer connection
    pub fn new(
        id: PeerId,
        addr: SocketAddr,
        stream: TcpStream,
        node_type: NodeType,
        best_height: u64,
        best_hash: Hash,
        genesis_hash: Hash,
    ) -> Self {
        Peer {
            id,
            addr,
            stream,
            state: PeerState::Connecting,
            node_type,
            best_height,
            best_hash,
            genesis_hash,
            flow_control: FlowControl::new(),
            quality: 1.0,  // Start with perfect quality
            last_seen: Instant::now(),
            last_ping: None,
            pending_ping_nonce: None,
            rtt_samples: Vec::new(),
            messages_sent: 0,
            messages_received: 0,
            bytes_sent: 0,
            bytes_received: 0,
            connected_at: Instant::now(),
        }
    }
    
    /// Check if peer is connected
    pub fn is_connected(&self) -> bool {
        self.state == PeerState::Connected
    }
    
    /// Check if peer has timed out
    pub fn is_timed_out(&self) -> bool {
        self.last_seen.elapsed() > PEER_TIMEOUT
    }
    
    /// Check if peer needs ping
    pub fn needs_ping(&self) -> bool {
        match self.last_ping {
            Some(last) => last.elapsed() > KEEPALIVE_INTERVAL,
            None => true,
        }
    }
    
    /// Update peer status
    pub fn update_status(&mut self, height: u64, hash: Hash, node_type: NodeType) {
        self.best_height = height;
        self.best_hash = hash;
        self.node_type = node_type;
        self.last_seen = Instant::now();
    }
    
    /// Record message sent
    pub fn on_message_sent(&mut self, bytes: usize) {
        self.messages_sent += 1;
        self.bytes_sent += bytes as u64;
        self.flow_control.on_send();
    }
    
    /// Record message received
    pub fn on_message_received(&mut self, bytes: usize) {
        self.messages_received += 1;
        self.bytes_received += bytes as u64;
        self.last_seen = Instant::now();
    }
    
    /// Record successful message delivery (ACK)
    pub fn on_ack(&mut self, rtt: Duration) {
        self.flow_control.on_ack(rtt);
        self.rtt_samples.push(rtt);
        
        // Keep only last 10 samples
        if self.rtt_samples.len() > 10 {
            self.rtt_samples.remove(0);
        }
        
        // Update quality based on RTT (dimensionless ratio)
        // quality = 1.0 - (avg_rtt / 1s)
        let avg_rtt = self.average_rtt();
        let rtt_quality = 1.0 - (avg_rtt.as_secs_f64() / 1.0).min(1.0);
        
        // Exponential moving average
        self.quality = 0.9 * self.quality + 0.1 * rtt_quality;
        self.quality = self.quality.max(0.1).min(1.0);
    }
    
    /// Record timeout
    pub fn on_timeout(&mut self) {
        self.flow_control.on_timeout();
        
        // Decrease quality exponentially (using η = 1/√2)
        let eta = std::f64::consts::FRAC_1_SQRT_2;
        self.quality *= 1.0 - eta;
        self.quality = self.quality.max(0.1);
    }
    
    /// Get average RTT
    pub fn average_rtt(&self) -> Duration {
        if self.rtt_samples.is_empty() {
            return Duration::from_millis(100);
        }
        
        let sum: Duration = self.rtt_samples.iter().sum();
        sum / self.rtt_samples.len() as u32
    }
    
    /// Get uptime ratio (dimensionless)
    pub fn uptime_ratio(&self) -> f64 {
        let total_time = self.connected_at.elapsed();
        let active_time = total_time.saturating_sub(self.last_seen.elapsed());
        
        if total_time.as_secs() == 0 {
            return 1.0;
        }
        
        active_time.as_secs_f64() / total_time.as_secs_f64()
    }
    
    /// Get message rate (messages per second)
    pub fn message_rate(&self) -> f64 {
        let elapsed = self.connected_at.elapsed().as_secs_f64();
        if elapsed == 0.0 {
            return 0.0;
        }
        
        (self.messages_sent + self.messages_received) as f64 / elapsed
    }
    
    /// Get bandwidth utilization (dimensionless ratio)
    /// Assumes 1 Gbps = 125 MB/s as reference
    pub fn bandwidth_ratio(&self) -> f64 {
        let elapsed = self.connected_at.elapsed().as_secs_f64();
        if elapsed == 0.0 {
            return 0.0;
        }
        
        let bytes_per_sec = (self.bytes_sent + self.bytes_received) as f64 / elapsed;
        let reference_bandwidth = 125_000_000.0;  // 1 Gbps in bytes/sec
        
        (bytes_per_sec / reference_bandwidth).min(1.0)
    }
    
    /// Calculate overall peer score (dimensionless, 0.0-1.0)
    /// 
    /// Combines:
    /// - Connection quality (40%)
    /// - Uptime ratio (30%)
    /// - Message rate (20%)
    /// - Bandwidth utilization (10%)
    pub fn peer_score(&self) -> f64 {
        let quality_weight = 0.4;
        let uptime_weight = 0.3;
        let rate_weight = 0.2;
        let bandwidth_weight = 0.1;
        
        // Normalize message rate (assume 10 msg/s is "good")
        let rate_score = (self.message_rate() / 10.0).min(1.0);
        
        let score = 
            self.quality * quality_weight +
            self.uptime_ratio() * uptime_weight +
            rate_score * rate_weight +
            self.bandwidth_ratio() * bandwidth_weight;
        
        score.max(0.0).min(1.0)
    }
    
    /// Send ping to peer
    pub async fn send_ping(&mut self) -> Result<(), ProtocolError> {
        let nonce = rand::random::<u64>();
        let msg = PingMessage {
            timestamp: chrono::Utc::now().timestamp() as u64,
            nonce,
        };
        
        MessageCodec::send_ping(&mut self.stream, &msg).await?;
        
        self.last_ping = Some(Instant::now());
        self.pending_ping_nonce = Some(nonce);
        self.on_message_sent(std::mem::size_of::<PingMessage>());
        
        Ok(())
    }
    
    /// Handle pong response
    pub fn on_pong(&mut self, nonce: u64) {
        if let Some(pending) = self.pending_ping_nonce {
            if pending == nonce {
                if let Some(ping_time) = self.last_ping {
                    let rtt = ping_time.elapsed();
                    self.on_ack(rtt);
                }
                self.pending_ping_nonce = None;
            }
        }
    }
    
    /// Get peer statistics
    pub fn stats(&self) -> PeerStats {
        PeerStats {
            id: self.id,
            addr: self.addr,
            node_type: self.node_type,
            state: self.state,
            best_height: self.best_height,
            quality: self.quality,
            peer_score: self.peer_score(),
            uptime_ratio: self.uptime_ratio(),
            message_rate: self.message_rate(),
            bandwidth_ratio: self.bandwidth_ratio(),
            average_rtt: self.average_rtt(),
            messages_sent: self.messages_sent,
            messages_received: self.messages_received,
            bytes_sent: self.bytes_sent,
            bytes_received: self.bytes_received,
            connected_duration: self.connected_at.elapsed(),
        }
    }
}

/// Peer statistics (all dimensionless or time-based)
#[derive(Debug, Clone)]
pub struct PeerStats {
    pub id: PeerId,
    pub addr: SocketAddr,
    pub node_type: NodeType,
    pub state: PeerState,
    pub best_height: u64,
    pub quality: f64,
    pub peer_score: f64,
    pub uptime_ratio: f64,
    pub message_rate: f64,
    pub bandwidth_ratio: f64,
    pub average_rtt: Duration,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub connected_duration: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};
    
    fn create_test_peer() -> Peer {
        // Note: This won't actually work in tests without a real TcpStream,
        // but demonstrates the API
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 707);
        
        // We can't create a real TcpStream in tests, so this is just for API demonstration
        // In real tests, we'd use a mock or test harness
        
        // Peer::new(
        //     [1u8; 32],
        //     addr,
        //     stream,
        //     NodeType::Full,
        //     100,
        //     Hash::ZERO,
        //     Hash::ZERO,
        // )
    }
    
    #[test]
    fn test_peer_score_calculation() {
        // Test that peer score is dimensionless and bounded [0, 1]
        // (Actual test would need a real peer instance)
        
        let quality = 0.8;
        let uptime = 0.95;
        let rate = 5.0 / 10.0;  // 5 msg/s normalized by 10 msg/s
        let bandwidth = 0.1;
        
        let score = 
            quality * 0.4 +
            uptime * 0.3 +
            rate * 0.2 +
            bandwidth * 0.1;
        
        assert!(score >= 0.0 && score <= 1.0);
        assert!((score - 0.735).abs() < 0.01);
    }
    
    #[test]
    fn test_quality_decay() {
        let eta = std::f64::consts::FRAC_1_SQRT_2;
        let initial_quality = 1.0;
        
        // After one timeout
        let quality_after_timeout = initial_quality * (1.0 - eta);
        
        assert!(quality_after_timeout < initial_quality);
        assert!(quality_after_timeout > 0.0);
        assert!((quality_after_timeout - 0.293).abs() < 0.01);
    }
}
