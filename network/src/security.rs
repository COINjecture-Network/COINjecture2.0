// =============================================================================
// COINjecture Network Security Layer
// =============================================================================
// Connection limiting, rate limiting, peer banning, eclipse attack protection,
// per-type message size policy, and network security metrics.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default max inbound connections per /32 (single IP)
pub const DEFAULT_MAX_CONNS_PER_IP: usize = 3;

/// Default total inbound + outbound connection cap
pub const DEFAULT_MAX_TOTAL_CONNECTIONS: usize = 128;

/// Default max peers per /16 subnet (eclipse attack protection)
pub const DEFAULT_MAX_PEERS_PER_SUBNET: usize = 8;

/// Default ban duration for misbehaving peers
pub const DEFAULT_BAN_DURATION: Duration = Duration::from_secs(3600); // 1 hour

/// Short ban for minor infractions (rate-limit violations)
pub const SHORT_BAN_DURATION: Duration = Duration::from_secs(300); // 5 min

/// Default token-bucket capacity (burst allowance)
pub const DEFAULT_RATE_BUCKET_CAPACITY: f64 = 200.0;

/// Default message refill rate (msgs/sec steady-state)
pub const DEFAULT_RATE_MSGS_PER_SEC: f64 = 50.0;

// ---------------------------------------------------------------------------
// Message size limits (item 2)
// ---------------------------------------------------------------------------

/// Per-type maximum payload sizes enforced at the protocol layer.
///
/// These are deliberately conservative:
///   Transaction    256 KB  — single tx fits comfortably
///   Block          4 MB    — largest valid block (~16 txs × 256 KB)
///   Consensus      64 KB   — small coordinator messages
///   Handshake      4 KB    — hello / auth material only
///   Default        1 MB    — fallback for types not listed above
pub struct MessageSizePolicy;

impl MessageSizePolicy {
    pub const TRANSACTION: usize   = 256 * 1_024;          //  256 KB
    pub const BLOCK: usize         = 4   * 1_024 * 1_024;  //    4 MB
    pub const CONSENSUS: usize     = 64  * 1_024;          //   64 KB
    pub const HANDSHAKE: usize     = 4   * 1_024;          //    4 KB
    pub const DEFAULT: usize       = 1   * 1_024 * 1_024;  //    1 MB

    /// Returns the maximum allowed payload size for the given CPP message-type
    /// byte (the `msg_type` byte from the wire header).
    pub fn max_for_type(msg_type_byte: u8) -> usize {
        match msg_type_byte {
            // Handshake
            0x01 | 0x02 => Self::HANDSHAKE,
            // Sync: individual block batches may reach BLOCK limit
            0x10 | 0x11 | 0x13 | 0x14 => Self::DEFAULT,
            // Block batch responses
            0x12 => Self::BLOCK,
            // NewBlock propagation
            0x20 => Self::BLOCK,
            // Transaction propagation
            0x21 => Self::TRANSACTION,
            // Mining work (contains txs, use BLOCK limit)
            0x30 | 0x31 | 0x32 | 0x33 | 0x34 => Self::DEFAULT,
            // Control messages are tiny
            0xF0 | 0xF1 | 0xFF => Self::HANDSHAKE,
            // Unknown — apply conservative default
            _ => Self::DEFAULT,
        }
    }
}

// ---------------------------------------------------------------------------
// ConnectionLimiter (item 4)
// ---------------------------------------------------------------------------

/// Tracks in-flight connections by IP and total count.
///
/// Call `try_acquire(ip)` before accepting a connection; call `release(ip)`
/// when the connection is closed (regardless of success or failure).
pub struct ConnectionLimiter {
    per_ip: HashMap<IpAddr, usize>,
    total: usize,
    max_per_ip: usize,
    max_total: usize,
}

#[derive(Debug, Clone)]
pub enum ConnectionDenied {
    TooManyFromIp { ip: IpAddr, current: usize, max: usize },
    TotalCapacityReached { current: usize, max: usize },
}

impl std::fmt::Display for ConnectionDenied {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooManyFromIp { ip, current, max } =>
                write!(f, "Too many connections from {} ({}/{})", ip, current, max),
            Self::TotalCapacityReached { current, max } =>
                write!(f, "Total connection capacity reached ({}/{})", current, max),
        }
    }
}

impl ConnectionLimiter {
    pub fn new(max_per_ip: usize, max_total: usize) -> Self {
        Self {
            per_ip: HashMap::new(),
            total: 0,
            max_per_ip,
            max_total,
        }
    }

