// JSON-RPC Server for COINjecture Network B
// Provides wallet and client API access
//
// NOTE: Some error codes are prepared for future RPC methods
#![allow(dead_code)]

use coinject_core::{
    Address, Balance, Block, BlockHeader, Hash, Transaction,
    ProblemType, SubmissionMode, ProblemReveal, WellformednessProof, ProblemParameters,
};
use coinject_mempool::{ProblemMarketplace, TransactionPool};
use coinject_state::{MarketplaceStats, ProblemSubmission};
use coinject_state::{
    AccountState, TimeLockState, TimeLock, EscrowState, Escrow,
    ChannelState, Channel, MarketplaceState
};
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    server::{Server, ServerHandle},
    types::ErrorObjectOwned,
};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tower::ServiceBuilder;
use tower::timeout::TimeoutLayer;
use tower_http::cors::{CorsLayer, Any};
use crate::middleware::{AuditLogLayer, SecurityGateLayer, SecurityConfig};
use crate::tls::TlsConfig;

/// Trait for reading blockchain data (allows node to provide chain state without circular dependency)
pub trait BlockchainReader: Send + Sync {
    fn get_block_by_height(&self, height: u64) -> Result<Option<Block>, String>;
    fn get_block_by_hash(&self, hash: &Hash) -> Result<Option<Block>, String>;
    fn get_header_by_height(&self, height: u64) -> Result<Option<BlockHeader>, String>;
    /// Calculate cumulative work score up to given height (sum of all block work_scores)
    fn calculate_chain_work(&self, up_to_height: u64) -> Result<u64, String> {
        // Default implementation: sum work_scores from all headers
        let mut total: u64 = 0;
        for h in 0..=up_to_height {
            if let Ok(Some(header)) = self.get_header_by_height(h) {
                total = total.saturating_add(header.work_score as u64);
            }
        }
        Ok(total)
    }
}

/// RPC error codes
const INVALID_PARAMS: i32 = -32602;
const INTERNAL_ERROR: i32 = -32603;
const NOT_FOUND: i32 = -32001;

/// Chain information response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainInfo {
    pub chain_id: String,
    pub best_height: u64,
    pub best_hash: String,
    pub genesis_hash: String,
    pub peer_count: usize,
    /// Cumulative work score of the best chain (fork-choice weight)
    pub total_work: u64,
    /// Whether the node is currently syncing
    pub is_syncing: bool,
}

/// Network information response (for P2P debugging)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInfo {
    pub peer_id: String,
    pub peer_count: usize,
    pub listen_addresses: Vec<String>,
    pub bootnode_address_hint: String,
}

/// Account information response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountInfo {
    pub address: String,
    pub balance: Balance,
    pub nonce: u64,
}

/// Transaction status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionStatus {
    pub tx_hash: String,
    pub status: String, // "pending", "confirmed", "failed"
    pub block_height: Option<u64>,
}

/// Problem marketplace response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProblemInfo {
    pub problem_id: String,
    pub submitter: String,
    pub bounty: Balance,
    pub min_work_score: f64,
    pub status: String,
    pub submitted_at: i64,
    pub expires_at: i64,
    pub is_private: bool,
    pub problem_type: Option<String>,
    pub problem_size: Option<usize>,
    pub is_revealed: bool,
}

/// Private problem submission parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivateProblemParams {
    pub commitment: String,
    pub proof_bytes: String,
    pub vk_hash: String,
    pub public_inputs: Vec<String>,
    pub problem_type: String,
    pub size: usize,
    pub complexity_estimate: f64,
    pub bounty: Balance,
    pub min_work_score: f64,
    pub expiration_days: u64,
}

/// Problem reveal parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevealParams {
    pub problem_id: String,
    pub problem: String, // JSON-encoded ProblemType
    pub salt: String,    // Hex-encoded 32-byte salt
}

/// Public SubsetSum problem submission (Phase 2 MVP - simple API)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicSubsetSumParams {
    /// Numbers in the subset sum problem
    pub numbers: Vec<i64>,
    /// Target sum to find
    pub target: i64,
    /// Bounty in tokens (must have sufficient balance)
    pub bounty: Balance,
    /// Minimum work score required for solution
    pub min_work_score: f64,
    /// Days until expiration (1-365)
    pub expiration_days: u64,
    /// Submitter's hex-encoded address
    pub submitter: String,
}

/// Solution submission parameters (Phase 2 MVP)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolutionSubmissionParams {
    /// Problem ID (hex-encoded)
    pub problem_id: String,
    /// Selected indices that sum to target
    pub selected_indices: Vec<usize>,
    /// Solver's hex-encoded address
    pub solver: String,
}

/// TimeLock information response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeLockInfo {
    pub tx_hash: String,
    pub from: String,
    pub recipient: String,
    pub amount: Balance,
    pub unlock_time: i64,
    pub created_at_height: u64,
}

/// Escrow information response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscrowInfo {
    pub escrow_id: String,
    pub sender: String,
    pub recipient: String,
    pub arbiter: Option<String>,
    pub amount: Balance,
    pub timeout: i64,
    pub conditions_hash: String,
    pub status: String,
    pub created_at_height: u64,
    pub resolved_at_height: Option<u64>,
}

/// Channel information response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelInfo {
    pub channel_id: String,
    pub participant_a: String,
    pub participant_b: String,
    pub deposit_a: Balance,
    pub deposit_b: Balance,
    pub balance_a: Balance,
    pub balance_b: Balance,
    pub sequence: u64,
    pub dispute_timeout: i64,
    pub status: String,
    pub opened_at_height: u64,
    pub closed_at_height: Option<u64>,
}

/// Faucet request response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaucetResponse {
    pub success: bool,
    pub amount: Option<Balance>,
    pub new_balance: Option<Balance>,
    pub message: String,
    pub cooldown_remaining: Option<u64>,
}

/// JSON-RPC API definition
#[rpc(server, client)]
pub trait CoinjectRpc {
    /// Get account balance
    #[method(name = "account_getBalance")]
    async fn get_balance(&self, address: String) -> RpcResult<Balance>;

