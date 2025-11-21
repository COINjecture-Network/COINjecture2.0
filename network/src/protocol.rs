// P2P Network Protocol with libp2p gossipsub
use coinject_core::{Block, Transaction, Hash};
use futures::StreamExt;
use libp2p::{
    gossipsub::{self, IdentTopic, MessageAuthenticity, ValidationMode},
    identify, identity,
    kad::{self, store::MemoryStore},
    mdns,
    noise,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux, PeerId, Swarm, Transport,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use tokio::sync::mpsc;

/// Network message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkMessage {
    /// New block announcement
    NewBlock(Block),
    /// New transaction announcement
    NewTransaction(Transaction),
    /// Block header for light clients
    BlockHeader {
        height: u64,
        hash: Hash,
        prev_hash: Hash,
        timestamp: u64,
    },
    /// Request block by hash
    GetBlock(Hash),
    /// Request blocks by height range
    GetBlocks { from: u64, to: u64 },
    /// Peer status announcement
    Status {
        best_height: u64,
        best_hash: Hash,
        genesis_hash: Hash,
    },
}

/// Network topics for gossipsub
pub struct NetworkTopics {
    pub blocks: IdentTopic,
    pub transactions: IdentTopic,
    pub status: IdentTopic,
}

impl NetworkTopics {
    pub fn new(chain_id: &str) -> Self {
        NetworkTopics {
            blocks: IdentTopic::new(format!("{}/blocks", chain_id)),
            transactions: IdentTopic::new(format!("{}/transactions", chain_id)),
            status: IdentTopic::new(format!("{}/status", chain_id)),
        }
    }
}

/// libp2p network behaviour combining gossipsub, mDNS, and Kademlia
#[derive(NetworkBehaviour)]
pub struct CoinjectBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub mdns: mdns::tokio::Behaviour,
    pub kademlia: kad::Behaviour<MemoryStore>,
    pub identify: identify::Behaviour,
}

/// Network protocol configuration
pub struct NetworkConfig {
    pub listen_addr: String,
    pub chain_id: String,
    pub max_peers: usize,
    pub enable_mdns: bool,
    pub genesis_hash: Hash,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        NetworkConfig {
            listen_addr: "/ip4/0.0.0.0/tcp/30333".to_string(),
            chain_id: "coinject-network-b".to_string(),
            max_peers: 50,
            enable_mdns: true,
            genesis_hash: Hash::ZERO,
        }
    }
}

/// Network service managing P2P connections
pub struct NetworkService {
    swarm: Swarm<CoinjectBehaviour>,
    topics: NetworkTopics,
    peers: HashSet<PeerId>,
    peer_scores: HashMap<PeerId, f64>,
    event_tx: mpsc::UnboundedSender<NetworkEvent>,
    genesis_hash: Hash,
}

/// Events emitted by the network service
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    /// Peer connected
    PeerConnected(PeerId),
    /// Peer disconnected
    PeerDisconnected(PeerId),
    /// New block received
    BlockReceived { block: Block, peer: PeerId },
    /// New transaction received
    TransactionReceived { tx: Transaction, peer: PeerId },
    /// Status update from peer
    StatusUpdate {
        peer: PeerId,
        best_height: u64,
        best_hash: Hash,
    },
    /// Blocks requested by peer (for sync)
    BlocksRequested {
        peer: PeerId,
        from_height: u64,
        to_height: u64,
    },
}

impl NetworkService {
    /// Create new network service
    pub fn new(
        config: NetworkConfig,
    ) -> Result<(Self, mpsc::UnboundedReceiver<NetworkEvent>), Box<dyn std::error::Error>> {
        // Generate keypair for this node
        let local_key = identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());

        println!("Network node PeerId: {}", local_peer_id);

