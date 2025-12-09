/**
 * RPC Client for COINjecture Network B
 * Connects to JSON-RPC endpoints for blockchain operations
 * Matches the actual Rust RPC server implementation in rpc/src/server.rs
 * Supports multiple nodes with failover and parallel querying
 * 
 * Version: 1.0.1 (2025-12-03) - Cache-bust update
 */

import { hexToBytes } from '@noble/hashes/utils';

// Parse RPC URLs from environment variable (comma-separated)
const parseRpcUrls = (): string[] => {
  const envUrl = import.meta.env.VITE_RPC_URL || 'http://localhost:9933';
  // Support comma-separated list of URLs
  return envUrl.split(',').map(url => url.trim()).filter(url => url.length > 0);
};

// In development, use Vite proxy to avoid CORS issues
// In production (CloudFront), use relative /api/rpc path that CloudFront will proxy
// The RPC client can specify target node via query parameter or header
const isDevelopment = import.meta.env.DEV;
const isProduction = import.meta.env.PROD;
const isHTTPS = typeof window !== 'undefined' && window.location.protocol === 'https:';

// Parse RPC URLs and create proxy URLs for HTTPS
const createProxyUrls = (): string[] => {
  const urls = parseRpcUrls();
  if (isHTTPS && !isDevelopment) {
    // In production HTTPS, use HTTPS domains directly (CORS enabled on RPC servers)
    // Map HTTP IP addresses to HTTPS domains if needed
    const mappedUrls = urls.map(url => {
      // Map known IP addresses to HTTPS domains
      if (url.includes('143.110.139.166')) {
        return 'https://rpc1.coinjecture.com';
      }
      if (url.includes('68.183.205.12')) {
        return 'https://rpc2.coinjecture.com';
      }
      if (url.includes('35.184.253.150')) {
        return 'https://rpc3.coinjecture.com';
      }
      // If already HTTPS, use as-is
      if (url.startsWith('https://')) {
        return url;
      }
      // Warn if non-HTTPS URL detected in production
      console.warn('⚠️  Non-HTTPS URL detected in production:', url);
      console.warn('⚠️  Please update VITE_RPC_URL to use HTTPS domains');
      return url;
    });
    
    // Validate all URLs are HTTPS
    const invalidUrls = mappedUrls.filter(url => !url.startsWith('https://'));
    if (invalidUrls.length > 0) {
      console.warn('⚠️  Non-HTTPS URLs detected in production:', invalidUrls);
      console.warn('⚠️  Please update VITE_RPC_URL to use HTTPS domains');
    }
    
    return mappedUrls;
  }
  return urls;
};