    /// Get account nonce
    #[method(name = "account_getNonce")]
    async fn get_nonce(&self, address: String) -> RpcResult<u64>;

    /// Get account information
    #[method(name = "account_getInfo")]
    async fn get_account_info(&self, address: String) -> RpcResult<AccountInfo>;

    /// Submit transaction
    #[method(name = "transaction_submit")]
    async fn submit_transaction(&self, tx_hex: String) -> RpcResult<String>;

    /// Get transaction status
    #[method(name = "transaction_getStatus")]
    async fn get_transaction_status(&self, tx_hash: String) -> RpcResult<TransactionStatus>;

    /// Get block by height
    #[method(name = "chain_getBlock")]
    async fn get_block(&self, height: u64) -> RpcResult<Option<Block>>;

    /// Get latest block
    #[method(name = "chain_getLatestBlock")]
    async fn get_latest_block(&self) -> RpcResult<Option<Block>>;

    /// Get block header by height
    #[method(name = "chain_getBlockHeader")]
    async fn get_block_header(&self, height: u64) -> RpcResult<Option<BlockHeader>>;

    /// Get chain information
    #[method(name = "chain_getInfo")]
    async fn get_chain_info(&self) -> RpcResult<ChainInfo>;

    /// Next mining instance: same deterministic `ProblemType` as the node's miner
    /// for `(next_height = best_height + 1, prev_hash = tip)`. Used by Solver Lab `instance.json`.
    #[method(name = "chain_getMiningWork")]
    async fn get_mining_work(&self) -> RpcResult<MiningWork>;

    /// Get open problems from marketplace
    #[method(name = "marketplace_getOpenProblems")]
    async fn get_open_problems(&self) -> RpcResult<Vec<ProblemInfo>>;

    /// Get problem by ID
    #[method(name = "marketplace_getProblem")]
    async fn get_problem(&self, problem_id: String) -> RpcResult<Option<ProblemInfo>>;

    /// Get marketplace statistics
    #[method(name = "marketplace_getStats")]
    async fn get_marketplace_stats(&self) -> RpcResult<MarketplaceStats>;

    /// Submit private problem with commitment and ZK proof
    #[method(name = "marketplace_submitPrivateProblem")]
    async fn submit_private_problem(&self, params: PrivateProblemParams) -> RpcResult<String>;

    /// Reveal problem for private bounty
    #[method(name = "marketplace_revealProblem")]
    async fn reveal_problem(&self, params: RevealParams) -> RpcResult<bool>;

    /// Submit a public SubsetSum problem (Phase 2 MVP - simple API)
    /// Returns problem_id on success
    #[method(name = "marketplace_submitPublicSubsetSum")]
    async fn submit_public_subset_sum(&self, params: PublicSubsetSumParams) -> RpcResult<String>;

    /// Submit solution to an open problem (Phase 2 MVP)
    /// Returns true if solution is valid and bounty awarded
    #[method(name = "marketplace_submitSolution")]
    async fn submit_solution(&self, params: SolutionSubmissionParams) -> RpcResult<bool>;

    /// Get timelocks for a recipient address
    #[method(name = "timelock_getByRecipient")]
    async fn get_timelocks_by_recipient(&self, recipient: String) -> RpcResult<Vec<TimeLockInfo>>;

    /// Get all unlocked timelocks
    #[method(name = "timelock_getUnlocked")]
    async fn get_unlocked_timelocks(&self) -> RpcResult<Vec<TimeLockInfo>>;

    /// Get escrows by sender address
    #[method(name = "escrow_getBySender")]
    async fn get_escrows_by_sender(&self, sender: String) -> RpcResult<Vec<EscrowInfo>>;

    /// Get escrows by recipient address
    #[method(name = "escrow_getByRecipient")]
    async fn get_escrows_by_recipient(&self, recipient: String) -> RpcResult<Vec<EscrowInfo>>;

    /// Get active escrows
    #[method(name = "escrow_getActive")]
    async fn get_active_escrows(&self) -> RpcResult<Vec<EscrowInfo>>;

    /// Get channels for an address
    #[method(name = "channel_getByAddress")]
    async fn get_channels_by_address(&self, address: String) -> RpcResult<Vec<ChannelInfo>>;

    /// Get open channels
    #[method(name = "channel_getOpen")]
    async fn get_open_channels(&self) -> RpcResult<Vec<ChannelInfo>>;

    /// Get disputed channels
    #[method(name = "channel_getDisputed")]
    async fn get_disputed_channels(&self) -> RpcResult<Vec<ChannelInfo>>;

    /// Request testnet tokens from faucet (testnet only)
    #[method(name = "faucet_requestTokens")]
    async fn faucet_request_tokens(&self, address: String) -> RpcResult<FaucetResponse>;

    /// Get network information including PeerId (for P2P debugging and bootnode configuration)
    #[method(name = "network_getInfo")]
    async fn get_network_info(&self) -> RpcResult<NetworkInfo>;

    /// Submit a mined block to the network
    #[method(name = "chain_submitBlock")]
    async fn submit_block(&self, block: Block) -> RpcResult<String>;
}

/// Faucet handler callback type
pub type FaucetHandler = Arc<dyn Fn(&Address) -> Result<Balance, String> + Send + Sync>;

/// Block submission handler callback type
pub type BlockSubmissionHandler = Arc<dyn Fn(Block) -> Result<String, String> + Send + Sync>;

/// Mining template for the block that would extend the current tip (Solver Lab / web miners).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiningWork {
    /// Height of the block built on the current tip (`best_height + 1`).
    pub next_height: u64,
    /// Parent block hash (epoch salt) as hex — must match `chain_getInfo.best_hash` when you pull and submit immediately.
    pub prev_hash: String,
    /// Deterministic instance: SubsetSum, SAT, or TSP (same serde as `chain_getBlock` / `solution_reveal.problem`).
    pub problem: ProblemType,
}

