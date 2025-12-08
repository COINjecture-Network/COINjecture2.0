// =============================================================================
// Light Client Protocol - Header-Only Sync for Mobile & Embedded Devices
// =============================================================================
//
// This module implements the "Light" node type from our 6-node classification system.
// Light nodes sync only block headers (not full blocks), enabling:
// - Minimal storage (< 10GB vs 500GB+ for Full nodes)
// - Fast sync (headers only, ~80 bytes per block vs ~1MB+)
// - Mobile/embedded compatibility
// - SPV-style transaction verification
//
// CRITICAL: Light nodes CANNOT mine or validate full blocks.
// They rely on Full/Archive nodes for transaction proofs.
//
// INTEGRATION: This module works with light_sync.rs for FlyClient/MMR proofs
// and mobile_sdk.rs for WASM/mobile compilation.
//
// Sync Modes:
// 1. Full Header Sync: Download all headers (traditional SPV)
// 2. FlyClient Sync: Probabilistic verification with O(log n) proofs (via light_sync.rs)

use coinject_core::{Block, BlockHeader, Hash};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

// Re-export FlyClient types for convenience
pub use crate::light_sync::{
    FlyClientProof, FlyClientError, LightSyncMessage, LightSyncServer,
    LightClientVerifier, MerkleMountainRange, MMRInclusionProof,
    SampledBlock, VerificationResult, FLYCLIENT_SECURITY_PARAM,
};

// =============================================================================
// Constants
// =============================================================================

/// Maximum headers to request in a single batch
pub const MAX_HEADERS_PER_REQUEST: u64 = 2000;

/// Maximum headers to store before pruning old ones
pub const MAX_STORED_HEADERS: u64 = 1_000_000;

/// Minimum confirmations for SPV proof
pub const SPV_MIN_CONFIRMATIONS: u64 = 6;

/// Header request timeout (seconds)
pub const HEADER_REQUEST_TIMEOUT_SECS: u64 = 30;

// =============================================================================
// Light Client State
// =============================================================================

/// Sync mode for light clients
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LightSyncMode {
    /// Traditional SPV: download all headers
    FullHeaders,
    /// FlyClient: probabilistic verification with O(log n) proofs
    FlyClient,
    /// Hybrid: FlyClient for initial sync, then full headers
    Hybrid,
}

impl Default for LightSyncMode {
    fn default() -> Self {
        LightSyncMode::Hybrid
    }
}

/// Lightweight header storage for Light nodes
#[derive(Debug)]
pub struct LightClientState {
    /// Block headers indexed by hash
    headers_by_hash: Arc<RwLock<HashMap<Hash, BlockHeader>>>,
    /// Block headers indexed by height
    headers_by_height: Arc<RwLock<HashMap<u64, Hash>>>,
    /// Best (highest) header
    best_header: Arc<RwLock<Option<BlockHeader>>>,
    /// Genesis hash
    genesis_hash: Hash,
    /// Total headers synced
    headers_synced: Arc<RwLock<u64>>,
    /// Sync progress (0.0 - 1.0)
    sync_progress: Arc<RwLock<f64>>,
    /// Current sync mode
    sync_mode: LightSyncMode,
    /// FlyClient verifier for probabilistic sync
    flyclient_verifier: Arc<RwLock<Option<LightClientVerifier>>>,
    /// Verified MMR root (from FlyClient)
    verified_mmr_root: Arc<RwLock<Option<Hash>>>,
}

impl LightClientState {
    /// Create new light client state
    pub fn new(genesis_hash: Hash, genesis_header: BlockHeader) -> Self {
        Self::with_mode(genesis_hash, genesis_header, LightSyncMode::default())
    }
    
    /// Create with specific sync mode
    pub fn with_mode(genesis_hash: Hash, genesis_header: BlockHeader, sync_mode: LightSyncMode) -> Self {
        let mut headers_by_hash = HashMap::new();
        let mut headers_by_height = HashMap::new();
        
        headers_by_hash.insert(genesis_hash, genesis_header.clone());
        headers_by_height.insert(0, genesis_hash);
        
        // Initialize FlyClient verifier if using FlyClient mode
        let flyclient_verifier = if matches!(sync_mode, LightSyncMode::FlyClient | LightSyncMode::Hybrid) {
            Some(LightClientVerifier::new(genesis_hash))
        } else {
            None
        };
        
        LightClientState {
            headers_by_hash: Arc::new(RwLock::new(headers_by_hash)),
            headers_by_height: Arc::new(RwLock::new(headers_by_height)),
            best_header: Arc::new(RwLock::new(Some(genesis_header))),
            genesis_hash,
            headers_synced: Arc::new(RwLock::new(1)),
            sync_progress: Arc::new(RwLock::new(0.0)),
            sync_mode,
            flyclient_verifier: Arc::new(RwLock::new(flyclient_verifier)),
            verified_mmr_root: Arc::new(RwLock::new(None)),
        }
    }
    
