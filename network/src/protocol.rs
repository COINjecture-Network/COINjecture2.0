// P2P Network Protocol with libp2p gossipsub and request-response
use crate::addr_filter::{AddressFilterConfig, validate_multiaddr, filter_multiaddrs_with_logging};
use coinject_core::{Block, Transaction, Hash};
use futures::StreamExt;
use libp2p::{
    autonat,
    gossipsub::{self, IdentTopic, MessageAuthenticity, ValidationMode},
    identify, identity,
    kad::{self, store::MemoryStore},
    mdns,
    noise,
    relay,
    swarm::{NetworkBehaviour, SwarmEvent, dial_opts::{DialOpts, PeerCondition}},
    tcp, yamux, Multiaddr, PeerId, Swarm,
};
use libp2p_request_response::{self as request_response, ProtocolSupport, ResponseChannel, OutboundRequestId};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};

// ============================================================================
// REQUEST-RESPONSE SYNC PROTOCOL
// Reliable, ordered block delivery - bypasses GossipSub deduplication issues
// ============================================================================

/// Request types for the sync protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncRequest {
    /// Request blocks by height range
    BlockRequest {
        from_height: u64,
        to_height: u64,
        request_id: u64,
    },
}

/// Response types for the sync protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncResponse {
    /// Block response with ordered blocks
    BlockResponse {
        blocks: Vec<Block>,
        request_id: u64,
    },
    /// Error response
    Error {
        message: String,
        request_id: u64,
    },
}

/// Codec for serializing/deserializing sync messages
#[derive(Debug, Clone, Default)]
pub struct SyncCodec;

impl request_response::Codec for SyncCodec {
    type Protocol = &'static str;
    type Request = SyncRequest;
    type Response = SyncResponse;

    fn read_request<'life0, 'life1, 'life2, 'async_trait, T>(
        &'life0 mut self,
        _protocol: &'life1 Self::Protocol,
        io: &'life2 mut T,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = std::io::Result<Self::Request>> + Send + 'async_trait>>
    where
        T: futures::AsyncRead + Unpin + Send + 'async_trait,
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
            use futures::AsyncReadExt;
            let mut buf = Vec::new();
            io.read_to_end(&mut buf).await?;
            bincode::deserialize(&buf)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
        })
    }

    fn read_response<'life0, 'life1, 'life2, 'async_trait, T>(
        &'life0 mut self,
        _protocol: &'life1 Self::Protocol,
        io: &'life2 mut T,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = std::io::Result<Self::Response>> + Send + 'async_trait>>
    where
        T: futures::AsyncRead + Unpin + Send + 'async_trait,
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
            use futures::AsyncReadExt;
            let mut buf = Vec::new();
            io.read_to_end(&mut buf).await?;
            bincode::deserialize(&buf)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
        })
    }

    fn write_request<'life0, 'life1, 'life2, 'async_trait, T>(
        &'life0 mut self,
        _protocol: &'life1 Self::Protocol,
        io: &'life2 mut T,
        req: Self::Request,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = std::io::Result<()>> + Send + 'async_trait>>
    where
        T: futures::AsyncWrite + Unpin + Send + 'async_trait,
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
            use futures::AsyncWriteExt;
            let data = bincode::serialize(&req)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            io.write_all(&data).await?;
            io.close().await?;
            Ok(())
        })
    }

    fn write_response<'life0, 'life1, 'life2, 'async_trait, T>(
        &'life0 mut self,
        _protocol: &'life1 Self::Protocol,
        io: &'life2 mut T,
        resp: Self::Response,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = std::io::Result<()>> + Send + 'async_trait>>
    where
        T: futures::AsyncWrite + Unpin + Send + 'async_trait,
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
            use futures::AsyncWriteExt;
            let data = bincode::serialize(&resp)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            io.write_all(&data).await?;
            io.close().await?;
            Ok(())
        })
    }
}

/// Node type for network messages (simplified for serialization)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum NetworkNodeType {
    Light = 0,
    Full = 1,
    Archive = 2,
    Validator = 3,
    Bounty = 4,
    Oracle = 5,
}

impl Default for NetworkNodeType {
    fn default() -> Self {
        NetworkNodeType::Full
    }
}

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
    /// Peer status announcement (with node type for capability routing)
    Status {
        best_height: u64,
        best_hash: Hash,
        genesis_hash: Hash,
        /// Node type for capability-based routing
        node_type: NetworkNodeType,
        /// Timestamp to make each status unique (avoids gossipsub duplicate rejection)
        timestamp: u64,
    },
}

/// Light sync protocol messages (for Light clients)
/// These are on a dedicated topic for efficient routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LightSyncNetworkMessage {
    /// Request headers from a height range
    GetHeaders {
        start_height: u64,
        max_headers: u32,
        request_id: u64,
    },
    /// Response with block headers
    Headers {
        headers: Vec<coinject_core::BlockHeader>,
        request_id: u64,
    },
    /// Request FlyClient proof for super-light verification
    GetFlyClientProof {
        security_param: u32,
        request_id: u64,
    },
    /// FlyClient proof response (MMR-based)
    FlyClientProof {
        proof_data: Vec<u8>, // Serialized FlyClientProof
        request_id: u64,
    },
    /// Request MMR inclusion proof for a specific block
    GetMMRProof {
        block_height: u64,
        request_id: u64,
    },
    /// MMR inclusion proof response
    MMRProof {
        header: coinject_core::BlockHeader,  // The block header being proven
        proof_data: Vec<u8>,                 // Serialized MMRInclusionProof
        mmr_root: Hash,                      // Current MMR root for verification
        request_id: u64,
    },
    /// Request transaction inclusion proof (SPV)
    GetTxProof {
        tx_hash: Hash,
        block_height: u64,
        request_id: u64,
    },
    /// Transaction inclusion proof (Merkle path)
    TxProof {
        tx_hash: Hash,
        merkle_path: Vec<Hash>,
        block_height: u64,
        request_id: u64,
    },
    /// Request current chain tip (lightweight status)
    GetChainTip {
        request_id: u64,
    },
    /// Chain tip response
    ChainTip {
        height: u64,
        hash: Hash,
        mmr_root: Hash,
        total_work: u128,
        request_id: u64,
    },
}

/// Network topics for gossipsub
pub struct NetworkTopics {
    pub blocks: IdentTopic,
    pub transactions: IdentTopic,
    pub status: IdentTopic,
    /// Light sync topic for header-only sync protocol
    pub light_sync: IdentTopic,
}

impl NetworkTopics {
    pub fn new(chain_id: &str) -> Self {
        NetworkTopics {
            blocks: IdentTopic::new(format!("{}/blocks", chain_id)),
            transactions: IdentTopic::new(format!("{}/transactions", chain_id)),
            status: IdentTopic::new(format!("{}/status", chain_id)),
            light_sync: IdentTopic::new(format!("{}/light-sync", chain_id)),
        }
    }
}

/// libp2p network behaviour combining gossipsub, mDNS, Kademlia, autonat, relay, and sync
#[derive(NetworkBehaviour)]
pub struct CoinjectBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub mdns: mdns::tokio::Behaviour,
    pub kademlia: kad::Behaviour<MemoryStore>,
    pub identify: identify::Behaviour,
    pub autonat: autonat::Behaviour,
    pub relay: relay::Behaviour,
    /// Request-response protocol for reliable block sync
    pub sync: request_response::Behaviour<SyncCodec>,
}

