// =============================================================================
// Peer Manager
// =============================================================================
//
// Tracks all known peers, manages discovery via seed nodes and peer exchange,
// handles reconnection with exponential backoff + jitter, and schedules
// heartbeats for liveness detection.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use super::connection::ConnectionState;
use super::identity::NodeId;

/// Information tracked for each known peer, whether connected or not.
#[derive(Debug, Clone)]
pub struct PeerRecord {
    /// The peer's identity.
    pub node_id: NodeId,
    /// The peer's listen address (from handshake or peer exchange).
    pub listen_addr: SocketAddr,
    /// Current connection state.
    pub state: ConnectionState,
    /// The peer's Ed25519 public key (set after handshake).
    pub public_key: Option<Vec<u8>>,
    /// When we last received any message from this peer.
    pub last_seen: Instant,
    /// Number of consecutive missed heartbeats.
    pub missed_heartbeats: u32,
    /// Number of reconnection attempts since last successful connection.
    pub reconnect_attempts: u32,
    /// When we should next attempt to reconnect (for backoff scheduling).
    pub next_reconnect: Option<Instant>,
    /// Whether we initiated this connection (outbound) or they did (inbound).
    pub outbound: bool,
}

/// Manages the set of known and connected peers.
pub struct PeerManager {
    /// All known peers indexed by NodeId.
    peers: HashMap<NodeId, PeerRecord>,
    /// Our own node ID (to avoid self-connections).
    local_id: NodeId,
    /// Base delay for exponential backoff.
    reconnect_base: Duration,
    /// Maximum backoff delay.
    reconnect_max: Duration,
    /// Consecutive heartbeat misses before marking dead.
    max_missed_heartbeats: u32,
}

impl PeerManager {
    /// Create a new peer manager.
    pub fn new(
        local_id: NodeId,
        reconnect_base: Duration,
        reconnect_max: Duration,
        max_missed_heartbeats: u32,
    ) -> Self {
        Self {
            peers: HashMap::new(),
            local_id,
            reconnect_base,
            reconnect_max,
            max_missed_heartbeats,
        }
    }

    /// Register a peer from a seed address (before we know their NodeId).
    /// Returns true if this is a new address we haven't seen before.
    pub fn add_seed(&mut self, addr: SocketAddr) -> bool {
        // Check if any existing peer has this address
        if self.peers.values().any(|p| p.listen_addr == addr) {
            return false;
        }
        // We don't know the NodeId yet — the handshake will establish it.
        // Store temporarily with a placeholder. The caller should dial and
        // register the real ID after handshake.
        true
    }

    /// Register or update a peer after a successful handshake.
    pub fn register_peer(
        &mut self,
        node_id: NodeId,
        listen_addr: SocketAddr,
        public_key: Vec<u8>,
        outbound: bool,
    ) {
        if node_id == self.local_id {
            return; // Don't register ourselves
        }

        let record = self.peers.entry(node_id).or_insert_with(|| PeerRecord {
            node_id,
            listen_addr,
            state: ConnectionState::Disconnected,
            public_key: None,
            last_seen: Instant::now(),
            missed_heartbeats: 0,
            reconnect_attempts: 0,
            next_reconnect: None,
            outbound,
        });

        record.state = ConnectionState::Connected;
        record.listen_addr = listen_addr;
        record.public_key = Some(public_key);
        record.last_seen = Instant::now();
        record.missed_heartbeats = 0;
        record.reconnect_attempts = 0;
        record.next_reconnect = None;
    }

    /// Mark a peer as disconnected and schedule reconnection.
    pub fn mark_disconnected(&mut self, node_id: &NodeId) {
        let base = self.reconnect_base;
        let max = self.reconnect_max;
        if let Some(peer) = self.peers.get_mut(node_id) {
            peer.state = ConnectionState::Dead;
            peer.reconnect_attempts += 1;
            let delay = Self::compute_backoff(base, max, peer.reconnect_attempts);
            peer.next_reconnect = Some(Instant::now() + delay);
        }
    }

    /// Update the last_seen timestamp for a peer (called on any message receipt).
    pub fn touch(&mut self, node_id: &NodeId) {
        if let Some(peer) = self.peers.get_mut(node_id) {
            peer.last_seen = Instant::now();
            peer.missed_heartbeats = 0;
        }
    }

    /// Increment missed heartbeats for a peer. Returns true if the peer
    /// should be declared dead (exceeded max_missed_heartbeats).
    pub fn heartbeat_missed(&mut self, node_id: &NodeId) -> bool {
        if let Some(peer) = self.peers.get_mut(node_id) {
            peer.missed_heartbeats += 1;
            if peer.missed_heartbeats >= self.max_missed_heartbeats {
                peer.state = ConnectionState::Dead;
                return true;
            }
        }
        false
    }

