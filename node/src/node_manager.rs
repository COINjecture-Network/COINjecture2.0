// =============================================================================
// Node Type Manager - Orchestration Layer
// =============================================================================
//
// This module is the CENTRAL ORCHESTRATOR that binds:
// 1. Node Type Classification (from node_types.rs)
// 2. Network Capabilities (what each node type can do)
// 3. Protocol Handlers (how each node type responds to messages)
// 4. Resource Management (storage, bandwidth, compute allocation)
//
// NOTE: Full node type orchestration is prepared for future use
#![allow(dead_code)]
//
// Architecture:
// ┌─────────────────────────────────────────────────────────────────┐
// │                    NodeTypeManager                               │
// │  ┌──────────────┐  ┌─────────────────┐  ┌───────────────────┐  │
// │  │Classification│──│NetworkCapability│──│ProtocolHandlers   │  │
// │  │   Manager    │  │    Registry     │  │   (per type)      │  │
// │  └──────────────┘  └─────────────────┘  └───────────────────┘  │
// │         │                  │                     │              │
// │         ▼                  ▼                     ▼              │
// │  ┌──────────────┐  ┌─────────────────┐  ┌───────────────────┐  │
// │  │ Behavioral   │  │  Capability     │  │   P2P Message     │  │
// │  │  Metrics     │  │   Flags         │  │   Routing         │  │
// │  └──────────────┘  └─────────────────┘  └───────────────────┘  │
// └─────────────────────────────────────────────────────────────────┘

use crate::light_sync::{FlyClientProof, LightClientVerifier, LightSyncMessage, LightSyncServer};
use crate::node_types::{
    ClassificationResult, NodeClassificationManager, NodeType, NodeTypeStatus,
};
use coinject_core::{Block, BlockHeader, Hash, Transaction};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, mpsc, RwLock};

// =============================================================================
// Network Capabilities
// =============================================================================

/// What a node type is capable of doing on the network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkCapabilities {
    // === Block Capabilities ===
    /// Can produce/mine new blocks
    pub can_produce_blocks: bool,
    /// Can validate full blocks (not just headers)
    pub can_validate_blocks: bool,
    /// Can serve full blocks to peers
    pub can_serve_blocks: bool,
    /// Can serve block headers to light clients
    pub can_serve_headers: bool,

    // === Storage Capabilities ===
    /// Stores full block history
    pub stores_full_history: bool,
    /// Stores only recent blocks (pruned)
    pub stores_pruned: bool,
    /// Stores only headers (light mode)
    pub stores_headers_only: bool,
    /// Maximum blocks to store (0 = unlimited)
    pub max_blocks_stored: u64,

    // === Sync Capabilities ===
    /// Can perform full block sync
    pub can_full_sync: bool,
    /// Can perform header-only sync
    pub can_header_sync: bool,
    /// Can generate/serve FlyClient proofs
    pub can_serve_flyclient: bool,
    /// Can verify FlyClient proofs
    pub can_verify_flyclient: bool,

    // === Transaction Capabilities ===
    /// Can relay transactions
    pub can_relay_transactions: bool,
    /// Can validate transactions
    pub can_validate_transactions: bool,
    /// Maintains transaction mempool
    pub maintains_mempool: bool,

    // === Problem Solving (Bounty) ===
    /// Can solve NP-problems
    pub can_solve_problems: bool,
    /// Priority for problem distribution
    pub problem_priority: u8,

    // === Oracle Capabilities ===
    /// Can provide oracle data feeds
    pub can_provide_oracle_data: bool,
    /// Oracle data types supported
    pub oracle_data_types: Vec<String>,

    // === Network Role ===
    /// Maximum outbound connections
    pub max_outbound_peers: usize,
    /// Maximum inbound connections
    pub max_inbound_peers: usize,
    /// Can serve as bootstrap node
    pub can_be_bootstrap: bool,
    /// Gossip participation level (0-100)
    pub gossip_participation: u8,
}

impl NetworkCapabilities {
    /// Get capabilities for a node type
    pub fn for_node_type(node_type: NodeType) -> Self {
        match node_type {
            NodeType::Light => Self::light_capabilities(),
            NodeType::Full => Self::full_capabilities(),
            NodeType::Archive => Self::archive_capabilities(),
            NodeType::Validator => Self::validator_capabilities(),
            NodeType::Bounty => Self::bounty_capabilities(),
            NodeType::Oracle => Self::oracle_capabilities(),
        }
    }

