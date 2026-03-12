// Node Service
// Main orchestrator tying all components together
//
// NOTE: Many protocol handlers are prepared for future protocol extensions
#![allow(dead_code)]

// Conditional ChainState: uses ADZDB when compiled with --features adzdb
#[cfg(not(feature = "adzdb"))]
use crate::chain::{ChainState, ChainBlockProvider};
#[cfg(feature = "adzdb")]
use crate::chain_adzdb::{AdzdbChainState as ChainState, ChainBlockProvider};
use crate::config::NodeConfig;
use crate::faucet::{Faucet, FaucetConfig};
use crate::genesis::{create_genesis_block, GenesisConfig};
use crate::peer_consensus::{PeerConsensus, ConsensusConfig};
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
enum NetworkCommand {
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
            let state_clone = Arc::clone(&self.state);
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
                    CppNetworkEvent::BlockReceived { block, peer_id } => {
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
                    CppNetworkEvent::StatusUpdate { peer_id, best_height, best_hash, node_type } => {
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
                    let bridge_cmd_tx_for_mining = bridge_cmd_tx.clone();
                    let bridge_cmd_tx_for_events = bridge_cmd_tx.clone();

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
                    let coord_cmd_for_bridge = coord_cmd_tx.clone();

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
        let state_for_metrics = Arc::clone(&self.state);
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

    /// Process buffered blocks sequentially
    async fn process_buffered_blocks(
        chain: &Arc<ChainState>,
        state: &Arc<AccountState>,
        timelock_state: &Arc<TimeLockState>,
        escrow_state: &Arc<EscrowState>,
        channel_state: &Arc<ChannelState>,
        trustline_state: &Arc<TrustLineState>,
        dimensional_pool_state: &Arc<DimensionalPoolState>,
        marketplace_state: &Arc<MarketplaceState>,
        validator: &Arc<BlockValidator>,
        tx_pool: &Arc<RwLock<TransactionPool>>,
        block_buffer: &Arc<RwLock<HashMap<u64, coinject_core::Block>>>,
        hf_sync: &Option<Arc<HuggingFaceSync>>,
        network_tx: Option<&mpsc::UnboundedSender<NetworkCommand>>,
    ) {
        loop {
            let best_height = chain.best_block_height().await;
            let next_height = best_height + 1;

            // Check if we have the next sequential block in buffer
            let block_opt = {
                let mut buffer = block_buffer.write().await;
                buffer.remove(&next_height)
            };

            match block_opt {
                Some(block) => {
                    println!("🔄 Processing buffered block {} from buffer", next_height);

                    let best_hash = chain.best_block_hash().await;

                    // Validate the buffered block (skip timestamp age check for historical blocks during sync)
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as i64;
                    let block_age = now - block.header.timestamp;
                    let skip_age_check = block_age > 7200; // 2 hours

                    // FORK HANDLING FIX: Check if buffered block extends our current chain or is a fork block
                    let extends_best_chain = block.header.prev_hash == best_hash;

                    let validation_result = if extends_best_chain {
                        // Normal case: block extends our best chain
                        validator.validate_block_with_options(&block, &best_hash, next_height, skip_age_check)
                    } else {
                        // Fork case: check if we have the parent block
                        match chain.has_block(&block.header.prev_hash) {
                            Ok(true) => {
                                // We have the parent - this is a valid sidechain block
                                println!("🔀 Buffered fork block detected at height {}: extends {:?} (not our tip {:?})",
                                    block.header.height,
                                    &block.header.prev_hash.to_string()[..16],
                                    &best_hash.to_string()[..16]);

                                // Validate against its declared parent (not best_hash)
                                validator.validate_block_with_options(&block, &block.header.prev_hash, next_height, skip_age_check)
                            }
                            Ok(false) => {
                                // Parent missing - this is an orphan block from a fork chain
                                // We can't process it without its parent. Don't re-add to buffer
                                // (that would cause infinite loop). The block will be re-sent by peers
                                // during normal sync or gossip if we need it later.
                                println!("👻 Buffered orphan block at height {}: parent {:?} not found - discarding fork block",
                                    block.header.height,
                                    &block.header.prev_hash.to_string()[..16]);

                                // Request missing blocks from peers to help sync
                                if let Some(net_tx) = network_tx {
                                    let _ = net_tx.send(NetworkCommand::RequestBlocks {
                                        from_height: next_height,
                                        to_height: next_height + 10,
                                    });
                                }
                                break; // Exit the loop - can't process more without syncing
                            }
                            Err(e) => {
                                println!("❌ Error checking for parent block: {}", e);
                                continue;
                            }
                        }
                    };

                    match validation_result {
                        Ok(()) => {
                            // Store and apply
                            // During sequential sync, we're processing blocks one by one starting from best_height + 1.
                            // If validation passed (prev_hash matches best_hash and height is sequential), the block
                            // should extend the best chain. However, store_block might return false due to race conditions
                            // or if the block was already stored. In this case, we should still apply it if it extends
                            // our current best chain.
                            match chain.store_block(&block).await {
                                Ok(is_new_best) => {
                                    // Check if this block extends our current best chain
                                    // Since we validated prev_hash == best_hash and height == best_height + 1,
                                    // this block should extend the chain. If is_new_best is false, it might be due
                                    // to a race condition, so we check if it actually extends the chain.
                                    let current_best = chain.best_block_height().await;
                                    let current_best_hash = chain.best_block_hash().await;
                                    
                                    // Block extends chain if: it's the next height AND prev_hash matches current best
                                    let extends_chain = block.header.height == current_best + 1 && block.header.prev_hash == current_best_hash;
                                    
                                    if is_new_best || extends_chain {
                                        // RUNTIME INTEGRATION: Save consensus state for buffered blocks
                                        use coinject_core::{TAU_C, ConsensusState};
                                        let tau = (block.header.height as f64) / TAU_C;
                                        let consensus_state = ConsensusState::at_tau(tau);

                                        if let Err(e) = dimensional_pool_state.save_consensus_state(block.header.height, &consensus_state) {
                                            println!("⚠️  Warning: Failed to save consensus state: {}", e);
                                        }

                                        match Self::apply_block_transactions(&block, state, timelock_state, escrow_state, channel_state, trustline_state, dimensional_pool_state, marketplace_state) {
                                            Ok(applied_txs) => {
                                                println!("✅ Buffered block {} applied to chain (τ={:.4})", next_height, tau);

                                                // If store_block didn't update best chain, manually update it
                                                if !is_new_best && extends_chain {
                                                    if let Err(e) = chain.update_best_chain(block.header.hash(), block.header.height).await {
                                                        println!("⚠️  Warning: Failed to update best chain after applying buffered block: {}", e);
                                                    } else {
                                                        println!("📈 Updated best chain to height {} (was {} before)", block.header.height, current_best);
                                                    }
                                                }

                                                // Remove only successfully applied transactions from pool
                                                let mut pool = tx_pool.write().await;
                                                for tx_hash in &applied_txs {
                                                    pool.remove(tx_hash);
                                                }
                                                drop(pool);

                                                // Push consensus block to Hugging Face (fire-and-forget)
                                                if let Some(ref hf_sync) = hf_sync {
                                                    let hf_sync_clone = Arc::clone(hf_sync);
                                                    let block_clone = block.clone();
                                                    tokio::spawn(async move {
                                                        if let Err(e) = hf_sync_clone.push_consensus_block(&block_clone, false).await {
                                                            eprintln!("⚠️  Failed to push consensus block to Hugging Face: {}", e);
                                                        }
                                                    });
                                                }

                                                // Continue loop to check for next sequential block
                                            }
                                            Err(e) => {
                                                println!("❌ Failed to apply buffered block transactions: {}", e);
                                                break;
                                            }
                                        }
                                    } else {
                                        // Block doesn't extend our chain - might be a fork, duplicate, or out of order
                                        // Skip it and continue processing (don't break, as there might be other sequential blocks)
                                        println!("⚠️  Buffered block {} doesn't extend best chain (best: {}), skipping", next_height, current_best);
                                        // Continue loop to check for next sequential block
                                    }
                                }
                                Err(e) => {
                                    println!("❌ Failed to store buffered block: {}", e);
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            println!("❌ Buffered block validation failed: {}", e);
                            // If validation failed due to invalid prev_hash, the block might have been
                            // buffered before the previous block was applied. Remove it from buffer
                            // so it can be re-received with the correct prev_hash.
                            if e.to_string().contains("Invalid previous hash") {
                                println!("   Removing invalid buffered block {} - will be re-requested", next_height);
                                // Block already removed from buffer above, so we can continue
                            }
                            // Don't break - continue to check for next sequential block
                            // The invalid block has been removed, so next iteration will skip it
                            continue;
                        }
                    }
                }
                None => {
                    // No more sequential blocks in buffer
                    // Check if we have blocks ahead in the buffer - if so, request missing blocks
                    let buffer = block_buffer.read().await;
                    if let Some(&max_buffered_height) = buffer.keys().max() {
                        if max_buffered_height > next_height {
                            // We have blocks ahead but missing the next one - request missing blocks
                            // CRITICAL FIX: Request blocks ONE AT A TIME for missing sequential blocks
                            // This ensures we get the exact block we need, even if it doesn't exist on some peers
                            let request_from = next_height;
                            // Request only the next missing block first, then expand if needed
                            let request_to = next_height;
                            
                            println!("⚠️  Missing block {} (have blocks up to {} in buffer), requesting single block {}", 
                                next_height, max_buffered_height, request_from);
                            
                            drop(buffer);
                            
                            // Request missing block ONE AT A TIME if network_tx is available
                            if let Some(network_tx) = network_tx {
                                if let Err(e) = network_tx.send(NetworkCommand::RequestBlocks {
                                    from_height: request_from,
                                    to_height: request_to,
                                }) {
                                    eprintln!("Failed to request missing block: {}", e);
                                }
                            }
                            
                            // Break and wait for block to arrive
                            // Will retry on next call to process_buffered_blocks
                            break;
                        } else {
                            // No blocks ahead in buffer - we're caught up or waiting
                            break;
                        }
                    } else {
                        // Buffer is empty - no blocks to process
                        break;
                    }
                }
            }
        }
    }

    /// Check for chain reorganization opportunities
    /// When we have blocks that form a longer valid chain, reorganize to it
    async fn check_and_reorganize_chain(
        chain: &Arc<ChainState>,
        state: &Arc<AccountState>,
        timelock_state: &Arc<TimeLockState>,
        escrow_state: &Arc<EscrowState>,
        channel_state: &Arc<ChannelState>,
        trustline_state: &Arc<TrustLineState>,
        dimensional_pool_state: &Arc<DimensionalPoolState>,
        marketplace_state: &Arc<MarketplaceState>,
        validator: &Arc<BlockValidator>,
        block_buffer: &Arc<RwLock<HashMap<u64, coinject_core::Block>>>,
        network_cmd_tx: Option<&mpsc::UnboundedSender<NetworkCommand>>,
        cpp_network_cmd_tx: Option<&mpsc::UnboundedSender<CppNetworkCommand>>,
        peer_consensus: &Arc<PeerConsensus>,
    ) {
        let current_best_height = chain.best_block_height().await;
        let current_best_hash = chain.best_block_hash().await;
        
        println!("🔍 Reorganization check: Current best height: {}, hash: {:?}", current_best_height, current_best_hash);

        // Check if we have blocks in buffer that might form a longer chain
        let buffer = block_buffer.read().await;
        let buffer_info = if buffer.is_empty() {
            println!("🔍 Reorganization check: Buffer is empty, checking stored blocks only");
            (0, Vec::new())
        } else {
            let heights: Vec<u64> = buffer.keys().take(10).copied().collect();
            println!("🔍 Reorganization check: Buffer has {} blocks (heights: {:?})", buffer.len(), heights);
            (buffer.keys().max().copied().unwrap_or(0), heights)
        };
        drop(buffer);

        // Find the highest block in buffer
        let max_buffered_height = buffer_info.0;
        
        // v4.7.45 FIX: Use find_common_ancestor() for buffered blocks to handle earlier forks properly
        // If we have blocks that extend beyond our current best, check if they form a valid chain
        if max_buffered_height > current_best_height {
            // Re-acquire buffer lock to check for chain path
            let buffer = block_buffer.read().await;
            
            // Find the highest buffered block and use find_common_ancestor to check for forks
            if let Some(highest_block) = buffer.get(&max_buffered_height) {
                let highest_hash = highest_block.header.hash();
                drop(buffer); // Release lock before async call
                
                // Use find_common_ancestor to properly detect if this is a fork from earlier
                match chain.find_common_ancestor(&highest_hash, max_buffered_height).await {
                    Ok(Some((common_hash, common_height))) => {
                        if common_height < current_best_height {
                            // This buffered chain forks from before our current best - it's a reorganization candidate
                            println!("🔍 Reorganization check: Buffered chain (height {}) forks at common ancestor height {}", 
                                max_buffered_height, common_height);
                            
                            // The chain at max_buffered_height branches from common_height
                            // If it's longer than our current chain, it's a reorg candidate
                            let fork_length = max_buffered_height - common_height;
                            let our_length = current_best_height - common_height;
                            
                            if fork_length > our_length {
                                println!("🔍 Buffered fork is longer: fork has {} blocks after common ancestor, we have {}", 
                                    fork_length, our_length);
                            }
                        } else {
                            println!("🔍 Reorganization check: Buffered blocks connect to current chain at height {}", common_height);
                        }
                    },
                    Ok(None) => {
                        // COMPLETE FORK DETECTED: No common ancestor means we're on a completely different chain
                        // This requires a full chain review from genesis OR complete chain replacement
                        println!("🚨 COMPLETE FORK DETECTED: Buffered blocks at height {} have no common ancestor with current chain", max_buffered_height);
                        
                        // Check if the buffered chain is longer than ours
                        if max_buffered_height > current_best_height {
                            println!("   🔀 Fork chain is LONGER ({} > {})", max_buffered_height, current_best_height);
                            println!("   💡 Requesting full chain from best peer to resolve fork");
                            
                            // Use CPP network commands with best peer from peer_consensus
                            if let Some(cpp_tx) = cpp_network_cmd_tx {
                                // Get best peer from peer_consensus (highest height)
                                let active_peers = peer_consensus.active_peers().await;
                                
                                if let Some((peer_id_str, peer_state)) = active_peers.iter()
                                    .max_by_key(|(_, state)| state.best_height) {
                                    
                                    // Parse peer_id from hex string
                                    if let Ok(peer_id_bytes) = hex::decode(peer_id_str) {
                                        if peer_id_bytes.len() == 32 {
                                            let mut peer_id = [0u8; 32];
                                            peer_id.copy_from_slice(&peer_id_bytes[..32]);
                                            
                                            // Use chunked requests (16 blocks per request, CPP network limit)
                                            const CHUNK_SIZE: u64 = 16; // MAX_BLOCKS_PER_RESPONSE
                                            let mut current = 0u64;
                                            println!("   📦 Requesting fork chain in {} block chunks from peer {} (height: {}) via CPP...", 
                                                CHUNK_SIZE, &peer_id_str[..8], peer_state.best_height);
                                            
                                            while current <= max_buffered_height {
                                                let end = std::cmp::min(current + CHUNK_SIZE - 1, max_buffered_height);
                                                let request_id: u64 = rand::random();
                                                
                                                if let Err(e) = cpp_tx.send(CppNetworkCommand::RequestBlocks {
                                                    peer_id,
                                                    from_height: current,
                                                    to_height: end,
                                                    request_id,
                                                }) {
                                                    eprintln!("⚠️  Failed to request chain chunk {}-{}: {}", current, end, e);
                                                    break;
                                                }
                                                current = end + 1;
                                            }
                                            println!("   ✅ Requested full chain from genesis (0 to {}) in chunks via CPP", max_buffered_height);
                                        } else {
                                            eprintln!("⚠️  Invalid peer_id length: expected 32 bytes, got {}", peer_id_bytes.len());
                                        }
                                    } else {
                                        eprintln!("⚠️  Failed to decode peer_id from hex: {}", peer_id_str);
                                    }
                                } else {
                                    println!("   ⚠️  No active peers available from peer_consensus, cannot request blocks");
                                }
                            } else {
                                println!("   ⚠️  No CPP network command channel available to request full chain");
                            }
                        } else {
                            println!("   ℹ️ Fork chain is NOT longer ({} <= {}), keeping current chain", max_buffered_height, current_best_height);
                        }
                    },
                    Err(e) => {
                        println!("⚠️  Error finding common ancestor for buffered blocks: {:?}", e);
                    }
                }
            } else {
                drop(buffer);
            }
            
            // Also try sequential path building for directly connected blocks
            let buffer = block_buffer.read().await;
            let mut chain_path = Vec::new();
            let mut walk_hash = current_best_hash;
            let mut walk_height = current_best_height;

            // Try to find a path through buffered blocks
            while walk_height < max_buffered_height {
                let next_height = walk_height + 1;
                
                // Look for a block at next_height that connects to walk_hash
                let mut found = false;
                for (height, block) in buffer.iter() {
                    if *height == next_height && block.header.prev_hash == walk_hash {
                        chain_path.push(block.clone());
                        walk_hash = block.header.hash();
                        walk_height = next_height;
                        found = true;
                        break;
                    }
                }

                if !found {
                    // Can't form a complete chain from buffer at this point
                    break;
                }
            }
            drop(buffer);

            // If we found a complete chain path, it will be processed by process_buffered_blocks
            // This check is mainly for detecting forks
        }

        // Check for forks at same height - if we have a block at current height with different hash
        // and it's part of a longer chain, we should reorganize
        {
            let buffer = block_buffer.read().await;
            if let Some(fork_block) = buffer.get(&current_best_height) {
                if fork_block.header.hash() != current_best_hash {
                    // Fork detected - we'd need to request the full chain from the peer
                    // to see if it's longer. This is handled by status update handler.
                    println!("   Fork block at height {} detected in buffer, waiting for full chain...", current_best_height);
                }
            }
        }
        
        // Also check stored blocks for longer chains (not just buffer)
        // This is critical when we've received and stored blocks from a longer fork
        // Instead of scanning sequentially (which stops at first missing block),
        // scan the buffer for blocks that might connect to our chain, then check if they're stored
        let mut max_stored_height = current_best_height;
        let mut max_stored_hash = current_best_hash;
        
        // First, check buffer for blocks that might form a chain
        // Look for blocks whose previous hash matches blocks in our current chain
        let buffer = block_buffer.read().await;
        let buffer_heights: Vec<u64> = buffer.keys().copied().collect();
        drop(buffer);
        
        if !buffer_heights.is_empty() {
            println!("🔍 Reorganization check: Checking {} buffered blocks for chain connections", buffer_heights.len());
            
            // Find the highest block in buffer
            let max_buffered_height = *buffer_heights.iter().max().unwrap_or(&current_best_height);
            
            // Try to find a chain path from current best to buffered blocks
            // Check ALL blocks in buffer, not just the highest one, to find any that connect
            if max_buffered_height > current_best_height {
                let buffer = block_buffer.read().await;
                let mut best_candidate_height = current_best_height;
                let mut best_candidate_hash = current_best_hash;
                
                // Iterate through buffered blocks to find ANY that connect to our current best chain
                // Sort by height descending to check highest blocks first
                let mut sorted_heights: Vec<u64> = buffer_heights.iter().copied().filter(|&h| h > current_best_height).collect();
                sorted_heights.sort_by(|a, b| b.cmp(a)); // Descending order
                
                // Walk back from current best to build a set of hashes that are on our current chain
                let mut current_chain_hashes = std::collections::HashSet::new();
                let mut walk_back_hash = current_best_hash;
                let mut walk_back_height = current_best_height;
                for _ in 0..1000 { // Walk back up to 1000 blocks
                    current_chain_hashes.insert(walk_back_hash);
                    if walk_back_height == 0 {
                        break;
                    }
                    if let Ok(Some(prev_block)) = chain.get_block_by_hash(&walk_back_hash) {
                        walk_back_hash = prev_block.header.prev_hash;
                        walk_back_height -= 1;
                    } else {
                        break;
                    }
                }
                
                for &check_height in sorted_heights.iter().take(100) { // Limit to top 100 to avoid excessive checks
                    if let Some(block) = buffer.get(&check_height) {
                        // Check if this block's previous hash is on our current best chain
                        if current_chain_hashes.contains(&block.header.prev_hash) {
                            // Found a connection! This block connects to our current best chain
                            // Walk forward from this connection point to see how far the chain extends
                            let mut walk_height = check_height;
                            let mut walk_hash = block.header.hash();
                            let mut valid_chain = true;
                            let mut chain_end_height = check_height;
                            let mut chain_end_hash = walk_hash;
                            
                            // Walk forward to find the end of this chain
                            while valid_chain {
                                // Check if next block exists in buffer or is stored
                                let next_height = walk_height + 1;
                                let mut found_next = false;
                                
                                // Check buffer first
                                if let Some(next_block) = buffer.get(&next_height) {
                                    if next_block.header.prev_hash == walk_hash {
                                        walk_height = next_height;
                                        walk_hash = next_block.header.hash();
                                        chain_end_height = next_height;
                                        chain_end_hash = walk_hash;
                                        found_next = true;
                                    }
                                }
                                
                                // Also check if stored block exists at next height
                                if !found_next {
                                    if let Ok(Some(stored_block)) = chain.get_block_by_height(next_height) {
                                        if stored_block.header.prev_hash == walk_hash {
                                            walk_height = next_height;
                                            walk_hash = stored_block.header.hash();
                                            chain_end_height = next_height;
                                            chain_end_hash = walk_hash;
                                            found_next = true;
                                        }
                                    }
                                }
                                
                                if !found_next {
                                    valid_chain = false;
                                }
                                
                                // Limit walk to prevent infinite loops
                                if walk_height > check_height + 1000 {
                                    break;
                                }
                            }
                            
                            // If this chain is longer than our best candidate, use it
                            if chain_end_height > best_candidate_height {
                                best_candidate_height = chain_end_height;
                                best_candidate_hash = chain_end_hash;
                                println!("🔍 Reorganization check: Found potential chain connection at height {} (connects to current chain at prev_hash {:?}, hash: {:?})", 
                                    chain_end_height, block.header.prev_hash, chain_end_hash);
                            }
                        }
                    }
                }
                
                if best_candidate_height > current_best_height {
                    max_stored_height = best_candidate_height;
                    max_stored_hash = best_candidate_hash;
                }
            }
        }
        
        // Also do sequential scan for blocks that are directly connected (no gaps)
        // This handles the case where blocks are stored sequentially
        // Scan up to 1000 blocks ahead, but don't stop at first missing block
        let scan_limit = current_best_height + 1000;
        println!("🔍 Reorganization check: Also scanning stored blocks sequentially from height {} to {} (continuing past gaps)", current_best_height + 1, scan_limit);
        for height in (current_best_height + 1)..=scan_limit {
            if let Ok(Some(block)) = chain.get_block_by_height(height) {
                if height <= current_best_height + 10 {
                    println!("🔍 Reorganization check: Found block at height {} in sequential scan", height);
                }
                // Verify this block is part of a valid chain by checking its previous block
                if let Ok(Some(prev_block)) = chain.get_block_by_hash(&block.header.prev_hash) {
                    if prev_block.header.height == height - 1 {
                        // Valid chain continuation
                        if height > max_stored_height {
                            max_stored_height = height;
                            max_stored_hash = block.header.hash();
                        }
                    } else {
                        // Chain broken - but don't stop, continue scanning
                        if height <= current_best_height + 10 {
                            println!("🔍 Reorganization check: Chain broken at height {} (prev block at height {}), continuing scan", height, prev_block.header.height);
                        }
                    }
                } else {
                    // Previous block not found - but don't stop, continue scanning
                    if height <= current_best_height + 10 {
                        println!("🔍 Reorganization check: Previous block not found for height {} (prev_hash: {:?}), continuing scan", height, block.header.prev_hash);
                    }
                }
            }
            // Don't break on missing blocks - continue scanning to find any stored blocks
        }
        
        // If we found blocks ahead but with gaps, request missing blocks aggressively
        if max_stored_height > current_best_height + 1 {
            // We have blocks ahead but possibly with gaps
            // Request the full range to fill gaps for reorganization
            let from_height = current_best_height + 1;
            let to_height = max_stored_height;
            
            println!("📡 Requesting missing blocks {} to {} to complete chain for reorganization", 
                from_height, to_height);
            
            if let Some(cmd_tx) = network_cmd_tx {
                if let Err(e) = cmd_tx.send(NetworkCommand::RequestBlocks {
                    from_height,
                    to_height,
                }) {
                    eprintln!("⚠️  Failed to request missing blocks for reorganization: {}", e);
                }
            }
        }
        
        // If we found a longer chain in stored blocks, attempt reorganization
        if max_stored_height > current_best_height {
            println!("🔍 Found longer chain in stored blocks (height {}), attempting reorganization...", max_stored_height);
            
            // Check if this chain has no common ancestor (complete fork)
            // If so, we need to validate from genesis
            match chain.find_common_ancestor(&max_stored_hash, max_stored_height).await {
                Ok(Some((common_hash, common_height))) => {
                    println!("   Found common ancestor at height {}", common_height);
                    // Normal reorganization with common ancestor
                }
                Ok(None) => {
                    println!("   ⚠️  No common ancestor - this is a complete fork, will validate from genesis");
                }
                Err(e) => {
                    println!("   ⚠️  Error finding common ancestor: {:?}", e);
                }
            }
            
            let chain_clone = Arc::clone(chain);
            let state_clone = Arc::clone(state);
            let timelock_clone = Arc::clone(timelock_state);
            let escrow_clone = Arc::clone(escrow_state);
            let channel_clone = Arc::clone(channel_state);
            let trustline_clone = Arc::clone(trustline_state);
            let dimensional_clone = Arc::clone(dimensional_pool_state);
            let marketplace_clone = Arc::clone(marketplace_state);
            let validator_clone = Arc::clone(validator);
            
            tokio::spawn(async move {
                if let Err(e) = Self::attempt_reorganization_if_longer_chain(
                    max_stored_hash,
                    max_stored_height,
                    &chain_clone,
                    &state_clone,
                    &timelock_clone,
                    &escrow_clone,
                    &channel_clone,
                    &trustline_clone,
                    &dimensional_clone,
                    &marketplace_clone,
                    &validator_clone,
                ).await {
                    eprintln!("⚠️  Failed to attempt reorganization for stored blocks: {}", e);
                }
            });
        }
    }

    /// Attempt chain reorganization when we have a longer valid chain available
    /// This is called when we've received blocks that form a longer chain than our current best
    async fn attempt_reorganization_if_longer_chain(
        new_chain_end_hash: coinject_core::Hash,
        new_chain_end_height: u64,
        chain: &Arc<ChainState>,
        state: &Arc<AccountState>,
        timelock_state: &Arc<TimeLockState>,
        escrow_state: &Arc<EscrowState>,
        channel_state: &Arc<ChannelState>,
        trustline_state: &Arc<TrustLineState>,
        dimensional_pool_state: &Arc<DimensionalPoolState>,
        marketplace_state: &Arc<MarketplaceState>,
        validator: &Arc<BlockValidator>,
    ) -> Result<bool, String> {

        let current_best_height = chain.best_block_height().await;
        let current_best_hash = chain.best_block_hash().await;

        // Only reorganize if new chain is actually longer
        if new_chain_end_height <= current_best_height {
            return Ok(false);
        }

        // COMMON ANCESTOR ANCHORING: Find common ancestor and validate it's anchored
        // This ensures we only reorganize to chains that share a valid common ancestor
        // The common ancestor must be:
        // 1. At least 6 blocks deep (to prevent shallow reorganizations)
        // 2. Valid and stored in our chain
        // 3. Not at genesis (unless absolutely necessary)
        let (common_hash, common_height) = match chain.find_common_ancestor(&new_chain_end_hash, new_chain_end_height).await
            .map_err(|e| format!("Failed to find common ancestor: {}", e)) {
            Ok(Some((hash, height))) => {
                // Validate common ancestor is anchored (at least 6 blocks deep)
                const MIN_ANCHOR_DEPTH: u64 = 6;
                if height < MIN_ANCHOR_DEPTH {
                    println!("⚠️  Common ancestor at height {} is too shallow (min depth: {}), cannot reorganize", 
                        height, MIN_ANCHOR_DEPTH);
                    return Ok(false);
                }
                
                // Verify common ancestor block exists and is valid
                match chain.get_block_by_hash(&hash) {
                    Ok(Some(block)) => {
                        // Verify the block is actually on our current chain
                        let current_best = chain.best_block_height().await;
                        if block.header.height > current_best {
                            println!("⚠️  Common ancestor block at height {} is ahead of our best ({})", 
                                block.header.height, current_best);
                            return Ok(false);
                        }
                        (hash, height)
                    }
                    Ok(None) => {
                        println!("⚠️  Common ancestor block not found in storage");
                        return Ok(false);
                    }
                    Err(e) => {
                        println!("⚠️  Error verifying common ancestor: {}", e);
                        return Ok(false);
                    }
                }
            }
            Ok(None) => {
                // COMPLETE FORK DETECTED: No common ancestor means we're on a completely different chain
                // This requires a full chain review from genesis
                println!("🚨 COMPLETE FORK DETECTED: No common ancestor found with chain ending at height {}", new_chain_end_height);
                println!("   This requires full chain validation from genesis");
                
                // Request full chain from genesis to validate the new chain
                // The caller should have already requested blocks, but we need to ensure we have the full chain
                // For now, we'll attempt to validate what we have and request if needed
                // This will be handled by the reorganization check that triggers this
                
                // Validate the new chain from genesis
                match Self::validate_chain_from_genesis(
                    &new_chain_end_hash,
                    new_chain_end_height,
                    chain,
                    validator,
                ).await {
                    Ok((new_chain_blocks, new_chain_work)) => {
                        println!("✅ New chain validated from genesis: {} blocks, total work: {}", 
                            new_chain_blocks.len(), new_chain_work);
                        
                        // Get our current chain from genesis
                        let (our_chain_blocks, our_chain_work) = match Self::get_chain_from_genesis(
                            current_best_hash,
                            current_best_height,
                            chain,
                        ).await {
                            Ok(chain_data) => chain_data,
                            Err(e) => {
                                println!("⚠️  Failed to get our chain from genesis: {}", e);
                                return Ok(false);
                            }
                        };
                        
                        println!("📊 Complete chain comparison:");
                        println!("   Our chain: {} blocks, total work: {}", our_chain_blocks.len(), our_chain_work);
                        println!("   New chain: {} blocks, total work: {}", new_chain_blocks.len(), new_chain_work);
                        
                        // Compare by work score first, then height
                        use crate::peer_consensus::WorkScoreCalculator;
                        let comparison = WorkScoreCalculator::compare_chains(our_chain_work, new_chain_work);
                        
                        if comparison <= 0 && new_chain_end_height <= current_best_height {
                            // Our chain has equal or more work and equal or greater height
                            println!("   ⏸️  Skipping reorganization: our chain has equal or better work/height");
                            return Ok(false);
                        }
                        
                        // New chain is better - reorganize from genesis
                        println!("🔄 Reorganizing from genesis: unwinding {} blocks (work: {}), applying {} blocks (work: {})",
                            our_chain_blocks.len(), our_chain_work, new_chain_blocks.len(), new_chain_work);
                        
                        // Perform complete reorganization (unwind all blocks to genesis, apply new chain)
                        Self::reorganize_chain_from_genesis(
                            our_chain_blocks,
                            new_chain_blocks,
                            chain,
                            state,
                            timelock_state,
                            escrow_state,
                            channel_state,
                            trustline_state,
                            dimensional_pool_state,
                            marketplace_state,
                            validator,
                        ).await?;
                        
                        return Ok(true);
                    }
                    Err(e) => {
                        println!("⚠️  Failed to validate new chain from genesis: {}", e);
                        println!("   Requesting full chain from genesis for validation...");
                        // Return false but the caller should request full chain
                        return Ok(false);
                    }
                }
            }
            Err(e) => return Err(e),
        };

        println!("🔄 Found anchored common ancestor at height {} (hash: {:?})", common_height, common_hash);

        // Get old chain blocks (from common ancestor to current best, excluding common ancestor)
        let mut old_chain_blocks = Vec::new();
        if common_height < current_best_height {
            for height in (common_height + 1)..=current_best_height {
                match chain.get_block_by_height(height) {
                    Ok(Some(block)) => old_chain_blocks.push(block),
                    Ok(None) => return Err(format!("Failed to get old chain block at height {}", height)),
                    Err(e) => return Err(format!("Error getting old chain block at height {}: {}", height, e)),
                }
            }
            old_chain_blocks.reverse(); // Reverse so we unwind from newest to oldest
        }

        // Get new chain blocks (from common ancestor to new best, excluding common ancestor)
        let mut new_chain_blocks = Vec::new();
        let mut current_hash = new_chain_end_hash;
        let mut current_height = new_chain_end_height;

        // Walk back from new best to common ancestor, collecting blocks
        while current_height > common_height {
            match chain.get_block_by_hash(&current_hash) {
                Ok(Some(block)) => {
                    new_chain_blocks.push(block.clone());
                    current_hash = block.header.prev_hash;
                    current_height -= 1;
                }
                Ok(None) => return Err(format!("Failed to get new chain block at height {}", current_height)),
                Err(e) => return Err(format!("Error getting new chain block at height {}: {}", current_height, e)),
            }
        }

        // Reverse new_chain_blocks so they're in forward order (common+1 to new_best)
        new_chain_blocks.reverse();

        // CRITICAL: Compare chains by work score, not just length
        // A longer chain might have less total work if blocks have lower work scores
        use crate::peer_consensus::WorkScoreCalculator;
        
        // Calculate cumulative work scores for both chains
        // Work scores are f64, so we need to convert to u64 for comparison
        let old_chain_work: u64 = old_chain_blocks.iter()
            .map(|b| b.header.work_score as u64)
            .sum();
        let new_chain_work: u64 = new_chain_blocks.iter()
            .map(|b| b.header.work_score as u64)
            .sum();
        
        println!("📊 Chain comparison:");
        println!("   Old chain: {} blocks, total work: {}", old_chain_blocks.len(), old_chain_work);
        println!("   New chain: {} blocks, total work: {}", new_chain_blocks.len(), new_chain_work);
        
        // Use work score comparison (with tolerance)
        let comparison = WorkScoreCalculator::compare_chains(old_chain_work, new_chain_work);
        
        if comparison <= 0 {
            // Our chain has equal or more work, don't reorganize
            println!("   ⏸️  Skipping reorganization: our chain has equal or more work (comparison: {})", comparison);
            return Ok(false);
        }
        
        // New chain has more work - proceed with reorganization
        println!("🔄 Reorganizing: unwinding {} blocks (work: {}), applying {} blocks (work: {})",
            old_chain_blocks.len(), old_chain_work, new_chain_blocks.len(), new_chain_work);

        // Perform reorganization
        Self::reorganize_chain(
            old_chain_blocks,
            new_chain_blocks,
            chain,
            state,
            timelock_state,
            escrow_state,
            channel_state,
            trustline_state,
            dimensional_pool_state,
            marketplace_state,
            validator,
        ).await?;

        Ok(true)
    }

    /// Apply block transactions to account state
    /// Returns a vector of successfully applied transaction hashes
    fn apply_block_transactions(
        block: &coinject_core::Block,
        state: &Arc<AccountState>,
        timelock_state: &Arc<TimeLockState>,
        escrow_state: &Arc<EscrowState>,
        channel_state: &Arc<ChannelState>,
        trustline_state: &Arc<TrustLineState>,
        dimensional_pool_state: &Arc<DimensionalPoolState>,
        marketplace_state: &Arc<MarketplaceState>,
    ) -> Result<Vec<coinject_core::Hash>, String> {
        // Apply coinbase reward
        let miner = block.header.miner;
        let reward = block.coinbase.reward;
        let current_balance = state.get_balance(&miner);
        state.set_balance(&miner, current_balance + reward)
            .map_err(|e| format!("Failed to set miner balance: {}", e))?;

        let mut applied_txs = Vec::new();
        let block_height = block.header.height;

        // Apply regular transactions
        for tx in &block.transactions {
            // Apply the transaction
            match Self::apply_single_transaction(tx, state, timelock_state, escrow_state, channel_state, trustline_state, dimensional_pool_state, marketplace_state, block_height) {
                Ok(()) => {
                    applied_txs.push(tx.hash());
                }
                Err(e) => {
                    println!("⚠️  Skipping transaction {:?}: {}", tx.hash(), e);
                    continue; // Skip this transaction and continue with the rest
                }
            }
        }

        if applied_txs.len() < block.transactions.len() {
            println!("📊 Applied {}/{} transactions from block",
                applied_txs.len(), block.transactions.len());
        }

        Ok(applied_txs)
    }

    /// Unwind block transactions (reverse apply_block_transactions)
    /// Used for chain reorganization
    fn unwind_block_transactions(
        block: &coinject_core::Block,
        state: &Arc<AccountState>,
        timelock_state: &Arc<TimeLockState>,
        escrow_state: &Arc<EscrowState>,
        channel_state: &Arc<ChannelState>,
        trustline_state: &Arc<TrustLineState>,
        dimensional_pool_state: &Arc<DimensionalPoolState>,
        marketplace_state: &Arc<MarketplaceState>,
    ) -> Result<(), String> {
        let block_height = block.header.height;

        // Unwind transactions in reverse order
        for tx in block.transactions.iter().rev() {
            if let Err(e) = Self::unwind_single_transaction(tx, state, timelock_state, escrow_state, channel_state, trustline_state, dimensional_pool_state, marketplace_state, block_height) {
                println!("⚠️  Warning: Failed to unwind transaction {:?}: {}", tx.hash(), e);
                // Continue unwinding other transactions even if one fails
            }
        }

        // Unwind coinbase reward
        let miner = block.header.miner;
        let reward = block.coinbase.reward;
        let current_balance = state.get_balance(&miner);
        if current_balance >= reward {
            state.set_balance(&miner, current_balance - reward)
                .map_err(|e| format!("Failed to unwind miner reward: {}", e))?;
        } else {
            // Miner balance insufficient - this shouldn't happen but handle gracefully
            println!("⚠️  Warning: Miner balance {} < reward {}, setting to 0", current_balance, reward);
            state.set_balance(&miner, 0)
                .map_err(|e| format!("Failed to set miner balance: {}", e))?;
        }

        Ok(())
    }

    /// Unwind a single transaction (reverse apply_single_transaction)
    fn unwind_single_transaction(
        tx: &coinject_core::Transaction,
        state: &Arc<AccountState>,
        timelock_state: &Arc<TimeLockState>,
        escrow_state: &Arc<EscrowState>,
        channel_state: &Arc<ChannelState>,
        _trustline_state: &Arc<TrustLineState>,
        _dimensional_pool_state: &Arc<DimensionalPoolState>,
        _marketplace_state: &Arc<MarketplaceState>,
        _block_height: u64,
    ) -> Result<(), String> {
        use coinject_core::{EscrowType, ChannelType};
        use coinject_state::{EscrowStatus, ChannelStatus};

        match tx {
            coinject_core::Transaction::Transfer(transfer_tx) => {
                // Reverse: credit sender, debit recipient, decrement nonce
                let sender_balance = state.get_balance(&transfer_tx.from);
                state.set_balance(&transfer_tx.from, sender_balance + transfer_tx.amount + transfer_tx.fee)
                    .map_err(|e| format!("Failed to unwind sender balance: {}", e))?;
                
                let current_nonce = state.get_nonce(&transfer_tx.from);
                if current_nonce > 0 {
                    state.set_nonce(&transfer_tx.from, current_nonce - 1)
                        .map_err(|e| format!("Failed to unwind sender nonce: {}", e))?;
                }

                let recipient_balance = state.get_balance(&transfer_tx.to);
                if recipient_balance >= transfer_tx.amount {
                    state.set_balance(&transfer_tx.to, recipient_balance - transfer_tx.amount)
                        .map_err(|e| format!("Failed to unwind recipient balance: {}", e))?;
                } else {
                    // Recipient balance insufficient - set to 0
                    state.set_balance(&transfer_tx.to, 0)
                        .map_err(|e| format!("Failed to set recipient balance: {}", e))?;
                }

                Ok(())
            }

            coinject_core::Transaction::TimeLock(timelock_tx) => {
                // Reverse: credit sender, remove timelock, decrement nonce
                let sender_balance = state.get_balance(&timelock_tx.from);
                state.set_balance(&timelock_tx.from, sender_balance + timelock_tx.amount + timelock_tx.fee)
                    .map_err(|e| format!("Failed to unwind sender balance: {}", e))?;
                
                let current_nonce = state.get_nonce(&timelock_tx.from);
                if current_nonce > 0 {
                    state.set_nonce(&timelock_tx.from, current_nonce - 1)
                        .map_err(|e| format!("Failed to unwind sender nonce: {}", e))?;
                }

                // Remove timelock if it exists
                let _ = timelock_state.remove_timelock(&tx.hash());
                Ok(())
            }

            coinject_core::Transaction::Escrow(escrow_tx) => {
                match &escrow_tx.escrow_type {
                    EscrowType::Create { .. } => {
                        // Reverse: credit sender, remove escrow, decrement nonce
                        let sender_balance = state.get_balance(&escrow_tx.from);
                        // We need to get the escrow to know the amount
                        if let Some(escrow) = escrow_state.get_escrow(&escrow_tx.escrow_id) {
                            state.set_balance(&escrow_tx.from, sender_balance + escrow.amount + escrow_tx.fee)
                                .map_err(|e| format!("Failed to unwind sender balance: {}", e))?;
                            
                            let current_nonce = state.get_nonce(&escrow_tx.from);
                            if current_nonce > 0 {
                                state.set_nonce(&escrow_tx.from, current_nonce - 1)
                                    .map_err(|e| format!("Failed to unwind sender nonce: {}", e))?;
                            }

                            // Remove escrow - note: perfect reversal requires delete method
                            // For now, we mark it as an approximate reversal
                            println!("   ⚠️  Escrow deletion requires delete_escrow method - state may be approximate");
                        }
                        Ok(())
                    }

                    EscrowType::Release => {
                        // Reverse: debit recipient, restore escrow to active
                        if let Some(escrow) = escrow_state.get_escrow(&escrow_tx.escrow_id) {
                            let recipient_balance = state.get_balance(&escrow.recipient);
                            if recipient_balance >= escrow.amount {
                                state.set_balance(&escrow.recipient, recipient_balance - escrow.amount)
                                    .map_err(|e| format!("Failed to unwind recipient balance: {}", e))?;
                            } else {
                                state.set_balance(&escrow.recipient, 0)
                                    .map_err(|e| format!("Failed to set recipient balance: {}", e))?;
                            }

                            // Restore escrow to active
                            escrow_state.update_escrow_status(&escrow_tx.escrow_id, EscrowStatus::Active, None)?;
                        }
                        Ok(())
                    }

                    EscrowType::Refund => {
                        // Reverse: debit sender, restore escrow to active
                        if let Some(escrow) = escrow_state.get_escrow(&escrow_tx.escrow_id) {
                            let sender_balance = state.get_balance(&escrow.sender);
                            if sender_balance >= escrow.amount {
                                state.set_balance(&escrow.sender, sender_balance - escrow.amount)
                                    .map_err(|e| format!("Failed to unwind sender balance: {}", e))?;
                            } else {
                                state.set_balance(&escrow.sender, 0)
                                    .map_err(|e| format!("Failed to set sender balance: {}", e))?;
                            }

                            // Restore escrow to active
                            escrow_state.update_escrow_status(&escrow_tx.escrow_id, EscrowStatus::Active, None)?;
                        }
                        Ok(())
                    }
                }
            }

            coinject_core::Transaction::Channel(channel_tx) => {
                match &channel_tx.channel_type {
                    ChannelType::Open { participant_a, participant_b, deposit_a, deposit_b, .. } => {
                        // Reverse: credit initiator, remove channel, decrement nonce
                        let initiator_deposit = if &channel_tx.from == participant_a { *deposit_a } else { *deposit_b };
                        let initiator_balance = state.get_balance(&channel_tx.from);
                        state.set_balance(&channel_tx.from, initiator_balance + initiator_deposit + channel_tx.fee)
                            .map_err(|e| format!("Failed to unwind initiator balance: {}", e))?;
                        
                        let current_nonce = state.get_nonce(&channel_tx.from);
                        if current_nonce > 0 {
                            state.set_nonce(&channel_tx.from, current_nonce - 1)
                                .map_err(|e| format!("Failed to unwind initiator nonce: {}", e))?;
                        }

                        // Remove channel - note: perfect reversal requires delete method
                        println!("   ⚠️  Channel deletion requires delete_channel method - state may be approximate");
                        Ok(())
                    }

                    ChannelType::Update { .. } => {
                        // Channel updates are state changes, hard to reverse perfectly
                        // For now, just log - in practice, we'd need to track previous state
                        println!("⚠️  Warning: Cannot perfectly reverse channel update, state may be inconsistent");
                        Ok(())
                    }

                    ChannelType::CooperativeClose { final_balance_a, final_balance_b } => {
                        // Reverse: debit both participants, restore channel
                        if let Some(channel) = channel_state.get_channel(&channel_tx.channel_id) {
                            let balance_a = state.get_balance(&channel.participant_a);
                            if balance_a >= *final_balance_a {
                                state.set_balance(&channel.participant_a, balance_a - *final_balance_a)
                                    .map_err(|e| format!("Failed to unwind participant A balance: {}", e))?;
                            } else {
                                state.set_balance(&channel.participant_a, 0)
                                    .map_err(|e| format!("Failed to set participant A balance: {}", e))?;
                            }

                            let balance_b = state.get_balance(&channel.participant_b);
                            if balance_b >= *final_balance_b {
                                state.set_balance(&channel.participant_b, balance_b - *final_balance_b)
                                    .map_err(|e| format!("Failed to unwind participant B balance: {}", e))?;
                            } else {
                                state.set_balance(&channel.participant_b, 0)
                                    .map_err(|e| format!("Failed to set participant B balance: {}", e))?;
                            }

                            // Restore channel to open (approximate - we don't have exact previous state)
                            // This is a limitation - we'd need to store channel history
                            println!("⚠️  Warning: Channel state restoration is approximate");
                        }
                        Ok(())
                    }

                    ChannelType::UnilateralClose { .. } => {
                        // Reverse dispute - restore channel state
                        // This is complex and approximate
                        println!("⚠️  Warning: Cannot perfectly reverse unilateral close, state may be inconsistent");
                        Ok(())
                    }
                }
            }

            coinject_core::Transaction::TrustLine(trustline_tx) => {
                // Reverse: credit fee, decrement nonce, reverse trustline operation
                let sender_balance = state.get_balance(&trustline_tx.from);
                state.set_balance(&trustline_tx.from, sender_balance + trustline_tx.fee)
                    .map_err(|e| format!("Failed to unwind sender balance: {}", e))?;
                
                let current_nonce = state.get_nonce(&trustline_tx.from);
                if current_nonce > 0 {
                    state.set_nonce(&trustline_tx.from, current_nonce - 1)
                        .map_err(|e| format!("Failed to unwind sender nonce: {}", e))?;
                }

                use coinject_core::TrustLineType;
                match &trustline_tx.trustline_type {
                    TrustLineType::Create { .. } => {
                        // Remove trustline - note: perfect reversal requires delete method
                        println!("   ⚠️  TrustLine deletion requires delete_trustline method - state may be approximate");
                    }
                    TrustLineType::UpdateLimits { .. } | TrustLineType::Freeze | TrustLineType::EvolvePhase { .. } => {
                        // These are state changes - hard to reverse perfectly
                        // In practice, we'd need to store previous state
                        println!("⚠️  Warning: TrustLine state reversal is approximate");
                    }
                    TrustLineType::Close => {
                        // Restore trustline - this is complex, would need previous state
                        println!("⚠️  Warning: Cannot perfectly reverse trustline close");
                    }
                }
                Ok(())
            }

            coinject_core::Transaction::DimensionalPoolSwap(pool_swap_tx) => {
                // Reverse: credit fee, decrement nonce, reverse swap
                let sender_balance = state.get_balance(&pool_swap_tx.from);
                state.set_balance(&pool_swap_tx.from, sender_balance + pool_swap_tx.fee)
                    .map_err(|e| format!("Failed to unwind sender balance: {}", e))?;
                
                let current_nonce = state.get_nonce(&pool_swap_tx.from);
                if current_nonce > 0 {
                    state.set_nonce(&pool_swap_tx.from, current_nonce - 1)
                        .map_err(|e| format!("Failed to unwind sender nonce: {}", e))?;
                }

                // Reverse swap - this is complex and may not be perfectly reversible
                // We'd need to track swap history
                println!("⚠️  Warning: Dimensional pool swap reversal is approximate");
                Ok(())
            }

            coinject_core::Transaction::Marketplace(marketplace_tx) => {
                use coinject_core::MarketplaceOperation;
                
                // Reverse: credit fee, decrement nonce
                let sender_balance = state.get_balance(&marketplace_tx.from);
                state.set_balance(&marketplace_tx.from, sender_balance + marketplace_tx.fee)
                    .map_err(|e| format!("Failed to unwind sender balance: {}", e))?;
                
                let current_nonce = state.get_nonce(&marketplace_tx.from);
                if current_nonce > 0 {
                    state.set_nonce(&marketplace_tx.from, current_nonce - 1)
                        .map_err(|e| format!("Failed to unwind sender nonce: {}", e))?;
                }

                match &marketplace_tx.operation {
                    MarketplaceOperation::SubmitProblem { bounty, .. } => {
                        // Reverse: credit bounty back, remove problem
                        state.set_balance(&marketplace_tx.from, sender_balance + marketplace_tx.fee + bounty)
                            .map_err(|e| format!("Failed to unwind problem submission: {}", e))?;
                        // Remove problem - would need problem_id
                        println!("⚠️  Warning: Problem removal requires problem_id tracking");
                    }
                    MarketplaceOperation::SubmitSolution { problem_id, .. } => {
                        // Reverse: remove solution, potentially reverse auto-payout
                        // This is complex - we'd need to track if bounty was paid
                        println!("⚠️  Warning: Solution reversal is approximate");
                    }
                    MarketplaceOperation::ClaimBounty { problem_id } => {
                        // Reverse: debit solver, restore bounty to escrow
                        // Would need to track who received the bounty
                        println!("⚠️  Warning: Bounty claim reversal requires tracking");
                    }
                    MarketplaceOperation::CancelProblem { problem_id } => {
                        // Reverse: debit refund, restore problem
                        // Would need to track refund amount
                        println!("⚠️  Warning: Problem cancellation reversal requires tracking");
                    }
                }
                Ok(())
            }
        }
    }

    /// Perform chain reorganization: unwind old chain and apply new chain
    async fn reorganize_chain(
        old_chain_blocks: Vec<coinject_core::Block>,
        new_chain_blocks: Vec<coinject_core::Block>,
        chain: &Arc<ChainState>,
        state: &Arc<AccountState>,
        timelock_state: &Arc<TimeLockState>,
        escrow_state: &Arc<EscrowState>,
        channel_state: &Arc<ChannelState>,
        trustline_state: &Arc<TrustLineState>,
        dimensional_pool_state: &Arc<DimensionalPoolState>,
        marketplace_state: &Arc<MarketplaceState>,
        validator: &Arc<BlockValidator>,
    ) -> Result<(), String> {
        println!("🔄 Starting chain reorganization: unwinding {} blocks, applying {} blocks",
            old_chain_blocks.len(), new_chain_blocks.len());

        // Step 1: Unwind old chain blocks (in reverse order - newest to oldest)
        for block in old_chain_blocks.iter().rev() {
            println!("   Unwinding block {}...", block.header.height);
            if let Err(e) = Self::unwind_block_transactions(
                block, state, timelock_state, escrow_state, channel_state,
                trustline_state, dimensional_pool_state, marketplace_state,
            ) {
                return Err(format!("Failed to unwind block {}: {}", block.header.height, e));
            }

            // Also need to reverse dimensional pool state changes
            // This is complex - for now we log a warning
            if block.header.height > 0 {
                println!("   ⚠️  Note: Dimensional pool state reversal is approximate");
            }
        }

        // Step 2: Validate new chain
        let mut prev_hash = if let Some(first_block) = new_chain_blocks.first() {
            first_block.header.prev_hash
        } else {
            return Err("New chain is empty".to_string());
        };

        for (idx, block) in new_chain_blocks.iter().enumerate() {
            let _expected_height = if idx == 0 {
                // First block height should be common_ancestor_height + 1
                // We'd need to pass this in, but for now we validate relative to prev_hash
                0 // Will be set properly
            } else {
                new_chain_blocks[idx - 1].header.height + 1
            };

            // Validate block connects to previous
            if block.header.prev_hash != prev_hash {
                return Err(format!("New chain block {} doesn't connect to previous (prev_hash mismatch)", block.header.height));
            }

            // Validate block (skip timestamp age check during chain reorganization/sync)
            match validator.validate_block_with_options(block, &prev_hash, block.header.height, true) {
                Ok(()) => {
                    prev_hash = block.header.hash();
                }
                Err(e) => {
                    return Err(format!("New chain block {} validation failed: {}", block.header.height, e));
                }
            }
        }

        // Step 3: Apply new chain blocks
        for block in &new_chain_blocks {
            println!("   Applying new chain block {}...", block.header.height);
            
            // Store block
            chain.store_block(block).await
                .map_err(|e| format!("Failed to store block {}: {}", block.header.height, e))?;

            // Apply transactions
            Self::apply_block_transactions(
                block, state, timelock_state, escrow_state, channel_state,
                trustline_state, dimensional_pool_state, marketplace_state,
            )?;

            // Update consensus state
            use coinject_core::{TAU_C, ConsensusState};
            let tau = (block.header.height as f64) / TAU_C;
            let consensus_state = ConsensusState::at_tau(tau);
            dimensional_pool_state.save_consensus_state(block.header.height, &consensus_state)
                .map_err(|e| format!("Failed to save consensus state: {}", e))?;
        }

        // Step 4: Update best chain
        if let Some(last_block) = new_chain_blocks.last() {
            chain.update_best_chain(last_block.header.hash(), last_block.header.height).await
                .map_err(|e| format!("Failed to update best chain: {}", e))?;
        }

        println!("✅ Chain reorganization complete!");
        Ok(())
    }

    /// Validate a chain from genesis block
    /// Returns (chain_blocks, total_work_score) if valid
    async fn validate_chain_from_genesis(
        end_hash: &coinject_core::Hash,
        end_height: u64,
        chain: &Arc<ChainState>,
        validator: &Arc<BlockValidator>,
    ) -> Result<(Vec<coinject_core::Block>, u64), String> {
        let genesis_hash = chain.genesis_hash();
        let mut chain_blocks = Vec::new();
        let mut current_hash = *end_hash;
        let mut current_height = end_height;
        let mut total_work: u64 = 0;

        // Walk back from end to genesis, collecting blocks
        while current_height > 0 {
            match chain.get_block_by_hash(&current_hash) {
                Ok(Some(block)) => {
                    // Validate block connects properly
                    if block.header.height != current_height {
                        return Err(format!("Block height mismatch: expected {}, got {}", 
                            current_height, block.header.height));
                    }
                    
                    // Add work score
                    total_work += block.header.work_score as u64;
                    
                    chain_blocks.push(block.clone());
                    current_hash = block.header.prev_hash;
                    current_height -= 1;
                }
                Ok(None) => {
                    return Err(format!("Missing block at height {} (hash: {:?})", 
                        current_height, current_hash));
                }
                Err(e) => {
                    return Err(format!("Error getting block at height {}: {}", current_height, e));
                }
            }
        }

        // Verify we reached genesis
        if current_hash != genesis_hash {
            return Err(format!("Chain doesn't connect to genesis. Expected {:?}, got {:?}", 
                genesis_hash, current_hash));
        }

        // Get genesis block
        match chain.get_block_by_hash(&genesis_hash) {
            Ok(Some(genesis_block)) => {
                if genesis_block.header.height != 0 {
                    return Err("Genesis block has wrong height".to_string());
                }
                total_work += genesis_block.header.work_score as u64;
                chain_blocks.push(genesis_block);
            }
            Ok(None) => {
                return Err("Genesis block not found".to_string());
            }
            Err(e) => {
                return Err(format!("Error getting genesis block: {}", e));
            }
        }

        // Reverse so blocks are in forward order (genesis to end)
        chain_blocks.reverse();

        // Validate chain integrity: each block must connect to previous
        for i in 1..chain_blocks.len() {
            let prev_block = &chain_blocks[i - 1];
            let curr_block = &chain_blocks[i];
            
            if curr_block.header.prev_hash != prev_block.header.hash() {
                return Err(format!("Chain integrity violation at height {}: prev_hash doesn't match previous block hash", 
                    curr_block.header.height));
            }
            
            if curr_block.header.height != prev_block.header.height + 1 {
                return Err(format!("Chain height gap at height {}: expected {}, got {}", 
                    curr_block.header.height, prev_block.header.height + 1, curr_block.header.height));
            }
        }

        // Validate all blocks (except genesis, which is assumed valid)
        for i in 1..chain_blocks.len() {
            let block = &chain_blocks[i];
            let prev_hash = chain_blocks[i - 1].header.hash();
            
            // Validate block (skip timestamp age check during chain validation)
            if let Err(e) = validator.validate_block_with_options(block, &prev_hash, block.header.height, true) {
                return Err(format!("Block {} validation failed: {}", block.header.height, e));
            }
        }

        Ok((chain_blocks, total_work))
    }

    /// Get our current chain from genesis
    /// Returns (chain_blocks, total_work_score)
    async fn get_chain_from_genesis(
        best_hash: coinject_core::Hash,
        best_height: u64,
        chain: &Arc<ChainState>,
    ) -> Result<(Vec<coinject_core::Block>, u64), String> {
        let genesis_hash = chain.genesis_hash();
        let mut chain_blocks = Vec::new();
        let mut current_hash = best_hash;
        let mut current_height = best_height;
        let mut total_work: u64 = 0;

        // Walk back from best to genesis, collecting blocks
        while current_height > 0 {
            match chain.get_block_by_height(current_height) {
                Ok(Some(block)) => {
                    if block.header.hash() != current_hash {
                        return Err(format!("Block hash mismatch at height {}", current_height));
                    }
                    total_work += block.header.work_score as u64;
                    chain_blocks.push(block.clone());
                    current_hash = block.header.prev_hash;
                    current_height -= 1;
                }
                Ok(None) => {
                    return Err(format!("Missing block at height {}", current_height));
                }
                Err(e) => {
                    return Err(format!("Error getting block at height {}: {}", current_height, e));
                }
            }
        }

        // Get genesis block
        match chain.get_block_by_height(0) {
            Ok(Some(genesis_block)) => {
                if genesis_block.header.hash() != genesis_hash {
                    return Err("Genesis block hash mismatch".to_string());
                }
                total_work += genesis_block.header.work_score as u64;
                chain_blocks.push(genesis_block);
            }
            Ok(None) => {
                return Err("Genesis block not found".to_string());
            }
            Err(e) => {
                return Err(format!("Error getting genesis block: {}", e));
            }
        }

        // Reverse so blocks are in forward order (genesis to best)
        chain_blocks.reverse();

        Ok((chain_blocks, total_work))
    }

    /// Perform complete chain reorganization from genesis
    /// Unwinds all blocks to genesis and applies new chain from genesis
    async fn reorganize_chain_from_genesis(
        old_chain_blocks: Vec<coinject_core::Block>,
        new_chain_blocks: Vec<coinject_core::Block>,
        chain: &Arc<ChainState>,
        state: &Arc<AccountState>,
        timelock_state: &Arc<TimeLockState>,
        escrow_state: &Arc<EscrowState>,
        channel_state: &Arc<ChannelState>,
        trustline_state: &Arc<TrustLineState>,
        dimensional_pool_state: &Arc<DimensionalPoolState>,
        marketplace_state: &Arc<MarketplaceState>,
        validator: &Arc<BlockValidator>,
    ) -> Result<(), String> {
        println!("🔄 Starting complete reorganization from genesis: unwinding {} blocks, applying {} blocks",
            old_chain_blocks.len(), new_chain_blocks.len());

        // Verify both chains start from genesis
        if old_chain_blocks.is_empty() || new_chain_blocks.is_empty() {
            return Err("Chain is empty".to_string());
        }
        
        let genesis_hash = chain.genesis_hash();
        if old_chain_blocks[0].header.hash() != genesis_hash || 
           new_chain_blocks[0].header.hash() != genesis_hash {
            return Err("Chains don't start from genesis".to_string());
        }

        // Step 1: Unwind all old chain blocks (except genesis) in reverse order
        // Start from the last block (highest height) and work backwards
                // FIX: Skip genesis (first element) before reversing, not after
        // old_chain_blocks is [genesis, block1, ..., tip]
        // [1..] skips genesis, then .rev() gives us [tip, ..., block1] (no genesis)
        for block in old_chain_blocks[1..].iter().rev() {
            println!("   Unwinding block {}...", block.header.height);
            if let Err(e) = Self::unwind_block_transactions(
                block, state, timelock_state, escrow_state, channel_state,
                trustline_state, dimensional_pool_state, marketplace_state,
            ) {
                return Err(format!("Failed to unwind block {}: {}", block.header.height, e));
            }
        }

        // Step 2: Validate new chain integrity (already validated in validate_chain_from_genesis)
        // But verify genesis matches
        if new_chain_blocks[0].header.hash() != genesis_hash {
            return Err("New chain doesn't start from correct genesis".to_string());
        }

        // Step 3: Apply new chain blocks (skip genesis, it's already applied)
        for block in new_chain_blocks.iter().skip(1) { // Skip genesis
            println!("   Applying new chain block {}...", block.header.height);
            
            // Store block
            chain.store_block(block).await
                .map_err(|e| format!("Failed to store block {}: {}", block.header.height, e))?;

            // Apply transactions
            Self::apply_block_transactions(
                block, state, timelock_state, escrow_state, channel_state,
                trustline_state, dimensional_pool_state, marketplace_state,
            )?;

            // Update consensus state
            use coinject_core::{TAU_C, ConsensusState};
            let tau = (block.header.height as f64) / TAU_C;
            let consensus_state = ConsensusState::at_tau(tau);
            dimensional_pool_state.save_consensus_state(block.header.height, &consensus_state)
                .map_err(|e| format!("Failed to save consensus state: {}", e))?;
        }

        // Step 4: Update best chain
        if let Some(last_block) = new_chain_blocks.last() {
            chain.update_best_chain(last_block.header.hash(), last_block.header.height).await
                .map_err(|e| format!("Failed to update best chain: {}", e))?;
        }

        println!("✅ Complete reorganization from genesis complete!");
        Ok(())
    }

    /// Apply a single transaction to state
    fn apply_single_transaction(
        tx: &coinject_core::Transaction,
        state: &Arc<AccountState>,
        timelock_state: &Arc<TimeLockState>,
        escrow_state: &Arc<EscrowState>,
        channel_state: &Arc<ChannelState>,
        trustline_state: &Arc<TrustLineState>,
        dimensional_pool_state: &Arc<DimensionalPoolState>,
        marketplace_state: &Arc<MarketplaceState>,
        block_height: u64,
    ) -> Result<(), String> {
        use coinject_core::{EscrowType, ChannelType};
        use coinject_state::{Escrow, EscrowStatus, TimeLock, Channel, ChannelStatus};

        // Pattern match on transaction type to maintain economic mathematics
        match tx {
            coinject_core::Transaction::Transfer(transfer_tx) => {
                // Validate sender has sufficient balance
                let sender_balance = state.get_balance(&transfer_tx.from);
                if sender_balance < transfer_tx.amount + transfer_tx.fee {
                    return Err(format!("Insufficient balance: has {}, needs {}",
                        sender_balance, transfer_tx.amount + transfer_tx.fee));
                }

                // Deduct from sender
                state.set_balance(&transfer_tx.from, sender_balance - transfer_tx.amount - transfer_tx.fee)
                    .map_err(|e| format!("Failed to set sender balance: {}", e))?;
                state.set_nonce(&transfer_tx.from, transfer_tx.nonce + 1)
                    .map_err(|e| format!("Failed to set sender nonce: {}", e))?;

                // Credit recipient
                let recipient_balance = state.get_balance(&transfer_tx.to);
                state.set_balance(&transfer_tx.to, recipient_balance + transfer_tx.amount)
                    .map_err(|e| format!("Failed to set recipient balance: {}", e))?;

                Ok(())
            }

            coinject_core::Transaction::TimeLock(timelock_tx) => {
                // Validate sender has sufficient balance
                let sender_balance = state.get_balance(&timelock_tx.from);
                if sender_balance < timelock_tx.amount + timelock_tx.fee {
                    return Err(format!("Insufficient balance for timelock: has {}, needs {}",
                        sender_balance, timelock_tx.amount + timelock_tx.fee));
                }

                // Deduct from sender
                state.set_balance(&timelock_tx.from, sender_balance - timelock_tx.amount - timelock_tx.fee)
                    .map_err(|e| format!("Failed to set sender balance: {}", e))?;
                state.set_nonce(&timelock_tx.from, timelock_tx.nonce + 1)
                    .map_err(|e| format!("Failed to set sender nonce: {}", e))?;

                // Create timelock entry
                let timelock = TimeLock {
                    tx_hash: tx.hash(),
                    from: timelock_tx.from,
                    recipient: timelock_tx.recipient,
                    amount: timelock_tx.amount,
                    unlock_time: timelock_tx.unlock_time,
                    created_at_height: block_height,
                };

                timelock_state.add_timelock(timelock)?;
                Ok(())
            }

            coinject_core::Transaction::Escrow(escrow_tx) => {
                match &escrow_tx.escrow_type {
                    EscrowType::Create { recipient, arbiter, amount, timeout, conditions_hash } => {
                        // Validate sender has sufficient balance
                        let sender_balance = state.get_balance(&escrow_tx.from);
                        if sender_balance < amount + escrow_tx.fee {
                            return Err(format!("Insufficient balance for escrow: has {}, needs {}",
                                sender_balance, amount + escrow_tx.fee));
                        }

                        // Deduct from sender
                        state.set_balance(&escrow_tx.from, sender_balance - amount - escrow_tx.fee)
                            .map_err(|e| format!("Failed to set sender balance: {}", e))?;
                        state.set_nonce(&escrow_tx.from, escrow_tx.nonce + 1)
                            .map_err(|e| format!("Failed to set sender nonce: {}", e))?;

                        // Create escrow entry
                        let escrow = Escrow {
                            escrow_id: escrow_tx.escrow_id,
                            sender: escrow_tx.from,
                            recipient: *recipient,
                            arbiter: *arbiter,
                            amount: *amount,
                            timeout: *timeout,
                            conditions_hash: *conditions_hash,
                            status: EscrowStatus::Active,
                            created_at_height: block_height,
                            resolved_at_height: None,
                        };

                        escrow_state.create_escrow(escrow)?;
                        Ok(())
                    }

                    EscrowType::Release => {
                        let escrow = escrow_state.get_escrow(&escrow_tx.escrow_id)
                            .ok_or("Escrow not found".to_string())?;

                        if !escrow_state.can_release(&escrow_tx.escrow_id, &escrow_tx.from) {
                            return Err("Not authorized to release escrow".to_string());
                        }

                        // Credit recipient
                        let recipient_balance = state.get_balance(&escrow.recipient);
                        state.set_balance(&escrow.recipient, recipient_balance + escrow.amount)
                            .map_err(|e| format!("Failed to set recipient balance: {}", e))?;

                        // Update escrow status
                        escrow_state.update_escrow_status(&escrow_tx.escrow_id, EscrowStatus::Released, Some(block_height))?;
                        Ok(())
                    }

                    EscrowType::Refund => {
                        let escrow = escrow_state.get_escrow(&escrow_tx.escrow_id)
                            .ok_or("Escrow not found".to_string())?;

                        if !escrow_state.can_refund(&escrow_tx.escrow_id, &escrow_tx.from) {
                            return Err("Not authorized to refund escrow".to_string());
                        }

                        // Credit sender (refund)
                        let sender_balance = state.get_balance(&escrow.sender);
                        state.set_balance(&escrow.sender, sender_balance + escrow.amount)
                            .map_err(|e| format!("Failed to set sender balance: {}", e))?;

                        // Update escrow status
                        escrow_state.update_escrow_status(&escrow_tx.escrow_id, EscrowStatus::Refunded, Some(block_height))?;
                        Ok(())
                    }
                }
            }

            coinject_core::Transaction::Channel(channel_tx) => {
                match &channel_tx.channel_type {
                    ChannelType::Open { participant_a, participant_b, deposit_a, deposit_b, timeout } => {
                        // Validate initiator has sufficient balance for their deposit
                        let initiator_balance = state.get_balance(&channel_tx.from);
                        let initiator_deposit = if &channel_tx.from == participant_a { *deposit_a } else { *deposit_b };

                        if initiator_balance < initiator_deposit + channel_tx.fee {
                            return Err(format!("Insufficient balance for channel: has {}, needs {}",
                                initiator_balance, initiator_deposit + channel_tx.fee));
                        }

                        // Deduct initiator's deposit
                        state.set_balance(&channel_tx.from, initiator_balance - initiator_deposit - channel_tx.fee)
                            .map_err(|e| format!("Failed to set initiator balance: {}", e))?;
                        state.set_nonce(&channel_tx.from, channel_tx.nonce + 1)
                            .map_err(|e| format!("Failed to set initiator nonce: {}", e))?;

                        // Create channel entry
                        let channel = Channel {
                            channel_id: channel_tx.channel_id,
                            participant_a: *participant_a,
                            participant_b: *participant_b,
                            deposit_a: *deposit_a,
                            deposit_b: *deposit_b,
                            balance_a: *deposit_a,
                            balance_b: *deposit_b,
                            sequence: 0,
                            dispute_timeout: *timeout,
                            status: ChannelStatus::Open,
                            opened_at_height: block_height,
                            closed_at_height: None,
                            dispute_started_at: None,
                        };

                        channel_state.open_channel(channel)?;
                        Ok(())
                    }

                    ChannelType::Update { sequence, balance_a, balance_b } => {
                        channel_state.update_channel_state(&channel_tx.channel_id, *sequence, *balance_a, *balance_b)?;
                        Ok(())
                    }

                    ChannelType::CooperativeClose { final_balance_a, final_balance_b } => {
                        let channel = channel_state.get_channel(&channel_tx.channel_id)
                            .ok_or("Channel not found".to_string())?;

                        // Credit both participants
                        let balance_a = state.get_balance(&channel.participant_a);
                        state.set_balance(&channel.participant_a, balance_a + final_balance_a)
                            .map_err(|e| format!("Failed to set participant A balance: {}", e))?;

                        let balance_b = state.get_balance(&channel.participant_b);
                        state.set_balance(&channel.participant_b, balance_b + final_balance_b)
                            .map_err(|e| format!("Failed to set participant B balance: {}", e))?;

                        // Close channel
                        channel_state.close_cooperative(&channel_tx.channel_id, *final_balance_a, *final_balance_b, block_height)?;
                        Ok(())
                    }

                    ChannelType::UnilateralClose { sequence, balance_a, balance_b, .. } => {
                        let _channel = channel_state.get_channel(&channel_tx.channel_id)
                            .ok_or("Channel not found".to_string())?;

                        // Start dispute
                        channel_state.start_dispute(&channel_tx.channel_id, *sequence, *balance_a, *balance_b)?;
                        Ok(())
                    }
                }
            }

            coinject_core::Transaction::TrustLine(trustline_tx) => {
                use coinject_core::TrustLineType;
                use coinject_state::{TrustLine, TrustLineStatus};

                // TrustLine transactions: dimensional economics with exponential decay
                // Validate sender has sufficient balance for fee
                let sender_balance = state.get_balance(&trustline_tx.from);
                if sender_balance < trustline_tx.fee {
                    return Err(format!("Insufficient balance for trustline fee: has {}, needs {}",
                        sender_balance, trustline_tx.fee));
                }

                // Deduct fee from sender and increment nonce
                state.set_balance(&trustline_tx.from, sender_balance - trustline_tx.fee)
                    .map_err(|e| format!("Failed to set sender balance: {}", e))?;
                state.set_nonce(&trustline_tx.from, trustline_tx.nonce + 1)
                    .map_err(|e| format!("Failed to set sender nonce: {}", e))?;

                // Apply trustline state operations based on transaction type
                match &trustline_tx.trustline_type {
                    TrustLineType::Create {
                        account_b,
                        limit_a_to_b,
                        limit_b_to_a,
                        quality_in,
                        quality_out,
                        ripple_enabled,
                        dimensional_scale,
                    } => {
                        // Create new bilateral trustline with dimensional economics
                        let mut trustline = TrustLine {
                            trustline_id: trustline_tx.trustline_id,
                            account_a: trustline_tx.from,
                            account_b: *account_b,
                            limit_a_to_b: *limit_a_to_b,
                            limit_b_to_a: *limit_b_to_a,
                            balance: 0,
                            quality_in: *quality_in,
                            quality_out: *quality_out,
                            ripple_enabled: *ripple_enabled,
                            dimensional_scale: *dimensional_scale,
                            tau: 0.0,
                            viviani_delta: 0.0,
                            status: TrustLineStatus::Active,
                            created_at_height: block_height,
                            modified_at_height: block_height,
                        };

                        // Initialize Viviani oracle metrics
                        trustline.update_viviani_oracle();

                        trustline_state.create_trustline(trustline)
                            .map_err(|e| format!("Failed to create trustline: {}", e))?;

                        Ok(())
                    }

                    TrustLineType::UpdateLimits { limit_a_to_b, limit_b_to_a } => {
                        // Update credit limits on existing trustline
                        let trustline = trustline_state.get_trustline(&trustline_tx.trustline_id)
                            .ok_or_else(|| "TrustLine not found".to_string())?;

                        // Verify sender is authorized (must be account_a or account_b)
                        if !trustline.is_participant(&trustline_tx.from) {
                            return Err("Not authorized to update trustline".to_string());
                        }

                        // Update limits via state manager (handles dimensional recalibration)
                        trustline_state.update_limits(
                            &trustline_tx.trustline_id,
                            *limit_a_to_b,
                            *limit_b_to_a,
                            block_height,
                        )?;

                        Ok(())
                    }

                    TrustLineType::Freeze => {
                        // Freeze trustline (prevents further transfers)
                        let trustline = trustline_state.get_trustline(&trustline_tx.trustline_id)
                            .ok_or_else(|| "TrustLine not found".to_string())?;

                        // Verify sender is authorized
                        if !trustline.is_participant(&trustline_tx.from) {
                            return Err("Not authorized to freeze trustline".to_string());
                        }

                        trustline_state.freeze_trustline(&trustline_tx.trustline_id, block_height)?;
                        Ok(())
                    }

                    TrustLineType::Close => {
                        // Close trustline (requires zero balance)
                        let trustline = trustline_state.get_trustline(&trustline_tx.trustline_id)
                            .ok_or_else(|| "TrustLine not found".to_string())?;

                        // Verify sender is authorized
                        if !trustline.is_participant(&trustline_tx.from) {
                            return Err("Not authorized to close trustline".to_string());
                        }

                        // close_trustline already validates zero balance internally
                        trustline_state.close_trustline(&trustline_tx.trustline_id, block_height)?;
                        Ok(())
                    }

                    TrustLineType::EvolvePhase { delta_tau } => {
                        // Evolve phase parameter: θ(τ) = λτ = τ/√2
                        let trustline = trustline_state.get_trustline(&trustline_tx.trustline_id)
                            .ok_or_else(|| "TrustLine not found".to_string())?;

                        // Verify sender is authorized
                        if !trustline.is_participant(&trustline_tx.from) {
                            return Err("Not authorized to evolve trustline phase".to_string());
                        }

                        // Evolve phase via state manager (handles oracle update)
                        trustline_state.evolve_trustline_phase(
                            &trustline_tx.trustline_id,
                            *delta_tau,
                            block_height,
                        )?;

                        Ok(())
                    }
                }
            }

            coinject_core::Transaction::DimensionalPoolSwap(pool_swap_tx) => {
                // Dimensional pool swap: exponential tokenomics with adaptive ratios
                // Validate sender has sufficient balance for fee
                let sender_balance = state.get_balance(&pool_swap_tx.from);
                if sender_balance < pool_swap_tx.fee {
                    return Err(format!("Insufficient balance for pool swap fee: has {}, needs {}",
                        sender_balance, pool_swap_tx.fee));
                }

                // Deduct fee from sender and increment nonce
                state.set_balance(&pool_swap_tx.from, sender_balance - pool_swap_tx.fee)
                    .map_err(|e| format!("Failed to set sender balance: {}", e))?;
                state.set_nonce(&pool_swap_tx.from, pool_swap_tx.nonce + 1)
                    .map_err(|e| format!("Failed to set sender nonce: {}", e))?;

                // Execute dimensional pool swap with exponential scaling
                let amount_out = dimensional_pool_state.execute_swap(
                    pool_swap_tx.pool_from,
                    pool_swap_tx.pool_to,
                    pool_swap_tx.amount_in,
                    pool_swap_tx.min_amount_out,
                    block_height,
                )?;

                // Record the swap transaction
                dimensional_pool_state.record_swap(
                    tx.hash(),
                    pool_swap_tx.from,
                    pool_swap_tx.pool_from,
                    pool_swap_tx.pool_to,
                    pool_swap_tx.amount_in,
                    amount_out,
                    block_height,
                )?;

                Ok(())
            }

            coinject_core::Transaction::Marketplace(marketplace_tx) => {
                // PoUW Marketplace transaction processing
                use coinject_core::MarketplaceOperation;

                // Validate sender has sufficient balance for fee
                let sender_balance = state.get_balance(&marketplace_tx.from);

                match &marketplace_tx.operation {
                    MarketplaceOperation::SubmitProblem { problem, bounty, min_work_score, expiration_days } => {
                        // Need fee + bounty for escrow
                        let total_needed = marketplace_tx.fee + bounty;
                        if sender_balance < total_needed {
                            return Err(format!("Insufficient balance for problem submission: has {}, needs {}",
                                sender_balance, total_needed));
                        }

                        // Deduct fee + bounty (bounty goes to escrow)
                        state.set_balance(&marketplace_tx.from, sender_balance - total_needed)
                            .map_err(|e| format!("Failed to set sender balance: {}", e))?;

                        // Submit problem to marketplace state
                        let problem_id = marketplace_state.submit_problem(
                            coinject_core::SubmissionMode::Public { problem: problem.clone() },
                            marketplace_tx.from,
                            *bounty,
                            *min_work_score,
                            *expiration_days,
                        ).map_err(|e| format!("Failed to submit problem: {}", e))?;

                        println!("✅ Problem submitted to marketplace: {:?} (bounty: {})", problem_id, bounty);
                    }
                    MarketplaceOperation::SubmitSolution { problem_id, solution } => {
                        // AUTONOMOUS BOUNTY PAYOUT
                        // When a valid solution is submitted, automatically claim and payout the bounty
                        // This makes the marketplace truly self-executing - no manual claim needed!

                        // Just need fee
                        if sender_balance < marketplace_tx.fee {
                            return Err(format!("Insufficient balance for marketplace fee: has {}, needs {}",
                                sender_balance, marketplace_tx.fee));
                        }

                        // Deduct fee
                        state.set_balance(&marketplace_tx.from, sender_balance - marketplace_tx.fee)
                            .map_err(|e| format!("Failed to set sender balance: {}", e))?;

                        // Submit solution to marketplace state (verifies and marks as solved)
                        marketplace_state.submit_solution(*problem_id, marketplace_tx.from, solution.clone())
                            .map_err(|e| format!("Failed to submit solution: {}", e))?;

                        // AUTONOMOUS PAYOUT: Immediately claim and release bounty to solver
                        let (solver, bounty) = marketplace_state.claim_bounty(*problem_id)
                            .map_err(|e| format!("Failed to auto-claim bounty: {}", e))?;

                        // Credit bounty to solver atomically in the same block
                        let solver_balance = state.get_balance(&solver);
                        state.set_balance(&solver, solver_balance + bounty)
                            .map_err(|e| format!("Failed to credit bounty to solver: {}", e))?;

                        println!("✅ Solution accepted! Auto-paid {} tokens to solver {:?}", bounty, solver);
                    }
                    MarketplaceOperation::ClaimBounty { problem_id } => {
                        // Just need fee
                        if sender_balance < marketplace_tx.fee {
                            return Err(format!("Insufficient balance for marketplace fee: has {}, needs {}",
                                sender_balance, marketplace_tx.fee));
                        }

                        // Deduct fee
                        state.set_balance(&marketplace_tx.from, sender_balance - marketplace_tx.fee)
                            .map_err(|e| format!("Failed to set sender balance: {}", e))?;

                        // Claim bounty from marketplace state
                        let (solver, bounty) = marketplace_state.claim_bounty(*problem_id)
                            .map_err(|e| format!("Failed to claim bounty: {}", e))?;

                        // Credit bounty to solver
                        let solver_balance = state.get_balance(&solver);
                        state.set_balance(&solver, solver_balance + bounty)
                            .map_err(|e| format!("Failed to credit bounty to solver: {}", e))?;

                        println!("✅ Bounty claimed: {} tokens paid to solver {:?}", bounty, solver);
                    }
                    MarketplaceOperation::CancelProblem { problem_id } => {
                        // Just need fee
                        if sender_balance < marketplace_tx.fee {
                            return Err(format!("Insufficient balance for marketplace fee: has {}, needs {}",
                                sender_balance, marketplace_tx.fee));
                        }

                        // Deduct fee
                        state.set_balance(&marketplace_tx.from, sender_balance - marketplace_tx.fee)
                            .map_err(|e| format!("Failed to set sender balance: {}", e))?;

                        // Cancel problem and refund bounty
                        let bounty = marketplace_state.cancel_problem(*problem_id, marketplace_tx.from)
                            .map_err(|e| format!("Failed to cancel problem: {}", e))?;

                        // Refund bounty to submitter
                        let submitter_balance = state.get_balance(&marketplace_tx.from);
                        state.set_balance(&marketplace_tx.from, submitter_balance + bounty)
                            .map_err(|e| format!("Failed to refund bounty to submitter: {}", e))?;

                        println!("✅ Problem cancelled: {} tokens refunded to submitter", bounty);
                    }
                }

                // Increment nonce
                state.set_nonce(&marketplace_tx.from, marketplace_tx.nonce + 1)
                    .map_err(|e| format!("Failed to set sender nonce: {}", e))?;

                Ok(())
            }
        }
    }

    /// Mining loop
    async fn mining_loop(
        miner: Arc<RwLock<Miner>>,
        chain: Arc<ChainState>,
        state: Arc<AccountState>,
        timelock_state: Arc<TimeLockState>,
        escrow_state: Arc<EscrowState>,
        channel_state: Arc<ChannelState>,
        trustline_state: Arc<TrustLineState>,
        dimensional_pool_state: Arc<DimensionalPoolState>,
        marketplace_state: Arc<MarketplaceState>,
        tx_pool: Arc<RwLock<TransactionPool>>,
        network_tx: mpsc::UnboundedSender<NetworkCommand>,
        cpp_network_tx: mpsc::UnboundedSender<coinject_network::cpp::NetworkCommand>,
        hf_sync: Option<Arc<HuggingFaceSync>>,
        peer_count: Arc<RwLock<usize>>,
        best_known_peer_height: Arc<RwLock<u64>>,
        peer_consensus: Arc<PeerConsensus>,
        dev_mode: bool,
    ) {
        // In dev mode, skip waiting for peers and start mining immediately
        if dev_mode {
            println!("🔧 Dev mode: Starting mining immediately (no peer sync required)");
        } else {
            // Wait for peer connections and initial chain sync before mining
            println!("⏳ Waiting for peer connections and chain sync before mining...");
        use coinject_core::ETA;
        let mut sync_wait_interval = time::interval(Duration::from_secs(2));
        let mut sync_attempts = 0;
        // MAX_SYNC_WAIT_ATTEMPTS: Network-derived timeout would be ETA * network_median_sync_time
        // For now, using ETA-scaled value: 150 attempts * 2s = 300s, scaled by ETA ≈ 212s effective
        const MAX_SYNC_WAIT_ATTEMPTS: u32 = (150.0 * ETA) as u32; // ETA-scaled sync timeout
        let mut last_height = 0u64;
        let mut stable_height_count = 0u32;
        // STABLE_HEIGHT_THRESHOLD: Dimensionless count, but could be ETA-scaled
        // 3 checks ensures stability without excessive delay
        const STABLE_HEIGHT_THRESHOLD: u32 = 3; // Height must be stable for 3 checks (6 seconds)
        
        loop {
            sync_wait_interval.tick().await;
            sync_attempts += 1;

            let current_peers = *peer_count.read().await;
            let best_height = chain.best_block_height().await;

            // Check if we have peers
            if current_peers > 0 {
                // Check if height is stable (not actively syncing)
                if best_height == last_height {
                    stable_height_count += 1;
                } else {
                    stable_height_count = 0;
                    last_height = best_height;
                }

                // If we're at genesis with peers, start mining after short wait (20 seconds = 10 attempts)
                if best_height == 0 {
                    if sync_attempts >= 10 {
                        // At genesis with peers - time to bootstrap the network!
                        println!("🚀 At genesis with {} peer(s), starting mining to bootstrap network!", current_peers);
                        break;
                    } else if sync_attempts >= 5 {
                        println!("✅ Connected to {} peer(s) at genesis, preparing to mine... ({}/10)", current_peers, sync_attempts);
                    }
                } else if stable_height_count >= STABLE_HEIGHT_THRESHOLD {
                    // Height is stable - we're either synced or caught up
                    println!("✅ Connected to {} peer(s) at height {} (stable), starting mining", current_peers, best_height);
                    break;
                } else {
                    // Height is changing - actively syncing
                    if sync_attempts % 10 == 0 {
                        println!("   Syncing... current height: {} (attempt {}/{})", 
                            best_height, sync_attempts, MAX_SYNC_WAIT_ATTEMPTS);
                    }
                }
            } else {
                // No peers yet
                if sync_attempts % 5 == 0 {
                    println!("   Still waiting for peers... (attempt {}/{}, current peers: {})", 
                        sync_attempts, MAX_SYNC_WAIT_ATTEMPTS, current_peers);
                }
            }

            if sync_attempts >= MAX_SYNC_WAIT_ATTEMPTS {
                println!("⚠️  Sync wait timeout after {}s (height: {}), starting mining anyway", 
                    sync_attempts * 2, best_height);
                break;
            }
        }
        } // end of else block (non-dev mode peer sync)

        // Start mining loop
        println!("⛏️  Starting mining loop...");
        let mut last_mined_height = chain.best_block_height().await;
        println!("⛏️  Mining loop initialized, last height: {}", last_mined_height);

        loop {
            // Use blocking sleep to bypass Tokio timer issues
            eprintln!("⏰ Mining loop - sleeping 5s (blocking)...");
            use std::io::Write;
            let _ = std::io::stderr().flush();
            
            // Use spawn_blocking with std::thread::sleep
            tokio::task::spawn_blocking(|| {
                std::thread::sleep(Duration::from_secs(5));
            }).await.unwrap();
            
            eprintln!("⏰ Mining loop - WOKE UP after blocking sleep!");
            let _ = std::io::stderr().flush();
            
            eprintln!("⏰ Mining loop - getting chain state...");

            let best_height = chain.best_block_height().await;
            println!("⏰ Got best_height: {}", best_height);
            let best_hash = chain.best_block_hash().await;
            println!("⏰ Got best_hash: {:?}", best_hash);

            // Check if chain advanced since last mining attempt (block received from peer)
            // v4.7.44 FIX: Don't skip mining entirely - just update last_mined_height and continue
            // to the consensus check. This fixes the race condition where only one node could mine.
            if best_height > last_mined_height {
                println!("📥 Chain advanced from {} to {} (block received from peer), updating height", 
                    last_mined_height, best_height);
                last_mined_height = best_height;
                // Note: We continue to consensus check below - this allows ALL nodes to potentially mine
                // The consensus check will properly coordinate who should mine
            }

            // SYNC-BEFORE-MINE: Multi-peer consensus check (XRPL-inspired)
            // Requires 5+ peers with 80% agreement before mining
            // SKIP in dev mode - allow solo mining
            if !dev_mode {
                let (should_mine, reason) = peer_consensus.should_mine(best_height).await;
                if !should_mine {
                    println!("⏸️  Mining PAUSED: {}", reason);
                    
                    // Fallback: Also check simple best-peer height (for bootstrap with <5 peers)
                    let peer_best = *best_known_peer_height.read().await;
                    const SYNC_THRESHOLD: u64 = 10;
                    if peer_best > 0 && best_height + SYNC_THRESHOLD < peer_best {
                        let blocks_behind = peer_best - best_height;
                        println!("   Also {} blocks behind best peer (our: {}, best: {})", 
                            blocks_behind, best_height, peer_best);
                    }
                    continue; // Skip mining, check again next interval
                }
                
                // Log consensus diagnostics
                let diagnostics = peer_consensus.diagnostics().await;
                println!("✅ Consensus OK: {}", diagnostics);
            } else {
                println!("🔧 Dev mode: Skipping peer consensus check");
            }

            // Ready to mine!
            println!("⛏️  Mining block {}...", best_height + 1);

            // Select transactions from pool (top 100 by fee)
            let pool = tx_pool.read().await;
            let pool_size = pool.len();
            let transactions = pool.get_top_n(100);
            drop(pool);

            println!("   Pool size: {}, Fetching top 100, Got: {} transactions", pool_size, transactions.len());

            // Mine block
            let mut miner_lock = miner.write().await;
            if let Some(block) = miner_lock
                .mine_block(best_hash, best_height + 1, transactions.clone())
                .await
            {
                println!("🎉 Mined new block {}!", block.header.height);
                drop(miner_lock);

                // Update last mined height to prevent immediate re-mining
                last_mined_height = block.header.height;

                // Store block
                if let Err(e) = chain.store_block(&block).await {
                    println!("❌ Failed to store mined block: {}", e);
                    continue;
                }

                // RUNTIME INTEGRATION: Calculate and save dimensional consensus state
                // τ = block_height / τ_c (dimensionless time progression)
                use coinject_core::{TAU_C, ConsensusState};
                let tau = (block.header.height as f64) / TAU_C;
                let consensus_state = ConsensusState::at_tau(tau);

                if let Err(e) = dimensional_pool_state.save_consensus_state(block.header.height, &consensus_state) {
                    println!("⚠️  Warning: Failed to save consensus state at height {}: {}", block.header.height, e);
                } else {
                    println!("📊 Consensus state: τ={:.4}, |ψ|={:.4}, θ={:.4} rad",
                        consensus_state.tau,
                        consensus_state.magnitude,
                        consensus_state.phase
                    );
                }

                // EMPIRICAL MEASUREMENT: Record work score for convergence analysis
                let block_time = if block.header.height > 1 {
                    // Approximate block time from timestamp difference
                    // In full implementation, track previous block timestamp
                    60.0 // Default to ~60s target block time
                } else {
                    0.0
                };

                if let Err(e) = dimensional_pool_state.record_work_score(
                    block.header.height,
                    consensus_state.tau,
                    block.header.work_score,
                    block_time
                ) {
                    println!("⚠️  Warning: Failed to record work score: {}", e);
                }

                // EMPIRICAL MEASUREMENT: Update consensus metrics every 50 blocks (after block 50)
                // This provides more frequent updates to see convergence trajectory
                if block.header.height % 50 == 0 && block.header.height >= 50 {
                    // Use adaptive window: smaller early on, larger later
                    let window_size = if block.header.height < 200 {
                        (block.header.height as usize).min(100)
                    } else {
                        300
                    };

                    match dimensional_pool_state.update_consensus_metrics(block.header.height, window_size) {
                        Ok(metrics) => {
                            println!("🔬 EMPIRICAL CONSENSUS METRICS (block {}):", block.header.height);
                            println!("   Measured η = {:.6} (theoretical = 0.707107)", metrics.measured_eta);
                            println!("   Measured λ = {:.6} (theoretical = 0.707107)", metrics.measured_lambda);
                            println!("   Oracle Δ = {:.6} (theoretical = 0.231)", metrics.measured_oracle_delta);
                            println!("   Convergence confidence (R²) = {:.4}", metrics.convergence_confidence);
                            println!("   Sample size: {} blocks", metrics.sample_size);

                            if let Some(status) = dimensional_pool_state.test_conjecture() {
                                println!("🧪 THE CONJECTURE STATUS:");
                                println!("   η convergence: {} (error: {:.4})",
                                    if status.eta_convergence { "✅" } else { "⏳" },
                                    (metrics.measured_eta - 0.707107).abs()
                                );
                                println!("   λ convergence: {} (error: {:.4})",
                                    if status.lambda_convergence { "✅" } else { "⏳" },
                                    (metrics.measured_lambda - 0.707107).abs()
                                );
                                println!("   Oracle alignment: {} (Δ error: {:.4})",
                                    if status.oracle_alignment { "✅" } else { "⏳" },
                                    (metrics.measured_oracle_delta - 0.231).abs()
                                );
                            }
                        },
                        Err(e) => {
                            println!("⚠️  Warning: Failed to update consensus metrics: {}", e);
                        }
                    }
                }

                // RUNTIME INTEGRATION: Distribute block reward dynamically across dimensional pools
                let block_reward = block.coinbase.reward;
                if let Err(e) = dimensional_pool_state.distribute_block_reward(block_reward, block.header.height) {
                    println!("⚠️  Warning: Failed to distribute block reward: {}", e);
                }

                // RUNTIME INTEGRATION: Execute unlock schedules (every 10 blocks to reduce spam)
                if block.header.height % 10 == 0 {
                    if let Err(e) = dimensional_pool_state.execute_unlock_schedules(block.header.height) {
                        println!("⚠️  Warning: Failed to execute unlock schedules: {}", e);
                    }
                }

                // RUNTIME INTEGRATION: Distribute yields (every 10 blocks)
                if block.header.height % 10 == 0 {
                    if let Err(e) = dimensional_pool_state.distribute_yields(block.header.height) {
                        println!("⚠️  Warning: Failed to distribute yields: {}", e);
                    }
                }

                // Apply block transactions to state
                let applied_txs = match Self::apply_block_transactions(&block, &state, &timelock_state, &escrow_state, &channel_state, &trustline_state, &dimensional_pool_state, &marketplace_state) {
                    Ok(txs) => txs,
                    Err(e) => {
                        println!("❌ Failed to apply mined block transactions: {}", e);
                        continue;
                    }
                };

                // Remove only successfully applied transactions from pool
                let mut pool = tx_pool.write().await;
                for tx_hash in &applied_txs {
                    pool.remove(tx_hash);
                }
                drop(pool);

                // Broadcast to network
                if let Err(e) = network_tx.send(NetworkCommand::BroadcastBlock(block.clone())) {
                    println!("❌ Failed to send broadcast command: {}", e);
                } else {
                    println!("[GOSSIP] sent block height={} hash={:?} ts={}", block.header.height, block.header.hash(), block.header.timestamp);
                }

                // Update CPP network chain state so it broadcasts correct height to peers
                if let Err(e) = cpp_network_tx.send(coinject_network::cpp::NetworkCommand::UpdateChainState {
                    best_height: block.header.height,
                    best_hash: block.header.hash(),
                }) {
                    eprintln!("⚠️ Failed to update CPP chain state: {}", e);
                }

                // Push consensus block to Hugging Face (inline within mining loop)
                if let Some(ref hf_sync) = hf_sync {
                    eprintln!("📦 Hugging Face: Uploading mined block {}", block.header.height);
                    match hf_sync.push_consensus_block(&block, true).await {
                        Ok(()) => eprintln!("✅ Hugging Face: Block {} queued for upload", block.header.height),
                        Err(e) => eprintln!("❌ HuggingFace upload error for block {}: {}", block.header.height, e),
                    }

                    // Upload marketplace transactions from this mined block
                    Self::upload_marketplace_transactions(&block, &marketplace_state, hf_sync);
                }
            } else {
                println!("❌ Mining failed");
            }
        }
    }

    /// Upload marketplace transactions from a block to Hugging Face
    fn upload_marketplace_transactions(
        block: &coinject_core::Block,
        marketplace_state: &Arc<MarketplaceState>,
        hf_sync: &Arc<HuggingFaceSync>,
    ) {
        use coinject_core::{Transaction, MarketplaceOperation};

        // Scan block for marketplace transactions
        for tx in &block.transactions {
            if let Transaction::Marketplace(marketplace_tx) = tx {
                match &marketplace_tx.operation {
                    MarketplaceOperation::SubmitProblem { problem, .. } => {
                        // Calculate problem_id from problem data (same as marketplace state does)
                        let problem_id = match bincode::serialize(problem) {
                            Ok(problem_data) => coinject_core::Hash::new(&problem_data),
                            Err(e) => {
                                eprintln!("❌ Failed to serialize problem for hash: {}", e);
                                return;
                            }
                        };

                        // Retrieve the submission from marketplace state
                        let marketplace_clone = Arc::clone(marketplace_state);
                        let hf_clone = Arc::clone(hf_sync);
                        let block_height = block.header.height;

                        tokio::spawn(async move {
                            match marketplace_clone.get_problem(&problem_id) {
                                Ok(Some(submission)) => {
                                    eprintln!("📊 Uploading problem submission {:?} to Hugging Face", problem_id);
                                    if let Err(e) = hf_clone.push_problem_submission(&submission, block_height).await {
                                        eprintln!("❌ Failed to upload problem submission: {}", e);
                                    } else {
                                        eprintln!("✅ Successfully uploaded problem submission {:?}", problem_id);
                                    }
                                }
                                Ok(None) => {
                                    eprintln!("⚠️  Problem {:?} not found in marketplace state", problem_id);
                                }
                                Err(e) => {
                                    eprintln!("❌ Failed to retrieve problem {:?}: {}", problem_id, e);
                                }
                            }
                        });
                    }
                    MarketplaceOperation::SubmitSolution { problem_id, .. } => {
                        // Retrieve the updated submission (now has solution) from marketplace state
                        let marketplace_clone = Arc::clone(marketplace_state);
                        let hf_clone = Arc::clone(hf_sync);
                        let problem_id = *problem_id;
                        let block_height = block.header.height;

                        tokio::spawn(async move {
                            match marketplace_clone.get_problem(&problem_id) {
                                Ok(Some(submission)) => {
                                    eprintln!("📊 Uploading solution submission for problem {:?} to Hugging Face", problem_id);

                                    // For now, use estimated timing (we'll refine this later with actual measurements)
                                    // Estimate based on problem complexity
                                    let solve_time = std::time::Duration::from_secs((submission.min_work_score * 10.0) as u64);
                                    let verify_time = std::time::Duration::from_millis(100);
                                    let solve_memory = 1024 * 1024; // 1 MB estimate
                                    let verify_memory = 512 * 1024; // 512 KB estimate

                                    if let Err(e) = hf_clone.push_solution_submission(
                                        &submission,
                                        block_height,
                                        solve_time,
                                        verify_time,
                                        solve_memory,
                                        verify_memory,
                                    ).await {
                                        eprintln!("❌ Failed to upload solution submission: {}", e);
                                    } else {
                                        eprintln!("✅ Successfully uploaded solution submission for problem {:?}", problem_id);
                                    }
                                }
                                Ok(None) => {
                                    eprintln!("⚠️  Problem {:?} not found in marketplace state", problem_id);
                                }
                                Err(e) => {
                                    eprintln!("❌ Failed to retrieve problem {:?}: {}", problem_id, e);
                                }
                            }
                        });
                    }
                    _ => {
                        // ClaimBounty and CancelProblem don't need uploads
                    }
                }
            }
        }
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

// =============================================================================
// MERKLE PROOF UTILITIES
// =============================================================================

/// Build a Merkle proof for a transaction in a block
/// Returns the authentication path with direction flags
fn build_merkle_proof(
    transactions: &[coinject_core::Transaction],
    target_tx_hash: &coinject_core::Hash,
) -> Vec<(coinject_core::Hash, bool)> {
    use sha2::{Sha256, Digest};
    
    if transactions.is_empty() {
        return Vec::new();
    }
    
    // Get transaction hashes
    let mut leaves: Vec<coinject_core::Hash> = transactions
        .iter()
        .map(|tx| tx.hash())
        .collect();
    
    // Find target index
    let target_index = match leaves.iter().position(|h| h == target_tx_hash) {
        Some(idx) => idx,
        None => return Vec::new(), // Transaction not found
    };
    
    // Build proof bottom-up
    let mut proof = Vec::new();
    let mut current_index = target_index;
    
    while leaves.len() > 1 {
        // Pad to even length
        if leaves.len() % 2 == 1 {
            leaves.push(*leaves.last().unwrap());
        }
        
        // Collect sibling
        let sibling_index = if current_index % 2 == 0 {
            current_index + 1
        } else {
            current_index - 1
        };
        
        let is_right = current_index % 2 == 0;
        proof.push((leaves[sibling_index], is_right));
        
        // Build next level
        let mut next_level = Vec::new();
        for i in (0..leaves.len()).step_by(2) {
            let left = &leaves[i];
            let right = &leaves[i + 1];
            
            let mut hasher = Sha256::new();
            hasher.update(b"MERKLE_NODE");
            hasher.update(left.as_bytes());
            hasher.update(right.as_bytes());
            next_level.push(coinject_core::Hash::from_bytes(hasher.finalize().into()));
        }
        
        leaves = next_level;
        current_index /= 2;
    }
    
    proof
}

/// Verify a Merkle proof against a root
fn verify_merkle_proof(
    tx_hash: &coinject_core::Hash,
    proof: &[(coinject_core::Hash, bool)],
    expected_root: &coinject_core::Hash,
) -> bool {
    use sha2::{Sha256, Digest};
    
    let mut current = *tx_hash;
    
    for (sibling, is_right) in proof {
        let mut hasher = Sha256::new();
        hasher.update(b"MERKLE_NODE");
        
        if *is_right {
            // Current is on the left, sibling is on the right
            hasher.update(current.as_bytes());
            hasher.update(sibling.as_bytes());
        } else {
            // Sibling is on the left, current is on the right
            hasher.update(sibling.as_bytes());
            hasher.update(current.as_bytes());
        }
        
        current = coinject_core::Hash::from_bytes(hasher.finalize().into());
    }
    
    &current == expected_root
}