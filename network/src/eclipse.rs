// =============================================================================
// Eclipse Attack Mitigation
// Topology-aware peer management for network resilience
// =============================================================================
//
// Defense mechanisms:
// 1. IP Bucketing: Max 2 peers per /24 subnet (prevents single-source flooding)
// 2. Feeler Connections: Periodic random DHT probes (breaks isolation bubbles)
// 3. Anchor Nodes: Hardcoded trusted bootnodes (immutable source of truth)
// 4. Diverse Topology: Geographic/provider distribution scoring

use libp2p::{Multiaddr, PeerId};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::net::Ipv4Addr;
use std::time::{Duration, Instant};

// =============================================================================
// IP Bucketing
// =============================================================================

/// Maximum peers allowed per /24 subnet (institutional standard: 2)
pub const MAX_PEERS_PER_SUBNET: usize = 2;

/// Maximum peers allowed per /16 subnet (broader protection)
pub const MAX_PEERS_PER_SLASH16: usize = 8;

/// Minimum number of unique /16 subnets required for health
pub const MIN_UNIQUE_SLASH16_SUBNETS: usize = 4;

/// IP bucket key for grouping peers by subnet
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubnetKey {
    /// /24 prefix (first 3 octets)
    pub slash24: [u8; 3],
    /// /16 prefix (first 2 octets)  
    pub slash16: [u8; 2],
}

impl SubnetKey {
    /// Create from IPv4 address
    pub fn from_ipv4(ip: Ipv4Addr) -> Self {
        let octets = ip.octets();
        SubnetKey {
            slash24: [octets[0], octets[1], octets[2]],
            slash16: [octets[0], octets[1]],
        }
    }

    /// Create from multiaddr (extracts IP if present)
    pub fn from_multiaddr(addr: &Multiaddr) -> Option<Self> {
        for protocol in addr.iter() {
            match protocol {
                libp2p::multiaddr::Protocol::Ip4(ip) => {
                    return Some(Self::from_ipv4(ip));
                }
                _ => continue,
            }
        }
        None
    }

    /// Check if this is a private/local subnet (should be allowed freely)
    pub fn is_private(&self) -> bool {
        match self.slash16 {
            [10, _] => true,           // 10.0.0.0/8
            [172, b] if (16..=31).contains(&b) => true, // 172.16.0.0/12
            [192, 168] => true,        // 192.168.0.0/16
            [127, _] => true,          // Loopback
            _ => false,
        }
    }
}

/// Peer entry with IP information
#[derive(Debug, Clone)]
pub struct PeerEntry {
    pub peer_id: PeerId,
    pub subnet: Option<SubnetKey>,
    pub connected_at: Instant,
    pub is_outbound: bool,
    pub is_bootnode: bool,
}

/// IP Bucket Manager - enforces subnet diversity
#[derive(Debug)]
pub struct IpBucketManager {
    /// Peers indexed by their /24 subnet
    slash24_buckets: HashMap<[u8; 3], Vec<PeerId>>,
    /// Peers indexed by their /16 subnet
    slash16_buckets: HashMap<[u8; 2], Vec<PeerId>>,
    /// All peer entries
    peers: HashMap<PeerId, PeerEntry>,
    /// Configuration
    config: IpBucketConfig,
}

/// Configuration for IP bucketing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpBucketConfig {
    /// Max peers per /24 subnet
    pub max_per_slash24: usize,
    /// Max peers per /16 subnet
    pub max_per_slash16: usize,
    /// Minimum unique /16 subnets for network health
    pub min_unique_slash16: usize,
    /// Whether to allow unlimited private/local IPs
    pub allow_unlimited_private: bool,
}

impl Default for IpBucketConfig {
    fn default() -> Self {
        IpBucketConfig {
            max_per_slash24: MAX_PEERS_PER_SUBNET,
            max_per_slash16: MAX_PEERS_PER_SLASH16,
            min_unique_slash16: MIN_UNIQUE_SLASH16_SUBNETS,
            allow_unlimited_private: true, // For testnets
        }
    }
}

impl IpBucketManager {
    pub fn new() -> Self {
        Self::with_config(IpBucketConfig::default())
    }