        // Create gossipsub behaviour
        // Configure mesh parameters for small networks:
        // Requirements: mesh_outbound_min <= mesh_n_low <= mesh_n <= mesh_n_high
        // Defaults: mesh_n=6, mesh_n_low=4, mesh_n_high=12, mesh_outbound_min=2
        // For 2-peer network, we need: mesh_outbound_min=1, mesh_n_low=2, mesh_n=2, mesh_n_high=4
        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(1))
            .validation_mode(ValidationMode::Strict)
            .mesh_outbound_min(1)  // Minimum outbound peers in mesh
            .mesh_n_low(1)  // Minimum mesh size before trying to add more (FIXED: was 2)
            .mesh_n(2)  // Desired mesh size (for 2-peer network)
            .mesh_n_high(4)  // Maximum mesh size before pruning
            .message_id_fn(|message| {
                // Use message content hash as ID
                let hash = blake3::hash(&message.data);
                gossipsub::MessageId::from(hash.as_bytes().to_vec())
            })
            .build()
            .map_err(|e| format!("Gossipsub config error: {}", e))?;

        let gossipsub = gossipsub::Behaviour::new(
            MessageAuthenticity::Signed(local_key.clone()),
            gossipsub_config,
        )
        .map_err(|e| format!("Gossipsub init error: {}", e))?;

        // Create mDNS for local peer discovery
        let mdns = mdns::tokio::Behaviour::new(
            mdns::Config::default(),
            local_peer_id,
        )?;

        // Create Kademlia DHT for distributed peer discovery
        let store = MemoryStore::new(local_peer_id);
        let kademlia = kad::Behaviour::new(local_peer_id, store);

        // Create identify protocol for peer info exchange
        let identify = identify::Behaviour::new(identify::Config::new(
            "/coinject/1.0.0".to_string(),
            local_key.public(),
        ));

        // Combine behaviours
        let behaviour = CoinjectBehaviour {
            gossipsub,
            mdns,
            kademlia,
            identify,
        };

        // Create transport
        let transport = tcp::tokio::Transport::default()
            .upgrade(libp2p::core::upgrade::Version::V1)
            .authenticate(noise::Config::new(&local_key)?)
            .multiplex(yamux::Config::default())
            .boxed();

        // Create swarm
        let swarm = Swarm::new(
            transport,
            behaviour,
            local_peer_id,
            libp2p::swarm::Config::with_tokio_executor(),
        );

        // Create event channel
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        let topics = NetworkTopics::new(&config.chain_id);

        Ok((
            NetworkService {
                swarm,
                topics,
                peers: HashSet::new(),
                peer_scores: HashMap::new(),
                event_tx,
                genesis_hash: config.genesis_hash,
            },
            event_rx,
        ))
    }

    /// Start listening on configured address
    pub fn start_listening(&mut self, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        println!("   Parsing multiaddr: {}", addr);
        let listen_addr: libp2p::Multiaddr = addr.parse()
            .map_err(|e| format!("Failed to parse multiaddr '{}': {:?}", addr, e))?;
        println!("   Parsed multiaddr: {}", listen_addr);
        self.swarm.listen_on(listen_addr)?;
        Ok(())
    }

    /// Subscribe to network topics
    pub fn subscribe_topics(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.swarm
            .behaviour_mut()
            .gossipsub
            .subscribe(&self.topics.blocks)?;
        self.swarm
            .behaviour_mut()
            .gossipsub
            .subscribe(&self.topics.transactions)?;
        self.swarm
            .behaviour_mut()
            .gossipsub
            .subscribe(&self.topics.status)?;
        Ok(())
    }

    /// Broadcast a block to the network
    pub fn broadcast_block(&mut self, block: Block) -> Result<(), Box<dyn std::error::Error>> {
        let message = NetworkMessage::NewBlock(block);
        let data = bincode::serialize(&message)?;
        self.swarm
            .behaviour_mut()
            .gossipsub
            .publish(self.topics.blocks.clone(), data)?;
        Ok(())
    }

    /// Broadcast a transaction to the network
    pub fn broadcast_transaction(
        &mut self,
        tx: Transaction,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let message = NetworkMessage::NewTransaction(tx);
        let data = bincode::serialize(&message)?;
        self.swarm
            .behaviour_mut()
            .gossipsub
            .publish(self.topics.transactions.clone(), data)?;
        Ok(())
    }

    /// Broadcast status to peers
    pub fn broadcast_status(
        &mut self,
        best_height: u64,
        best_hash: Hash,
        genesis_hash: Hash,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Only broadcast if we have peers
        if self.peers.is_empty() {
            return Err("InsufficientPeers".into());
        }
        
        // Check if gossipsub has peers in the mesh for the status topic
        // Note: We can't directly query mesh peers, but we can try to publish and handle errors
        let message = NetworkMessage::Status {
            best_height,
            best_hash,
            genesis_hash,
        };
        let data = bincode::serialize(&message)?;
        
        // Try to publish - gossipsub will return an error if there are no peers in the mesh
        match self.swarm
            .behaviour_mut()
            .gossipsub
            .publish(self.topics.status.clone(), data) {
            Ok(message_id) => {
                println!("📤 Status broadcast queued, message_id: {:?}", message_id);
                Ok(())
            }
            Err(e) => {
                // Check if it's an "insufficient peers" error
                let error_str = format!("{:?}", e);
                if error_str.contains("insufficient") || error_str.contains("no peers") || error_str.contains("NotEnoughPeers") {
                    Err("InsufficientPeers".into())
                } else {
                    Err(format!("Gossipsub publish error: {:?}", e).into())
                }
            }
        }
    }

    /// Get connected peer count
    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    /// Connect to bootstrap nodes
    pub fn connect_to_bootnodes(&mut self, bootnodes: &[String]) -> Result<(), Box<dyn std::error::Error>> {
        for bootnode in bootnodes {
            println!("   Connecting to bootnode: {}", bootnode);
            let addr: libp2p::Multiaddr = bootnode.parse()
                .map_err(|e| format!("Failed to parse bootnode address '{}': {:?}", bootnode, e))?;

            // Extract peer ID from multiaddr and add to Kademlia before dialing
            if let Some(peer_id) = addr.iter().find_map(|proto| {
                if let libp2p::multiaddr::Protocol::P2p(peer_id) = proto {
                    Some(peer_id)
                } else {
                    None
                }
            }) {
                // Create address without peer ID for Kademlia
                let mut addr_without_p2p = addr.clone();
                addr_without_p2p.pop();
                println!("   Adding bootnode address {} to Kademlia for peer {}", addr_without_p2p, peer_id);
                self.swarm.behaviour_mut().kademlia.add_address(&peer_id, addr_without_p2p);
            }

            // Dial the bootnode
            match self.swarm.dial(addr.clone()) {
                Ok(()) => {
                    println!("   ✅ Dial initiated successfully for bootnode: {}", bootnode);
                }
                Err(e) => {
                    eprintln!("   ❌ Failed to initiate dial to '{}': {:?}", bootnode, e);
                    return Err(format!("Failed to dial bootnode '{}': {:?}", bootnode, e).into());
                }
            }
        }
        Ok(())
    }

    /// Broadcast GetBlocks request
    pub fn request_blocks(
        &mut self,
        from: u64,
        to: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let message = NetworkMessage::GetBlocks { from, to };
        let data = bincode::serialize(&message)?;
        self.swarm
            .behaviour_mut()
            .gossipsub
            .publish(self.topics.blocks.clone(), data)?;
        Ok(())
    }

    /// Handle incoming gossipsub message
    fn handle_gossipsub_message(&mut self, peer: PeerId, message: Vec<u8>) {
        match bincode::deserialize::<NetworkMessage>(&message) {
            Ok(NetworkMessage::NewBlock(block)) => {
                let _ = self.event_tx.send(NetworkEvent::BlockReceived {
                    block,
                    peer,
                });
            }
            Ok(NetworkMessage::NewTransaction(tx)) => {
                let _ = self.event_tx.send(NetworkEvent::TransactionReceived {
                    tx,
                    peer,
                });
            }
            Ok(NetworkMessage::Status {
                best_height,
                best_hash,
                genesis_hash,
            }) => {
                // Verify peer is on the same chain by checking genesis hash
                if genesis_hash == self.genesis_hash {
                    let _ = self.event_tx.send(NetworkEvent::StatusUpdate {
                        peer,
                        best_height,
                        best_hash,
                    });
                } else {
                    println!("⚠️  Rejecting status from peer {:?}: genesis hash mismatch (ours: {:?}, theirs: {:?})", peer, self.genesis_hash, genesis_hash);
                }
            }
            Ok(NetworkMessage::GetBlocks { from, to }) => {
                let _ = self.event_tx.send(NetworkEvent::BlocksRequested {
                    peer,
                    from_height: from,
                    to_height: to,
                });
            }
            Ok(_) => {
                // Other message types handled separately
            }
            Err(e) => {
                eprintln!("Failed to deserialize network message: {}", e);
            }
        }
    }

    /// Process swarm events (call this in a loop)
    pub async fn process_events(&mut self) {
        match self.swarm.select_next_some().await {
            SwarmEvent::Behaviour(event) => match event {
                CoinjectBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                    propagation_source,
                    message,
                    ..
                }) => {
                    self.handle_gossipsub_message(propagation_source, message.data);
                }
                CoinjectBehaviourEvent::Gossipsub(gossipsub::Event::Subscribed { peer_id, topic }) => {
                    println!("📰 Peer {} subscribed to topic: {}", peer_id, topic);
                }
                CoinjectBehaviourEvent::Gossipsub(gossipsub::Event::Unsubscribed { peer_id, topic }) => {
                    println!("📰 Peer {} unsubscribed from topic: {}", peer_id, topic);
                }
                CoinjectBehaviourEvent::Gossipsub(_) => {
                    // Other gossipsub events
                }
                CoinjectBehaviourEvent::Mdns(mdns::Event::Discovered(peers)) => {
                    for (peer, addr) in peers {
                        println!("mDNS discovered peer: {} at {}", peer, addr);
                        self.swarm
                            .behaviour_mut()
                            .gossipsub
                            .add_explicit_peer(&peer);
                        self.swarm
                            .behaviour_mut()
                            .kademlia
                            .add_address(&peer, addr);
                    }
                }
                CoinjectBehaviourEvent::Mdns(mdns::Event::Expired(peers)) => {
                    for (peer, _) in peers {
                        println!("mDNS peer expired: {}", peer);
                        self.swarm
                            .behaviour_mut()
                            .gossipsub
                            .remove_explicit_peer(&peer);
                    }
                }
                CoinjectBehaviourEvent::Identify(identify::Event::Received {
                    peer_id,
                    info,
                    ..
                }) => {
                    println!(
                        "✅ Identified peer: {} - protocol: {}, agent: {}, observed_addr: {:?}",
                        peer_id, info.protocol_version, info.agent_version, info.observed_addr
                    );
                    for addr in info.listen_addrs {
                        println!("   Adding address {} to Kademlia for peer {}", addr, peer_id);
                        self.swarm
                            .behaviour_mut()
                            .kademlia
                            .add_address(&peer_id, addr);
                    }
                }
                CoinjectBehaviourEvent::Identify(identify::Event::Sent { peer_id, .. }) => {
                    println!("📤 Sent identify info to peer: {}", peer_id);
                }
                CoinjectBehaviourEvent::Identify(identify::Event::Error { peer_id, error, connection_id }) => {
                    eprintln!("❌ Identify protocol error with peer {} on connection {:?}: {:?}", peer_id, connection_id, error);
                }
                CoinjectBehaviourEvent::Identify(identify::Event::Pushed { peer_id, connection_id, info }) => {
                    println!("📤 Pushed identify info to peer: {} on connection {:?}, info: {:?}", peer_id, connection_id, info);
                }
                CoinjectBehaviourEvent::Kademlia(kad::Event::RoutingUpdated {
                    peer,
                    ..
                }) => {
                    println!("🌐 Kademlia routing updated for peer: {}", peer);
                }
                CoinjectBehaviourEvent::Kademlia(kad::Event::OutboundQueryProgressed {
                    result,
                    ..
                }) => {
                    match result {
                        kad::QueryResult::Bootstrap(Ok(kad::BootstrapOk { peer, .. })) => {
                            println!("🌐 Kademlia bootstrap successful with peer: {}", peer);
                        }
                        kad::QueryResult::Bootstrap(Err(e)) => {
                            eprintln!("⚠️  Kademlia bootstrap failed: {:?}", e);
                        }
                        kad::QueryResult::GetRecord(Ok(ok)) => {
                            println!("🌐 Kademlia GetRecord successful: {:?}", ok);
                        }
                        kad::QueryResult::GetRecord(Err(e)) => {
                            eprintln!("⚠️  Kademlia GetRecord failed: {:?}", e);
                        }
                        kad::QueryResult::PutRecord(Ok(ok)) => {
                            println!("🌐 Kademlia PutRecord successful: {:?}", ok);
                        }
                        kad::QueryResult::PutRecord(Err(e)) => {
                            eprintln!("⚠️  Kademlia PutRecord failed: {:?}", e);
                        }
                        _ => {}
                    }
                }
                _ => {}
            },
            SwarmEvent::NewListenAddr { address, .. } => {
                println!("📡 Listening on: {}", address);
            }
            SwarmEvent::ListenerClosed { addresses, reason, .. } => {
                eprintln!("⚠️  Listener closed on {:?}: {:?}", addresses, reason);
            }
            SwarmEvent::ListenerError { error, .. } => {
                eprintln!("⚠️  Listener error: {:?}", error);
            }
            SwarmEvent::IncomingConnection { local_addr, send_back_addr, .. } => {
                println!("🔌 Incoming connection from {} to {}", send_back_addr, local_addr);
            }
            SwarmEvent::IncomingConnectionError { error, local_addr, send_back_addr, .. } => {
                eprintln!("❌ Failed to accept incoming connection from {} to {}: {:?}", send_back_addr, local_addr, error);
            }
            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                if let Some(peer) = peer_id {
                    eprintln!("❌ Failed to connect to peer {}: {:?}", peer, error);
                } else {
                    eprintln!("❌ Failed to establish outgoing connection: {:?}", error);
                }
            }
            SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                println!("✅ Connection established with peer: {} via {:?}", peer_id, endpoint);
                println!("   Connection info: {:?}", endpoint);
                self.peers.insert(peer_id);
                println!("   Peer added to peers set (total: {})", self.peers.len());
                
                // Add peer to gossipsub as explicit peer so we can exchange messages
                // Note: This doesn't immediately add them to the mesh - gossipsub will do that
                // during its next heartbeat if both peers are subscribed to the same topics
                self.swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                println!("   Added peer {} as explicit peer to gossipsub (mesh will form on next heartbeat)", peer_id);
                
                // Bootstrap Kademlia to discover more peers (only if we have at least one peer in routing table)
                if self.peers.len() == 1 {
                    // First peer - bootstrap Kademlia
                    println!("   Bootstrapping Kademlia DHT...");
                    match self.swarm.behaviour_mut().kademlia.bootstrap() {
                        Ok(_) => println!("   Kademlia bootstrap initiated"),
                        Err(e) => eprintln!("⚠️  Failed to bootstrap Kademlia: {:?}", e),
                    }
                }
                let _ = self.event_tx.send(NetworkEvent::PeerConnected(peer_id));
            }
            SwarmEvent::ConnectionClosed { peer_id, cause, endpoint, .. } => {
                println!("🔌 Connection closed with peer: {} via {:?}", peer_id, endpoint);
                println!("   Cause: {:?}", cause);
                if let Some(error) = cause.as_ref() {
                    eprintln!("   Error details: {:?}", error);
                }
                self.peers.remove(&peer_id);
                let _ = self.event_tx.send(NetworkEvent::PeerDisconnected(peer_id));
            }
            SwarmEvent::Dialing { peer_id, .. } => {
                if let Some(peer) = peer_id {
                    println!("🔄 Dialing peer: {}", peer);
                } else {
                    println!("🔄 Dialing unknown peer...");
                }
            }
            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                if let Some(peer) = peer_id {
                    eprintln!("❌ Failed to connect to peer {}: {:?}", peer, error);
                } else {
                    eprintln!("❌ Failed to establish outgoing connection: {:?}", error);
                }
            }
            SwarmEvent::IncomingConnectionError { error, .. } => {
                eprintln!("❌ Failed to accept incoming connection: {:?}", error);
            }
            SwarmEvent::Dialing { peer_id, .. } => {
                if let Some(peer) = peer_id {
                    println!("🔄 Dialing peer: {}", peer);
                } else {
                    println!("🔄 Dialing unknown peer...");
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_network_creation() {
        let config = NetworkConfig::default();
        let result = NetworkService::new(config);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_topics_creation() {
        let topics = NetworkTopics::new("test-chain");
        assert_eq!(topics.blocks.hash(), IdentTopic::new("test-chain/blocks").hash());
    }
}
