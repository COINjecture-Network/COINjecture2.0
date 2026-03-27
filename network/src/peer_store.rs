//! Persistent peer database with vetted/unvetted buckets, ban scoring,
//! and /16 Sybil resistance. Backed by a JSON file for simplicity.

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

// ── Constants ───────────────────────────────────────────────────────────────

const MAX_VETTED: usize = 64;
const MAX_UNVETTED: usize = 256;
const MAX_PER_SUBNET: usize = 8;
const BAN_THRESHOLD: i32 = 100;
const BAN_DURATION_SECS: u64 = 86400; // 24 hours

// ── Types ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredPeer {
    pub address: String, // "ip:port"
    pub public_key: Option<[u8; 32]>,
    pub peer_id: Option<String>,
    pub last_seen: u64,
    pub last_connected: Option<u64>,
    pub connection_attempts: u32,
    pub successful_connections: u32,
    pub bucket: PeerBucket,
    pub ip_group: String, // /16 subnet
    pub ban_score: i32,
    pub ban_expires: Option<u64>,
    pub reputation_score: f64,
    pub source: PeerSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeerBucket {
    Vetted,
    Unvetted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeerSource {
    Manual,
    PeerExchange,
    DnsSeed,
    HardcodedSeed,
    Inbound,
}

#[derive(Debug)]
pub enum PeerStoreError {
    Io(std::io::Error),
    Json(serde_json::Error),
    StoreFull,
    SubnetFull,
}

impl std::fmt::Display for PeerStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "IO: {e}"),
            Self::Json(e) => write!(f, "JSON: {e}"),
            Self::StoreFull => write!(f, "Peer store is at capacity"),
            Self::SubnetFull => write!(f, "Subnet limit reached"),
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn ip_group(addr: &str) -> String {
    ip_group_from_addr(addr)
}

/// Extract /16 subnet from an address string: "1.2.3.4:707" → "1.2"
pub fn ip_group_from_addr(addr: &str) -> String {
    addr.split(':')
        .next()
        .unwrap_or("")
        .split('.')
        .take(2)
        .collect::<Vec<_>>()
        .join(".")
}

// ── PeerStore ───────────────────────────────────────────────────────────────

pub struct PeerStore {
    peers: DashMap<String, StoredPeer>,
    path: PathBuf,
}

impl PeerStore {
    /// Open or create the peer store at `{data_dir}/peers.json`.
    pub fn open(data_dir: &Path) -> Result<Self, PeerStoreError> {
        let path = data_dir.join("peers.json");
        let peers = DashMap::new();

        if path.exists() {
            let data = std::fs::read_to_string(&path).map_err(PeerStoreError::Io)?;
            let map: std::collections::HashMap<String, StoredPeer> =
                serde_json::from_str(&data).unwrap_or_default();
            for (k, v) in map {
                peers.insert(k, v);
            }
        }

        Ok(Self { peers, path })
    }

    /// Create an in-memory peer store (for tests).
    pub fn in_memory() -> Self {
        Self {
            peers: DashMap::new(),
            path: PathBuf::from("/dev/null"),
        }
    }

    /// Flush to disk.
    fn flush(&self) {
        let map: std::collections::HashMap<String, StoredPeer> = self
            .peers
            .iter()
            .map(|r| (r.key().clone(), r.value().clone()))
            .collect();

        if let Ok(json) = serde_json::to_string_pretty(&map) {
            let _ = std::fs::write(&self.path, json);
        }
    }

    /// Insert or update a peer.
    pub fn upsert_peer(&self, peer: StoredPeer) -> Result<(), PeerStoreError> {
        let key = peer.address.clone();

        // Subnet limit check for new unvetted peers
        if !self.peers.contains_key(&key) && peer.bucket == PeerBucket::Unvetted {
            let subnet = &peer.ip_group;
            let subnet_count = self
                .peers
                .iter()
                .filter(|r| &r.value().ip_group == subnet)
                .count();
            if subnet_count >= MAX_PER_SUBNET {
                return Err(PeerStoreError::SubnetFull);
            }
        }

        self.peers.insert(key, peer);
        self.enforce_limits();
        self.flush();
        Ok(())
    }