    fn light_capabilities() -> Self {
        NetworkCapabilities {
            // Block capabilities - minimal
            can_produce_blocks: false,
            can_validate_blocks: false, // Only validates headers
            can_serve_blocks: false,
            can_serve_headers: false,

            // Storage - headers only
            stores_full_history: false,
            stores_pruned: false,
            stores_headers_only: true,
            max_blocks_stored: 0, // Only headers

            // Sync - header/FlyClient only
            can_full_sync: false,
            can_header_sync: true,
            can_serve_flyclient: false,
            can_verify_flyclient: true,

            // Transactions - relay only
            can_relay_transactions: true,
            can_validate_transactions: false,
            maintains_mempool: false,

            // No problem solving
            can_solve_problems: false,
            problem_priority: 0,

            // No oracle
            can_provide_oracle_data: false,
            oracle_data_types: vec![],

            // Minimal network role
            max_outbound_peers: 8,
            max_inbound_peers: 0, // Light nodes don't serve others
            can_be_bootstrap: false,
            gossip_participation: 30, // Limited gossip
        }
    }

    fn full_capabilities() -> Self {
        NetworkCapabilities {
            // Full block capabilities
            can_produce_blocks: false, // Validators produce
            can_validate_blocks: true,
            can_serve_blocks: true,
            can_serve_headers: true,

            // Recent history storage
            stores_full_history: false,
            stores_pruned: true,
            stores_headers_only: false,
            max_blocks_stored: 100_000, // ~6 months

            // Full sync capabilities
            can_full_sync: true,
            can_header_sync: true,
            can_serve_flyclient: true,
            can_verify_flyclient: true,

            // Full transaction support
            can_relay_transactions: true,
            can_validate_transactions: true,
            maintains_mempool: true,

            // No problem solving (use Bounty for that)
            can_solve_problems: false,
            problem_priority: 0,

            // No oracle
            can_provide_oracle_data: false,
            oracle_data_types: vec![],

            // Standard network role
            max_outbound_peers: 25,
            max_inbound_peers: 25,
            can_be_bootstrap: false,
            gossip_participation: 80,
        }
    }

    fn archive_capabilities() -> Self {
        NetworkCapabilities {
            // Full block capabilities
            can_produce_blocks: false,
            can_validate_blocks: true,
            can_serve_blocks: true,
            can_serve_headers: true,

            // FULL history storage
            stores_full_history: true,
            stores_pruned: false,
            stores_headers_only: false,
            max_blocks_stored: 0, // Unlimited

            // All sync capabilities
            can_full_sync: true,
            can_header_sync: true,
            can_serve_flyclient: true,
            can_verify_flyclient: true,

            // Full transaction support
            can_relay_transactions: true,
            can_validate_transactions: true,
            maintains_mempool: true,

            // No problem solving
            can_solve_problems: false,
            problem_priority: 0,

            // No oracle
            can_provide_oracle_data: false,
            oracle_data_types: vec![],

            // Heavy network role - serve everyone
            max_outbound_peers: 50,
            max_inbound_peers: 100,    // Serve many peers
            can_be_bootstrap: true,    // Can be bootstrap
            gossip_participation: 100, // Full participation
        }
    }

    fn validator_capabilities() -> Self {
        NetworkCapabilities {
            // FULL block production
            can_produce_blocks: true,
            can_validate_blocks: true,
            can_serve_blocks: true,
            can_serve_headers: true,

            // Recent history (pruned)
            stores_full_history: false,
            stores_pruned: true,
            stores_headers_only: false,
            max_blocks_stored: 50_000, // ~3 months

            // All sync capabilities
            can_full_sync: true,
            can_header_sync: true,
            can_serve_flyclient: true,
            can_verify_flyclient: true,

            // Full transaction support
            can_relay_transactions: true,
            can_validate_transactions: true,
            maintains_mempool: true,

            // No problem solving (focus on validation)
            can_solve_problems: false,
            problem_priority: 0,

            // No oracle
            can_provide_oracle_data: false,
            oracle_data_types: vec![],

            // Well-connected for block propagation
            max_outbound_peers: 50,
            max_inbound_peers: 50,
            can_be_bootstrap: true,
            gossip_participation: 100,
        }
    }

