//! Peer scoring: ban-score for misbehavior + positive reputation for useful peers.

use crate::peer_store::{PeerStore, StoredPeer};
use std::net::SocketAddr;
use std::sync::Arc;

/// Ban score penalties (threshold = 100 → 24h ban).
pub mod penalties {
    pub const INVALID_BLOCK: i32 = 100;
    pub const INVALID_TRANSACTION: i32 = 50;
    pub const EXCESSIVE_PEX_REQUESTS: i32 = 20;
    pub const PROTOCOL_VIOLATION: i32 = 30;
    pub const EXCESSIVE_MESSAGES: i32 = 10;
    pub const TIMEOUT: i32 = 5;
}

/// Reputation rewards.
pub mod rewards {
    pub const VALID_BLOCK_RELAYED: f64 = 1.0;
    pub const VALID_TX_RELAYED: f64 = 0.1;
    pub const UPTIME_HOUR: f64 = 0.5;
    pub const PEX_USEFUL_PEER: f64 = 0.2;
}

pub struct PeerScorer {
    peer_store: Arc<PeerStore>,
}

impl PeerScorer {
    pub fn new(peer_store: Arc<PeerStore>) -> Self {
        Self { peer_store }
    }

    /// Report bad behavior — returns true if peer was banned.
    pub fn report_bad(&self, addr: &SocketAddr, penalty: i32, reason: &str) -> bool {
        let key = addr.to_string();
        let banned = self.peer_store.apply_ban_score(&key, penalty, reason);
        if banned {
            tracing::warn!(%addr, %reason, "Peer banned for 24 hours");
        } else {
            tracing::debug!(%addr, penalty, %reason, "Peer penalty applied");
        }
        banned
    }

    /// Report good behavior.
    pub fn report_good(&self, addr: &SocketAddr, reward: f64, reason: &str) {
        let key = addr.to_string();
        self.peer_store.apply_reputation(&key, reward);

        // Auto-promote to vetted at reputation >= 5.0
        if let Some(peer) = self.peer_store.get(&key) {
            if peer.reputation_score >= 5.0
                && peer.bucket == crate::peer_store::PeerBucket::Unvetted
            {
                self.peer_store.promote_to_vetted(&key);
                tracing::info!(%addr, "Peer promoted to vetted bucket");
            }
        }
        tracing::trace!(%addr, reward, %reason, "Peer rewarded");
    }

    /// Calculate connection priority (higher = connect first).
    pub fn connection_priority(peer: &StoredPeer) -> f64 {
        let mut score = peer.reputation_score;

        // Boost recently-seen peers
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let staleness_hours = (now.saturating_sub(peer.last_seen)) as f64 / 3600.0;
        score -= staleness_hours * 0.1;

        // Boost peers with high success rate
        if peer.connection_attempts > 0 {
            let rate = peer.successful_connections as f64 / peer.connection_attempts as f64;
            score += rate * 5.0;
        }

        score
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peer_store::{PeerBucket, PeerSource, StoredPeer};

    fn test_store() -> Arc<PeerStore> {
        let store = Arc::new(PeerStore::in_memory());
        let peer = StoredPeer {
            address: "1.2.3.4:707".into(),
            public_key: None,
            peer_id: None,
            last_seen: 100,
            last_connected: None,
            connection_attempts: 10,
            successful_connections: 8,
            bucket: PeerBucket::Unvetted,
            ip_group: "1.2".into(),
            ban_score: 0,
            ban_expires: None,
            reputation_score: 0.0,
            source: PeerSource::PeerExchange,
        };
        store.upsert_peer(peer).unwrap();
        store
    }

    #[test]
    fn test_ban_threshold() {
        let store = test_store();
        let scorer = PeerScorer::new(store);
        let addr: SocketAddr = "1.2.3.4:707".parse().unwrap();

        assert!(!scorer.report_bad(&addr, 50, "test"));
        assert!(scorer.report_bad(&addr, 60, "test")); // cumulative = 110 > 100
    }

    #[test]
    fn test_reputation_accumulation() {
        let store = test_store();
        let scorer = PeerScorer::new(store.clone());
        let addr: SocketAddr = "1.2.3.4:707".parse().unwrap();

        for _ in 0..6 {
            scorer.report_good(&addr, rewards::VALID_BLOCK_RELAYED, "block");
        }

        let peer = store.get("1.2.3.4:707").unwrap();
        assert!(peer.reputation_score >= 5.0);
        assert_eq!(peer.bucket, PeerBucket::Vetted); // auto-promoted
    }

    #[test]
    fn test_connection_priority() {
        let good_peer = StoredPeer {
            address: "1.1.1.1:707".into(),
            public_key: None,
            peer_id: None,
            last_seen: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            last_connected: None,
            connection_attempts: 10,
            successful_connections: 9,
            bucket: PeerBucket::Vetted,
            ip_group: "1.1".into(),
            ban_score: 0,
            ban_expires: None,
            reputation_score: 10.0,
            source: PeerSource::Manual,
        };

        let bad_peer = StoredPeer {
            reputation_score: 0.0,
            successful_connections: 1,
            connection_attempts: 10,
            last_seen: 0, // very stale
            ..good_peer.clone()
        };

        assert!(PeerScorer::connection_priority(&good_peer) > PeerScorer::connection_priority(&bad_peer));
    }
}