    /// Get peers that need reconnection (their backoff timer has expired).
    pub fn peers_needing_reconnect(&self) -> Vec<(NodeId, SocketAddr)> {
        let now = Instant::now();
        self.peers
            .values()
            .filter(|p| {
                p.state == ConnectionState::Dead && p.next_reconnect.is_none_or(|t| now >= t)
            })
            .map(|p| (p.node_id, p.listen_addr))
            .collect()
    }

    /// Determine if we should dial this peer (deduplication rule:
    /// the node with the lexicographically lower NodeId dials).
    pub fn should_dial(&self, peer_id: &NodeId) -> bool {
        self.local_id < *peer_id
    }

    /// Get all currently connected peers.
    pub fn connected_peers(&self) -> Vec<NodeId> {
        self.peers
            .values()
            .filter(|p| p.state == ConnectionState::Connected)
            .map(|p| p.node_id)
            .collect()
    }

    /// Get a peer record by NodeId.
    pub fn get_peer(&self, node_id: &NodeId) -> Option<&PeerRecord> {
        self.peers.get(node_id)
    }

    /// Get a mutable peer record by NodeId.
    pub fn get_peer_mut(&mut self, node_id: &NodeId) -> Option<&mut PeerRecord> {
        self.peers.get_mut(node_id)
    }

    /// Get all known peers with their state (for PeerList response).
    pub fn all_peers(&self) -> Vec<(NodeId, SocketAddr, ConnectionState)> {
        self.peers
            .values()
            .map(|p| (p.node_id, p.listen_addr, p.state))
            .collect()
    }

    /// Get (NodeId, SocketAddr) pairs for peer exchange messages.
    pub fn peer_exchange_list(&self) -> Vec<(NodeId, SocketAddr)> {
        self.peers
            .values()
            .filter(|p| p.state == ConnectionState::Connected)
            .map(|p| (p.node_id, p.listen_addr))
            .collect()
    }

    /// Ingest a peer exchange list from a remote peer. Adds any new peers
    /// we don't already know about. Returns newly discovered addresses.
    pub fn ingest_peer_exchange(
        &mut self,
        peers: &[(NodeId, SocketAddr)],
    ) -> Vec<(NodeId, SocketAddr)> {
        let mut new_peers = Vec::new();
        for (id, addr) in peers {
            if *id == self.local_id {
                continue;
            }
            if !self.peers.contains_key(id) {
                self.peers.insert(
                    *id,
                    PeerRecord {
                        node_id: *id,
                        listen_addr: *addr,
                        state: ConnectionState::Disconnected,
                        public_key: None,
                        last_seen: Instant::now(),
                        missed_heartbeats: 0,
                        reconnect_attempts: 0,
                        next_reconnect: Some(Instant::now()), // Dial immediately
                        outbound: true,
                    },
                );
                new_peers.push((*id, *addr));
            }
        }
        new_peers
    }

    /// Build a map of NodeId → public key for all peers with known keys.
    pub fn public_key_map(&self) -> HashMap<NodeId, Vec<u8>> {
        self.peers
            .iter()
            .filter_map(|(id, p)| p.public_key.as_ref().map(|pk| (*id, pk.clone())))
            .collect()
    }

    /// Number of currently connected peers.
    pub fn connected_count(&self) -> usize {
        self.peers
            .values()
            .filter(|p| p.state == ConnectionState::Connected)
            .count()
    }

    /// Total number of known peers.
    pub fn known_count(&self) -> usize {
        self.peers.len()
    }

    /// Compute exponential backoff delay with jitter.
    fn compute_backoff(base: Duration, max: Duration, attempts: u32) -> Duration {
        let exp = 2u64.saturating_pow(attempts.min(6));
        let base_ms = base.as_millis() as u64;
        let delay_ms = (base_ms * exp).min(max.as_millis() as u64);

        // Add jitter: ±25% of the delay
        let jitter_range = delay_ms / 4;
        let jitter = if jitter_range > 0 {
            (rand::random::<u64>() % (jitter_range * 2)) as i64 - jitter_range as i64
        } else {
            0
        };
        let final_ms = (delay_ms as i64 + jitter).max(0) as u64;

        Duration::from_millis(final_ms)
    }

    /// Mark a peer as currently connecting (to prevent duplicate dials).
    pub fn mark_connecting(&mut self, node_id: &NodeId) {
        if let Some(peer) = self.peers.get_mut(node_id) {
            peer.state = ConnectionState::Connecting;
        }
    }

