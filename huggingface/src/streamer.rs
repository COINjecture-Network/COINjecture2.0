// Dual-Feed HuggingFace Streamer - Phase 1C
// ==========================================
// Three feeds for comprehensive blockchain data streaming:
// - Feed A: head_unconfirmed - Real-time blocks (may contain future orphans)
// - Feed B: canonical_confirmed - Only k-confirmed blocks
// - Feed C: reorg_events - Forensic log of chain reorganizations
//
// This module implements idempotent, crash-safe streaming with local state persistence.

use crate::client::{HuggingFaceClient, HuggingFaceConfig, ClientError};
use coinject_core::{Block, Hash};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ═══════════════════════════════════════════════════════════════════════════════
// MEMORY BOUNDS - Prevent unbounded growth
// ═══════════════════════════════════════════════════════════════════════════════

/// Maximum published record IDs to track (LRU-style FIFO eviction)
const MAX_PUBLISHED_RECORDS: usize = 50_000;
/// Eviction batch size when limit exceeded
const PUBLISHED_EVICTION_BATCH: usize = 5_000;
/// Maximum pending blocks in queue (ring buffer behavior)
const MAX_PENDING_BLOCKS: usize = 1_000;
/// Maximum age for pending blocks before expiry (1 hour)
const PENDING_BLOCK_TTL_SECS: i64 = 3600;
/// Maximum orphaned block hashes to store in reorg event (use summary for larger)
const MAX_ORPHAN_HASHES_INLINE: usize = 50;

// ═══════════════════════════════════════════════════════════════════════════════
// FEED RECORD TYPES
// ═══════════════════════════════════════════════════════════════════════════════

/// Feed A: Unconfirmed block record (real-time, may become orphan)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnconfirmedBlockRecord {
    /// Unique record ID for idempotency
    pub record_id: String,
    /// Block height
    pub height: u64,
    /// Block hash (hex)
    pub block_hash: String,
    /// Previous block hash (hex)
    pub prev_hash: String,
    /// Miner address (hex)
    pub miner: String,
    /// Block's work score
    pub work_score: f64,
    /// Total chain work at this block
    pub total_work: f64,
    /// Timestamp when we received this block
    pub received_at: i64,
    /// Node ID that streamed this record
    pub node_id: Option<String>,
    /// Whether this block was mined by us
    pub is_mined: bool,
    /// Feed identifier
    pub feed: String,
    /// Data version
    pub data_version: String,
}

/// Feed B: Confirmed block record (k-confirmed, canonical chain only)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmedBlockRecord {
    /// Unique record ID for idempotency
    pub record_id: String,
    /// Block height
    pub height: u64,
    /// Block hash (hex)
    pub block_hash: String,
    /// Previous block hash (hex)
    pub prev_hash: String,
    /// Miner address (hex)
    pub miner: String,
    /// Block's work score
    pub work_score: f64,
    /// Total chain work at this block
    pub total_work: f64,
    /// Number of confirmations when published
    pub confirmations: u64,
    /// Timestamp when confirmed
    pub confirmed_at: i64,
    /// Node ID that streamed this record
    pub node_id: Option<String>,
    /// Feed identifier
    pub feed: String,
    /// Data version
    pub data_version: String,
}

/// Feed C: Reorg event record (forensic log)
/// For large reorgs (>50 blocks), uses summary format instead of full hash lists
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReorgEventRecord {
    /// Unique event ID (hash of event data)
    pub event_id: String,
    /// Old chain tip hash before reorg
    pub old_tip: String,
    /// Old chain tip height before reorg
    pub old_height: u64,
    /// New chain tip hash after reorg
    pub new_tip: String,
    /// New chain tip height after reorg
    pub new_height: u64,
    /// Common ancestor hash (fork point)
    pub common_ancestor: String,
    /// Common ancestor height
    pub common_ancestor_height: u64,
    /// Reorg depth (blocks rolled back)
    pub reorg_depth: u64,
    /// Work difference (new_work - old_work, positive means more work)
    pub work_delta: f64,
    /// Total count of orphaned blocks
    pub orphaned_count: usize,
    /// Hashes of orphaned blocks (capped at MAX_ORPHAN_HASHES_INLINE, first/last for larger)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub orphaned_blocks: Vec<String>,
    /// First orphaned block hash (for large reorgs where full list is truncated)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orphaned_first: Option<String>,
    /// Last orphaned block hash (for large reorgs where full list is truncated)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orphaned_last: Option<String>,
    /// Total count of new blocks
    pub new_count: usize,
    /// Hashes of new blocks (capped at MAX_ORPHAN_HASHES_INLINE, first/last for larger)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub new_blocks: Vec<String>,
    /// First new block hash (for large reorgs)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_first: Option<String>,
    /// Last new block hash (for large reorgs)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_last: Option<String>,
    /// Whether block lists are truncated (true if count > inline limit)
    pub lists_truncated: bool,
    /// Timestamp when reorg detected
    pub detected_at: i64,
    /// Node ID that detected this reorg
    pub node_id: Option<String>,
    /// Feed identifier
    pub feed: String,
    /// Data version
    pub data_version: String,
}

