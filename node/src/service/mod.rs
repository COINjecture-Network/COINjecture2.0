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
use crate::chain::{ChainState, ChainBlockProvider};
#[cfg(feature = "adzdb")]
use crate::chain_adzdb::{AdzdbChainState as ChainState, ChainBlockProvider};
use crate::config::NodeConfig;
use crate::faucet::{Faucet, FaucetConfig};
use crate::genesis::{create_genesis_block, GenesisConfig};
use crate::peer_consensus::PeerConsensus;
use crate::validator::BlockValidator;
use coinject_consensus::{Miner, MiningConfig};
use coinject_core::Address;
use coinject_mempool::{ProblemMarketplace, TransactionPool};
// libp2p removed - using CPP protocol only
use coinject_network::cpp::{
    CppNetwork, NetworkEvent as CppNetworkEvent, NetworkCommand as CppNetworkCommand, 
    CppConfig, NodeType as CppNodeType, PeerId as CppPeerId, BlockProvider
};
use coinject_rpc::{RpcServer, RpcServerState};
use coinject_rpc::websocket::{WebSocketRpc, RpcEvent as WebSocketRpcEvent, RpcCommand as WebSocketRpcCommand};
use coinject_state::{AccountState, TimeLockState, EscrowState, ChannelState, TrustLineState, DimensionalPoolState, MarketplaceState};
use coinject_huggingface::{
    HuggingFaceSync, HuggingFaceConfig, EnergyConfig, EnergyMeasurementMethod, SyncConfig,
    DualFeedStreamer, StreamerConfig,
};
use rand;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tokio::time;
use blake3;
use hex;

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
    SendSyncBlock { block: coinject_core::Block, request_id: u64 },
    BroadcastTransaction(coinject_core::Transaction),
    BroadcastStatus { 
        best_height: u64, 
        best_hash: coinject_core::Hash, 
        genesis_hash: coinject_core::Hash,
        node_type: coinject_network::NetworkNodeType,
    },
    RequestBlocks { from_height: u64, to_height: u64 },
    /// Legacy: Send block to specific peer (kept for compatibility)
    SendBlockToPeer { block: coinject_core::Block, peer: CppPeerId },
    // === REQUEST-RESPONSE SYNC COMMANDS ===
    // Reliable, ordered block delivery - bypasses GossipSub deduplication issues
    /// Request blocks from a specific peer via request-response (preferred for sync)
    RequestBlocksRR { peer: CppPeerId, from_height: u64, to_height: u64 },
    /// Send blocks response via request-response
    SendBlocksResponse { request_id: u64, blocks: Vec<coinject_core::Block> },
    /// Send error response via request-response
    SendErrorResponse { request_id: u64, message: String },
    // === LIGHT SYNC COMMANDS ===
    /// Send headers to a requesting peer
    SendHeaders { headers: Vec<coinject_core::BlockHeader>, request_id: u64 },
    /// Send FlyClient proof response
    SendFlyClientProof { proof_data: Vec<u8>, request_id: u64 },
    /// Send MMR proof response
    SendMMRProof { header: coinject_core::BlockHeader, proof_data: Vec<u8>, mmr_root: coinject_core::Hash, request_id: u64 },
    /// Send chain tip response
    SendChainTip { height: u64, hash: coinject_core::Hash, mmr_root: coinject_core::Hash, total_work: u128, request_id: u64 },
    /// Request headers (for Light client sync)
    RequestHeaders { start_height: u64, max_headers: u32 },
    /// Request FlyClient proof
    RequestFlyClientProof { security_param: u32 },
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
        println!("🚀 Initializing COINjecture Network B Node...");
        println!();

        // Validate configuration
        config.validate()?;

        // Log block version configuration (P2P.F: Prove the F)
        println!("📋 Block Version Configuration:");
        println!("   Supported versions: {:?}", crate::config::SUPPORTED_VERSIONS);
        println!("   Minimum accepted:   v{} ({})", config.min_block_version, crate::config::version_name(config.min_block_version));
        println!("   Produce version:    v{} ({})", config.produce_block_version, crate::config::version_name(config.produce_block_version));
        if config.strict_version {
            println!("   ⚠️  STRICT MODE: Only accepting v{} blocks", config.produce_block_version);
        }
        println!();

        // Create data directory (parent directory for database files)
        std::fs::create_dir_all(&config.data_dir)?;

        // Initialize genesis block
        println!("📦 Loading genesis block...");
        let genesis = create_genesis_block(GenesisConfig::default());
        let genesis_hash = genesis.header.hash();
        println!("   Genesis hash: {:?}", genesis_hash);
        println!();

        // Initialize chain state
        println!("⛓️  Initializing blockchain state...");
        // Ensure parent directory exists for chain database file
        let chain_db_path = config.chain_db_path();
        if let Some(parent) = chain_db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let chain = Arc::new(ChainState::new(chain_db_path, &genesis)?);
        let best_height = chain.best_block_height().await;
        println!("   Best height: {}", best_height);
        println!();

        // Initialize account state and advanced transaction states
        println!("💰 Initializing account state...");
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
        let advanced_state_db_path = state_db_path.parent()
            .unwrap_or(std::path::Path::new("."))
            .join("advanced_state.db");
        let advanced_state_db = Arc::new(redb::Database::create(&advanced_state_db_path)?);
        let timelock_state = Arc::new(TimeLockState::new(Arc::clone(&advanced_state_db))?);
        let escrow_state = Arc::new(EscrowState::new(Arc::clone(&advanced_state_db))?);
        let channel_state = Arc::new(ChannelState::new(Arc::clone(&advanced_state_db))?);
        let trustline_state = Arc::new(TrustLineState::new(Arc::clone(&advanced_state_db))?);
        let dimensional_pool_state = Arc::new(DimensionalPoolState::new(Arc::clone(&advanced_state_db))?);
        let marketplace_state = Arc::new(MarketplaceState::from_db(Arc::clone(&advanced_state_db))?);

        // Apply genesis if this is a new chain
        if best_height == 0 {
            println!("   Applying genesis block to state...");
            let genesis_addr = genesis.header.miner;
            let genesis_reward = genesis.coinbase.reward;
            state.set_balance(&genesis_addr, genesis_reward)?;
            println!("   Genesis account funded with {} tokens", genesis_reward);

            // Initialize dimensional pools with genesis liquidity
            println!("   Initializing dimensional pools...");
            dimensional_pool_state.initialize_pools(genesis_reward, 0)?;
        }
        println!();

        // Initialize validator
        let validator = Arc::new(BlockValidator::new(config.difficulty));

        // Initialize mempool components
        let marketplace = Arc::new(RwLock::new(ProblemMarketplace::new()));
        let tx_pool = Arc::new(RwLock::new(TransactionPool::new()));

        // Initialize miner if enabled
        let miner = if config.mine {
            println!("⛏️  Initializing miner...");

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
                let validator_key = keystore.get_or_create_key()
                    .map_err(|e| format!("Failed to get validator key: {}", e))?;
                validator_key.address()
            };

            let mining_config = MiningConfig {
                miner_address,
                target_block_time: Duration::from_secs(config.block_time),
                min_difficulty: config.difficulty,
                max_difficulty: config.difficulty + 20,
                golden_activation_height: config.golden_activation_height,
            };

            println!("   Miner address: {}", hex::encode(miner_address.as_bytes()));
            if config.golden_activation_height > 0 {
                println!("   Golden activation height: {}", config.golden_activation_height);
            } else {
                println!("   Golden features: active from genesis");
            }
            println!("   Target block time: {}s", config.block_time);
            println!();

            Some(Arc::new(RwLock::new(Miner::new(mining_config))))
        } else {
            None
        };

        // Initialize faucet if enabled
        let faucet = if config.enable_faucet {
            println!("💧 Faucet enabled:");
            println!("   Amount per request: {} tokens", config.faucet_amount);
            println!("   Cooldown: {} seconds", config.faucet_cooldown);
            println!();

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
        let hf_sync = if let (Some(hf_token), Some(hf_dataset_name)) = (&config.hf_token, &config.hf_dataset_name) {
            println!("🤗 Initializing Hugging Face sync...");
            println!("   Unified dataset: {} (all problem types in one continuous dataset)", hf_dataset_name);

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
            };

            match HuggingFaceSync::new(hf_config, energy_config, sync_config.clone()) {
                Ok(sync) => {
                    println!("   ✅ Hugging Face sync initialized");
                    Some(Arc::new(sync))
                }
                Err(e) => {
                    eprintln!("   ⚠️  Failed to initialize Hugging Face sync: {}", e);
                    None
                }
            }
        } else {
            None
        };

        // Initialize Phase 1C: Dual-Feed Streamer (alongside legacy hf_sync)
        let dual_feed_streamer = if config.hf_token.is_some() {
            println!("📊 Initializing Phase 1C Dual-Feed Streamer...");
            println!("   Feed A: head_unconfirmed (real-time blocks)");
            println!("   Feed B: canonical_confirmed (k-confirmed blocks)");
            println!("   Feed C: reorg_events (chain reorganizations)");

            let streamer_config = StreamerConfig {
                min_confirmations: 20, // Same k as legacy sync
                batch_size: 10,
                batch_interval_secs: 60,
                enabled: true,
                node_id: None, // Will be set when network starts
                data_dir: config.data_dir.clone(),
            };

            let streamer = DualFeedStreamer::new(streamer_config);
            println!("   ✅ Dual-feed streamer initialized");
            println!();
            Some(Arc::new(streamer))
        } else {
            None
        };

        // Create shutdown channel
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

        // Initialize Node Classification Manager
        println!("📊 Initializing Node Classification Manager...");
        let mut classification_manager = crate::node_types::NodeClassificationManager::new(best_height);
        
        // Set target type from config
        let target_type = config.target_node_type();
        classification_manager.set_target_type(target_type);
        
        // Set headers-only mode if configured
        if config.is_light_mode() {
            classification_manager.set_headers_only(true);
            println!("   📱 Light mode enabled (headers-only sync)");
        }
        
        let node_classification = Arc::new(RwLock::new(classification_manager));
        println!("   Target type: {} (actual type determined by behavior)", target_type);
        println!();
        
        // Initialize Light Client if in headers-only mode
        let light_client = if config.is_light_mode() {
            println!("📱 Initializing Light Client (headers-only sync)...");
            let light_state = crate::light_client::LightClientState::new(
                genesis_hash,
                genesis.header.clone(),
            );
            println!("   Light client ready for header sync");
            println!();
            Some(Arc::new(light_state))
        } else {
            None
        };

        // Initialize Node Type Manager (Central Orchestrator)
        println!("🎯 Initializing Node Type Manager (Orchestration Layer)...");
        let (node_manager, _manager_rx, _classification_rx) = crate::node_manager::NodeTypeManager::new(
            best_height,
            target_type,
            Some(genesis.header.clone()),
        );
        let node_manager = Arc::new(node_manager);
        
        // Initialize Capability Router
        let capability_router = Arc::new(crate::node_manager::CapabilityRouter::new());
        
        let capabilities = crate::node_manager::NetworkCapabilities::for_node_type(target_type);
        println!("   Node capabilities:");
        println!("   • Can produce blocks: {}", capabilities.can_produce_blocks);
        println!("   • Can validate blocks: {}", capabilities.can_validate_blocks);
        println!("   • Can serve FlyClient proofs: {}", capabilities.can_serve_flyclient);
        println!("   • Can solve problems: {}", capabilities.can_solve_problems);
        println!("   • Can provide oracle data: {}", capabilities.can_provide_oracle_data);
        println!("   • Max peers: {} in / {} out", capabilities.max_inbound_peers, capabilities.max_outbound_peers);
        println!();

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
        // libp2p removed - using CPP protocol only
        println!("🌐 Starting CPP Network (libp2p removed)...");

        // Create shared peer count for RPC
        let peer_count = Arc::new(RwLock::new(0));

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
        
        println!("   CPP PeerId: {}", local_peer_id_str);
        println!();
        
        // Track listen addresses for RPC (CPP addresses)
        let listen_addresses: Arc<RwLock<Vec<String>>> = Arc::new(RwLock::new(vec![
            format!("cpp://{}", self.config.cpp_p2p_addr),
            format!("ws://{}", self.config.cpp_ws_addr),
        ]));

        // Create command channel for CPP network operations (legacy NetworkCommand kept for compatibility)
        let (network_cmd_tx, _network_cmd_rx) = mpsc::unbounded_channel::<NetworkCommand>();

        // Start RPC server
        println!("🔌 Starting JSON-RPC server...");
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
        let network_tx_for_submission = network_cmd_tx.clone();
        let hf_sync_for_submission = self.hf_sync.clone();
        
        let block_submission_handler: Option<coinject_rpc::BlockSubmissionHandler> = Some(Arc::new(move |block: coinject_core::Block| -> Result<String, String> {
            // Get runtime handle for async operations
            let rt_handle = tokio::runtime::Handle::try_current()
                .map_err(|_| "No async runtime available".to_string())?;
            
            // Use a oneshot channel to get the result from the async task
            let (tx, rx) = tokio::sync::oneshot::channel();
            
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
            let network_tx = network_tx_for_submission.clone();
            let hf_sync = hf_sync_for_submission.clone();
            
            // Spawn async task to handle block submission
            rt_handle.spawn(async move {
                let result = async {
                    // Get current chain state
                    let best_height = chain.best_block_height().await;
                    let best_hash = chain.best_block_hash().await;
                    let expected_height = best_height + 1;
                    
                    // Validate block height
                    if block.header.height != expected_height {
                        return Err(format!("Invalid block height: expected {}, got {}", expected_height, block.header.height));
                    }
                    
                    // Validate previous hash
                    if block.header.prev_hash != best_hash {
                        return Err(format!("Invalid previous hash: expected {}, got {}", best_hash, block.header.prev_hash));
                    }
                    
                    // Validate block (skip timestamp age check for RPC submissions)
                    match validator.validate_block_with_options(&block, &best_hash, expected_height, false) {
                        Ok(()) => {},
                        Err(e) => return Err(format!("Block validation failed: {:?}", e)),
                    }
                    
                    // Store block
                    match chain.store_block(&block).await {
                        Ok(is_new_best) => {
                            if !is_new_best {
                                return Err("Block did not extend the chain".to_string());
                            }
                        },
                        Err(e) => return Err(format!("Failed to store block: {}", e)),
                    }
                    
                    // Apply block transactions
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
                            // Remove applied transactions from pool
                            let mut pool = tx_pool.write().await;
                            for tx_hash in &applied_txs {
                                pool.remove(tx_hash);
                            }
                            drop(pool);
                            
                            // Broadcast block to network
                            if let Err(e) = network_tx.send(NetworkCommand::BroadcastBlock(block.clone())) {
                                return Err(format!("Failed to broadcast block: {}", e));
                            }
                            
                            // Push to Hugging Face if enabled
                            if let Some(ref hf_sync) = hf_sync {
                                let hf_sync_clone = Arc::clone(hf_sync);
                                let block_clone = block.clone();
                                tokio::spawn(async move {
                                    if let Err(e) = hf_sync_clone.push_consensus_block(&block_clone, false).await {
                                        eprintln!("⚠️  Failed to push RPC-submitted block to Hugging Face: {}", e);
                                    }
                                });
                            }
                            
                            Ok(block.hash().to_string())
                        },
                        Err(e) => Err(format!("Failed to apply block transactions: {}", e)),
                    }
                }.await;
                
                // Send result back to synchronous handler
                let _ = tx.send(result);
            });
            
            // Wait for result (with timeout)
            rt_handle.block_on(async {
                // Timeout should be network-derived: ETA * network_median_block_time
                // For now, using ETA-scaled default: 10s * ETA ≈ 7s effective
                use coinject_core::ETA;
                tokio::time::timeout(
                    Duration::from_secs_f64(10.0 * ETA),
                    rx
                ).await
            })
            .map_err(|_| "Block submission timeout".to_string())?
            .map_err(|_| "Failed to receive result".to_string())?
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
            faucet_handler,
            block_submission_handler,
            local_peer_id: Some(local_peer_id_str.clone()),
            listen_addresses: Arc::clone(&listen_addresses),
            is_syncing: Arc::new(tokio::sync::RwLock::new(false)), // Node starts not syncing
        });

        let rpc_server = RpcServer::new(rpc_addr, rpc_state).await?;
        println!("   RPC listening on: {}", rpc_addr);
        println!();

        self.network_cmd_tx = Some(network_cmd_tx.clone());
        self.rpc = Some(rpc_server);

        // =====================================================================
        // Phase 3: Initialize CPP Network and WebSocket RPC
        // =====================================================================
        println!("🌐 Starting CPP Network (Phase 3)...");
        println!("   CPP P2P address: {}", self.config.cpp_p2p_addr);
        println!("   CPP WebSocket address: {}", self.config.cpp_ws_addr);
        
        let genesis_hash = self.chain.genesis_hash();
        let local_peer_id_bytes: [u8; 32] = {
            // Convert PeerId to bytes (simplified - in production use actual peer ID)
            let peer_id_str = local_peer_id_str.as_bytes();
            let mut bytes = [0u8; 32];
            let len = peer_id_str.len().min(32);
            bytes[..len].copy_from_slice(&peer_id_str[..len]);
            bytes
        };
        
        // Parse CPP bootnodes from config (format: "IP:PORT" or multiaddr "/ip4/IP/tcp/PORT/p2p/PEER_ID")
        // For CPP, we extract IP:PORT from multiaddr format or use as-is if already IP:PORT
        // If no bootnodes provided, CPP will work in standalone mode
        let cpp_bootnodes: Vec<String> = if self.config.bootnodes.is_empty() {
            vec![] // No bootnodes - standalone mode
        } else {
            self.config.bootnodes.iter()
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
        };
        
        // Get current chain state before creating CPP network
        let current_height = self.chain.best_block_height().await;
        let current_hash = self.chain.best_block_hash().await;
        
        // Create block provider for serving sync requests to peers
        let block_provider: Arc<dyn BlockProvider> = Arc::new(ChainBlockProvider::new(self.chain.clone()));
        
        let (cpp_network, cpp_network_cmd_tx, mut cpp_network_event_rx) = 
            CppNetwork::new_with_block_provider(cpp_config, local_peer_id_bytes, genesis_hash, current_height, current_hash, block_provider);
        
        println!("✅ Initialized CPP network with BlockProvider: height={}, hash={:?}", current_height, current_hash);
        
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
                            eprintln!("[CPP] Failed to resolve bootnode '{}': {}", bootnode_addr, e);
                            None
                        }
                    }
                };
                if let Some(addr) = resolved {
                    println!("[CPP] Connecting to bootnode: {} (resolved from '{}')", addr, bootnode_addr);
                    if let Err(e) = cpp_network_cmd_tx_for_bootnodes.send(
                        coinject_network::cpp::NetworkCommand::ConnectBootnode { addr }
                    ) {
                        eprintln!("[CPP] Failed to send connect command: {}", e);
                    }
                } else {
                    eprintln!("[CPP] Invalid bootnode address format: {}", bootnode_addr);
                }
            }
        });
        
        // Spawn CPP network task
        let cpp_p2p_addr_clone = self.config.cpp_p2p_addr.clone();
        tokio::spawn(async move {
            println!("[CPP] Starting CPP network task...");
            match cpp_network.start().await {
                Ok(()) => {
                    println!("[CPP] Network task completed normally");
                                    }
                                    Err(e) => {
                    eprintln!("[CPP] Network error: {}", e);
                    eprintln!("[CPP] Failed to bind to: {}", cpp_p2p_addr_clone);
                }
            }
        });
        
        // Create block buffer for out-of-order blocks (used by CPP sync)
        let block_buffer: Arc<RwLock<HashMap<u64, coinject_core::Block>>> = Arc::new(RwLock::new(HashMap::new()));

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
        let peer_consensus_clone = Arc::clone(&peer_consensus);
        let cpp_network_cmd_tx_for_events = cpp_network_cmd_tx.clone();
        let hf_sync_clone = self.hf_sync.clone();
        let block_buffer_clone = Arc::clone(&block_buffer);
        // Clone config for version checking in event handler
        let config_clone = self.config.clone();

        tokio::spawn(async move {
            while let Some(event) = cpp_network_event_rx.recv().await {
                match event {
                    CppNetworkEvent::BlockReceived { block, peer_id: _peer_id } => {
                        // Log block with version info (P2P.F: Prove the F)
                        let version_info = config_clone.version_info(block.header.version);
                        println!("[BLOCK] Received block height={} {} hash={:?}",
                            block.header.height, version_info, block.header.hash());

                        // Check version policy before validation
                        if let Err(reason) = config_clone.should_accept_version(block.header.version) {
                            println!("[BLOCK] REJECTED block height={} {} ({})",
                                block.header.height, version_info, reason);
                            continue;
                        }
                        
                        let best_height = chain_clone.best_block_height().await;
                        let best_hash = chain_clone.best_block_hash().await;
                        let expected_height = best_height + 1;
                        
                        // Validate block height
                        if block.header.height != expected_height {
                            println!("⚠️  [CPP] Block height mismatch: expected {}, got {}", expected_height, block.header.height);
                            continue;
                        }
                        
                        // Validate previous hash
                        if block.header.prev_hash != best_hash {
                            println!("⚠️  [CPP] Block prev_hash mismatch: expected {}, got {}", best_hash, block.header.prev_hash);
                            continue;
                        }
                        
                        // Validate block
                        match validator_clone.validate_block_with_options(&block, &best_hash, expected_height, false) {
                            Ok(()) => {
                                // Store block
                                match chain_clone.store_block(&block).await {
                                    Ok(is_new_best) => {
                                        if is_new_best {
                                            println!("[APPLY] applied height={} new_best={:?}", block.header.height, block.header.hash());
                                            
                                            // Update CPP network chain state
                                            let new_height = block.header.height;
                                            let new_hash = block.header.hash();
                                            if let Err(e) = cpp_network_cmd_tx_for_legacy.send(CppNetworkCommand::UpdateChainState {
                                                best_height: new_height,
                                                best_hash: new_hash,
                                            }) {
                                                eprintln!("⚠️  Failed to update CPP network chain state: {}", e);
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
                                                eprintln!("❌ [CPP] Failed to apply block transactions: {}", e);
                                            } else {
                                                // Remove applied transactions from pool
                                                let mut pool = tx_pool_clone.write().await;
                                                for tx in &block.transactions {
                                                    pool.remove(&tx.hash());
                                                }
                                                
                                                // Update best known peer height
                                                let mut best_peer = best_known_peer_height_clone.write().await;
                                                if block.header.height > *best_peer {
                                                    *best_peer = block.header.height;
                                                }
                                                
                                                // Push to Hugging Face if enabled
                                                if let Some(ref hf_sync) = hf_sync_clone {
                                                    let hf_sync_clone2 = Arc::clone(hf_sync);
                                                    let block_clone = block.clone();
                                                    tokio::spawn(async move {
                                                        if let Err(e) = hf_sync_clone2.push_consensus_block(&block_clone, false).await {
                                                            eprintln!("⚠️  [CPP] Failed to push block to Hugging Face: {}", e);
                                                        }
                                                    });
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("❌ [CPP] Failed to store block: {}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("❌ [CPP] Block validation failed: {:?}", e);
                            }
                        }
                    }
                    CppNetworkEvent::TransactionReceived { transaction, peer_id } => {
                        println!("📨 [CPP] Received transaction {:?} from peer {:?}", transaction.hash(), hex::encode(peer_id));
                        let mut pool = tx_pool_clone.write().await;
                        if let Err(e) = pool.add(transaction) {
                            eprintln!("⚠️  [CPP] Failed to add transaction to pool: {}", e);
                        }
                    }
                    CppNetworkEvent::BlocksReceived { blocks, request_id: _, peer_id } => {
                        println!("📦 [CPP] Received {} blocks for sync from peer {:?}", blocks.len(), hex::encode(peer_id));

                        let mut highest_received: u64 = 0;
                        let mut blocks_applied: u64 = 0;
                        let mut blocks_rejected_version: u64 = 0;

                        // Process sync blocks - buffer future blocks, apply sequential ones
                        for block in blocks {
                            // Check version policy first (P2P.F: Prove the F)
                            let version_info = config_clone.version_info(block.header.version);
                            if let Err(reason) = config_clone.should_accept_version(block.header.version) {
                                println!("[BLOCK] REJECTED sync block height={} {} ({})",
                                    block.header.height, version_info, reason);
                                blocks_rejected_version += 1;
                                continue;
                            }

                            let best_height = chain_clone.best_block_height().await;
                            let best_hash = chain_clone.best_block_hash().await;
                            let expected_height = best_height + 1;

                            if block.header.height > highest_received {
                                highest_received = block.header.height;
                            }

                            if block.header.height == expected_height && block.header.prev_hash == best_hash {
                                // Validate and store (skip age check for sync blocks)
                                if let Ok(()) = validator_clone.validate_block_with_options(&block, &best_hash, expected_height, true) {
                                    if let Ok(is_new_best) = chain_clone.store_block(&block).await {
                                        if is_new_best {
                                            blocks_applied += 1;
                                            println!("[SYNC_APPLY] Block {} {} applied", block.header.height, version_info);
                                            // Update CPP network chain state
                                            let new_height = block.header.height;
                                            let new_hash = block.header.hash();
                                            if let Err(e) = cpp_network_cmd_tx_for_events.send(CppNetworkCommand::UpdateChainState {
                                                best_height: new_height,
                                                best_hash: new_hash,
                                            }) {
                                                eprintln!("⚠️  Failed to update CPP network chain state: {}", e);
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
                                                    // Success - sync block applied
                                                }
                                                Err(e) => {
                                                    eprintln!("❌ [CPP] Failed to apply sync block transactions: {}", e);
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    println!("⚠️  [CPP] Block {} validation failed, buffering", block.header.height);
                                    let mut buffer = block_buffer_clone.write().await;
                                    buffer.insert(block.header.height, block);
                                }
                            } else if block.header.height > expected_height {
                                // Future block - buffer it for later
                                println!("🗃️  [CPP] Buffering future block {} (expected: {})", block.header.height, expected_height);
                                let mut buffer = block_buffer_clone.write().await;
                                buffer.insert(block.header.height, block);
                            } else {
                                println!("⏭️  [CPP] Skipping block {} (already have height {})", block.header.height, best_height);
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
                                    let buffer_version_info = config_clone.version_info(block.header.version);
                                    if let Err(reason) = config_clone.should_accept_version(block.header.version) {
                                        println!("[BLOCK] REJECTED buffered block height={} {} ({})",
                                            block.header.height, buffer_version_info, reason);
                                        blocks_rejected_version += 1;
                                        continue;
                                    }

                                    if block.header.prev_hash == best_hash {
                                        if let Ok(()) = validator_clone.validate_block_with_options(&block, &best_hash, next_height, true) {
                                            if let Ok(is_new_best) = chain_clone.store_block(&block).await {
                                                if is_new_best {
                                                    blocks_applied += 1;
                                                    println!("[SYNC_BUFFER] Block {} {} applied from buffer", block.header.height, buffer_version_info);
                                                    let new_height = block.header.height;
                                                    let new_hash = block.header.hash();
                                                    let _ = cpp_network_cmd_tx_for_events.send(CppNetworkCommand::UpdateChainState {
                                                        best_height: new_height,
                                                        best_hash: new_hash,
                                                    });
                                                    let _ = Self::apply_block_transactions(
                                                        &block,
                                                        &state_clone,
                                                        &timelock_state_clone,
                                                        &escrow_state_clone,
                                                        &channel_state_clone,
                                                        &trustline_state_clone,
                                                        &dimensional_pool_state_clone,
                                                        &marketplace_state_clone,
                                                    );
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
                        let peer_height = peer_consensus_clone.get_peer_height(&hex::encode(peer_id)).await.unwrap_or(0);

                        if blocks_rejected_version > 0 {
                            println!("📊 [CPP] Sync progress: applied {} blocks, rejected {} (version), now at height {}, peer at {}",
                                blocks_applied, blocks_rejected_version, current_height, peer_height);
                        } else {
                            println!("📊 [CPP] Sync progress: applied {} blocks, now at height {}, peer at {}",
                                blocks_applied, current_height, peer_height);
                        }

                        if peer_height > current_height {
                            // Still behind - request more blocks
                            let from_height = current_height + 1;
                            let to_height = peer_height.min(current_height + 16); // MAX_BLOCKS_PER_RESPONSE
                            println!("🔄 [CPP] Requesting continuation: blocks {}-{} from peer {:?}",
                                from_height, to_height, hex::encode(peer_id));
                            let _ = cpp_network_cmd_tx_for_events.send(CppNetworkCommand::RequestBlocks {
                                peer_id,
                                from_height,
                                to_height,
                                request_id: rand::random(),
                            });
                        }
                    }
                    CppNetworkEvent::PeerConnected { peer_id, addr, node_type: _, best_height, best_hash } => {
                        println!("🤝 [CPP] Peer connected: {:?} at {}", hex::encode(peer_id), addr);
                        // Update peer count
                        {
                            let mut count = peer_count_clone.write().await;
                            *count += 1;
                            println!("   📊 Peer count: {}", *count);
                        }
                        // Update peer consensus tracker
                        let peer_id_str = hex::encode(peer_id);
                        let best_hash_bytes: [u8; 32] = *best_hash.as_bytes();
                        peer_consensus_clone.update_peer(peer_id_str, best_height, best_hash_bytes).await;
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
                            let from_height = current_height + 1;
                            let to_height = median_height.min(current_height + 100); // Request up to 100 blocks at a time
                            println!("📡 [Consensus Math] Behind median peer height: {} blocks (our: {}, median: {}, threshold: {}), requesting blocks {}-{} for sync", 
                                blocks_behind, current_height, median_height, sync_threshold, from_height, to_height);
                            let _ = cpp_network_cmd_tx_for_events.send(CppNetworkCommand::RequestBlocks {
                                peer_id,
                                from_height,
                                to_height,
                                request_id: rand::random(),
                            });
                        } else if best_height > current_height {
                            // Fallback: if this specific peer is ahead (but median check didn't trigger)
                            // This handles edge cases where median is close but individual peer is ahead
                            let from_height = current_height + 1;
                            let to_height = best_height.min(current_height + 100);
                            println!("📡 Peer is ahead (peer: {}, us: {}), requesting blocks {}-{} for sync", 
                                best_height, current_height, from_height, to_height);
                            let _ = cpp_network_cmd_tx_for_events.send(CppNetworkCommand::RequestBlocks {
                                peer_id,
                                from_height,
                                to_height,
                                request_id: rand::random(),
                            });
                        }
                    }
                    CppNetworkEvent::PeerDisconnected { peer_id, reason: _ } => {
                        println!("👋 [CPP] Peer disconnected: {:?}", hex::encode(peer_id));
                        // Update peer count
                        {
                            let mut count = peer_count_clone.write().await;
                            if *count > 0 {
                                *count -= 1;
                            }
                            println!("   📊 Peer count: {}", *count);
                        }
                        // Mark peer as disconnected in consensus tracker
                        let peer_id_str = hex::encode(peer_id);
                        peer_consensus_clone.mark_peer_disconnected(&peer_id_str).await;
                    }
                    CppNetworkEvent::StatusUpdate { peer_id, best_height, best_hash, node_type: _node_type } => {
                        println!("📡 [CPP] Status update from peer {:?}: height {}, hash {:?}",
                            hex::encode(peer_id), best_height, best_hash);

                        // Update peer consensus tracker
                        let peer_id_str = hex::encode(peer_id);
                        let hash_bytes: [u8; 32] = *best_hash.as_bytes();
                        peer_consensus_clone.update_peer(peer_id_str, best_height, hash_bytes).await;

                        // Update best known peer height
                        {
                            let mut best_height_guard = best_known_peer_height_clone.write().await;
                            if best_height > *best_height_guard {
                                *best_height_guard = best_height;
                                println!("   📊 Updated best known peer height: {}", best_height);
                            }
                        }

                        // === FIX: Trigger sync on StatusUpdate ===
                        // Previously, StatusUpdate only updated trackers but never requested blocks.
                        // This caused nodes to stay stuck even when peers announced higher heights.
                        let current_height = chain_clone.best_block_height().await;
                        if best_height > current_height {
                            let from_height = current_height + 1;
                            // Request up to 100 blocks at a time, capped by MAX_BLOCKS_PER_RESPONSE (16)
                            let to_height = best_height.min(current_height + 100);
                            println!("🔄 [StatusUpdate Sync] Peer is ahead (peer: {}, us: {}), requesting blocks {}-{}",
                                best_height, current_height, from_height, to_height);
                            let _ = cpp_network_cmd_tx_for_events.send(CppNetworkCommand::RequestBlocks {
                                peer_id,
                                from_height,
                                to_height,
                                request_id: rand::random(),
                            });
                        } else {
                            println!("   ✅ In sync with peer (peer: {}, us: {})", best_height, current_height);
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
            println!("🕸️  Starting Mesh Network...");

            let mesh_listen_addr: std::net::SocketAddr = self.config.mesh_listen
                .parse()
                .map_err(|e| format!("Invalid mesh listen address: {}", e))?;

            let mut mesh_seeds: Vec<std::net::SocketAddr> = Vec::new();
            for seed_str in &self.config.mesh_seed {
                match seed_str.parse::<std::net::SocketAddr>() {
                    Ok(addr) => mesh_seeds.push(addr),
                    Err(e) => eprintln!("[Mesh] Invalid seed address '{}': {}", seed_str, e),
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
                    let (bridge_cmd_tx, bridge_cmd_rx) = mpsc::unbounded_channel::<coinject_network::MeshBridgeCommand>();
                    let (bridge_event_tx, mut bridge_event_rx) = mpsc::unbounded_channel::<coinject_network::MeshBridgeEvent>();

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
                        ).await;
                    });

                    // Forward mined blocks to mesh (clone of bridge_cmd_tx for mining)
                    let _bridge_cmd_tx_for_mining = bridge_cmd_tx.clone();
                    let _bridge_cmd_tx_for_events = bridge_cmd_tx.clone();

                    // ── Epoch Coordinator (optional, alongside mesh) ─────
                    // Create coordinator channels
                    let (coord_cmd_tx, coord_cmd_rx) = mpsc::unbounded_channel::<coinject_consensus::CoordinatorCommand>();
                    let (coord_event_tx, mut coord_event_rx) = mpsc::unbounded_channel::<coinject_consensus::CoordinatorEvent>();

                    // Use mesh node identity as coordinator ID
                    let coord_node_id: [u8; 32] = mesh_service.local_id().0;

                    let coord_config = coinject_consensus::CoordinatorConfig::default();
                    let (coordinator, _coord_shared_state) = coinject_consensus::EpochCoordinator::new(
                        coord_node_id,
                        coord_config,
                        current_height,
                        current_hash,
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
                                coinject_consensus::CoordinatorEvent::BroadcastSalt { epoch, salt } => {
                                    tracing::info!(epoch, "coordinator: broadcasting salt via mesh");
                                    let _ = bridge_cmd_for_coord.send(
                                        coinject_network::MeshBridgeCommand::BroadcastConsensusSalt { epoch, salt }
                                    );
                                }
                                coinject_consensus::CoordinatorEvent::BroadcastCommit { epoch, solution_hash, work_score } => {
                                    tracing::info!(epoch, "coordinator: broadcasting commit via mesh");
                                    let _ = bridge_cmd_for_coord.send(
                                        coinject_network::MeshBridgeCommand::BroadcastCommit {
                                            epoch,
                                            solution_hash,
                                            node_id: coord_node_id,
                                            work_score,
                                            signature: Vec::new(),
                                        }
                                    );
                                }
                                coinject_consensus::CoordinatorEvent::EpochStarted { epoch, leader, .. } => {
                                    tracing::info!(epoch, leader = hex::encode(&leader[..4]), "coordinator: epoch started");
                                }
                                coinject_consensus::CoordinatorEvent::MinePhaseStarted { epoch, .. } => {
                                    tracing::info!(epoch, "coordinator: mine phase started");
                                }
                                coinject_consensus::CoordinatorEvent::CommitPhaseStarted { epoch } => {
                                    tracing::info!(epoch, "coordinator: commit phase started");
                                }
                                coinject_consensus::CoordinatorEvent::EpochSealed { epoch, winner, work_score, commit_count } => {
                                    tracing::info!(
                                        epoch, winner = hex::encode(&winner[..4]),
                                        work_score, commit_count,
                                        "coordinator: epoch sealed"
                                    );
                                }
                                coinject_consensus::CoordinatorEvent::EpochStalled { epoch, phase, reason } => {
                                    tracing::warn!(epoch, phase = %phase, reason = %reason, "coordinator: epoch stalled");
                                }
                                coinject_consensus::CoordinatorEvent::BlockProduced { block, epoch } => {
                                    let block_hash = block.header.hash();
                                    let height = block.header.height;
                                    tracing::info!(
                                        epoch, height,
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

                    tokio::spawn(async move {
                        while let Some(event) = bridge_event_rx.recv().await {
                            match event {
                                coinject_network::MeshBridgeEvent::BlockReceived { block, peer_id } => {
                                    let height = block.header.height;
                                    tracing::info!(height, peer = %peer_id.short(), "mesh: block received");

                                    let best_height = chain_for_mesh.best_block_height().await;
                                    let best_hash = chain_for_mesh.best_block_hash().await;
                                    let expected_height = best_height + 1;

                                    if height != expected_height {
                                        tracing::debug!(height, expected_height, "mesh: unexpected block height");
                                        continue;
                                    }

                                    if block.header.prev_hash != best_hash {
                                        tracing::debug!(height, "mesh: prev_hash mismatch");
                                        continue;
                                    }

                                    match validator_for_mesh.validate_block_with_options(&block, &best_hash, expected_height, false) {
                                        Ok(()) => {
                                            match chain_for_mesh.store_block(&block).await {
                                                Ok(is_new_best) => {
                                                    if is_new_best {
                                                        tracing::info!(height, "mesh: block stored as new best");
                                                        let new_hash = block.header.hash();

                                                        // Apply state transitions
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
                                                            tracing::warn!(height, error = %e, "mesh: apply txs failed");
                                                        } else {
                                                            // Remove applied txs from pool
                                                            let mut pool = tx_pool_for_mesh.write().await;
                                                            for tx in &block.transactions {
                                                                pool.remove(&tx.hash());
                                                            }
                                                        }

                                                        // Update bridge state
                                                        let mut bs = bridge_state_for_events.write().await;
                                                        bs.best_height = height;
                                                        bs.best_hash = new_hash;

                                                        // Update CPP with new chain state
                                                        let _ = cpp_cmd_for_mesh.send(CppNetworkCommand::UpdateChainState {
                                                            best_height: height,
                                                            best_hash: new_hash,
                                                        });

                                                        // Update coordinator chain tip
                                                        let _ = coord_cmd_for_events.send(
                                                            coinject_consensus::CoordinatorCommand::ChainTipUpdated {
                                                                height,
                                                                hash: new_hash,
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
                                coinject_network::MeshBridgeEvent::TransactionReceived { transaction, peer_id } => {
                                    tracing::debug!(peer = %peer_id.short(), "mesh: transaction received");
                                    let mut pool = tx_pool_for_mesh.write().await;
                                    let _ = pool.add(transaction);
                                }
                                coinject_network::MeshBridgeEvent::PeerConnected { peer_id, best_height, .. } => {
                                    tracing::info!(peer = %peer_id.short(), best_height, "mesh: peer connected");
                                    let _ = coord_cmd_for_events.send(
                                        coinject_consensus::CoordinatorCommand::PeerJoined { node_id: peer_id.0 }
                                    );
                                }
                                coinject_network::MeshBridgeEvent::PeerDisconnected { peer_id, reason } => {
                                    tracing::info!(peer = %peer_id.short(), reason = %reason, "mesh: peer disconnected");
                                    let _ = coord_cmd_for_events.send(
                                        coinject_consensus::CoordinatorCommand::PeerLeft { node_id: peer_id.0 }
                                    );
                                }
                                coinject_network::MeshBridgeEvent::StatusUpdate { peer_id, best_height, .. } => {
                                    tracing::debug!(peer = %peer_id.short(), best_height, "mesh: status update");
                                }
                                coinject_network::MeshBridgeEvent::BlocksReceived { blocks, request_id, peer_id } => {
                                    tracing::info!(
                                        count = blocks.len(), request_id,
                                        peer = %peer_id.short(), "mesh: sync blocks received"
                                    );
                                    for block in blocks {
                                        let height = block.header.height;
                                        let best_hash = chain_for_mesh.best_block_hash().await;
                                        let expected = chain_for_mesh.best_block_height().await + 1;
                                        match validator_for_mesh.validate_block_with_options(&block, &best_hash, expected, false) {
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
                                                            }
                                                        }
                                                    }
                                                    Err(e) => tracing::warn!(height, error = %e, "mesh: sync store failed"),
                                                }
                                            }
                                            Err(e) => tracing::warn!(height, error = ?e, "mesh: sync block invalid"),
                                        }
                                    }
                                }
                                // ── Consensus payloads → Coordinator ─────────────
                                coinject_network::MeshBridgeEvent::ConsensusSaltReceived { epoch, salt, from } => {
                                    tracing::debug!(epoch, peer = %from.short(), "mesh: consensus salt received");
                                    let _ = coord_cmd_for_events.send(
                                        coinject_consensus::CoordinatorCommand::SaltReceived {
                                            epoch,
                                            salt,
                                            from: from.0,
                                        }
                                    );
                                }
                                coinject_network::MeshBridgeEvent::ConsensusCommitReceived { epoch, block_hash: _, commits, from } => {
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
                    println!("   Mesh Network listening on: {}", self.config.mesh_listen);
                    println!("   Epoch Coordinator: active (Salt→Mine→Commit→Seal)");
                    if !self.config.mesh_seed.is_empty() {
                        println!("   Mesh seeds: {:?}", self.config.mesh_seed);
                    }
                    println!();
                }
                Err(e) => {
                    eprintln!("[Mesh] Failed to start mesh network: {}", e);
                    eprintln!("[Mesh] Continuing with CPP-only transport");
                }
            }
        }

        // Initialize WebSocket RPC
        println!("🔌 Starting WebSocket RPC (Phase 3)...");
        let ws_addr: std::net::SocketAddr = self.config.cpp_ws_addr
            .parse()
            .map_err(|e| format!("Invalid WebSocket address: {}", e))?;
        
        let (websocket_rpc, websocket_rpc_cmd_tx, mut websocket_rpc_event_rx) = 
            WebSocketRpc::new(ws_addr);
        
        // Spawn WebSocket RPC task
        let ws_addr_clone = self.config.cpp_ws_addr.clone();
        tokio::spawn(async move {
            println!("[WebSocket] Starting WebSocket RPC task...");
            match websocket_rpc.start().await {
                Ok(()) => {
                    println!("[WebSocket] RPC task completed normally");
                }
                Err(e) => {
                    eprintln!("[WebSocket] RPC error: {}", e);
                    eprintln!("[WebSocket] Failed to bind to: {}", ws_addr_clone);
                }
            }
        });
        
        // Spawn WebSocket RPC event handler
        let tx_pool_clone2 = Arc::clone(&self.tx_pool);
        tokio::spawn(async move {
            while let Some(event) = websocket_rpc_event_rx.recv().await {
                match event {
                    WebSocketRpcEvent::WorkSubmitted { client_id: _, work_id: _, solution: _, nonce: _ } => {
                        // TODO: Validate and process PoW submission
                        println!("WebSocket RPC: Received work submission");
                    }
                    WebSocketRpcEvent::TransactionSubmitted { transaction, client_id: _ } => {
                        // TODO: Add transaction to pool
                        let mut pool = tx_pool_clone2.write().await;
                        let _ = pool.add(transaction);
                    }
                    _ => {
                        // Handle other events
                    }
                }
            }
        });
        
        self.cpp_network_cmd_tx = Some(cpp_network_cmd_tx_for_storage);
        self.websocket_rpc_cmd_tx = Some(websocket_rpc_cmd_tx);
        
        println!("   CPP Network listening on: {}", self.config.cpp_p2p_addr);
        println!("   WebSocket RPC listening on: {}", self.config.cpp_ws_addr);
        println!();

        // Start event loop
        println!("✅ Node is ready!");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!();

        // libp2p network task removed - using CPP protocol only
        // Legacy NetworkCommand channel kept for compatibility but commands are routed to CPP network
        let cpp_network_cmd_tx_for_legacy = cpp_network_cmd_tx.clone();
        let mut network_cmd_rx_for_legacy = _network_cmd_rx;
        tokio::spawn(async move {
            while let Some(cmd) = network_cmd_rx_for_legacy.recv().await {
                // Route legacy NetworkCommand to CPP network
                match cmd {
                    NetworkCommand::BroadcastBlock(block) => {
                        println!("[LEGACY] Forwarding BroadcastBlock for height {} to CPP", block.header.height);
                        // Route to CPP network
                        if let Err(e) = cpp_network_cmd_tx_for_legacy.send(
                            CppNetworkCommand::BroadcastBlock { block }
                        ) {
                            eprintln!("Failed to broadcast block via CPP: {}", e);
                        }
                    }
                    NetworkCommand::BroadcastTransaction(tx) => {
                        // Route to CPP network
                        if let Err(e) = cpp_network_cmd_tx_for_legacy.send(
                            CppNetworkCommand::BroadcastTransaction { transaction: tx }
                        ) {
                            eprintln!("Failed to broadcast transaction via CPP: {}", e);
                        }
                    }
                    _ => {
                        // Other commands not yet implemented in CPP - log for now
                        eprintln!("⚠️  Legacy NetworkCommand not yet routed to CPP: {:?}", cmd);
                    }
                }
            }
        });

        // TEMPORARY: Periodic status broadcast disabled (was libp2p)
        // CPP network handles status updates internally
        // TODO: Re-enable with CPP network status broadcasting

        // Spawn periodic reorganization check task (every 60 seconds)
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

        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(60)); // Check every minute
            loop {
                interval.tick().await;
                println!("⏰ Periodic reorganization check triggered");
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
                ).await;
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

                for pool_id in 1..=8 {  // All 8 pools: D1-D8
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

                    if let Some(liquidity) = dimensional_pool_state_for_metrics.get_pool_liquidity(&pool) {
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
                if let Some(consensus_state) = dimensional_pool_state_for_metrics.get_current_consensus_state() {
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
                    let constraint = metrics.measured_eta * metrics.measured_eta +
                                   metrics.measured_lambda * metrics.measured_lambda;
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
                        tracing::info!(
                            "🏷️ Node classified as {} (confidence: {:.2}%): {}",
                            result.node_type,
                            result.confidence * 100.0,
                            result.reason
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
                            tracing::debug!("📈 Node improvement advice: {}", advice);
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

            // CRITICAL FIX: Use tokio::spawn for multi-threaded I/O scheduling
            let cpp_tx_for_mining = cpp_network_cmd_tx_for_mining;
            tokio::spawn(async move {
                println!("🔧 Mining task started");
                Self::mining_loop(miner, chain, state, timelock_state, escrow_state, channel_state, trustline_state, dimensional_pool_state, marketplace_state, tx_pool, network_tx, cpp_tx_for_mining, hf_sync_for_mining, peer_count_for_mining, best_peer_height_for_mining, peer_consensus_for_mining, dev_mode).await;
                println!("⚠️ Mining loop exited (unexpected)");
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
                    }).await.unwrap();
                    
                    let current_height = chain_for_flush.best_block_height().await;
                    if current_height > last_flush_height + 50 {
                        eprintln!("🔄 Hugging Face: Periodic flush ({} blocks since last)", current_height - last_flush_height);
                        if let Err(e) = hf_sync_for_flush.flush().await {
                            eprintln!("⚠️  HuggingFace flush error: {}", e);
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
                    }).await.unwrap();

                    let current_height = chain_for_streamer.best_block_height().await;

                    // Process pending blocks for k-confirmation promotion
                    if let Err(e) = streamer_for_task.process_confirmations(current_height).await {
                        eprintln!("⚠️  Dual-feed streamer confirmation error: {}", e);
                    }
                }
            });
            eprintln!("📊 Phase 1C: Dual-feed confirmation processor started");
        }

        Ok(())
    }

    /// Wait for shutdown signal
    pub async fn wait_for_shutdown(&mut self) {
        self.shutdown_rx.recv().await;
        println!("🛑 Shutting down node...");
    }

    /// Trigger shutdown
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.try_send(());
    }
}