    /// Get sync mode
    pub fn sync_mode(&self) -> LightSyncMode {
        self.sync_mode
    }
    
    /// Verify and apply a FlyClient proof
    /// This is the fast-sync path: O(log n) instead of O(n)
    pub async fn verify_flyclient_proof(&self, proof: &FlyClientProof) -> Result<VerificationResult, FlyClientError> {
        let mut verifier_guard = self.flyclient_verifier.write().await;
        let verifier = verifier_guard.as_mut()
            .ok_or(FlyClientError::VerificationFailed("FlyClient not enabled".into()))?;
        
        let result = verifier.verify_and_update(proof)?;
        
        if result.valid {
            // Update our verified MMR root
            *self.verified_mmr_root.write().await = Some(proof.mmr_root);
            
            // Update best header from proof
            *self.best_header.write().await = Some(proof.tip_header.clone());
            
            // Update sync progress
            *self.sync_progress.write().await = 1.0; // FlyClient proof = instant sync
            
            tracing::info!(
                "✅ FlyClient proof verified: height={}, samples={}, proof_size={}KB",
                result.new_tip_height,
                result.samples_verified,
                result.proof_size_bytes / 1024
            );
        }
        
        Ok(result)
    }
    
    /// Verify a block is in the chain using MMR proof
    pub async fn verify_block_mmr(&self, proof: &MMRInclusionProof) -> Result<bool, LightClientError> {
        let mmr_root = self.verified_mmr_root.read().await
            .ok_or(LightClientError::ValidationFailed)?;
        
        Ok(proof.verify(&mmr_root))
    }
    
    /// Get verified MMR root
    pub async fn mmr_root(&self) -> Option<Hash> {
        *self.verified_mmr_root.read().await
    }
    
    /// Check if FlyClient sync is complete
    pub async fn is_flyclient_synced(&self) -> bool {
        self.verified_mmr_root.read().await.is_some()
    }
    
    /// Get the best header height
    pub async fn best_height(&self) -> u64 {
        self.best_header
            .read()
            .await
            .as_ref()
            .map(|h| h.height)
            .unwrap_or(0)
    }
    
    /// Get the best header hash
    pub async fn best_hash(&self) -> Hash {
        self.best_header
            .read()
            .await
            .as_ref()
            .map(|h| h.hash())
            .unwrap_or(self.genesis_hash)
    }
    
    /// Get header by hash
    pub async fn get_header(&self, hash: &Hash) -> Option<BlockHeader> {
        self.headers_by_hash.read().await.get(hash).cloned()
    }
    
    /// Get header by height
    pub async fn get_header_at_height(&self, height: u64) -> Option<BlockHeader> {
        let hash = self.headers_by_height.read().await.get(&height).cloned()?;
        self.get_header(&hash).await
    }
    
    /// Store a new header
    pub async fn store_header(&self, header: BlockHeader) -> Result<bool, LightClientError> {
        let hash = header.hash();
        
        // Validate parent exists (except for genesis)
        if header.height > 0 {
            let parent_exists = self.headers_by_hash
                .read()
                .await
                .contains_key(&header.prev_hash);
            
            if !parent_exists {
                return Err(LightClientError::MissingParent(header.prev_hash));
            }
        }
        
        // Store header
        let mut by_hash = self.headers_by_hash.write().await;
        let mut by_height = self.headers_by_height.write().await;
        
        // Check if already have this header
        if by_hash.contains_key(&hash) {
            return Ok(false); // Already have it
        }
        
        by_hash.insert(hash, header.clone());
        by_height.insert(header.height, hash);
        
        // Update best header if this is higher
        let mut best = self.best_header.write().await;
        if best.as_ref().map(|b| header.height > b.height).unwrap_or(true) {
            *best = Some(header);
        }
        
        // Update sync count
        let mut count = self.headers_synced.write().await;
        *count += 1;
        
        Ok(true)
    }
    