pub type MiningWorkFuture = Pin<Box<dyn Future<Output = Result<MiningWork, String>> + Send>>;
pub type MiningWorkProvider = Arc<dyn Fn() -> MiningWorkFuture + Send + Sync>;

/// RPC server state
pub struct RpcServerState {
    pub account_state: Arc<AccountState>,
    pub timelock_state: Arc<TimeLockState>,
    pub escrow_state: Arc<EscrowState>,
    pub channel_state: Arc<ChannelState>,
    pub marketplace_state: Arc<MarketplaceState>,
    pub blockchain: Arc<dyn BlockchainReader>,
    pub marketplace: Arc<RwLock<ProblemMarketplace>>,
    pub tx_pool: Arc<RwLock<TransactionPool>>,
    pub chain_id: String,
    pub best_height: Arc<RwLock<u64>>,
    pub best_hash: Arc<RwLock<Hash>>,
    pub genesis_hash: Hash,
    pub peer_count: Arc<RwLock<usize>>,
    pub faucet_handler: Option<FaucetHandler>,
    /// Block submission handler (validates and broadcasts blocks)
    pub block_submission_handler: Option<BlockSubmissionHandler>,
    /// Local PeerId for network identification
    pub local_peer_id: Option<String>,
    /// Listen addresses for the P2P network
    pub listen_addresses: Arc<RwLock<Vec<String>>>,
    /// Whether the node is currently syncing
    pub is_syncing: Arc<RwLock<bool>>,
    /// When set (nodes with consensus miner), serves [`chain_getMiningWork`].
    pub mining_work_provider: Option<MiningWorkProvider>,
}

/// RPC server implementation
pub struct RpcServerImpl {
    state: Arc<RpcServerState>,
}

impl RpcServerImpl {
    pub fn new(state: Arc<RpcServerState>) -> Self {
        RpcServerImpl { state }
    }

    // -----------------------------------------------------------------------
    // Input validation helpers
    // -----------------------------------------------------------------------

    /// Reject if the string exceeds `max_len` UTF-8 bytes.
    fn validate_str_len(value: &str, max_len: usize, field: &str) -> RpcResult<()> {
        if value.len() > max_len {
            return Err(ErrorObjectOwned::owned(
                INVALID_PARAMS,
                format!("{} exceeds maximum length of {} bytes", field, max_len),
                None::<()>,
            ));
        }
        Ok(())
    }

    /// Sanitise an internal error: log the real message, return a generic one.
    /// Prevents file paths, panics, or database details from leaking over the wire.
    fn internal_error(detail: impl std::fmt::Display) -> ErrorObjectOwned {
        tracing::error!(detail = %detail, "rpc.internal_error");
        ErrorObjectOwned::owned(INTERNAL_ERROR, "Internal server error", None::<()>)
    }

    // -----------------------------------------------------------------------
    // Address / hash parsing
    // -----------------------------------------------------------------------

    /// Parse hex address to Address type
    fn parse_address(&self, address: &str) -> RpcResult<Address> {
        let bytes = hex::decode(address.trim_start_matches("0x"))
            .map_err(|e| ErrorObjectOwned::owned(INVALID_PARAMS, e.to_string(), None::<()>))?;

        if bytes.len() != 32 {
            return Err(ErrorObjectOwned::owned(
                INVALID_PARAMS,
                "Address must be 32 bytes",
                None::<()>,
            ));
        }

        let mut addr_bytes = [0u8; 32];
        addr_bytes.copy_from_slice(&bytes);
        Ok(Address::from_bytes(addr_bytes))
    }

    /// Parse hex hash to Hash type
    fn parse_hash(&self, hash: &str) -> RpcResult<Hash> {
        let bytes = hex::decode(hash.trim_start_matches("0x"))
            .map_err(|e| ErrorObjectOwned::owned(INVALID_PARAMS, e.to_string(), None::<()>))?;

        if bytes.len() != 32 {
            return Err(ErrorObjectOwned::owned(
                INVALID_PARAMS,
                "Hash must be 32 bytes",
                None::<()>,
            ));
        }

        let mut hash_bytes = [0u8; 32];
        hash_bytes.copy_from_slice(&bytes);
        Ok(Hash::from_bytes(hash_bytes))
    }

    /// Convert TimeLock to TimeLockInfo
    fn timelock_to_info(&self, timelock: &TimeLock) -> TimeLockInfo {
        TimeLockInfo {
            tx_hash: hex::encode(timelock.tx_hash.as_bytes()),
            from: hex::encode(timelock.from.as_bytes()),
            recipient: hex::encode(timelock.recipient.as_bytes()),
            amount: timelock.amount,
            unlock_time: timelock.unlock_time,
            created_at_height: timelock.created_at_height,
        }
    }

    /// Convert Escrow to EscrowInfo
    fn escrow_to_info(&self, escrow: &Escrow) -> EscrowInfo {
        EscrowInfo {
            escrow_id: hex::encode(escrow.escrow_id.as_bytes()),
            sender: hex::encode(escrow.sender.as_bytes()),
            recipient: hex::encode(escrow.recipient.as_bytes()),
            arbiter: escrow.arbiter.as_ref().map(|a| hex::encode(a.as_bytes())),
            amount: escrow.amount,
            timeout: escrow.timeout,
            conditions_hash: hex::encode(escrow.conditions_hash.as_bytes()),
            status: format!("{:?}", escrow.status),
            created_at_height: escrow.created_at_height,
            resolved_at_height: escrow.resolved_at_height,
        }
    }

    /// Convert Channel to ChannelInfo
    fn channel_to_info(&self, channel: &Channel) -> ChannelInfo {
        ChannelInfo {
            channel_id: hex::encode(channel.channel_id.as_bytes()),
            participant_a: hex::encode(channel.participant_a.as_bytes()),
            participant_b: hex::encode(channel.participant_b.as_bytes()),
            deposit_a: channel.deposit_a,
            deposit_b: channel.deposit_b,
            balance_a: channel.balance_a,
            balance_b: channel.balance_b,
            sequence: channel.sequence,
            dispute_timeout: channel.dispute_timeout,
            status: format!("{:?}", channel.status),
            opened_at_height: channel.opened_at_height,
            closed_at_height: channel.closed_at_height,
        }
    }