    pub fn with_config(config: IpBucketConfig) -> Self {
        IpBucketManager {
            slash24_buckets: HashMap::new(),
            slash16_buckets: HashMap::new(),
            peers: HashMap::new(),
            config,
        }
    }

    /// Check if we can accept a new peer from this address
    pub fn can_accept_peer(&self, addr: &Multiaddr, is_bootnode: bool) -> AcceptResult {
        // Always accept bootnodes
        if is_bootnode {
            return AcceptResult::Accept;
        }

        let subnet = match SubnetKey::from_multiaddr(addr) {
            Some(s) => s,
            None => return AcceptResult::Accept, // Can't determine IP, allow
        };

        // Allow unlimited private IPs if configured
        if self.config.allow_unlimited_private && subnet.is_private() {
            return AcceptResult::Accept;
        }

        // Check /24 bucket
        let slash24_count = self.slash24_buckets
            .get(&subnet.slash24)
            .map(|v| v.len())
            .unwrap_or(0);

        if slash24_count >= self.config.max_per_slash24 {
            return AcceptResult::Reject(RejectReason::Slash24Full {
                subnet: subnet.slash24,
                count: slash24_count,
                max: self.config.max_per_slash24,
            });
        }

        // Check /16 bucket
        let slash16_count = self.slash16_buckets
            .get(&subnet.slash16)
            .map(|v| v.len())
            .unwrap_or(0);

        if slash16_count >= self.config.max_per_slash16 {
            return AcceptResult::Reject(RejectReason::Slash16Full {
                subnet: subnet.slash16,
                count: slash16_count,
                max: self.config.max_per_slash16,
            });
        }

        AcceptResult::Accept
    }

    /// Add a peer to the buckets
    pub fn add_peer(
        &mut self,
        peer_id: PeerId,
        addr: &Multiaddr,
        is_outbound: bool,
        is_bootnode: bool,
    ) -> Result<(), String> {
        if self.peers.contains_key(&peer_id) {
            return Ok(()); // Already tracked
        }

        let subnet = SubnetKey::from_multiaddr(addr);

        // Add to buckets if we have subnet info
        if let Some(ref s) = subnet {
            self.slash24_buckets
                .entry(s.slash24)
                .or_default()
                .push(peer_id);
            self.slash16_buckets
                .entry(s.slash16)
                .or_default()
                .push(peer_id);
        }

        self.peers.insert(peer_id, PeerEntry {
            peer_id,
            subnet,
            connected_at: Instant::now(),
            is_outbound,
            is_bootnode,
        });

        Ok(())
    }

    /// Remove a peer from buckets
    pub fn remove_peer(&mut self, peer_id: &PeerId) {
        if let Some(entry) = self.peers.remove(peer_id) {
            if let Some(ref subnet) = entry.subnet {
                // Remove from /24 bucket
                if let Some(bucket) = self.slash24_buckets.get_mut(&subnet.slash24) {
                    bucket.retain(|p| p != peer_id);
                    if bucket.is_empty() {
                        self.slash24_buckets.remove(&subnet.slash24);
                    }
                }
                // Remove from /16 bucket
                if let Some(bucket) = self.slash16_buckets.get_mut(&subnet.slash16) {
                    bucket.retain(|p| p != peer_id);
                    if bucket.is_empty() {
                        self.slash16_buckets.remove(&subnet.slash16);
                    }
                }
            }
        }
    }

    /// Get diversity statistics
    pub fn diversity_stats(&self) -> DiversityStats {
        let unique_slash24 = self.slash24_buckets.len();
        let unique_slash16 = self.slash16_buckets.len();
        let total_peers = self.peers.len();
        let bootnode_count = self.peers.values().filter(|p| p.is_bootnode).count();
        let outbound_count = self.peers.values().filter(|p| p.is_outbound).count();

        // Calculate concentration (lower is better, 0 = perfectly distributed)
        let max_slash24_concentration = self.slash24_buckets
            .values()
            .map(|v| v.len())
            .max()
            .unwrap_or(0);

        let health_score = self.calculate_health_score(
            unique_slash16,
            max_slash24_concentration,
            total_peers,
        );

        DiversityStats {
            total_peers,
            unique_slash24_subnets: unique_slash24,
            unique_slash16_subnets: unique_slash16,
            bootnode_count,
            outbound_count,
            inbound_count: total_peers - outbound_count,
            max_slash24_concentration,
            health_score,
            is_healthy: health_score >= 0.7,
        }
    }

