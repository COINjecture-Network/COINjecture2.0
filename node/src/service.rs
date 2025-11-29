// Node Service
// Main orchestrator tying all components together

use crate::chain::ChainState;
use crate::config::NodeConfig;
use crate::faucet::{Faucet, FaucetConfig};
use crate::genesis::{create_genesis_block, GenesisConfig};
use crate::validator::BlockValidator;
use coinject_consensus::{Miner, MiningConfig};
use coinject_core::Address;
use coinject_mempool::{ProblemMarketplace, TransactionPool};
use coinject_network::{NetworkConfig, NetworkEvent, NetworkService};
use coinject_rpc::{RpcServer, RpcServerState};
use coinject_state::{AccountState, TimeLockState, EscrowState, ChannelState, TrustLineState, DimensionalPoolState, MarketplaceState};
use coinject_huggingface::{HuggingFaceSync, HuggingFaceConfig, EnergyConfig, EnergyMeasurementMethod, SyncConfig};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tokio::time;

/// Commands that can be sent to the network task
enum NetworkCommand {
    BroadcastBlock(coinject_core::Block),
    BroadcastTransaction(coinject_core::Transaction),
    BroadcastStatus { best_height: u64, best_hash: coinject_core::Hash, genesis_hash: coinject_core::Hash },
    RequestBlocks { from_height: u64, to_height: u64 },
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
    hf_sync: Option<Arc<HuggingFaceSync>>,
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