    fn bounty_capabilities() -> Self {
        NetworkCapabilities {
            // Minimal block capabilities
            can_produce_blocks: false,
            can_validate_blocks: true,
            can_serve_blocks: false, // Focus on solving
            can_serve_headers: false,

            // Minimal storage
            stores_full_history: false,
            stores_pruned: true,
            stores_headers_only: false,
            max_blocks_stored: 10_000, // Just recent

            // Basic sync
            can_full_sync: true,
            can_header_sync: true,
            can_serve_flyclient: false,
            can_verify_flyclient: true,

            // Transaction relay
            can_relay_transactions: true,
            can_validate_transactions: true,
            maintains_mempool: false, // Focus on problems

            // PROBLEM SOLVING FOCUS
            can_solve_problems: true,
            problem_priority: 100, // Highest priority

            // No oracle
            can_provide_oracle_data: false,
            oracle_data_types: vec![],

            // Minimal connections (focus compute on solving)
            max_outbound_peers: 10,
            max_inbound_peers: 5,
            can_be_bootstrap: false,
            gossip_participation: 50,
        }
    }

    fn oracle_capabilities() -> Self {
        NetworkCapabilities {
            // Block capabilities
            can_produce_blocks: false,
            can_validate_blocks: true,
            can_serve_blocks: true,
            can_serve_headers: true,

            // Moderate storage
            stores_full_history: false,
            stores_pruned: true,
            stores_headers_only: false,
            max_blocks_stored: 50_000,

            // Sync capabilities
            can_full_sync: true,
            can_header_sync: true,
            can_serve_flyclient: true,
            can_verify_flyclient: true,

            // Full transaction support
            can_relay_transactions: true,
            can_validate_transactions: true,
            maintains_mempool: true,

            // No problem solving
            can_solve_problems: false,
            problem_priority: 0,

            // ORACLE FOCUS
            can_provide_oracle_data: true,
            oracle_data_types: vec![
                "price_feed".to_string(),
                "weather".to_string(),
                "random".to_string(),
                "cross_chain".to_string(),
            ],

            // Well-connected for data distribution
            max_outbound_peers: 30,
            max_inbound_peers: 50,
            can_be_bootstrap: false,
            gossip_participation: 90,
        }
    }

    /// Check if this node can handle a specific request type
    pub fn can_handle(&self, request_type: &RequestType) -> bool {
        match request_type {
            RequestType::GetBlocks { .. } => self.can_serve_blocks,
            RequestType::GetHeaders { .. } => self.can_serve_headers,
            RequestType::GetFlyClientProof => self.can_serve_flyclient,
            RequestType::GetTransaction { .. } => self.can_relay_transactions,
            RequestType::SubmitProblemSolution { .. } => self.can_solve_problems,
            RequestType::GetOracleData { .. } => self.can_provide_oracle_data,
            RequestType::NewBlock { .. } => self.can_validate_blocks,
            RequestType::NewTransaction { .. } => self.can_relay_transactions,
        }
    }
}

/// Types of requests that can be made
#[derive(Debug, Clone)]
pub enum RequestType {
    GetBlocks { from: u64, to: u64 },
    GetHeaders { from: u64, to: u64 },
    GetFlyClientProof,
    GetTransaction { hash: Hash },
    SubmitProblemSolution { problem_id: Hash },
    GetOracleData { data_type: String },
    NewBlock { block: Box<Block> },
    NewTransaction { tx: Box<Transaction> },
}

// =============================================================================
// Protocol Handler Registry
// =============================================================================

/// Trait for node-type-specific protocol handlers
pub trait ProtocolHandler: Send + Sync {
    /// Handle an incoming message
    fn handle_message(&self, msg: &NodeMessage) -> Option<NodeMessage>;

    /// Get supported message types
    fn supported_messages(&self) -> Vec<MessageType>;

    /// Priority for handling (higher = first)
    fn priority(&self) -> u8;
}

/// Message types in the protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MessageType {
    // Block-related
    BlockAnnounce,
    BlockRequest,
    BlockResponse,

    // Header-related (Light sync)
    HeaderRequest,
    HeaderResponse,

    // FlyClient
    FlyClientProofRequest,
    FlyClientProofResponse,

    // Transactions
    TransactionAnnounce,
    TransactionRequest,
    TransactionResponse,

    // Problem marketplace
    ProblemAnnounce,
    SolutionSubmit,
    SolutionVerify,

    // Oracle
    OracleDataRequest,
    OracleDataResponse,

    // Status
    StatusRequest,
    StatusResponse,

    // Peer discovery
    PeerRequest,
    PeerResponse,
}

/// Unified message format for P2P
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMessage {
    /// Message type
    pub msg_type: MessageType,
    /// Request/correlation ID
    pub request_id: u64,
    /// Sender node type (so receivers know capabilities)
    pub sender_type: NodeType,
    /// Payload (serialized)
    pub payload: Vec<u8>,
    /// Timestamp
    pub timestamp: u64,
}