    /// Store multiple headers (batch operation)
    pub async fn store_headers(&self, headers: Vec<BlockHeader>) -> Result<u64, LightClientError> {
        let mut stored = 0u64;
        
        for header in headers {
            if self.store_header(header).await? {
                stored += 1;
            }
        }
        
        Ok(stored)
    }
    
    /// Validate header chain from height A to B
    pub async fn validate_header_chain(&self, from: u64, to: u64) -> Result<bool, LightClientError> {
        if from > to {
            return Err(LightClientError::InvalidRange);
        }
        
        let by_height = self.headers_by_height.read().await;
        let by_hash = self.headers_by_hash.read().await;
        
        for height in from..=to {
            let hash = by_height.get(&height)
                .ok_or(LightClientError::MissingHeader(height))?;
            let header = by_hash.get(hash)
                .ok_or(LightClientError::MissingHeader(height))?;
            
            // Validate parent link (except genesis)
            if height > 0 {
                let expected_parent = by_height.get(&(height - 1))
                    .ok_or(LightClientError::MissingHeader(height - 1))?;
                
                if &header.prev_hash != expected_parent {
                    return Ok(false); // Chain broken
                }
            }
        }
        
        Ok(true)
    }
    
    /// Get headers synced count
    pub async fn headers_synced(&self) -> u64 {
        *self.headers_synced.read().await
    }
    
    /// Update sync progress
    pub async fn update_sync_progress(&self, network_height: u64) {
        let local_height = self.best_height().await;
        let progress = if network_height > 0 {
            (local_height as f64) / (network_height as f64)
        } else {
            1.0
        };
        *self.sync_progress.write().await = progress.min(1.0);
    }
    
    /// Get sync progress (0.0 - 1.0)
    pub async fn sync_progress(&self) -> f64 {
        *self.sync_progress.read().await
    }
    
    /// Prune old headers (keep recent N headers)
    pub async fn prune_old_headers(&self, keep_recent: u64) -> u64 {
        let best_height = self.best_height().await;
        if best_height <= keep_recent {
            return 0;
        }
        
        let prune_below = best_height - keep_recent;
        let mut pruned = 0u64;
        
        let mut by_hash = self.headers_by_hash.write().await;
        let mut by_height = self.headers_by_height.write().await;
        
        let heights_to_remove: Vec<u64> = by_height
            .keys()
            .filter(|&&h| h < prune_below && h != 0) // Never prune genesis
            .cloned()
            .collect();
        
        for height in heights_to_remove {
            if let Some(hash) = by_height.remove(&height) {
                by_hash.remove(&hash);
                pruned += 1;
            }
        }
        
        pruned
    }
    
    /// Get status summary
    pub async fn status(&self) -> LightClientStatus {
        LightClientStatus {
            best_height: self.best_height().await,
            best_hash: self.best_hash().await,
            headers_synced: self.headers_synced().await,
            sync_progress: self.sync_progress().await,
            genesis_hash: self.genesis_hash,
        }
    }
}

// =============================================================================
// Protocol Messages
// =============================================================================

/// Messages for Light Client protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LightClientMessage {
    /// Request headers from a range
    GetHeaders {
        /// Starting height (inclusive)
        from_height: u64,
        /// Number of headers to request
        count: u64,
        /// Request ID for correlation
        request_id: u64,
    },
    
    /// Response with headers
    Headers {
        /// The requested headers
        headers: Vec<BlockHeader>,
        /// Request ID for correlation
        request_id: u64,
        /// Whether there are more headers available
        has_more: bool,
    },
    
    /// Request header by hash
    GetHeaderByHash {
        hash: Hash,
        request_id: u64,
    },
    
    /// Single header response
    Header {
        header: Option<BlockHeader>,
        request_id: u64,
    },
    
    /// Request SPV proof for a transaction
    GetSPVProof {
        /// Transaction hash
        tx_hash: Hash,
        /// Block hash containing the transaction
        block_hash: Hash,
        request_id: u64,
    },
    
    /// SPV proof response
    SPVProof {
        /// Merkle proof path
        proof: Option<MerkleProof>,
        request_id: u64,
    },
    
    /// Announce new header (push notification)
    NewHeader {
        header: BlockHeader,
    },
    
    /// Request current tip
    GetTip {
        request_id: u64,
    },
    
    /// Tip response
    Tip {
        height: u64,
        hash: Hash,
        request_id: u64,
    },
}