    /// Convert ProblemSubmission to ProblemInfo
    fn problem_to_info(&self, problem: &ProblemSubmission) -> ProblemInfo {
        let (is_private, problem_type, problem_size, is_revealed) = match &problem.submission_mode {
            SubmissionMode::Public { problem } => {
                let problem_type_name = match problem {
                    ProblemType::SubsetSum { numbers, .. } => {
                        Some(format!("SubsetSum({})", numbers.len()))
                    }
                    ProblemType::SAT { variables, clauses } => {
                        Some(format!("SAT(vars={}, clauses={})", variables, clauses.len()))
                    }
                    ProblemType::TSP { cities, .. } => {
                        Some(format!("TSP(cities={})", cities))
                    }
                    ProblemType::Custom { .. } => Some("Custom".to_string()),
                };
                let size = match problem {
                    ProblemType::SubsetSum { numbers, .. } => Some(numbers.len()),
                    ProblemType::SAT { variables, .. } => Some(*variables),
                    ProblemType::TSP { cities, .. } => Some(*cities),
                    ProblemType::Custom { .. } => None,
                };
                (false, problem_type_name, size, true)
            }
            SubmissionMode::Private { public_params, .. } => {
                let problem_type_name = Some(public_params.problem_type.clone());
                let size = Some(public_params.size);
                let is_revealed = problem.problem_reveal.is_some();
                (true, problem_type_name, size, is_revealed)
            }
        };

        ProblemInfo {
            problem_id: hex::encode(problem.problem_id.as_bytes()),
            submitter: hex::encode(problem.submitter.as_bytes()),
            bounty: problem.bounty,
            min_work_score: problem.min_work_score,
            status: format!("{:?}", problem.status),
            submitted_at: problem.submitted_at,
            expires_at: problem.expires_at,
            is_private,
            problem_type,
            problem_size,
            is_revealed,
        }
    }
}

#[async_trait]
impl CoinjectRpcServer for RpcServerImpl {
    async fn get_balance(&self, address: String) -> RpcResult<Balance> {
        Self::validate_str_len(&address, 256, "address")?;
        let addr = self.parse_address(&address)?;
        Ok(self.state.account_state.get_balance(&addr))
    }

    async fn get_nonce(&self, address: String) -> RpcResult<u64> {
        Self::validate_str_len(&address, 256, "address")?;
        let addr = self.parse_address(&address)?;
        Ok(self.state.account_state.get_nonce(&addr))
    }

    async fn get_account_info(&self, address: String) -> RpcResult<AccountInfo> {
        Self::validate_str_len(&address, 256, "address")?;
        let addr = self.parse_address(&address)?;
        let balance = self.state.account_state.get_balance(&addr);
        let nonce = self.state.account_state.get_nonce(&addr);

        Ok(AccountInfo {
            address: address.clone(),
            balance,
            nonce,
        })
    }

    async fn submit_transaction(&self, tx_hex: String) -> RpcResult<String> {
        // Hard size limit: 256 KB for transaction payloads
        const TX_MAX_BYTES: usize = 256 * 1024;
        if tx_hex.len() > TX_MAX_BYTES {
            return Err(ErrorObjectOwned::owned(
                INVALID_PARAMS,
                "Transaction payload exceeds 256 KB limit",
                None::<()>,
            ));
        }

        // Check if it's JSON format (from web wallet) or hex-encoded bincode (from CLI)
        let tx: Transaction = if tx_hex.trim().starts_with('{') {
            // JSON format (from web wallet)
            serde_json::from_str(&tx_hex)
                .map_err(|e| ErrorObjectOwned::owned(INVALID_PARAMS, format!("JSON deserialize error: {}", e), None::<()>))?
        } else {
            // Hex-encoded bincode format (from CLI wallet)
            let tx_bytes = hex::decode(tx_hex.trim_start_matches("0x"))
                .map_err(|e| ErrorObjectOwned::owned(INVALID_PARAMS, format!("Hex decode error: {}", e), None::<()>))?;
            bincode::deserialize(&tx_bytes)
                .map_err(|e| ErrorObjectOwned::owned(INVALID_PARAMS, format!("Bincode deserialize error: {}", e), None::<()>))?
        };

        // Basic validation
        if !tx.verify_signature() {
            return Err(ErrorObjectOwned::owned(
                INVALID_PARAMS,
                "Invalid transaction signature",
                None::<()>,
            ));
        }

        // Add to mempool
        let mut pool = self.state.tx_pool.write().await;
        match pool.add(tx.clone()) {
            Ok(hash) => {
                let pool_size = pool.len();
                drop(pool);
                println!("✅ Transaction added to pool! Hash: {}, Pool size: {}", hex::encode(hash.as_bytes()), pool_size);
                Ok(hex::encode(hash.as_bytes()))
            }
            Err(e) => {
                drop(pool);
                println!("❌ Failed to add transaction to pool: {}", e);
                Err(ErrorObjectOwned::owned(
                    INVALID_PARAMS,
                    format!("Failed to add transaction to pool: {}", e),
                    None::<()>,
                ))
            }
        }
    }

    async fn get_transaction_status(&self, tx_hash: String) -> RpcResult<TransactionStatus> {
        Self::validate_str_len(&tx_hash, 256, "tx_hash")?;
        let hash = self.parse_hash(&tx_hash)?;

        // Check if transaction is in mempool (pending)
        let pool = self.state.tx_pool.read().await;
        if pool.contains(&hash) {
            return Ok(TransactionStatus {
                tx_hash: tx_hash.clone(),
                status: "pending".to_string(),
                block_height: None,
            });
        }
        drop(pool);

        // TODO: Check blockchain for confirmed transactions
        // For now, if not in mempool, return unknown
        Ok(TransactionStatus {
            tx_hash: tx_hash.clone(),
            status: "unknown".to_string(),
            block_height: None,
        })
    }

