// Node Service
// Main orchestrator tying all components together
//
// NOTE: Many protocol handlers are prepared for future protocol extensions
#![allow(dead_code)]

mod block_processing;
mod fork;
mod merkle;
mod mining;

// Conditional ChainState: uses ADZDB when compiled with --features adzdb
#[cfg(not(feature = "adzdb"))]
use crate::chain::{ChainBlockProvider, ChainState};
#[cfg(feature = "adzdb")]
use crate::chain_adzdb::{AdzdbChainState as ChainState, ChainBlockProvider};
use crate::config::NodeConfig;
use crate::faucet::{Faucet, FaucetConfig};
use crate::genesis::{create_genesis_block, GenesisConfig};
use crate::peer_consensus::PeerConsensus;
use crate::validator::BlockValidator;
use coinject_consensus::{default_registry, Miner, MiningConfig};
use coinject_core::Address;
use coinject_mempool::{ProblemMarketplace, TransactionPool};
// libp2p removed - using CPP protocol only
use coinject_huggingface::{
    DualFeedStreamer, EnergyConfig, EnergyMeasurementMethod, HuggingFaceConfig, HuggingFaceSync,
    StreamerConfig, SyncConfig,
};
use coinject_network::cpp::{
    BlockProvider, CppConfig, CppNetwork, NetworkCommand as CppNetworkCommand,
    NetworkEvent as CppNetworkEvent, NodeType as CppNodeType, PeerId as CppPeerId,
};
use coinject_rpc::server::{
    MiningDifficultyTip, MiningDifficultyTipFuture, MiningWork, MiningWorkFuture, NetworkPeerInfo,
};
use coinject_rpc::websocket::{
    RpcCommand as WebSocketRpcCommand, RpcEvent as WebSocketRpcEvent, WebSocketRpc,
};
use coinject_rpc::{MiningDifficultyTipProvider, MiningWorkProvider, RpcServer, RpcServerState};
use coinject_state::{
    AccountState, ChannelState, DimensionalPoolState, EscrowState, MarketplaceState, TimeLockState,
    TrustLineState,
};
use coinject_tokenomics::NetworkMetrics;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tokio::time;
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SyncHealthState {
    Normal,
    SuspectFork,
    Recovery,
}

/// Prefer the active peer advertising the most cumulative work for hash-anchored sync.
async fn select_sync_peer_by_cumulative_work(
    peer_consensus: &Arc<PeerConsensus>,
    fallback: CppPeerId,
) -> CppPeerId {
    let Some((id, _)) = peer_consensus.best_active_peer_by_cumulative_work().await else {
        return fallback;
    };
    hex::decode(&id)
        .ok()
        .and_then(|b| b.try_into().ok())
        .unwrap_or(fallback)
}

const MAX_BLOCKS_PER_RESPONSE: u64 = crate::sync_canonical::SYNC_BATCH_BLOCKS;

/// Peer tip + fallback CPP peer for a hash-anchored GetBlocks request.
#[derive(Debug, Clone, Copy)]
struct HashAnchoredSyncRequest {
    fallback_peer: CppPeerId,
    peer_tip: crate::sync_canonical::PeerSyncTip,
}

fn hash_anchored_sync_req(
    fallback_peer: CppPeerId,
    height: u64,
    hash: coinject_core::Hash,
    cumulative_work: u128,
) -> HashAnchoredSyncRequest {
    HashAnchoredSyncRequest {
        fallback_peer,
        peer_tip: crate::sync_canonical::PeerSyncTip {
            height,
            hash,
            cumulative_work,
        },
    }
}

/// Send GetBlocks using hash-anchored `[from, to]` planning (re-fetch fork height when needed).
async fn request_hash_anchored_sync(
    chain: &Arc<ChainState>,
    peer_consensus: &Arc<PeerConsensus>,
    sync_health: &Arc<RwLock<SyncHealthState>>,
    cpp_tx: &mpsc::UnboundedSender<CppNetworkCommand>,
    req: HashAnchoredSyncRequest,
) {
    let local_height = chain.best_block_height().await;
    let local_hash = chain.best_block_hash().await;
    let suspect_fork = matches!(
        *sync_health.read().await,
        SyncHealthState::SuspectFork | SyncHealthState::Recovery
    );

    let plan = match crate::sync_canonical::plan_sync_batch(
        chain.as_ref(),
        local_height,
        local_hash,
        req.peer_tip,
        suspect_fork,
        MAX_BLOCKS_PER_RESPONSE,
    )
    .await
    {
        Ok(p) => p,
        Err(e) => {
            warn!(error = %e, "hash-anchored sync plan failed");
            return;
        }
    };

    if plan.from_height > plan.to_height {
        return;
    }

    let sync_peer = select_sync_peer_by_cumulative_work(peer_consensus, req.fallback_peer).await;
    debug!(
        from_height = plan.from_height,
        to_height = plan.to_height,
        peer_id = %hex::encode(sync_peer),
        peer_tip_height = req.peer_tip.height,
        suspect_fork,
        "requesting hash-anchored sync blocks"
    );
    let _ = cpp_tx.send(CppNetworkCommand::RequestBlocks {
        peer_id: sync_peer,
        from_height: plan.from_height,
        to_height: plan.to_height,
        request_id: rand::random(),
    });

    // Re-fetch the fork-height header explicitly — batched sync may deliver h+1..h+N without
    // the winning block at our tip when the local DB already maps that height to an orphan.
    if suspect_fork && plan.from_height == local_height {
        let _ = cpp_tx.send(CppNetworkCommand::RequestBlocks {
            peer_id: sync_peer,
            from_height: local_height,
            to_height: local_height,
            request_id: rand::random(),
        });
    }
}

/// Hash-anchored catch-up from the mesh peer with the greatest work (or height when work is 0).
async fn request_recovery_sync_from_heaviest_peer(
    chain: &Arc<ChainState>,
    peer_consensus: &Arc<PeerConsensus>,
    sync_health: &Arc<RwLock<SyncHealthState>>,
    cpp_tx: &mpsc::UnboundedSender<CppNetworkCommand>,
) {
    let local_work = chain.best_cumulative_work().await;
    let our_height = chain.best_block_height().await;
    let our_hash = *chain.best_block_hash().await.as_bytes();
    let Some((peer_id, peer_state)) = peer_consensus
        .best_peer_for_fork_recovery(local_work, our_height, our_hash)
        .await
    else {
        return;
    };
    let Ok(peer_id_bytes) = hex::decode(&peer_id) else {
        return;
    };
    let Ok(peer_arr) = <[u8; 32]>::try_from(peer_id_bytes) else {
        return;
    };
    request_hash_anchored_sync(
        chain,
        peer_consensus,
        sync_health,
        cpp_tx,
        hash_anchored_sync_req(
            peer_arr,
            peer_state.best_height,
            coinject_core::Hash::from_bytes(peer_state.best_hash),
            peer_state.cumulative_work,
        ),
    )
    .await;
}

/// Fetch the competing header at the current tip height on the peer's winning branch.
async fn request_fork_point_block_from_heaviest_peer(
    chain: &Arc<ChainState>,
    peer_consensus: &Arc<PeerConsensus>,
    cpp_tx: &mpsc::UnboundedSender<CppNetworkCommand>,
    fork_height: u64,
) {
    let local_work = chain.best_cumulative_work().await;
    let our_hash = *chain.best_block_hash().await.as_bytes();
    let Some((peer_id, _)) = peer_consensus
        .best_peer_for_fork_recovery(local_work, fork_height, our_hash)
        .await
    else {
        return;
    };
    let Ok(peer_id_bytes) = hex::decode(&peer_id) else {
        return;
    };
    let Ok(peer_arr) = <[u8; 32]>::try_from(peer_id_bytes) else {
        return;
    };
    debug!(
        fork_height,
        peer_id = %peer_id,
        "requesting explicit fork-point block from heaviest peer"
    );
    let _ = cpp_tx.send(CppNetworkCommand::RequestBlocks {
        peer_id: peer_arr,
        from_height: fork_height,
        to_height: fork_height,
        request_id: rand::random(),
    });
}

async fn run_startup_chain_sanity_checks(chain: &Arc<ChainState>) {
    let best_height = chain.best_block_height().await;
    let best_hash = chain.best_block_hash().await;
    let best_work = chain.best_cumulative_work().await;
    let genesis = chain.genesis_hash();

    match chain.get_block_by_height(0) {
        Ok(Some(genesis_block)) if genesis_block.header.hash() == genesis => {}
        Ok(Some(genesis_block)) => {
            error!(
                expected = %genesis,
                got = %genesis_block.header.hash(),
                "startup sanity: genesis hash mismatch"
            );
        }
        Ok(None) => error!("startup sanity: missing genesis block"),
        Err(e) => error!(error = %e, "startup sanity: failed reading genesis block"),
    }

    match chain.get_block_by_height(best_height) {
        Ok(Some(best_block)) if best_block.header.hash() == best_hash => {}
        Ok(Some(best_block)) => warn!(
            best_height,
            expected_tip = %best_hash,
            got_tip = %best_block.header.hash(),
            "startup sanity: canonical height lookup differs from best hash"
        ),
        Ok(None) => warn!(
            best_height,
            "startup sanity: best height has no retrievable block"
        ),
        Err(e) => warn!(best_height, error = %e, "startup sanity: failed retrieving best height"),
    }

    if best_height > 0 && best_work == 0 {
        warn!(
            best_height,
            "startup sanity: non-genesis tip reports zero cumulative work"
        );
    }
}

async fn set_sync_health_state(
    state: &Arc<RwLock<SyncHealthState>>,
    next: SyncHealthState,
    reason: &'static str,
) {
    let mut guard = state.write().await;
    if *guard != next {
        info!(from = ?*guard, to = ?next, reason, "sync health state changed");
        *guard = next;
    }
}

/// Get the debug log path from DATA_DIR environment variable
pub fn get_debug_log_path() -> std::path::PathBuf {
    let data_dir = std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".to_string());
    std::path::PathBuf::from(data_dir).join("debug.log")
}

/// Commands that can be sent to the network task
#[derive(Debug)]
pub(crate) enum NetworkCommand {
    /// Broadcast newly mined block to all peers
    BroadcastBlock(coinject_core::Block),
    /// Send historical block for sync with unique request_id (bypasses gossipsub dedup)
    /// This is the INSTITUTIONAL-GRADE solution for reliable sync
    SendSyncBlock {
        block: coinject_core::Block,
        request_id: u64,
    },
    BroadcastTransaction(coinject_core::Transaction),
    BroadcastStatus {
        best_height: u64,
        best_hash: coinject_core::Hash,
        genesis_hash: coinject_core::Hash,
        node_type: coinject_network::NetworkNodeType,
    },
    RequestBlocks {
        from_height: u64,
        to_height: u64,
    },
    /// Legacy: Send block to specific peer (kept for compatibility)
    SendBlockToPeer {
        block: coinject_core::Block,
        peer: CppPeerId,
    },
    // === REQUEST-RESPONSE SYNC COMMANDS ===
    // Reliable, ordered block delivery - bypasses GossipSub deduplication issues
    /// Request blocks from a specific peer via request-response (preferred for sync)
    RequestBlocksRR {
        peer: CppPeerId,
        from_height: u64,
        to_height: u64,
    },
    /// Send blocks response via request-response
    SendBlocksResponse {
        request_id: u64,
        blocks: Vec<coinject_core::Block>,
    },
    /// Send error response via request-response
    SendErrorResponse {
        request_id: u64,
        message: String,
    },
    // === LIGHT SYNC COMMANDS ===
    /// Send headers to a requesting peer
    SendHeaders {
        headers: Vec<coinject_core::BlockHeader>,
        request_id: u64,
    },
    /// Send FlyClient proof response
    SendFlyClientProof {
        proof_data: Vec<u8>,
        request_id: u64,
    },
    /// Send MMR proof response
    SendMMRProof {
        header: coinject_core::BlockHeader,
        proof_data: Vec<u8>,
        mmr_root: coinject_core::Hash,
        request_id: u64,
    },
    /// Send chain tip response
    SendChainTip {
        height: u64,
        hash: coinject_core::Hash,
        mmr_root: coinject_core::Hash,
        total_work: u128,
        request_id: u64,
    },
    /// Request headers (for Light client sync)
    RequestHeaders {
        start_height: u64,
        max_headers: u32,
    },
    /// Request FlyClient proof
    RequestFlyClientProof {
        security_param: u32,
    },
}

/// Main node service coordinating all blockchain components
pub struct CoinjectNode {
    config: NodeConfig,
    chain: Arc<ChainState>,
    state: Arc<AccountState>,
    timelock_state: Arc<TimeLockState>,
    escrow_state: Arc<EscrowState>,
    channel_state: Arc<ChannelState>,
    trustline_state: Arc<TrustLineState>,
    dimensional_pool_state: Arc<DimensionalPoolState>,
    marketplace_state: Arc<MarketplaceState>,
    validator: Arc<BlockValidator>,
    marketplace: Arc<RwLock<ProblemMarketplace>>,
    tx_pool: Arc<RwLock<TransactionPool>>,
    miner: Option<Arc<RwLock<Miner>>>,
    faucet: Option<Arc<Faucet>>,
    network_cmd_tx: Option<mpsc::UnboundedSender<NetworkCommand>>,
    rpc: Option<RpcServer>,
    /// CPP Network service (Phase 3)
    cpp_network_cmd_tx: Option<mpsc::UnboundedSender<CppNetworkCommand>>,
    /// WebSocket RPC service (Phase 3)
    websocket_rpc_cmd_tx: Option<mpsc::UnboundedSender<WebSocketRpcCommand>>,
    hf_sync: Option<Arc<HuggingFaceSync>>,
    /// Phase 1C: Dual-feed HuggingFace streamer
    dual_feed_streamer: Option<Arc<DualFeedStreamer>>,
    /// Node type classification manager (6 specialized types)
    node_classification: Arc<RwLock<crate::node_types::NodeClassificationManager>>,
    /// Light client state (for headers-only mode)
    light_client: Option<Arc<crate::light_client::LightClientState>>,
    /// Node Type Manager - Central orchestrator for capabilities and protocol
    node_manager: Arc<crate::node_manager::NodeTypeManager>,
    /// Capability-based peer router
    capability_router: Arc<crate::node_manager::CapabilityRouter>,
    shutdown_tx: mpsc::Sender<()>,
    shutdown_rx: mpsc::Receiver<()>,
}