    /// Get peers to connect to, ordered by priority.
    pub fn get_peers_to_try(&self, count: usize) -> Vec<StoredPeer> {
        let now = now_secs();
        let mut peers: Vec<StoredPeer> = self
            .peers
            .iter()
            .filter(|r| !self.is_banned_inner(r.value(), now))
            .map(|r| r.value().clone())
            .collect();

        // Sort: vetted first, then by last_seen descending
        peers.sort_by(|a, b| {
            a.bucket
                .cmp_vetted_first(&b.bucket)
                .then(b.last_seen.cmp(&a.last_seen))
        });

        peers.truncate(count);
        peers
    }

    /// Get all vetted peers (for PEX responses).
    pub fn get_vetted_peers(&self, max: usize) -> Vec<StoredPeer> {
        let now = now_secs();
        let mut peers: Vec<StoredPeer> = self
            .peers
            .iter()
            .filter(|r| r.value().bucket == PeerBucket::Vetted && !self.is_banned_inner(r.value(), now))
            .map(|r| r.value().clone())
            .collect();
        peers.truncate(max);
        peers
    }

    /// Mark a peer as successfully connected.
    pub fn mark_connected(&self, addr: &str) {
        if let Some(mut peer) = self.peers.get_mut(addr) {
            let now = now_secs();
            peer.last_connected = Some(now);
            peer.last_seen = now;
            peer.successful_connections += 1;
        }
        self.flush();
    }

    /// Promote a peer from unvetted to vetted.
    pub fn promote_to_vetted(&self, addr: &str) {
        if let Some(mut peer) = self.peers.get_mut(addr) {
            peer.bucket = PeerBucket::Vetted;
        }
        self.flush();
    }

    /// Apply a ban score delta. Returns true if the peer is now banned.
    pub fn apply_ban_score(&self, addr: &str, delta: i32, _reason: &str) -> bool {
        let banned = if let Some(mut peer) = self.peers.get_mut(addr) {
            peer.ban_score += delta;
            if peer.ban_score >= BAN_THRESHOLD {
                peer.ban_expires = Some(now_secs() + BAN_DURATION_SECS);
                true
            } else {
                false
            }
        } else {
            false
        };
        self.flush();
        banned
    }

    /// Apply positive reputation.
    pub fn apply_reputation(&self, addr: &str, delta: f64) {
        if let Some(mut peer) = self.peers.get_mut(addr) {
            peer.reputation_score += delta;
        }
    }

    /// Check if a peer is currently banned.
    pub fn is_banned(&self, addr: &str) -> bool {
        let now = now_secs();
        self.peers
            .get(addr)
            .map(|r| self.is_banned_inner(r.value(), now))
            .unwrap_or(false)
    }

    fn is_banned_inner(&self, peer: &StoredPeer, now: u64) -> bool {
        match peer.ban_expires {
            Some(expires) => now < expires,
            None => false,
        }
    }

    /// Remove expired bans, return count removed.
    pub fn cleanup_expired_bans(&self) -> usize {
        let now = now_secs();
        let mut count = 0;
        for mut entry in self.peers.iter_mut() {
            if let Some(expires) = entry.value().ban_expires {
                if now >= expires {
                    entry.value_mut().ban_expires = None;
                    entry.value_mut().ban_score = 0;
                    count += 1;
                }
            }
        }
        if count > 0 {
            self.flush();
        }
        count
    }

    /// (vetted_count, unvetted_count)
    pub fn peer_counts(&self) -> (usize, usize) {
        let vetted = self
            .peers
            .iter()
            .filter(|r| r.value().bucket == PeerBucket::Vetted)
            .count();
        let unvetted = self.peers.len() - vetted;
        (vetted, unvetted)
    }

    /// Total peer count.
    pub fn len(&self) -> usize {
        self.peers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.peers.is_empty()
    }

    /// Get a specific peer.
    pub fn get(&self, addr: &str) -> Option<StoredPeer> {
        self.peers.get(addr).map(|r| r.value().clone())
    }

    /// Remove a peer.
    pub fn remove(&self, addr: &str) {
        self.peers.remove(addr);
        self.flush();
    }

    /// Evict least-recently-seen peers to stay within bucket limits.
    fn enforce_limits(&self) {
        self.evict_bucket(PeerBucket::Vetted, MAX_VETTED);
        self.evict_bucket(PeerBucket::Unvetted, MAX_UNVETTED);
    }