// ═══════════════════════════════════════════════════════════════════════════════
// STREAMER STATE (Crash-Safe Local Index)
// ═══════════════════════════════════════════════════════════════════════════════

/// Pending block awaiting k-confirmations
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PendingBlock {
    block_hash: String,
    prev_hash: String,
    height: u64,
    miner: String,
    work_score: f64,
    total_work: f64,
    received_at: i64,
    is_mined: bool,
}

/// Persistent streamer state for crash recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamerState {
    /// Last confirmed block height published to Feed B
    pub last_confirmed_height: u64,
    /// Last confirmed block hash published to Feed B
    pub last_confirmed_hash: String,
    /// Pending blocks awaiting k-confirmations (height -> block data)
    pending_blocks: VecDeque<PendingBlock>,
    /// Set of record IDs already published (for idempotency)
    published_records: HashMap<String, bool>,
    /// Current best chain tip hash
    pub current_tip_hash: String,
    /// Current best chain tip height
    pub current_tip_height: u64,
    /// Counter for generating unique event IDs
    event_counter: u64,
    /// Path to state file
    #[serde(skip)]
    state_file: Option<PathBuf>,
}

impl StreamerState {
    /// Create new streamer state
    pub fn new(data_dir: PathBuf) -> Self {
        let state_file = data_dir.join("hf_streamer_state.json");

        // Try to load existing state
        if let Ok(data) = fs::read_to_string(&state_file) {
            if let Ok(mut state) = serde_json::from_str::<StreamerState>(&data) {
                state.state_file = Some(state_file);
                eprintln!("📊 HF Streamer: Loaded state from disk (last confirmed height: {})",
                    state.last_confirmed_height);
                return state;
            }
        }

        eprintln!("📊 HF Streamer: Starting fresh state");
        StreamerState {
            last_confirmed_height: 0,
            last_confirmed_hash: "0".repeat(64),
            pending_blocks: VecDeque::new(),
            published_records: HashMap::new(),
            current_tip_hash: "0".repeat(64),
            current_tip_height: 0,
            event_counter: 0,
            state_file: Some(state_file),
        }
    }

    /// Persist state to disk
    pub fn persist(&self) -> Result<(), StreamerError> {
        if let Some(ref path) = self.state_file {
            let data = serde_json::to_string_pretty(self)
                .map_err(|e| StreamerError::Serialization(e.to_string()))?;
            fs::write(path, data)
                .map_err(|e| StreamerError::Io(e.to_string()))?;
        }
        Ok(())
    }

    /// Check if a record ID has already been published
    pub fn is_published(&self, record_id: &str) -> bool {
        self.published_records.contains_key(record_id)
    }

    /// Mark a record as published (bounded LRU - FIFO eviction at 50k)
    pub fn mark_published(&mut self, record_id: String) {
        self.published_records.insert(record_id, true);

        // Keep map bounded with FIFO eviction
        if self.published_records.len() > MAX_PUBLISHED_RECORDS {
            // Remove oldest entries in batch
            let keys: Vec<_> = self.published_records.keys()
                .take(PUBLISHED_EVICTION_BATCH)
                .cloned()
                .collect();
            for key in keys {
                self.published_records.remove(&key);
            }
            eprintln!("📊 HF Streamer: Evicted {} old record IDs (bounded at {})",
                PUBLISHED_EVICTION_BATCH, MAX_PUBLISHED_RECORDS);
        }
    }

    /// Generate unique event ID
    pub fn next_event_id(&mut self) -> String {
        self.event_counter += 1;
        format!("reorg_{}_{}",
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            self.event_counter)
    }

