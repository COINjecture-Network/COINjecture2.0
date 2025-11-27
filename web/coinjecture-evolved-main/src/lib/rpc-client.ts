/**
 * RPC Client for COINjecture Network B
 * Connects to JSON-RPC endpoints for blockchain operations
 * Matches the actual Rust RPC server implementation in rpc/src/server.rs
 */

const DEFAULT_RPC_URL = import.meta.env.VITE_RPC_URL || 'http://localhost:9933';

export interface RpcError {
  code: number;
  message: string;
  data?: unknown;
}

export interface RpcResponse<T> {
  jsonrpc: '2.0';
  id: number;
  result?: T;
  error?: RpcError;
}

// Problem marketplace response - matches ProblemInfo in rpc/src/server.rs
export interface ProblemInfo {
  problem_id: string;
  submitter: string;
  bounty: number; // Balance type
  min_work_score: number;
  status: string; // "OPEN", "SOLVED", "EXPIRED", "CANCELLED"
  submitted_at: number; // i64 timestamp
  expires_at: number; // i64 timestamp
  is_private: boolean;
  problem_type: string | null; // e.g., "SubsetSum(5)", "SAT(vars=10, clauses=20)"
  problem_size: number | null; // usize
  is_revealed: boolean;
}

// Marketplace statistics - matches MarketplaceStats in state/src/marketplace.rs
export interface MarketplaceStats {
  total_problems: number; // usize
  open_problems: number; // usize
  solved_problems: number; // usize
  expired_problems: number; // usize
  cancelled_problems: number; // usize
  total_bounty_pool: number; // Balance
}

// Chain information - matches ChainInfo in rpc/src/server.rs
export interface ChainInfo {
  chain_id: string;
  best_height: number; // u64
  best_hash: string; // hex-encoded
  genesis_hash: string; // hex-encoded
  peer_count: number; // usize
}

// Account information - matches AccountInfo in rpc/src/server.rs
export interface AccountInfo {
  address: string;
  balance: number; // Balance
  nonce: number; // u64
}

// Transaction status - matches TransactionStatus in rpc/src/server.rs
export interface TransactionStatus {
  tx_hash: string;
  status: string; // "pending", "confirmed", "failed", "unknown"
  block_height: number | null; // Option<u64>
}

// Block structure - matches Block in core/src/block.rs
export interface Block {
  header: {
    version: number;
    height: number;
    prev_hash: string;
    timestamp: number;
    transactions_root: string;
    solutions_root: string;
    commitment: {
      hash: string;
      problem_hash: string;
    };
    work_score: number;
    miner: string; // Address as hex string
    nonce: number;
    solve_time_us: number;
    verify_time_us: number;
    time_asymmetry_ratio: number;
    solution_quality: number;
    complexity_weight: number;
    energy_estimate_joules: number;
  };
  coinbase: unknown;
  transactions: unknown[];
  solution_reveal: {
    problem: ProblemType;
    solution: SolutionType;
    commitment: {
      hash: string;
      problem_hash: string;
    };
  };
}

// Problem type from solution_reveal
export interface ProblemType {
  SubsetSum?: { numbers: number[]; target: number };
  SAT?: { variables: number; clauses: any[] };
  TSP?: { cities: number; distances: number[][] };
  Custom?: { problem_id: string; data: string };
}

// Solution type from solution_reveal
export interface SolutionType {
  SubsetSum?: number[];
  SAT?: boolean[];
  TSP?: number[];
  Custom?: string;
}

// Block header
export interface BlockHeader {
  height: number;
  previous_hash: string;
  merkle_root: string;
  timestamp: number;
  difficulty: number;
  work_score: number;
}

// TimeLock information - matches TimeLockInfo in rpc/src/server.rs
export interface TimeLockInfo {
  tx_hash: string;
  from: string;
  recipient: string;
  amount: number; // Balance
  unlock_time: number; // i64 timestamp
  created_at_height: number; // u64
}

// Escrow information - matches EscrowInfo in rpc/src/server.rs
export interface EscrowInfo {
  escrow_id: string;
  sender: string;
  recipient: string;
  arbiter: string | null;
  amount: number; // Balance
  timeout: number; // i64 timestamp
  conditions_hash: string;
  status: string;
  created_at_height: number; // u64
  resolved_at_height: number | null; // Option<u64>
}

// Channel information - matches ChannelInfo in rpc/src/server.rs
export interface ChannelInfo {
  channel_id: string;
  participant_a: string;
  participant_b: string;
  deposit_a: number; // Balance
  deposit_b: number; // Balance
  balance_a: number; // Balance
  balance_b: number; // Balance
  sequence: number; // u64
  dispute_timeout: number; // i64 timestamp
  status: string;
  opened_at_height: number; // u64
  closed_at_height: number | null; // Option<u64>
}

// Faucet response - matches FaucetResponse in rpc/src/server.rs
export interface FaucetResponse {
  success: boolean;
  amount: number | null; // Option<Balance>
  new_balance: number | null; // Option<Balance>
  message: string;
  cooldown_remaining: number | null; // Option<u64>
}

// Private problem submission parameters - matches PrivateProblemParams in rpc/src/server.rs
export interface PrivateProblemParams {
  commitment: string; // hex-encoded hash
  proof_bytes: string; // hex-encoded
  vk_hash: string; // hex-encoded hash
  public_inputs: string[]; // array of hex-encoded bytes
  problem_type: string;
  size: number; // usize
  complexity_estimate: number; // f64
  bounty: number; // Balance
  min_work_score: number; // f64
  expiration_days: number; // u64
}