/// Network protocol configuration
pub struct NetworkConfig {
    pub listen_addr: String,
    pub chain_id: String,
    pub max_peers: usize,
    pub enable_mdns: bool,
    /// Optional path to persist the keypair (for stable PeerId across restarts)
    pub keypair_path: Option<PathBuf>,
    /// Optional external address to advertise (e.g., /ip4/<PUBLIC_IP>/tcp/30333)
    /// Use this when running behind NAT/Docker to ensure peers dial the correct address
    pub external_addr: Option<String>,
    /// Address filter configuration for cloud vs local deployment
    pub addr_filter_config: Option<AddressFilterConfig>,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        NetworkConfig {
            listen_addr: "/ip4/0.0.0.0/tcp/30333".to_string(),
            chain_id: "coinject-network-b".to_string(),
            max_peers: 50,
            enable_mdns: true,
            keypair_path: None,
            external_addr: None,
            addr_filter_config: Some(AddressFilterConfig::default()),
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
    /// Relay addresses for each peer (for fallback when direct connection fails)
    peer_relay_addresses: HashMap<PeerId, Multiaddr>,
    /// Track incoming connection attempts to prevent simultaneous outbound dials
    /// Maps connection_id to the source address
    incoming_connection_attempts: HashMap<libp2p::swarm::ConnectionId, Multiaddr>,
    /// Blacklisted peer IDs that should be rejected/ignored
    blacklisted_peers: HashSet<PeerId>,
    /// Address filter configuration
    addr_filter_config: AddressFilterConfig,
    /// External address we advertise (if set)
    external_addr: Option<Multiaddr>,
    /// Track pending outbound dials to prevent simultaneous dial collisions
    pending_dials: HashSet<PeerId>,
    /// Pending request-response channels for sync requests (keyed by request_id)
    pending_sync_channels: HashMap<u64, ResponseChannel<SyncResponse>>,
    /// Map outbound request IDs to peer IDs for tracking responses
    outbound_sync_requests: HashMap<OutboundRequestId, (PeerId, u64)>, // (peer, our_request_id)
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
    /// Status update from peer (includes node type for capability routing)
    StatusUpdate {
        peer: PeerId,
        best_height: u64,
        best_hash: Hash,
        genesis_hash: Hash, // CRITICAL: Genesis hash for chain validation during "handshake"
        node_type: NetworkNodeType,
    },
    /// Blocks requested by peer (for sync)
    /// If rr_request_id is Some, this is a request-response request and needs a response via SendBlocksResponse
    BlocksRequested {
        peer: PeerId,
        from_height: u64,
        to_height: u64,
        /// Request ID for request-response protocol (None for GossipSub requests)
        rr_request_id: Option<u64>,
    },
    // === LIGHT SYNC EVENTS ===
    /// Headers requested by light client
    HeadersRequested {
        peer: PeerId,
        start_height: u64,
        max_headers: u32,
        request_id: u64,
    },
    /// Headers received (for light clients)
    HeadersReceived {
        peer: PeerId,
        headers: Vec<coinject_core::BlockHeader>,
        request_id: u64,
    },
    /// FlyClient proof requested
    FlyClientProofRequested {
        peer: PeerId,
        security_param: u32,
        request_id: u64,
    },
    /// FlyClient proof received
    FlyClientProofReceived {
        peer: PeerId,
        proof_data: Vec<u8>,
        request_id: u64,
    },
    /// MMR proof requested
    MMRProofRequested {
        peer: PeerId,
        block_height: u64,
        request_id: u64,
    },
    /// MMR proof received
    MMRProofReceived {
        peer: PeerId,
        header: coinject_core::BlockHeader,
        proof_data: Vec<u8>,
        mmr_root: Hash,
        request_id: u64,
    },
    /// Transaction proof requested (SPV)
    TxProofRequested {
        peer: PeerId,
        tx_hash: Hash,
        block_height: u64,
        request_id: u64,
    },
    /// Transaction proof received (SPV)
    TxProofReceived {
        peer: PeerId,
        tx_hash: Hash,
        merkle_path: Vec<Hash>,
        block_height: u64,
        request_id: u64,
    },
    /// Chain tip requested
    ChainTipRequested {
        peer: PeerId,
        request_id: u64,
    },
    /// Chain tip received
    ChainTipReceived {
        peer: PeerId,
        height: u64,
        hash: Hash,
        mmr_root: Hash,
        total_work: u128,
        request_id: u64,
    },
}

/// Check if a multiaddr contains a private/internal IP address
/// Used to filter out Docker bridge IPs that cause connection timeouts
fn is_private_address(addr: &Multiaddr) -> bool {
    let s = addr.to_string();
    s.contains("/ip4/10.") ||      // Docker internal networks
    s.contains("/ip4/172.1") ||    // Docker bridge (172.16-172.31)
    s.contains("/ip4/172.2") ||    // Docker bridge
    s.contains("/ip4/172.3") ||    // Docker bridge  
    s.contains("/ip4/192.168.") || // Private networks
    s.contains("/ip4/127.") ||     // Loopback
    s.contains("/ip4/169.254.") || // Link-local auto-configuration (RFC 3927) - fixes ghost IPs
    s.contains("/ip6/::1") ||      // IPv6 loopback
    s.contains("/ip6/fe80")        // IPv6 link-local
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
        // FIXED: mesh_n=1 violates gossipsub constraints (mesh_outbound_min <= mesh_n/2)
        // Gossipsub spec requires: mesh_n_out < mesh_n_low <= mesh_n
        // For small networks, mesh_n=2 is the minimum safe value
        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(1))
            .validation_mode(ValidationMode::Permissive) // Allow propagation in small networks
            .mesh_n(2)              // Target 2 peers in mesh (minimum safe value)
            .mesh_n_low(1)          // Seek more peers when below 1
            .mesh_n_high(3)         // Prune when above 3 peers  
            .mesh_outbound_min(1)   // Require at least 1 outbound (must be <= mesh_n/2 = 1)
            .gossip_lazy(2)         // Lazy push to 2 peers not in mesh
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
        // NOTE: We filter Docker internal IPs (172.x, 10.x) in the event loop instead of
        // using with_hide_listen_addrs (not available in libp2p 0.54)
        let identify = identify::Behaviour::new(identify::Config::new(
            "/coinject/1.0.0".to_string(),
            local_key.public(),
        ));

        // Create autonat for NAT detection and hole punching
        // This enables bidirectional connectivity by detecting NAT type and attempting hole punching
        let autonat = autonat::Behaviour::new(
            local_peer_id,
            autonat::Config {
                // Use longer timeout for slow networks
                timeout: Duration::from_secs(10),
                // Retry interval for NAT detection
                retry_interval: Duration::from_secs(30),
                // Only use servers we trust (for now, allow any)
                ..Default::default()
            },
        );

        // Create relay for nodes behind restrictive NATs
        // This allows nodes to connect through relay nodes when direct connection fails
        let relay = relay::Behaviour::new(local_peer_id, relay::Config::default());

        // Create request-response protocol for reliable, ordered block sync
        // This bypasses GossipSub deduplication issues that cause unreliable delivery
        let sync_protocol = "/coinject/sync/1.0.0";
        let sync = request_response::Behaviour::new(
            [(sync_protocol, ProtocolSupport::Full)],
            request_response::Config::default()
                .with_request_timeout(Duration::from_secs(120)), // 2 minute timeout for large responses
        );

        // Combine behaviours
        let behaviour = CoinjectBehaviour {
            gossipsub,
            mdns,
            kademlia,
            identify,
            autonat,
            relay,
            sync,
        };

        // Configure TCP with nodelay to prevent Nagle's algorithm buffering small handshake packets
        // This is CRITICAL for Linux where Nagle can cause silent Noise handshake failures
        let tcp_config = tcp::Config::default()
            .nodelay(true)  // CRITICAL: Disable Nagle's algorithm for Noise handshake
            .listen_backlog(2048);  // Larger backlog for connection queue
        
        // Configure Noise with longer timeout for slow networks
        // Default timeout is 10s, increase to 30s for high-latency connections
        // CRITICAL FIX: Use default config to avoid handshake issues
        let noise_config = noise::Config::new;
        
        // FIXED: Use default Yamux config to avoid flow control issues with mixed-version peers
        // Custom window/buffer sizes (like 1MB/2MB) can cause silent disconnects due to
        // window mismatches between yamux v2.1.0+ (InitialStreamWindowSize=16KB) and
        // older nodes (MaxStreamWindowSize=16MB). Issue #1257 documents RST frames
        // being sent instead of backpressure when buffers fill.
        // 
        // Use SwarmBuilder for proper Tokio executor integration
        // This is ESSENTIAL for Linux - manual transport construction can cause
        // connection tasks to not be properly driven by the executor
        let swarm = libp2p::SwarmBuilder::with_existing_identity(local_key)
            .with_tokio()
            .with_tcp(
                tcp_config,
                noise_config,
                yamux::Config::default,  // FIXED: Use defaults for peer compatibility
            )?
            .with_behaviour(|_keypair| Ok(behaviour))?
            .with_swarm_config(|cfg| {
                // FIXED: 60 seconds is sufficient (24h was excessive and wastes resources)
                // Gossipsub heartbeats (1s) keep active connections alive
                // 60s allows cleanup of truly dead connections
                // Note: libp2p 0.54 doesn't expose dial timeout directly, but the transport
                // layer handles timeouts. The Noise handshake timeout is controlled by
                // the transport implementation.
                cfg.with_idle_connection_timeout(Duration::from_secs(60))
            })
            .build();

        // Create event channel
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        let topics = NetworkTopics::new(&config.chain_id);