    /// Attempt to acquire a connection slot for `ip`.
    ///
    /// Returns `Ok(())` on success.  The caller **must** call `release(ip)` when
    /// the connection ends.
    pub fn try_acquire(&mut self, ip: IpAddr) -> Result<(), ConnectionDenied> {
        if self.total >= self.max_total {
            return Err(ConnectionDenied::TotalCapacityReached {
                current: self.total,
                max: self.max_total,
            });
        }
        let count = self.per_ip.entry(ip).or_insert(0);
        if *count >= self.max_per_ip {
            return Err(ConnectionDenied::TooManyFromIp {
                ip,
                current: *count,
                max: self.max_per_ip,
            });
        }
        *count += 1;
        self.total += 1;
        Ok(())
    }

    /// Release the connection slot previously acquired for `ip`.
    pub fn release(&mut self, ip: IpAddr) {
        if let Some(count) = self.per_ip.get_mut(&ip) {
            if *count > 0 {
                *count -= 1;
                self.total = self.total.saturating_sub(1);
            }
            if *count == 0 {
                self.per_ip.remove(&ip);
            }
        }
    }

    pub fn total(&self) -> usize {
        self.total
    }

    pub fn count_for_ip(&self, ip: IpAddr) -> usize {
        self.per_ip.get(&ip).copied().unwrap_or(0)
    }
}

impl Default for ConnectionLimiter {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_CONNS_PER_IP, DEFAULT_MAX_TOTAL_CONNECTIONS)
    }
}

// ---------------------------------------------------------------------------
// BanList (item 6)
// ---------------------------------------------------------------------------

/// Reason for a ban, for logging and operator inspection.
#[derive(Debug, Clone)]
pub struct BanEntry {
    pub reason: String,
    pub expires_at: Instant,
    pub banned_at: Instant,
}

impl BanEntry {
    fn new(reason: &str, duration: Duration) -> Self {
        let now = Instant::now();
        BanEntry {
            reason: reason.to_string(),
            banned_at: now,
            expires_at: now + duration,
        }
    }

    pub fn is_expired(&self) -> bool {
        Instant::now() >= self.expires_at
    }
}

/// Ban list for both IP addresses and peer IDs (32-byte arrays).
///
/// Peers are banned for sending malformed messages, exceeding rate limits,
/// or otherwise behaving maliciously.  Bans auto-expire after `ban_duration`.
pub struct BanList {
    banned_ips: HashMap<IpAddr, BanEntry>,
    banned_peers: HashMap<[u8; 32], BanEntry>,
    default_ban_duration: Duration,
    short_ban_duration: Duration,
}

impl BanList {
    pub fn new(default_ban_duration: Duration) -> Self {
        BanList {
            banned_ips: HashMap::new(),
            banned_peers: HashMap::new(),
            default_ban_duration,
            short_ban_duration: SHORT_BAN_DURATION,
        }
    }

    // -- Ban operations --

    pub fn ban_ip(&mut self, ip: IpAddr, reason: &str) {
        tracing::warn!("[SECURITY][BAN] Banning IP {} for {}", ip, reason);
        self.banned_ips.insert(ip, BanEntry::new(reason, self.default_ban_duration));
    }

    pub fn ban_ip_short(&mut self, ip: IpAddr, reason: &str) {
        tracing::info!("[SECURITY][BAN_SHORT] Short-banning IP {} for {}", ip, reason);
        self.banned_ips.insert(ip, BanEntry::new(reason, self.short_ban_duration));
    }

    pub fn ban_peer(&mut self, peer_id: &[u8; 32], reason: &str) {
        let id_hex: String = peer_id.iter().take(4).map(|b| format!("{:02x}", b)).collect();
        tracing::warn!("[SECURITY][BAN] Banning peer {} for {}", id_hex, reason);
        self.banned_peers.insert(*peer_id, BanEntry::new(reason, self.default_ban_duration));
    }

    pub fn ban_peer_short(&mut self, peer_id: &[u8; 32], reason: &str) {
        let id_hex: String = peer_id.iter().take(4).map(|b| format!("{:02x}", b)).collect();
        tracing::info!("[SECURITY][BAN_SHORT] Short-banning peer {} for {}", id_hex, reason);
        self.banned_peers.insert(*peer_id, BanEntry::new(reason, self.short_ban_duration));
    }

    // -- Query operations --