impl NodeMessage {
    pub fn new(msg_type: MessageType, sender_type: NodeType, payload: Vec<u8>) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        NodeMessage {
            msg_type,
            request_id: rand::random(),
            sender_type,
            payload,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
}

// =============================================================================
// Node Type Manager
// =============================================================================

/// Central orchestrator for node type management
pub struct NodeTypeManager {
    // === Core Components ===
    /// Classification manager (determines node type from behavior)
    classification: Arc<RwLock<NodeClassificationManager>>,
    /// Current capabilities based on classification
    capabilities: Arc<RwLock<NetworkCapabilities>>,
    /// Target node type (operator preference)
    target_type: NodeType,

    // === Protocol Components ===
    /// Light sync server (for serving Light clients)
    light_sync_server: Option<Arc<RwLock<LightSyncServer>>>,
    /// FlyClient verifier (for Light nodes)
    flyclient_verifier: Option<Arc<RwLock<LightClientVerifier>>>,

    // === State ===
    /// Start time for uptime tracking
    start_time: Instant,
    /// Last capability update
    last_capability_update: Arc<RwLock<Instant>>,
    /// Pending requests by type
    pending_requests: Arc<RwLock<HashMap<u64, PendingRequest>>>,

    // === Channels ===
    /// Outbound message channel
    outbound_tx: mpsc::UnboundedSender<NodeMessage>,
    /// Classification change broadcast
    classification_broadcast: broadcast::Sender<ClassificationResult>,

    // === Metrics ===
    /// Messages handled by type
    messages_handled: Arc<RwLock<HashMap<MessageType, u64>>>,
    /// Requests served
    requests_served: Arc<RwLock<u64>>,
    /// Requests rejected (capability mismatch)
    requests_rejected: Arc<RwLock<u64>>,
}

/// Pending request tracking
#[derive(Debug)]
struct PendingRequest {
    request_type: RequestType,
    requested_at: Instant,
    peer_id: String,
}

impl NodeTypeManager {
    /// Create new node type manager
    pub fn new(
        initial_height: u64,
        target_type: NodeType,
        genesis_header: Option<BlockHeader>,
    ) -> (
        Self,
        mpsc::UnboundedReceiver<NodeMessage>,
        broadcast::Receiver<ClassificationResult>,
    ) {
        let (outbound_tx, outbound_rx) = mpsc::unbounded_channel();
        let (classification_broadcast, classification_rx) = broadcast::channel(16);

        // Initialize classification manager
        let mut classification_manager = NodeClassificationManager::new(initial_height);
        classification_manager.set_target_type(target_type);

        // Set headers-only if Light mode
        if matches!(target_type, NodeType::Light) {
            classification_manager.set_headers_only(true);
        }

        // Initialize capabilities for target type
        let capabilities = NetworkCapabilities::for_node_type(target_type);

        // Initialize LightSync components based on type
        let (light_sync_server, flyclient_verifier) = match target_type {
            NodeType::Light => {
                // Light nodes need FlyClient verifier
                let verifier = genesis_header
                    .as_ref()
                    .map(|h| LightClientVerifier::new(h.hash()));
                (None, verifier.map(|v| Arc::new(RwLock::new(v))))
            }
            NodeType::Full | NodeType::Archive | NodeType::Validator => {
                // Full+ nodes can serve Light clients
                let server = genesis_header.map(LightSyncServer::new);
                (server.map(|s| Arc::new(RwLock::new(s))), None)
            }
            _ => (None, None),
        };

        let manager = NodeTypeManager {
            classification: Arc::new(RwLock::new(classification_manager)),
            capabilities: Arc::new(RwLock::new(capabilities)),
            target_type,
            light_sync_server,
            flyclient_verifier,
            start_time: Instant::now(),
            last_capability_update: Arc::new(RwLock::new(Instant::now())),
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
            outbound_tx,
            classification_broadcast,
            messages_handled: Arc::new(RwLock::new(HashMap::new())),
            requests_served: Arc::new(RwLock::new(0)),
            requests_rejected: Arc::new(RwLock::new(0)),
        };

        (manager, outbound_rx, classification_rx)
    }

    /// Get current node type
    pub async fn current_type(&self) -> NodeType {
        self.classification.read().await.current_type()
    }

    /// Get current capabilities
    pub async fn capabilities(&self) -> NetworkCapabilities {
        self.capabilities.read().await.clone()
    }

    /// Check if we can handle a request
    pub async fn can_handle(&self, request_type: &RequestType) -> bool {
        self.capabilities.read().await.can_handle(request_type)
    }