impl CoinjectNode {
    /// Create and initialize a new node
    pub async fn new(config: NodeConfig) -> Result<Self, Box<dyn std::error::Error>> {
        info!("initializing coinject node");

        // Validate configuration
        config.validate()?;

        // Log block version configuration (P2P.F: Prove the F)
        info!(
            supported_versions = ?crate::config::SUPPORTED_VERSIONS,
            min_version = config.min_block_version,
            produce_version = config.produce_block_version,
            strict = config.strict_version,
            "block version config"
        );

        // Create data directory (parent directory for database files)
        std::fs::create_dir_all(&config.data_dir)?;

        // Initialize genesis block
        let genesis = create_genesis_block(GenesisConfig::default());
        let genesis_hash = genesis.header.hash();
        info!(genesis_hash = ?genesis_hash, "genesis block loaded");

        // Initialize chain state
        // Ensure parent directory exists for chain database file
        let chain_db_path = config.chain_db_path();
        if let Some(parent) = chain_db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let chain = Arc::new({
            #[cfg(not(feature = "adzdb"))]
            let cs = ChainState::new(chain_db_path, &genesis, config.block_cache_size)?;
            #[cfg(feature = "adzdb")]
            let cs = ChainState::new(chain_db_path, &genesis)?;
            cs
        });
        let best_height = chain.best_block_height().await;
        info!(best_height, "blockchain state initialized");

        // Initialize account state and advanced transaction states
        // Ensure parent directory exists for state database file
        let state_db_path = config.state_db_path();
        if let Some(parent) = state_db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Account state: uses ADZDB when feature is enabled (avoids Windows file locking)
        #[cfg(not(feature = "adzdb"))]
        let state = {
            let state_db = Arc::new(redb::Database::create(&state_db_path)?);
            Arc::new(AccountState::from_db(Arc::clone(&state_db)))
        };
        #[cfg(feature = "adzdb")]
        let state = Arc::new(AccountState::new(&state_db_path)?);

        // Advanced transaction states still use redb (they don't have Windows locking issues)
        // Create a separate redb database for advanced states
        let advanced_state_db_path = state_db_path
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .join("advanced_state.db");
        let advanced_state_db = Arc::new(redb::Database::create(&advanced_state_db_path)?);
        let timelock_state = Arc::new(TimeLockState::new(Arc::clone(&advanced_state_db))?);
        let escrow_state = Arc::new(EscrowState::new(Arc::clone(&advanced_state_db))?);
        let channel_state = Arc::new(ChannelState::new(Arc::clone(&advanced_state_db))?);
        let trustline_state = Arc::new(TrustLineState::new(Arc::clone(&advanced_state_db))?);
        let dimensional_pool_state =
            Arc::new(DimensionalPoolState::new(Arc::clone(&advanced_state_db))?);
        let marketplace_state =
            Arc::new(MarketplaceState::from_db(Arc::clone(&advanced_state_db))?);

        // Apply genesis if this is a new chain
        if best_height == 0 {
            let genesis_addr = genesis.header.miner;
            let genesis_reward = genesis.coinbase.reward;
            state.set_balance(&genesis_addr, genesis_reward)?;
            info!(genesis_reward, "genesis account funded");

            // Initialize dimensional pools with genesis liquidity
            dimensional_pool_state.initialize_pools(genesis_reward, 0)?;
            info!("dimensional pools initialized with genesis liquidity");
        }

        // Initialize validator
        let validator = Arc::new(BlockValidator::new(config.difficulty));

        // Initialize mempool components
        let marketplace = Arc::new(RwLock::new(ProblemMarketplace::new()));
        let tx_pool = Arc::new(RwLock::new(TransactionPool::new()));

        // Initialize miner if enabled
        let miner = if config.mine {
            let miner_address = if let Some(ref addr_hex) = config.miner_address {
                // Use explicitly provided miner address
                let addr_bytes = hex::decode(addr_hex)?;
                if addr_bytes.len() != 32 {
                    return Err("Invalid miner address length".into());
                }
                let mut bytes = [0u8; 32];
                bytes.copy_from_slice(&addr_bytes);
                Address::from_bytes(bytes)
            } else {
                // Load or generate validator key from data directory
                let keystore = crate::keystore::ValidatorKeystore::new(&config.data_dir);
                let validator_key = keystore
                    .get_or_create_key()
                    .map_err(|e| format!("Failed to get validator key: {}", e))?;
                validator_key.address()
            };

            let mining_config = MiningConfig {
                miner_address,
                target_block_time: Duration::from_secs(config.block_time),
                min_difficulty: config.difficulty,
                golden_activation_height: config.golden_activation_height,
            };

            info!(
                miner_address = hex::encode(miner_address.as_bytes()),
                golden_activation_height = config.golden_activation_height,
                target_block_time_secs = config.block_time,
                "miner initialized"
            );

            // Create problem registry (shared across consensus components)
            let registry = default_registry();

            let mut miner = Miner::new(mining_config);
            miner
                .set_network_metrics(Arc::new(RwLock::new(NetworkMetrics::default_window())))
                .await;
            miner.set_registry(registry).await;
            Some(Arc::new(RwLock::new(miner)))
        } else {
            None
        };

        // Initialize faucet if enabled
        let faucet = if config.enable_faucet {
            info!(
                amount = config.faucet_amount,
                cooldown_secs = config.faucet_cooldown,
                "faucet enabled"
            );

            let faucet_config = FaucetConfig {
                enabled: true,
                amount: config.faucet_amount,
                cooldown: config.faucet_cooldown,
            };
            Some(Arc::new(Faucet::new(faucet_config)))
        } else {
            None
        };

        // Initialize HuggingFace sync if configured
        // Fix 1: Resolve token from config, falling back to env vars HUGGINGFACE_TOKEN / HF_TOKEN
        let hf_token_resolved = config
            .hf_token
            .clone()
            .or_else(|| std::env::var("HUGGINGFACE_TOKEN").ok())
            .or_else(|| std::env::var("HF_TOKEN").ok());
        let hf_sync = if let (Some(hf_token), Some(hf_dataset_name)) =
            (&hf_token_resolved, &config.hf_dataset_name)
        {
            info!(dataset = %hf_dataset_name, "initializing huggingface sync");

            let hf_config = HuggingFaceConfig {
                token: hf_token.clone(),
                dataset_prefix: hf_dataset_name.clone(),
                dataset_config: None,
                ..Default::default()
            };

            let energy_config = EnergyConfig {
                enabled: true,
                method: EnergyMeasurementMethod::Estimate,
                cpu_tdp_watts: 65.0,
                min_energy_threshold_joules: 0.000001, // 1 microjoule minimum
            };

            let sync_config = SyncConfig {
                enabled: true,
                include_submitter_address: false,
                include_solver_address: false,
                batch_size: 10,
                batch_interval: Duration::from_secs(60),
                min_confirmations: 20, // k-confirmation guard for reorg safety
                mining_difficulty_bits: config.difficulty,
            };

            match HuggingFaceSync::new(hf_config, energy_config, sync_config.clone()) {
                Ok(sync) => {
                    info!("huggingface sync initialized");
                    Some(Arc::new(sync))
                }
                Err(e) => {
                    warn!(error = %e, "failed to initialize huggingface sync");
                    None
                }
            }
        } else {
            None
        };

        // Initialize Phase 1C: Dual-Feed Streamer (alongside legacy hf_sync)
        let dual_feed_streamer = if hf_token_resolved.is_some() {
            info!("initializing dual-feed streamer (head_unconfirmed, canonical_confirmed, reorg_events)");

            let streamer_config = StreamerConfig {
                min_confirmations: 20, // Same k as legacy sync
                batch_size: 10,
                batch_interval_secs: 60,
                enabled: true,
                node_id: None, // Will be set when network starts
                data_dir: config.data_dir.clone(),
            };

            let streamer = DualFeedStreamer::new(streamer_config);
            info!("dual-feed streamer initialized");
            Some(Arc::new(streamer))
        } else {
            None
        };

        // Create shutdown channel
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

        // Initialize Node Classification Manager
        let mut classification_manager =
            crate::node_types::NodeClassificationManager::new(best_height);

        // Set target type from config
        let target_type = config.target_node_type();
        classification_manager.set_target_type(target_type);

        // Set headers-only mode if configured
        if config.is_light_mode() {
            classification_manager.set_headers_only(true);
            info!("light mode enabled (headers-only sync)");
        }

        let node_classification = Arc::new(RwLock::new(classification_manager));
        info!(target_type = %target_type, "node classification manager initialized");

        // Initialize Light Client if in headers-only mode
        let light_client = if config.is_light_mode() {
            let light_state =
                crate::light_client::LightClientState::new(genesis_hash, genesis.header.clone());
            info!("light client initialized for header sync");
            Some(Arc::new(light_state))
        } else {
            None
        };

        // Initialize Node Type Manager (Central Orchestrator)
        let (node_manager, _manager_rx, _classification_rx) =
            crate::node_manager::NodeTypeManager::new(
                best_height,
                target_type,
                Some(genesis.header.clone()),
            );
        let node_manager = Arc::new(node_manager);

        // Initialize Capability Router
        let capability_router = Arc::new(crate::node_manager::CapabilityRouter::new());

        let capabilities = crate::node_manager::NetworkCapabilities::for_node_type(target_type);
        info!(
            can_produce_blocks = capabilities.can_produce_blocks,
            can_validate_blocks = capabilities.can_validate_blocks,
            can_serve_flyclient = capabilities.can_serve_flyclient,
            can_solve_problems = capabilities.can_solve_problems,
            can_provide_oracle_data = capabilities.can_provide_oracle_data,
            max_inbound_peers = capabilities.max_inbound_peers,
            max_outbound_peers = capabilities.max_outbound_peers,
            "node type manager initialized"
        );

        Ok(CoinjectNode {
            config,
            chain,
            state,
            timelock_state,
            escrow_state,
            channel_state,
            trustline_state,
            dimensional_pool_state,
            marketplace_state,
            validator,
            marketplace,
            tx_pool,
            miner,
            faucet,
            network_cmd_tx: None,
            rpc: None,
            cpp_network_cmd_tx: None,
            websocket_rpc_cmd_tx: None,
            hf_sync,
            dual_feed_streamer,
            node_classification,
            light_client,
            node_manager,
            capability_router,
            shutdown_tx,
            shutdown_rx,
        })
    }

    /// Start the node services
    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("starting node services");

        // Create shared peer count for RPC
        let peer_count = Arc::new(RwLock::new(0));
        let peer_directory: Arc<RwLock<HashMap<String, NetworkPeerInfo>>> =
            Arc::new(RwLock::new(HashMap::new()));

        // Generate CPP PeerId — random per instance to avoid collisions in Docker
        // (Previous deterministic scheme used data_dir + chain_id, which collided
        // when all containers use --data-dir /data with the same chain_id)
        let peer_id_hash = blake3::hash(&rand::random::<[u8; 32]>());
        let local_peer_id_bytes: CppPeerId = {
            let mut bytes = [0u8; 32];
            bytes.copy_from_slice(&peer_id_hash.as_bytes()[..32]);
            bytes
        };
        let local_peer_id_str = hex::encode(local_peer_id_bytes);
        info!(peer_id = %local_peer_id_str, "local peer id generated");

        // Track listen addresses for RPC (CPP addresses)
        let listen_addresses: Arc<RwLock<Vec<String>>> = Arc::new(RwLock::new(vec![
            format!("cpp://{}", self.config.cpp_p2p_addr),
            format!("ws://{}", self.config.cpp_ws_addr),
        ]));

        // Create command channel for CPP network operations (legacy NetworkCommand kept for compatibility)
        let (network_cmd_tx, _network_cmd_rx) = mpsc::unbounded_channel::<NetworkCommand>();

        // Start RPC server
        let rpc_addr = self.config.rpc_socket_addr()?;

        // Create faucet handler if faucet is enabled
        let faucet_handler = self.faucet.as_ref().map(|faucet| {
            let faucet_clone = Arc::clone(faucet);
            let _state_clone = Arc::clone(&self.state);
            Arc::new(move |addr: &Address| -> Result<u128, String> {
                faucet_clone.request_tokens(addr).map_err(|e| e.to_string())
            }) as coinject_rpc::FaucetHandler
        });

        // NOTE: peer_count was already created at line 255 and passed to NetworkService
        // DO NOT create a new one here - use the same Arc so network updates are visible!

        // Track best known peer height for sync-before-mine logic
        let best_known_peer_height = Arc::new(RwLock::new(0u64));

        // Multi-peer consensus tracker (XRPL-inspired, requires 5+ peers for 80% threshold)
        let peer_consensus = Arc::new(PeerConsensus::with_defaults());
        run_startup_chain_sanity_checks(&self.chain).await;

        // Create block submission handler
        // This handler validates, stores, and broadcasts blocks submitted via RPC
        let chain_for_submission = Arc::clone(&self.chain);
        let state_for_submission = Arc::clone(&self.state);
        let timelock_state_for_submission = Arc::clone(&self.timelock_state);
        let escrow_state_for_submission = Arc::clone(&self.escrow_state);
        let channel_state_for_submission = Arc::clone(&self.channel_state);
        let trustline_state_for_submission = Arc::clone(&self.trustline_state);
        let dimensional_pool_state_for_submission = Arc::clone(&self.dimensional_pool_state);
        let marketplace_state_for_submission = Arc::clone(&self.marketplace_state);
        let validator_for_submission = Arc::clone(&self.validator);
        let tx_pool_for_submission = Arc::clone(&self.tx_pool);
        let miner_for_submission = self.miner.clone();
        let network_tx_for_submission = network_cmd_tx.clone();
        let hf_sync_for_submission = self.hf_sync.clone();

