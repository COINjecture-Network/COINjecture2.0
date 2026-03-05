// =============================================================================
// COINjecture P2P Protocol (CPP) - Peer Management
// =============================================================================
// Peer connection management with node classification integration

use crate::cpp::{
    config::{NodeType, PEER_TIMEOUT, KEEPALIVE_INTERVAL, MAX_CONSECUTIVE_TIMEOUTS, PEER_QUALITY_THRESHOLD},
    message::*,
    flow_control::FlowControl,
};
use coinject_core::Hash;
use tokio::net::TcpStream;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tokio::time::{Instant, Duration};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

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

    /// Channel for sending messages to write task
    pub send_tx: mpsc::UnboundedSender<Vec<u8>>,

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

    // === NEW FIELDS FOR CONNECTION STABILITY ===

    /// Consecutive read timeouts (resets on successful read)
    /// Used for forced disconnect after MAX_CONSECUTIVE_TIMEOUTS
    pub consecutive_timeouts: u32,

    /// Last successful message read timestamp (distinct from last_seen)
    /// Used for half-dead detection
    pub last_successful_read: Instant,

    /// Connection nonce for tie-breaking simultaneous connections
    pub connection_nonce: u64,

    /// Whether this connection was initiated by us (outbound) or them (inbound)
    pub is_outbound: bool,

    /// Cancellation signal for write task (set to true to stop write task)
    write_task_cancel: Arc<AtomicBool>,
}

impl Peer {
    /// Create new peer connection
    /// Returns (Peer, read_half) - the read half must be used in a separate task
    ///
    /// # Parameters
    /// - `connection_nonce`: Nonce for deterministic tie-breaking of simultaneous connections
    /// - `is_outbound`: Whether we initiated this connection (true) or received it (false)
    pub fn new(
        id: PeerId,
        addr: SocketAddr,
        stream: TcpStream,
        node_type: NodeType,
        best_height: u64,
        best_hash: Hash,
        genesis_hash: Hash,
        connection_nonce: u64,
        is_outbound: bool,
    ) -> (Self, tokio::io::ReadHalf<TcpStream>) {
        // Split stream into read and write halves
        let (read_half, mut write_half) = tokio::io::split(stream);

        // Create channel for sending messages
        let (send_tx, mut send_rx) = mpsc::unbounded_channel::<Vec<u8>>();

        // Create cancellation signal for write task cleanup
        let cancel_signal = Arc::new(AtomicBool::new(false));
        let cancel_signal_clone = cancel_signal.clone();

        // Spawn write task with instrumented logging and cancellation support
        let peer_id_short: String = id.iter().take(4).map(|b| format!("{:02x}", b)).collect();
        tokio::spawn(async move {
            loop {
                // Check for cancellation signal
                if cancel_signal_clone.load(Ordering::Relaxed) {
                    tracing::info!("[CPP][CONN][WRITE_CANCEL] peer={} write task cancelled gracefully", peer_id_short);
                    break;
                }

                // Use select! to handle both messages and cancellation
                tokio::select! {
                    msg = send_rx.recv() => {
                        match msg {
                            Some(data) => {
                                let frame_len = data.len();
                                let msg_type = if data.len() >= 6 { data[5] } else { 0xFF };
                                let msg_type_name = match msg_type {
                                    0x01 => "Hello",
                                    0x02 => "HelloAck",
                                    0x10 => "Status",
                                    0x11 => "GetBlocks",
                                    0x12 => "Blocks",
                                    0x20 => "NewBlock",
                                    0x21 => "NewTransaction",
                                    0xF0 => "Ping",
                                    0xF1 => "Pong",
                                    _ => "Other",
                                };

                                if let Err(e) = write_half.write_all(&data).await {
                                    tracing::error!("[CPP][CONN][WRITE_ERR] peer={} msg={} frame_len={} err={}",
                                        peer_id_short, msg_type_name, frame_len, e);
                                    break;
                                }
                                if let Err(e) = write_half.flush().await {
                                    tracing::error!("[CPP][CONN][WRITE_ERR] peer={} msg={} frame_len={} flush_err={}",
                                        peer_id_short, msg_type_name, frame_len, e);
                                    break;
                                }
                            }
                            None => {
                                // Channel closed
                                tracing::info!("[CPP][CONN][WRITE_CLOSE] peer={} channel closed, exiting", peer_id_short);
                                break;
                            }
                        }
                    }
                    // Periodic check for cancellation (every 1 second)
                    _ = tokio::time::sleep(Duration::from_secs(1)) => {
                        if cancel_signal_clone.load(Ordering::Relaxed) {
                            tracing::info!("[CPP][CONN][WRITE_CANCEL] peer={} write task cancelled gracefully", peer_id_short);
                            break;
                        }
                    }
                }
            }
            tracing::info!("[CPP][CONN][WRITE_EXIT] peer={} write task exiting", peer_id_short);
        });

        let now = Instant::now();
        let peer = Peer {
            id,
            addr,
            send_tx,
            state: PeerState::Connecting,
            node_type,
            best_height,
            best_hash,
            genesis_hash,
            flow_control: FlowControl::new(),
            quality: 1.0, // Start with perfect quality
            last_seen: now,
            last_ping: None,
            pending_ping_nonce: None,
            rtt_samples: Vec::new(),
            messages_sent: 0,
            messages_received: 0,
            bytes_sent: 0,
            bytes_received: 0,
            connected_at: now,
            // New fields for connection stability
            consecutive_timeouts: 0,
            last_successful_read: now,
            connection_nonce,
            is_outbound,
            write_task_cancel: cancel_signal,
        };

        (peer, read_half)
    }
    