    pub fn is_ip_banned(&self, ip: IpAddr) -> bool {
        self.banned_ips.get(&ip)
            .map(|e| !e.is_expired())
            .unwrap_or(false)
    }

    pub fn is_peer_banned(&self, peer_id: &[u8; 32]) -> bool {
        self.banned_peers.get(peer_id)
            .map(|e| !e.is_expired())
            .unwrap_or(false)
    }

    /// Returns the ban entry for an IP if it is currently banned.
    pub fn ip_ban_entry(&self, ip: IpAddr) -> Option<&BanEntry> {
        self.banned_ips.get(&ip).filter(|e| !e.is_expired())
    }

    // -- Maintenance --

    /// Remove all expired ban entries.  Call this periodically (e.g., every
    /// 60 s in the cleanup loop).
    pub fn cleanup_expired(&mut self) {
        self.banned_ips.retain(|_, e| !e.is_expired());
        self.banned_peers.retain(|_, e| !e.is_expired());
    }

    pub fn banned_ip_count(&self) -> usize {
        self.banned_ips.values().filter(|e| !e.is_expired()).count()
    }

    pub fn banned_peer_count(&self) -> usize {
        self.banned_peers.values().filter(|e| !e.is_expired()).count()
    }
}

impl Default for BanList {
    fn default() -> Self {
        Self::new(DEFAULT_BAN_DURATION)
    }
}

// ---------------------------------------------------------------------------
// Token-bucket rate limiter (item 5)
// ---------------------------------------------------------------------------

/// Token-bucket rate limiter for a single peer.
///
/// Allows burst traffic up to `capacity` messages, then throttles to
/// `refill_rate` messages per second.  When the bucket is empty an
/// incoming message is *dropped* and the peer accrues a strike.
pub struct TokenBucket {
    tokens: f64,
    capacity: f64,
    refill_rate: f64, // tokens per second
    last_refill: Instant,
    strikes: u32,
}

impl TokenBucket {
    pub fn new(capacity: f64, refill_rate: f64) -> Self {
        TokenBucket {
            tokens: capacity,
            capacity,
            refill_rate,
            last_refill: Instant::now(),
            strikes: 0,
        }
    }

    /// Attempt to consume one token.  Returns `true` if allowed.
    pub fn try_consume(&mut self) -> bool {
        self.refill();
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            self.strikes += 1;
            false
        }
    }

    /// Add tokens proportional to elapsed time.
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.capacity);
        self.last_refill = now;
    }

    pub fn strikes(&self) -> u32 {
        self.strikes
    }

    pub fn reset_strikes(&mut self) {
        self.strikes = 0;
    }
}

// ---------------------------------------------------------------------------
// Eclipse attack protection (item 8)
// ---------------------------------------------------------------------------

/// Tracks how many connected peers share a /16 (IPv4) or /32 (IPv6)
/// prefix, enforcing a cap to prevent eclipse attacks from a single AS.
///
/// Peers from the same /16 subnet are likely on the same network or
/// hosted by the same provider and can cooperate to isolate the node.
pub struct EclipseGuard {
    subnet_counts: HashMap<SubnetKey, usize>,
    max_per_subnet: usize,
}

/// Subnet key: first two octets of IPv4, first four octets of IPv6
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SubnetKey {
    V4([u8; 2]),  // /16
    V6([u8; 4]),  // /32
    Loopback,
}

impl SubnetKey {
    pub fn from_ip(ip: IpAddr) -> Self {
        match ip {
            IpAddr::V4(v4) => {
                if v4.is_loopback() || v4.is_private() {
                    return SubnetKey::Loopback;
                }
                let octets = v4.octets();
                SubnetKey::V4([octets[0], octets[1]])
            }
            IpAddr::V6(v6) => {
                if v6.is_loopback() {
                    return SubnetKey::Loopback;
                }
                // Try to handle IPv4-mapped IPv6 addresses
                if let Some(v4) = v6.to_ipv4_mapped().or_else(|| v6.to_ipv4()) {
                    let octets = v4.octets();
                    if v4.is_loopback() || v4.is_private() {
                        return SubnetKey::Loopback;
                    }
                    return SubnetKey::V4([octets[0], octets[1]]);
                }
                let segments = v6.octets();
                SubnetKey::V6([segments[0], segments[1], segments[2], segments[3]])
            }
        }
    }
}

impl EclipseGuard {
    pub fn new(max_per_subnet: usize) -> Self {
        EclipseGuard {
            subnet_counts: HashMap::new(),
            max_per_subnet,
        }
    }