    async fn get_block(&self, height: u64) -> RpcResult<Option<Block>> {
        // height is a u64 — range is enforced by the type system
        self.state
            .blockchain
            .get_block_by_height(height)
            .map_err(|e| Self::internal_error(e))
    }

    async fn get_latest_block(&self) -> RpcResult<Option<Block>> {
        let best_height = *self.state.best_height.read().await;
        self.state
            .blockchain
            .get_block_by_height(best_height)
            .map_err(|e| Self::internal_error(e))
    }

    async fn get_block_header(&self, height: u64) -> RpcResult<Option<BlockHeader>> {
        self.state
            .blockchain
            .get_header_by_height(height)
            .map_err(|e| Self::internal_error(e))
    }

    async fn get_chain_info(&self) -> RpcResult<ChainInfo> {
        let best_height = *self.state.best_height.read().await;
        let best_hash = *self.state.best_hash.read().await;
        let peer_count = *self.state.peer_count.read().await;

        // Calculate cumulative work from chain (sum of work scores)
        let total_work = self.state.blockchain.calculate_chain_work(best_height)
            .unwrap_or(0);

        // Check if syncing (simplified: syncing if we have peers but recent blocks are slow)
        let is_syncing = *self.state.is_syncing.read().await;

        Ok(ChainInfo {
            chain_id: self.state.chain_id.clone(),
            best_height,
            best_hash: hex::encode(best_hash.as_bytes()),
            genesis_hash: hex::encode(self.state.genesis_hash.as_bytes()),
            peer_count,
            total_work,
            is_syncing,
        })
    }

    async fn get_mining_work(&self) -> RpcResult<MiningWork> {
        let provider = self.state.mining_work_provider.as_ref().ok_or_else(|| {
            ErrorObjectOwned::owned(
                NOT_FOUND,
                "Mining work not available on this node (mining disabled)",
                None::<()>,
            )
        })?;
        provider()
            .await
            .map_err(|e| ErrorObjectOwned::owned(INTERNAL_ERROR, e, None::<()>))
    }

    async fn get_open_problems(&self) -> RpcResult<Vec<ProblemInfo>> {
        let problems = self.state.marketplace_state.get_open_problems()
            .map_err(|e| Self::internal_error(e))?;
        Ok(problems.iter().map(|p| self.problem_to_info(p)).collect())
    }

    async fn get_problem(&self, problem_id: String) -> RpcResult<Option<ProblemInfo>> {
        Self::validate_str_len(&problem_id, 256, "problem_id")?;
        let hash = self.parse_hash(&problem_id)?;
        let problem = self.state.marketplace_state.get_problem(&hash)
            .map_err(|e| Self::internal_error(e))?;
        Ok(problem.map(|p| self.problem_to_info(&p)))
    }

    async fn get_marketplace_stats(&self) -> RpcResult<MarketplaceStats> {
        self.state.marketplace_state.get_stats()
            .map_err(|e| Self::internal_error(e))
    }

    async fn submit_private_problem(&self, params: PrivateProblemParams) -> RpcResult<String> {
        // Input validation
        Self::validate_str_len(&params.commitment, 256, "commitment")?;
        Self::validate_str_len(&params.proof_bytes, 1024 * 1024, "proof_bytes")?;
        Self::validate_str_len(&params.vk_hash, 256, "vk_hash")?;
        Self::validate_str_len(&params.problem_type, 256, "problem_type")?;
        if params.size > 100_000 {
            return Err(ErrorObjectOwned::owned(INVALID_PARAMS, "size exceeds maximum of 100000", None::<()>));
        }
        if params.complexity_estimate < 0.0 || params.complexity_estimate > 1e15 {
            return Err(ErrorObjectOwned::owned(INVALID_PARAMS, "complexity_estimate out of range", None::<()>));
        }
        if params.expiration_days == 0 || params.expiration_days > 365 {
            return Err(ErrorObjectOwned::owned(INVALID_PARAMS, "expiration_days must be 1-365", None::<()>));
        }

        // Parse commitment hash
        let commitment = self.parse_hash(&params.commitment)?;

        // Parse proof bytes
        let proof_bytes = hex::decode(params.proof_bytes.trim_start_matches("0x"))
            .map_err(|e| ErrorObjectOwned::owned(INVALID_PARAMS, e.to_string(), None::<()>))?;

        // Parse VK hash
        let vk_hash = self.parse_hash(&params.vk_hash)?;

        // Parse public inputs
        let mut public_inputs = Vec::new();
        for input_hex in params.public_inputs {
            let input_bytes = hex::decode(input_hex.trim_start_matches("0x"))
                .map_err(|e| ErrorObjectOwned::owned(INVALID_PARAMS, e.to_string(), None::<()>))?;
            public_inputs.push(input_bytes);
        }

        // Construct ZK proof
        let zk_proof = WellformednessProof {
            proof_bytes,
            vk_hash,
            public_inputs,
        };

        // Construct public parameters
        let public_params = ProblemParameters {
            problem_type: params.problem_type,
            size: params.size,
            complexity_estimate: params.complexity_estimate,
        };

        // Construct private submission mode
        let submission_mode = SubmissionMode::Private {
            problem_commitment: commitment,
            zk_wellformed_proof: zk_proof,
            public_params,
        };

        // Submit to marketplace state (using placeholder address - in production this would come from authenticated session)
        let submitter = Address::from_bytes([0u8; 32]); // TODO: Get from authenticated user session

        let problem_id = self.state.marketplace_state.submit_problem(
            submission_mode,
            submitter,
            params.bounty,
            params.min_work_score,
            params.expiration_days,
        )
        .map_err(|e| Self::internal_error(e))?;

        Ok(hex::encode(problem_id.as_bytes()))
    }