        let block_submission_handler: Option<coinject_rpc::BlockSubmissionHandler> = Some(
            Arc::new(
                move |block: coinject_core::Block| -> coinject_rpc::server::BlockSubmissionFuture {
                    let submission_started_at = Instant::now();
                    let submission_height = block.header.height;
                    let block_hash = block.hash();
                    let submission_trace = format!(
                        "{}:{}",
                        submission_height,
                        hex::encode(&block_hash.as_bytes()[..4])
                    );
                    info!(
                        trace = %submission_trace,
                        height = submission_height,
                        elapsed_ms = submission_started_at.elapsed().as_millis(),
                        "rpc block submission received"
                    );

                    let chain = Arc::clone(&chain_for_submission);
                    let state = Arc::clone(&state_for_submission);
                    let timelock_state = Arc::clone(&timelock_state_for_submission);
                    let escrow_state = Arc::clone(&escrow_state_for_submission);
                    let channel_state = Arc::clone(&channel_state_for_submission);
                    let trustline_state = Arc::clone(&trustline_state_for_submission);
                    let dimensional_pool_state = Arc::clone(&dimensional_pool_state_for_submission);
                    let marketplace_state = Arc::clone(&marketplace_state_for_submission);
                    let validator = Arc::clone(&validator_for_submission);
                    let tx_pool = Arc::clone(&tx_pool_for_submission);
                    let miner_opt = miner_for_submission.clone();
                    let network_tx = network_tx_for_submission.clone();
                    let hf_sync = hf_sync_for_submission.clone();
                    let submission_trace_for_task = submission_trace.clone();

                    Box::pin(async move {
                        let task_started_at = Instant::now();
                        let result = async {
                    // Get current chain state
                    let best_height = chain.best_block_height().await;
                    let best_hash = chain.best_block_hash().await;
                    let expected_height = best_height + 1;
                    info!(
                        trace = %submission_trace_for_task,
                        height = block.header.height,
                        elapsed_ms = task_started_at.elapsed().as_millis(),
                        best_height,
                        expected_height,
                        best_hash = %best_hash,
                        "rpc block submission fetched chain tip"
                    );

                    // Validate block height
                    if block.header.height != expected_height {
                        return Err(format!("Invalid block height: expected {}, got {}", expected_height, block.header.height));
                    }

                    // Validate previous hash
                    if block.header.prev_hash != best_hash {
                        return Err(format!("Invalid previous hash: expected {}, got {}", best_hash, block.header.prev_hash));
                    }

                    // Validate block (skip timestamp age check for RPC submissions)
                    let validation_started_at = Instant::now();
                    let emission_w = if block.header.height == 0 {
                        None
                    } else {
                        match chain.cumulative_work_at_tip_hash(block.header.prev_hash) {
                            Ok(w) => Some(w),
                            Err(e) => {
                                return Err(format!(
                                    "Could not compute parent cumulative work: {}",
                                    e
                                ));
                            }
                        }
                    };
                    match validator.validate_block_with_options(
                        &block,
                        &best_hash,
                        expected_height,
                        false,
                        emission_w,
                    ) {
                        Ok(()) => {
                            info!(
                                trace = %submission_trace_for_task,
                                height = block.header.height,
                                elapsed_ms = task_started_at.elapsed().as_millis(),
                                stage_elapsed_ms = validation_started_at.elapsed().as_millis(),
                                "rpc block submission validation completed"
                            );
                        },
                        Err(e) => return Err(format!("Block validation failed: {:?}", e)),
                    }

                    // Store block
                    let store_started_at = Instant::now();
                    match chain.store_block(&block).await {
                        Ok(is_new_best) => {
                            info!(
                                trace = %submission_trace_for_task,
                                height = block.header.height,
                                elapsed_ms = task_started_at.elapsed().as_millis(),
                                stage_elapsed_ms = store_started_at.elapsed().as_millis(),
                                is_new_best,
                                "rpc block submission store completed"
                            );
                            if !is_new_best {
                                return Err("Block did not extend the chain".to_string());
                            }
                        },
                        Err(e) => return Err(format!("Failed to store block: {}", e)),
                    }

                    // Apply block transactions
                    let apply_started_at = Instant::now();
                    match Self::apply_block_transactions(
                        &block,
                        &state,
                        &timelock_state,
                        &escrow_state,
                        &channel_state,
                        &trustline_state,
                        &dimensional_pool_state,
                        &marketplace_state,
                    ) {
                        Ok(applied_txs) => {
                            info!(
                                trace = %submission_trace_for_task,
                                height = block.header.height,
                                elapsed_ms = task_started_at.elapsed().as_millis(),
                                stage_elapsed_ms = apply_started_at.elapsed().as_millis(),
                                applied_tx_count = applied_txs.len(),
                                "rpc block submission applied block transactions"
                            );
                            // Remove applied transactions from pool
                            let mut pool = tx_pool.write().await;
                            for tx_hash in &applied_txs {
                                pool.remove(tx_hash);
                            }
                            drop(pool);

                            Self::retarget_header_pow_from_stored_block(&miner_opt, &chain, &block)
                                .await;

                            // Broadcast block to network
                            let broadcast_started_at = Instant::now();
                            if let Err(e) = network_tx.send(NetworkCommand::BroadcastBlock(block.clone())) {
                                return Err(format!("Failed to broadcast block: {}", e));
                            }
                            info!(
                                trace = %submission_trace_for_task,
                                height = block.header.height,
                                elapsed_ms = task_started_at.elapsed().as_millis(),
                                stage_elapsed_ms = broadcast_started_at.elapsed().as_millis(),
                                "rpc block submission queued network broadcast"
                            );

                            // Push to Hugging Face if enabled
                            if let Some(ref hf_sync) = hf_sync {
                                let hf_sync_clone = Arc::clone(hf_sync);
                                let block_clone = block.clone();
                                tokio::spawn(async move {
                                    if let Err(e) = hf_sync_clone.push_consensus_block(&block_clone, false).await {
                                        warn!(error = %e, "failed to push rpc-submitted block to huggingface");
                                    }
                                });
                                info!(
                                    trace = %submission_trace_for_task,
                                    height = block.header.height,
                                    elapsed_ms = task_started_at.elapsed().as_millis(),
                                    "rpc block submission spawned huggingface push"
                                );
                            }

                            Ok(block.hash().to_string())
                        },
                        Err(e) => Err(format!("Failed to apply block transactions: {}", e)),
                    }
                }.await;

                        info!(
                            trace = %submission_trace_for_task,
                            height = block.header.height,
                            elapsed_ms = task_started_at.elapsed().as_millis(),
                            ok = result.is_ok(),
                            "rpc block submission async handler completed"
                        );
                        info!(
                            trace = %submission_trace,
                            height = submission_height,
                            elapsed_ms = submission_started_at.elapsed().as_millis(),
                            ok = result.is_ok(),
                            "rpc block submission completed"
                        );
                        result
                    })
                },
            ),
        );

        let mining_template_cache: mining::MiningTemplateCache =
            Arc::new(tokio::sync::RwLock::new(None));

        let mining_work_provider: Option<MiningWorkProvider> = self.miner.as_ref().map(|miner| {
            let miner = Arc::clone(miner);
            let cache = Arc::clone(&mining_template_cache);
            let best_height = self.chain.best_height_ref();
            let best_hash = self.chain.best_hash_ref();
            Arc::new(move || -> MiningWorkFuture {
                let miner = Arc::clone(&miner);
                let cache = Arc::clone(&cache);
                let best_height = Arc::clone(&best_height);
                let best_hash = Arc::clone(&best_hash);
                Box::pin(async move {
                    let bh = *best_height.read().await;
                    let prev = *best_hash.read().await;
                    let next_height = bh + 1;
                    let prev_hex = hex::encode(prev.as_bytes());

                    if let Some(cached) = cache.read().await.clone() {
                        if cached.next_height == next_height && cached.prev_hash == prev_hex {
                            return Ok(cached);
                        }
                    }

                    if let Ok(miner_guard) = miner.try_read() {
                        let difficulty = miner_guard.current_difficulty();
                        let problem = miner_guard.generate_problem(next_height, prev).await;
                        let work = MiningWork {
                            next_height,
                            prev_hash: prev_hex.clone(),
                            difficulty,
                            problem,
                        };
                        *cache.write().await = Some(work.clone());
                        return Ok(work);
                    }

                    // Mining loop holds `write()`; return template published just before `mine_block`.
                    if let Some(cached) = cache.read().await.clone() {
                        if cached.next_height == next_height && cached.prev_hash == prev_hex {
                            return Ok(cached);
                        }
                    }

                    Err(
                        "Mining template not ready yet; retry chain_getMiningWork in a few seconds"
                            .to_string(),
                    )
                }) as MiningWorkFuture
            }) as MiningWorkProvider
        });

        let mining_difficulty_tip_provider: Option<MiningDifficultyTipProvider> =
            self.miner.as_ref().map(|miner| {
                let miner = Arc::clone(miner);
                Arc::new(move || -> MiningDifficultyTipFuture {
                    let miner = Arc::clone(&miner);
                    Box::pin(async move {
                        let header_pow_difficulty = {
                            let m = miner.read().await;
                            m.current_difficulty()
                        };
                        let np_problem_size = {
                            let m = miner.read().await;
                            m.current_np_problem_size().await
                        };
                        Ok(MiningDifficultyTip {
                            header_pow_difficulty,
                            np_problem_size,
                        })
                    }) as MiningDifficultyTipFuture
                }) as MiningDifficultyTipProvider
            });

        // Updated by the periodic reorg/sync task: true when our tip is behind the peer median
        // (or best peer height) by more than `sync_threshold_blocks` — see `docs/FORKING_AND_REORG.md`.
        let is_syncing = Arc::new(tokio::sync::RwLock::new(false));
        let sync_health_state = Arc::new(tokio::sync::RwLock::new(SyncHealthState::Normal));

        let network_tx_for_rpc = network_cmd_tx.clone();
        let transaction_broadcast: Option<coinject_rpc::TransactionBroadcastFn> =
            Some(Arc::new(move |tx| {
                if let Err(e) = network_tx_for_rpc.send(NetworkCommand::BroadcastTransaction(tx)) {
                    warn!(error = %e, "failed to queue transaction broadcast from rpc");
                }
            }));

        let rpc_state = Arc::new(RpcServerState {
            account_state: Arc::clone(&self.state),
            timelock_state: Arc::clone(&self.timelock_state),
            escrow_state: Arc::clone(&self.escrow_state),
            channel_state: Arc::clone(&self.channel_state),
            marketplace_state: Arc::clone(&self.marketplace_state),
            blockchain: Arc::clone(&self.chain) as Arc<dyn coinject_rpc::BlockchainReader>,
            marketplace: Arc::clone(&self.marketplace),
            tx_pool: Arc::clone(&self.tx_pool),
            chain_id: self.config.chain_id.clone(),
            best_height: self.chain.best_height_ref(),
            best_hash: self.chain.best_hash_ref(),
            genesis_hash: self.chain.genesis_hash(),
            peer_count: Arc::clone(&peer_count),
            peer_directory: Arc::clone(&peer_directory),
            faucet_handler,
            block_submission_handler,
            local_peer_id: Some(local_peer_id_str.clone()),
            listen_addresses: Arc::clone(&listen_addresses),
            is_syncing: Arc::clone(&is_syncing),
            mining_work_provider,
            mining_difficulty_tip_provider,
            transaction_broadcast,
        });

        let rpc_server = RpcServer::new(rpc_addr, rpc_state).await?;
        info!(addr = %rpc_addr, "json-rpc server listening");

        self.network_cmd_tx = Some(network_cmd_tx.clone());
        self.rpc = Some(rpc_server);

        // =====================================================================
        // Phase 3: Initialize CPP Network and WebSocket RPC
        // =====================================================================
        info!(
            p2p_addr = %self.config.cpp_p2p_addr,
            ws_addr = %self.config.cpp_ws_addr,
            "starting cpp network"
        );

        let genesis_hash = self.chain.genesis_hash();
        let local_peer_id_bytes: [u8; 32] = {
            let decoded = hex::decode(&local_peer_id_str)
                .expect("generated local peer id should always be valid hex");
            let mut bytes = [0u8; 32];
            bytes.copy_from_slice(&decoded[..32]);
            bytes
        };

        // Parse CPP bootnodes from config (format: "IP:PORT" or multiaddr "/ip4/IP/tcp/PORT/p2p/PEER_ID")
        // For CPP, we extract IP:PORT from multiaddr format or use as-is if already IP:PORT
        // If no bootnodes provided, CPP will work in standalone mode
        let cpp_bootnodes: Vec<String> = if self.config.bootnodes.is_empty() {
            vec![] // No bootnodes - standalone mode
        } else {
            self.config
                .bootnodes
                .iter()
                .filter_map(|addr| {
                    // Try parsing as multiaddr first
                    if addr.starts_with('/') {
                        // Extract IP:PORT from multiaddr format: /ip4/IP/tcp/PORT/p2p/PEER_ID
                        let parts: Vec<&str> = addr.split('/').collect();
                        if parts.len() >= 5 && parts[1] == "ip4" && parts[3] == "tcp" {
                            let ip = parts[2];
                            let port = parts[4];
                            return Some(format!("{}:{}", ip, port));
                        }
                        None
                    } else {
                        // Already in IP:PORT format
                        Some(addr.clone())
                    }
                })
                .collect()
        };

        let cpp_config = CppConfig {
            p2p_listen: self.config.cpp_p2p_addr.clone(),
            ws_listen: self.config.cpp_ws_addr.clone(),
            bootnodes: cpp_bootnodes.clone(),
            max_peers: self.config.max_peers,
            enable_websocket: true,
            node_type: CppNodeType::Full, // TODO: Get from node classification
            // Must stay false until CppNetwork uses `with_signing_key`; otherwise inbound peers are rejected.
            require_encryption: false,
            ..Default::default()
        };