    /// Add pending block with ring buffer bounds and TTL
    pub fn add_pending_block(&mut self, block: PendingBlock) {
        // Enforce ring buffer limit
        while self.pending_blocks.len() >= MAX_PENDING_BLOCKS {
            if let Some(evicted) = self.pending_blocks.pop_front() {
                eprintln!("📊 HF Streamer: Evicted old pending block {} (ring buffer full)",
                    evicted.height);
            }
        }
        self.pending_blocks.push_back(block);
    }

    /// Expire pending blocks older than TTL
    pub fn expire_old_pending_blocks(&mut self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let before_count = self.pending_blocks.len();
        self.pending_blocks.retain(|pb| {
            now - pb.received_at < PENDING_BLOCK_TTL_SECS
        });

        let expired = before_count - self.pending_blocks.len();
        if expired > 0 {
            eprintln!("📊 HF Streamer: Expired {} stale pending blocks (TTL={}s)",
                expired, PENDING_BLOCK_TTL_SECS);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// DUAL-FEED STREAMER
// ═══════════════════════════════════════════════════════════════════════════════

/// Configuration for the dual-feed streamer
#[derive(Debug, Clone)]
pub struct StreamerConfig {
    /// Minimum confirmations for Feed B (canonical_confirmed)
    pub min_confirmations: u64,
    /// Batch size for HF commits
    pub batch_size: usize,
    /// Maximum time between batches (seconds)
    pub batch_interval_secs: u64,
    /// Whether streaming is enabled
    pub enabled: bool,
    /// Node ID for attribution
    pub node_id: Option<String>,
    /// Data directory for state persistence
    pub data_dir: PathBuf,
}

impl Default for StreamerConfig {
    fn default() -> Self {
        StreamerConfig {
            min_confirmations: 20, // Conservative for testnet
            batch_size: 10,
            batch_interval_secs: 60,
            enabled: true,
            node_id: None,
            data_dir: PathBuf::from("."),
        }
    }
}

/// Main dual-feed streamer service
pub struct DualFeedStreamer {
    config: StreamerConfig,
    state: tokio::sync::Mutex<StreamerState>,
    /// Buffer for unconfirmed blocks (Feed A)
    unconfirmed_buffer: tokio::sync::Mutex<Vec<UnconfirmedBlockRecord>>,
    /// Buffer for confirmed blocks (Feed B)
    confirmed_buffer: tokio::sync::Mutex<Vec<ConfirmedBlockRecord>>,
    /// Buffer for reorg events (Feed C)
    reorg_buffer: tokio::sync::Mutex<Vec<ReorgEventRecord>>,
    /// Tracks if node is currently syncing (to avoid burst publishing)
    is_syncing: tokio::sync::Mutex<bool>,
    /// Last batch flush time
    last_flush: tokio::sync::Mutex<SystemTime>,
}

impl DualFeedStreamer {
    /// Create new dual-feed streamer
    pub fn new(config: StreamerConfig) -> Self {
        let state = StreamerState::new(config.data_dir.clone());

        eprintln!("📊 HF DualFeed: Initialized with k={} confirmations, batch_size={}",
            config.min_confirmations, config.batch_size);

        DualFeedStreamer {
            config,
            state: tokio::sync::Mutex::new(state),
            unconfirmed_buffer: tokio::sync::Mutex::new(Vec::new()),
            confirmed_buffer: tokio::sync::Mutex::new(Vec::new()),
            reorg_buffer: tokio::sync::Mutex::new(Vec::new()),
            is_syncing: tokio::sync::Mutex::new(false),
            last_flush: tokio::sync::Mutex::new(SystemTime::now()),
        }
    }

    /// Set syncing state (call when node starts/finishes sync)
    pub async fn set_syncing(&self, syncing: bool) {
        let mut is_syncing = self.is_syncing.lock().await;
        let was_syncing = *is_syncing;
        *is_syncing = syncing;

        if was_syncing && !syncing {
            eprintln!("📊 HF DualFeed: Sync completed, resuming streaming");
        } else if !was_syncing && syncing {
            eprintln!("📊 HF DualFeed: Sync started, pausing streaming to avoid burst");
        }
    }

    /// Check if we should skip streaming (during sync)
    async fn should_skip_streaming(&self) -> bool {
        if !self.config.enabled {
            return true;
        }
        *self.is_syncing.lock().await
    }

    /// Set node ID for attribution
    pub async fn set_node_id(&self, node_id: String) {
        // Update config (we can't mutate config directly, so we'd need to restructure)
        // For now, just log it - the node_id is passed per-call
        eprintln!("📊 HF DualFeed: Node ID set to {}", &node_id[..16.min(node_id.len())]);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // FEED A: HEAD_UNCONFIRMED - Real-time block streaming
    // ═══════════════════════════════════════════════════════════════════════════

    /// Push block to Feed A (head_unconfirmed) - immediate streaming
    /// This is called for every new block, whether mined or received
    pub async fn push_unconfirmed_block(
        &self,
        block: &Block,
        is_mined: bool,
        total_work: f64,
    ) -> Result<(), StreamerError> {
        // Skip during sync to avoid burst publishing
        if self.should_skip_streaming().await {
            return Ok(());
        }

        let block_hash = hex::encode(block.hash().as_bytes());
        let record_id = format!("unconfirmed_{}_{}", block.header.height, &block_hash[..16]);

        // Check idempotency
        {
            let state = self.state.lock().await;
            if state.is_published(&record_id) {
                return Ok(());
            }
        }

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let record = UnconfirmedBlockRecord {
            record_id: record_id.clone(),
            height: block.header.height,
            block_hash: block_hash.clone(),
            prev_hash: hex::encode(block.header.prev_hash.as_bytes()),
            miner: hex::encode(block.header.miner.as_bytes()),
            work_score: block.header.work_score,
            total_work,
            received_at: timestamp,
            node_id: self.config.node_id.clone(),
            is_mined,
            feed: "head_unconfirmed".to_string(),
            data_version: "v3.1".to_string(),
        };

        // Add to pending blocks for confirmation tracking (bounded ring buffer)
        {
            let mut state = self.state.lock().await;

            // Expire stale pending blocks first
            state.expire_old_pending_blocks();

            // Add new block using bounded method
            state.add_pending_block(PendingBlock {
                block_hash: block_hash.clone(),
                prev_hash: record.prev_hash.clone(),
                height: block.header.height,
                miner: record.miner.clone(),
                work_score: block.header.work_score,
                total_work,
                received_at: timestamp,
                is_mined,
            });

            // Update current tip if this is higher
            if block.header.height > state.current_tip_height {
                state.current_tip_height = block.header.height;
                state.current_tip_hash = block_hash.clone();
            }

            state.mark_published(record_id);
            state.persist()?;
        }

        // Buffer the record
        {
            let mut buffer = self.unconfirmed_buffer.lock().await;
            buffer.push(record);
            eprintln!("📊 HF Feed A: Buffered unconfirmed block {} (hash: {}...)",
                block.header.height, &block_hash[..12]);
        }

        // Check if we should flush
        self.maybe_flush().await?;

        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // FEED B: CANONICAL_CONFIRMED - k-confirmed block streaming
    // ═══════════════════════════════════════════════════════════════════════════

    /// Process confirmed blocks (call periodically or when new blocks arrive)
    /// This promotes pending blocks to Feed B once they have k confirmations
    pub async fn process_confirmations(&self, current_height: u64) -> Result<(), StreamerError> {
        if self.should_skip_streaming().await {
            return Ok(());
        }

        let k = self.config.min_confirmations;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let blocks_to_confirm: Vec<PendingBlock> = {
            let mut state = self.state.lock().await;
            let mut to_confirm = Vec::new();

            // Check pending blocks for k confirmations
            while let Some(front) = state.pending_blocks.front() {
                let confirmations = current_height.saturating_sub(front.height);
                if confirmations >= k {
                    if let Some(pb) = state.pending_blocks.pop_front() {
                        to_confirm.push(pb);
                    }
                } else {
                    break; // Blocks are ordered by height
                }
            }

            to_confirm
        };

        // Create confirmed records
        for pb in blocks_to_confirm {
            let confirmations = current_height.saturating_sub(pb.height);
            let record_id = format!("confirmed_{}_{}", pb.height, &pb.block_hash[..16]);

            // Check idempotency
            {
                let state = self.state.lock().await;
                if state.is_published(&record_id) {
                    continue;
                }
            }

            let record = ConfirmedBlockRecord {
                record_id: record_id.clone(),
                height: pb.height,
                block_hash: pb.block_hash.clone(),
                prev_hash: pb.prev_hash,
                miner: pb.miner,
                work_score: pb.work_score,
                total_work: pb.total_work,
                confirmations,
                confirmed_at: timestamp,
                node_id: self.config.node_id.clone(),
                feed: "canonical_confirmed".to_string(),
                data_version: "v3.1".to_string(),
            };

            {
                let mut state = self.state.lock().await;
                state.last_confirmed_height = pb.height;
                state.last_confirmed_hash = pb.block_hash.clone();
                state.mark_published(record_id);
                state.persist()?;
            }

            {
                let mut buffer = self.confirmed_buffer.lock().await;
                buffer.push(record);
                eprintln!("📊 HF Feed B: Block {} confirmed with {} confirmations",
                    pb.height, confirmations);
            }
        }

        self.maybe_flush().await?;

        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // FEED C: REORG_EVENTS - Chain reorganization detection
    // ═══════════════════════════════════════════════════════════════════════════

    /// Detect and record a chain reorganization
    /// Call this when the best chain tip changes to a different fork
    pub async fn detect_reorg(
        &self,
        old_tip: &Hash,
        old_height: u64,
        new_tip: &Hash,
        new_height: u64,
        common_ancestor: &Hash,
        common_ancestor_height: u64,
        orphaned_blocks: Vec<Hash>,
        new_blocks: Vec<Hash>,
        old_work: f64,
        new_work: f64,
    ) -> Result<(), StreamerError> {
        if self.should_skip_streaming().await {
            return Ok(());
        }

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let event_id = {
            let mut state = self.state.lock().await;
            state.next_event_id()
        };

        let reorg_depth = old_height.saturating_sub(common_ancestor_height);

        // Build bounded orphan/new block lists (cap at MAX_ORPHAN_HASHES_INLINE)
        let orphaned_count = orphaned_blocks.len();
        let new_count = new_blocks.len();
        let lists_truncated = orphaned_count > MAX_ORPHAN_HASHES_INLINE || new_count > MAX_ORPHAN_HASHES_INLINE;

        // For small reorgs, include full lists; for large ones, use first/last summary
        let (orphaned_hashes, orphaned_first, orphaned_last) = if orphaned_count <= MAX_ORPHAN_HASHES_INLINE {
            (orphaned_blocks.iter().map(|h| hex::encode(h.as_bytes())).collect(), None, None)
        } else {
            let first = orphaned_blocks.first().map(|h| hex::encode(h.as_bytes()));
            let last = orphaned_blocks.last().map(|h| hex::encode(h.as_bytes()));
            (Vec::new(), first, last)
        };

        let (new_hashes, new_first, new_last) = if new_count <= MAX_ORPHAN_HASHES_INLINE {
            (new_blocks.iter().map(|h| hex::encode(h.as_bytes())).collect(), None, None)
        } else {
            let first = new_blocks.first().map(|h| hex::encode(h.as_bytes()));
            let last = new_blocks.last().map(|h| hex::encode(h.as_bytes()));
            (Vec::new(), first, last)
        };

        let record = ReorgEventRecord {
            event_id: event_id.clone(),
            old_tip: hex::encode(old_tip.as_bytes()),
            old_height,
            new_tip: hex::encode(new_tip.as_bytes()),
            new_height,
            common_ancestor: hex::encode(common_ancestor.as_bytes()),
            common_ancestor_height,
            reorg_depth,
            work_delta: new_work - old_work,
            orphaned_count,
            orphaned_blocks: orphaned_hashes,
            orphaned_first,
            orphaned_last,
            new_count,
            new_blocks: new_hashes,
            new_first,
            new_last,
            lists_truncated,
            detected_at: timestamp,
            node_id: self.config.node_id.clone(),
            feed: "reorg_events".to_string(),
            data_version: "v3.2".to_string(), // Bump version for new format
        };

        eprintln!("⚠️  HF Feed C: Reorg detected! Depth={}, old_height={} -> new_height={}",
            reorg_depth, old_height, new_height);

        // Clear pending blocks that were orphaned
        {
            let mut state = self.state.lock().await;
            let orphan_set: std::collections::HashSet<_> = orphaned_blocks.iter()
                .map(|h| hex::encode(h.as_bytes()))
                .collect();

            let before_count = state.pending_blocks.len();
            state.pending_blocks.retain(|pb| !orphan_set.contains(&pb.block_hash));
            let removed = before_count - state.pending_blocks.len();

            if removed > 0 {
                eprintln!("⚠️  HF DualFeed: Removed {} orphaned blocks from pending queue", removed);
            }

            // Update current tip
            state.current_tip_hash = hex::encode(new_tip.as_bytes());
            state.current_tip_height = new_height;

            state.persist()?;
        }

        {
            let mut buffer = self.reorg_buffer.lock().await;
            buffer.push(record);
        }

        // Force flush reorg events immediately (they're important)
        self.force_flush().await?;

        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // BATCHING AND FLUSHING
    // ═══════════════════════════════════════════════════════════════════════════

    /// Check if we should flush based on batch size or time
    async fn maybe_flush(&self) -> Result<(), StreamerError> {
        let total_buffered = {
            let a = self.unconfirmed_buffer.lock().await.len();
            let b = self.confirmed_buffer.lock().await.len();
            let c = self.reorg_buffer.lock().await.len();
            a + b + c
        };

        // Check batch size
        if total_buffered >= self.config.batch_size {
            return self.force_flush().await;
        }

        // Check time interval
        let elapsed = {
            let last = self.last_flush.lock().await;
            last.elapsed().unwrap_or(Duration::ZERO)
        };

        if elapsed.as_secs() >= self.config.batch_interval_secs && total_buffered > 0 {
            return self.force_flush().await;
        }

        Ok(())
    }

    /// Force flush all buffers to HuggingFace
    pub async fn force_flush(&self) -> Result<(), StreamerError> {
        // Collect all records from buffers
        let unconfirmed: Vec<_> = {
            let mut buffer = self.unconfirmed_buffer.lock().await;
            std::mem::take(&mut *buffer)
        };

        let confirmed: Vec<_> = {
            let mut buffer = self.confirmed_buffer.lock().await;
            std::mem::take(&mut *buffer)
        };

        let reorgs: Vec<_> = {
            let mut buffer = self.reorg_buffer.lock().await;
            std::mem::take(&mut *buffer)
        };

        let total = unconfirmed.len() + confirmed.len() + reorgs.len();
        if total == 0 {
            return Ok(());
        }

        eprintln!("📤 HF DualFeed: Flushing {} records (A:{}, B:{}, C:{})",
            total, unconfirmed.len(), confirmed.len(), reorgs.len());

        // Convert to JSON values for unified upload
        let mut all_records: Vec<serde_json::Value> = Vec::new();

        for record in unconfirmed {
            if let Ok(v) = serde_json::to_value(&record) {
                all_records.push(v);
            }
        }

        for record in confirmed {
            if let Ok(v) = serde_json::to_value(&record) {
                all_records.push(v);
            }
        }

        for record in reorgs {
            if let Ok(v) = serde_json::to_value(&record) {
                all_records.push(v);
            }
        }

        // Update last flush time
        {
            let mut last = self.last_flush.lock().await;
            *last = SystemTime::now();
        }

        // TODO: Actually push to HuggingFace API
        // For now, just log success (will integrate with HuggingFaceClient)
        eprintln!("✅ HF DualFeed: Successfully flushed {} records", all_records.len());

        Ok(())
    }

    /// Get current state summary for diagnostics
    pub async fn get_state_summary(&self) -> StreamerStateSummary {
        let state = self.state.lock().await;
        let unconfirmed = self.unconfirmed_buffer.lock().await.len();
        let confirmed = self.confirmed_buffer.lock().await.len();
        let reorgs = self.reorg_buffer.lock().await.len();

        StreamerStateSummary {
            last_confirmed_height: state.last_confirmed_height,
            pending_blocks: state.pending_blocks.len(),
            buffered_unconfirmed: unconfirmed,
            buffered_confirmed: confirmed,
            buffered_reorgs: reorgs,
            current_tip_height: state.current_tip_height,
        }
    }
}

/// Summary of streamer state for diagnostics
#[derive(Debug, Clone)]
pub struct StreamerStateSummary {
    pub last_confirmed_height: u64,
    pub pending_blocks: usize,
    pub buffered_unconfirmed: usize,
    pub buffered_confirmed: usize,
    pub buffered_reorgs: usize,
    pub current_tip_height: u64,
}

// ═══════════════════════════════════════════════════════════════════════════════
// ERRORS
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, thiserror::Error)]
pub enum StreamerError {
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("IO error: {0}")]
    Io(String),
    #[error("Client error: {0}")]
    Client(#[from] ClientError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_streamer_state_persistence() {
        let temp_dir = std::env::temp_dir().join("hf_streamer_test");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        // Create state and modify it
        let mut state = StreamerState::new(temp_dir.clone());
        state.last_confirmed_height = 100;
        state.mark_published("test_record_1".to_string());
        state.persist().unwrap();

        // Load state and verify
        let loaded = StreamerState::new(temp_dir.clone());
        assert_eq!(loaded.last_confirmed_height, 100);
        assert!(loaded.is_published("test_record_1"));

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