/// Merkle proof for SPV verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleProof {
    /// The transaction hash being proven
    pub tx_hash: Hash,
    /// Merkle path (hashes and directions)
    pub path: Vec<(Hash, bool)>, // (sibling_hash, is_left)
    /// Block header containing the transaction
    pub block_header: BlockHeader,
    /// Transaction index in block
    pub tx_index: u32,
}

impl MerkleProof {
    /// Verify this proof against a transaction hash and merkle root
    pub fn verify(&self, tx_hash: &Hash, merkle_root: &Hash) -> bool {
        let mut current = *tx_hash;
        
        for (sibling, is_left) in &self.path {
            current = if *is_left {
                // Sibling is on the left
                hash_pair(sibling, &current)
            } else {
                // Sibling is on the right
                hash_pair(&current, sibling)
            };
        }
        
        &current == merkle_root
    }
}

/// Hash two hashes together (for Merkle tree)
fn hash_pair(left: &Hash, right: &Hash) -> Hash {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(left.as_bytes());
    hasher.update(right.as_bytes());
    let result = hasher.finalize();
    Hash::from_bytes(result.into())
}

// =============================================================================
// Light Client Status
// =============================================================================

/// Status of light client sync
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightClientStatus {
    pub best_height: u64,
    pub best_hash: Hash,
    pub headers_synced: u64,
    pub sync_progress: f64,
    pub genesis_hash: Hash,
}

// =============================================================================
// Errors
// =============================================================================

#[derive(Debug, thiserror::Error)]
pub enum LightClientError {
    #[error("Missing parent header: {0:?}")]
    MissingParent(Hash),
    
    #[error("Missing header at height: {0}")]
    MissingHeader(u64),
    
    #[error("Invalid height range")]
    InvalidRange,
    
    #[error("Header validation failed")]
    ValidationFailed,
    
    #[error("SPV proof invalid")]
    InvalidProof,
    
    #[error("Network error: {0}")]
    NetworkError(String),
    
    #[error("Timeout waiting for headers")]
    Timeout,
}

// =============================================================================
// Light Client Sync Manager
// =============================================================================

/// Manager for syncing headers from Full/Archive nodes
#[derive(Debug)]
pub struct LightClientSync {
    /// Light client state
    state: Arc<LightClientState>,
    /// Pending header requests
    pending_requests: Arc<RwLock<HashMap<u64, HeaderRequest>>>,
    /// Next request ID
    next_request_id: Arc<RwLock<u64>>,
    /// Full node peers serving us
    full_node_peers: Arc<RwLock<Vec<String>>>,
}

/// Pending header request
#[derive(Debug, Clone)]
pub struct HeaderRequest {
    pub from_height: u64,
    pub count: u64,
    pub requested_at: std::time::Instant,
    pub peer_id: String,
}

