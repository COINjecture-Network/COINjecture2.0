// =============================================================================
// Message Router
// =============================================================================
//
// Central routing logic for the mesh layer. Handles:
// - Signature verification on incoming envelopes
// - Routing decisions (broadcast vs direct)
// - Forwarding to gossip engine for flood dissemination
// - Delivering messages to the application layer via mpsc channel
// - Constructing and signing outbound envelopes

use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

use super::gossip::GossipEngine;
use super::identity::{verify_signature, Keypair, NodeId};
use super::protocol::{Envelope, Payload, RoutingMode};

/// Create a unique message ID from sender, nonce, and payload.
///
/// msg_id = SHA-256(sender_id || nonce || serialized_payload)
pub fn compute_msg_id(sender: &NodeId, nonce: &[u8], payload: &Payload) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(sender.as_bytes());
    hasher.update(nonce);
    let payload_bytes = bincode::serialize(payload).expect("payload serialization");
    hasher.update(&payload_bytes);
    let result = hasher.finalize();
    let mut id = [0u8; 32];
    id.copy_from_slice(&result);
    id
}

/// Compute the signable data for an envelope: (msg_id || routing || payload).
pub fn signable_data(msg_id: &[u8; 32], routing: &RoutingMode, payload: &Payload) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(msg_id);
    let routing_bytes = bincode::serialize(routing).expect("routing serialization");
    data.extend_from_slice(&routing_bytes);
    let payload_bytes = bincode::serialize(payload).expect("payload serialization");
    data.extend_from_slice(&payload_bytes);
    data
}

/// Build and sign a new envelope for outbound transmission.
pub fn create_envelope(
    keypair: &Keypair,
    routing: RoutingMode,
    payload: Payload,
    ttl: u8,
) -> Envelope {
    // Generate random nonce
    let mut nonce = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut nonce);

    let sender = *keypair.node_id();
    let msg_id = compute_msg_id(&sender, &nonce, &payload);
    let data = signable_data(&msg_id, &routing, &payload);
    let signature = keypair.sign(&data);

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before epoch")
        .as_millis() as u64;

    Envelope {
        msg_id,
        sender,
        routing,
        payload,
        signature,
        ttl,
        timestamp,
    }
}

/// Result of processing an incoming envelope.
#[derive(Debug)]
pub enum RouteAction {
    /// Deliver to the application layer and forward to peers (broadcast).
    DeliverAndForward,
    /// Deliver to the application layer only (direct message for us).
    DeliverOnly,
    /// Forward to peers only (direct message not for us).
    ForwardOnly,
    /// Drop the message (duplicate, expired TTL, bad signature).
    Drop(String),
}