        // Parse and set external address if provided (CRITICAL for NAT/Docker)
        let mut swarm = swarm;
        let external_addr = if let Some(ref ext_addr) = config.external_addr {
            match ext_addr.parse::<Multiaddr>() {
                Ok(addr) => {
                    println!("[ADDR] Setting external address: {}", addr);
                    swarm.add_external_address(addr.clone());
                    Some(addr)
                }
                Err(e) => {
                    eprintln!("[ADDR] Failed to parse external address '{}': {}", ext_addr, e);
                    None
                }
            }
        } else {
            None
        };

        let addr_filter_config = config.addr_filter_config.clone().unwrap_or_default();

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
                peer_relay_addresses: HashMap::new(),
                incoming_connection_attempts: HashMap::new(),
                blacklisted_peers: {
                    // Blacklist the GCE VM peer that's interfering with connections
                    let mut blacklist = HashSet::new();
                    if let Ok(blacklisted_peer) = "12D3KooWFL8uuMmeoWyU46SdX8g2aJEk4Fv5qAr4dZXmZfsGiefa".parse::<PeerId>() {
                        blacklist.insert(blacklisted_peer);
                        println!("🚫 Blacklisted interfering peer: {}", blacklisted_peer);
                    }
                    blacklist
                },
                addr_filter_config,
                external_addr,
                pending_dials: HashSet::new(),
                pending_sync_channels: HashMap::new(),
                outbound_sync_requests: HashMap::new(),
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
        self.swarm
            .behaviour_mut()
            .gossipsub
            .subscribe(&self.topics.light_sync)?;
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

    /// Broadcast status to peers (includes node type for capability routing)
    pub fn broadcast_status(
        &mut self,
        best_height: u64,
        best_hash: Hash,
        genesis_hash: Hash,
        node_type: NetworkNodeType,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Check if we have peers in the mesh before broadcasting
        if self.mesh_peers.is_empty() {
            return Err("InsufficientPeers: No peers in gossipsub mesh".into());
        }

        let message = NetworkMessage::Status {
            best_height,
            best_hash,
            genesis_hash,
            node_type,
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
    
    // =========================================================================
    // LIGHT SYNC PROTOCOL METHODS
    // =========================================================================
    
    /// Request headers from the network (for Light clients)
    pub fn request_headers(
        &mut self,
        start_height: u64,
        max_headers: u32,
    ) -> Result<u64, Box<dyn std::error::Error>> {
        let request_id = Self::generate_request_id();
        let message = LightSyncNetworkMessage::GetHeaders {
            start_height,
            max_headers,
            request_id,
        };
        self.publish_light_sync_message(message)?;
        Ok(request_id)
    }
    
    /// Send headers response (for Full/Archive nodes serving Light clients)
    pub fn send_headers(
        &mut self,
        headers: Vec<coinject_core::BlockHeader>,
        request_id: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let message = LightSyncNetworkMessage::Headers { headers, request_id };
        self.publish_light_sync_message(message)
    }
    
    /// Request FlyClient proof (for super-light verification)
    pub fn request_flyclient_proof(
        &mut self,
        security_param: u32,
    ) -> Result<u64, Box<dyn std::error::Error>> {
        let request_id = Self::generate_request_id();
        let message = LightSyncNetworkMessage::GetFlyClientProof {
            security_param,
            request_id,
        };
        self.publish_light_sync_message(message)?;
        Ok(request_id)
    }
    
    /// Send FlyClient proof response
    pub fn send_flyclient_proof(
        &mut self,
        proof_data: Vec<u8>,
        request_id: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let message = LightSyncNetworkMessage::FlyClientProof { proof_data, request_id };
        self.publish_light_sync_message(message)
    }
    
    /// Request MMR inclusion proof for a block
    pub fn request_mmr_proof(
        &mut self,
        block_height: u64,
    ) -> Result<u64, Box<dyn std::error::Error>> {
        let request_id = Self::generate_request_id();
        let message = LightSyncNetworkMessage::GetMMRProof {
            block_height,
            request_id,
        };
        self.publish_light_sync_message(message)?;
        Ok(request_id)
    }
    
    /// Send MMR proof response
    pub fn send_mmr_proof(
        &mut self,
        header: coinject_core::BlockHeader,
        proof_data: Vec<u8>,
        mmr_root: Hash,
        request_id: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let message = LightSyncNetworkMessage::MMRProof {
            header,
            proof_data,
            mmr_root,
            request_id,
        };
        self.publish_light_sync_message(message)
    }
    
    /// Request transaction proof (SPV)
    pub fn request_tx_proof(
        &mut self,
        tx_hash: Hash,
        block_height: u64,
    ) -> Result<u64, Box<dyn std::error::Error>> {
        let request_id = Self::generate_request_id();
        let message = LightSyncNetworkMessage::GetTxProof {
            tx_hash,
            block_height,
            request_id,
        };
        self.publish_light_sync_message(message)?;
        Ok(request_id)
    }
    
    /// Send transaction proof response
    pub fn send_tx_proof(
        &mut self,
        tx_hash: Hash,
        merkle_path: Vec<Hash>,
        block_height: u64,
        request_id: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let message = LightSyncNetworkMessage::TxProof {
            tx_hash,
            merkle_path,
            block_height,
            request_id,
        };
        self.publish_light_sync_message(message)
    }
    
    /// Request chain tip (lightweight)
    pub fn request_chain_tip(&mut self) -> Result<u64, Box<dyn std::error::Error>> {
        let request_id = Self::generate_request_id();
        let message = LightSyncNetworkMessage::GetChainTip { request_id };
        self.publish_light_sync_message(message)?;
        Ok(request_id)
    }
    
    /// Send chain tip response
    pub fn send_chain_tip(
        &mut self,
        height: u64,
        hash: Hash,
        mmr_root: Hash,
        total_work: u128,
        request_id: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let message = LightSyncNetworkMessage::ChainTip {
            height,
            hash,
            mmr_root,
            total_work,
            request_id,
        };
        self.publish_light_sync_message(message)
    }
    
    /// Generate unique request ID for deduplication bypass
    fn generate_request_id() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
    }
    
    /// Publish a light sync message to the dedicated topic
    fn publish_light_sync_message(
        &mut self,
        message: LightSyncNetworkMessage,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.mesh_peers.is_empty() {
            return Err("InsufficientPeers: No peers in gossipsub mesh".into());
        }
        
        let data = bincode::serialize(&message)?;
        match self.swarm
            .behaviour_mut()
            .gossipsub
            .publish(self.topics.light_sync.clone(), data) {
            Ok(_) => Ok(()),
            Err(gossipsub::PublishError::InsufficientPeers) => {
                Err("InsufficientPeers: Not enough peers in mesh for light-sync topic".into())
            }
            Err(e) => Err(format!("Gossipsub publish error: {:?}", e).into()),
        }
    }

    /// Get connected peer count
    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }
    
    /// Get mesh peer count (peers that can receive gossipsub broadcasts)
    pub fn mesh_peer_count(&self) -> usize {
        self.mesh_peers.len()
    }

    /// Disconnect a peer (e.g., for genesis hash mismatch)
    pub fn disconnect_peer(&mut self, peer: PeerId) {
        println!("🔌 Disconnecting peer: {:?}", peer);
        self.swarm.disconnect_peer_id(peer);
        self.peers.remove(&peer);
        self.mesh_peers.remove(&peer);
        if let Ok(mut count) = self.peer_count.try_write() {
            *count = self.peers.len();
        }
    }
    
    /// Log comprehensive network health status (call periodically for monitoring)
    pub fn log_network_health(&self) {
        let total_peers = self.peers.len();
        let mesh_peers = self.mesh_peers.len();
        let bootnode_count = self.bootnode_addrs.len();
        
        println!("═══════════════════════════════════════════════════════════");
        println!("📊 NETWORK HEALTH STATUS");
        println!("───────────────────────────────────────────────────────────");
        println!("   Total connected peers: {}", total_peers);
        println!("   Gossipsub mesh peers:  {} (can receive broadcasts)", mesh_peers);
        println!("   Configured bootnodes:  {}", bootnode_count);
        println!("   Local PeerId:          {}", self.local_peer_id);
        
        // Health assessment
        let health = if mesh_peers == 0 {
            "🔴 CRITICAL - No mesh peers, cannot propagate blocks/txs"
        } else if mesh_peers == 1 {
            "🟡 WARNING - Only 1 mesh peer, single point of failure"
        } else if total_peers < 3 {
            "🟡 WARNING - Low peer count, network may be unstable"
        } else {
            "🟢 HEALTHY - Sufficient peers for reliable propagation"
        };
        println!("   Health status:         {}", health);
        
        // List connected peers
        if !self.peers.is_empty() {
            println!("───────────────────────────────────────────────────────────");
            println!("   Connected peers:");
            for peer in &self.peers {
                let in_mesh = if self.mesh_peers.contains(peer) { "✅ mesh" } else { "⏳ pending" };
                println!("     {} [{}]", peer, in_mesh);
            }
        }
        println!("═══════════════════════════════════════════════════════════");
    }
    