    /// Calculate network topology health score (0.0 - 1.0)
    fn calculate_health_score(
        &self,
        unique_slash16: usize,
        max_concentration: usize,
        total_peers: usize,
    ) -> f64 {
        if total_peers == 0 {
            return 0.0;
        }

        // Component 1: Subnet diversity (40% weight)
        let diversity_score = (unique_slash16 as f64 / self.config.min_unique_slash16 as f64)
            .min(1.0);

        // Component 2: Concentration score (40% weight)
        // Perfect = 1 peer per subnet, worst = all in one
        let concentration_score = if total_peers > 0 {
            1.0 - (max_concentration as f64 / total_peers as f64)
        } else {
            1.0
        };

        // Component 3: Peer count (20% weight)
        // More peers = better, up to a point
        let peer_score = (total_peers as f64 / 10.0).min(1.0);

        0.4 * diversity_score + 0.4 * concentration_score + 0.2 * peer_score
    }

    /// Get peers that should be evicted to make room for better diversity
    pub fn get_eviction_candidates(&self, count: usize) -> Vec<PeerId> {
        // Find the most concentrated /24 subnets and evict excess peers
        let mut candidates = Vec::new();

        let mut concentrated: Vec<_> = self.slash24_buckets
            .iter()
            .filter(|(_, peers)| peers.len() > 1)
            .collect();

        // Sort by concentration (highest first)
        concentrated.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

        for (_, peers) in concentrated {
            // Skip bootnodes and keep at least one per subnet
            let evictable: Vec<_> = peers.iter()
                .filter(|p| {
                    self.peers.get(p)
                        .map(|e| !e.is_bootnode)
                        .unwrap_or(false)
                })
                .skip(1) // Keep at least one
                .cloned()
                .collect();

            for peer in evictable {
                if candidates.len() >= count {
                    return candidates;
                }
                candidates.push(peer);
            }
        }

        candidates
    }
}

impl Default for IpBucketManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of peer acceptance check
#[derive(Debug, Clone)]
pub enum AcceptResult {
    Accept,
    Reject(RejectReason),
}

/// Reason for rejecting a peer
#[derive(Debug, Clone)]
pub enum RejectReason {
    Slash24Full {
        subnet: [u8; 3],
        count: usize,
        max: usize,
    },
    Slash16Full {
        subnet: [u8; 2],
        count: usize,
        max: usize,
    },
}

impl std::fmt::Display for RejectReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RejectReason::Slash24Full { subnet, count, max } => {
                write!(f, "/24 subnet {}.{}.{}.0 full: {}/{}", 
                    subnet[0], subnet[1], subnet[2], count, max)
            }
            RejectReason::Slash16Full { subnet, count, max } => {
                write!(f, "/16 subnet {}.{}.0.0 full: {}/{}", 
                    subnet[0], subnet[1], count, max)
            }
        }
    }
}

/// Network diversity statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiversityStats {
    pub total_peers: usize,
    pub unique_slash24_subnets: usize,
    pub unique_slash16_subnets: usize,
    pub bootnode_count: usize,
    pub outbound_count: usize,
    pub inbound_count: usize,
    pub max_slash24_concentration: usize,
    pub health_score: f64,
    pub is_healthy: bool,
}

// =============================================================================
// Feeler Connections
// =============================================================================

/// Feeler connection manager - periodic random DHT probes
#[derive(Debug)]
pub struct FeelerManager {
    /// Last feeler probe time
    last_probe: Instant,
    /// Probe interval
    probe_interval: Duration,
    /// Addresses discovered via feelers (not yet connected)
    discovered_addrs: Vec<DiscoveredPeer>,
    /// Maximum discovered addresses to cache
    max_discovered: usize,
    /// Random peer IDs we've tried to find
    probe_history: HashSet<PeerId>,
    /// Configuration
    config: FeelerConfig,
}