impl LightClientSync {
    /// Create new sync manager
    pub fn new(state: Arc<LightClientState>) -> Self {
        LightClientSync {
            state,
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
            next_request_id: Arc::new(RwLock::new(1)),
            full_node_peers: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    /// Add a full node peer
    pub async fn add_full_node_peer(&self, peer_id: String) {
        let mut peers = self.full_node_peers.write().await;
        if !peers.contains(&peer_id) {
            peers.push(peer_id);
        }
    }
    
    /// Remove a full node peer
    pub async fn remove_full_node_peer(&self, peer_id: &str) {
        let mut peers = self.full_node_peers.write().await;
        peers.retain(|p| p != peer_id);
    }
    
    /// Get count of full node peers
    pub async fn full_node_peer_count(&self) -> usize {
        self.full_node_peers.read().await.len()
    }
    
    /// Create a header request message
    pub async fn create_header_request(&self, from_height: u64, count: u64, peer_id: String) -> LightClientMessage {
        let mut next_id = self.next_request_id.write().await;
        let request_id = *next_id;
        *next_id += 1;
        
        let request = HeaderRequest {
            from_height,
            count: count.min(MAX_HEADERS_PER_REQUEST),
            requested_at: std::time::Instant::now(),
            peer_id,
        };
        
        self.pending_requests.write().await.insert(request_id, request);
        
        LightClientMessage::GetHeaders {
            from_height,
            count: count.min(MAX_HEADERS_PER_REQUEST),
            request_id,
        }
    }
    
    /// Handle received headers
    pub async fn handle_headers(&self, headers: Vec<BlockHeader>, request_id: u64) -> Result<u64, LightClientError> {
        // Remove from pending
        self.pending_requests.write().await.remove(&request_id);
        
        // Store headers
        self.state.store_headers(headers).await
    }
    
    /// Handle new header announcement
    pub async fn handle_new_header(&self, header: BlockHeader) -> Result<bool, LightClientError> {
        self.state.store_header(header).await
    }
    
    /// Get next headers to request for sync
    pub async fn get_sync_request(&self, network_height: u64) -> Option<(u64, u64)> {
        let local_height = self.state.best_height().await;
        
        if local_height >= network_height {
            return None; // Already synced
        }
        
        let from = local_height + 1;
        let count = (network_height - local_height).min(MAX_HEADERS_PER_REQUEST);
        
        Some((from, count))
    }
    
    /// Clean up timed out requests
    pub async fn cleanup_timed_out_requests(&self) -> Vec<u64> {
        let timeout = std::time::Duration::from_secs(HEADER_REQUEST_TIMEOUT_SECS);
        let mut pending = self.pending_requests.write().await;
        
        let timed_out: Vec<u64> = pending
            .iter()
            .filter(|(_, req)| req.requested_at.elapsed() > timeout)
            .map(|(id, _)| *id)
            .collect();
        
        for id in &timed_out {
            pending.remove(id);
        }
        
        timed_out
    }
    
    /// Get sync status
    pub async fn status(&self) -> LightClientStatus {
        self.state.status().await
    }
}

// =============================================================================
// Full Node Header Server
// =============================================================================

/// Server component for Full/Archive nodes to serve Light clients
pub struct HeaderServer {
    /// Function to get header by height (provided by chain)
    get_header_fn: Box<dyn Fn(u64) -> Option<BlockHeader> + Send + Sync>,
    /// Function to get block for merkle proof
    get_block_fn: Box<dyn Fn(&Hash) -> Option<Block> + Send + Sync>,
    /// Current best height
    best_height: Arc<RwLock<u64>>,
}

impl std::fmt::Debug for HeaderServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HeaderServer")
            .field("best_height", &self.best_height)
            .finish_non_exhaustive()
    }
}

impl HeaderServer {
    /// Create new header server
    pub fn new<F, G>(get_header: F, get_block: G) -> Self 
    where
        F: Fn(u64) -> Option<BlockHeader> + Send + Sync + 'static,
        G: Fn(&Hash) -> Option<Block> + Send + Sync + 'static,
    {
        HeaderServer {
            get_header_fn: Box::new(get_header),
            get_block_fn: Box::new(get_block),
            best_height: Arc::new(RwLock::new(0)),
        }
    }
    
    /// Update best height
    pub async fn update_best_height(&self, height: u64) {
        *self.best_height.write().await = height;
    }
    
    /// Handle GetHeaders request
    pub async fn handle_get_headers(&self, from_height: u64, count: u64, request_id: u64) -> LightClientMessage {
        let count = count.min(MAX_HEADERS_PER_REQUEST);
        let best = *self.best_height.read().await;
        
        let mut headers = Vec::with_capacity(count as usize);
        
        for height in from_height..(from_height + count) {
            if height > best {
                break;
            }
            if let Some(header) = (self.get_header_fn)(height) {
                headers.push(header);
            } else {
                break;
            }
        }
        
        let has_more = from_height + count <= best;
        
        LightClientMessage::Headers {
            headers,
            request_id,
            has_more,
        }
    }
    
    /// Handle GetHeaderByHash request
    pub async fn handle_get_header_by_hash(&self, _hash: Hash, request_id: u64) -> LightClientMessage {
        // This would need a hash-to-height index in practice
        // For now, return None
        LightClientMessage::Header {
            header: None,
            request_id,
        }
    }
    
    /// Handle GetTip request
    pub async fn handle_get_tip(&self, request_id: u64) -> LightClientMessage {
        let height = *self.best_height.read().await;
        let hash = (self.get_header_fn)(height)
            .map(|h| h.hash())
            .unwrap_or(Hash::ZERO);
        
        LightClientMessage::Tip {
            height,
            hash,
            request_id,
        }
    }
    
    /// Handle GetSPVProof request
    pub async fn handle_get_spv_proof(&self, tx_hash: Hash, block_hash: Hash, request_id: u64) -> LightClientMessage {
        let proof = if let Some(block) = (self.get_block_fn)(&block_hash) {
            // Find transaction in block and build merkle proof
            build_merkle_proof(&block, &tx_hash)
        } else {
            None
        };
        
        LightClientMessage::SPVProof { proof, request_id }
    }
}