        // Get current chain state before creating CPP network
        let current_height = self.chain.best_block_height().await;
        let current_hash = self.chain.best_block_hash().await;
        let current_tip_cumulative_work = self.chain.best_cumulative_work().await;

        // Create block provider for serving sync requests to peers
        let block_provider: Arc<dyn BlockProvider> =
            Arc::new(ChainBlockProvider::new(self.chain.clone()));

        let (cpp_network, cpp_network_cmd_tx, mut cpp_network_event_rx) =
            CppNetwork::new_with_block_provider(
                cpp_config,
                local_peer_id_bytes,
                genesis_hash,
                current_height,
                current_hash,
                block_provider,
            );

        info!(height = current_height, hash = ?current_hash, "cpp network initialized with block provider");

        // Clone cpp_network_cmd_tx for multiple uses (before any moves)
        let cpp_network_cmd_tx_for_bootnodes = cpp_network_cmd_tx.clone();
        let cpp_network_cmd_tx_for_legacy = cpp_network_cmd_tx.clone();
        let cpp_network_cmd_tx_for_mining = cpp_network_cmd_tx.clone(); // For mining loop
        let cpp_network_cmd_tx_for_storage = cpp_network_cmd_tx.clone(); // Store for later use

        // Connect to CPP bootnodes after a short delay
        let cpp_bootnodes_for_connect = cpp_bootnodes.clone();
        tokio::spawn(async move {
            // Wait a bit for network to start listening
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

            // Connect to each bootnode
            for bootnode_addr in cpp_bootnodes_for_connect {
                // Try direct SocketAddr parse first, then DNS resolution for hostnames (e.g. Docker service names)
                let resolved = if let Ok(addr) = bootnode_addr.parse::<std::net::SocketAddr>() {
                    Some(addr)
                } else {
                    match tokio::net::lookup_host(&bootnode_addr).await {
                        Ok(mut addrs) => addrs.next(),
                        Err(e) => {
                            warn!(bootnode_addr = %bootnode_addr, error = %e, "failed to resolve bootnode");
                            None
                        }
                    }
                };
                if let Some(addr) = resolved {
                    info!(addr = %addr, bootnode = %bootnode_addr, "connecting to bootnode");
                    if let Err(e) = cpp_network_cmd_tx_for_bootnodes
                        .send(coinject_network::cpp::NetworkCommand::ConnectBootnode { addr })
                    {
                        error!(error = %e, "failed to send bootnode connect command");
                    }
                } else {
                    warn!(bootnode_addr = %bootnode_addr, "invalid bootnode address format");
                }
            }
        });

        // Spawn CPP network task
        let cpp_p2p_addr_clone = self.config.cpp_p2p_addr.clone();
        tokio::spawn(async move {
            info!("cpp network task starting");
            match cpp_network.start().await {
                Ok(()) => {
                    info!("cpp network task completed");
                }
                Err(e) => {
                    error!(error = %e, addr = %cpp_p2p_addr_clone, "cpp network error");
                }
            }
        });

        // Create block buffer for out-of-order blocks (used by CPP sync)
        let block_buffer: Arc<RwLock<HashMap<u64, coinject_core::Block>>> =
            Arc::new(RwLock::new(HashMap::new()));

        // Spawn CPP network event handler - fully integrated
        let chain_clone = Arc::clone(&self.chain);
        let state_clone = Arc::clone(&self.state);
        let timelock_state_clone = Arc::clone(&self.timelock_state);
        let escrow_state_clone = Arc::clone(&self.escrow_state);
        let channel_state_clone = Arc::clone(&self.channel_state);
        let trustline_state_clone = Arc::clone(&self.trustline_state);
        let dimensional_pool_state_clone = Arc::clone(&self.dimensional_pool_state);
        let marketplace_state_clone = Arc::clone(&self.marketplace_state);
        let validator_clone = Arc::clone(&self.validator);
        let tx_pool_clone = Arc::clone(&self.tx_pool);
        let best_known_peer_height_clone = Arc::clone(&best_known_peer_height);
        let peer_count_clone = Arc::clone(&peer_count);
        let peer_directory_clone = Arc::clone(&peer_directory);
        let peer_consensus_clone = Arc::clone(&peer_consensus);
        let network_cmd_tx_for_events = network_cmd_tx.clone();
        let cpp_network_cmd_tx_for_events = cpp_network_cmd_tx.clone();
        let hf_sync_clone = self.hf_sync.clone();
        let block_buffer_clone = Arc::clone(&block_buffer);
        let sync_health_state_clone = Arc::clone(&sync_health_state);
        // Clone config for version checking in event handler
        let config_clone = self.config.clone();
        let miner_clone = self.miner.clone();