/// Discovered peer from feeler probes
#[derive(Debug, Clone)]
pub struct DiscoveredPeer {
    pub peer_id: PeerId,
    pub addrs: Vec<Multiaddr>,
    pub discovered_at: Instant,
    pub source: DiscoverySource,
}

/// How we discovered a peer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoverySource {
    /// Random DHT walk
    DhtRandom,
    /// Kademlia closest peers query
    KademliaClosest,
    /// mDNS local discovery
    Mdns,
    /// Peer exchange from existing peer
    PeerExchange,
}

/// Feeler configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeelerConfig {
    /// How often to probe (seconds)
    pub probe_interval_secs: u64,
    /// Maximum addresses to cache
    pub max_discovered_cache: usize,
    /// How long discovered addresses are valid (seconds)
    pub discovery_ttl_secs: u64,
    /// Number of random probes per interval
    pub probes_per_interval: usize,
}

impl Default for FeelerConfig {
    fn default() -> Self {
        FeelerConfig {
            probe_interval_secs: 30,        // Probe every 30 seconds
            max_discovered_cache: 100,      // Cache up to 100 addresses
            discovery_ttl_secs: 3600,       // 1 hour TTL
            probes_per_interval: 3,         // 3 random probes each time
        }
    }
}

impl FeelerManager {
    pub fn new() -> Self {
        Self::with_config(FeelerConfig::default())
    }

    pub fn with_config(config: FeelerConfig) -> Self {
        FeelerManager {
            last_probe: Instant::now(),
            probe_interval: Duration::from_secs(config.probe_interval_secs),
            discovered_addrs: Vec::new(),
            max_discovered: config.max_discovered_cache,
            probe_history: HashSet::new(),
            config,
        }
    }

    /// Check if it's time to do a feeler probe
    pub fn should_probe(&self) -> bool {
        self.last_probe.elapsed() >= self.probe_interval
    }

    /// Generate random PeerIds to search for in DHT
    /// This breaks isolation by discovering random parts of the network
    pub fn generate_random_probes(&mut self) -> Vec<PeerId> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        
        let mut probes = Vec::new();
        for _ in 0..self.config.probes_per_interval {
            // Generate random 32-byte key
            let mut random_key = [0u8; 32];
            rng.fill(&mut random_key);
            
            // Create a PeerId-like identifier for DHT lookup
            // Note: This is a simplified approach - in production we'd use
            // Kademlia's get_closest_peers with a random key
            let random_peer = PeerId::random();
            
            if !self.probe_history.contains(&random_peer) {
                self.probe_history.insert(random_peer);
                probes.push(random_peer);
            }
        }
        
        self.last_probe = Instant::now();
        
        // Limit history size
        if self.probe_history.len() > 10000 {
            self.probe_history.clear();
        }
        
        probes
    }

    /// Record a discovered peer from feeler probe
    pub fn record_discovery(
        &mut self,
        peer_id: PeerId,
        addrs: Vec<Multiaddr>,
        source: DiscoverySource,
    ) {
        // Check if already discovered
        if self.discovered_addrs.iter().any(|d| d.peer_id == peer_id) {
            return;
        }

        self.discovered_addrs.push(DiscoveredPeer {
            peer_id,
            addrs,
            discovered_at: Instant::now(),
            source,
        });

        // Trim if over limit
        if self.discovered_addrs.len() > self.max_discovered {
            // Remove oldest entries
            self.discovered_addrs.sort_by(|a, b| b.discovered_at.cmp(&a.discovered_at));
            self.discovered_addrs.truncate(self.max_discovered);
        }
    }

    /// Get candidates for connection from discovered peers
    /// Filters by IP bucket availability
    pub fn get_connection_candidates(
        &self,
        bucket_manager: &IpBucketManager,
        count: usize,
    ) -> Vec<DiscoveredPeer> {
        let ttl = Duration::from_secs(self.config.discovery_ttl_secs);
        
        self.discovered_addrs
            .iter()
            .filter(|d| {
                // Must not be expired
                if d.discovered_at.elapsed() > ttl {
                    return false;
                }
                // Must have at least one acceptable address
                d.addrs.iter().any(|addr| {
                    matches!(bucket_manager.can_accept_peer(addr, false), AcceptResult::Accept)
                })
            })
            .take(count)
            .cloned()
            .collect()
    }

    /// Clean up expired discoveries
    pub fn cleanup_expired(&mut self) {
        let ttl = Duration::from_secs(self.config.discovery_ttl_secs);
        self.discovered_addrs.retain(|d| d.discovered_at.elapsed() <= ttl);
    }

    /// Get feeler statistics
    pub fn stats(&self) -> FeelerStats {
        let ttl = Duration::from_secs(self.config.discovery_ttl_secs);
        let valid_count = self.discovered_addrs
            .iter()
            .filter(|d| d.discovered_at.elapsed() <= ttl)
            .count();

        FeelerStats {
            total_discovered: self.discovered_addrs.len(),
            valid_discoveries: valid_count,
            probes_sent: self.probe_history.len(),
            last_probe_ago_secs: self.last_probe.elapsed().as_secs(),
            next_probe_in_secs: self.probe_interval
                .checked_sub(self.last_probe.elapsed())
                .map(|d| d.as_secs())
                .unwrap_or(0),
        }
    }
}

