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
    tcp, yamux, Multiaddr, PeerId, Swarm, Transport,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};

/// Network message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkMessage {
    /// New block announcement (for newly mined blocks)
    NewBlock(Block),
    /// Sync block response (for historical block sync)
    /// The unique request_id ensures gossipsub won't reject as duplicate
    /// This is CRITICAL for reliable sync - without it, historical blocks get rejected
    SyncBlock {
        block: Block,
        /// Unique identifier (timestamp_nanos + height) to bypass gossipsub deduplication
        request_id: u64,
    },
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
    /// Request blocks by height range (with unique request_id to bypass dedup)
    GetBlocks { 
        from: u64, 
        to: u64,
        /// Unique request ID (timestamp_nanos) to bypass gossipsub deduplication
        /// CRITICAL: Without this, repeated sync requests get rejected as "Duplicate"
        request_id: u64,
    },
    /// Peer status announcement
    Status {
        best_height: u64,
        best_hash: Hash,
        genesis_hash: Hash,
        /// Timestamp to make each status unique (avoids gossipsub duplicate rejection)
        timestamp: u64,
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
    /// Optional path to persist the keypair (for stable PeerId across restarts)
    pub keypair_path: Option<PathBuf>,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        NetworkConfig {
            listen_addr: "/ip4/0.0.0.0/tcp/30333".to_string(),
            chain_id: "coinject-network-b".to_string(),
            max_peers: 50,
            enable_mdns: true,
            keypair_path: None,
        }
    }
}

/// Load or generate a persistent Ed25519 keypair
fn load_or_generate_keypair(path: Option<&PathBuf>) -> Result<identity::Keypair, Box<dyn std::error::Error>> {
    if let Some(keypair_path) = path {
        // Try to load existing keypair
        if keypair_path.exists() {
            let bytes = std::fs::read(keypair_path)?;
            // Try protobuf encoding first (new format), fall back to raw ed25519 bytes (old format)
            let keypair = identity::Keypair::from_protobuf_encoding(&bytes)
                .or_else(|_| {
                    // Try legacy ed25519 raw bytes format
                    identity::Keypair::ed25519_from_bytes(bytes.clone())
                })
                .map_err(|e| format!("Failed to parse keypair from bytes: {:?}", e))?;
            println!("   ✅ Loaded existing keypair from {:?}", keypair_path);
            return Ok(keypair);
        }
        
        // Generate new keypair and save it
        let keypair = identity::Keypair::generate_ed25519();
        
        // Save the keypair using protobuf encoding (works with all key types)
        let bytes = keypair.to_protobuf_encoding()
            .map_err(|e| format!("Failed to encode keypair: {:?}", e))?;
        if let Some(parent) = keypair_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(keypair_path, bytes)?;
        println!("   ✅ Generated and saved new keypair to {:?}", keypair_path);
        
        Ok(keypair)
    } else {
        // Generate ephemeral keypair
        Ok(identity::Keypair::generate_ed25519())
    }
}

/// Network service managing P2P connections
pub struct NetworkService {
    swarm: Swarm<CoinjectBehaviour>,
    topics: NetworkTopics,
    peers: HashSet<PeerId>,
    mesh_peers: HashSet<PeerId>, // Peers in gossipsub mesh (can receive broadcasts)
    peer_scores: HashMap<PeerId, f64>,
    event_tx: mpsc::UnboundedSender<NetworkEvent>,
    peer_count: Arc<RwLock<usize>>,
    /// Local PeerId for this node
    local_peer_id: PeerId,
    /// Bootnode addresses to reconnect to if disconnected
    bootnode_addrs: Vec<Multiaddr>,
    /// Track which bootnodes are currently connected (by their PeerId if known)
    connected_bootnodes: HashSet<PeerId>,
}

