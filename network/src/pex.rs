//! PEX (Peer Exchange) Reactor — automatic peer discovery and sharing.
//!
//! Peers periodically request known peers from their connections and
//! share their vetted peers with those who ask. Rate-limited to prevent abuse.

use crate::peer_store::{PeerBucket, PeerSource, PeerStore, StoredPeer};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

// ── Config ──────────────────────────────────────────────────────────────────

pub struct PexConfig {
    pub request_interval: Duration,
    pub max_peers_per_response: usize,
    pub min_interval_between_requests: Duration,
    pub target_outbound: usize,
    pub max_total: usize,
}

impl Default for PexConfig {
    fn default() -> Self {
        Self {
            request_interval: Duration::from_secs(30),
            max_peers_per_response: 25,
            min_interval_between_requests: Duration::from_secs(30),
            target_outbound: 25,
            max_total: 100,
        }
    }
}

// ── PEX peer info (wire format) ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PexPeerInfo {
    pub address: String,
    pub public_key: Option<[u8; 32]>,
    pub last_seen: u64,
}

// ── PEX Reactor ─────────────────────────────────────────────────────────────

pub struct PexReactor {
    peer_store: Arc<PeerStore>,
    last_request: DashMap<String, Instant>,
    config: PexConfig,
}

impl PexReactor {
    pub fn new(peer_store: Arc<PeerStore>, config: PexConfig) -> Self {
        Self {
            peer_store,
            last_request: DashMap::new(),
            config,
        }
    }

    /// Handle an incoming PEX request. Returns peers to share, or None if rate-limited.
    pub fn handle_pex_request(&self, from: &SocketAddr) -> Option<Vec<PexPeerInfo>> {
        let key = from.to_string();

        // Rate limit
        if let Some(last) = self.last_request.get(&key) {
            if last.elapsed() < self.config.min_interval_between_requests {
                tracing::debug!(%from, "PEX request rate-limited");
                return None;
            }
        }

        self.last_request.insert(key, Instant::now());

        // Share vetted peers, excluding the requester
        let peers: Vec<PexPeerInfo> = self
            .peer_store
            .get_vetted_peers(self.config.max_peers_per_response * 2)
            .into_iter()
            .filter(|p| p.address != from.to_string())
            .take(self.config.max_peers_per_response)
            .map(|p| PexPeerInfo {
                address: p.address,
                public_key: p.public_key,
                last_seen: p.last_seen,
            })
            .collect();

        if peers.is_empty() {
            None
        } else {
            Some(peers)
        }
    }

    /// Handle an incoming PEX response (peers shared by another node).
    pub fn handle_pex_response(&self, from: &SocketAddr, peers: Vec<PexPeerInfo>) {
        let mut added = 0;

        for peer_info in peers {
            // Skip self
            if let Ok(addr) = peer_info.address.parse::<SocketAddr>() {
                if addr == *from {
                    continue;
                }
            }

            // Skip banned
            if self.peer_store.is_banned(&peer_info.address) {
                continue;
            }

            // Skip already known and recently seen
            if let Some(existing) = self.peer_store.get(&peer_info.address) {
                if existing.last_seen > peer_info.last_seen {
                    continue;
                }
            }

            let stored = StoredPeer {
                address: peer_info.address.clone(),
                public_key: peer_info.public_key,
                peer_id: None,
                last_seen: peer_info.last_seen,
                last_connected: None,
                connection_attempts: 0,
                successful_connections: 0,
                bucket: PeerBucket::Unvetted,
                ip_group: crate::peer_store::ip_group_from_addr(&peer_info.address),
                ban_score: 0,
                ban_expires: None,
                reputation_score: 0.0,
                source: PeerSource::PeerExchange,
            };

            if self.peer_store.upsert_peer(stored).is_ok() {
                added += 1;
            }
        }

        if added > 0 {
            tracing::info!(%from, added, "PEX: discovered new peers");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_reactor() -> PexReactor {
        let store = Arc::new(PeerStore::in_memory());
        PexReactor::new(store, PexConfig::default())
    }

    #[test]
    fn test_rate_limiting() {
        let reactor = test_reactor();
        let addr: SocketAddr = "1.2.3.4:707".parse().unwrap();

        // First request: allowed (returns empty since no vetted peers)
        let _ = reactor.handle_pex_request(&addr);

        // Immediate second request: rate-limited
        let result = reactor.handle_pex_request(&addr);
        assert!(result.is_none());
    }

    #[test]
    fn test_response_filtering() {
        let store = Arc::new(PeerStore::in_memory());
        let reactor = PexReactor::new(store.clone(), PexConfig::default());

        // Add a vetted peer
        let peer = StoredPeer {
            address: "10.0.0.1:707".into(),
            public_key: None,
            peer_id: None,
            last_seen: 100,
            last_connected: None,
            connection_attempts: 0,
            successful_connections: 0,
            bucket: PeerBucket::Vetted,
            ip_group: "10.0".into(),
            ban_score: 0,
            ban_expires: None,
            reputation_score: 5.0,
            source: PeerSource::Manual,
        };
        store.upsert_peer(peer).unwrap();

        let addr: SocketAddr = "5.5.5.5:707".parse().unwrap();
        let result = reactor.handle_pex_request(&addr);
        assert!(result.is_some());
        let peers = result.unwrap();
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].address, "10.0.0.1:707");
    }

    #[test]
    fn test_max_peers_limit() {
        let store = Arc::new(PeerStore::in_memory());
        let config = PexConfig {
            max_peers_per_response: 3,
            ..Default::default()
        };
        let reactor = PexReactor::new(store.clone(), config);

        for i in 0..10 {
            let peer = StoredPeer {
                address: format!("{}.0.0.1:707", i + 20),
                public_key: None,
                peer_id: None,
                last_seen: 100,
                last_connected: None,
                connection_attempts: 0,
                successful_connections: 0,
                bucket: PeerBucket::Vetted,
                ip_group: format!("{}.0", i + 20),
                ban_score: 0,
                ban_expires: None,
                reputation_score: 5.0,
                source: PeerSource::Manual,
            };
            store.upsert_peer(peer).unwrap();
        }

        let addr: SocketAddr = "5.5.5.5:707".parse().unwrap();
        let result = reactor.handle_pex_request(&addr).unwrap();
        assert!(result.len() <= 3);
    }
}