    async fn reveal_problem(&self, params: RevealParams) -> RpcResult<bool> {
        // Input validation
        Self::validate_str_len(&params.problem_id, 256, "problem_id")?;
        Self::validate_str_len(&params.salt, 256, "salt")?;
        // problem JSON limit: 1 MB
        if params.problem.len() > 1024 * 1024 {
            return Err(ErrorObjectOwned::owned(INVALID_PARAMS, "problem JSON exceeds 1 MB", None::<()>));
        }

        // Parse problem ID
        let problem_id = self.parse_hash(&params.problem_id)?;

        // Parse problem (deserialize from JSON)
        let problem: ProblemType = serde_json::from_str(&params.problem)
            .map_err(|e| ErrorObjectOwned::owned(INVALID_PARAMS, format!("Invalid problem JSON: {}", e), None::<()>))?;

        // Parse salt
        let salt_bytes = hex::decode(params.salt.trim_start_matches("0x"))
            .map_err(|e| ErrorObjectOwned::owned(INVALID_PARAMS, e.to_string(), None::<()>))?;

        if salt_bytes.len() != 32 {
            return Err(ErrorObjectOwned::owned(
                INVALID_PARAMS,
                "Salt must be 32 bytes",
                None::<()>,
            ));
        }

        let mut salt = [0u8; 32];
        salt.copy_from_slice(&salt_bytes);

        // Create reveal
        let reveal = ProblemReveal::new(problem, salt);

        // Submit reveal to marketplace state
        self.state.marketplace_state.reveal_problem(problem_id, reveal)
            .map_err(|e| Self::internal_error(e))?;

        Ok(true)
    }

    async fn submit_public_subset_sum(&self, params: PublicSubsetSumParams) -> RpcResult<String> {
        // Input validation
        Self::validate_str_len(&params.submitter, 256, "submitter")?;
        if params.numbers.len() > 10_000 {
            return Err(ErrorObjectOwned::owned(INVALID_PARAMS, "numbers array exceeds maximum of 10000 elements", None::<()>));
        }

        // Parse submitter address
        let submitter = self.parse_address(&params.submitter)?;

        // Validate parameters
        if params.numbers.is_empty() {
            return Err(ErrorObjectOwned::owned(
                INVALID_PARAMS,
                "Numbers array cannot be empty",
                None::<()>,
            ));
        }
        if params.numbers.len() > 1000 {
            return Err(ErrorObjectOwned::owned(
                INVALID_PARAMS,
                "Numbers array too large (max 1000)",
                None::<()>,
            ));
        }
        if params.bounty == 0 {
            return Err(ErrorObjectOwned::owned(
                INVALID_PARAMS,
                "Bounty must be greater than 0",
                None::<()>,
            ));
        }
        if params.expiration_days == 0 || params.expiration_days > 365 {
            return Err(ErrorObjectOwned::owned(
                INVALID_PARAMS,
                "Expiration must be 1-365 days",
                None::<()>,
            ));
        }

        // Check submitter has sufficient balance for bounty
        let balance = self.state.account_state.get_balance(&submitter);
        if balance < params.bounty {
            return Err(ErrorObjectOwned::owned(
                INVALID_PARAMS,
                format!("Insufficient balance: have {}, need {}", balance, params.bounty),
                None::<()>,
            ));
        }

        // Save bounty before params fields are moved
        let bounty = params.bounty;

        // Create SubsetSum problem
        let problem = ProblemType::SubsetSum {
            numbers: params.numbers,
            target: params.target,
        };

        // Submit to marketplace state
        let problem_id = self.state.marketplace_state.submit_public_problem(
            problem,
            submitter,
            bounty,
            params.min_work_score,
            params.expiration_days,
        )
        .map_err(|e| Self::internal_error(e))?;

        // Deduct bounty from submitter's balance (escrow)
        let new_balance = balance - bounty;
        self.state.account_state.set_balance(&submitter, new_balance)
            .map_err(|e| Self::internal_error(e))?;

        tracing::info!(problem_id = %hex::encode(problem_id.as_bytes()), bounty, "subset_sum_submitted");

        Ok(hex::encode(problem_id.as_bytes()))
    }

    async fn submit_solution(&self, params: SolutionSubmissionParams) -> RpcResult<bool> {
        // Input validation
        Self::validate_str_len(&params.solver, 256, "solver")?;
        Self::validate_str_len(&params.problem_id, 256, "problem_id")?;
        if params.selected_indices.len() > 10_000 {
            return Err(ErrorObjectOwned::owned(INVALID_PARAMS, "selected_indices exceeds 10000 elements", None::<()>));
        }

        // Parse solver address
        let solver = self.parse_address(&params.solver)?;

        // Parse problem ID
        let problem_id = self.parse_hash(&params.problem_id)?;

        // Create solution
        let solution = coinject_core::Solution::SubsetSum(params.selected_indices);

        // Submit solution to marketplace state (validates and updates status)
        self.state.marketplace_state.submit_solution(problem_id, solver, solution)
            .map_err(|e| Self::internal_error(e))?;

        // Claim bounty and credit solver
        let (solver_addr, bounty) = self.state.marketplace_state.claim_bounty(problem_id)
            .map_err(|e| Self::internal_error(e))?;

        // Credit solver's account with bounty
        let current_balance = self.state.account_state.get_balance(&solver_addr);
        let new_balance = current_balance + bounty;
        self.state.account_state.set_balance(&solver_addr, new_balance)
            .map_err(|e| Self::internal_error(e))?;

        tracing::info!(solver = %hex::encode(solver_addr.as_bytes()), bounty, "solution_accepted");

        Ok(true)
    }

    async fn get_timelocks_by_recipient(&self, recipient: String) -> RpcResult<Vec<TimeLockInfo>> {
        Self::validate_str_len(&recipient, 256, "recipient")?;
        let addr = self.parse_address(&recipient)?;
        let timelocks = self.state.timelock_state.get_timelocks_for_recipient(&addr);
        Ok(timelocks.into_iter().map(|tl| self.timelock_to_info(&tl)).collect())
    }

    async fn get_unlocked_timelocks(&self) -> RpcResult<Vec<TimeLockInfo>> {
        let timelocks = self.state.timelock_state.get_unlocked_timelocks();
        Ok(timelocks.into_iter().map(|tl| self.timelock_to_info(&tl)).collect())
    }