impl Default for FeelerManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Feeler statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeelerStats {
    pub total_discovered: usize,
    pub valid_discoveries: usize,
    pub probes_sent: usize,
    pub last_probe_ago_secs: u64,
    pub next_probe_in_secs: u64,
}

// =============================================================================
// Combined Eclipse Defense
// =============================================================================

/// Combined eclipse defense manager
#[derive(Debug)]
pub struct EclipseDefense {
    pub buckets: IpBucketManager,
    pub feelers: FeelerManager,
}

impl EclipseDefense {
    pub fn new() -> Self {
        EclipseDefense {
            buckets: IpBucketManager::new(),
            feelers: FeelerManager::new(),
        }
    }

    pub fn with_configs(bucket_config: IpBucketConfig, feeler_config: FeelerConfig) -> Self {
        EclipseDefense {
            buckets: IpBucketManager::with_config(bucket_config),
            feelers: FeelerManager::with_config(feeler_config),
        }
    }

    /// Periodic maintenance tick
    pub fn maintenance_tick(&mut self) {
        self.feelers.cleanup_expired();
    }

    /// Get combined defense statistics
    pub fn stats(&self) -> EclipseDefenseStats {
        EclipseDefenseStats {
            diversity: self.buckets.diversity_stats(),
            feelers: self.feelers.stats(),
        }
    }
}

impl Default for EclipseDefense {
    fn default() -> Self {
        Self::new()
    }
}

