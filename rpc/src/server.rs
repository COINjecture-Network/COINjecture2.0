// JSON-RPC Server for COINjecture Network B
// Provides wallet and client API access

use coinject_core::{Address, Balance, Block, BlockHeader, Hash, Transaction};
use coinject_mempool::{MarketplaceStats, ProblemSubmission, ProblemMarketplace, TransactionPool};
use coinject_state::{
    AccountState, TimeLockState, TimeLock, EscrowState, Escrow,
    ChannelState, Channel
};
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    server::{Server, ServerHandle},
    types::ErrorObjectOwned,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Trait for reading blockchain data (allows node to provide chain state without circular dependency)
pub trait BlockchainReader: Send + Sync {
    fn get_block_by_height(&self, height: u64) -> Result<Option<Block>, String>;
    fn get_block_by_hash(&self, hash: &Hash) -> Result<Option<Block>, String>;
    fn get_header_by_height(&self, height: u64) -> Result<Option<BlockHeader>, String>;
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

    /// Get open problems from marketplace
    #[method(name = "marketplace_getOpenProblems")]
    async fn get_open_problems(&self) -> RpcResult<Vec<ProblemInfo>>;

    /// Get problem by ID
    #[method(name = "marketplace_getProblem")]
    async fn get_problem(&self, problem_id: String) -> RpcResult<Option<ProblemInfo>>;

    /// Get marketplace statistics
    #[method(name = "marketplace_getStats")]
    async fn get_marketplace_stats(&self) -> RpcResult<MarketplaceStats>;

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
}

/// RPC server state
pub struct RpcServerState {
    pub account_state: Arc<AccountState>,
    pub timelock_state: Arc<TimeLockState>,
    pub escrow_state: Arc<EscrowState>,
    pub channel_state: Arc<ChannelState>,
    pub blockchain: Arc<dyn BlockchainReader>,
    pub marketplace: Arc<RwLock<ProblemMarketplace>>,
    pub tx_pool: Arc<RwLock<TransactionPool>>,
    pub chain_id: String,
    pub best_height: Arc<RwLock<u64>>,
    pub best_hash: Arc<RwLock<Hash>>,
    pub genesis_hash: Hash,
    pub peer_count: Arc<RwLock<usize>>,
}

/// RPC server implementation
pub struct RpcServerImpl {
    state: Arc<RpcServerState>,
}

impl RpcServerImpl {
    pub fn new(state: Arc<RpcServerState>) -> Self {
        RpcServerImpl { state }
    }

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
        ProblemInfo {
            problem_id: hex::encode(problem.problem_id.as_bytes()),
            submitter: hex::encode(problem.submitter.as_bytes()),
            bounty: problem.bounty,
            min_work_score: problem.min_work_score,
            status: format!("{:?}", problem.status),
            submitted_at: problem.submitted_at,
            expires_at: problem.expires_at,
        }
    }
}

#[async_trait]
impl CoinjectRpcServer for RpcServerImpl {
    async fn get_balance(&self, address: String) -> RpcResult<Balance> {
        let addr = self.parse_address(&address)?;
        Ok(self.state.account_state.get_balance(&addr))
    }

    async fn get_nonce(&self, address: String) -> RpcResult<u64> {
        let addr = self.parse_address(&address)?;
        Ok(self.state.account_state.get_nonce(&addr))
    }

    async fn get_account_info(&self, address: String) -> RpcResult<AccountInfo> {
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
        let tx_bytes = hex::decode(tx_hex.trim_start_matches("0x"))
            .map_err(|e| ErrorObjectOwned::owned(INVALID_PARAMS, e.to_string(), None::<()>))?;

        let tx: Transaction = bincode::deserialize(&tx_bytes)
            .map_err(|e| ErrorObjectOwned::owned(INVALID_PARAMS, e.to_string(), None::<()>))?;

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
        self.state
            .blockchain
            .get_block_by_height(height)
            .map_err(|e| ErrorObjectOwned::owned(INTERNAL_ERROR, e, None::<()>))
    }

    async fn get_latest_block(&self) -> RpcResult<Option<Block>> {
        let best_height = *self.state.best_height.read().await;
        self.state
            .blockchain
            .get_block_by_height(best_height)
            .map_err(|e| ErrorObjectOwned::owned(INTERNAL_ERROR, e, None::<()>))
    }

    async fn get_block_header(&self, height: u64) -> RpcResult<Option<BlockHeader>> {
        self.state
            .blockchain
            .get_header_by_height(height)
            .map_err(|e| ErrorObjectOwned::owned(INTERNAL_ERROR, e, None::<()>))
    }

    async fn get_chain_info(&self) -> RpcResult<ChainInfo> {
        let best_height = *self.state.best_height.read().await;
        let best_hash = *self.state.best_hash.read().await;
        let peer_count = *self.state.peer_count.read().await;

        Ok(ChainInfo {
            chain_id: self.state.chain_id.clone(),
            best_height,
            best_hash: hex::encode(best_hash.as_bytes()),
            genesis_hash: hex::encode(self.state.genesis_hash.as_bytes()),
            peer_count,
        })
    }