    async fn get_escrows_by_sender(&self, sender: String) -> RpcResult<Vec<EscrowInfo>> {
        Self::validate_str_len(&sender, 256, "sender")?;
        let addr = self.parse_address(&sender)?;
        let escrows = self.state.escrow_state.get_escrows_by_sender(&addr);
        Ok(escrows.into_iter().map(|e| self.escrow_to_info(&e)).collect())
    }

    async fn get_escrows_by_recipient(&self, recipient: String) -> RpcResult<Vec<EscrowInfo>> {
        Self::validate_str_len(&recipient, 256, "recipient")?;
        let addr = self.parse_address(&recipient)?;
        let escrows = self.state.escrow_state.get_escrows_by_recipient(&addr);
        Ok(escrows.into_iter().map(|e| self.escrow_to_info(&e)).collect())
    }

    async fn get_active_escrows(&self) -> RpcResult<Vec<EscrowInfo>> {
        let escrows = self.state.escrow_state.get_active_escrows();
        Ok(escrows.into_iter().map(|e| self.escrow_to_info(&e)).collect())
    }

    async fn get_channels_by_address(&self, address: String) -> RpcResult<Vec<ChannelInfo>> {
        Self::validate_str_len(&address, 256, "address")?;
        let addr = self.parse_address(&address)?;
        let channels = self.state.channel_state.get_channels_for_address(&addr);
        Ok(channels.into_iter().map(|c| self.channel_to_info(&c)).collect())
    }

    async fn get_open_channels(&self) -> RpcResult<Vec<ChannelInfo>> {
        let channels = self.state.channel_state.get_open_channels();
        Ok(channels.into_iter().map(|c| self.channel_to_info(&c)).collect())
    }

    async fn get_disputed_channels(&self) -> RpcResult<Vec<ChannelInfo>> {
        let channels = self.state.channel_state.get_disputed_channels();
        Ok(channels.into_iter().map(|c| self.channel_to_info(&c)).collect())
    }

    async fn faucet_request_tokens(&self, address: String) -> RpcResult<FaucetResponse> {
        Self::validate_str_len(&address, 256, "address")?;
        // Check if faucet is enabled
        let faucet_handler = match &self.state.faucet_handler {
            Some(handler) => handler,
            None => {
                return Ok(FaucetResponse {
                    success: false,
                    amount: None,
                    new_balance: None,
                    message: "Faucet is not enabled on this node. Use --enable-faucet flag to enable.".to_string(),
                    cooldown_remaining: None,
                });
            }
        };

        // Parse address
        let addr = self.parse_address(&address)?;

        // Call faucet handler
        match faucet_handler(&addr) {
            Ok(amount) => {
                // Credit the account by adding to current balance
                let current_balance = self.state.account_state.get_balance(&addr);
                let new_balance = current_balance + amount;

                if let Err(e) = self.state.account_state.set_balance(&addr, new_balance) {
                    return Ok(FaucetResponse {
                        success: false,
                        amount: None,
                        new_balance: None,
                        message: format!("Failed to credit account: {}", e),
                        cooldown_remaining: None,
                    });
                }

                Ok(FaucetResponse {
                    success: true,
                    amount: Some(amount),
                    new_balance: Some(new_balance),
                    message: format!("Successfully credited {} tokens to your account!", amount),
                    cooldown_remaining: None,
                })
            }
            Err(error_msg) => {
                // Parse cooldown from error message if present
                let cooldown_remaining = if error_msg.contains("Try again in") {
                    error_msg
                        .split("Try again in ")
                        .nth(1)
                        .and_then(|s| s.split(" seconds").next())
                        .and_then(|s| s.parse::<u64>().ok())
                } else {
                    None
                };

                Ok(FaucetResponse {
                    success: false,
                    amount: None,
                    new_balance: None,
                    message: error_msg,
                    cooldown_remaining,
                })
            }
        }
    }

    async fn get_network_info(&self) -> RpcResult<NetworkInfo> {
        let peer_count = *self.state.peer_count.read().await;
        let listen_addresses = self.state.listen_addresses.read().await.clone();
        
        let peer_id = self.state.local_peer_id.clone().unwrap_or_else(|| "unknown".to_string());
        
        // Generate a bootnode address hint for operators
        let bootnode_hint = if !listen_addresses.is_empty() {
            format!("{}/p2p/{}", listen_addresses[0], peer_id)
        } else {
            format!("/ip4/<YOUR_IP>/tcp/30333/p2p/{}", peer_id)
        };

        Ok(NetworkInfo {
            peer_id,
            peer_count,
            listen_addresses,
            bootnode_address_hint: bootnode_hint,
        })
    }

    async fn submit_block(&self, block: Block) -> RpcResult<String> {
        println!("📥 RPC: Received block submission for height {}", block.header.height);
        
        let handler = self.state.block_submission_handler.as_ref()
            .ok_or_else(|| {
                ErrorObjectOwned::owned(
                    INTERNAL_ERROR,
                    "Block submission not enabled on this node",
                    None::<()>,
                )
            })?;

        match handler(block) {
            Ok(block_hash) => {
                tracing::info!(hash = %block_hash, "block_submitted");
                Ok(block_hash)
            }
            Err(e) => Err(Self::internal_error(format!("block submission: {}", e))),
        }
    }
}

/// RPC server handle
pub struct RpcServer {
    handle: ServerHandle,
    addr: SocketAddr,
    /// Optional TLS termination proxy task (aborted on stop)
    tls_task: Option<tokio::task::JoinHandle<()>>,
}