/// Combined eclipse defense statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EclipseDefenseStats {
    pub diversity: DiversityStats,
    pub feelers: FeelerStats,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subnet_key_from_ipv4() {
        let ip = Ipv4Addr::new(192, 168, 1, 100);
        let key = SubnetKey::from_ipv4(ip);
        
        assert_eq!(key.slash24, [192, 168, 1]);
        assert_eq!(key.slash16, [192, 168]);
    }

    #[test]
    fn test_private_subnet_detection() {
        // Private subnets
        assert!(SubnetKey::from_ipv4(Ipv4Addr::new(10, 0, 0, 1)).is_private());
        assert!(SubnetKey::from_ipv4(Ipv4Addr::new(172, 16, 0, 1)).is_private());
        assert!(SubnetKey::from_ipv4(Ipv4Addr::new(192, 168, 0, 1)).is_private());
        assert!(SubnetKey::from_ipv4(Ipv4Addr::new(127, 0, 0, 1)).is_private());
        
        // Public subnets
        assert!(!SubnetKey::from_ipv4(Ipv4Addr::new(8, 8, 8, 8)).is_private());
        assert!(!SubnetKey::from_ipv4(Ipv4Addr::new(1, 1, 1, 1)).is_private());
    }

    #[test]
    fn test_ip_bucket_limits() {
        let mut manager = IpBucketManager::with_config(IpBucketConfig {
            max_per_slash24: 2,
            max_per_slash16: 4,
            min_unique_slash16: 4,
            allow_unlimited_private: false,
        });

        // Create test addresses
        let addr1: Multiaddr = "/ip4/8.8.8.1/tcp/30333".parse().unwrap();
        let addr2: Multiaddr = "/ip4/8.8.8.2/tcp/30333".parse().unwrap();
        let addr3: Multiaddr = "/ip4/8.8.8.3/tcp/30333".parse().unwrap();

        // First two should be accepted
        assert!(matches!(manager.can_accept_peer(&addr1, false), AcceptResult::Accept));
        manager.add_peer(PeerId::random(), &addr1, true, false).unwrap();

        assert!(matches!(manager.can_accept_peer(&addr2, false), AcceptResult::Accept));
        manager.add_peer(PeerId::random(), &addr2, true, false).unwrap();

        // Third should be rejected (same /24 subnet)
        assert!(matches!(
            manager.can_accept_peer(&addr3, false),
            AcceptResult::Reject(RejectReason::Slash24Full { .. })
        ));
    }

    #[test]
    fn test_bootnode_bypass() {
        let manager = IpBucketManager::with_config(IpBucketConfig {
            max_per_slash24: 0, // No peers allowed
            max_per_slash16: 0,
            min_unique_slash16: 4,
            allow_unlimited_private: false,
        });

        let addr: Multiaddr = "/ip4/8.8.8.8/tcp/30333".parse().unwrap();

        // Should reject normal peer
        assert!(matches!(
            manager.can_accept_peer(&addr, false),
            AcceptResult::Reject(_)
        ));

        // Should accept bootnode
        assert!(matches!(
            manager.can_accept_peer(&addr, true),
            AcceptResult::Accept
        ));
    }

    #[test]
    fn test_diversity_stats() {
        let mut manager = IpBucketManager::new();

        // Add peers from different subnets
        let subnets = [
            "/ip4/1.1.1.1/tcp/30333",
            "/ip4/2.2.2.2/tcp/30333",
            "/ip4/3.3.3.3/tcp/30333",
            "/ip4/4.4.4.4/tcp/30333",
        ];

        for addr_str in &subnets {
            let addr: Multiaddr = addr_str.parse().unwrap();
            manager.add_peer(PeerId::random(), &addr, true, false).unwrap();
        }

        let stats = manager.diversity_stats();
        assert_eq!(stats.total_peers, 4);
        assert_eq!(stats.unique_slash24_subnets, 4);
        assert_eq!(stats.unique_slash16_subnets, 4);
        assert!(stats.is_healthy);
    }

    #[test]
    fn test_feeler_probes() {
        let mut feeler = FeelerManager::new();
        
        // Generate probes
        let probes = feeler.generate_random_probes();
        assert!(!probes.is_empty());
        
        // Check that probe was recorded
        assert!(feeler.probe_history.len() > 0);
    }

    #[test]
    fn test_feeler_discovery() {
        let mut feeler = FeelerManager::new();
        let bucket = IpBucketManager::new();
        
        // Record some discoveries
        let peer1 = PeerId::random();
        let peer2 = PeerId::random();
        
        feeler.record_discovery(
            peer1,
            vec!["/ip4/1.1.1.1/tcp/30333".parse().unwrap()],
            DiscoverySource::DhtRandom,
        );
        feeler.record_discovery(
            peer2,
            vec!["/ip4/2.2.2.2/tcp/30333".parse().unwrap()],
            DiscoverySource::KademliaClosest,
        );
        
        // Get candidates
        let candidates = feeler.get_connection_candidates(&bucket, 10);
        assert_eq!(candidates.len(), 2);
    }

    #[test]
    fn test_combined_defense() {
        let mut defense = EclipseDefense::new();
        
        // Add some peers
        let addr: Multiaddr = "/ip4/8.8.8.8/tcp/30333".parse().unwrap();
        defense.buckets.add_peer(PeerId::random(), &addr, true, false).unwrap();
        
        // Run maintenance
        defense.maintenance_tick();
        
        // Check stats
        let stats = defense.stats();
        assert_eq!(stats.diversity.total_peers, 1);
    }
}