    /// Process incoming message
    pub async fn process_message(&self, msg: NodeMessage) -> Option<NodeMessage> {
        // Track message
        {
            let mut handled = self.messages_handled.write().await;
            *handled.entry(msg.msg_type).or_insert(0) += 1;
        }

        // Check if we can handle this message type
        let can_handle = match msg.msg_type {
            MessageType::BlockRequest | MessageType::BlockResponse => {
                self.capabilities.read().await.can_serve_blocks
            }
            MessageType::HeaderRequest | MessageType::HeaderResponse => {
                self.capabilities.read().await.can_serve_headers
            }
            MessageType::FlyClientProofRequest | MessageType::FlyClientProofResponse => {
                let caps = self.capabilities.read().await;
                caps.can_serve_flyclient || caps.can_verify_flyclient
            }
            MessageType::ProblemAnnounce | MessageType::SolutionSubmit => {
                self.capabilities.read().await.can_solve_problems
            }
            MessageType::OracleDataRequest | MessageType::OracleDataResponse => {
                self.capabilities.read().await.can_provide_oracle_data
            }
            _ => true, // Status, peer discovery, etc. - always handle
        };

        if !can_handle {
            *self.requests_rejected.write().await += 1;
            tracing::debug!(
                "Rejecting {:?} - not supported by {:?} node",
                msg.msg_type,
                self.current_type().await
            );
            return None;
        }

        // Route to appropriate handler
        match msg.msg_type {
            MessageType::HeaderRequest => self.handle_header_request(&msg).await,
            MessageType::FlyClientProofRequest => self.handle_flyclient_request(&msg).await,
            MessageType::StatusRequest => self.handle_status_request(&msg).await,
            _ => {
                // Other messages passed through for service.rs handling
                *self.requests_served.write().await += 1;
                None
            }
        }
    }

    /// Handle header request (for Light clients)
    async fn handle_header_request(&self, msg: &NodeMessage) -> Option<NodeMessage> {
        let server = self.light_sync_server.as_ref()?;

        // Deserialize request
        let request: LightSyncMessage = bincode::deserialize(&msg.payload).ok()?;

        let response = match request {
            LightSyncMessage::GetHeaders {
                start_height,
                max_headers,
                request_id,
            } => server
                .read()
                .await
                .handle_get_headers(start_height, max_headers, request_id),
            LightSyncMessage::GetChainTip { request_id } => {
                // handle_get_chain_tip returns Option<LightSyncMessage>, unwrap or return None
                server.read().await.handle_get_chain_tip(request_id)?
            }
            _ => return None,
        };

        let payload = bincode::serialize(&response).ok()?;
        *self.requests_served.write().await += 1;

        Some(NodeMessage::new(
            MessageType::HeaderResponse,
            self.current_type().await,
            payload,
        ))
    }

    /// Handle FlyClient proof request
    async fn handle_flyclient_request(&self, msg: &NodeMessage) -> Option<NodeMessage> {
        let server = self.light_sync_server.as_ref()?;

        let request: LightSyncMessage = bincode::deserialize(&msg.payload).ok()?;

        let response = match request {
            LightSyncMessage::GetFlyClientProof {
                security_param,
                request_id,
            } => server
                .read()
                .await
                .handle_get_flyclient_proof(security_param, request_id),
            LightSyncMessage::GetMMRProof {
                block_height,
                request_id,
            } => server
                .read()
                .await
                .handle_get_mmr_proof(block_height, request_id),
            _ => return None,
        };

        let payload = bincode::serialize(&response).ok()?;
        *self.requests_served.write().await += 1;

        Some(NodeMessage::new(
            MessageType::FlyClientProofResponse,
            self.current_type().await,
            payload,
        ))
    }

    /// Handle status request
    async fn handle_status_request(&self, _msg: &NodeMessage) -> Option<NodeMessage> {
        let status = self.get_status().await;
        let payload = bincode::serialize(&status).ok()?;

        Some(NodeMessage::new(
            MessageType::StatusResponse,
            self.current_type().await,
            payload,
        ))
    }

    /// Update classification with new block
    pub async fn on_block_validated(&self, block: &Block, validation_time_ms: u64) {
        let mut classification = self.classification.write().await;

        // Record validation
        classification.record_block_validated(validation_time_ms);
        classification.record_block_stored();
        classification.update_chain_height(block.header.height);

        // Update LightSync server if we have one
        if let Some(ref server) = self.light_sync_server {
            server.write().await.add_header(block.header.clone());
        }

        // Check for reclassification
        if let Some(result) = classification.maybe_reclassify(block.header.height) {
            // Update capabilities for new type
            let new_caps = NetworkCapabilities::for_node_type(result.node_type);
            *self.capabilities.write().await = new_caps;
            *self.last_capability_update.write().await = Instant::now();

            // Broadcast classification change
            let _ = self.classification_broadcast.send(result.clone());

            tracing::info!(
                "🔄 Node reclassified: {} -> {} (confidence: {:.1}%)",
                self.target_type,
                result.node_type,
                result.confidence * 100.0
            );
        }
    }

