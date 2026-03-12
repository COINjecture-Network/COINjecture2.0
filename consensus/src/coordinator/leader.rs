// =============================================================================
// Deterministic Leader Election
// =============================================================================
//
// Leader for epoch E = sorted_peers[ H(epoch || prev_hash) % len(peers) ]
//
// The peer set is a BTreeSet<NodeId> which provides deterministic ordering
// (lexicographic on the 32-byte ID). Combined with the hash-based index,
// all honest nodes agree on the same leader for any given epoch.
//
// Failover: if the primary leader stalls, nodes promote the next N candidates
// in the sorted order (wrapping around) up to `failover_depth`.

use std::collections::BTreeSet;

use coinject_core::Hash;

// Re-export NodeId from network crate via the path the coordinator will use
pub type NodeId = [u8; 32];

/// Deterministic leader election for a given epoch.
///
/// Returns `None` if the peer set is empty.
pub fn elect_leader(
    epoch: u64,
    prev_hash: &Hash,
    peers: &BTreeSet<NodeId>,
) -> Option<NodeId> {
    if peers.is_empty() {
        return None;
    }

    let index = leader_index(epoch, prev_hash, peers.len());
    peers.iter().nth(index).copied()
}

/// Return up to `depth` failover candidates starting from the primary leader's
/// position + 1 in the sorted set, wrapping around.
pub fn failover_candidates(
    epoch: u64,
    prev_hash: &Hash,
    peers: &BTreeSet<NodeId>,
    depth: usize,
) -> Vec<NodeId> {
    if peers.is_empty() {
        return Vec::new();
    }

    let primary_idx = leader_index(epoch, prev_hash, peers.len());
    let peer_vec: Vec<NodeId> = peers.iter().copied().collect();
    let n = peer_vec.len();

    (1..=depth.min(n - 1))
        .map(|offset| peer_vec[(primary_idx + offset) % n])
        .collect()
}

/// Check if a given node is the leader for this epoch.
pub fn is_leader(
    node_id: &NodeId,
    epoch: u64,
    prev_hash: &Hash,
    peers: &BTreeSet<NodeId>,
) -> bool {
    elect_leader(epoch, prev_hash, peers)
        .map(|leader| &leader == node_id)
        .unwrap_or(false)
}

/// Compute the deterministic index: H(epoch || prev_hash) % peer_count.
fn leader_index(epoch: u64, prev_hash: &Hash, peer_count: usize) -> usize {
    let mut data = Vec::with_capacity(8 + 32);
    data.extend_from_slice(&epoch.to_le_bytes());
    data.extend_from_slice(prev_hash.as_bytes());
    let hash = Hash::new(&data);

    // Use the first 8 bytes of the hash as a u64 for modular index
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&hash.as_bytes()[..8]);
    let value = u64::from_le_bytes(bytes);

    (value % peer_count as u64) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_peers(count: usize) -> BTreeSet<NodeId> {
        (0..count)
            .map(|i| {
                let mut id = [0u8; 32];
                id[0] = i as u8;
                id
            })
            .collect()
    }

    #[test]
    fn test_deterministic_leader() {
        let peers = make_peers(5);
        let prev_hash = Hash::from_bytes([0xAA; 32]);

        let leader1 = elect_leader(1, &prev_hash, &peers).unwrap();
        let leader2 = elect_leader(1, &prev_hash, &peers).unwrap();
        assert_eq!(leader1, leader2, "same inputs must produce same leader");
    }

    #[test]
    fn test_different_epochs_different_leaders() {
        let peers = make_peers(10);
        let prev_hash = Hash::from_bytes([0xBB; 32]);

        // With 10 peers, different epochs should (usually) pick different leaders
        let leaders: BTreeSet<NodeId> = (0..100)
            .map(|e| elect_leader(e, &prev_hash, &peers).unwrap())
            .collect();

        // Should have elected more than 1 unique leader across 100 epochs
        assert!(leaders.len() > 1, "election should rotate across peers");
    }

    #[test]
    fn test_empty_peers() {
        let peers = BTreeSet::new();
        let prev_hash = Hash::from_bytes([0; 32]);
        assert!(elect_leader(0, &prev_hash, &peers).is_none());
    }

    #[test]
    fn test_single_peer() {
        let peers = make_peers(1);
        let prev_hash = Hash::from_bytes([0; 32]);

        // Single peer is always the leader
        for epoch in 0..10 {
            let leader = elect_leader(epoch, &prev_hash, &peers).unwrap();
            assert_eq!(leader[0], 0);
        }
    }

    #[test]
    fn test_failover_candidates() {
        let peers = make_peers(5);
        let prev_hash = Hash::from_bytes([0xCC; 32]);

        let primary = elect_leader(1, &prev_hash, &peers).unwrap();
        let candidates = failover_candidates(1, &prev_hash, &peers, 2);

        assert_eq!(candidates.len(), 2);
        // Candidates must not include the primary leader
        for c in &candidates {
            assert_ne!(c, &primary, "failover must not include primary");
        }
    }

    #[test]
    fn test_failover_wraps_around() {
        let peers = make_peers(3);
        let prev_hash = Hash::from_bytes([0xDD; 32]);

        // With 3 peers, max failover depth is 2 (excluding self)
        let candidates = failover_candidates(1, &prev_hash, &peers, 5);
        assert_eq!(candidates.len(), 2, "failover depth capped at peer_count - 1");
    }

    #[test]
    fn test_is_leader() {
        let peers = make_peers(5);
        let prev_hash = Hash::from_bytes([0xEE; 32]);

        let leader = elect_leader(1, &prev_hash, &peers).unwrap();
        assert!(is_leader(&leader, 1, &prev_hash, &peers));

        // A non-leader should return false
        let mut non_leader = [0xFFu8; 32];
        non_leader[0] = 99;
        assert!(!is_leader(&non_leader, 1, &prev_hash, &peers));
    }

    #[test]
    fn test_prev_hash_affects_election() {
        let peers = make_peers(10);

        let hash_a = Hash::from_bytes([0x11; 32]);
        let hash_b = Hash::from_bytes([0x22; 32]);

        // Same epoch, different prev_hash → likely different leader
        let leaders: BTreeSet<NodeId> = (0..50)
            .flat_map(|e| {
                vec![
                    elect_leader(e, &hash_a, &peers).unwrap(),
                    elect_leader(e, &hash_b, &peers).unwrap(),
                ]
            })
            .collect();

        assert!(leaders.len() > 1);
    }
}