    /// Send message to peer (non-blocking)
    pub fn send_message(&self, data: Vec<u8>) -> Result<(), String> {
        self.send_tx.send(data)
            .map_err(|e| format!("Failed to send message: {}", e))
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

    // === NEW METHODS FOR CONNECTION STABILITY ===

    /// Record a read timeout
    /// Returns true if peer should be disconnected (exceeded max timeouts)
    pub fn on_read_timeout(&mut self) -> bool {
        self.consecutive_timeouts += 1;

        let peer_id_short: String = self.id.iter().take(4).map(|b| format!("{:02x}", b)).collect();
        let should_disconnect = self.consecutive_timeouts >= MAX_CONSECUTIVE_TIMEOUTS;

        if should_disconnect {
            tracing::warn!(
                "[CPP][PEER][TIMEOUT_EXCEEDED] peer={} consecutive_timeouts={} max={} -> forcing disconnect",
                peer_id_short, self.consecutive_timeouts, MAX_CONSECUTIVE_TIMEOUTS
            );
        } else {
            tracing::warn!(
                "[CPP][PEER][TIMEOUT] peer={} consecutive_timeouts={}/{}",
                peer_id_short, self.consecutive_timeouts, MAX_CONSECUTIVE_TIMEOUTS
            );
        }

        // Also update quality
        self.on_timeout();

        should_disconnect
    }

    /// Record successful message read (resets timeout counter)
    pub fn on_successful_read(&mut self, bytes: usize) {
        self.consecutive_timeouts = 0;
        self.last_successful_read = Instant::now();
        self.on_message_received(bytes);
    }

    /// Check if peer is in a half-dead state
    ///
    /// Half-dead: has timeouts but last_seen is recent (possible write-only connection)
    /// This can happen when a peer accepts writes but doesn't send responses.
    pub fn is_half_dead(&self) -> bool {
        // Has had recent timeouts
        let has_timeouts = self.consecutive_timeouts > 0;

        // last_seen is recent (peer might still be accepting writes via Status messages)
        let last_seen_recent = self.last_seen.elapsed() < PEER_TIMEOUT;

        // But last successful read is old (no actual messages coming through)
        let last_read_stale = self.last_successful_read.elapsed() > PEER_TIMEOUT / 2;

        has_timeouts && last_seen_recent && last_read_stale
    }

    /// Check if peer is healthy (quality above threshold and not half-dead)
    pub fn is_healthy(&self) -> bool {
        self.state == PeerState::Connected
            && self.quality >= PEER_QUALITY_THRESHOLD
            && !self.is_half_dead()
    }

    /// Gracefully shutdown the peer connection
    /// This signals the write task to stop and prevents resource leaks
    pub fn shutdown(&mut self) {
        let peer_id_short: String = self.id.iter().take(4).map(|b| format!("{:02x}", b)).collect();
        tracing::info!("[CPP][PEER][SHUTDOWN] peer={} initiating shutdown", peer_id_short);

        // Signal write task to stop
        self.write_task_cancel.store(true, Ordering::Relaxed);

        // Update state
        self.state = PeerState::Disconnecting;
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
    pub fn send_ping(&mut self) -> Result<(), String> {
        use crate::cpp::protocol::MessageEnvelope;
        use crate::cpp::message::MessageType;
        
        let nonce = rand::random::<u64>();
        let msg = PingMessage {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            nonce,
        };
        
        // Serialize and send via channel
        let envelope = MessageEnvelope::new(MessageType::Ping, &msg)
            .map_err(|e| format!("Failed to create ping envelope: {}", e))?;
        let data = envelope.encode();
        
        self.send_message(data.clone())?;
        
        self.last_ping = Some(Instant::now());
        self.pending_ping_nonce = Some(nonce);
        self.on_message_sent(data.len());
        
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
    
    fn create_test_peer_addr() -> SocketAddr {
        // Helper used only to demonstrate API shape in docs/tests
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 707)
    }
    
    #[test]
    fn test_peer_score_calculation() {
        // Test that peer score is dimensionless and bounded [0, 1]
        // (Actual test would need a real peer instance)
        
        let quality = 0.8;
        let uptime = 0.95;
        let rate = 5.0 / 10.0;  // 5 msg/s normalized by 10 msg/s
        let bandwidth = 0.1;
        
        let score: f64 = 
            quality * 0.4 +
            uptime * 0.3 +
            rate * 0.2 +
            bandwidth * 0.1;
        
        // Calculate expected: 0.8*0.4 + 0.95*0.3 + 0.5*0.2 + 0.1*0.1
        // = 0.32 + 0.285 + 0.1 + 0.01 = 0.715
        assert!(score >= 0.0 && score <= 1.0);
        let expected: f64 = 0.715;
        assert!((score - expected).abs() < 0.01_f64, "Expected score ~{}, got {}", expected, score);
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