    /// Update with block propagated
    pub async fn on_block_propagated(&self) {
        self.classification.write().await.record_block_propagated();
        crate::metrics::record_block_propagated();
    }

    /// Update with data served
    pub async fn on_data_served(&self, bytes: u64) {
        self.classification.write().await.record_data_served(bytes);
        crate::metrics::record_data_served(bytes);
    }

    /// Update with solution submitted
    pub async fn on_solution_submitted(&self, accepted: bool) {
        self.classification
            .write()
            .await
            .record_solution_submitted(accepted);
    }

    /// Update with oracle feed
    pub async fn on_oracle_feed(&self, accurate: bool) {
        self.classification
            .write()
            .await
            .record_oracle_feed(accurate);
        crate::metrics::record_oracle_feed();
    }

    /// Update peer count
    pub async fn on_peer_count_change(&self, count: usize) {
        self.classification.write().await.update_peer_count(count);
    }

    /// Verify FlyClient proof (for Light nodes)
    pub async fn verify_flyclient_proof(&self, proof: &FlyClientProof) -> Result<(), String> {
        let verifier = self
            .flyclient_verifier
            .as_ref()
            .ok_or("Not a Light node - FlyClient verification not available")?;

        let result = verifier
            .write()
            .await
            .verify_and_update(proof)
            .map_err(|e| e.to_string())?;

        if result.valid {
            tracing::info!(
                "✅ FlyClient proof verified: height={}, samples={}",
                result.new_tip_height,
                result.samples_verified
            );
            Ok(())
        } else {
            Err("FlyClient proof verification failed".to_string())
        }
    }

    // =========================================================================
    // LIGHT SYNC SERVER METHODS (for serving Light clients)
    // =========================================================================

    /// Generate a FlyClient proof for the current chain
    /// Returns serialized proof bytes ready for network transmission
    pub async fn generate_flyclient_proof(&self, security_param: usize) -> Option<Vec<u8>> {
        let server = self.light_sync_server.as_ref()?;
        let proof = server.read().await.generate_flyclient_proof(security_param);
        bincode::serialize(&proof).ok()
    }

    /// Generate an MMR inclusion proof for a specific block height
    /// Returns (header, proof_bytes, mmr_root)
    pub async fn generate_mmr_proof(
        &self,
        block_height: u64,
    ) -> Option<(BlockHeader, Vec<u8>, Hash)> {
        let server = self.light_sync_server.as_ref()?;
        let server_guard = server.read().await;

        let proof = server_guard.generate_mmr_proof(block_height)?;
        let header = server_guard.get_header(block_height)?;
        let mmr_root = server_guard.mmr_root();

        let proof_bytes = bincode::serialize(&proof).ok()?;
        Some((header, proof_bytes, mmr_root))
    }

    /// Get headers in a range (for standard SPV sync)
    pub async fn get_headers(&self, start_height: u64, max_headers: u64) -> Vec<BlockHeader> {
        let server = match self.light_sync_server.as_ref() {
            Some(s) => s,
            None => return Vec::new(),
        };

        let server_guard = server.read().await;
        let end_height = start_height + max_headers;

        (start_height..end_height)
            .filter_map(|h| server_guard.get_header(h))
            .collect()
    }

    /// Get current MMR root
    pub async fn get_mmr_root(&self) -> Option<Hash> {
        let server = self.light_sync_server.as_ref()?;
        Some(server.read().await.mmr_root())
    }

    /// Get total accumulated work
    pub async fn get_total_work(&self) -> Option<u128> {
        let server = self.light_sync_server.as_ref()?;
        Some(server.read().await.total_work())
    }

    /// Get current chain height from LightSyncServer
    pub async fn get_light_sync_height(&self) -> Option<u64> {
        let server = self.light_sync_server.as_ref()?;
        Some(server.read().await.chain_height())
    }

    /// Check if this node can serve Light clients
    pub fn can_serve_light_clients(&self) -> bool {
        self.light_sync_server.is_some()
    }

