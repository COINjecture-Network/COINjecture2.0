// RPC Client for COINjecture Network B
// Handles all communication with the blockchain node

export interface Block {
  header: {
    version: number;
    height: number;
    prev_hash: string;
    timestamp: number;
    miner: string;
    work_score: number;
    nonce: number;
    // PoUW Transparency Metrics
    solve_time_ms: number;
    verify_time_ms: number;
    time_asymmetry_ratio: number;
    solution_quality: number;
    complexity_weight: number;
    energy_estimate_joules: number;
  };
  coinbase: {
    reward: number;
  };
  transactions: Transaction[];
}

export interface Transaction {
  hash: string;
  from: string;
  to: string;
  amount: number;
  fee: number;
  nonce: number;
  signature: string;
}

export interface AccountInfo {
  balance: number;
  nonce: number;
}

export interface ChainInfo {
  chain_id: string;
  best_height: number;
  best_hash: string;
  genesis_hash: string;
  peer_count: number;
}

export interface PoolMetrics {
  d1: number;
  d2: number;
  d3: number;
  d4: number;
  d5: number;
  d6: number;
  d7: number;
  d8: number;
}

export interface PoolLiquidity {
  total: number;
  locked: number;
  unlocked: number;
  unlockFraction: number; // 0.0 to 1.0
  yieldRate: number;
}

export interface AllPoolsData {
  [key: string]: PoolLiquidity; // D1, D2, D3, ..., D8
}

export interface ConsensusState {
  tau: number;           // τ = block_height / τ_c
  magnitude: number;     // |ψ(τ)| = e^(-ητ)
  phase: number;         // θ(τ) = λτ (radians)
}

export interface SatoshiConstants {
  eta: number;
  lambda: number;
  unit_circle_constraint: number;
  damping_coefficient: number;
}

export interface ConvergenceMetrics {
  measured_eta: number;
  measured_lambda: number;
  theoretical_eta: number; // 1/√2
  theoretical_lambda: number; // 1/√2
  convergence_confidence: number; // R² from exponential fitting
  measured_oracle_delta: number;
  eta_error: number;
  lambda_error: number;
}

export interface FaucetResponse {
  success: boolean;
  amount?: number;
  new_balance?: number;
  message: string;
  cooldown_remaining?: number;
}

export class RpcClient {
  private baseUrl: string;
  private requestId: number = 1;

  constructor(baseUrl: string = '/rpc') {
    this.baseUrl = baseUrl;
  }

  private async call<T>(method: string, params: any[] = []): Promise<T> {
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

    const data = await response.json();

    if (data.error) {
      throw new Error(data.error.message || 'RPC error');
    }

    return data.result;
  }

  async getBlock(height: number): Promise<Block | null> {
    return this.call<Block | null>('chain_getBlock', [height]);
  }

  async getLatestBlock(): Promise<Block | null> {
    return this.call<Block | null>('chain_getLatestBlock', []);
  }

  async getAccountBalance(address: string): Promise<number> {
    return this.call<number>('account_getBalance', [address]);
  }

  async getAccountInfo(address: string): Promise<AccountInfo> {
    return this.call<AccountInfo>('account_getInfo', [address]);
  }

  async submitTransaction(txHex: string): Promise<string> {
    return this.call<string>('transaction_submit', [txHex]);
  }

  async getChainInfo(): Promise<ChainInfo> {
    return this.call<ChainInfo>('chain_getInfo', []);
  }

  async faucetRequest(address: string): Promise<FaucetResponse> {
    return this.call<FaucetResponse>('faucet_requestTokens', [address]);
  }
}

export class MetricsClient {
  private baseUrl: string;

  constructor(baseUrl: string = '/metrics') {
    this.baseUrl = baseUrl;
  }