    /// Check if network is healthy enough for block propagation
    pub fn is_healthy_for_broadcast(&self) -> bool {
        self.mesh_peers.len() >= 1
    }

    /// Connect to bootstrap nodes
    /// Connect to bootstrap nodes with address validation and dial collision prevention
    pub fn connect_to_bootnodes(&mut self, bootnodes: &[String]) -> Result<(), Box<dyn std::error::Error>> {
        for bootnode in bootnodes {
            println!("[BOOT] Connecting to bootnode: {}", bootnode);
            let addr: Multiaddr = bootnode.parse()
                .map_err(|e| format!("Failed to parse bootnode address '{}': {:?}", bootnode, e))?;

            // Validate the bootnode address
            let validation = validate_multiaddr(&addr, &self.addr_filter_config);
            if !validation.is_accepted() {
                println!("[BOOT] Bootnode address rejected: {} (reason: {})", addr, validation.reason());
                println!("[BOOT]   Hint: Bootnodes should use public IPs with port 30333");
                continue;
            }

            // Store for reconnection attempts
            if !self.bootnode_addrs.contains(&addr) {
                self.bootnode_addrs.push(addr.clone());
            }

            // Extract PeerId from the multiaddr if present
            let mut peer_id_opt: Option<PeerId> = None;
            for proto in addr.iter() {
                if let libp2p::multiaddr::Protocol::P2p(peer_id) = proto {
                    peer_id_opt = Some(peer_id);
                    
                    // Check if we're already connected or have pending dial
                    if self.peers.contains(&peer_id) {
                        println!("[BOOT] Already connected to bootnode: {}", peer_id);
                        continue;
                    }
                    if self.pending_dials.contains(&peer_id) {
                        println!("[BOOT] Already dialing bootnode: {}", peer_id);
                        continue;
                    }
                    
                    // Add to Kademlia routing table
                    self.swarm.behaviour_mut().kademlia.add_address(&peer_id, addr.clone());
                    break;
                }
            }

            // Use DialOpts with PeerCondition to prevent simultaneous dial collisions
            let dial_result = if let Some(peer_id) = peer_id_opt {
                // Dial with peer condition: only dial if not already connected/dialing
                let opts = DialOpts::peer_id(peer_id)
                    .addresses(vec![addr.clone()])
                    .condition(PeerCondition::Disconnected)
                    .build();
                self.swarm.dial(opts)
            } else {
                println!("[BOOT] Warning: Bootnode address missing /p2p/<PeerId> suffix");
                println!("[BOOT]   For reliable connectivity, use: {}/p2p/<PEER_ID>", bootnode);
                self.swarm.dial(addr.clone())
            };

            match dial_result {
                Ok(()) => {
                    println!("[BOOT] Dial initiated to bootnode: {}", bootnode);
                }
                Err(e) => {
                    let error_str = format!("{:?}", e);
                    if error_str.contains("AlreadyDialing") || error_str.contains("AlreadyConnected") || error_str.contains("already pending") {
                        println!("[BOOT] Already dialing/connected to bootnode: {}", bootnode);
                    } else {
                        eprintln!("[BOOT] Failed to dial bootnode '{}': {:?}", bootnode, e);
                    }
                }
            }
        }
        
        // Bootstrap Kademlia DHT to discover peers
        if let Err(e) = self.swarm.behaviour_mut().kademlia.bootstrap() {
            eprintln!("[BOOT] Kademlia bootstrap warning: {:?}", e);
        }
        
        Ok(())
    }

    /// Check if canonical bootnode (Node 1) is connected
    pub fn is_bootnode_connected(&self) -> bool {
        if self.bootnode_addrs.is_empty() {
            return false;
        }
        
        // Check if any bootnode peer ID is in our connected peers
        // CRITICAL: Check both our internal peers set AND the swarm's connected peers
        // This fixes the issue where inbound connections aren't being tracked
        for addr in &self.bootnode_addrs {
            if let Some(bootnode_peer_id) = addr.iter().find_map(|p| {
                if let libp2p::multiaddr::Protocol::P2p(peer_id) = p {
                    Some(peer_id)
                } else {
                    None
                }
            }) {
                // Check our internal tracking
                if self.peers.contains(&bootnode_peer_id) {
                    return true;
                }
                // CRITICAL: Also check swarm's connected peers (for inbound connections)
                // This ensures we recognize bootnode even if it connected inbound before we tracked it
                if self.swarm.is_connected(&bootnode_peer_id) {
                    return true;
                }
            }
        }
        false
    }

    /// Sync connected peers from swarm to internal tracking
    /// This fixes the issue where connections exist in swarm but aren't tracked
    fn sync_connected_peers(&mut self) {
        // Get all connected peers from swarm
        let connected_peers: Vec<PeerId> = self.swarm.connected_peers().cloned().collect();
        
        for peer_id in connected_peers {
            // If swarm says connected but we haven't tracked it, track it NOW
            if !self.peers.contains(&peer_id) {
                self.peers.insert(peer_id);
                self.mesh_peers.insert(peer_id);
                self.swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                if let Ok(mut count) = self.peer_count.try_write() {
                    *count = self.peers.len();
                }
                let _ = self.event_tx.send(NetworkEvent::PeerConnected(peer_id));
            }
        }
    }