    /// Attempt to add a peer from `ip`.  Returns `true` if the peer is allowed
    /// (subnet is not full), `false` if it would violate the diversity limit.
    ///
    /// Loopback / private addresses always succeed (for local testing).
    pub fn try_add(&mut self, ip: IpAddr) -> bool {
        let key = SubnetKey::from_ip(ip);
        if key == SubnetKey::Loopback {
            return true; // always allow local peers
        }
        let count = self.subnet_counts.entry(key).or_insert(0);
        if *count >= self.max_per_subnet {
            tracing::warn!("[SECURITY][ECLIPSE] Rejecting peer from {:?}: subnet full ({}/{})",
                key, count, self.max_per_subnet);
            return false;
        }
        *count += 1;
        true
    }

    /// Remove a peer from `ip` when it disconnects.
    pub fn remove(&mut self, ip: IpAddr) {
        let key = SubnetKey::from_ip(ip);
        if key == SubnetKey::Loopback {
            return;
        }
        if let Some(count) = self.subnet_counts.get_mut(&key) {
            if *count > 0 {
                *count -= 1;
            }
            if *count == 0 {
                self.subnet_counts.remove(&key);
            }
        }
    }

    /// Number of distinct /16 subnets currently represented.
    pub fn subnet_diversity(&self) -> usize {
        self.subnet_counts.len()
    }
}

impl Default for EclipseGuard {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_PEERS_PER_SUBNET)
    }
}

// ---------------------------------------------------------------------------
// Network security metrics (item 10)
// ---------------------------------------------------------------------------

/// Counters and gauges for network security observability.
///
/// These are intentionally simple (not Prometheus-wrapped here) so that the
/// rest of the code can update them lock-free via `Arc<Mutex<>>`.  A separate
/// metrics layer can read them and publish to Prometheus.
#[derive(Debug, Clone)]
pub struct NetworkSecurityMetrics {
    // Connection state
    pub connected_peers: u64,
    pub peak_connected_peers: u64,

    // Message traffic (indexed by message-type byte 0x00..=0xFF)
    pub messages_sent: [u64; 256],
    pub messages_received: [u64; 256],
    pub bytes_sent: u64,
    pub bytes_received: u64,

    // Security events
    pub connections_rejected_ban: u64,
    pub connections_rejected_ip_limit: u64,
    pub connections_rejected_eclipse: u64,
    pub connections_rejected_total_limit: u64,
    pub auth_failures: u64,
    pub messages_dropped_rate_limit: u64,
    pub messages_dropped_size_limit: u64,
    pub messages_dropped_malformed: u64,
    pub peers_banned: u64,
    pub ips_banned: u64,

    // Peer churn
    pub peer_connects: u64,
    pub peer_disconnects: u64,
}

impl Default for NetworkSecurityMetrics {
    fn default() -> Self {
        NetworkSecurityMetrics {
            connected_peers: 0,
            peak_connected_peers: 0,
            messages_sent: [0u64; 256],
            messages_received: [0u64; 256],
            bytes_sent: 0,
            bytes_received: 0,
            connections_rejected_ban: 0,
            connections_rejected_ip_limit: 0,
            connections_rejected_eclipse: 0,
            connections_rejected_total_limit: 0,
            auth_failures: 0,
            messages_dropped_rate_limit: 0,
            messages_dropped_size_limit: 0,
            messages_dropped_malformed: 0,
            peers_banned: 0,
            ips_banned: 0,
            peer_connects: 0,
            peer_disconnects: 0,
        }
    }
}

impl NetworkSecurityMetrics {
    pub fn on_peer_connect(&mut self) {
        self.peer_connects += 1;
        self.connected_peers += 1;
        if self.connected_peers > self.peak_connected_peers {
            self.peak_connected_peers = self.connected_peers;
        }
    }

    pub fn on_peer_disconnect(&mut self) {
        self.peer_disconnects += 1;
        self.connected_peers = self.connected_peers.saturating_sub(1);
    }

    pub fn on_message_sent(&mut self, msg_type_byte: u8, bytes: u64) {
        self.messages_sent[msg_type_byte as usize] += 1;
        self.bytes_sent += bytes;
    }

    pub fn on_message_received(&mut self, msg_type_byte: u8, bytes: u64) {
        self.messages_received[msg_type_byte as usize] += 1;
        self.bytes_received += bytes;
    }

