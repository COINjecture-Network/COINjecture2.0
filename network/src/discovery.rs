//! Cascading peer discovery: persistent DB → DNS seeds → hardcoded seeds → manual.

use crate::peer_store::{PeerSource, PeerStore};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

pub struct DiscoveryConfig {
    pub dns_seeds: Vec<String>,
    pub hardcoded_seeds: Vec<SocketAddr>,
    pub manual_peers: Vec<SocketAddr>,
    pub data_dir: PathBuf,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            dns_seeds: vec![
                "seed1.coinjecture.net".into(),
                "seed2.coinjecture.net".into(),
            ],
            hardcoded_seeds: vec![],
            manual_peers: vec![],
            data_dir: PathBuf::from("."),
        }
    }
}

pub struct PeerDiscovery {
    peer_store: Arc<PeerStore>,
    config: DiscoveryConfig,
}

impl PeerDiscovery {
    pub fn new(peer_store: Arc<PeerStore>, config: DiscoveryConfig) -> Self {
        Self { peer_store, config }
    }

    /// Run the full discovery cascade. Returns addresses to try connecting to.
    pub async fn discover(&self) -> Vec<SocketAddr> {
        let mut peers = Vec::new();

        // Layer 1: Persistent peer DB (highest priority — these worked before)
        let stored = self.peer_store.get_peers_to_try(50);
        for p in &stored {
            if let Ok(addr) = p.address.parse() {
                peers.push(addr);
            }
        }
        tracing::info!(count = stored.len(), "Discovery layer 1: persistent store");

        // Layer 2: DNS seed resolution
        if peers.len() < 25 {
            let dns_peers = self.resolve_dns_seeds().await;
            let new_count = dns_peers.len();
            for addr in dns_peers {
                if !peers.contains(&addr) {
                    peers.push(addr);
                    // Add to store
                    let stored = crate::peer_store::new_stored_peer(addr, PeerSource::DnsSeed);
                    let _ = self.peer_store.upsert_peer(stored);
                }
            }
            tracing::info!(count = new_count, "Discovery layer 2: DNS seeds");
        }

        // Layer 3: Hardcoded seeds (fallback)
        if peers.len() < 10 {
            for seed in &self.config.hardcoded_seeds {
                if !peers.contains(seed) {
                    peers.push(*seed);
                    let stored = crate::peer_store::new_stored_peer(*seed, PeerSource::HardcodedSeed);
                    let _ = self.peer_store.upsert_peer(stored);
                }
            }
            tracing::info!(
                count = self.config.hardcoded_seeds.len(),
                "Discovery layer 3: hardcoded seeds"
            );
        }

        // Layer 4: Manual peers (always added, highest priority)
        for manual in &self.config.manual_peers {
            if !peers.contains(manual) {
                peers.insert(0, *manual);
            }
            let stored = crate::peer_store::new_stored_peer(*manual, PeerSource::Manual);
            let _ = self.peer_store.upsert_peer(stored);
        }

        peers.dedup();
        tracing::info!(total = peers.len(), "Discovery complete");
        peers
    }

    /// Resolve DNS seed domains to socket addresses.
    async fn resolve_dns_seeds(&self) -> Vec<SocketAddr> {
        let mut addrs = Vec::new();

        for domain in &self.config.dns_seeds {
            let host = if domain.contains(':') {
                domain.clone()
            } else {
                format!("{domain}:707")
            };

            let resolved = tokio::time::timeout(
                Duration::from_secs(5),
                tokio::net::lookup_host(host),
            )
            .await;

            match resolved {
                Ok(Ok(iter)) => {
                    let v: Vec<SocketAddr> = iter.collect();
                    addrs.extend(v);
                }
                Ok(Err(e)) => {
                    tracing::debug!(domain, error = %e, "DNS seed resolution failed");
                }
                Err(_) => {
                    tracing::debug!(domain, "DNS seed resolution timed out");
                }
            }
        }

        addrs
    }
}