    fn evict_bucket(&self, bucket: PeerBucket, max: usize) {
        let mut in_bucket: Vec<(String, u64, bool)> = self
            .peers
            .iter()
            .filter(|r| r.value().bucket == bucket)
            .map(|r| {
                (
                    r.key().clone(),
                    r.value().last_seen,
                    r.value().source == PeerSource::Manual,
                )
            })
            .collect();

        if in_bucket.len() <= max {
            return;
        }

        // Never evict manual peers; sort by last_seen ascending (oldest first)
        in_bucket.sort_by(|a, b| a.1.cmp(&b.1));

        let to_remove = in_bucket.len() - max;
        let mut removed = 0;
        for (addr, _, is_manual) in &in_bucket {
            if removed >= to_remove {
                break;
            }
            if !is_manual {
                self.peers.remove(addr);
                removed += 1;
            }
        }
    }
}

// Helper for bucket sorting
impl PeerBucket {
    fn cmp_vetted_first(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (PeerBucket::Vetted, PeerBucket::Unvetted) => std::cmp::Ordering::Less,
            (PeerBucket::Unvetted, PeerBucket::Vetted) => std::cmp::Ordering::Greater,
            _ => std::cmp::Ordering::Equal,
        }
    }
}

/// Helper to create a StoredPeer from a SocketAddr.
pub fn new_stored_peer(addr: SocketAddr, source: PeerSource) -> StoredPeer {
    let addr_str = addr.to_string();
    StoredPeer {
        ip_group: ip_group(&addr_str),
        address: addr_str,
        public_key: None,
        peer_id: None,
        last_seen: now_secs(),
        last_connected: None,
        connection_attempts: 0,
        successful_connections: 0,
        bucket: PeerBucket::Unvetted,
        ban_score: 0,
        ban_expires: None,
        reputation_score: 0.0,
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_peer(addr: &str, bucket: PeerBucket) -> StoredPeer {
        StoredPeer {
            address: addr.to_string(),
            public_key: None,
            peer_id: None,
            last_seen: now_secs(),
            last_connected: None,
            connection_attempts: 0,
            successful_connections: 0,
            bucket,
            ip_group: ip_group(addr),
            ban_score: 0,
            ban_expires: None,
            reputation_score: 0.0,
            source: PeerSource::PeerExchange,
        }
    }

    #[test]
    fn test_insert_and_retrieve() {
        let store = PeerStore::in_memory();
        let peer = test_peer("1.2.3.4:707", PeerBucket::Unvetted);
        store.upsert_peer(peer).unwrap();
        assert_eq!(store.len(), 1);
        assert!(store.get("1.2.3.4:707").is_some());
    }

    #[test]
    fn test_bucket_limits() {
        let store = PeerStore::in_memory();
        for i in 0..300 {
            let addr = format!("{}.{}.0.1:707", i / 256, i % 256);
            let peer = test_peer(&addr, PeerBucket::Unvetted);
            let _ = store.upsert_peer(peer);
        }
        let (_, unvetted) = store.peer_counts();
        assert!(unvetted <= MAX_UNVETTED);
    }

    #[test]
    fn test_subnet_sybil_resistance() {
        let store = PeerStore::in_memory();
        // Same /16 subnet
        for i in 0..20 {
            let addr = format!("10.0.{}.1:707", i);
            let peer = test_peer(&addr, PeerBucket::Unvetted);
            let result = store.upsert_peer(peer);
            if i >= MAX_PER_SUBNET {
                assert!(result.is_err());
            }
        }
    }

    #[test]
    fn test_ban_score_threshold() {
        let store = PeerStore::in_memory();
        let peer = test_peer("5.5.5.5:707", PeerBucket::Unvetted);
        store.upsert_peer(peer).unwrap();

        let banned = store.apply_ban_score("5.5.5.5:707", 50, "test");
        assert!(!banned);
        assert!(!store.is_banned("5.5.5.5:707"));

        let banned = store.apply_ban_score("5.5.5.5:707", 60, "test");
        assert!(banned);
        assert!(store.is_banned("5.5.5.5:707"));
    }

    #[test]
    fn test_promotion() {
        let store = PeerStore::in_memory();
        let peer = test_peer("9.9.9.9:707", PeerBucket::Unvetted);
        store.upsert_peer(peer).unwrap();
        store.promote_to_vetted("9.9.9.9:707");
        let p = store.get("9.9.9.9:707").unwrap();
        assert_eq!(p.bucket, PeerBucket::Vetted);
    }
}