    /// Get comprehensive status
    pub async fn get_status(&self) -> NodeManagerStatus {
        let classification = self.classification.read().await;
        let capabilities = self.capabilities.read().await.clone();
        let uptime = self.start_time.elapsed();

        NodeManagerStatus {
            current_type: classification.current_type(),
            target_type: self.target_type,
            classification_status: classification.status(),
            capabilities,
            uptime_seconds: uptime.as_secs(),
            requests_served: *self.requests_served.read().await,
            requests_rejected: *self.requests_rejected.read().await,
            messages_handled: self.messages_handled.read().await.clone(),
            is_light_sync_ready: self.light_sync_server.is_some(),
            is_flyclient_verifier_ready: self.flyclient_verifier.is_some(),
        }
    }

    /// Get reward multiplier for current type
    pub async fn reward_multiplier(&self) -> f64 {
        self.current_type().await.reward_multiplier()
    }

    /// Send outbound message
    pub fn send_message(&self, msg: NodeMessage) -> Result<(), String> {
        self.outbound_tx
            .send(msg)
            .map_err(|e| format!("Failed to send message: {}", e))
    }

    /// Subscribe to classification changes
    pub fn subscribe_classification_changes(&self) -> broadcast::Receiver<ClassificationResult> {
        self.classification_broadcast.subscribe()
    }
}

/// Status of the Node Type Manager
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeManagerStatus {
    pub current_type: NodeType,
    pub target_type: NodeType,
    pub classification_status: NodeTypeStatus,
    pub capabilities: NetworkCapabilities,
    pub uptime_seconds: u64,
    pub requests_served: u64,
    pub requests_rejected: u64,
    pub messages_handled: HashMap<MessageType, u64>,
    pub is_light_sync_ready: bool,
    pub is_flyclient_verifier_ready: bool,
}

// =============================================================================
// Capability-Based Routing
// =============================================================================

/// Routes messages to capable peers
pub struct CapabilityRouter {
    /// Known peer capabilities
    peer_capabilities: Arc<RwLock<HashMap<String, (NodeType, NetworkCapabilities)>>>,
}

impl CapabilityRouter {
    pub fn new() -> Self {
        CapabilityRouter {
            peer_capabilities: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a peer's capabilities
    pub async fn register_peer(&self, peer_id: String, node_type: NodeType) {
        let caps = NetworkCapabilities::for_node_type(node_type);
        self.peer_capabilities
            .write()
            .await
            .insert(peer_id, (node_type, caps));
    }

    /// Remove a peer
    pub async fn remove_peer(&self, peer_id: &str) {
        self.peer_capabilities.write().await.remove(peer_id);
    }

    /// Find peers that can handle a request type
    pub async fn find_capable_peers(&self, request_type: &RequestType) -> Vec<String> {
        self.peer_capabilities
            .read()
            .await
            .iter()
            .filter(|(_, (_, caps))| caps.can_handle(request_type))
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Find best peer for a request (considering capabilities and load)
    pub async fn find_best_peer(&self, request_type: &RequestType) -> Option<String> {
        let capable = self.find_capable_peers(request_type).await;
        // Simple: return first capable peer
        // TODO: Add load balancing, latency consideration
        capable.into_iter().next()
    }

    /// Get peers by type
    pub async fn get_peers_by_type(&self, node_type: NodeType) -> Vec<String> {
        self.peer_capabilities
            .read()
            .await
            .iter()
            .filter(|(_, (t, _))| *t == node_type)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Get Archive nodes (for historical data)
    pub async fn get_archive_peers(&self) -> Vec<String> {
        self.get_peers_by_type(NodeType::Archive).await
    }

    /// Get Validator nodes (for block announcements)
    pub async fn get_validator_peers(&self) -> Vec<String> {
        self.get_peers_by_type(NodeType::Validator).await
    }

    /// Get Oracle nodes (for data feeds)
    pub async fn get_oracle_peers(&self) -> Vec<String> {
        self.get_peers_by_type(NodeType::Oracle).await
    }
}

impl Default for CapabilityRouter {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use coinject_core::Address;

    fn test_header(height: u64) -> BlockHeader {
        BlockHeader {
            version: 1,
            height,
            prev_hash: Hash::ZERO,
            timestamp: 1000000 + height as i64,
            transactions_root: Hash::ZERO,
            solutions_root: Hash::ZERO,
            commitment: coinject_core::Commitment {
                hash: Hash::ZERO,
                problem_hash: Hash::ZERO,
            },
            work_score: 0.0,
            miner: Address::from_bytes([0u8; 32]),
            nonce: 0,
            solve_time_us: 0,
            verify_time_us: 0,
            time_asymmetry_ratio: 0.0,
            solution_quality: 0.0,
            complexity_weight: 0.0,
            energy_estimate_joules: 0.0,
        }
    }

    #[test]
    fn test_light_capabilities() {
        let caps = NetworkCapabilities::for_node_type(NodeType::Light);

        assert!(!caps.can_produce_blocks);
        assert!(!caps.can_validate_blocks);
        assert!(caps.stores_headers_only);
        assert!(caps.can_header_sync);
        assert!(caps.can_verify_flyclient);
        assert!(!caps.can_serve_flyclient);
    }

    #[test]
    fn test_full_capabilities() {
        let caps = NetworkCapabilities::for_node_type(NodeType::Full);

        assert!(!caps.can_produce_blocks);
        assert!(caps.can_validate_blocks);
        assert!(caps.can_serve_blocks);
        assert!(caps.can_serve_flyclient);
        assert!(caps.stores_pruned);
    }

    #[test]
    fn test_archive_capabilities() {
        let caps = NetworkCapabilities::for_node_type(NodeType::Archive);

        assert!(caps.stores_full_history);
        assert!(caps.can_be_bootstrap);
        assert_eq!(caps.max_blocks_stored, 0); // Unlimited
        assert_eq!(caps.gossip_participation, 100);
    }

    #[test]
    fn test_validator_capabilities() {
        let caps = NetworkCapabilities::for_node_type(NodeType::Validator);

        assert!(caps.can_produce_blocks);
        assert!(caps.can_validate_blocks);
        assert!(caps.maintains_mempool);
        assert!(caps.can_be_bootstrap);
    }

    #[test]
    fn test_bounty_capabilities() {
        let caps = NetworkCapabilities::for_node_type(NodeType::Bounty);

        assert!(caps.can_solve_problems);
        assert_eq!(caps.problem_priority, 100);
        assert!(!caps.can_serve_blocks);
    }

    #[test]
    fn test_oracle_capabilities() {
        let caps = NetworkCapabilities::for_node_type(NodeType::Oracle);

        assert!(caps.can_provide_oracle_data);
        assert!(!caps.oracle_data_types.is_empty());
    }

    #[test]
    fn test_capability_can_handle() {
        let full_caps = NetworkCapabilities::for_node_type(NodeType::Full);
        let light_caps = NetworkCapabilities::for_node_type(NodeType::Light);

        let block_request = RequestType::GetBlocks { from: 0, to: 100 };
        let header_request = RequestType::GetHeaders { from: 0, to: 100 };

        assert!(full_caps.can_handle(&block_request));
        assert!(!light_caps.can_handle(&block_request));

        // Both can handle header requests (Full serves, Light requests)
        assert!(full_caps.can_handle(&header_request));
        assert!(!light_caps.can_handle(&header_request)); // Light can't SERVE
    }

    #[tokio::test]
    async fn test_capability_router() {
        let router = CapabilityRouter::new();

        router
            .register_peer("peer1".to_string(), NodeType::Full)
            .await;
        router
            .register_peer("peer2".to_string(), NodeType::Archive)
            .await;
        router
            .register_peer("peer3".to_string(), NodeType::Light)
            .await;

        let block_request = RequestType::GetBlocks { from: 0, to: 100 };
        let capable = router.find_capable_peers(&block_request).await;

        // Full and Archive can serve blocks, Light cannot
        assert!(capable.contains(&"peer1".to_string()));
        assert!(capable.contains(&"peer2".to_string()));
        assert!(!capable.contains(&"peer3".to_string()));
    }

    #[tokio::test]
    async fn test_node_manager_creation() {
        let header = test_header(0);
        let (manager, _rx, _broadcast_rx) = NodeTypeManager::new(0, NodeType::Full, Some(header));

        assert_eq!(manager.current_type().await, NodeType::Full);
        assert!(manager.light_sync_server.is_some());
        assert!(manager.flyclient_verifier.is_none());
    }

    #[tokio::test]
    async fn test_light_node_manager() {
        let header = test_header(0);
        let (manager, _rx, _broadcast_rx) = NodeTypeManager::new(0, NodeType::Light, Some(header));

        // For Light nodes, current_type() defaults to Full until classification runs
        // But we can verify the target type and that headers_only is set
        assert_eq!(manager.target_type, NodeType::Light);
        assert!(
            manager
                .classification
                .read()
                .await
                .local_metrics
                .headers_only
        );
        assert!(manager.light_sync_server.is_none());
        assert!(manager.flyclient_verifier.is_some());
    }
}