        // Initialize account state and advanced transaction states (sharing same DB)
        println!("💰 Initializing account state...");
        // Ensure parent directory exists for state database file
        let state_db_path = config.state_db_path();
        if let Some(parent) = state_db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let state_db = Arc::new(redb::Database::create(state_db_path)?);
        let state = Arc::new(AccountState::from_db(Arc::clone(&state_db)));
        let timelock_state = Arc::new(TimeLockState::new(Arc::clone(&state_db))?);
        let escrow_state = Arc::new(EscrowState::new(Arc::clone(&state_db))?);
        let channel_state = Arc::new(ChannelState::new(Arc::clone(&state_db))?);
        let trustline_state = Arc::new(TrustLineState::new(Arc::clone(&state_db))?);
        let dimensional_pool_state = Arc::new(DimensionalPoolState::new(Arc::clone(&state_db))?);
        let marketplace_state = Arc::new(MarketplaceState::from_db(Arc::clone(&state_db))?);

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
            };

            println!("   Miner address: {}", hex::encode(miner_address.as_bytes()));
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

        // Create shutdown channel
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

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
            hf_sync,
            shutdown_tx,
            shutdown_rx,
        })
    }

    /// Start the node services
    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Start P2P network
        println!("🌐 Starting P2P network...");

        // Create shared peer count for RPC and network service
        let peer_count = Arc::new(RwLock::new(0));

        // Keypair path for persistent PeerId
        let keypair_path = self.config.data_dir.join("network_key");
        
        let network_config = NetworkConfig {
            listen_addr: self.config.p2p_addr.clone(),
            chain_id: self.config.chain_id.clone(),
            max_peers: self.config.max_peers,
            enable_mdns: true,
            keypair_path: Some(keypair_path),
        };

        let (mut network_service, mut event_rx) = NetworkService::new(network_config, Arc::clone(&peer_count))?;
        
        // Get local PeerId for RPC and logging
        let local_peer_id = network_service.local_peer_id();
        let local_peer_id_str = local_peer_id.to_string();
        
        network_service.start_listening(&self.config.p2p_addr)?;
        network_service.subscribe_topics()?;

        // Connect to bootnodes if provided
        if !self.config.bootnodes.is_empty() {
            println!("   Connecting to {} bootnode(s)...", self.config.bootnodes.len());
            network_service.connect_to_bootnodes(&self.config.bootnodes)?;
        }

        println!("   Listening on: {}", self.config.p2p_addr);
        println!("   PeerId: {}", local_peer_id_str);
        println!();
        
        // Track listen addresses for RPC
        let listen_addresses: Arc<RwLock<Vec<String>>> = Arc::new(RwLock::new(vec![self.config.p2p_addr.clone()]));

        // Create command channel for network operations
        let (network_cmd_tx, mut network_cmd_rx) = mpsc::unbounded_channel::<NetworkCommand>();

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

        // Create shared peer count tracker (used by both RPC and network event handler)
        let peer_count = Arc::new(RwLock::new(0usize));

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
            block_submission_handler: None, // Block submission handled via network events
            local_peer_id: Some(local_peer_id_str.clone()),
            listen_addresses: Arc::clone(&listen_addresses),
        });

        let rpc_server = RpcServer::new(rpc_addr, rpc_state).await?;
        println!("   RPC listening on: {}", rpc_addr);
        println!();

        self.network_cmd_tx = Some(network_cmd_tx.clone());
        self.rpc = Some(rpc_server);

        // Start event loop
        println!("✅ Node is ready!");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!();

        // Spawn network task (processes events and commands)
        tokio::task::spawn_local(async move {
            let mut network = network_service;
            let mut bootnode_retry_interval = time::interval(Duration::from_secs(10)); // Retry bootnodes every 10 seconds
            bootnode_retry_interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
            
            loop {
                tokio::select! {
                    // Process network events
                    _ = network.process_events() => {},

                    // Periodic bootnode retry
                    _ = bootnode_retry_interval.tick() => {
                        network.retry_bootnodes();
                    },

                    // Handle commands from other tasks
                    Some(cmd) = network_cmd_rx.recv() => {
                        match cmd {
                            NetworkCommand::BroadcastBlock(block) => {
                                match network.broadcast_block(block) {
                                    Err(e) if e.to_string().contains("InsufficientPeers") => {
                                        // Silently ignore InsufficientPeers - it's expected when no peers are connected
                                    }
                                    Err(e) => {
                                        eprintln!("Failed to broadcast block: {}", e);
                                    }
                                    Ok(_) => {}
                                }
                            }
                            NetworkCommand::BroadcastTransaction(tx) => {
                                if let Err(e) = network.broadcast_transaction(tx) {
                                    eprintln!("Failed to broadcast transaction: {}", e);
                                }
                            }
                            NetworkCommand::BroadcastStatus { best_height, best_hash, genesis_hash } => {
                                match network.broadcast_status(best_height, best_hash, genesis_hash) {
                                    Err(e) if e.to_string().contains("InsufficientPeers") => {
                                        // Silently ignore InsufficientPeers - it's expected when no peers are connected
                                    }
                                    Err(e) => {
                                        eprintln!("Failed to broadcast status: {}", e);
                                    }
                                    Ok(_) => {}
                                }
                            }
                            NetworkCommand::RequestBlocks { from_height, to_height } => {
                                println!("📡 Requesting blocks {}-{} from network", from_height, to_height);
                                if let Err(e) = network.request_blocks(from_height, to_height) {
                                    eprintln!("Failed to request blocks: {}", e);
                                }
                            }
                        }
                    }
                }
            }
        });

        // Create block buffer for out-of-order blocks
        let block_buffer: Arc<RwLock<HashMap<u64, coinject_core::Block>>> = Arc::new(RwLock::new(HashMap::new()));

        // Spawn network event handler
        let chain = Arc::clone(&self.chain);
        let state = Arc::clone(&self.state);
        let timelock_state = Arc::clone(&self.timelock_state);
        let escrow_state = Arc::clone(&self.escrow_state);
        let channel_state = Arc::clone(&self.channel_state);
        let trustline_state = Arc::clone(&self.trustline_state);
        let dimensional_pool_state = Arc::clone(&self.dimensional_pool_state);
        let marketplace_state = Arc::clone(&self.marketplace_state);
        let validator = Arc::clone(&self.validator);
        let tx_pool = Arc::clone(&self.tx_pool);
        let network_tx_for_events = network_cmd_tx.clone();
        let buffer_for_events = Arc::clone(&block_buffer);
        let peer_count_for_events = Arc::clone(&peer_count);
        let hf_sync_for_events = self.hf_sync.clone();

        tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                Self::handle_network_event(event, &chain, &state, &timelock_state, &escrow_state, &channel_state, &trustline_state, &dimensional_pool_state, &marketplace_state, &validator, &tx_pool, &network_tx_for_events, &buffer_for_events, &peer_count_for_events, &hf_sync_for_events).await;
            }
        });

        // Spawn periodic status broadcast task
        let chain_for_status = Arc::clone(&self.chain);
        let genesis_hash = self.chain.genesis_hash();
        let network_tx_for_status = network_cmd_tx.clone();

        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(10));
            loop {
                interval.tick().await;
                let best_height = chain_for_status.best_block_height().await;
                let best_hash = chain_for_status.best_block_hash().await;

                if let Err(e) = network_tx_for_status.send(NetworkCommand::BroadcastStatus {
                    best_height,
                    best_hash,
                    genesis_hash,
                }) {
                    eprintln!("Failed to send status broadcast command: {}", e);
                }
            }
        });

        // Spawn periodic metrics update task
        let chain_for_metrics = Arc::clone(&self.chain);
        let state_for_metrics = Arc::clone(&self.state);
        let dimensional_pool_state_for_metrics = Arc::clone(&self.dimensional_pool_state);
        let tx_pool_for_metrics = Arc::clone(&self.tx_pool);

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

            tokio::spawn(async move {
                Self::mining_loop(miner, chain, state, timelock_state, escrow_state, channel_state, trustline_state, dimensional_pool_state, marketplace_state, tx_pool, network_tx, hf_sync_for_mining, peer_count_for_mining).await;
            });
        }

        // Spawn periodic HuggingFace buffer flush task (fallback - block-based flushing is primary)
        // This ensures data is flushed even if block-based flushing doesn't trigger (e.g., during sync)
        if let Some(ref hf_sync) = self.hf_sync {
            let hf_sync_for_flush = Arc::clone(hf_sync);
            let chain_for_flush = Arc::clone(&self.chain);
            tokio::spawn(async move {
                let mut interval = time::interval(Duration::from_secs(600)); // Check every 10 minutes as fallback
                let mut last_flush_height = 0u64;
                loop {
                    interval.tick().await;
                    // Only flush if we haven't seen a new block in a while (fallback safety)
                    let current_height = chain_for_flush.best_block_height().await;
                    if current_height > last_flush_height + 600 {
                        // More than ~10 minutes of blocks since last check - force flush as safety measure
                        eprintln!(
                            "🔄 Hugging Face: Fallback flush triggered ({} blocks since last check)",
                            current_height - last_flush_height
                        );
                        if let Err(e) = hf_sync_for_flush.flush().await {
                            eprintln!("⚠️  Failed to flush Hugging Face buffer: {}", e);
                        } else {
                            eprintln!("✅ Hugging Face: Fallback buffer flush completed");
                        }
                        last_flush_height = current_height;
                    }
                }
            });
        }

        Ok(())
    }

    /// Handle network events
    async fn handle_network_event(
        event: NetworkEvent,
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
        network_tx: &mpsc::UnboundedSender<NetworkCommand>,
        block_buffer: &Arc<RwLock<HashMap<u64, coinject_core::Block>>>,
        peer_count: &Arc<RwLock<usize>>,
        hf_sync: &Option<Arc<HuggingFaceSync>>,
    ) {
        match event {
            NetworkEvent::BlockReceived { block, peer } => {
                println!("📥 Received block {} from {:?}", block.header.height, peer);

                let best_height = chain.best_block_height().await;
                let expected_height = best_height + 1;

                // Check if block is the next sequential block we need
                if block.header.height == expected_height {
                    // This is the next block we need - validate and apply immediately
                    let best_hash = chain.best_block_hash().await;

                    // Skip timestamp age check during sync (when receiving historical blocks)
                    // Check if block timestamp is older than 2 hours - if so, we're likely syncing
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as i64;
                    let block_age = now - block.header.timestamp;
                    let skip_age_check = block_age > 7200; // 2 hours

                    match validator.validate_block_with_options(&block, &best_hash, expected_height, skip_age_check) {
                        Ok(()) => {
                            // Store and apply block
                            match chain.store_block(&block).await {
                                Ok(is_new_best) => {
                                    if is_new_best {
                                        // RUNTIME INTEGRATION: Calculate and save consensus state for received blocks
                                        use coinject_core::{TAU_C, ConsensusState};
                                        let tau = (block.header.height as f64) / TAU_C;
                                        let consensus_state = ConsensusState::at_tau(tau);

                                        if let Err(e) = dimensional_pool_state.save_consensus_state(block.header.height, &consensus_state) {
                                            println!("⚠️  Warning: Failed to save consensus state: {}", e);
                                        }

                                        // Apply block transactions to state
                                        match Self::apply_block_transactions(&block, state, timelock_state, escrow_state, channel_state, trustline_state, dimensional_pool_state, marketplace_state) {
                                            Ok(applied_txs) => {
                                                println!("✅ Block {} accepted and applied to chain (τ={:.4})", block.header.height, tau);

                                                // Remove only successfully applied transactions from pool
                                                let mut pool = tx_pool.write().await;
                                                for tx_hash in &applied_txs {
                                                    pool.remove(tx_hash);
                                                }
                                                drop(pool);

                                                // Update block metrics
                                                crate::metrics::BLOCK_HEIGHT.set(block.header.height as i64);

                                                // Push consensus block to Hugging Face (fire-and-forget)
                                                if let Some(ref hf_sync) = hf_sync {
                                                    let hf_sync_clone = Arc::clone(hf_sync);
                                                    let block_clone = block.clone();
                                                    tokio::spawn(async move {
                                                        if let Err(e) = hf_sync_clone.push_consensus_block(&block_clone, false).await {
                                                            eprintln!("⚠️  Failed to push consensus block to Hugging Face: {}", e);
                                                        }
                                                    });

                                                    // Upload marketplace transactions from this block
                                                    Self::upload_marketplace_transactions(&block, marketplace_state, hf_sync);
                                                }

                                                // After applying this block, try to apply buffered blocks sequentially
                                                Self::process_buffered_blocks(
                                                    chain,
                                                    state,
                                                    timelock_state,
                                                    escrow_state,
                                                    channel_state,
                                                    trustline_state,
                                                    dimensional_pool_state,
                                                    marketplace_state,
                                                    validator,
                                                    tx_pool,
                                                    block_buffer,
                                                    hf_sync,
                                                    Some(network_tx),
                                                ).await;

                                                // After processing buffered blocks, check if we have a longer chain available
                                                // This handles the case where we received blocks from a fork that's longer
                                                let new_best_height = chain.best_block_height().await;
                                                let new_best_hash = chain.best_block_hash().await;
                                                
                                                // If we've advanced, check if there are any fork blocks that might form a longer chain
                                                // This is a simplified check - full implementation would track all fork chains
                                                if new_best_height > block.header.height {
                                                    // We've advanced past this block, check for reorganization opportunities
                                                    let _ = Self::check_and_reorganize_chain(
                                                        chain,
                                                        state,
                                                        timelock_state,
                                                        escrow_state,
                                                        channel_state,
                                                        trustline_state,
                                                        dimensional_pool_state,
                                                        marketplace_state,
                                                        validator,
                                                        block_buffer,
                                                    ).await;
                                                }
                                            }
                                            Err(e) => {
                                                println!("❌ Failed to apply block transactions: {}", e);
                                            }
                                        }
                                    }
                                }
                                Err(e) => println!("❌ Failed to store block: {}", e),
                            }
                        }
                        Err(e) => {
                            println!("❌ Block validation failed: {}", e);
                        }
                    }
                } else if block.header.height > expected_height {
                    // Future block - add to buffer for later processing
                    let mut buffer = block_buffer.write().await;

                    // Only buffer if we don't already have it
                    if !buffer.contains_key(&block.header.height) {
                        println!(
                            "🗃️  Buffering future block {} (expected: {}, buffer size: {})",
                            block.header.height,
                            expected_height,
                            buffer.len() + 1
                        );
                        buffer.insert(block.header.height, block);
                        
                        // After buffering, try to process buffered blocks in case we now have sequential blocks
                        drop(buffer);
                        Self::process_buffered_blocks(
                            chain,
                            state,
                            timelock_state,
                            escrow_state,
                            channel_state,
                            trustline_state,
                            dimensional_pool_state,
                            marketplace_state,
                            validator,
                            tx_pool,
                            block_buffer,
                            hf_sync,
                            Some(network_tx),
                        ).await;
                    }
                } else if block.header.height == best_height {
                    // Block at same height but potentially different hash - fork detected
                    let best_hash = chain.best_block_hash().await;
                    if block.header.hash() != best_hash {
                        println!("⚠️  Fork detected at height {}! Our hash: {:?}, Received hash: {:?}", 
                            block.header.height, best_hash, block.header.hash());
                        println!("   Storing fork block for potential reorganization...");
                        
                        // Store the fork block (it might be part of a longer chain)
                        let _ = chain.store_block(&block).await;
                        
                        // Request full chain from this peer to check if it's longer
                        // The status update handler will trigger this, but we can also request here
                        // For now, just log - the status update will handle requesting the chain
                    } else {
                        // Same block, ignore
                        println!("⏭️  Ignoring duplicate block {} (current height: {})", block.header.height, best_height);
                    }
                } else {
                    // Old block we already have - ignore it
                    println!("⏭️  Ignoring old block {} (current height: {})", block.header.height, best_height);
                }
            }
            NetworkEvent::TransactionReceived { tx, peer } => {
                println!("📨 Received transaction {:?} from {:?}", tx.hash(), peer);

                // Validate and add to transaction pool
                if tx.verify_signature() {
                    let mut pool = tx_pool.write().await;
                    match pool.add(tx) {
                        Ok(hash) => println!("✅ Added transaction {:?} to pool", hash),
                        Err(e) => println!("❌ Failed to add transaction to pool: {}", e),
                    }
                } else {
                    println!("❌ Invalid transaction signature, rejecting");
                }
            }
            NetworkEvent::PeerConnected(peer) => {
                println!("🤝 Peer connected: {:?}", peer);

                // Update peer count
                let mut count = peer_count.write().await;
                *count += 1;
                let count_value = *count;
                drop(count);

                // Update Prometheus metric
                crate::metrics::PEER_COUNT.set(count_value as i64);
            }
            NetworkEvent::PeerDisconnected(peer) => {
                println!("👋 Peer disconnected: {:?}", peer);

                // Update peer count
                let mut count = peer_count.write().await;
                if *count > 0 {
                    *count -= 1;
                }
                let count_value = *count;
                drop(count);

                // Update Prometheus metric
                crate::metrics::PEER_COUNT.set(count_value as i64);
            }
            NetworkEvent::StatusUpdate { peer, best_height, best_hash } => {
                let our_height = chain.best_block_height().await;
                let our_hash = chain.best_block_hash().await;

                println!(
                    "📊 Status update from {:?}: height {} hash={:?} (ours: {} hash={:?})",
                    peer, best_height, best_hash, our_height, our_hash
                );

                // Check if peer has a longer or different chain
                if best_height > our_height {
                    // Peer is ahead - request blocks to catch up
                    let sync_from = our_height + 1;
                    let sync_to = best_height;

                    println!(
                        "🔄 Peer is ahead! Requesting blocks {}-{} for sync",
                        sync_from, sync_to
                    );

                    // Request missing blocks (in chunks of 100 to avoid overwhelming)
                    let chunk_size = 100u64;
                    let mut current = sync_from;

                    while current <= sync_to {
                        let end = std::cmp::min(current + chunk_size - 1, sync_to);

                        if let Err(e) = network_tx.send(NetworkCommand::RequestBlocks {
                            from_height: current,
                            to_height: end,
                        }) {
                            eprintln!("Failed to send RequestBlocks command: {}", e);
                            break;
                        }

                        current = end + 1;
                    }
                } else if best_height == our_height && best_hash != our_hash {
                    // Fork detected at same height - check if peer's chain is longer by requesting their chain
                    println!("⚠️  Fork detected at height {}! Our hash: {:?}, Peer hash: {:?}", 
                        best_height, our_hash, best_hash);
                    println!("   Requesting peer's full chain to check for longer fork...");
                    
                    // Request blocks from genesis to their best to validate their chain
                    // We'll reorganize if their chain is valid and longer
                    // Note: After receiving these blocks, we'll need to check if they form a longer chain
                    // and trigger reorganization. This is handled by checking after block processing.
                    if let Err(e) = network_tx.send(NetworkCommand::RequestBlocks {
                        from_height: 0,
                        to_height: best_height,
                    }) {
                        eprintln!("Failed to request full chain for fork analysis: {}", e);
                    }
                    
                    // Also check if we already have the peer's best block stored
                    // If so, we can immediately check for reorganization
                    if let Ok(Some(_peer_best_block)) = chain.get_block_by_hash(&best_hash) {
                        // We have the peer's best block - check if it's part of a longer chain
                        // This will be handled after we receive more blocks, but we can check now
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
                            // Attempt reorganization if this forms a longer chain
                            match Self::attempt_reorganization_if_longer_chain(
                                best_hash,
                                best_height,
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
                                Ok(reorganized) => {
                                    if reorganized {
                                        println!("✅ Successfully reorganized to longer chain ending at height {}", best_height);
                                    }
                                }
                                Err(e) => {
                                    eprintln!("⚠️  Failed to attempt reorganization: {}", e);
                                }
                            }
                        });
                    }
                } else if best_height < our_height {
                    // Peer is behind - they should sync from us (they'll request when they see our status)
                    // But also check if their chain might be a fork that's actually longer
                    // by checking if their best hash exists in our chain at that height
                    if let Ok(Some(block_at_height)) = chain.get_block_by_height(best_height) {
                        if block_at_height.header.hash() != best_hash {
                            // Different block at same height - potential fork, but we're ahead so ignore
                            println!("   Peer is behind and on different fork, ignoring");
                        }
                    }
                }
            }
            NetworkEvent::BlocksRequested { peer, from_height, to_height } => {
                println!(
                    "📮 Blocks requested by {:?}: heights {}-{}",
                    peer, from_height, to_height
                );

                // Respond by broadcasting the requested blocks
                let mut sent_count = 0;
                for height in from_height..=to_height {
                    match chain.get_block_by_height(height) {
                        Ok(Some(block)) => {
                        if height <= 20 {
                            println!(
                                "   ↳ Serving requested block {} (hash {:?}) to {:?}",
                                height,
                                block.header.hash(),
                                peer
                            );
                        }
                            if let Err(e) = network_tx.send(NetworkCommand::BroadcastBlock(block)) {
                                eprintln!("Failed to broadcast block {}: {}", height, e);
                                break;
                            }
                            sent_count += 1;
                        }
                        Ok(None) => {
                        if height <= 20 {
                            println!(
                                "   ↳ Missing requested block {} (first missing in range {}-{})",
                                height, from_height, to_height
                            );
                        }
                            break;
                        }
                        Err(e) => {
                            eprintln!("Error fetching block {}: {}", height, e);
                            break;
                        }
                    }
                }

                if sent_count > 0 {
                    println!("📤 Sent {} blocks in response to sync request", sent_count);
                }
            }
        }
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

                    match validator.validate_block_with_options(&block, &best_hash, next_height, skip_age_check) {
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
                            let request_from = next_height;
                            let request_to = std::cmp::min(next_height + 99, max_buffered_height - 1);
                            
                            println!("⚠️  Missing block {} (have blocks up to {} in buffer), requesting {}-{}", 
                                next_height, max_buffered_height, request_from, request_to);
                            
                            drop(buffer);
                            
                            // Request missing blocks if network_tx is available
                            if let Some(network_tx) = network_tx {
                                if let Err(e) = network_tx.send(NetworkCommand::RequestBlocks {
                                    from_height: request_from,
                                    to_height: request_to,
                                }) {
                                    eprintln!("Failed to request missing blocks: {}", e);
                                }
                            }
                            
                            // Break and wait for blocks to arrive
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
    ) {
        let current_best_height = chain.best_block_height().await;
        let current_best_hash = chain.best_block_hash().await;

        // Check if we have blocks in buffer that might form a longer chain
        let buffer = block_buffer.read().await;
        if buffer.is_empty() {
            return;
        }

        // Find the highest block in buffer
        let max_buffered_height = buffer.keys().max().copied().unwrap_or(0);
        
        // If we have blocks that extend beyond our current best, check if they form a valid chain
        if max_buffered_height > current_best_height {
            // Try to build a chain from current best to max buffered height
            let mut chain_path = Vec::new();
            let mut current_hash = current_best_hash;
            let mut current_height = current_best_height;

            // Try to find a path through buffered blocks
            while current_height < max_buffered_height {
                let next_height = current_height + 1;
                
                // Look for a block at next_height that connects to current_hash
                let mut found = false;
                for (height, block) in buffer.iter() {
                    if *height == next_height && block.header.prev_hash == current_hash {
                        chain_path.push(block.clone());
                        current_hash = block.header.hash();
                        current_height = next_height;
                        found = true;
                        break;
                    }
                }

                if !found {
                    // Can't form a complete chain from buffer
                    break;
                }
            }

            // If we found a complete chain path, it will be processed by process_buffered_blocks
            // This check is mainly for detecting forks
        }

        // Check for forks at same height - if we have a block at current height with different hash
        // and it's part of a longer chain, we should reorganize
        if let Some(fork_block) = buffer.get(&current_best_height) {
            if fork_block.header.hash() != current_best_hash {
                // Fork detected - we'd need to request the full chain from the peer
                // to see if it's longer. This is handled by status update handler.
                println!("   Fork block at height {} detected in buffer, waiting for full chain...", current_best_height);
            }
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

        // Find common ancestor
        let (common_hash, common_height) = match chain.find_common_ancestor(&new_chain_end_hash, new_chain_end_height).await
            .map_err(|e| format!("Failed to find common ancestor: {}", e)) {
            Ok(Some((hash, height))) => (hash, height),
            Ok(None) => {
                println!("⚠️  No common ancestor found, cannot reorganize");
                return Ok(false);
            }
            Err(e) => return Err(e),
        };

        println!("🔄 Found common ancestor at height {} (hash: {:?})", common_height, common_hash);

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

        // Validate new chain is actually longer
        if new_chain_blocks.len() <= old_chain_blocks.len() {
            println!("   New chain is not longer ({} vs {} blocks), skipping reorganization", 
                new_chain_blocks.len(), old_chain_blocks.len());
            return Ok(false);
        }

        println!("🔄 Reorganizing: unwinding {} blocks, applying {} blocks",
            old_chain_blocks.len(), new_chain_blocks.len());

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
        trustline_state: &Arc<TrustLineState>,
        dimensional_pool_state: &Arc<DimensionalPoolState>,
        marketplace_state: &Arc<MarketplaceState>,
        block_height: u64,
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
            let expected_height = if idx == 0 {
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
                        let channel = channel_state.get_channel(&channel_tx.channel_id)
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
        hf_sync: Option<Arc<HuggingFaceSync>>,
        peer_count: Arc<RwLock<usize>>,
    ) {
        // Wait for peer connections and initial chain sync before mining
        println!("⏳ Waiting for peer connections and chain sync before mining...");
        let mut sync_wait_interval = time::interval(Duration::from_secs(2));
        let mut sync_attempts = 0;
        const MAX_SYNC_WAIT_ATTEMPTS: u32 = 150; // Wait up to 5 minutes for sync
        let mut last_height = 0u64;
        let mut stable_height_count = 0u32;
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

                // If we're at genesis, wait longer for status updates (at least 15 seconds = 7-8 attempts)
                if best_height == 0 {
                    if sync_attempts >= 8 {
                        println!("✅ Connected to {} peer(s) at genesis, waiting for status updates...", current_peers);
                        // Continue waiting - don't break yet
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

        // Start mining loop
        let mut interval = time::interval(Duration::from_secs(10));
        let mut last_mined_height = chain.best_block_height().await;

        loop {
            interval.tick().await;

            let best_height = chain.best_block_height().await;
            let best_hash = chain.best_block_hash().await;

            // Check if chain advanced since last mining attempt (block received from peer)
            if best_height > last_mined_height {
                println!("📥 Chain advanced from {} to {} (block received from peer), skipping this mining cycle", 
                    last_mined_height, best_height);
                last_mined_height = best_height;
                continue; // Skip mining this cycle, wait for next interval
            }

            // Only mine if we're still at the same height (no new blocks received)
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
                    println!("📡 Broadcasted block to network");
                }

                // Push consensus block to Hugging Face (fire-and-forget)
                if let Some(ref hf_sync) = hf_sync {
                    eprintln!("📦 Hugging Face: Preparing to upload mined block {}", block.header.height);
                    let hf_sync_clone = Arc::clone(hf_sync);
                    let block_clone = block.clone();
                    tokio::spawn(async move {
                        eprintln!("📦 Hugging Face: Starting async upload for block {}", block_clone.header.height);
                        match hf_sync_clone.push_consensus_block(&block_clone, true).await {
                            Ok(()) => eprintln!("✅ Hugging Face: Successfully queued block {} for upload", block_clone.header.height),
                            Err(e) => eprintln!("❌ Failed to push consensus block {} to Hugging Face: {}", block_clone.header.height, e),
                        }
                    });

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