  // Parse Prometheus text format metrics
  private async fetchMetrics(): Promise<Map<string, Map<string, number>>> {
    const response = await fetch(this.baseUrl);
    if (!response.ok) {
      throw new Error(`HTTP error! status: ${response.status}`);
    }

    const text = await response.text();
    const metrics = new Map<string, Map<string, number>>();

    // Parse Prometheus text format
    const lines = text.split('\n');
    for (const line of lines) {
      // Skip comments and empty lines
      if (line.startsWith('#') || line.trim() === '') continue;

      // Parse metric line: metric_name{label="value"} value
      const labelMatch = line.match(/^(\w+)\{dimension="([^"]+)"\}\s+([0-9.e+-]+)$/);
      if (labelMatch) {
        const [, metricName, dimension, value] = labelMatch;
        if (!metrics.has(metricName)) {
          metrics.set(metricName, new Map());
        }
        metrics.get(metricName)!.set(dimension, parseFloat(value));
        continue;
      }

      // Parse simple metric line: metric_name value
      const simpleMatch = line.match(/^(\w+)\s+([0-9.e+-]+)$/);
      if (simpleMatch) {
        const [, metricName, value] = simpleMatch;
        if (!metrics.has(metricName)) {
          metrics.set(metricName, new Map());
        }
        metrics.get(metricName)!.set('', parseFloat(value));
      }
    }

    return metrics;
  }

  async getPoolBalances(): Promise<PoolMetrics> {
    const metrics = await this.fetchMetrics();
    const balances = metrics.get('coinject_pool_balance') || new Map();

    return {
      d1: balances.get('D1') || 0,
      d2: balances.get('D2') || 0,
      d3: balances.get('D3') || 0,
      d4: balances.get('D4') || 0,
      d5: balances.get('D5') || 0,
      d6: balances.get('D6') || 0,
      d7: balances.get('D7') || 0,
      d8: balances.get('D8') || 0,
    };
  }

  async getAllPoolsData(): Promise<AllPoolsData> {
    const metrics = await this.fetchMetrics();

    const balances = metrics.get('coinject_pool_balance') || new Map();
    const locked = metrics.get('coinject_pool_locked') || new Map();
    const unlocked = metrics.get('coinject_pool_unlocked') || new Map();
    const fractions = metrics.get('coinject_pool_unlock_fraction') || new Map();
    const yields = metrics.get('coinject_pool_yield_rate') || new Map();

    const poolsData: AllPoolsData = {};

    // Initialize all 8 pools
    for (let i = 1; i <= 8; i++) {
      const poolKey = `D${i}`;
      poolsData[poolKey] = {
        total: balances.get(poolKey) || 0,
        locked: locked.get(poolKey) || 0,
        unlocked: unlocked.get(poolKey) || 0,
        unlockFraction: fractions.get(poolKey) || 0,
        yieldRate: yields.get(poolKey) || 0
      };
    }

    return poolsData;
  }

  async getConsensusState(): Promise<ConsensusState> {
    const metrics = await this.fetchMetrics();

    return {
      tau: metrics.get('coinject_consensus_tau')?.get('') || 0,
      magnitude: metrics.get('coinject_consensus_magnitude')?.get('') || 0,
      phase: metrics.get('coinject_consensus_phase')?.get('') || 0
    };
  }

  async getSatoshiConstants(): Promise<SatoshiConstants> {
    const metrics = await this.fetchMetrics();

    return {
      eta: metrics.get('coinject_measured_eta')?.get('') || 0,
      lambda: metrics.get('coinject_measured_lambda')?.get('') || 0,
      unit_circle_constraint: metrics.get('coinject_unit_circle_constraint')?.get('') || 0,
      damping_coefficient: metrics.get('coinject_damping_coefficient')?.get('') || 0
    };
  }

  async getBlockHeight(): Promise<number> {
    const metrics = await this.fetchMetrics();
    return metrics.get('coinject_block_height')?.get('') || 0;
  }

  async getConvergenceMetrics(): Promise<ConvergenceMetrics> {
    const metrics = await this.fetchMetrics();
    const THEORETICAL = 1 / Math.sqrt(2); // 0.707107...

    return {
      measured_eta: metrics.get('coinject_measured_eta')?.get('') || THEORETICAL,
      measured_lambda: metrics.get('coinject_measured_lambda')?.get('') || THEORETICAL,
      theoretical_eta: THEORETICAL,
      theoretical_lambda: THEORETICAL,
      convergence_confidence: metrics.get('coinject_convergence_confidence')?.get('') || 0,
      measured_oracle_delta: metrics.get('coinject_measured_oracle_delta')?.get('') || 0.231,
      eta_error: metrics.get('coinject_eta_convergence_error')?.get('') || 0,
      lambda_error: metrics.get('coinject_lambda_convergence_error')?.get('') || 0
    };
  }
}