const DEFAULT_RPC_URLS = isDevelopment 
  ? ['/api/rpc'] // Use Vite proxy in development
  : createProxyUrls(); // Use CloudFront /api/rpc proxy in production HTTPS

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
  private baseUrls: string[];
  private requestId: number = 1;
  private currentUrlIndex: number = 0;

  constructor(baseUrls: string[] = DEFAULT_RPC_URLS) {
    this.baseUrls = baseUrls.length > 0 ? baseUrls : ['http://localhost:9933'];
  }

  /**
   * Get the current active RPC URL (for round-robin)
   */
  private getCurrentUrl(): string {
    return this.baseUrls[this.currentUrlIndex % this.baseUrls.length];
  }

  /**
   * Rotate to the next RPC URL (round-robin)
   */
  private rotateUrl(): void {
    this.currentUrlIndex = (this.currentUrlIndex + 1) % this.baseUrls.length;
  }

  /**
   * Call RPC method with failover support
   * Tries each node in order until one succeeds
   */
  private async call<T>(method: string, params: unknown[] = []): Promise<T> {
    const errors: Error[] = [];
    
    // Try each URL in order (failover)
    for (let i = 0; i < this.baseUrls.length; i++) {
      const url = this.baseUrls[i];
      try {
        const response = await fetch(url, {
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

        // Success! Rotate to next URL for load balancing
        this.rotateUrl();
        return data.result;
      } catch (error: any) {
        // Store error and try next URL
        errors.push(error);
        // Continue to next URL
      }
    }

    // All URLs failed
    const errorMessages = errors.map(e => e.message).join('; ');
    throw new Error(`Cannot connect to any RPC server. Tried: ${this.baseUrls.join(', ')}. Errors: ${errorMessages}`);
  }

  /**
   * Call RPC method on all nodes in parallel and return the best result
   * For chain info, returns the node with the highest block height
   * For other queries, returns the first successful response
   */
  private async callAll<T>(method: string, params: unknown[] = [], selector?: (results: T[]) => T): Promise<T> {
    const promises = this.baseUrls.map(async (url) => {
      try {
        const response = await fetch(url, {
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

        return { success: true, result: data.result, url };
      } catch (error: any) {
        return { success: false, error, url };
      }
    });

    const results = await Promise.all(promises);
    const successful = results.filter(r => r.success) as Array<{ success: true; result: T; url: string }>;

    if (successful.length === 0) {
      const errorMessages = results.map(r => (r as any).error?.message || 'Unknown error').join('; ');
      throw new Error(`Cannot connect to any RPC server. Tried: ${this.baseUrls.join(', ')}. Errors: ${errorMessages}`);
    }

    // Use selector if provided, otherwise return first successful result
    if (selector) {
      return selector(successful.map(r => r.result));
    }

    return successful[0].result;
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
    // Query all nodes and return the one with the highest block height
    return this.callAll<ChainInfo>('chain_getInfo', [], (results) => {
      // Return the chain info with the highest block height
      return results.reduce((best, current) => 
        current.best_height > best.best_height ? current : best
      );
    });
  }

  async submitBlock(block: Block): Promise<string> {
    // The block needs to match Rust's serialization format exactly
    // Hash and Address are serialized as byte arrays [u8; 32]
    // Convert any hex strings to byte arrays before submission
    const serializedBlock = this.serializeBlockForRpc(block);
    return this.call<string>('chain_submitBlock', [serializedBlock]);
  }

  private serializeBlockForRpc(block: Block): any {
    // Convert block to match Rust serialization format
    // Hash and Address fields need to be byte arrays
    const serializeHash = (hash: string | number[]): number[] => {
      if (Array.isArray(hash)) return hash;
      const bytes = hexToBytes(hash);
      return Array.from(bytes);
    };

    const serializeAddress = (addr: string | number[]): number[] => {
      if (Array.isArray(addr)) return addr;
      const bytes = hexToBytes(addr);
      return Array.from(bytes);
    };

    // Explicitly construct header with all fields in exact Rust struct order
    // This prevents JavaScript from reordering fields when using spread operator
    // Field order matches BlockHeader in core/src/block.rs:
    // version, height, prev_hash, timestamp, transactions_root, solutions_root,
    // commitment, work_score, miner, nonce, solve_time_us, verify_time_us,
    // time_asymmetry_ratio, solution_quality, complexity_weight, energy_estimate_joules
    return {
      header: {
        version: block.header.version,
        height: block.header.height,
        prev_hash: serializeHash(block.header.prev_hash),
        timestamp: block.header.timestamp,
        transactions_root: serializeHash(block.header.transactions_root),
        solutions_root: serializeHash(block.header.solutions_root),
        commitment: {
          hash: serializeHash(block.header.commitment.hash),
          problem_hash: serializeHash(block.header.commitment.problem_hash),
        },
        work_score: block.header.work_score,
        miner: serializeAddress(block.header.miner),
        nonce: block.header.nonce,
        solve_time_us: block.header.solve_time_us,
        verify_time_us: block.header.verify_time_us,
        time_asymmetry_ratio: block.header.time_asymmetry_ratio,
        solution_quality: block.header.solution_quality,
        complexity_weight: block.header.complexity_weight,
        energy_estimate_joules: block.header.energy_estimate_joules,
      },
      coinbase: block.coinbase,
      transactions: block.transactions,
      solution_reveal: {
        ...block.solution_reveal,
        commitment: {
          hash: serializeHash(block.solution_reveal.commitment.hash),
          problem_hash: serializeHash(block.solution_reveal.commitment.problem_hash),
        },
      },
    };
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