/// Build a merkle proof for a transaction in a block
fn build_merkle_proof(block: &Block, tx_hash: &Hash) -> Option<MerkleProof> {
    // Find transaction index
    let tx_index = block.transactions.iter()
        .position(|tx| tx.hash() == *tx_hash)?;
    
    // Build merkle tree and extract path
    let tx_hashes: Vec<Hash> = block.transactions.iter()
        .map(|tx| tx.hash())
        .collect();
    
    if tx_hashes.is_empty() {
        return None;
    }
    
    let path = build_merkle_path(&tx_hashes, tx_index);
    
    Some(MerkleProof {
        tx_hash: *tx_hash,
        path,
        block_header: block.header.clone(),
        tx_index: tx_index as u32,
    })
}

/// Build merkle path for proof
fn build_merkle_path(leaves: &[Hash], index: usize) -> Vec<(Hash, bool)> {
    if leaves.len() <= 1 {
        return Vec::new();
    }
    
    let mut path = Vec::new();
    let mut current_level = leaves.to_vec();
    let mut current_index = index;
    
    while current_level.len() > 1 {
        // Pad to even length if needed
        if current_level.len() % 2 != 0 {
            current_level.push(*current_level.last().unwrap());
        }
        
        // Find sibling
        let sibling_index = if current_index % 2 == 0 {
            current_index + 1
        } else {
            current_index - 1
        };
        
        let is_left = current_index % 2 != 0;
        path.push((current_level[sibling_index], is_left));
        
        // Build next level
        let mut next_level = Vec::new();
        for i in (0..current_level.len()).step_by(2) {
            next_level.push(hash_pair(&current_level[i], &current_level[i + 1]));
        }
        
        current_level = next_level;
        current_index /= 2;
    }
    
    path
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use coinject_core::{Address, Commitment};
    
    fn test_header(height: u64, parent: Hash) -> BlockHeader {
        BlockHeader {
            version: 1,
            height,
            prev_hash: parent,
            timestamp: (1000000 + height) as i64,
            transactions_root: Hash::ZERO,
            solutions_root: Hash::ZERO,
            commitment: Commitment {
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
    
    #[tokio::test]
    async fn test_light_client_store_headers() {
        let genesis = test_header(0, Hash::ZERO);
        let genesis_hash = genesis.hash();
        
        let state = LightClientState::new(genesis_hash, genesis.clone());
        
        // Store next header
        let header1 = test_header(1, genesis_hash);
        let stored = state.store_header(header1.clone()).await.unwrap();
        assert!(stored);
        
        // Check best height
        assert_eq!(state.best_height().await, 1);
        
        // Store duplicate should return false
        let stored = state.store_header(header1).await.unwrap();
        assert!(!stored);
    }
    
    #[tokio::test]
    async fn test_light_client_validate_chain() {
        let genesis = test_header(0, Hash::ZERO);
        let genesis_hash = genesis.hash();
        
        let state = LightClientState::new(genesis_hash, genesis);
        
        // Build chain of 5 headers
        let mut parent = genesis_hash;
        for height in 1..=5 {
            let header = test_header(height, parent);
            parent = header.hash();
            state.store_header(header).await.unwrap();
        }
        
        // Validate chain
        let valid = state.validate_header_chain(0, 5).await.unwrap();
        assert!(valid);
    }
    
    #[tokio::test]
    async fn test_light_client_missing_parent() {
        let genesis = test_header(0, Hash::ZERO);
        let genesis_hash = genesis.hash();
        
        let state = LightClientState::new(genesis_hash, genesis);
        
        // Try to store header with missing parent
        let bad_header = test_header(2, Hash::from_bytes([1u8; 32])); // Wrong parent
        let result = state.store_header(bad_header).await;
        assert!(result.is_err());
    }
    
    #[test]
    fn test_merkle_proof_verify() {
        let tx_hash = Hash::from_bytes([1u8; 32]);
        let sibling = Hash::from_bytes([2u8; 32]);
        let merkle_root = hash_pair(&tx_hash, &sibling);
        
        let proof = MerkleProof {
            tx_hash,
            path: vec![(sibling, false)], // Sibling on right
            block_header: test_header(1, Hash::ZERO),
            tx_index: 0,
        };
        
        assert!(proof.verify(&tx_hash, &merkle_root));
    }
}