/// Events emitted by the network service
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    /// Peer connected
    PeerConnected(PeerId),
    /// Peer disconnected
    PeerDisconnected(PeerId),
    /// New block received
    BlockReceived { block: Block, peer: PeerId, is_sync_block: bool },
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
        peer_count: Arc<RwLock<usize>>,
    ) -> Result<(Self, mpsc::UnboundedReceiver<NetworkEvent>), Box<dyn std::error::Error>> {
        // Load or generate keypair for this node (persistent if path provided)
        let local_key = load_or_generate_keypair(config.keypair_path.as_ref())?;
        let local_peer_id = PeerId::from(local_key.public());

        println!("🔑 Network node PeerId: {}", local_peer_id);
        println!("   (Use this PeerId in bootnode addresses: /ip4/<IP>/tcp/30333/p2p/{})", local_peer_id);

        // Create gossipsub behaviour
        // Configure for small networks: allow broadcasting with just 1 peer
        // Constraint: mesh_outbound_min <= mesh_n / 2
        // For 2-node networks, mesh_n must be 1 (each node needs only 1 peer)
        // With mesh_n=1, mesh_outbound_min must be <= 0.5, so set to 0
        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(1))
            .validation_mode(ValidationMode::Permissive) // Use Permissive to allow message propagation in small networks
            .mesh_outbound_min(0) // Minimum outbound connections: 0 (required when mesh_n=1)
            .mesh_n_low(1) // Low threshold: 1 peer
            .mesh_n(1) // Target mesh size: 1 peer (for 2-node networks)
            .mesh_n_high(2) // High threshold: 2 peers
            .gossip_lazy(1) // Lazy gossip threshold: 1 peer
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

        // Create transport with TCP keepalive to prevent NAT/firewall disconnects
        // Note: libp2p TCP config doesn't expose keepalive directly, but we configure
        // connection timeouts and retry logic to maintain persistent connections
        let tcp_config = tcp::Config::default()
            .nodelay(true)  // Disable Nagle's algorithm for lower latency
            .port_reuse(true);  // Allow port reuse for faster reconnects
        
        // Configure yamux with larger buffers for unstable connections
        let mut yamux_config = yamux::Config::default();
        yamux_config.set_receive_window_size(16 * 1024 * 1024);  // 16MB window
        yamux_config.set_max_buffer_size(24 * 1024 * 1024);  // 24MB buffer
        
        let transport = tcp::tokio::Transport::new(tcp_config)
            .upgrade(libp2p::core::upgrade::Version::V1)
            .authenticate(noise::Config::new(&local_key)?)
            .multiplex(yamux_config)
            .boxed();

        // Create swarm with longer connection keep-alive to maintain persistent connections
        // Increased idle timeout to 10 minutes to prevent premature disconnects
        let swarm_config = libp2p::swarm::Config::with_tokio_executor()
            .with_idle_connection_timeout(Duration::from_secs(600));  // 10 min idle timeout
        
        let swarm = Swarm::new(
            transport,
            behaviour,
            local_peer_id,
            swarm_config,
        );

        // Create event channel
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        let topics = NetworkTopics::new(&config.chain_id);

        Ok((
            NetworkService {
                swarm,
                topics,
                peers: HashSet::new(),
                mesh_peers: HashSet::new(),
                peer_scores: HashMap::new(),
                event_tx,
                peer_count,
                local_peer_id,
                bootnode_addrs: Vec::new(),
                connected_bootnodes: HashSet::new(),
            },
            event_rx,
        ))
    }

    /// Get the local PeerId for this node
    pub fn local_peer_id(&self) -> PeerId {
        self.local_peer_id
    }

    /// Get the full bootnode multiaddr for this node (IP needs to be provided externally)
    pub fn bootnode_addr(&self, external_ip: &str, port: u16) -> String {
        format!("/ip4/{}/tcp/{}/p2p/{}", external_ip, port, self.local_peer_id)
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
        // Check if we have peers in the mesh before broadcasting
        if self.mesh_peers.is_empty() {
            return Err("InsufficientPeers: No peers in gossipsub mesh".into());
        }

        let message = NetworkMessage::NewBlock(block);
        let data = bincode::serialize(&message)?;
        match self.swarm
            .behaviour_mut()
            .gossipsub
            .publish(self.topics.blocks.clone(), data) {
            Ok(_) => Ok(()),
            Err(gossipsub::PublishError::InsufficientPeers) => {
                Err("InsufficientPeers: Not enough peers in mesh for topic".into())
            }
            Err(e) => Err(format!("Gossipsub publish error: {:?}", e).into()),
        }
    }

    /// INSTITUTIONAL-GRADE: Send sync block with unique request_id
    /// 
    /// This is the CRITICAL fix for historical block sync. The unique request_id
    /// ensures gossipsub treats each sync response as a NEW message, bypassing
    /// the deduplication that was causing sync failures and chain forks.
    /// 
    /// WHY THIS MATTERS:
    /// - Gossipsub caches message IDs for ~2 minutes
    /// - If a block was broadcast before, resending it gets rejected as "Duplicate"
    /// - This prevents nodes from syncing historical blocks
    /// - The request_id (timestamp_nanos + height) makes EVERY message unique
    pub fn send_sync_block(&mut self, block: Block, request_id: u64) -> Result<(), Box<dyn std::error::Error>> {
        if self.mesh_peers.is_empty() {
            return Err("InsufficientPeers: No peers in gossipsub mesh".into());
        }

        // Use SyncBlock with unique request_id - this GUARANTEES no duplicate rejection
        let message = NetworkMessage::SyncBlock { block, request_id };
        let data = bincode::serialize(&message)?;
        match self.swarm
            .behaviour_mut()
            .gossipsub
            .publish(self.topics.blocks.clone(), data) {
            Ok(_) => Ok(()),
            Err(gossipsub::PublishError::InsufficientPeers) => {
                Err("InsufficientPeers: Not enough peers in mesh for topic".into())
            }
            Err(e) => Err(format!("Gossipsub publish error: {:?}", e).into()),
        }
    }

    /// Send a block directly to a specific peer (for sync responses)
    /// Ensures the peer is in the mesh and then publishes the block
    pub fn send_block_to_peer(&mut self, block: Block, peer: PeerId) -> Result<(), Box<dyn std::error::Error>> {
        // Ensure the peer is connected and in the mesh
        if !self.peers.contains(&peer) {
            return Err(format!("Peer {} not connected", peer).into());
        }

        // Explicitly add peer to gossipsub mesh to ensure they receive the message
        self.swarm
            .behaviour_mut()
            .gossipsub
            .add_explicit_peer(&peer);
        self.mesh_peers.insert(peer);

        // Now broadcast the block - it will go to all mesh peers including the target
        let message = NetworkMessage::NewBlock(block);
        let data = bincode::serialize(&message)?;
        match self.swarm
            .behaviour_mut()
            .gossipsub
            .publish(self.topics.blocks.clone(), data) {
            Ok(_) => Ok(()),
            Err(gossipsub::PublishError::InsufficientPeers) => {
                // Even with explicit peer, might need at least one mesh peer
                // Try again with just the explicit peer
                Err("InsufficientPeers: Not enough peers in mesh for topic".into())
            }
            Err(e) => Err(format!("Gossipsub publish error: {:?}", e).into()),
        }
    }

    /// Broadcast a transaction to the network
    pub fn broadcast_transaction(
        &mut self,
        tx: Transaction,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Check if we have peers in the mesh before broadcasting
        if self.mesh_peers.is_empty() {
            return Err("InsufficientPeers: No peers in gossipsub mesh".into());
        }

        let message = NetworkMessage::NewTransaction(tx);
        let data = bincode::serialize(&message)?;
        match self.swarm
            .behaviour_mut()
            .gossipsub
            .publish(self.topics.transactions.clone(), data) {
            Ok(_) => Ok(()),
            Err(gossipsub::PublishError::InsufficientPeers) => {
                Err("InsufficientPeers: Not enough peers in mesh for topic".into())
            }
            Err(e) => Err(format!("Gossipsub publish error: {:?}", e).into()),
        }
    }

    /// Broadcast status to peers
    pub fn broadcast_status(
        &mut self,
        best_height: u64,
        best_hash: Hash,
        genesis_hash: Hash,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Check if we have peers in the mesh before broadcasting
        if self.mesh_peers.is_empty() {
            return Err("InsufficientPeers: No peers in gossipsub mesh".into());
        }

        let message = NetworkMessage::Status {
            best_height,
            best_hash,
            genesis_hash,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        };
        let data = bincode::serialize(&message)?;
        match self.swarm
            .behaviour_mut()
            .gossipsub
            .publish(self.topics.status.clone(), data) {
            Ok(_) => Ok(()),
            Err(gossipsub::PublishError::InsufficientPeers) => {
                Err("InsufficientPeers: Not enough peers in mesh for topic".into())
            }
            Err(e) => Err(format!("Gossipsub publish error: {:?}", e).into()),
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
            let addr: Multiaddr = bootnode.parse()
                .map_err(|e| format!("Failed to parse bootnode address '{}': {:?}", bootnode, e))?;

            // Store for reconnection attempts
            if !self.bootnode_addrs.contains(&addr) {
                self.bootnode_addrs.push(addr.clone());
            }

            // Extract PeerId from the multiaddr if present
            let mut peer_id_opt: Option<PeerId> = None;
            for proto in addr.iter() {
                if let libp2p::multiaddr::Protocol::P2p(peer_id) = proto {
                    peer_id_opt = Some(peer_id);
                    // Add to Kademlia routing table
                    self.swarm.behaviour_mut().kademlia.add_address(&peer_id, addr.clone());
                    break;
                }
            }

            // Dial the bootnode
            match self.swarm.dial(addr.clone()) {
                Ok(()) => {
                    println!("   ✅ Dial initiated to bootnode: {}", bootnode);
                    if peer_id_opt.is_none() {
                        println!("   ⚠️  Warning: Bootnode address missing /p2p/<PeerId> suffix");
                        println!("      For reliable connectivity, use: {}/p2p/<PEER_ID>", bootnode);
                    }
                }
                Err(e) => {
                    eprintln!("   ❌ Failed to dial bootnode '{}': {:?}", bootnode, e);
                    // Continue trying other bootnodes even if one fails
                }
            }
        }
        
        // Bootstrap Kademlia DHT to discover peers
        if let Err(e) = self.swarm.behaviour_mut().kademlia.bootstrap() {
            eprintln!("   ⚠️  Kademlia bootstrap warning: {:?}", e);
        }
        
        Ok(())
    }

    /// Retry connecting to bootnodes that may have disconnected
    pub fn retry_bootnodes(&mut self) {
        if self.bootnode_addrs.is_empty() {
            return;
        }

        // Check how many peers we have
        let peer_count = self.peers.len();
        
        // If we have no peers, try to reconnect to all bootnodes
        if peer_count == 0 {
            println!("📡 No peers connected, retrying {} bootnode(s)...", self.bootnode_addrs.len());
            for addr in self.bootnode_addrs.clone() {
                match self.swarm.dial(addr.clone()) {
                    Ok(()) => {
                        println!("   🔄 Retry dial to: {}", addr);
                    }
                    Err(e) => {
                        // Connection might already be in progress, that's ok
                        if !e.to_string().contains("already pending") {
                            eprintln!("   ❌ Retry dial failed: {:?}", e);
                        }
                    }
                }
            }
            
            // Try bootstrapping Kademlia again
            let _ = self.swarm.behaviour_mut().kademlia.bootstrap();
        }
    }

    /// Broadcast GetBlocks request with unique ID to bypass gossipsub deduplication
    pub fn request_blocks(
        &mut self,
        from: u64,
        to: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Generate unique request_id to bypass gossipsub deduplication
        // This is the FIX for the "Gossip Trap" - each request is now unique!
        let request_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        
        let message = NetworkMessage::GetBlocks { from, to, request_id };
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
                    is_sync_block: false,
                });
            }
            Ok(NetworkMessage::SyncBlock { block, request_id: _ }) => {
                // SyncBlock is used for historical block sync
                // The request_id ensures unique message ID (we don't need it after deserialization)
                // These are explicitly requested blocks, so they should bypass sync threshold checks
                let _ = self.event_tx.send(NetworkEvent::BlockReceived {
                    block,
                    peer,
                    is_sync_block: true,
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
                genesis_hash: _,
                timestamp: _, // Ignored - only used to prevent gossipsub duplicate rejection
            }) => {
                let _ = self.event_tx.send(NetworkEvent::StatusUpdate {
                    peer,
                    best_height,
                    best_hash,
                });
            }
            Ok(NetworkMessage::GetBlocks { from, to, request_id: _ }) => {
                // request_id is only used to bypass gossipsub dedup, not needed after deserialize
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
                    println!("📡 Peer {} subscribed to topic: {:?}", peer_id, topic);
                    // When a peer subscribes to blocks topic, they're in the mesh
                    // Compare topic hashes to check if it's the blocks topic
                    let blocks_topic_hash = self.topics.blocks.hash();
                    if topic == blocks_topic_hash {
                        self.mesh_peers.insert(peer_id);
                    }
                }
                CoinjectBehaviourEvent::Gossipsub(gossipsub::Event::Unsubscribed { peer_id, topic }) => {
                    println!("📡 Peer {} unsubscribed from topic: {:?}", peer_id, topic);
                    // When a peer unsubscribes from blocks topic, remove from mesh
                    let blocks_topic_hash = self.topics.blocks.hash();
                    if topic == blocks_topic_hash {
                        self.mesh_peers.remove(&peer_id);
                    }
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
                        "Identified peer: {} - protocol: {}",
                        peer_id, info.protocol_version
                    );
                    for addr in info.listen_addrs {
                        self.swarm
                            .behaviour_mut()
                            .kademlia
                            .add_address(&peer_id, addr);
                    }
                }
                CoinjectBehaviourEvent::Kademlia(kad::Event::RoutingUpdated {
                    peer,
                    ..
                }) => {
                    println!("Kademlia routing updated for peer: {}", peer);
                }
                _ => {}
            },
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                println!("Connection established with peer: {}", peer_id);
                self.peers.insert(peer_id);
                
                // Track bootnode connections by checking if peer_id matches any bootnode PeerIds
                // (We'll identify bootnodes when they connect via their PeerId)
                // For now, we'll track them in the retry_bootnodes function
                
                // Add peer to gossipsub mesh explicitly to ensure it participates in message propagation
                self.swarm
                    .behaviour_mut()
                    .gossipsub
                    .add_explicit_peer(&peer_id);
                // Note: Peer will be added to mesh_peers when gossipsub mesh is established
                // Update shared peer count
                if let Ok(mut count) = self.peer_count.try_write() {
                    *count = self.peers.len();
                }
                let _ = self.event_tx.send(NetworkEvent::PeerConnected(peer_id));
            }
            SwarmEvent::ConnectionClosed { peer_id, cause, num_established, .. } => {
                // Log detailed close reason for debugging connection stability
                let reason = match &cause {
                    Some(err) => format!("{:?}", err),
                    None => "graceful close".to_string(),
                };
                println!("Connection closed with peer: {} (reason: {}, remaining: {})", 
                    peer_id, reason, num_established);
                
                // Only remove from tracking if ALL connections to this peer are closed
                if num_established == 0 {
                    self.peers.remove(&peer_id);
                    self.mesh_peers.remove(&peer_id);
                    // Remove peer from gossipsub mesh
                    self.swarm
                        .behaviour_mut()
                        .gossipsub
                        .remove_explicit_peer(&peer_id);
                    // Update shared peer count
                    if let Ok(mut count) = self.peer_count.try_write() {
                        *count = self.peers.len();
                    }
                    let _ = self.event_tx.send(NetworkEvent::PeerDisconnected(peer_id));
                }
            }
            SwarmEvent::NewListenAddr { address, .. } => {
                println!("Listening on: {}", address);
            }
            SwarmEvent::OutgoingConnectionError { error, .. } => {
                eprintln!("❌ Outgoing connection error: {:?}", error);
            }
            SwarmEvent::IncomingConnectionError { error, .. } => {
                eprintln!("❌ Incoming connection error: {:?}", error);
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
        let peer_count = Arc::new(RwLock::new(0));
        let result = NetworkService::new(config, peer_count);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_topics_creation() {
        let topics = NetworkTopics::new("test-chain");
        assert_eq!(topics.blocks.hash(), IdentTopic::new("test-chain/blocks").hash());
    }
}