    async fn get_open_problems(&self) -> RpcResult<Vec<ProblemInfo>> {
        let marketplace = self.state.marketplace.read().await;
        let problems = marketplace.get_open_problems();

        Ok(problems.iter().map(|p| self.problem_to_info(p)).collect())
    }

    async fn get_problem(&self, problem_id: String) -> RpcResult<Option<ProblemInfo>> {
        let hash = self.parse_hash(&problem_id)?;
        let marketplace = self.state.marketplace.read().await;

        Ok(marketplace.get_problem(&hash).map(|p| self.problem_to_info(p)))
    }

    async fn get_marketplace_stats(&self) -> RpcResult<MarketplaceStats> {
        let marketplace = self.state.marketplace.read().await;
        Ok(marketplace.get_stats())
    }

    async fn get_timelocks_by_recipient(&self, recipient: String) -> RpcResult<Vec<TimeLockInfo>> {
        let addr = self.parse_address(&recipient)?;
        let timelocks = self.state.timelock_state.get_timelocks_for_recipient(&addr);
        Ok(timelocks.into_iter().map(|tl| self.timelock_to_info(&tl)).collect())
    }

    async fn get_unlocked_timelocks(&self) -> RpcResult<Vec<TimeLockInfo>> {
        let timelocks = self.state.timelock_state.get_unlocked_timelocks();
        Ok(timelocks.into_iter().map(|tl| self.timelock_to_info(&tl)).collect())
    }

    async fn get_escrows_by_sender(&self, sender: String) -> RpcResult<Vec<EscrowInfo>> {
        let addr = self.parse_address(&sender)?;
        let escrows = self.state.escrow_state.get_escrows_by_sender(&addr);
        Ok(escrows.into_iter().map(|e| self.escrow_to_info(&e)).collect())
    }

    async fn get_escrows_by_recipient(&self, recipient: String) -> RpcResult<Vec<EscrowInfo>> {
        let addr = self.parse_address(&recipient)?;
        let escrows = self.state.escrow_state.get_escrows_by_recipient(&addr);
        Ok(escrows.into_iter().map(|e| self.escrow_to_info(&e)).collect())
    }

    async fn get_active_escrows(&self) -> RpcResult<Vec<EscrowInfo>> {
        let escrows = self.state.escrow_state.get_active_escrows();
        Ok(escrows.into_iter().map(|e| self.escrow_to_info(&e)).collect())
    }

    async fn get_channels_by_address(&self, address: String) -> RpcResult<Vec<ChannelInfo>> {
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
}

/// RPC server handle
pub struct RpcServer {
    handle: ServerHandle,
    addr: SocketAddr,
}

impl RpcServer {
    /// Create and start new RPC server
    pub async fn new(
        listen_addr: SocketAddr,
        state: Arc<RpcServerState>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let server = Server::builder().build(listen_addr).await?;
        let addr = server.local_addr()?;

        let rpc_impl = RpcServerImpl::new(state);
        let handle = server.start(rpc_impl.into_rpc());

        println!("JSON-RPC server listening on {}", addr);

        Ok(RpcServer { handle, addr })
    }

    /// Get the listening address
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// Stop the server
    pub fn stop(self) -> Result<(), Box<dyn std::error::Error>> {
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

        let state = Arc::new(RpcServerState {
            account_state: Arc::new(AccountState::new(&temp_dir).unwrap()),
            blockchain: Arc::new(MockBlockchainReader) as Arc<dyn BlockchainReader>,
            marketplace: Arc::new(RwLock::new(ProblemMarketplace::new())),
            tx_pool: Arc::new(RwLock::new(TransactionPool::new())),
            chain_id: "test".to_string(),
            best_height: Arc::new(RwLock::new(0)),
            best_hash: Arc::new(RwLock::new(Hash::ZERO)),
            genesis_hash: Hash::ZERO,
            peer_count: Arc::new(RwLock::new(0)),
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

        let state = Arc::new(RpcServerState {
            account_state: Arc::new(AccountState::new(&temp_dir).unwrap()),
            blockchain: Arc::new(MockBlockchainReader) as Arc<dyn BlockchainReader>,
            marketplace: Arc::new(RwLock::new(ProblemMarketplace::new())),
            tx_pool: Arc::new(RwLock::new(TransactionPool::new())),
            chain_id: "test".to_string(),
            best_height: Arc::new(RwLock::new(0)),
            best_hash: Arc::new(RwLock::new(Hash::ZERO)),
            genesis_hash: Hash::ZERO,
            peer_count: Arc::new(RwLock::new(0)),
        });

        let rpc = RpcServerImpl::new(state);

        // Valid 32-byte hex hash
        let hash_hex = "0x0000000000000000000000000000000000000000000000000000000000000001";
        let result = rpc.parse_hash(hash_hex);
        assert!(result.is_ok());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