impl RpcServer {
    /// Create and start a new RPC server with the full Phase-2 security stack.
    ///
    /// Middleware order (outer → inner):
    ///   CORS → AuditLog → Timeout(30s) → SecurityGate → jsonrpsee
    ///
    /// If `RPC_TLS_CERT` and `RPC_TLS_KEY` env vars are set a TLS termination
    /// proxy is also spawned on `RPC_TLS_BIND` (default: same IP, port+1).
    pub async fn new(
        listen_addr: SocketAddr,
        state: Arc<RpcServerState>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
            .expose_headers(Any)
            .max_age(Duration::from_secs(86400));

        let timeout_secs: u64 = std::env::var("RPC_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);

        // CORS is outermost so OPTIONS preflight is handled before AuditLog, Timeout, or
        // SecurityGate (rate limits / auth). Browsers send preflight before the JSON-RPC POST.
        let middleware = ServiceBuilder::new()
            .layer(cors)
            .layer(AuditLogLayer)
            .layer(TimeoutLayer::new(Duration::from_secs(timeout_secs)))
            .layer(SecurityGateLayer::new(SecurityConfig::default()));

        let server = Server::builder()
            .set_http_middleware(middleware)
            .build(listen_addr)
            .await?;
        let addr = server.local_addr()?;

        let rpc_impl = RpcServerImpl::new(state);
        let handle = server.start(rpc_impl.into_rpc());

        tracing::info!(addr = %addr, timeout_secs, "rpc.server.started");

        // Optional TLS termination proxy
        let tls_task = TlsConfig::from_env(addr).map(|tls_cfg| {
            let tls_bind = tls_cfg.bind_addr;
            tracing::info!(tls_bind = %tls_bind, backend = %addr, "rpc.tls.proxy.spawning");
            tokio::spawn(async move {
                if let Err(e) = crate::tls::run_tls_proxy(tls_cfg, addr).await {
                    tracing::error!(error = %e, "rpc.tls.proxy.error");
                }
            })
        });

        Ok(RpcServer { handle, addr, tls_task })
    }

    /// Get the listening address
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// Stop the server and any TLS proxy task
    pub fn stop(self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(t) = self.tls_task {
            t.abort();
        }
        self.handle.stop()?;
        Ok(())
    }

    /// Wait for the server to finish
    pub async fn stopped(self) {
        self.handle.stopped().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock blockchain reader for tests
    struct MockBlockchainReader;

    impl BlockchainReader for MockBlockchainReader {
        fn get_block_by_height(&self, _height: u64) -> Result<Option<Block>, String> {
            Ok(None)
        }

        fn get_block_by_hash(&self, _hash: &Hash) -> Result<Option<Block>, String> {
            Ok(None)
        }

        fn get_header_by_height(&self, _height: u64) -> Result<Option<BlockHeader>, String> {
            Ok(None)
        }
    }

    #[test]
    fn test_address_parsing() {
        let temp_dir = std::env::temp_dir().join("coinject-rpc-test-addr");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Create test database for state objects
        let db_path = temp_dir.join("test.db");
        let db = Arc::new(redb::Database::create(&db_path).unwrap());
        let account_db_path = temp_dir.join("accounts.db");

        let state = Arc::new(RpcServerState {
            account_state: Arc::new(AccountState::new(&account_db_path).unwrap()),
            timelock_state: Arc::new(TimeLockState::new(db.clone()).unwrap()),
            escrow_state: Arc::new(EscrowState::new(db.clone()).unwrap()),
            channel_state: Arc::new(ChannelState::new(db.clone()).unwrap()),
            marketplace_state: Arc::new(MarketplaceState::from_db(db.clone()).unwrap()),
            blockchain: Arc::new(MockBlockchainReader) as Arc<dyn BlockchainReader>,
            marketplace: Arc::new(RwLock::new(ProblemMarketplace::new())),
            tx_pool: Arc::new(RwLock::new(TransactionPool::new())),
            chain_id: "test".to_string(),
            best_height: Arc::new(RwLock::new(0)),
            best_hash: Arc::new(RwLock::new(Hash::ZERO)),
            genesis_hash: Hash::ZERO,
            peer_count: Arc::new(RwLock::new(0)),
            faucet_handler: None,
            block_submission_handler: None,
            local_peer_id: Some("test-peer-id".to_string()),
            listen_addresses: Arc::new(RwLock::new(vec![])),
            is_syncing: Arc::new(RwLock::new(false)),
            mining_work_provider: None,
        });

        let rpc = RpcServerImpl::new(state);

        // Valid 32-byte hex address
        let addr_hex = "0x0000000000000000000000000000000000000000000000000000000000000001";
        let result = rpc.parse_address(addr_hex);
        assert!(result.is_ok());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_hash_parsing() {
        let temp_dir = std::env::temp_dir().join("coinject-rpc-test-hash");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Create test database for state objects
        let db_path = temp_dir.join("test.db");
        let db = Arc::new(redb::Database::create(&db_path).unwrap());
        let account_db_path = temp_dir.join("accounts.db");

        let state = Arc::new(RpcServerState {
            account_state: Arc::new(AccountState::new(&account_db_path).unwrap()),
            timelock_state: Arc::new(TimeLockState::new(db.clone()).unwrap()),
            escrow_state: Arc::new(EscrowState::new(db.clone()).unwrap()),
            channel_state: Arc::new(ChannelState::new(db.clone()).unwrap()),
            marketplace_state: Arc::new(MarketplaceState::from_db(db.clone()).unwrap()),
            blockchain: Arc::new(MockBlockchainReader) as Arc<dyn BlockchainReader>,
            marketplace: Arc::new(RwLock::new(ProblemMarketplace::new())),
            tx_pool: Arc::new(RwLock::new(TransactionPool::new())),
            chain_id: "test".to_string(),
            best_height: Arc::new(RwLock::new(0)),
            best_hash: Arc::new(RwLock::new(Hash::ZERO)),
            genesis_hash: Hash::ZERO,
            peer_count: Arc::new(RwLock::new(0)),
            faucet_handler: None,
            block_submission_handler: None,
            local_peer_id: Some("test-peer-id".to_string()),
            listen_addresses: Arc::new(RwLock::new(vec![])),
            is_syncing: Arc::new(RwLock::new(false)),
            mining_work_provider: None,
        });

        let rpc = RpcServerImpl::new(state);

        // Valid 32-byte hex hash
        let hash_hex = "0x0000000000000000000000000000000000000000000000000000000000000001";
        let result = rpc.parse_hash(hash_hex);
        assert!(result.is_ok());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