    /// Churn rate proxy: (connects + disconnects) total
    pub fn churn(&self) -> u64 {
        self.peer_connects + self.peer_disconnects
    }

    /// Total messages sent across all types
    pub fn total_messages_sent(&self) -> u64 {
        self.messages_sent.iter().sum()
    }

    /// Total messages received across all types
    pub fn total_messages_received(&self) -> u64 {
        self.messages_received.iter().sum()
    }
}

// ---------------------------------------------------------------------------
// DNS seed validator (item 7)
// ---------------------------------------------------------------------------

/// Validates and deduplicates peer addresses obtained from DNS seed queries.
///
/// The main concerns with DNS seeds are:
/// 1. A compromised seed returning only attacker-controlled peers
/// 2. Seeds returning the same peers (eclipse risk)
/// 3. DNS poisoning / BGP hijack
///
/// Mitigation: require agreement from ≥ 2 independent seeds before using an
/// address, and enforce subnet diversity on the resulting set.
pub struct DnsSeedValidator {
    /// How many seeds must return an address before it is trusted
    min_seed_agreement: usize,
}

impl DnsSeedValidator {
    pub fn new(min_seed_agreement: usize) -> Self {
        DnsSeedValidator { min_seed_agreement }
    }

    /// Given results from multiple DNS seeds, return only addresses that
    /// appear in ≥ `min_seed_agreement` seeds AND survive subnet diversity
    /// filtering.
    pub fn validate(
        &self,
        seed_results: &[Vec<std::net::SocketAddr>],
        eclipse_guard: &EclipseGuard,
    ) -> Vec<std::net::SocketAddr> {
        use std::collections::HashMap;

        if seed_results.len() < self.min_seed_agreement {
            tracing::warn!("[SECURITY][DNS_SEED] Only {} seeds provided, need ≥ {}; using all addresses",
                seed_results.len(), self.min_seed_agreement);
        }

        // Count how many seeds returned each address
        let mut addr_counts: HashMap<std::net::SocketAddr, usize> = HashMap::new();
        for results in seed_results {
            // Deduplicate within a single seed result first
            let mut seen = std::collections::HashSet::new();
            for &addr in results {
                if seen.insert(addr) {
                    *addr_counts.entry(addr).or_insert(0) += 1;
                }
            }
        }

        // Keep only addresses with ≥ min_seed_agreement votes
        let threshold = if seed_results.len() >= self.min_seed_agreement {
            self.min_seed_agreement
        } else {
            1
        };

        // Also enforce subnet diversity on the final candidate set
        let mut guard = EclipseGuard::new(eclipse_guard.max_per_subnet);
        let mut validated: Vec<std::net::SocketAddr> = Vec::new();

        // Sort by vote count descending for deterministic output
        let mut candidates: Vec<_> = addr_counts.into_iter()
            .filter(|(_, count)| *count >= threshold)
            .collect();
        candidates.sort_by(|a, b| b.1.cmp(&a.1));

        for (addr, _) in candidates {
            if guard.try_add(addr.ip()) {
                validated.push(addr);
            }
        }

        tracing::info!("[SECURITY][DNS_SEED] Validated {} peer addresses from {} seeds",
            validated.len(), seed_results.len());

        validated
    }
}

impl Default for DnsSeedValidator {
    fn default() -> Self {
        Self::new(2)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    fn ipv4(a: u8, b: u8, c: u8, d: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(a, b, c, d))
    }

    // --- MessageSizePolicy ---

    #[test]
    fn test_message_size_policy() {
        // Transaction limit
        assert_eq!(MessageSizePolicy::max_for_type(0x21), MessageSizePolicy::TRANSACTION);
        // Block limit
        assert_eq!(MessageSizePolicy::max_for_type(0x12), MessageSizePolicy::BLOCK);
        assert_eq!(MessageSizePolicy::max_for_type(0x20), MessageSizePolicy::BLOCK);
        // Handshake limit
        assert_eq!(MessageSizePolicy::max_for_type(0x01), MessageSizePolicy::HANDSHAKE);
        assert_eq!(MessageSizePolicy::max_for_type(0xF0), MessageSizePolicy::HANDSHAKE);
        // Sizes are in expected order
        assert!(MessageSizePolicy::BLOCK > MessageSizePolicy::TRANSACTION);
        assert!(MessageSizePolicy::TRANSACTION > MessageSizePolicy::CONSENSUS);
        assert!(MessageSizePolicy::CONSENSUS > MessageSizePolicy::HANDSHAKE);
    }

