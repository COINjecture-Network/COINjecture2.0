// =============================================================================
// Gossip Engine — Flood-Based Dissemination
// =============================================================================
//
// Manages epidemic gossip: deduplication via an LRU cache of seen message IDs,
// TTL enforcement, and flood forwarding to all connected peers except the sender.

use lru::LruCache;
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};

use super::identity::NodeId;

/// Tracks seen message IDs to prevent duplicate processing and forwarding.
///
/// Uses an LRU cache with a time-based TTL. Messages are deduplicated by their
/// msg_id (SHA-256 hash). The cache has a fixed capacity; the oldest entries
/// are evicted when full.
pub struct GossipEngine {
    /// LRU cache mapping msg_id → timestamp when first seen.
    seen: LruCache<[u8; 32], Instant>,
    /// How long a message ID stays in the dedup cache.
    ttl: Duration,
}

impl GossipEngine {
    /// Create a new gossip engine with the given dedup cache capacity and TTL.
    pub fn new(capacity: usize, ttl: Duration) -> Self {
        Self {
            seen: LruCache::new(NonZeroUsize::new(capacity).expect("capacity must be > 0")),
            ttl,
        }
    }

    /// Check if a message has been seen before. If not, mark it as seen and
    /// return true (meaning: process and forward it). If already seen, return false.
    pub fn check_and_mark(&mut self, msg_id: &[u8; 32]) -> bool {
        // Prune expired entries on access (lazy cleanup)
        if let Some(timestamp) = self.seen.get(msg_id) {
            if timestamp.elapsed() < self.ttl {
                return false; // Already seen and still valid
            }
            // Expired — treat as unseen, will be overwritten below
        }
        self.seen.put(*msg_id, Instant::now());
        true
    }

    /// Check if a message has been seen (without marking it).
    pub fn has_seen(&mut self, msg_id: &[u8; 32]) -> bool {
        if let Some(timestamp) = self.seen.get(msg_id) {
            if timestamp.elapsed() < self.ttl {
                return true;
            }
        }
        false
    }

    /// Decrement TTL and return the new value. Returns None if TTL reaches 0
    /// (message should be dropped).
    pub fn decrement_ttl(ttl: u8) -> Option<u8> {
        if ttl <= 1 {
            None
        } else {
            Some(ttl - 1)
        }
    }

    /// Determine which peers should receive a forwarded broadcast message.
    /// Returns all connected peer IDs except the sender (flood forwarding).
    pub fn forward_targets(
        connected_peers: &[NodeId],
        sender: &NodeId,
        local_id: &NodeId,
    ) -> Vec<NodeId> {
        connected_peers
            .iter()
            .filter(|id| *id != sender && *id != local_id)
            .copied()
            .collect()
    }

    /// Number of entries currently in the dedup cache.
    pub fn cache_size(&self) -> usize {
        self.seen.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dedup_marks_seen() {
        let mut engine = GossipEngine::new(100, Duration::from_secs(60));
        let msg_id = [0xAA; 32];

        assert!(engine.check_and_mark(&msg_id), "first time should be new");
        assert!(!engine.check_and_mark(&msg_id), "second time should be dup");
    }

    #[test]
    fn test_dedup_different_ids() {
        let mut engine = GossipEngine::new(100, Duration::from_secs(60));
        let id1 = [0x01; 32];
        let id2 = [0x02; 32];

        assert!(engine.check_and_mark(&id1));
        assert!(engine.check_and_mark(&id2));
        assert!(!engine.check_and_mark(&id1));
        assert!(!engine.check_and_mark(&id2));
    }

    #[test]
    fn test_lru_eviction() {
        let mut engine = GossipEngine::new(2, Duration::from_secs(60));
        let id1 = [0x01; 32];
        let id2 = [0x02; 32];
        let id3 = [0x03; 32];

        engine.check_and_mark(&id1);
        engine.check_and_mark(&id2);
        assert_eq!(engine.cache_size(), 2);

        // Adding a third should evict id1 (LRU)
        engine.check_and_mark(&id3);
        assert_eq!(engine.cache_size(), 2);

        // id1 was evicted, so it should appear as "new" again
        assert!(engine.check_and_mark(&id1));
    }

    #[test]
    fn test_ttl_decrement() {
        assert_eq!(GossipEngine::decrement_ttl(10), Some(9));
        assert_eq!(GossipEngine::decrement_ttl(2), Some(1));
        assert_eq!(GossipEngine::decrement_ttl(1), None);
        assert_eq!(GossipEngine::decrement_ttl(0), None);
    }

    #[test]
    fn test_forward_targets_excludes_sender_and_self() {
        let local = NodeId([0x00; 32]);
        let sender = NodeId([0x01; 32]);
        let peer_a = NodeId([0x02; 32]);
        let peer_b = NodeId([0x03; 32]);

        let peers = vec![sender, peer_a, peer_b, local];
        let targets = GossipEngine::forward_targets(&peers, &sender, &local);

        assert_eq!(targets.len(), 2);
        assert!(targets.contains(&peer_a));
        assert!(targets.contains(&peer_b));
        assert!(!targets.contains(&sender));
        assert!(!targets.contains(&local));
    }

    #[test]
    fn test_has_seen_without_marking() {
        let mut engine = GossipEngine::new(100, Duration::from_secs(60));
        let msg_id = [0xBB; 32];

        assert!(!engine.has_seen(&msg_id));
        engine.check_and_mark(&msg_id);
        assert!(engine.has_seen(&msg_id));
    }
}