    /// Retry connecting to bootnodes that may have disconnected
    pub fn retry_bootnodes(&mut self) {
        // CRITICAL: Sync connected peers FIRST before checking bootnode status
        // This ensures we catch any connections that were established but not tracked
        self.sync_connected_peers();
        if self.bootnode_addrs.is_empty() {
            return;
        }

        // CRITICAL FIX: Only retry if bootnode is not connected AND we have no peers
        // This prevents infinite retry loops when connection is established but not tracked
        // IMPORTANT: If bootnode connects inbound, we should recognize it and stop dialing
        let bootnode_connected = self.is_bootnode_connected();
        let has_peers = !self.peers.is_empty();
        
        // If bootnode is connected (inbound or outbound), don't retry
        if bootnode_connected {
            return;
        }
        
        // CRITICAL: Check if we have incoming connection attempts from bootnodes
        // If so, don't dial outbound - wait for the incoming connection to complete
        let has_incoming_bootnode_attempt = self.bootnode_addrs.iter().any(|bootnode_addr| {
            let bootnode_ip = bootnode_addr.iter()
                .find_map(|p| {
                    if let libp2p::multiaddr::Protocol::Ip4(ip) = p {
                        Some(ip)
                    } else {
                        None
                    }
                });
            
            self.incoming_connection_attempts.values().any(|inc_addr| {
                let inc_ip = inc_addr.iter()
                    .find_map(|p| {
                        if let libp2p::multiaddr::Protocol::Ip4(ip) = p {
                            Some(ip)
                        } else {
                            None
                        }
                    });
                
                bootnode_ip.is_some() && inc_ip.is_some() && bootnode_ip == inc_ip
            })
        });
        
        if has_incoming_bootnode_attempt {
            println!("📡 Incoming connection attempt from bootnode detected - waiting for handshake to complete (skipping outbound dial)");
            return;
        }
        
        // Only retry if we have no peers at all
        if !has_peers {
            println!("📡 Bootnode not connected and no peers, retrying {} bootnode(s)...", self.bootnode_addrs.len());
            
            // Try direct connection first, then relay if available
            for addr in self.bootnode_addrs.clone() {
                // Extract target peer ID from bootnode address
                let mut target_peer_id: Option<PeerId> = None;
                for proto in addr.iter() {
                    if let libp2p::multiaddr::Protocol::P2p(peer_id) = proto {
                        target_peer_id = Some(peer_id);
                        break;
                    }
                }
                
                // CRITICAL: Check if already connected or pending before dialing
                if let Some(bootnode_peer_id) = target_peer_id {
                    if self.swarm.is_connected(&bootnode_peer_id) || self.peers.contains(&bootnode_peer_id) {
                        // Already connected - sync tracking and skip dial
                        if !self.peers.contains(&bootnode_peer_id) {
                            self.peers.insert(bootnode_peer_id);
                            self.mesh_peers.insert(bootnode_peer_id);
                            self.swarm.behaviour_mut().gossipsub.add_explicit_peer(&bootnode_peer_id);
                            if let Ok(mut count) = self.peer_count.try_write() {
                                *count = self.peers.len();
                            }
                            let _ = self.event_tx.send(NetworkEvent::PeerConnected(bootnode_peer_id));
                        }
                        continue; // Skip dial, already connected
                    }
                    if self.pending_dials.contains(&bootnode_peer_id) {
                        println!("[RETRY] Already have pending dial to: {}", bootnode_peer_id);
                        continue;
                    }
                }
                
                // Use DialOpts with PeerCondition for collision prevention
                let dial_result = if let Some(peer_id) = target_peer_id {
                    let opts = DialOpts::peer_id(peer_id)
                        .addresses(vec![addr.clone()])
                        .condition(PeerCondition::Disconnected)
                        .build();
                    self.swarm.dial(opts)
                } else {
                    self.swarm.dial(addr.clone())
                };
                
                match dial_result {
                    Ok(()) => {
                        println!("[RETRY] Dial initiated to: {}", addr);
                    }
                    Err(e) => {
                        let err_str = e.to_string();
                        // Connection might already be in progress, that's ok
                        if !err_str.contains("already pending") && 
                           !err_str.contains("AlreadyDialing") &&
                           !err_str.contains("AlreadyConnected") {
                            eprintln!("[RETRY] Dial failed: {:?}", e);
                            
                            // If direct connection failed and we have a relay address for this peer, try relay
                            if let Some(peer_id) = target_peer_id {
                                if let Some(relay_addr) = self.peer_relay_addresses.get(&peer_id) {
                                    println!("   🔄 Attempting relay connection to {} via: {}", peer_id, relay_addr);
                                    match self.swarm.dial(relay_addr.clone()) {
                                        Ok(()) => {
                                            println!("   ✅ Relay dial initiated");
                                        }
                                        Err(relay_err) => {
                                            eprintln!("   ❌ Relay dial also failed: {:?}", relay_err);
                                        }
                                    }
                                } else {
                                    println!("   ℹ️  No relay address available for peer {}", peer_id);
                                }
                            }
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

    // ========================================================================
    // REQUEST-RESPONSE SYNC PROTOCOL
    // Reliable, ordered block delivery - bypasses GossipSub deduplication issues
    // ========================================================================

    /// Request blocks from a specific peer via request-response protocol
    /// Returns the request_id for tracking the response
    pub fn request_blocks_rr(
        &mut self,
        peer: PeerId,
        from_height: u64,
        to_height: u64,
    ) -> Result<u64, Box<dyn std::error::Error>> {
        // Generate unique request_id
        let request_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        let request = SyncRequest::BlockRequest {
            from_height,
            to_height,
            request_id,
        };

        println!("📤 [RR-SYNC] Sending block request to {}: heights {}-{} (id: {})",
            peer, from_height, to_height, request_id);

        let outbound_request_id = self.swarm.behaviour_mut().sync.send_request(&peer, request);
        
        // Track this request so we can match the response
        self.outbound_sync_requests.insert(outbound_request_id, (peer, request_id));

        Ok(request_id)
    }

    /// Send blocks response via request-response protocol
    /// Called by service layer when handling BlocksRequested with rr_request_id
    pub fn send_blocks_response(
        &mut self,
        request_id: u64,
        blocks: Vec<Block>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(channel) = self.pending_sync_channels.remove(&request_id) {
            let num_blocks = blocks.len();
            let response = SyncResponse::BlockResponse { blocks, request_id };
            
            println!("📤 [RR-SYNC] Sending {} blocks response (id: {})", num_blocks, request_id);
            
            match self.swarm.behaviour_mut().sync.send_response(channel, response) {
                Ok(()) => Ok(()),
                Err(resp) => {
                    eprintln!("❌ [RR-SYNC] Failed to send blocks response: channel closed");
                    Err(format!("Failed to send response: {:?}", resp).into())
                }
            }
        } else {
            eprintln!("❌ [RR-SYNC] No pending channel for request_id {}", request_id);
            Err(format!("No pending channel for request_id {}", request_id).into())
        }
    }

    /// Send error response via request-response protocol
    pub fn send_error_response(
        &mut self,
        request_id: u64,
        message: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(channel) = self.pending_sync_channels.remove(&request_id) {
            let response = SyncResponse::Error { message: message.clone(), request_id };
            
            println!("📤 [RR-SYNC] Sending error response (id: {}): {}", request_id, message);
            
            match self.swarm.behaviour_mut().sync.send_response(channel, response) {
                Ok(()) => Ok(()),
                Err(resp) => {
                    eprintln!("❌ [RR-SYNC] Failed to send error response: channel closed");
                    Err(format!("Failed to send response: {:?}", resp).into())
                }
            }
        } else {
            eprintln!("❌ [RR-SYNC] No pending channel for request_id {}", request_id);
            Err(format!("No pending channel for request_id {}", request_id).into())
        }
    }

    /// Handle incoming gossipsub message
    fn handle_gossipsub_message(&mut self, peer: PeerId, message: Vec<u8>, topic_hash: &gossipsub::TopicHash) {
        // Check if this is a light sync message
        let light_sync_topic_hash = self.topics.light_sync.hash();
        if *topic_hash == light_sync_topic_hash {
            self.handle_light_sync_message(peer, message);
            return;
        }
        
        // Handle regular network messages
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
                genesis_hash,
                node_type,
                timestamp: _, // Ignored - only used to prevent gossipsub duplicate rejection
            }) => {
                // CRITICAL: Validate genesis hash - reject peers on different chains
                // This is part of the "handshake" - if genesis doesn't match, disconnect
                let _ = self.event_tx.send(NetworkEvent::StatusUpdate {
                    peer,
                    best_height,
                    best_hash,
                    genesis_hash, // Pass genesis_hash to handler for validation
                    node_type,
                });
            }
            Ok(NetworkMessage::GetBlocks { from, to, request_id: _ }) => {
                // request_id is only used to bypass gossipsub dedup, not needed after deserialize
                // rr_request_id is None because this is a GossipSub request, not request-response
                let _ = self.event_tx.send(NetworkEvent::BlocksRequested {
                    peer,
                    from_height: from,
                    to_height: to,
                    rr_request_id: None, // GossipSub request - respond via GossipSub
                });
            }
            Ok(_) => {
                // Other message types handled separately
            }
            Err(e) => {
                eprintln!("❌ Failed to deserialize NetworkMessage from peer {:?}: {}", peer, e);
                eprintln!("   Message length: {} bytes", message.len());
                if message.len() > 0 {
                    let preview_len = message.len().min(32);
                    let hex_preview: String = message[..preview_len].iter()
                        .map(|b| format!("{:02x}", b))
                        .collect();
                    eprintln!("   First {} bytes (hex): {}", preview_len, hex_preview);
                }
                // Try to deserialize as LightSyncNetworkMessage to see if it's a topic mismatch
                if let Ok(_light_msg) = bincode::deserialize::<LightSyncNetworkMessage>(&message) {
                    eprintln!("   ⚠️  Message is actually a LightSyncNetworkMessage (topic mismatch?)");
                }
            }
        }
    }
    
    /// Handle incoming light sync protocol messages
    fn handle_light_sync_message(&mut self, peer: PeerId, message: Vec<u8>) {
        match bincode::deserialize::<LightSyncNetworkMessage>(&message) {
            Ok(LightSyncNetworkMessage::GetHeaders { start_height, max_headers, request_id }) => {
                let _ = self.event_tx.send(NetworkEvent::HeadersRequested {
                    peer,
                    start_height,
                    max_headers,
                    request_id,
                });
            }
            Ok(LightSyncNetworkMessage::Headers { headers, request_id }) => {
                let _ = self.event_tx.send(NetworkEvent::HeadersReceived {
                    peer,
                    headers,
                    request_id,
                });
            }
            Ok(LightSyncNetworkMessage::GetFlyClientProof { security_param, request_id }) => {
                let _ = self.event_tx.send(NetworkEvent::FlyClientProofRequested {
                    peer,
                    security_param,
                    request_id,
                });
            }
            Ok(LightSyncNetworkMessage::FlyClientProof { proof_data, request_id }) => {
                let _ = self.event_tx.send(NetworkEvent::FlyClientProofReceived {
                    peer,
                    proof_data,
                    request_id,
                });
            }
            Ok(LightSyncNetworkMessage::GetMMRProof { block_height, request_id }) => {
                let _ = self.event_tx.send(NetworkEvent::MMRProofRequested {
                    peer,
                    block_height,
                    request_id,
                });
            }
            Ok(LightSyncNetworkMessage::MMRProof { header, proof_data, mmr_root, request_id }) => {
                let _ = self.event_tx.send(NetworkEvent::MMRProofReceived {
                    peer,
                    header,
                    proof_data,
                    mmr_root,
                    request_id,
                });
            }
            Ok(LightSyncNetworkMessage::GetTxProof { tx_hash, block_height, request_id }) => {
                let _ = self.event_tx.send(NetworkEvent::TxProofRequested {
                    peer,
                    tx_hash,
                    block_height,
                    request_id,
                });
            }
            Ok(LightSyncNetworkMessage::TxProof { tx_hash, merkle_path, block_height, request_id }) => {
                let _ = self.event_tx.send(NetworkEvent::TxProofReceived {
                    peer,
                    tx_hash,
                    merkle_path,
                    block_height,
                    request_id,
                });
            }
            Ok(LightSyncNetworkMessage::GetChainTip { request_id }) => {
                let _ = self.event_tx.send(NetworkEvent::ChainTipRequested {
                    peer,
                    request_id,
                });
            }
            Ok(LightSyncNetworkMessage::ChainTip { height, hash, mmr_root, total_work, request_id }) => {
                let _ = self.event_tx.send(NetworkEvent::ChainTipReceived {
                    peer,
                    height,
                    hash,
                    mmr_root,
                    total_work,
                    request_id,
                });
            }
            Err(e) => {
                eprintln!("Failed to deserialize light sync message: {}", e);
            }
        }
    }

    /// Process swarm events (call this in a loop)
    pub async fn process_events(&mut self) {
        // CRITICAL: Sync connected peers before processing events
        // This catches connections that exist in swarm but weren't tracked
        self.sync_connected_peers();
        
        match self.swarm.select_next_some().await {
            SwarmEvent::Behaviour(event) => match event {
                CoinjectBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                    propagation_source,
                    message,
                    ..
                }) => {
                    self.handle_gossipsub_message(propagation_source, message.data, &message.topic);
                }
                CoinjectBehaviourEvent::Gossipsub(gossipsub::Event::Subscribed { peer_id, topic }) => {
                    println!("📡 GOSSIPSUB: Peer {} subscribed to topic: {:?}", peer_id, topic);
                    // When a peer subscribes to blocks topic, they're in the mesh
                    let blocks_topic_hash = self.topics.blocks.hash();
                    if topic == blocks_topic_hash {
                        self.mesh_peers.insert(peer_id);
                        println!("   ✅ Peer added to blocks mesh (mesh size: {})", self.mesh_peers.len());
                    }
                }
                CoinjectBehaviourEvent::Gossipsub(gossipsub::Event::Unsubscribed { peer_id, topic }) => {
                    println!("📡 GOSSIPSUB: Peer {} unsubscribed from topic: {:?}", peer_id, topic);
                    let blocks_topic_hash = self.topics.blocks.hash();
                    if topic == blocks_topic_hash {
                        self.mesh_peers.remove(&peer_id);
                        println!("   ⚠️  Peer removed from blocks mesh (mesh size: {})", self.mesh_peers.len());
                    }
                }
                CoinjectBehaviourEvent::Gossipsub(gossipsub::Event::GossipsubNotSupported { peer_id }) => {
                    // DIAGNOSTIC: Peer doesn't support gossipsub protocol
                    println!("🚨 PROTOCOL NEGOTIATION FAILED: Peer {} does not support gossipsub!", peer_id);
                    println!("   This peer cannot participate in block/tx propagation.");
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
                        "[IDENTIFY] Received from peer: {} - protocol: {} - agent: {}",
                        peer_id, info.protocol_version, info.agent_version
                    );
                    
                    // Log all listen addresses before filtering
                    println!("[IDENTIFY] Peer {} reported {} listen address(es):", peer_id, info.listen_addrs.len());
                    for addr in &info.listen_addrs {
                        println!("[IDENTIFY]   Raw: {}", addr);
                    }
                    
                    // Extract relay addresses first (they bypass normal filtering)
                    let mut relay_addrs = Vec::new();
                    let mut non_relay_addrs = Vec::new();
                    
                    for addr in info.listen_addrs {
                        let addr_str = addr.to_string();
                        if addr_str.contains("/p2p-circuit/") {
                            // This is a relay address - store it for fallback connection attempts
                            println!("[IDENTIFY]   🔄 Found relay address for peer {}: {}", peer_id, addr);
                            self.peer_relay_addresses.insert(peer_id, addr.clone());
                            relay_addrs.push(addr);
                        } else {
                            non_relay_addrs.push(addr);
                        }
                    }
                    
                    // Filter non-relay addresses using addr_filter
                    let filtered_addrs = filter_multiaddrs_with_logging(
                        non_relay_addrs,
                        &self.addr_filter_config,
                        &format!("identify:{}", peer_id),
                    );
                    
                    println!("[IDENTIFY] Accepted {} address(es) for peer {} ({} relay, {} filtered)", 
                        filtered_addrs.len() + relay_addrs.len(), peer_id, relay_addrs.len(), filtered_addrs.len());
                    
                    // Add relay addresses to kademlia
                    for addr in relay_addrs {
                        self.swarm
                            .behaviour_mut()
                            .kademlia
                            .add_address(&peer_id, addr);
                    }
                    
                    // Add filtered addresses to kademlia
                    for addr in filtered_addrs {
                        println!("[IDENTIFY]   Adding to kademlia: {}", addr);
                        self.swarm
                            .behaviour_mut()
                            .kademlia
                            .add_address(&peer_id, addr);
                    }
                }
                CoinjectBehaviourEvent::Identify(identify::Event::Sent { peer_id, .. }) => {
                    println!("🆔 IDENTIFY SENT to peer: {}", peer_id);
                }
                CoinjectBehaviourEvent::Identify(identify::Event::Error { peer_id, error, .. }) => {
                    eprintln!("🚨 IDENTIFY ERROR with peer {}: {:?}", peer_id, error);
                }
                CoinjectBehaviourEvent::Identify(identify::Event::Pushed { peer_id, info, .. }) => {
                    println!("🆔 IDENTIFY PUSHED to peer: {} (protocol: {})", peer_id, info.protocol_version);
                }
                CoinjectBehaviourEvent::Kademlia(kad::Event::RoutingUpdated {
                    peer,
                    ..
                }) => {
                    println!("Kademlia routing updated for peer: {}", peer);
                }
                CoinjectBehaviourEvent::Autonat(autonat::Event::StatusChanged { old, new }) => {
                    println!("🌐 Autonat status changed: {:?} -> {:?}", old, new);
                }
                CoinjectBehaviourEvent::Autonat(autonat::Event::InboundProbe { .. }) => {
                    // NAT detection probe received
                }
                CoinjectBehaviourEvent::Autonat(autonat::Event::OutboundProbe { .. }) => {
                    // NAT detection probe sent
                }
                CoinjectBehaviourEvent::Relay(relay::Event::ReservationReqAccepted { .. }) => {
                    println!("🔄 Relay reservation request accepted");
                    println!("   ℹ️  Relay reservation obtained - peers can now connect via relay");
                    // Note: In libp2p 0.54, relay addresses are automatically advertised via identify protocol
                    // We extract them from identify::Event::Received (listen_addrs with /p2p-circuit/)
                }
                CoinjectBehaviourEvent::Relay(relay::Event::ReservationReqDenied { .. }) => {
                    println!("⚠️  Relay reservation request denied");
                }
                CoinjectBehaviourEvent::Relay(relay::Event::CircuitReqDenied { .. }) => {
                    println!("⚠️  Relay circuit request denied");
                }
                CoinjectBehaviourEvent::Relay(relay::Event::CircuitReqAccepted { src_peer_id, dst_peer_id, .. }) => {
                    println!("✅ Relay circuit established");
                    println!("   From: {} → To: {}", src_peer_id, dst_peer_id);
                    println!("   🔄 Connection via relay is now active");
                }
                
                // ============================================================
                // REQUEST-RESPONSE SYNC PROTOCOL EVENTS
                // Reliable, ordered block delivery - bypasses GossipSub issues
                // ============================================================
                CoinjectBehaviourEvent::Sync(request_response::Event::Message { peer, message }) => {
                    match message {
                        request_response::Message::Request { request, channel, request_id } => {
                            // Incoming block request - store channel for response
                            match &request {
                                SyncRequest::BlockRequest { from_height, to_height, request_id: req_id } => {
                                    println!("📥 [RR-SYNC] Block request from {}: heights {}-{} (id: {})", 
                                        peer, from_height, to_height, req_id);
                                    
                                    // Store channel for response (keyed by the request_id in the message)
                                    self.pending_sync_channels.insert(*req_id, channel);
                                    
                                    // Emit event to service layer to fetch blocks and respond
                                    let _ = self.event_tx.send(NetworkEvent::BlocksRequested {
                                        peer,
                                        from_height: *from_height,
                                        to_height: *to_height,
                                        rr_request_id: Some(*req_id),
                                    });
                                }
                            }
                        }
                        request_response::Message::Response { response, request_id } => {
                            // Incoming block response - emit BlockReceived events
                            if let Some((peer_id, our_req_id)) = self.outbound_sync_requests.remove(&request_id) {
                                match response {
                                    SyncResponse::BlockResponse { blocks, request_id: resp_id } => {
                                        println!("📥 [RR-SYNC] Received {} blocks from {} (id: {})", 
                                            blocks.len(), peer_id, resp_id);
                                        // Emit BlockReceived events IN ORDER
                                        for block in blocks {
                                            let height = block.header.height;
                                            println!("   ↳ Block {} received via RR", height);
                                            let _ = self.event_tx.send(NetworkEvent::BlockReceived {
                                                block,
                                                peer: peer_id,
                                                is_sync_block: true,
                                            });
                                        }
                                    }
                                    SyncResponse::Error { message, request_id: err_id } => {
                                        eprintln!("❌ [RR-SYNC] Error from {} (id: {}): {}", peer_id, err_id, message);
                                    }
                                }
                            } else {
                                eprintln!("⚠️ [RR-SYNC] Received response for unknown request: {:?}", request_id);
                            }
                        }
                    }
                }
                CoinjectBehaviourEvent::Sync(request_response::Event::OutboundFailure { peer, request_id, error }) => {
                    eprintln!("❌ [RR-SYNC] Outbound request to {} failed (id: {:?}): {:?}", peer, request_id, error);
                    self.outbound_sync_requests.remove(&request_id);
                }
                CoinjectBehaviourEvent::Sync(request_response::Event::InboundFailure { peer, request_id, error }) => {
                    eprintln!("❌ [RR-SYNC] Inbound request from {} failed (id: {:?}): {:?}", peer, request_id, error);
                }
                CoinjectBehaviourEvent::Sync(request_response::Event::ResponseSent { peer, request_id }) => {
                    println!("✅ [RR-SYNC] Response sent to {} (id: {:?})", peer, request_id);
                }
                
                _ => {}
            },
            SwarmEvent::ConnectionEstablished { 
                peer_id, 
                endpoint, 
                num_established,
                concurrent_dial_errors,
                established_in,
                ..
            } => {
                // DIAGNOSTIC: Log full connection establishment details for debugging
                let direction = if endpoint.is_dialer() { "outbound" } else { "inbound" };
                let addr = endpoint.get_remote_address();
                let handshake_time = format!("{:?}", established_in);
                
                println!("🔗 CONNECTION ESTABLISHED [{}]", direction.to_uppercase());
                println!("   Peer: {}", peer_id);
                println!("   Address: {}", addr);
                println!("   Handshake time: {} (Noise+Yamux negotiation)", handshake_time);
                println!("   Concurrent connections to peer: {}", num_established);
                
                // Log any dial errors that occurred during connection attempts
                if let Some(errors) = concurrent_dial_errors {
                    if !errors.is_empty() {
                        println!("   ⚠️  Dial errors during connection:");
                        for (addr, err) in errors {
                            println!("      {} → {:?}", addr, err);
                        }
                    }
                }
                
                // Remove from pending dials now that connection is established
                if endpoint.is_dialer() {
                    self.pending_dials.remove(&peer_id);
                }
                
                // CRITICAL: Check if this peer is blacklisted - if so, disconnect immediately
                if self.blacklisted_peers.contains(&peer_id) {
                    println!("🚫 DISCONNECTING blacklisted peer immediately: {}", peer_id);
                    println!("   Connection established but peer is blacklisted - closing connection");
                    self.swarm.disconnect_peer_id(peer_id);
                    // Remove from any tracking
                    self.peers.remove(&peer_id);
                    self.mesh_peers.remove(&peer_id);
                    // Remove from incoming connection tracking
                    self.incoming_connection_attempts.retain(|_, addr| {
                        !addr.to_string().contains(&peer_id.to_string())
                    });
                    return; // Don't process this connection further
                }
                
                // Remove from incoming connection tracking since it succeeded
                // Find the connection_id by matching the peer_id (we'll track this better in future)
                // For now, just clear any stale entries
                self.incoming_connection_attempts.retain(|_, addr| {
                    // Keep entries that don't match this peer's address
                    !addr.to_string().contains(&peer_id.to_string())
                });
                
                // CRITICAL FIX: Only track peer on first connection, not every connection
                // Multiple connections to same peer can cause issues
                let is_new_peer = !self.peers.contains(&peer_id);
                if is_new_peer {
                    self.peers.insert(peer_id);
                    
                    // Add peer to gossipsub mesh explicitly to ensure it participates in message propagation
                    self.swarm
                        .behaviour_mut()
                        .gossipsub
                        .add_explicit_peer(&peer_id);
                    
                    self.mesh_peers.insert(peer_id);
                    
                    // Update shared peer count
                    if let Ok(mut count) = self.peer_count.try_write() {
                        *count = self.peers.len();
                    }
                    let _ = self.event_tx.send(NetworkEvent::PeerConnected(peer_id));
                }
                
                // Log mesh state after adding peer
                let mesh_size = self.mesh_peers.len();
                let total_peers = self.peers.len();
                println!("   📊 Network state: {} total peers, {} in gossipsub mesh (new peer: {})", total_peers, mesh_size, is_new_peer);
            }
            SwarmEvent::ConnectionClosed { peer_id, cause, num_established, connection_id, endpoint } => {
                // DIAGNOSTIC: Detailed connection close analysis for stability debugging
                let direction = if endpoint.is_dialer() { "outbound" } else { "inbound" };
                let reason = match &cause {
                    Some(err) => {
                        // Categorize the error type for easier diagnosis
                        let err_str = format!("{:?}", err);
                        if err_str.contains("Timeout") || err_str.contains("timeout") {
                            format!("TIMEOUT - {}", err_str)
                        } else if err_str.contains("Reset") || err_str.contains("reset") {
                            format!("CONNECTION_RESET - {}", err_str)
                        } else if err_str.contains("Refused") || err_str.contains("refused") {
                            format!("CONNECTION_REFUSED - {}", err_str)
                        } else if err_str.contains("Io") || err_str.contains("IO") {
                            format!("IO_ERROR - {}", err_str)
                        } else if err_str.contains("KeepAlive") || err_str.contains("keep-alive") {
                            format!("KEEPALIVE_TIMEOUT - {}", err_str)
                        } else {
                            err_str
                        }
                    },
                    None => "GRACEFUL_CLOSE".to_string(),
                };
                
                let was_in_mesh = self.mesh_peers.contains(&peer_id);
                
                println!("❌ CONNECTION CLOSED [{}]", direction.to_uppercase());
                println!("   Peer: {}", peer_id);
                println!("   Connection ID: {:?}", connection_id);
                println!("   Reason: {}", reason);
                println!("   Was in gossipsub mesh: {}", was_in_mesh);
                println!("   Remaining connections to peer: {}", num_established);
                
                // Only remove from tracking if ALL connections to this peer are closed
                if num_established == 0 {
                    self.peers.remove(&peer_id);
                    self.mesh_peers.remove(&peer_id);
                    // Remove peer from gossipsub mesh
                    self.swarm
                        .behaviour_mut()
                        .gossipsub
                        .remove_explicit_peer(&peer_id);
                    
                    // Log network state after removal
                    let mesh_size = self.mesh_peers.len();
                    let total_peers = self.peers.len();
                    println!("   📊 Network state after disconnect: {} total peers, {} in mesh", total_peers, mesh_size);
                    
                    // Alert if mesh is now empty
                    if mesh_size == 0 && total_peers == 0 {
                        println!("   🚨 WARNING: No peers remaining! Network isolated.");
                    }
                    
                    // Update shared peer count
                    if let Ok(mut count) = self.peer_count.try_write() {
                        *count = self.peers.len();
                    }
                    let _ = self.event_tx.send(NetworkEvent::PeerDisconnected(peer_id));
                }
            }
            SwarmEvent::NewListenAddr { address, listener_id } => {
                println!("[LISTEN] Listening on: {} (listener_id={:?})", address, listener_id);
                println!("[LISTEN]   Local PeerId: {}", self.local_peer_id);
                println!("[LISTEN]   Full bootnode addr: {}/p2p/{}", address, self.local_peer_id);
                
                // If we have an external address configured, remind the user
                if let Some(ref ext) = self.external_addr {
                    println!("[LISTEN]   External addr (advertised): {}/p2p/{}", ext, self.local_peer_id);
                }
            }
            SwarmEvent::NewExternalAddrCandidate { address } => {
                // CRITICAL: Filter invalid addresses (private IPs + wrong ports)
                // Docker NAT assigns ephemeral source ports that peers can't dial back
                let validation = validate_multiaddr(&address, &self.addr_filter_config);
                if !validation.is_accepted() {
                    println!("[ADDR_FILTER] Rejecting external address candidate: {} ({})", address, validation.reason());
                } else {
                    println!("✅ Adding external address: {}", address);
                    self.swarm.add_external_address(address);
                }
            }
            SwarmEvent::ExternalAddrConfirmed { address } => {
                // Remove confirmed addresses if they're invalid (private IP or wrong port)
                // Exception: relay addresses use different port logic
                let addr_str = address.to_string();
                if addr_str.contains("/p2p-circuit/") {
                    println!("✅ External relay address confirmed: {}", address);
                    println!("   🔄 This is a relay address - peers can use this to connect via relay");
                } else {
                    let validation = validate_multiaddr(&address, &self.addr_filter_config);
                    if !validation.is_accepted() {
                        println!("[ADDR_FILTER] Removing invalid confirmed address: {} ({})", address, validation.reason());
                        self.swarm.remove_external_address(&address);
                    } else {
                        println!("✅ External address confirmed: {}", address);
                    }
                }
            }
            SwarmEvent::ExternalAddrExpired { address } => {
                println!("📤 External address expired: {}", address);
            }
            SwarmEvent::OutgoingConnectionError { peer_id, error, connection_id } => {
                // Remove from pending dials
                if let Some(pid) = peer_id {
                    self.pending_dials.remove(&pid);
                }
                
                // DIAGNOSTIC: Detailed outgoing connection failure analysis
                let peer_str = peer_id.map(|p| p.to_string()).unwrap_or_else(|| "unknown".to_string());
                let err_str = format!("{:?}", error);
                
                // Categorize for easier diagnosis
                let category = if err_str.contains("Timeout") || err_str.contains("timeout") {
                    "TIMEOUT"
                } else if err_str.contains("Noise") || err_str.contains("noise") {
                    "NOISE_HANDSHAKE_FAILED"
                } else if err_str.contains("Yamux") || err_str.contains("yamux") {
                    "YAMUX_NEGOTIATION_FAILED"
                } else if err_str.contains("Transport") {
                    "TRANSPORT_ERROR"
                } else if err_str.contains("Denied") || err_str.contains("denied") {
                    "CONNECTION_DENIED"
                } else if err_str.contains("Refused") {
                    "CONNECTION_REFUSED"
                } else {
                    "OTHER"
                };
                
                eprintln!("[DIAL_ERR] Outgoing connection failed [{}] to {}: {}", category, peer_str, err_str);
                eprintln!("[DIAL_ERR]   Connection ID: {:?}", connection_id);
            }
            SwarmEvent::IncomingConnectionError { local_addr, send_back_addr, error, connection_id } => {
                // Remove from tracking since connection failed
                self.incoming_connection_attempts.remove(&connection_id);
                
                // Check if this was from a blacklisted peer
                let mut incoming_peer_id: Option<PeerId> = None;
                for proto in send_back_addr.iter() {
                    if let libp2p::multiaddr::Protocol::P2p(peer_id) = proto {
                        incoming_peer_id = Some(peer_id);
                        break;
                    }
                }
                if let Some(peer_id) = incoming_peer_id {
                    if self.blacklisted_peers.contains(&peer_id) {
                        // Silently ignore errors from blacklisted peers
                        return;
                    }
                }
                
                // DIAGNOSTIC: Detailed incoming connection failure analysis
                let err_str = format!("{:?}", error);
                
                let category = if err_str.contains("Timeout") || err_str.contains("timeout") {
                    "TIMEOUT"
                } else if err_str.contains("Noise") || err_str.contains("noise") {
                    "NOISE_HANDSHAKE_FAILED"
                } else if err_str.contains("Yamux") || err_str.contains("yamux") {
                    "YAMUX_NEGOTIATION_FAILED"
                } else if err_str.contains("Reset") || err_str.contains("reset") {
                    "CONNECTION_RESET"
                } else {
                    "OTHER"
                };
                
                eprintln!("❌ INCOMING CONNECTION FAILED [{}]", category);
                eprintln!("   From: {}", send_back_addr);
                eprintln!("   To local: {}", local_addr);
                eprintln!("   Connection ID: {:?}", connection_id);
                eprintln!("   Error: {}", err_str);
                
                // If this was a timeout from a bootnode, log it prominently
                for bootnode_addr in &self.bootnode_addrs {
                    let bootnode_ip = bootnode_addr.iter()
                        .find_map(|p| {
                            if let libp2p::multiaddr::Protocol::Ip4(ip) = p {
                                Some(ip)
                            } else {
                                None
                            }
                        });
                    
                    let incoming_ip = send_back_addr.iter()
                        .find_map(|p| {
                            if let libp2p::multiaddr::Protocol::Ip4(ip) = p {
                                Some(ip)
                            } else {
                                None
                            }
                        });
                    
                    if let (Some(boot_ip), Some(inc_ip)) = (bootnode_ip, incoming_ip) {
                        if boot_ip == inc_ip && category == "TIMEOUT" {
                            eprintln!("   🚨 CRITICAL: Bootnode handshake timeout - this may indicate:");
                            eprintln!("      - Simultaneous dial attempts causing conflicts");
                            eprintln!("      - Network latency issues");
                            eprintln!("      - Firewall/NAT interference");
                        }
                    }
                }
            }
            SwarmEvent::Dialing { peer_id, connection_id } => {
                if let Some(peer) = peer_id {
                    println!("[DIAL] Dialing peer: {} (conn_id={:?})", peer, connection_id);
                    self.pending_dials.insert(peer);
                } else {
                    println!("[DIAL] Dialing unknown peer (conn_id={:?})", connection_id);
                }
            }
            SwarmEvent::IncomingConnection { local_addr, send_back_addr, connection_id } => {
                // CRITICAL: Check if this connection is from a blacklisted peer
                // Extract peer ID from address if available
                let mut incoming_peer_id: Option<PeerId> = None;
                for proto in send_back_addr.iter() {
                    if let libp2p::multiaddr::Protocol::P2p(peer_id) = proto {
                        incoming_peer_id = Some(peer_id);
                        break;
                    }
                }
                
                // If we have a peer ID and it's blacklisted, reject the connection
                // Note: We can't reject at this stage, but we'll disconnect immediately after handshake
                if let Some(peer_id) = incoming_peer_id {
                    if self.blacklisted_peers.contains(&peer_id) {
                        println!("🚫 BLACKLISTED peer attempting connection: {}", peer_id);
                        println!("   From: {}", send_back_addr);
                        println!("   Connection ID: {:?}", connection_id);
                        println!("   ⚠️  Will disconnect after handshake completes");
                        // Store connection_id to disconnect after handshake
                        // We'll handle this in ConnectionEstablished event
                    }
                }
                
                // DIAGNOSTIC: Log incoming connection attempts BEFORE protocol negotiation
                println!("📥 INCOMING CONNECTION ATTEMPT");
                println!("   From: {}", send_back_addr);
                println!("   To local: {}", local_addr);
                println!("   Connection ID: {:?}", connection_id);
                println!("   (Noise+Yamux handshake starting...)");
                
                // CRITICAL: Track incoming connection attempts to prevent simultaneous outbound dials
                // If we're receiving an incoming connection from a bootnode, don't dial outbound
                self.incoming_connection_attempts.insert(connection_id, send_back_addr.clone());
                
                // Check if this incoming connection is from a bootnode
                // If so, we should stop trying to dial outbound to avoid conflicts
                for bootnode_addr in &self.bootnode_addrs {
                    // Extract IP from bootnode address
                    let bootnode_ip = bootnode_addr.iter()
                        .find_map(|p| {
                            if let libp2p::multiaddr::Protocol::Ip4(ip) = p {
                                Some(ip)
                            } else {
                                None
                            }
                        });
                    
                    // Check if incoming connection is from bootnode IP
                    let incoming_ip = send_back_addr.iter()
                        .find_map(|p| {
                            if let libp2p::multiaddr::Protocol::Ip4(ip) = p {
                                Some(ip)
                            } else {
                                None
                            }
                        });
                    
                    if let (Some(boot_ip), Some(inc_ip)) = (bootnode_ip, incoming_ip) {
                        if boot_ip == inc_ip {
                            println!("   ✅ Incoming connection from bootnode IP - will skip outbound dial attempts");
                        }
                    }
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
