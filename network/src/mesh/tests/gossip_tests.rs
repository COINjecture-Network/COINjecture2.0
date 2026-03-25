// =============================================================================
// Gossip Engine Tests
// =============================================================================

use crate::mesh::gossip::GossipEngine;
use crate::mesh::identity::{Keypair, NodeId};
use crate::mesh::protocol::{Payload, RoutingMode};
use crate::mesh::router;
use std::time::Duration;

#[test]
fn test_flood_propagation_excludes_sender_and_self() {
    let local = NodeId([0x00; 32]);
    let sender = NodeId([0x01; 32]);
    let peer_a = NodeId([0x02; 32]);
    let peer_b = NodeId([0x03; 32]);
    let peer_c = NodeId([0x04; 32]);

    let connected = vec![sender, peer_a, peer_b, peer_c];
    let targets = GossipEngine::forward_targets(&connected, &sender, &local);

    assert_eq!(targets.len(), 3);
    assert!(targets.contains(&peer_a));
    assert!(targets.contains(&peer_b));
    assert!(targets.contains(&peer_c));
    assert!(!targets.contains(&sender));
    assert!(!targets.contains(&local));
}

#[test]
fn test_dedup_prevents_reprocessing() {
    let mut engine = GossipEngine::new(1000, Duration::from_secs(300));
    let kp = Keypair::generate();

    let payload = Payload::ConsensusSalt {
        epoch: 1,
        salt: [0xAB; 32],
    };
    let envelope = router::create_envelope(&kp, RoutingMode::Broadcast, payload, 10);

    // First check should succeed (new message)
    assert!(engine.check_and_mark(&envelope.msg_id));
    // Second check should fail (duplicate)
    assert!(!engine.check_and_mark(&envelope.msg_id));
}

#[test]
fn test_ttl_enforcement_drops_at_zero() {
    assert!(GossipEngine::decrement_ttl(10).is_some());
    assert!(GossipEngine::decrement_ttl(1).is_none());
    assert!(GossipEngine::decrement_ttl(0).is_none());
}

#[test]
fn test_ttl_decrements_correctly() {
    assert_eq!(GossipEngine::decrement_ttl(5), Some(4));
    assert_eq!(GossipEngine::decrement_ttl(2), Some(1));
}

#[test]
fn test_dedup_cache_respects_capacity() {
    let mut engine = GossipEngine::new(3, Duration::from_secs(300));

    // Fill cache
    let ids: Vec<[u8; 32]> = (0..5).map(|i| [i as u8; 32]).collect();

    for id in &ids[..3] {
        assert!(engine.check_and_mark(id));
    }
    assert_eq!(engine.cache_size(), 3);

    // Adding more should evict oldest
    assert!(engine.check_and_mark(&ids[3]));
    assert_eq!(engine.cache_size(), 3);

    // ids[0] should have been evicted
    assert!(engine.check_and_mark(&ids[0])); // "new" again
}

#[test]
fn test_100k_message_dedup_performance() {
    let mut engine = GossipEngine::new(100_000, Duration::from_secs(300));

    // Insert 100k unique messages
    for i in 0u32..100_000 {
        let mut id = [0u8; 32];
        id[..4].copy_from_slice(&i.to_be_bytes());
        assert!(engine.check_and_mark(&id));
    }

    assert_eq!(engine.cache_size(), 100_000);

    // Verify dedup works for all of them
    for i in 0u32..100_000 {
        let mut id = [0u8; 32];
        id[..4].copy_from_slice(&i.to_be_bytes());
        assert!(
            !engine.check_and_mark(&id),
            "message {} should be deduped",
            i
        );
    }
}