        tokio::spawn(async move {
            while let Some(event) = cpp_network_event_rx.recv().await {
                match event {
                    CppNetworkEvent::BlockReceived {
                        block,
                        peer_id: _peer_id,
                    } => {
                        // Log block with version info (P2P.F: Prove the F)
                        let version_info = config_clone.version_info(block.header.version);
                        info!(
                            block_height = block.header.height,
                            block_hash = ?block.header.hash(),
                            version = %version_info,
                            "block received"
                        );

                        // Check version policy before validation
                        if let Err(reason) =
                            config_clone.should_accept_version(block.header.version)
                        {
                            warn!(
                                block_height = block.header.height,
                                version = %version_info,
                                reason = %reason,
                                "block rejected: version policy"
                            );
                            continue;
                        }

                        let best_height = chain_clone.best_block_height().await;
                        let best_hash = chain_clone.best_block_hash().await;
                        let expected_height = best_height + 1;

                        // Validate block height
                        if block.header.height != expected_height {
                            warn!(
                                expected_height,
                                block_height = block.header.height,
                                "block height mismatch"
                            );
                            continue;
                        }

                        // Validate previous hash
                        if block.header.prev_hash != best_hash {
                            warn!(
                                block_height = block.header.height,
                                "block prev_hash mismatch"
                            );
                            continue;
                        }

                        // Validate block
                        let emission_w = if block.header.height == 0 {
                            None
                        } else {
                            match chain_clone.cumulative_work_at_tip_hash(block.header.prev_hash) {
                                Ok(w) => Some(w),
                                Err(e) => {
                                    error!(block_height = block.header.height, error = %e, "parent cumulative work for emission");
                                    continue;
                                }
                            }
                        };
                        match validator_clone.validate_block_with_options(
                            &block,
                            &best_hash,
                            expected_height,
                            false,
                            emission_w,
                        ) {
                            Ok(()) => {
                                // Store block
                                match chain_clone.store_block(&block).await {
                                    Ok(is_new_best) => {
                                        if is_new_best {
                                            info!(block_height = block.header.height, block_hash = ?block.header.hash(), "block applied as new best");

                                            // Update CPP network chain state
                                            let new_height = block.header.height;
                                            let new_hash = block.header.hash();
                                            let cum = chain_clone.best_cumulative_work().await;
                                            if let Err(e) = cpp_network_cmd_tx_for_legacy.send(
                                                CppNetworkCommand::UpdateChainState {
                                                    best_height: new_height,
                                                    best_hash: new_hash,
                                                    cumulative_work: cum,
                                                },
                                            ) {
                                                warn!(error = %e, "failed to update cpp network chain state");
                                            }

                                            // Apply block transactions
                                            if let Err(e) = Self::apply_block_transactions(
                                                &block,
                                                &state_clone,
                                                &timelock_state_clone,
                                                &escrow_state_clone,
                                                &channel_state_clone,
                                                &trustline_state_clone,
                                                &dimensional_pool_state_clone,
                                                &marketplace_state_clone,
                                            ) {
                                                error!(block_height = block.header.height, error = %e, "failed to apply block transactions");
                                            } else {
                                                // Remove applied transactions from pool
                                                let mut pool = tx_pool_clone.write().await;
                                                for tx in &block.transactions {
                                                    pool.remove(&tx.hash());
                                                }

                                                CoinjectNode::retarget_header_pow_from_stored_block(
                                                    &miner_clone,
                                                    &chain_clone,
                                                    &block,
                                                )
                                                .await;

                                                // Update best known peer height
                                                let mut best_peer =
                                                    best_known_peer_height_clone.write().await;
                                                if block.header.height > *best_peer {
                                                    *best_peer = block.header.height;
                                                }

                                                // Push to Hugging Face if enabled
                                                if let Some(ref hf_sync) = hf_sync_clone {
                                                    let hf_sync_clone2 = Arc::clone(hf_sync);
                                                    let block_clone = block.clone();
                                                    tokio::spawn(async move {
                                                        if let Err(e) = hf_sync_clone2
                                                            .push_consensus_block(
                                                                &block_clone,
                                                                false,
                                                            )
                                                            .await
                                                        {
                                                            warn!(error = %e, "failed to push block to huggingface");
                                                        }
                                                    });
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        error!(block_height = block.header.height, error = %e, "failed to store block");
                                    }
                                }
                            }
                            Err(e) => {
                                error!(block_height = block.header.height, error = ?e, "block validation failed");
                            }
                        }
                    }
                    CppNetworkEvent::TransactionReceived {
                        transaction,
                        peer_id,
                    } => {
                        debug!(tx_hash = ?transaction.hash(), peer_id = %hex::encode(peer_id), "transaction received");
                        let mut pool = tx_pool_clone.write().await;
                        if let Err(e) = pool.add(transaction) {
                            warn!(error = %e, "failed to add transaction to pool");
                        }
                    }
                    CppNetworkEvent::BlocksReceived {
                        blocks,
                        request_id: _,
                        peer_id,
                    } => {
                        debug!(count = blocks.len(), peer_id = %hex::encode(peer_id), "sync blocks received");

                        let mut highest_received: u64 = 0;
                        let mut blocks_applied: u64 = 0;
                        let mut blocks_rejected_version: u64 = 0;

                        // Process sync blocks - buffer future blocks, apply sequential ones
                        for block in blocks {
                            // Check version policy first (P2P.F: Prove the F)
                            let version_info = config_clone.version_info(block.header.version);
                            if let Err(reason) =
                                config_clone.should_accept_version(block.header.version)
                            {
                                warn!(
                                    block_height = block.header.height,
                                    version = %version_info,
                                    reason = %reason,
                                    "sync block rejected: version policy"
                                );
                                blocks_rejected_version += 1;
                                continue;
                            }

                            let best_height = chain_clone.best_block_height().await;
                            let best_hash = chain_clone.best_block_hash().await;
                            let expected_height = best_height + 1;
                            let needed_fork_parent = {
                                let buffer = block_buffer_clone.read().await;
                                crate::sync_canonical::missing_fork_point_parent_hash(
                                    best_height,
                                    best_hash,
                                    buffer.get(&expected_height),
                                    buffer.get(&best_height),
                                )
                            };

                            if block.header.height > highest_received {
                                highest_received = block.header.height;
                            }

                            if needed_fork_parent
                                .is_some_and(|parent| block.header.hash() == parent)
                            {
                                warn!(
                                    block_height = block.header.height,
                                    parent_hash = ?needed_fork_parent,
                                    "buffering fork-point block matching buffered parent hash"
                                );
                                let mut buffer = block_buffer_clone.write().await;
                                buffer.insert(block.header.height, block);
                                continue;
                            }

                            // Same-height fork point from a heavier peer: always buffer (replace orphan).
                            if block.header.height == best_height
                                && block.header.hash() != best_hash
                            {
                                warn!(
                                    block_height = block.header.height,
                                    local_tip_hash = ?best_hash,
                                    incoming_hash = ?block.header.hash(),
                                    "fork-point block at tip height; buffering for reorg"
                                );
                                set_sync_health_state(
                                    &sync_health_state_clone,
                                    SyncHealthState::SuspectFork,
                                    "fork-point block received",
                                )
                                .await;
                                let mut buffer = block_buffer_clone.write().await;
                                buffer.insert(block.header.height, block);
                                continue;
                            }

                            if block.header.height == expected_height
                                && block.header.prev_hash == best_hash
                            {
                                let peer_key = hex::encode(peer_id);
                                let (peer_tip_h, peer_tip_hash_bytes, peer_tip_w) =
                                    peer_consensus_clone
                                        .get_peer_tip(&peer_key)
                                        .await
                                        .unwrap_or((0, [0u8; 32], 0));
                                let peer_tip_hash =
                                    coinject_core::Hash::from_bytes(peer_tip_hash_bytes);
                                let candidate_hash = block.header.hash();
                                match crate::sync_canonical::should_apply_sync_extension(
                                    &chain_clone,
                                    &candidate_hash,
                                    block.header.height,
                                    peer_tip_h,
                                    peer_tip_hash,
                                    peer_tip_w,
                                )
                                .await
                                {
                                    Ok(false) => {
                                        warn!(
                                            block_height = block.header.height,
                                            peer_id = %peer_key,
                                            peer_tip_height = peer_tip_h,
                                            peer_cumulative_work = peer_tip_w,
                                            "sync extension not on peer heavy chain; buffering for reorg"
                                        );
                                        peer_consensus_clone.note_incompatible_tip(&peer_key).await;
                                        set_sync_health_state(
                                            &sync_health_state_clone,
                                            SyncHealthState::SuspectFork,
                                            "incompatible sync extension from heavy peer",
                                        )
                                        .await;
                                        let mut buffer = block_buffer_clone.write().await;
                                        buffer.insert(block.header.height, block);
                                        continue;
                                    }
                                    Ok(true) => {}
                                    Err(e) => {
                                        warn!(
                                            error = %e,
                                            block_height = block.header.height,
                                            "sync extension check failed; buffering"
                                        );
                                        let mut buffer = block_buffer_clone.write().await;
                                        buffer.insert(block.header.height, block);
                                        continue;
                                    }
                                }

                                // Validate and store (skip age check for sync blocks)
                                let emission_w = if block.header.height == 0 {
                                    None
                                } else {
                                    match chain_clone
                                        .cumulative_work_at_tip_hash(block.header.prev_hash)
                                    {
                                        Ok(w) => Some(w),
                                        Err(_) => continue,
                                    }
                                };
                                if let Ok(()) = validator_clone.validate_block_with_options(
                                    &block,
                                    &best_hash,
                                    expected_height,
                                    true,
                                    emission_w,
                                ) {
                                    if let Ok(is_new_best) = chain_clone.store_block(&block).await {
                                        if is_new_best {
                                            peer_consensus_clone
                                                .clear_incompatible_tip_penalty(&peer_key)
                                                .await;
                                            blocks_applied += 1;
                                            debug!(block_height = block.header.height, version = %version_info, "sync block applied");

                                            // Update CPP network chain state
                                            let new_height = block.header.height;
                                            let new_hash = block.header.hash();
                                            let cum = chain_clone.best_cumulative_work().await;
                                            if let Err(e) = cpp_network_cmd_tx_for_events.send(
                                                CppNetworkCommand::UpdateChainState {
                                                    best_height: new_height,
                                                    best_hash: new_hash,
                                                    cumulative_work: cum,
                                                },
                                            ) {
                                                warn!(error = %e, "failed to update cpp network chain state");
                                            }

                                            match Self::apply_block_transactions(
                                                &block,
                                                &state_clone,
                                                &timelock_state_clone,
                                                &escrow_state_clone,
                                                &channel_state_clone,
                                                &trustline_state_clone,
                                                &dimensional_pool_state_clone,
                                                &marketplace_state_clone,
                                            ) {
                                                Ok(_) => {
                                                    CoinjectNode::retarget_header_pow_from_stored_block(
                                                        &miner_clone,
                                                        &chain_clone,
                                                        &block,
                                                    )
                                                    .await;
                                                }
                                                Err(e) => {
                                                    error!(block_height = block.header.height, error = %e, "failed to apply sync block transactions");
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    warn!(
                                        block_height = block.header.height,
                                        "block validation failed, buffering"
                                    );
                                    let mut buffer = block_buffer_clone.write().await;
                                    buffer.insert(block.header.height, block);
                                }
                            } else if block.header.height == expected_height {
                                // Same next height but different parent hash means we're seeing an alternate
                                // chain. Keep it buffered so fork resolution can compare and request the
                                // correct ancestor range instead of dropping the block as "already known".
                                warn!(
                                    block_height = block.header.height,
                                    expected_height,
                                    "next sync block has mismatched parent hash, buffering for fork resolution"
                                );
                                let mut buffer = block_buffer_clone.write().await;
                                buffer.insert(block.header.height, block);
                            } else if block.header.height > expected_height {
                                // Future block - buffer it for later
                                debug!(
                                    block_height = block.header.height,
                                    expected_height, "buffering future block"
                                );
                                let mut buffer = block_buffer_clone.write().await;
                                buffer.insert(block.header.height, block);
                            } else {
                                // Older-height blocks can still belong to an alternate branch that we
                                // need for complete-fork validation. If the local block at this height
                                // has a different hash, keep the incoming block in the fork buffer
                                // instead of dropping it as "already known".
                                let conflicting_historical_block = match chain_clone
                                    .get_block_by_height(block.header.height)
                                {
                                    Ok(Some(existing_block)) => {
                                        existing_block.header.hash() != block.header.hash()
                                    }
                                    Ok(None) => true,
                                    Err(e) => {
                                        warn!(
                                            block_height = block.header.height,
                                            error = %e,
                                            "failed to inspect local historical block, buffering incoming block"
                                        );
                                        true
                                    }
                                };

                                if conflicting_historical_block
                                    || (block.header.height == best_height
                                        && block.header.hash() != best_hash)
                                {
                                    warn!(
                                        block_height = block.header.height,
                                        best_height,
                                        "historical sync block conflicts with local chain, buffering for fork resolution"
                                    );
                                    let mut buffer = block_buffer_clone.write().await;
                                    buffer.insert(block.header.height, block);
                                } else {
                                    debug!(
                                        block_height = block.header.height,
                                        best_height, "skipping already-known block"
                                    );
                                }
                            }
                        }

                        // Process any buffered blocks that might now be sequential
                        loop {
                            let best_height = chain_clone.best_block_height().await;
                            let best_hash = chain_clone.best_block_hash().await;
                            let next_height = best_height + 1;

                            let block_opt = {
                                let mut buffer = block_buffer_clone.write().await;
                                buffer.remove(&next_height)
                            };

                            match block_opt {
                                Some(block) => {
                                    // Check version policy for buffered blocks (P2P.F: Prove the F)
                                    let buffer_version_info =
                                        config_clone.version_info(block.header.version);
                                    if let Err(reason) =
                                        config_clone.should_accept_version(block.header.version)
                                    {
                                        warn!(
                                            block_height = block.header.height,
                                            version = %buffer_version_info,
                                            reason = %reason,
                                            "buffered block rejected: version policy"
                                        );
                                        blocks_rejected_version += 1;
                                        continue;
                                    }

                                    if block.header.prev_hash == best_hash {
                                        let emission_w = if block.header.height == 0 {
                                            None
                                        } else {
                                            match chain_clone
                                                .cumulative_work_at_tip_hash(block.header.prev_hash)
                                            {
                                                Ok(w) => Some(w),
                                                Err(_) => continue,
                                            }
                                        };
                                        if let Ok(()) = validator_clone.validate_block_with_options(
                                            &block,
                                            &best_hash,
                                            next_height,
                                            true,
                                            emission_w,
                                        ) {
                                            if let Ok(is_new_best) =
                                                chain_clone.store_block(&block).await
                                            {
                                                if is_new_best {
                                                    blocks_applied += 1;
                                                    debug!(block_height = block.header.height, version = %buffer_version_info, "buffered block applied");

                                                    let new_height = block.header.height;
                                                    let new_hash = block.header.hash();
                                                    let cum =
                                                        chain_clone.best_cumulative_work().await;
                                                    let _ = cpp_network_cmd_tx_for_events.send(
                                                        CppNetworkCommand::UpdateChainState {
                                                            best_height: new_height,
                                                            best_hash: new_hash,
                                                            cumulative_work: cum,
                                                        },
                                                    );
                                                    if Self::apply_block_transactions(
                                                        &block,
                                                        &state_clone,
                                                        &timelock_state_clone,
                                                        &escrow_state_clone,
                                                        &channel_state_clone,
                                                        &trustline_state_clone,
                                                        &dimensional_pool_state_clone,
                                                        &marketplace_state_clone,
                                                    )
                                                    .is_ok()
                                                    {
                                                        CoinjectNode::retarget_header_pow_from_stored_block(
                                                            &miner_clone,
                                                            &chain_clone,
                                                            &block,
                                                        )
                                                        .await;
                                                    }
                                                }
                                            }
                                        }
                                    } else {
                                        // Put it back - wrong prev_hash
                                        let mut buffer = block_buffer_clone.write().await;
                                        buffer.insert(block.header.height, block);
                                        break;
                                    }
                                }
                                None => break,
                            }
                        }

                        // Check if we need to request more blocks (continuation)
                        let current_height = chain_clone.best_block_height().await;
                        let peer_height = peer_consensus_clone
                            .get_peer_height(&hex::encode(peer_id))
                            .await
                            .unwrap_or(0);

                        info!(
                            blocks_applied,
                            blocks_rejected_version, current_height, peer_height, "sync progress"
                        );

                        let expected_height = current_height + 1;
                        let current_hash = chain_clone.best_block_hash().await;
                        let needs_fork_point = {
                            let buffer = block_buffer_clone.read().await;
                            crate::sync_canonical::missing_fork_point_parent_hash(
                                current_height,
                                current_hash,
                                buffer.get(&expected_height),
                                buffer.get(&current_height),
                            )
                            .is_some()
                        };
                        if needs_fork_point {
                            warn!(
                                fork_height = current_height,
                                expected_height,
                                "buffered extension parents off missing fork-point block; requesting tip height from heaviest peer"
                            );
                            request_fork_point_block_from_heaviest_peer(
                                &chain_clone,
                                &peer_consensus_clone,
                                &cpp_network_cmd_tx_for_events,
                                current_height,
                            )
                            .await;
                        }

                        if blocks_applied == 0 && highest_received >= expected_height {
                            warn!(
                                peer_id = %hex::encode(peer_id),
                                current_height,
                                expected_height,
                                highest_received,
                                "sync batch stalled on alternate branch; triggering immediate reorg evaluation"
                            );
                            set_sync_health_state(
                                &sync_health_state_clone,
                                SyncHealthState::Recovery,
                                "sync batch stalled and reorg triggered",
                            )
                            .await;
                            Self::check_and_reorganize_chain(
                                &chain_clone,
                                &state_clone,
                                &timelock_state_clone,
                                &escrow_state_clone,
                                &channel_state_clone,
                                &trustline_state_clone,
                                &dimensional_pool_state_clone,
                                &marketplace_state_clone,
                                &validator_clone,
                                &block_buffer_clone,
                                Some(&network_cmd_tx_for_events),
                                Some(&cpp_network_cmd_tx_for_events),
                                &peer_consensus_clone,
                            )
                            .await;

                            // After reorg attempt, schedule a delayed sync continuation so we re-request
                            // blocks instead of waiting only for the next periodic tick. Use the max of
                            // tracker height and this batch's highest block so we still request when
                            // peer_height is briefly stale. The delay lets in-flight reorg work finish
                            // and avoids tight-looping the bootnode rate limiter.
                            let effective_peer_tip = peer_height.max(highest_received);
                            if effective_peer_tip > current_height {
                                let chain_delayed = Arc::clone(&chain_clone);
                                let peer_consensus_delayed = Arc::clone(&peer_consensus_clone);
                                let sync_health_delayed = Arc::clone(&sync_health_state_clone);
                                let cpp_tx = cpp_network_cmd_tx_for_events.clone();
                                tokio::spawn(async move {
                                    tokio::time::sleep(Duration::from_secs(5)).await;
                                    request_recovery_sync_from_heaviest_peer(
                                        &chain_delayed,
                                        &peer_consensus_delayed,
                                        &sync_health_delayed,
                                        &cpp_tx,
                                    )
                                    .await;
                                });
                            }
                        }

                        if peer_height > current_height {
                            if blocks_applied > 0 {
                                set_sync_health_state(
                                    &sync_health_state_clone,
                                    SyncHealthState::Normal,
                                    "sync batch progressed",
                                )
                                .await;
                                // Only continue immediately when the last batch advanced our tip.
                                let (peer_tip_h, peer_tip_hash_bytes, peer_tip_w) =
                                    peer_consensus_clone
                                        .get_peer_tip(&hex::encode(peer_id))
                                        .await
                                        .unwrap_or((peer_height, [0u8; 32], 0));
                                request_hash_anchored_sync(
                                    &chain_clone,
                                    &peer_consensus_clone,
                                    &sync_health_state_clone,
                                    &cpp_network_cmd_tx_for_events,
                                    hash_anchored_sync_req(
                                        peer_id,
                                        peer_tip_h.max(peer_height),
                                        coinject_core::Hash::from_bytes(peer_tip_hash_bytes),
                                        peer_tip_w,
                                    ),
                                )
                                .await;
                            } else {
                                warn!(
                                    peer_id = %hex::encode(peer_id),
                                    current_height,
                                    peer_height,
                                    highest_received,
                                    "sync batch made no progress; requesting hash-anchored fork recovery"
                                );
                                if *sync_health_state_clone.read().await == SyncHealthState::Normal
                                {
                                    set_sync_health_state(
                                        &sync_health_state_clone,
                                        SyncHealthState::SuspectFork,
                                        "sync continuation made no progress",
                                    )
                                    .await;
                                }
                                request_recovery_sync_from_heaviest_peer(
                                    &chain_clone,
                                    &peer_consensus_clone,
                                    &sync_health_state_clone,
                                    &cpp_network_cmd_tx_for_events,
                                )
                                .await;
                            }
                        }
                    }
                    CppNetworkEvent::PeerConnected {
                        peer_id,
                        addr,
                        node_type: _,
                        best_height,
                        best_hash,
                    } => {
                        info!(peer_id = %hex::encode(peer_id), addr = %addr, "peer connected");
                        let peer_key = hex::encode(peer_id);
                        {
                            let mut dir = peer_directory_clone.write().await;
                            dir.insert(
                                peer_key.clone(),
                                NetworkPeerInfo {
                                    peer_id: peer_key,
                                    address: addr.to_string(),
                                    best_height,
                                    best_hash: format!("{}", best_hash),
                                    cumulative_work: "0".to_string(),
                                },
                            );
                        }
                        // Update peer count
                        {
                            let mut count = peer_count_clone.write().await;
                            *count += 1;
                            debug!(peer_count = *count, "peer count updated");
                        }
                        // Update peer consensus tracker
                        let peer_id_str = hex::encode(peer_id);
                        let best_hash_bytes: [u8; 32] = *best_hash.as_bytes();
                        peer_consensus_clone
                            .update_peer(peer_id_str, best_height, best_hash_bytes, 0)
                            .await;
                        // Update best known peer height
                        {
                            let mut best_height_guard = best_known_peer_height_clone.write().await;
                            if best_height > *best_height_guard {
                                *best_height_guard = best_height;
                            }
                        }
                        // Use peer consensus mathematics to determine if we need to sync
                        // This uses median height and adaptive thresholds (COINjecture consensus framework)
                        let current_height = chain_clone.best_block_height().await;
                        let median_height = peer_consensus_clone.median_peer_height().await;
                        let sync_threshold = peer_consensus_clone.sync_threshold_blocks();

                        // Check if we're behind the median peer height by more than sync_threshold
                        // This is more robust than checking individual peer heights
                        if current_height + sync_threshold < median_height {
                            let blocks_behind = median_height - current_height;
                            info!(
                                blocks_behind,
                                current_height,
                                median_height,
                                sync_threshold,
                                "behind median peer height, requesting hash-anchored sync blocks"
                            );
                            request_hash_anchored_sync(
                                &chain_clone,
                                &peer_consensus_clone,
                                &sync_health_state_clone,
                                &cpp_network_cmd_tx_for_events,
                                hash_anchored_sync_req(
                                    peer_id,
                                    best_height.max(median_height),
                                    best_hash,
                                    0,
                                ),
                            )
                            .await;
                        } else if best_height > current_height {
                            debug!(
                                peer_height = best_height,
                                current_height,
                                "peer is ahead, requesting hash-anchored sync blocks"
                            );
                            request_hash_anchored_sync(
                                &chain_clone,
                                &peer_consensus_clone,
                                &sync_health_state_clone,
                                &cpp_network_cmd_tx_for_events,
                                hash_anchored_sync_req(peer_id, best_height, best_hash, 0),
                            )
                            .await;
                        }
                    }
                    CppNetworkEvent::PeerDisconnected { peer_id, reason: _ } => {
                        info!(peer_id = %hex::encode(peer_id), "peer disconnected");
                        {
                            let mut dir = peer_directory_clone.write().await;
                            dir.remove(&hex::encode(peer_id));
                        }
                        // Update peer count
                        {
                            let mut count = peer_count_clone.write().await;
                            if *count > 0 {
                                *count -= 1;
                            }
                            debug!(peer_count = *count, "peer count updated");
                        }
                        // Mark peer as disconnected in consensus tracker
                        let peer_id_str = hex::encode(peer_id);
                        peer_consensus_clone
                            .mark_peer_disconnected(&peer_id_str)
                            .await;
                    }
                    CppNetworkEvent::StatusUpdate {
                        peer_id,
                        best_height,
                        best_hash,
                        node_type: _node_type,
                        cumulative_work,
                    } => {
                        debug!(peer_id = %hex::encode(peer_id), best_height, best_hash = ?best_hash, cumulative_work, "status update received");

                        {
                            let peer_key = hex::encode(peer_id);
                            let mut dir = peer_directory_clone.write().await;
                            let entry = dir.entry(peer_key.clone()).or_insert(NetworkPeerInfo {
                                peer_id: peer_key.clone(),
                                address: String::new(),
                                best_height: 0,
                                best_hash: String::new(),
                                cumulative_work: "0".to_string(),
                            });
                            entry.best_height = best_height;
                            entry.best_hash = format!("{}", best_hash);
                            entry.cumulative_work = format!("{}", cumulative_work);
                        }

                        // Update peer consensus tracker
                        let peer_id_str = hex::encode(peer_id);
                        let hash_bytes: [u8; 32] = *best_hash.as_bytes();
                        peer_consensus_clone
                            .update_peer(peer_id_str, best_height, hash_bytes, cumulative_work)
                            .await;

                        // Update best known peer height
                        {
                            let mut best_height_guard = best_known_peer_height_clone.write().await;
                            if best_height > *best_height_guard {
                                *best_height_guard = best_height;
                                debug!(
                                    best_known_peer_height = best_height,
                                    "best known peer height updated"
                                );
                            }
                        }

                        // Trigger sync on StatusUpdate if peer is ahead
                        let current_height = chain_clone.best_block_height().await;
                        if best_height > current_height {
                            request_hash_anchored_sync(
                                &chain_clone,
                                &peer_consensus_clone,
                                &sync_health_state_clone,
                                &cpp_network_cmd_tx_for_events,
                                hash_anchored_sync_req(
                                    peer_id,
                                    best_height,
                                    best_hash,
                                    cumulative_work,
                                ),
                            )
                            .await;
                        } else {
                            debug!(
                                peer_height = best_height,
                                current_height, "in sync with peer"
                            );
                        }
                    }
                    _ => {
                        // Handle other events as needed
                    }
                }
            }
        });

        // ─── Mesh Network (optional parallel transport) ────────────────────
        if self.config.enable_mesh {
            info!("starting mesh network");

            let mesh_listen_addr: std::net::SocketAddr = self
                .config
                .mesh_listen
                .parse()
                .map_err(|e| format!("Invalid mesh listen address: {}", e))?;

            let mut mesh_seeds: Vec<std::net::SocketAddr> = Vec::new();
            for seed_str in &self.config.mesh_seed {
                match seed_str.parse::<std::net::SocketAddr>() {
                    Ok(addr) => mesh_seeds.push(addr),
                    Err(e) => warn!(addr = %seed_str, error = %e, "invalid mesh seed address"),
                }
            }

            let mesh_data_dir = self.config.data_dir.join("mesh");
            let mesh_config = coinject_network::MeshNetworkConfig {
                listen_addr: mesh_listen_addr,
                seed_nodes: mesh_seeds,
                data_dir: mesh_data_dir,
                ..Default::default()
            };

            match coinject_network::NetworkService::start(mesh_config).await {
                Ok((mesh_service, mesh_event_rx)) => {
                    let mesh_cmd_tx = mesh_service.command_sender();

                    // Create bridge channels
                    let (bridge_cmd_tx, bridge_cmd_rx) =
                        mpsc::unbounded_channel::<coinject_network::MeshBridgeCommand>();
                    let (bridge_event_tx, mut bridge_event_rx) =
                        mpsc::unbounded_channel::<coinject_network::MeshBridgeEvent>();

                    // Create bridge state with current chain tip
                    let bridge_state = Arc::new(RwLock::new(coinject_network::MeshBridgeState {
                        best_height: current_height,
                        best_hash: current_hash,
                        epoch: 0,
                    }));

                    // Spawn the bridge task
                    let bridge_state_clone = Arc::clone(&bridge_state);
                    tokio::spawn(async move {
                        coinject_network::mesh::bridge::run_bridge(
                            bridge_cmd_rx,
                            bridge_event_tx,
                            mesh_cmd_tx,
                            mesh_event_rx,
                            bridge_state_clone,
                        )
                        .await;
                    });

                    // Forward mined blocks to mesh (clone of bridge_cmd_tx for mining)
                    let _bridge_cmd_tx_for_mining = bridge_cmd_tx.clone();
                    let _bridge_cmd_tx_for_events = bridge_cmd_tx.clone();

                    // ── Epoch Coordinator (optional, alongside mesh) ─────
                    // Create coordinator channels
                    let (coord_cmd_tx, coord_cmd_rx) =
                        mpsc::unbounded_channel::<coinject_consensus::CoordinatorCommand>();
                    let (coord_event_tx, mut coord_event_rx) =
                        mpsc::unbounded_channel::<coinject_consensus::CoordinatorEvent>();

                    // Use mesh node identity as coordinator ID
                    let coord_node_id: [u8; 32] = mesh_service.local_id().0;

                    let coord_config = coinject_consensus::CoordinatorConfig::default();
                    let (coordinator, _coord_shared_state) =
                        coinject_consensus::EpochCoordinator::new(
                            coord_node_id,
                            coord_config,
                            current_height,
                            current_hash,
                            current_tip_cumulative_work,
                        );

                    // Spawn coordinator task
                    tokio::spawn(async move {
                        coordinator.run(coord_cmd_rx, coord_event_tx).await;
                    });
                    tracing::info!("epoch coordinator started");

                    // Clone coordinator command sender for the bridge event handler
                    let _coord_cmd_for_bridge = coord_cmd_tx.clone();

                    // Spawn coordinator event handler — translates coordinator events
                    // into bridge commands (outbound consensus messages)
                    let bridge_cmd_for_coord = bridge_cmd_tx.clone();
                    tokio::spawn(async move {
                        while let Some(event) = coord_event_rx.recv().await {
                            match event {
                                coinject_consensus::CoordinatorEvent::BroadcastSalt {
                                    epoch,
                                    salt,
                                } => {
                                    tracing::info!(
                                        epoch,
                                        "coordinator: broadcasting salt via mesh"
                                    );
                                    let _ = bridge_cmd_for_coord.send(
                                        coinject_network::MeshBridgeCommand::BroadcastConsensusSalt { epoch, salt }
                                    );
                                }
                                coinject_consensus::CoordinatorEvent::BroadcastCommit {
                                    epoch,
                                    solution_hash,
                                    work_score,
                                } => {
                                    tracing::info!(
                                        epoch,
                                        "coordinator: broadcasting commit via mesh"
                                    );
                                    let _ = bridge_cmd_for_coord.send(
                                        coinject_network::MeshBridgeCommand::BroadcastCommit {
                                            epoch,
                                            solution_hash,
                                            node_id: coord_node_id,
                                            work_score,
                                            signature: Vec::new(),
                                        },
                                    );
                                }
                                coinject_consensus::CoordinatorEvent::EpochStarted {
                                    epoch,
                                    leader,
                                    ..
                                } => {
                                    tracing::info!(
                                        epoch,
                                        leader = hex::encode(&leader[..4]),
                                        "coordinator: epoch started"
                                    );
                                }
                                coinject_consensus::CoordinatorEvent::MinePhaseStarted {
                                    epoch,
                                    ..
                                } => {
                                    tracing::info!(epoch, "coordinator: mine phase started");
                                }
                                coinject_consensus::CoordinatorEvent::CommitPhaseStarted {
                                    epoch,
                                } => {
                                    tracing::info!(epoch, "coordinator: commit phase started");
                                }
                                coinject_consensus::CoordinatorEvent::EpochSealed {
                                    epoch,
                                    winner,
                                    work_score,
                                    commit_count,
                                } => {
                                    tracing::info!(
                                        epoch,
                                        winner = hex::encode(&winner[..4]),
                                        work_score,
                                        commit_count,
                                        "coordinator: epoch sealed"
                                    );
                                }
                                coinject_consensus::CoordinatorEvent::EpochStalled {
                                    epoch,
                                    phase,
                                    reason,
                                } => {
                                    tracing::warn!(epoch, phase = %phase, reason = %reason, "coordinator: epoch stalled");
                                }
                                coinject_consensus::CoordinatorEvent::BlockProduced {
                                    block,
                                    epoch,
                                } => {
                                    let block_hash = block.header.hash();
                                    let height = block.header.height;
                                    tracing::info!(
                                        epoch,
                                        height,
                                        hash = hex::encode(&block_hash.as_bytes()[..4]),
                                        "coordinator: block produced, broadcasting via mesh"
                                    );

                                    // Broadcast the block via mesh bridge
                                    let _ = bridge_cmd_for_coord.send(
                                        coinject_network::mesh::bridge::BridgeCommand::BroadcastBlock { block },
                                    );
                                }
                            }
                        }
                        tracing::info!("coordinator event handler exited");
                    });

                    // Spawn mesh bridge event handler — feeds blocks/txs into the same
                    // validation pipeline as CPP events
                    let chain_for_mesh = Arc::clone(&self.chain);
                    let state_for_mesh = Arc::clone(&self.state);
                    let timelock_for_mesh = Arc::clone(&self.timelock_state);
                    let escrow_for_mesh = Arc::clone(&self.escrow_state);
                    let channel_for_mesh = Arc::clone(&self.channel_state);
                    let trustline_for_mesh = Arc::clone(&self.trustline_state);
                    let dim_pool_for_mesh = Arc::clone(&self.dimensional_pool_state);
                    let marketplace_for_mesh = Arc::clone(&self.marketplace_state);
                    let validator_for_mesh = Arc::clone(&self.validator);
                    let tx_pool_for_mesh = Arc::clone(&self.tx_pool);
                    let cpp_cmd_for_mesh = cpp_network_cmd_tx.clone();
                    let bridge_state_for_events = Arc::clone(&bridge_state);
                    let coord_cmd_for_events = coord_cmd_tx.clone();
                    let miner_for_mesh = self.miner.clone();

                    tokio::spawn(async move {
                        while let Some(event) = bridge_event_rx.recv().await {
                            match event {
                                coinject_network::MeshBridgeEvent::BlockReceived {
                                    block,
                                    peer_id,
                                } => {
                                    let height = block.header.height;
                                    tracing::info!(height, peer = %peer_id.short(), "mesh: block received");

                                    let best_height = chain_for_mesh.best_block_height().await;
                                    let best_hash = chain_for_mesh.best_block_hash().await;
                                    let expected_height = best_height + 1;

                                    if height != expected_height {
                                        tracing::debug!(
                                            height,
                                            expected_height,
                                            "mesh: unexpected block height"
                                        );
                                        continue;
                                    }

                                    if block.header.prev_hash != best_hash {
                                        tracing::debug!(height, "mesh: prev_hash mismatch");
                                        continue;
                                    }

                                    let emission_w = if block.header.height == 0 {
                                        None
                                    } else {
                                        match chain_for_mesh
                                            .cumulative_work_at_tip_hash(block.header.prev_hash)
                                        {
                                            Ok(w) => Some(w),
                                            Err(e) => {
                                                tracing::warn!(height, error = %e, "mesh: emission parent work");
                                                continue;
                                            }
                                        }
                                    };

                                    match validator_for_mesh.validate_block_with_options(
                                        &block,
                                        &best_hash,
                                        expected_height,
                                        false,
                                        emission_w,
                                    ) {
                                        Ok(()) => {
                                            match chain_for_mesh.store_block(&block).await {
                                                Ok(is_new_best) => {
                                                    if is_new_best {
                                                        tracing::info!(
                                                            height,
                                                            "mesh: block stored as new best"
                                                        );
                                                        let new_hash = block.header.hash();

                                                        // Apply state transitions
                                                        if let Err(e) =
                                                            CoinjectNode::apply_block_transactions(
                                                                &block,
                                                                &state_for_mesh,
                                                                &timelock_for_mesh,
                                                                &escrow_for_mesh,
                                                                &channel_for_mesh,
                                                                &trustline_for_mesh,
                                                                &dim_pool_for_mesh,
                                                                &marketplace_for_mesh,
                                                            )
                                                        {
                                                            tracing::warn!(height, error = %e, "mesh: apply txs failed");
                                                        } else {
                                                            CoinjectNode::retarget_header_pow_from_stored_block(
                                                                &miner_for_mesh,
                                                                &chain_for_mesh,
                                                                &block,
                                                            )
                                                            .await;
                                                            // Remove applied txs from pool
                                                            let mut pool =
                                                                tx_pool_for_mesh.write().await;
                                                            for tx in &block.transactions {
                                                                pool.remove(&tx.hash());
                                                            }
                                                        }

                                                        // Update bridge state
                                                        let mut bs =
                                                            bridge_state_for_events.write().await;
                                                        bs.best_height = height;
                                                        bs.best_hash = new_hash;

                                                        // Update CPP with new chain state
                                                        let cum = chain_for_mesh
                                                            .best_cumulative_work()
                                                            .await;
                                                        let _ = cpp_cmd_for_mesh.send(
                                                            CppNetworkCommand::UpdateChainState {
                                                                best_height: height,
                                                                best_hash: new_hash,
                                                                cumulative_work: cum,
                                                            },
                                                        );

                                                        // Update coordinator chain tip
                                                        let _ = coord_cmd_for_events.send(
                                                            coinject_consensus::CoordinatorCommand::ChainTipUpdated {
                                                                height,
                                                                hash: new_hash,
                                                                best_cumulative_work: cum,
                                                            }
                                                        );
                                                    }
                                                }
                                                Err(e) => {
                                                    tracing::warn!(height, error = %e, "mesh: block store failed");
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            tracing::warn!(height, error = ?e, "mesh: block validation failed");
                                        }
                                    }
                                }
                                coinject_network::MeshBridgeEvent::TransactionReceived {
                                    transaction,
                                    peer_id,
                                } => {
                                    tracing::debug!(peer = %peer_id.short(), "mesh: transaction received");
                                    let mut pool = tx_pool_for_mesh.write().await;
                                    let _ = pool.add(transaction);
                                }
                                coinject_network::MeshBridgeEvent::PeerConnected {
                                    peer_id,
                                    best_height,
                                    ..
                                } => {
                                    tracing::info!(peer = %peer_id.short(), best_height, "mesh: peer connected");
                                    let _ = coord_cmd_for_events.send(
                                        coinject_consensus::CoordinatorCommand::PeerJoined {
                                            node_id: peer_id.0,
                                        },
                                    );
                                }
                                coinject_network::MeshBridgeEvent::PeerDisconnected {
                                    peer_id,
                                    reason,
                                } => {
                                    tracing::info!(peer = %peer_id.short(), reason = %reason, "mesh: peer disconnected");
                                    let _ = coord_cmd_for_events.send(
                                        coinject_consensus::CoordinatorCommand::PeerLeft {
                                            node_id: peer_id.0,
                                        },
                                    );
                                }
                                coinject_network::MeshBridgeEvent::StatusUpdate {
                                    peer_id,
                                    best_height,
                                    ..
                                } => {
                                    tracing::debug!(peer = %peer_id.short(), best_height, "mesh: status update");
                                }
                                coinject_network::MeshBridgeEvent::BlocksReceived {
                                    blocks,
                                    request_id,
                                    peer_id,
                                } => {
                                    tracing::info!(
                                        count = blocks.len(), request_id,
                                        peer = %peer_id.short(), "mesh: sync blocks received"
                                    );
                                    for block in blocks {
                                        let height = block.header.height;
                                        let best_hash = chain_for_mesh.best_block_hash().await;
                                        let expected = chain_for_mesh.best_block_height().await + 1;
                                        let emission_w = if block.header.height == 0 {
                                            None
                                        } else {
                                            match chain_for_mesh
                                                .cumulative_work_at_tip_hash(block.header.prev_hash)
                                            {
                                                Ok(w) => Some(w),
                                                Err(_) => continue,
                                            }
                                        };
                                        match validator_for_mesh.validate_block_with_options(
                                            &block, &best_hash, expected, false, emission_w,
                                        ) {
                                            Ok(()) => {
                                                match chain_for_mesh.store_block(&block).await {
                                                    Ok(is_new_best) => {
                                                        if is_new_best {
                                                            if let Err(e) = CoinjectNode::apply_block_transactions(
                                                                &block,
                                                                &state_for_mesh,
                                                                &timelock_for_mesh,
                                                                &escrow_for_mesh,
                                                                &channel_for_mesh,
                                                                &trustline_for_mesh,
                                                                &dim_pool_for_mesh,
                                                                &marketplace_for_mesh,
                                                            ) {
                                                                tracing::warn!(height, error = %e, "mesh: sync apply failed");
                                                            } else {
                                                                CoinjectNode::retarget_header_pow_from_stored_block(
                                                                    &miner_for_mesh,
                                                                    &chain_for_mesh,
                                                                    &block,
                                                                )
                                                                .await;
                                                            }
                                                        }
                                                    }
                                                    Err(e) => {
                                                        tracing::warn!(height, error = %e, "mesh: sync store failed")
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                tracing::warn!(height, error = ?e, "mesh: sync block invalid")
                                            }
                                        }
                                    }
                                }
                                // ── Consensus payloads → Coordinator ─────────────
                                coinject_network::MeshBridgeEvent::ConsensusSaltReceived {
                                    epoch,
                                    salt,
                                    from,
                                } => {
                                    tracing::debug!(epoch, peer = %from.short(), "mesh: consensus salt received");
                                    let _ = coord_cmd_for_events.send(
                                        coinject_consensus::CoordinatorCommand::SaltReceived {
                                            epoch,
                                            salt,
                                            from: from.0,
                                        },
                                    );
                                }
                                coinject_network::MeshBridgeEvent::ConsensusCommitReceived {
                                    epoch,
                                    block_hash: _,
                                    commits,
                                    from,
                                } => {
                                    tracing::debug!(epoch, peer = %from.short(), "mesh: consensus commit received");
                                    for commit in commits {
                                        let _ = coord_cmd_for_events.send(
                                            coinject_consensus::CoordinatorCommand::CommitReceived {
                                                epoch,
                                                commit: coinject_consensus::SolutionCommit {
                                                    node_id: commit.node_id.0,
                                                    solution_hash: commit.solution_hash,
                                                    work_score: commit.work_score,
                                                    signature: commit.signature,
                                                    // Migration default: network peers that have not yet
                                                    // upgraded to include their public key send all-zeros.
                                                    // The commit collector accepts these during the transition
                                                    // window (all-zero pubkey bypasses signature verification).
                                                    public_key: [0u8; 32],
                                                },
                                            }
                                        );
                                    }
                                }
                            }
                        }
                        tracing::info!("mesh bridge event handler exited");
                    });

                    // Also forward mined blocks to mesh alongside CPP
                    // This piggybacks on the existing mining broadcast by cloning to bridge
                    // The bridge_cmd_tx is stored for use in the mining loop
                    info!(
                        listen = %self.config.mesh_listen,
                        seeds = ?self.config.mesh_seed,
                        "mesh network listening, epoch coordinator active"
                    );
                }
                Err(e) => {
                    warn!(error = %e, "failed to start mesh network, continuing with cpp-only transport");
                }
            }
        }

        // Initialize WebSocket RPC
        let ws_addr: std::net::SocketAddr = self
            .config
            .cpp_ws_addr
            .parse()
            .map_err(|e| format!("Invalid WebSocket address: {}", e))?;

        let (websocket_rpc, websocket_rpc_cmd_tx, mut websocket_rpc_event_rx) =
            WebSocketRpc::new(ws_addr);

        // Spawn WebSocket RPC task
        let ws_addr_clone = self.config.cpp_ws_addr.clone();
        tokio::spawn(async move {
            info!("websocket rpc task starting");
            match websocket_rpc.start().await {
                Ok(()) => {
                    info!("websocket rpc task completed");
                }
                Err(e) => {
                    error!(error = %e, addr = %ws_addr_clone, "websocket rpc error");
                }
            }
        });

        // Spawn WebSocket RPC event handler
        let tx_pool_clone2 = Arc::clone(&self.tx_pool);
        tokio::spawn(async move {
            while let Some(event) = websocket_rpc_event_rx.recv().await {
                match event {
                    WebSocketRpcEvent::WorkSubmitted {
                        client_id: _,
                        work_id: _,
                        solution: _,
                        nonce: _,
                    } => {
                        // TODO: Validate and process PoW submission
                        debug!("websocket rpc: work submission received");
                    }
                    WebSocketRpcEvent::TransactionSubmitted {
                        transaction,
                        client_id: _,
                    } => {
                        // TODO: Add transaction to pool
                        let mut pool = tx_pool_clone2.write().await;
                        let _ = pool.add(*transaction);
                    }
                    _ => {
                        // Handle other events
                    }
                }
            }
        });

        self.cpp_network_cmd_tx = Some(cpp_network_cmd_tx_for_storage);
        self.websocket_rpc_cmd_tx = Some(websocket_rpc_cmd_tx);

        info!(
            cpp_p2p_addr = %self.config.cpp_p2p_addr,
            ws_addr = %self.config.cpp_ws_addr,
            "node is ready"
        );

        // libp2p network task removed - using CPP protocol only
        // Legacy NetworkCommand channel kept for compatibility but commands are routed to CPP network
        let cpp_network_cmd_tx_for_legacy = cpp_network_cmd_tx.clone();
        let peer_consensus_for_legacy = Arc::clone(&peer_consensus);
        let mut network_cmd_rx_for_legacy = _network_cmd_rx;
        tokio::spawn(async move {
            while let Some(cmd) = network_cmd_rx_for_legacy.recv().await {
                // Route legacy NetworkCommand to CPP network
                match cmd {
                    NetworkCommand::BroadcastBlock(block) => {
                        debug!(
                            block_height = block.header.height,
                            "forwarding broadcast block to cpp"
                        );
                        // Route to CPP network
                        if let Err(e) = cpp_network_cmd_tx_for_legacy
                            .send(CppNetworkCommand::BroadcastBlock { block })
                        {
                            error!(error = %e, "failed to broadcast block via cpp");
                        }
                    }
                    NetworkCommand::BroadcastTransaction(tx) => {
                        // Route to CPP network
                        if let Err(e) = cpp_network_cmd_tx_for_legacy
                            .send(CppNetworkCommand::BroadcastTransaction { transaction: tx })
                        {
                            error!(error = %e, "failed to broadcast transaction via cpp");
                        }
                    }
                    NetworkCommand::RequestBlocks {
                        from_height,
                        to_height,
                    } => {
                        // Fork/sync gap-fill and block_processing used to send this into a dead-end.
                        // Route to CPP using the same peer pick as fork recovery: heaviest chain work, then height.
                        let active_peers = peer_consensus_for_legacy.active_peers().await;
                        if active_peers.is_empty() {
                            warn!(
                                from_height,
                                to_height,
                                "legacy RequestBlocks: no active CPP peers; will retry when mesh connects"
                            );
                            continue;
                        }
                        if let Some((peer_id_str, peer_state)) =
                            active_peers.iter().max_by(|(_, a), (_, b)| {
                                a.cumulative_work
                                    .cmp(&b.cumulative_work)
                                    .then_with(|| a.best_height.cmp(&b.best_height))
                            })
                        {
                            if let Ok(peer_id_bytes) = hex::decode(peer_id_str) {
                                if peer_id_bytes.len() == 32 {
                                    let mut peer_id = [0u8; 32];
                                    peer_id.copy_from_slice(&peer_id_bytes[..32]);
                                    let request_id: u64 = rand::random();
                                    debug!(
                                        from_height,
                                        to_height,
                                        peer = &peer_id_str[..peer_id_str.len().min(16)],
                                        peer_height = peer_state.best_height,
                                        "routing legacy RequestBlocks to cpp"
                                    );
                                    if let Err(e) = cpp_network_cmd_tx_for_legacy.send(
                                        CppNetworkCommand::RequestBlocks {
                                            peer_id,
                                            from_height,
                                            to_height,
                                            request_id,
                                        },
                                    ) {
                                        error!(
                                            error = %e,
                                            from_height,
                                            to_height,
                                            "failed to send cpp RequestBlocks from legacy channel"
                                        );
                                    }
                                } else {
                                    warn!(
                                        peer_len = peer_id_bytes.len(),
                                        "legacy RequestBlocks: bad peer id length"
                                    );
                                }
                            } else {
                                warn!(peer = %peer_id_str, "legacy RequestBlocks: peer id hex decode failed");
                            }
                        }
                    }
                    NetworkCommand::RequestBlocksRR {
                        peer,
                        from_height,
                        to_height,
                    } => {
                        let request_id: u64 = rand::random();
                        if let Err(e) =
                            cpp_network_cmd_tx_for_legacy.send(CppNetworkCommand::RequestBlocks {
                                peer_id: peer,
                                from_height,
                                to_height,
                                request_id,
                            })
                        {
                            error!(
                                error = %e,
                                from_height,
                                to_height,
                                "failed to send cpp RequestBlocks from RequestBlocksRR"
                            );
                        }
                    }
                    _ => {
                        // Other commands not yet implemented in CPP - log for now
                        warn!(cmd = ?cmd, "legacy network command not yet routed to cpp");
                    }
                }
            }
        });

        // TEMPORARY: Periodic status broadcast disabled (was libp2p)
        // CPP network handles status updates internally
        // TODO: Re-enable with CPP network status broadcasting

        // Spawn periodic reorganization check task (every 15 seconds)
        let chain_periodic = Arc::clone(&self.chain);
        let state_periodic = Arc::clone(&self.state);
        let timelock_periodic = Arc::clone(&self.timelock_state);
        let escrow_periodic = Arc::clone(&self.escrow_state);
        let channel_periodic = Arc::clone(&self.channel_state);
        let trustline_periodic = Arc::clone(&self.trustline_state);
        let dimensional_periodic = Arc::clone(&self.dimensional_pool_state);
        let marketplace_periodic = Arc::clone(&self.marketplace_state);
        let validator_periodic = Arc::clone(&self.validator);
        let buffer_periodic = Arc::clone(&block_buffer);
        let network_tx_periodic = network_cmd_tx.clone();
        let cpp_network_cmd_tx_periodic = cpp_network_cmd_tx.clone();
        let peer_consensus_periodic = Arc::clone(&peer_consensus);
        let is_syncing_periodic = Arc::clone(&is_syncing);
        let sync_health_state_periodic = Arc::clone(&sync_health_state);
        let mut suspect_fork_streak: u32 = 0;

        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(15));
            loop {
                interval.tick().await;
                debug!("periodic reorganization check triggered");
                Self::check_and_reorganize_chain(
                    &chain_periodic,
                    &state_periodic,
                    &timelock_periodic,
                    &escrow_periodic,
                    &channel_periodic,
                    &trustline_periodic,
                    &dimensional_periodic,
                    &marketplace_periodic,
                    &validator_periodic,
                    &buffer_periodic,
                    Some(&network_tx_periodic),
                    Some(&cpp_network_cmd_tx_periodic),
                    &peer_consensus_periodic,
                )
                .await;

                let cur = chain_periodic.best_block_height().await;
                let median = peer_consensus_periodic.median_peer_height().await;
                let best_ph = peer_consensus_periodic.best_peer_height().await;
                let thresh = peer_consensus_periodic.sync_threshold_blocks();
                let behind_median = cur.saturating_add(thresh) < median;
                let behind_best_peer = cur.saturating_add(thresh) < best_ph;
                let syncing = behind_median || behind_best_peer;
                *is_syncing_periodic.write().await = syncing;

                let health = *sync_health_state_periodic.read().await;
                if health == SyncHealthState::SuspectFork {
                    suspect_fork_streak = suspect_fork_streak.saturating_add(1);
                } else {
                    suspect_fork_streak = 0;
                }

                // After ~45s on SuspectFork, pull from the heaviest peer and re-run reorg.
                if suspect_fork_streak >= 3 {
                    warn!(
                        our_height = cur,
                        "auto fork recovery: requesting hash-anchored blocks from heaviest peer"
                    );
                    request_recovery_sync_from_heaviest_peer(
                        &chain_periodic,
                        &peer_consensus_periodic,
                        &sync_health_state_periodic,
                        &cpp_network_cmd_tx_periodic,
                    )
                    .await;
                    suspect_fork_streak = 0;
                }

                if syncing {
                    set_sync_health_state(
                        &sync_health_state_periodic,
                        SyncHealthState::Recovery,
                        "periodic detector found node behind peers",
                    )
                    .await;
                } else if health != SyncHealthState::SuspectFork {
                    set_sync_health_state(
                        &sync_health_state_periodic,
                        SyncHealthState::Normal,
                        "periodic detector sees node aligned with peers",
                    )
                    .await;
                }
            }
        });

        // Spawn periodic metrics update task
        let chain_for_metrics = Arc::clone(&self.chain);
        let _state_for_metrics = Arc::clone(&self.state);
        let dimensional_pool_state_for_metrics = Arc::clone(&self.dimensional_pool_state);
        let tx_pool_for_metrics = Arc::clone(&self.tx_pool);
        let node_classification_for_metrics = Arc::clone(&self.node_classification);

        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(15));
            loop {
                interval.tick().await;

                // Update blockchain metrics
                let block_height = chain_for_metrics.best_block_height().await;
                crate::metrics::BLOCK_HEIGHT.set(block_height as i64);

                // Update pool balance metrics (all 8 dimensional pools)
                use coinject_core::DimensionalPool;

                // Get unlock fractions and yield rates from consensus state
                let unlock_fractions = dimensional_pool_state_for_metrics.get_unlock_fractions();
                let yield_rates = dimensional_pool_state_for_metrics.get_yield_rates();

                for pool_id in 1..=8 {
                    // All 8 pools: D1-D8
                    let pool = match pool_id {
                        1 => DimensionalPool::D1,
                        2 => DimensionalPool::D2,
                        3 => DimensionalPool::D3,
                        4 => DimensionalPool::D4,
                        5 => DimensionalPool::D5,
                        6 => DimensionalPool::D6,
                        7 => DimensionalPool::D7,
                        8 => DimensionalPool::D8,
                        _ => continue,
                    };

                    if let Some(liquidity) =
                        dimensional_pool_state_for_metrics.get_pool_liquidity(&pool)
                    {
                        let dimension_label = format!("D{}", pool_id);

                        // Total balance
                        crate::metrics::POOL_BALANCE
                            .with_label_values(&[&dimension_label])
                            .set(liquidity.liquidity as f64);

                        // Locked liquidity (not yet unlocked)
                        crate::metrics::POOL_LOCKED
                            .with_label_values(&[&dimension_label])
                            .set(liquidity.locked_liquidity as f64);

                        // Unlocked liquidity (available for withdrawal/yields)
                        crate::metrics::POOL_UNLOCKED
                            .with_label_values(&[&dimension_label])
                            .set(liquidity.unlocked_liquidity as f64);

                        // Unlock fraction U_n(τ)
                        if let Some(ref fractions) = unlock_fractions {
                            crate::metrics::POOL_UNLOCK_FRACTION
                                .with_label_values(&[&dimension_label])
                                .set(fractions[pool_id - 1]);
                        }

                        // Yield rate r_n(τ)
                        if let Some(ref rates) = yield_rates {
                            crate::metrics::POOL_YIELD_RATE
                                .with_label_values(&[&dimension_label])
                                .set(rates[pool_id - 1]);
                        }
                    }
                }

                // Update mempool metrics
                let pool = tx_pool_for_metrics.read().await;
                let mempool_size = pool.len();
                drop(pool);
                crate::metrics::MEMPOOL_SIZE.set(mempool_size as i64);

                // RUNTIME INTEGRATION: Read actual consensus state instead of hard-coded constants
                // Calculate and update Satoshi constants (η and λ) from live network state
                use coinject_core::{ETA, LAMBDA, TAU_C};

                // Export live consensus state (τ, |ψ|, θ) from database
                if let Some(consensus_state) =
                    dimensional_pool_state_for_metrics.get_current_consensus_state()
                {
                    crate::metrics::CONSENSUS_TAU.set(consensus_state.tau);
                    crate::metrics::CONSENSUS_MAGNITUDE.set(consensus_state.magnitude);
                    crate::metrics::CONSENSUS_PHASE.set(consensus_state.phase);
                } else {
                    // Fallback if no consensus state saved yet (early blocks)
                    let tau = (block_height as f64) / TAU_C;
                    let magnitude = (-ETA * tau).exp();
                    let phase = LAMBDA * tau;
                    crate::metrics::CONSENSUS_TAU.set(tau);
                    crate::metrics::CONSENSUS_MAGNITUDE.set(magnitude);
                    crate::metrics::CONSENSUS_PHASE.set(phase);
                }

                // EMPIRICAL MEASUREMENT: Get measured η and λ from consensus metrics
                // These values are computed from actual work score exponential decay and timing coherence
                if let Some(metrics) = dimensional_pool_state_for_metrics.get_consensus_metrics() {
                    crate::metrics::MEASURED_ETA.set(metrics.measured_eta);
                    crate::metrics::MEASURED_LAMBDA.set(metrics.measured_lambda);
                    crate::metrics::CONVERGENCE_CONFIDENCE.set(metrics.convergence_confidence);
                    crate::metrics::MEASURED_ORACLE_DELTA.set(metrics.measured_oracle_delta);

                    // Calculate convergence errors
                    let eta_error = (metrics.measured_eta - ETA).abs();
                    let lambda_error = (metrics.measured_lambda - LAMBDA).abs();
                    crate::metrics::ETA_CONVERGENCE_ERROR.set(eta_error);
                    crate::metrics::LAMBDA_CONVERGENCE_ERROR.set(lambda_error);

                    // Update unit circle constraint: |μ|² = η² + λ² should equal 1
                    let constraint = metrics.measured_eta * metrics.measured_eta
                        + metrics.measured_lambda * metrics.measured_lambda;
                    crate::metrics::UNIT_CIRCLE_CONSTRAINT.set(constraint);

                    // Update damping coefficient: ζ = η/√2
                    let damping = metrics.measured_eta / std::f64::consts::SQRT_2;
                    crate::metrics::DAMPING_COEFFICIENT.set(damping);
                } else {
                    // Fallback to theoretical values until enough data collected
                    crate::metrics::MEASURED_ETA.set(ETA);
                    crate::metrics::MEASURED_LAMBDA.set(LAMBDA);
                    crate::metrics::CONVERGENCE_CONFIDENCE.set(0.0);
                    crate::metrics::MEASURED_ORACLE_DELTA.set(0.231); // Theoretical value
                    crate::metrics::ETA_CONVERGENCE_ERROR.set(0.0);
                    crate::metrics::LAMBDA_CONVERGENCE_ERROR.set(0.0);

                    let constraint = ETA * ETA + LAMBDA * LAMBDA;
                    crate::metrics::UNIT_CIRCLE_CONSTRAINT.set(constraint);

                    let damping = ETA / std::f64::consts::SQRT_2;
                    crate::metrics::DAMPING_COEFFICIENT.set(damping);
                }

                // ============================================================
                // NODE TYPE CLASSIFICATION METRICS UPDATE
                // ============================================================
                // Update classification manager with current chain height and metrics
                {
                    let mut classification = node_classification_for_metrics.write().await;

                    // Update chain height for classification calculations
                    classification.update_chain_height(block_height);

                    // Update storage tracking (blocks stored = chain height for Full nodes)
                    // In a real implementation, this would track actual blocks stored
                    classification.local_metrics.blocks_stored = block_height;

                    // Update uptime (seconds since start)
                    if let Some(started) = classification.local_metrics.observation_started {
                        let uptime_secs = started.elapsed().as_secs();
                        classification.update_uptime(uptime_secs, uptime_secs);
                    }

                    // Attempt reclassification if enough blocks have passed
                    if let Some(result) = classification.maybe_reclassify(block_height) {
                        // Log classification change
                        info!(
                            node_type = %result.node_type,
                            confidence_pct = result.confidence * 100.0,
                            reason = %result.reason,
                            "node reclassified"
                        );

                        // Update classification scores in metrics
                        crate::metrics::update_node_type_scores(&result);
                    }

                    // Always export current classification status to Prometheus
                    let status = classification.status();
                    crate::metrics::update_node_classification(&status);

                    // Check if meeting target and log advice
                    if let Some((meeting_target, advice)) = classification.is_meeting_target() {
                        if !meeting_target {
                            debug!(advice = %advice, "node improvement advice");
                        }
                    }
                }
            }
        });

        // Start mining loop if enabled
        if let Some(ref miner) = self.miner {
            let miner = Arc::clone(miner);
            let chain = Arc::clone(&self.chain);
            let state = Arc::clone(&self.state);
            let timelock_state = Arc::clone(&self.timelock_state);
            let escrow_state = Arc::clone(&self.escrow_state);
            let channel_state = Arc::clone(&self.channel_state);
            let trustline_state = Arc::clone(&self.trustline_state);
            let dimensional_pool_state = Arc::clone(&self.dimensional_pool_state);
            let marketplace_state = Arc::clone(&self.marketplace_state);
            let tx_pool = Arc::clone(&self.tx_pool);
            let network_tx = network_cmd_tx.clone();
            let hf_sync_for_mining = self.hf_sync.clone();
            let peer_count_for_mining = Arc::clone(&peer_count);
            let best_peer_height_for_mining = Arc::clone(&best_known_peer_height);
            let peer_consensus_for_mining = Arc::clone(&peer_consensus);
            let dev_mode = self.config.dev;
            let mining_template_cache_for_loop = Arc::clone(&mining_template_cache);

            // CRITICAL FIX: Use tokio::spawn for multi-threaded I/O scheduling
            let cpp_tx_for_mining = cpp_network_cmd_tx_for_mining;
            tokio::spawn(async move {
                info!("mining task started");
                Self::mining_loop(
                    miner,
                    mining_template_cache_for_loop,
                    chain,
                    state,
                    timelock_state,
                    escrow_state,
                    channel_state,
                    trustline_state,
                    dimensional_pool_state,
                    marketplace_state,
                    tx_pool,
                    network_tx,
                    cpp_tx_for_mining,
                    hf_sync_for_mining,
                    peer_count_for_mining,
                    best_peer_height_for_mining,
                    peer_consensus_for_mining,
                    dev_mode,
                )
                .await;
                warn!("mining loop exited unexpectedly");
            });
        }

        // Spawn periodic HuggingFace buffer flush task (using blocking HTTP via spawn_blocking)
        if let Some(ref hf_sync) = self.hf_sync {
            let hf_sync_for_flush = Arc::clone(hf_sync);
            let chain_for_flush = Arc::clone(&self.chain);
            // CRITICAL FIX: Use tokio::spawn for multi-threaded I/O scheduling
            tokio::spawn(async move {
                let mut last_flush_height = 0u64;
                loop {
                    // Use blocking sleep to avoid tokio timer issues
                    tokio::task::spawn_blocking(|| {
                        std::thread::sleep(Duration::from_secs(120)); // Check every 2 minutes
                    })
                    .await
                    .unwrap();

                    let current_height = chain_for_flush.best_block_height().await;
                    if current_height > last_flush_height + 50 {
                        debug!(
                            blocks_since_last = current_height - last_flush_height,
                            "huggingface periodic flush"
                        );
                        if let Err(e) = hf_sync_for_flush.flush().await {
                            warn!(error = %e, "huggingface flush error");
                        }
                        last_flush_height = current_height;
                    }
                }
            });
        }

        // Spawn Phase 1C: Dual-feed streamer confirmation processing task
        if let Some(ref streamer) = self.dual_feed_streamer {
            let streamer_for_task = Arc::clone(streamer);
            let chain_for_streamer = Arc::clone(&self.chain);
            tokio::spawn(async move {
                loop {
                    // Process confirmations every 30 seconds
                    tokio::task::spawn_blocking(|| {
                        std::thread::sleep(Duration::from_secs(30));
                    })
                    .await
                    .unwrap();

                    let current_height = chain_for_streamer.best_block_height().await;

                    // Process pending blocks for k-confirmation promotion
                    if let Err(e) = streamer_for_task
                        .process_confirmations(current_height)
                        .await
                    {
                        warn!(error = %e, "dual-feed streamer confirmation error");
                    }
                }
            });
            info!("dual-feed confirmation processor started");
        }

        Ok(())
    }

    /// Wait for shutdown signal
    pub async fn wait_for_shutdown(&mut self) {
        self.shutdown_rx.recv().await;
        info!("shutting down node");
    }

    /// Trigger shutdown
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.try_send(());
    }
}