    /// Remove a peer entirely (for banned/permanently unreachable peers).
    pub fn remove_peer(&mut self, node_id: &NodeId) {
        self.peers.remove(node_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_id(byte: u8) -> NodeId {
        NodeId([byte; 32])
    }

    #[test]
    fn test_register_and_connected() {
        let local = make_id(0x00);
        let mut pm = PeerManager::new(local, Duration::from_secs(1), Duration::from_secs(60), 3);

        let peer1 = make_id(0x01);
        let addr: SocketAddr = "127.0.0.1:9001".parse().unwrap();
        pm.register_peer(peer1, addr, vec![0; 32], true);

        assert_eq!(pm.connected_count(), 1);
        assert!(pm.connected_peers().contains(&peer1));
    }

    #[test]
    fn test_mark_disconnected_schedules_reconnect() {
        let local = make_id(0x00);
        let mut pm = PeerManager::new(local, Duration::from_secs(1), Duration::from_secs(60), 3);

        let peer1 = make_id(0x01);
        let addr: SocketAddr = "127.0.0.1:9001".parse().unwrap();
        pm.register_peer(peer1, addr, vec![0; 32], true);
        pm.mark_disconnected(&peer1);

        assert_eq!(pm.connected_count(), 0);
        let record = pm.get_peer(&peer1).unwrap();
        assert_eq!(record.state, ConnectionState::Dead);
        assert!(record.next_reconnect.is_some());
    }

    #[test]
    fn test_heartbeat_missed_threshold() {
        let local = make_id(0x00);
        let mut pm = PeerManager::new(local, Duration::from_secs(1), Duration::from_secs(60), 3);

        let peer1 = make_id(0x01);
        pm.register_peer(peer1, "127.0.0.1:9001".parse().unwrap(), vec![0; 32], true);

        assert!(!pm.heartbeat_missed(&peer1)); // 1 miss
        assert!(!pm.heartbeat_missed(&peer1)); // 2 misses
        assert!(pm.heartbeat_missed(&peer1)); // 3 misses → dead
    }

    #[test]
    fn test_should_dial_deterministic() {
        let low = make_id(0x00);
        let high = make_id(0xFF);

        let pm_low = PeerManager::new(low, Duration::from_secs(1), Duration::from_secs(60), 3);
        let pm_high = PeerManager::new(high, Duration::from_secs(1), Duration::from_secs(60), 3);

        // Lower ID should dial, higher should not
        assert!(pm_low.should_dial(&high));
        assert!(!pm_high.should_dial(&low));
    }

    #[test]
    fn test_peer_exchange_ingest() {
        let local = make_id(0x00);
        let mut pm = PeerManager::new(local, Duration::from_secs(1), Duration::from_secs(60), 3);

        let exchange = vec![
            (make_id(0x01), "127.0.0.1:9001".parse().unwrap()),
            (make_id(0x02), "127.0.0.1:9002".parse().unwrap()),
            (local, "127.0.0.1:9000".parse().unwrap()), // Should be ignored
        ];

        let new_peers = pm.ingest_peer_exchange(&exchange);
        assert_eq!(new_peers.len(), 2);
        assert_eq!(pm.known_count(), 2);
    }

    #[test]
    fn test_dont_register_self() {
        let local = make_id(0x00);
        let mut pm = PeerManager::new(local, Duration::from_secs(1), Duration::from_secs(60), 3);

        pm.register_peer(local, "127.0.0.1:9000".parse().unwrap(), vec![0; 32], true);
        assert_eq!(pm.connected_count(), 0);
    }

    #[test]
    fn test_touch_resets_heartbeat_misses() {
        let local = make_id(0x00);
        let mut pm = PeerManager::new(local, Duration::from_secs(1), Duration::from_secs(60), 3);

        let peer1 = make_id(0x01);
        pm.register_peer(peer1, "127.0.0.1:9001".parse().unwrap(), vec![0; 32], true);

        pm.heartbeat_missed(&peer1); // 1 miss
        pm.heartbeat_missed(&peer1); // 2 misses
        pm.touch(&peer1); // Reset

        // Should need 3 more misses now
        assert!(!pm.heartbeat_missed(&peer1));
        assert!(!pm.heartbeat_missed(&peer1));
        assert!(pm.heartbeat_missed(&peer1));
    }

    #[test]
    fn test_backoff_increases() {
        let base = Duration::from_secs(1);
        let max = Duration::from_secs(60);

        let _d1 = PeerManager::compute_backoff(base, max, 0);
        let d5 = PeerManager::compute_backoff(base, max, 5);
        // d5 should be significantly larger than d1 on average
        // Base: 1s * 2^0 = 1s vs 1s * 2^5 = 32s
        assert!(d5 > Duration::from_secs(10)); // Even with negative jitter
    }
}