    // --- ConnectionLimiter ---

    #[test]
    fn test_connection_limiter_per_ip() {
        let mut limiter = ConnectionLimiter::new(3, 128);
        let ip = ipv4(1, 2, 3, 4);

        // First 3 connections succeed
        assert!(limiter.try_acquire(ip).is_ok());
        assert!(limiter.try_acquire(ip).is_ok());
        assert!(limiter.try_acquire(ip).is_ok());
        assert_eq!(limiter.count_for_ip(ip), 3);

        // 4th is rejected
        assert!(limiter.try_acquire(ip).is_err());

        // After release, another is allowed
        limiter.release(ip);
        assert!(limiter.try_acquire(ip).is_ok());
    }

    #[test]
    fn test_connection_limiter_total() {
        let mut limiter = ConnectionLimiter::new(10, 2);
        let ip1 = ipv4(1, 2, 3, 4);
        let ip2 = ipv4(5, 6, 7, 8);

        assert!(limiter.try_acquire(ip1).is_ok());
        assert!(limiter.try_acquire(ip2).is_ok());
        // Total cap reached
        assert!(matches!(
            limiter.try_acquire(ipv4(9, 10, 11, 12)),
            Err(ConnectionDenied::TotalCapacityReached { .. })
        ));
    }

    // --- BanList ---

    #[test]
    fn test_ban_list_ip() {
        let mut ban_list = BanList::new(Duration::from_secs(3600));
        let ip = ipv4(10, 0, 0, 1);
        assert!(!ban_list.is_ip_banned(ip));
        ban_list.ban_ip(ip, "test");
        assert!(ban_list.is_ip_banned(ip));
    }

    #[test]
    fn test_ban_list_peer() {
        let mut ban_list = BanList::new(Duration::from_secs(3600));
        let peer_id = [1u8; 32];
        assert!(!ban_list.is_peer_banned(&peer_id));
        ban_list.ban_peer(&peer_id, "test");
        assert!(ban_list.is_peer_banned(&peer_id));
    }

    #[test]
    fn test_ban_expiry() {
        let mut ban_list = BanList::new(Duration::from_nanos(1));
        let ip = ipv4(10, 0, 0, 2);
        ban_list.ban_ip(ip, "test");
        // Sleep just enough for expiry
        std::thread::sleep(Duration::from_millis(1));
        assert!(!ban_list.is_ip_banned(ip));

        ban_list.cleanup_expired();
        assert_eq!(ban_list.banned_ip_count(), 0);
    }

    // --- TokenBucket ---

    #[test]
    fn test_token_bucket_allows_burst() {
        let mut bucket = TokenBucket::new(5.0, 1.0);
        for _ in 0..5 {
            assert!(bucket.try_consume(), "should allow burst");
        }
        // 6th message exceeds burst capacity immediately
        assert!(!bucket.try_consume(), "should be rate-limited");
        assert_eq!(bucket.strikes(), 1);
    }

    // --- EclipseGuard ---

    #[test]
    fn test_eclipse_guard_subnet_limit() {
        let mut guard = EclipseGuard::new(2);

        let ip1 = ipv4(1, 2, 3, 4);
        let ip2 = ipv4(1, 2, 10, 20);
        let ip3 = ipv4(1, 2, 50, 60); // same /16

        assert!(guard.try_add(ip1));
        assert!(guard.try_add(ip2));
        assert!(!guard.try_add(ip3)); // subnet full

        // After removing one, another from same subnet is allowed
        guard.remove(ip1);
        assert!(guard.try_add(ip3));
    }

    #[test]
    fn test_eclipse_guard_loopback_always_allowed() {
        let mut guard = EclipseGuard::new(1);
        let loopback = IpAddr::V4(Ipv4Addr::LOCALHOST);
        // Can add as many loopback peers as we want
        assert!(guard.try_add(loopback));
        assert!(guard.try_add(loopback));
        assert!(guard.try_add(loopback));
    }

    // --- SubnetKey ---

    #[test]
    fn test_subnet_key_v4() {
        let ip = ipv4(192, 168, 1, 100);
        // Private IPs map to Loopback (allowed without restriction)
        assert_eq!(SubnetKey::from_ip(ip), SubnetKey::Loopback);

        let public_ip = ipv4(8, 8, 8, 8);
        assert_eq!(SubnetKey::from_ip(public_ip), SubnetKey::V4([8, 8]));
    }
}