/// Process an incoming envelope: verify signature, check dedup, check TTL,
/// and decide the routing action.
pub fn process_incoming(
    envelope: &Envelope,
    local_id: &NodeId,
    gossip: &mut GossipEngine,
    peer_public_keys: &std::collections::HashMap<NodeId, Vec<u8>>,
) -> RouteAction {
    // 1. Check dedup
    if !gossip.check_and_mark(&envelope.msg_id) {
        return RouteAction::Drop("duplicate message".into());
    }

    // 2. Check TTL
    if envelope.ttl == 0 {
        return RouteAction::Drop("TTL expired".into());
    }

    // 3. Verify signature
    if let Some(pk) = peer_public_keys.get(&envelope.sender) {
        let data = signable_data(&envelope.msg_id, &envelope.routing, &envelope.payload);
        if let Err(e) = verify_signature(pk, &data, &envelope.signature) {
            return RouteAction::Drop(format!("invalid signature: {}", e));
        }
    } else {
        // We don't have the sender's public key — they might be multiple hops away.
        // For now, accept but log. In production, we'd want a PKI or trust chain.
        tracing::debug!(
            sender = %envelope.sender.short(),
            "accepting message from unknown sender (no cached public key)"
        );
    }

    // 4. Route based on mode
    match &envelope.routing {
        RoutingMode::Broadcast => RouteAction::DeliverAndForward,
        RoutingMode::Direct { target } => {
            if target == local_id {
                RouteAction::DeliverOnly
            } else {
                RouteAction::ForwardOnly
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_test_keypair_and_envelope(ttl: u8) -> (Keypair, Envelope) {
        let kp = Keypair::generate();
        let payload = Payload::Heartbeat {
            epoch: 1,
            peer_count: 3,
            best_block: [0; 32],
        };
        let envelope = create_envelope(&kp, RoutingMode::Broadcast, payload, ttl);
        (kp, envelope)
    }

    #[test]
    fn test_create_and_verify_envelope() {
        let (kp, envelope) = make_test_keypair_and_envelope(10);
        let data = signable_data(&envelope.msg_id, &envelope.routing, &envelope.payload);
        assert!(verify_signature(&kp.public_key_bytes(), &data, &envelope.signature).is_ok());
    }

    #[test]
    fn test_msg_id_is_unique() {
        let kp = Keypair::generate();
        let payload = Payload::Heartbeat {
            epoch: 1,
            peer_count: 1,
            best_block: [0; 32],
        };
        let e1 = create_envelope(&kp, RoutingMode::Broadcast, payload.clone(), 10);
        let e2 = create_envelope(&kp, RoutingMode::Broadcast, payload, 10);
        // Different nonces → different msg_ids
        assert_ne!(e1.msg_id, e2.msg_id);
    }

    #[test]
    fn test_process_incoming_broadcast() {
        let (kp, envelope) = make_test_keypair_and_envelope(10);
        let local_id = NodeId([0xFF; 32]); // Different from sender
        let mut gossip = GossipEngine::new(100, std::time::Duration::from_secs(60));
        let mut keys = HashMap::new();
        keys.insert(*kp.node_id(), kp.public_key_bytes());

        match process_incoming(&envelope, &local_id, &mut gossip, &keys) {
            RouteAction::DeliverAndForward => {} // Expected
            other => panic!("expected DeliverAndForward, got {:?}", other),
        }
    }

    #[test]
    fn test_process_incoming_dedup() {
        let (kp, envelope) = make_test_keypair_and_envelope(10);
        let local_id = NodeId([0xFF; 32]);
        let mut gossip = GossipEngine::new(100, std::time::Duration::from_secs(60));
        let mut keys = HashMap::new();
        keys.insert(*kp.node_id(), kp.public_key_bytes());

        // First time: deliver
        process_incoming(&envelope, &local_id, &mut gossip, &keys);
        // Second time: drop as duplicate
        match process_incoming(&envelope, &local_id, &mut gossip, &keys) {
            RouteAction::Drop(reason) => assert!(reason.contains("duplicate")),
            other => panic!("expected Drop, got {:?}", other),
        }
    }

    #[test]
    fn test_process_incoming_ttl_zero() {
        let (_kp, envelope) = make_test_keypair_and_envelope(0);
        let local_id = NodeId([0xFF; 32]);
        let mut gossip = GossipEngine::new(100, std::time::Duration::from_secs(60));
        let keys = HashMap::new();

        match process_incoming(&envelope, &local_id, &mut gossip, &keys) {
            RouteAction::Drop(reason) => assert!(reason.contains("TTL")),
            other => panic!("expected Drop, got {:?}", other),
        }
    }

    #[test]
    fn test_process_incoming_direct_for_us() {
        let kp = Keypair::generate();
        let local_id = NodeId([0xFF; 32]);
        let payload = Payload::ChainSyncRequest {
            from_block: 0,
            to_block: Some(10),
        };
        let envelope = create_envelope(
            &kp,
            RoutingMode::Direct { target: local_id },
            payload,
            10,
        );

        let mut gossip = GossipEngine::new(100, std::time::Duration::from_secs(60));
        let mut keys = HashMap::new();
        keys.insert(*kp.node_id(), kp.public_key_bytes());

        match process_incoming(&envelope, &local_id, &mut gossip, &keys) {
            RouteAction::DeliverOnly => {} // Expected
            other => panic!("expected DeliverOnly, got {:?}", other),
        }
    }

    #[test]
    fn test_process_incoming_direct_not_for_us() {
        let kp = Keypair::generate();
        let local_id = NodeId([0xFF; 32]);
        let target = NodeId([0xAA; 32]); // Someone else
        let payload = Payload::BountyResult {
            bounty_id: "b1".into(),
            accepted: true,
            reward: 100,
            details: vec![],
        };
        let envelope = create_envelope(&kp, RoutingMode::Direct { target }, payload, 10);

        let mut gossip = GossipEngine::new(100, std::time::Duration::from_secs(60));
        let mut keys = HashMap::new();
        keys.insert(*kp.node_id(), kp.public_key_bytes());

        match process_incoming(&envelope, &local_id, &mut gossip, &keys) {
            RouteAction::ForwardOnly => {} // Expected
            other => panic!("expected ForwardOnly, got {:?}", other),
        }
    }

    #[test]
    fn test_process_incoming_bad_signature() {
        let (kp, mut envelope) = make_test_keypair_and_envelope(10);
        envelope.signature = vec![0u8; 64]; // Garbage signature
        let local_id = NodeId([0xFF; 32]);
        let mut gossip = GossipEngine::new(100, std::time::Duration::from_secs(60));
        let mut keys = HashMap::new();
        keys.insert(*kp.node_id(), kp.public_key_bytes());

        match process_incoming(&envelope, &local_id, &mut gossip, &keys) {
            RouteAction::Drop(reason) => assert!(reason.contains("signature")),
            other => panic!("expected Drop, got {:?}", other),
        }
    }
}