// Problem reveal parameters - matches RevealParams in rpc/src/server.rs
export interface RevealParams {
  problem_id: string; // hex-encoded hash
  problem: string; // JSON-encoded ProblemType
  salt: string; // hex-encoded 32-byte salt
}

export class RpcClient {
  private baseUrl: string;
  private requestId: number = 1;

  constructor(baseUrl: string = DEFAULT_RPC_URL) {
    this.baseUrl = baseUrl;
  }

  private async call<T>(method: string, params: unknown[] = []): Promise<T> {
    try {
      const response = await fetch(this.baseUrl, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          jsonrpc: '2.0',
          id: this.requestId++,
          method,
          params,
        }),
      });

      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }

      const data: RpcResponse<T> = await response.json();

      if (data.error) {
        throw new Error(data.error.message || 'RPC error');
      }

      if (data.result === undefined) {
        throw new Error('No result in RPC response');
      }

      return data.result;
    } catch (error: any) {
      // Handle connection errors gracefully
      if (error.message?.includes('ERR_CONNECTION_REFUSED') || 
          error.message?.includes('Failed to fetch') ||
          error.name === 'TypeError') {
        throw new Error(`Cannot connect to RPC server at ${this.baseUrl}. Make sure the node is running.`);
      }
      throw error;
    }
  }

  // ========== Account Methods ==========
  
  async getBalance(address: string): Promise<number> {
    return this.call<number>('account_getBalance', [address]);
  }

  async getNonce(address: string): Promise<number> {
    return this.call<number>('account_getNonce', [address]);
  }

  async getAccountInfo(address: string): Promise<AccountInfo> {
    return this.call<AccountInfo>('account_getInfo', [address]);
  }

  // ========== Chain Methods ==========
  
  async getBlock(height: number): Promise<Block | null> {
    return this.call<Block | null>('chain_getBlock', [height]);
  }

  async getLatestBlock(): Promise<Block | null> {
    return this.call<Block | null>('chain_getLatestBlock', []);
  }

  async getBlockHeader(height: number): Promise<BlockHeader | null> {
    return this.call<BlockHeader | null>('chain_getBlockHeader', [height]);
  }

  async getChainInfo(): Promise<ChainInfo> {
    return this.call<ChainInfo>('chain_getInfo', []);
  }

  // ========== Transaction Methods ==========
  
  async submitTransaction(txHex: string): Promise<string> {
    return this.call<string>('transaction_submit', [txHex]);
  }

  async getTransactionStatus(txHash: string): Promise<TransactionStatus> {
    return this.call<TransactionStatus>('transaction_getStatus', [txHash]);
  }

  // ========== Marketplace Methods ==========
  
  async getOpenProblems(): Promise<ProblemInfo[]> {
    return this.call<ProblemInfo[]>('marketplace_getOpenProblems', []);
  }

  async getProblem(problemId: string): Promise<ProblemInfo | null> {
    return this.call<ProblemInfo | null>('marketplace_getProblem', [problemId]);
  }

  async getMarketplaceStats(): Promise<MarketplaceStats> {
    return this.call<MarketplaceStats>('marketplace_getStats', []);
  }

  async submitPrivateProblem(params: PrivateProblemParams): Promise<string> {
    return this.call<string>('marketplace_submitPrivateProblem', [params]);
  }

  async revealProblem(params: RevealParams): Promise<boolean> {
    return this.call<boolean>('marketplace_revealProblem', [params]);
  }

  // ========== TimeLock Methods ==========
  
  async getTimelocksByRecipient(recipient: string): Promise<TimeLockInfo[]> {
    return this.call<TimeLockInfo[]>('timelock_getByRecipient', [recipient]);
  }

  async getUnlockedTimelocks(): Promise<TimeLockInfo[]> {
    return this.call<TimeLockInfo[]>('timelock_getUnlocked', []);
  }

  // ========== Escrow Methods ==========
  
  async getEscrowsBySender(sender: string): Promise<EscrowInfo[]> {
    return this.call<EscrowInfo[]>('escrow_getBySender', [sender]);
  }

  async getEscrowsByRecipient(recipient: string): Promise<EscrowInfo[]> {
    return this.call<EscrowInfo[]>('escrow_getByRecipient', [recipient]);
  }

  async getActiveEscrows(): Promise<EscrowInfo[]> {
    return this.call<EscrowInfo[]>('escrow_getActive', []);
  }

  // ========== Channel Methods ==========
  
  async getChannelsByAddress(address: string): Promise<ChannelInfo[]> {
    return this.call<ChannelInfo[]>('channel_getByAddress', [address]);
  }

  async getOpenChannels(): Promise<ChannelInfo[]> {
    return this.call<ChannelInfo[]>('channel_getOpen', []);
  }

  async getDisputedChannels(): Promise<ChannelInfo[]> {
    return this.call<ChannelInfo[]>('channel_getDisputed', []);
  }

  // ========== Faucet Methods ==========
  
  async faucetRequestTokens(address: string): Promise<FaucetResponse> {
    return this.call<FaucetResponse>('faucet_requestTokens', [address]);
  }
}

// Singleton instance
export const rpcClient = new RpcClient();
