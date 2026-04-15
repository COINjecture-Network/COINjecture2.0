/**
 * RPC Client for COINjecture Network B
 * Connects to JSON-RPC endpoints for blockchain operations
 * Matches the actual Rust RPC server implementation in rpc/src/server.rs
 * Supports multiple nodes with failover and parallel querying
 * 
 * Version: 1.0.1 (2025-12-03) - Cache-bust update
 */

import { hexToBytes } from '@noble/hashes/utils';

/** Cross-origin JSON-RPC: omit cookies so `Access-Control-Allow-Origin: *` is valid. */
const RPC_FETCH_TIMEOUT_MS = 45_000;
/** Full block JSON can be large; allow longer read than generic RPC calls. */
const RPC_BLOCK_BODY_TIMEOUT_MS = 90_000;
/** `chain_submitBlock` payloads are large; API + Nginx must allow big bodies and long proxy reads. */
const RPC_SUBMIT_BLOCK_TIMEOUT_MS = 300_000;
/**
 * Parallel `callAll` waits for **every** in-flight request to finish (success or timeout), then picks
 * the best result — so wall time is ~max(per-endpoint latency). Keep this modest for dashboard loads.
 */
const RPC_CALL_ALL_TIMEOUT_MS = 10_000;
/** `chain_getInfo` across all RPCs — slightly tighter so the metrics hero does not sit blank ~2×14s. */
const RPC_CALL_ALL_CHAIN_INFO_TIMEOUT_MS = 8_000;

async function fetchWithTimeout(
  url: string,
  init: RequestInit,
  timeoutMs: number = RPC_FETCH_TIMEOUT_MS,
): Promise<Response> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    return await fetch(url, {
      ...init,
      signal: controller.signal,
      credentials: 'omit',
      mode: 'cors',
    });
  } finally {
    clearTimeout(timer);
  }
}

/** Non-OK responses: include body (API JSON or nginx HTML snippet) so 503 is debuggable in the UI. */
async function httpErrorFromResponse(response: Response): Promise<Error> {
  const hint = await response.text().catch(() => '');
  const trimmed = hint.replace(/\s+/g, ' ').trim().slice(0, 400);
  return trimmed.length > 0
    ? new Error(`HTTP ${response.status}: ${trimmed}`)
    : new Error(`HTTP error! status: ${response.status}`);
}

// Parse RPC URLs from environment variable (comma-separated). Empty in production = use API tunnel.
const parseRpcUrls = (): string[] => {
  const raw = (import.meta.env.VITE_RPC_URL as string | undefined)?.trim();
  if (!raw) {
    return [];
  }
  return raw.split(',').map((url) => url.trim()).filter((url) => url.length > 0);
};

// In development, use Vite proxy to avoid CORS issues
// In production (CloudFront), use relative /api/rpc path that CloudFront will proxy
// The RPC client can specify target node via query parameter or header
const isDevelopment = import.meta.env.DEV;
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
      // Hostinger VPS (current DNS targets)
      if (url.includes('193.203.164.13')) {
        return 'https://rpc1.coinjecture.com';
      }
      if (url.includes('76.13.101.67')) {
        return 'https://rpc2.coinjecture.com';
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

function apiBaseTrimmed(): string {
  return ((import.meta.env.VITE_API_URL as string | undefined) || '').replace(/\/$/, '');
}

function urlsAreUnsafeForHttpsBrowser(urls: string[]): boolean {
  return urls.some((u) => {
    try {
      const { protocol, hostname } = new URL(u);
      if (protocol !== 'https:') {
        return true;
      }
      return hostname === 'localhost' || hostname === '127.0.0.1';
    } catch {
      return true;
    }
  });
}

/**
 * Dev: Vite `/api/rpc` → first `VITE_RPC_URL` (see vite.config).
 * Prod: explicit `VITE_RPC_URL` (HTTPS public RPC) if set; otherwise `VITE_API_URL/node-rpc` so the
 * bundle never needs to call `http://localhost:9933` from the user's browser.
 */
export function getDefaultRpcBaseUrls(): string[] {
  if (isDevelopment) {
    return ['/api/rpc'];
  }

  const apiBase = apiBaseTrimmed();
  const fromEnv = createProxyUrls();

  if (fromEnv.length === 0 && apiBase) {
    return [`${apiBase}/node-rpc`];
  }

  if (fromEnv.length > 0 && apiBase && isHTTPS && urlsAreUnsafeForHttpsBrowser(fromEnv)) {
    return [`${apiBase}/node-rpc`];
  }

  if (fromEnv.length > 0) {
    return fromEnv;
  }

  console.error(
    '[rpc-client] Production: set VITE_API_URL (uses /node-rpc) or HTTPS VITE_RPC_URL. Using localhost (broken on deployed sites).',
  );
  return ['http://localhost:9933'];
}

/** Production: same-origin-friendly chain summary from the API (CORS already on `VITE_API_URL`). */
async function fetchChainInfoFromApi(): Promise<ChainInfo> {
  const raw = import.meta.env.VITE_API_URL as string | undefined;
  const base = (raw || '').replace(/\/$/, '');
  if (!base) {
    throw new Error('VITE_API_URL not set');
  }
  const response = await fetchWithTimeout(
    `${base}/chain/info`,
    {
      method: 'GET',
      headers: { Accept: 'application/json' },
    },
    15_000,
  );
  if (!response.ok) {
    throw await httpErrorFromResponse(response);
  }
  const j = (await response.json()) as Record<string, unknown>;
  const network = typeof j.network === 'string' ? j.network : 'mainnet';

  // When the API cannot reach the node, `height` is omitted or null — do not coerce to 0 or the UI
  // shows a fake "0" tip and skips JSON-RPC fallback in getChainInfo().
  const rawHeight = j.height ?? j.best_height;
  if (rawHeight === null || rawHeight === undefined) {
    throw new Error(
      'API /chain/info has no block height (check API NODE_RPC_URL and that the chain node RPC is up)',
    );
  }
  let best_height: number;
  if (typeof rawHeight === 'number' && Number.isFinite(rawHeight)) {
    best_height = rawHeight;
  } else if (typeof rawHeight === 'string' && rawHeight.trim() !== '') {
    const n = Number(rawHeight);
    if (!Number.isFinite(n)) {
      throw new Error('API /chain/info height is not a valid number');
    }
    best_height = n;
  } else {
    throw new Error('API /chain/info has invalid block height');
  }

  const best_hash = typeof j.best_hash === 'string' ? j.best_hash : '';
  const genesis_hash = typeof j.genesis_hash === 'string' ? j.genesis_hash : '';

  const header_pow_difficulty =
    typeof j.header_pow_difficulty === 'number'
      ? j.header_pow_difficulty
      : j.header_pow_difficulty != null
        ? Number(j.header_pow_difficulty)
        : undefined;
  const np_problem_size =
    typeof j.np_problem_size === 'number'
      ? j.np_problem_size
      : j.np_problem_size != null
        ? Number(j.np_problem_size)
        : undefined;

  const best_cumulative_work =
    typeof j.best_cumulative_work === 'string' && j.best_cumulative_work.trim() !== ''
      ? j.best_cumulative_work.trim()
      : undefined;
  const total_minted_rewards =
    typeof j.total_minted_rewards === 'string' && j.total_minted_rewards.trim() !== ''
      ? j.total_minted_rewards.trim()
      : undefined;

  return {
    chain_id: typeof j.chain_id === 'string' ? j.chain_id : `coinjecture:${network}`,
    best_height,
    best_hash,
    genesis_hash,
    peer_count: typeof j.peer_count === 'number' ? j.peer_count : Number(j.peer_count ?? 0) || 0,
    total_work: typeof j.total_work === 'number' ? j.total_work : undefined,
    is_syncing: Boolean(j.syncing),
    header_pow_difficulty: Number.isFinite(header_pow_difficulty)
      ? header_pow_difficulty
      : undefined,
    np_problem_size: Number.isFinite(np_problem_size) ? np_problem_size : undefined,
    best_cumulative_work,
    total_minted_rewards,
  };
}

/** True when API returned 200 but looks like no live tip (bad NODE_RPC vs real genesis with hashes). */
function apiChainInfoLooksEmptyTip(info: ChainInfo): boolean {
  return info.best_height === 0 && !info.best_hash && !info.genesis_hash;
}

function finiteOrUndef(n: number | undefined): number | undefined {
  return n != null && Number.isFinite(n) ? n : undefined;
}

/** Prefer API for identity fields; fill mining / W / minted from JSON-RPC when the API omits them. */
function mergeChainInfoFromRpc(api: ChainInfo, rpc: ChainInfo): ChainInfo {
  const bcwApi = api.best_cumulative_work?.trim();
  const bcwRpc = rpc.best_cumulative_work?.trim();
  const tmrApi = api.total_minted_rewards?.trim();
  const tmrRpc = rpc.total_minted_rewards?.trim();
  return {
    ...api,
    total_work: api.total_work ?? rpc.total_work,
    is_syncing: api.is_syncing ?? rpc.is_syncing,
    header_pow_difficulty:
      finiteOrUndef(api.header_pow_difficulty) ?? finiteOrUndef(rpc.header_pow_difficulty),
    np_problem_size: finiteOrUndef(api.np_problem_size) ?? finiteOrUndef(rpc.np_problem_size),
    best_cumulative_work: bcwApi || bcwRpc || undefined,
    total_minted_rewards: tmrApi || tmrRpc || undefined,
  };
}

/** Best-effort NP “size” from `chain_getMiningWork` for dashboard when `np_problem_size` is absent. */
export function npProblemSizeFromMiningProblem(problem: ProblemType | null | undefined): number | undefined {
  if (!problem || typeof problem !== 'object') return undefined;
  if (problem.SubsetSum?.numbers && Array.isArray(problem.SubsetSum.numbers)) {
    const n = problem.SubsetSum.numbers.length;
    return n > 0 ? n : undefined;
  }
  if (problem.SAT) {
    const clauses = problem.SAT.clauses;
    const clen = Array.isArray(clauses) ? clauses.length : 0;
    const v = typeof problem.SAT.variables === 'number' ? problem.SAT.variables : 0;
    const prod = v * clen;
    return prod > 0 ? prod : undefined;
  }
  if (problem.TSP?.cities != null) {
    const c = Number(problem.TSP.cities);
    return Number.isFinite(c) && c > 0 ? c : undefined;
  }
  return undefined;
}

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

// Solution payload (same shape as block solution_reveal) — referenced by ProblemInfo
export interface SolutionType {
  SubsetSum?: number[];
  SAT?: boolean[];
  TSP?: number[];
  Custom?: string;
}

// Problem marketplace response - matches ProblemInfo in rpc/src/server.rs
export interface ProblemInfo {
  problem_id: string;
  submitter: string;
  /** Ledger atoms (API may send string for large `u128`). */
  bounty: string | number;
  min_work_score: number;
  status: string; // Rust Debug, e.g. "Open", "Solved" (case varies; use isMarketplaceListingOpen)
  submitted_at: number; // i64 timestamp
  expires_at: number; // i64 timestamp
  is_private: boolean;
  problem_type: string | null; // e.g., "SubsetSum(5)", "SAT(vars=10, clauses=20)"
  problem_size: number | null; // usize
  is_revealed: boolean;
  problem?: ProblemType | null;
  /** Hex-encoded solver address when the listing is solved */
  solver?: string | null;
  /** Winning solution attached on-chain when solved */
  solution?: SolutionType | null;
}

// Marketplace statistics - matches MarketplaceStats in state/src/marketplace.rs
export interface MarketplaceStats {
  total_problems: number; // usize
  open_problems: number; // usize
  solved_problems: number; // usize
  expired_problems: number; // usize
  cancelled_problems: number; // usize
  total_bounty_pool: string | number; // Balance atoms
}

// Chain information - matches ChainInfo in rpc/src/server.rs
export interface ChainInfo {
  chain_id: string;
  best_height: number; // u64
  best_hash: string; // hex-encoded
  genesis_hash: string; // hex-encoded
  peer_count: number; // usize
  total_work?: number;
  is_syncing?: boolean;
  /** Leading hex `0` nibbles on header hash (node mining / RPC). */
  header_pow_difficulty?: number;
  /** NP post-block adjuster size for the next template. */
  np_problem_size?: number;
  /** Canonical cumulative work `W` at best tip (decimal string, u128). */
  best_cumulative_work?: string;
  /** Sum of coinbase rewards on the best chain (decimal string, u128). */
  total_minted_rewards?: string;
}

/** `chain_getMiningWork` — deterministic instance for the next block (same as node miner). */
export interface MiningWork {
  next_height: number;
  prev_hash: string;
  /**
   * Required leading **hex** `0` characters on the header hash hex string (not Bitcoin nBits).
   * E.g. `4` means the hash hex must start with `0000`. Max meaningful value is 64.
   */
  difficulty: number;
  problem: ProblemType;
}

// Account information - matches AccountInfo in rpc/src/server.rs
export interface AccountInfo {
  address: string;
  /** Ledger atoms (`Balance`); 1 display BEANS = 10^12 atoms — use `formatBeans` from chain-metrics. */
  balance: bigint;
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
  /** coinject_core::transaction::CoinbaseTransaction */
  coinbase?: {
    to: string | number[];
    reward: number | string;
    height: number;
  } | null;
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
  /** Ledger atoms (decimal string or safe integer). */
  bounty: string | number;
  min_work_score: number; // f64
  expiration_days: number; // u64
}

export interface PrivateProblemWalletParams {
  problem: ProblemType;
  salt: string;
  /** Ledger atoms — use `displayBeansToAtoms` × display BEANS. */
  bounty: string | number;
  min_work_score: number;
  expiration_days: number;
  submitter: string;
}

export interface PrivateProblemSubmissionResult {
  problem_id: string;
  commitment: string;
}

// Public problem submission parameters - matches PublicProblemParams in rpc/src/server.rs
export interface PublicProblemParams {
  problem: ProblemType;
  /** Ledger atoms — use `displayBeansToAtoms` × display BEANS. */
  bounty: string | number;
  min_work_score: number;
  expiration_days: number;
  submitter: string;
}

// Problem reveal parameters - matches RevealParams in rpc/src/server.rs
export interface RevealParams {
  problem_id: string; // hex-encoded hash
  problem: string; // JSON-encoded ProblemType
  salt: string; // hex-encoded 32-byte salt
}

export interface SolutionSubmissionParams {
  problem_id: string;
  solution: SolutionType;
  solver: string;
}

export class RpcClient {
  private baseUrls: string[];
  private requestId: number = 1;
  private currentUrlIndex: number = 0;

  constructor(baseUrls?: string[]) {
    const resolved = baseUrls?.length ? baseUrls : getDefaultRpcBaseUrls();
    this.baseUrls = resolved.length > 0 ? resolved : ['http://localhost:9933'];
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
  private async call<T>(
    method: string,
    params: unknown[] = [],
    timeoutMs: number = RPC_FETCH_TIMEOUT_MS,
  ): Promise<T> {
    const errors: Error[] = [];
    
    // Try each URL in order (failover)
    for (let i = 0; i < this.baseUrls.length; i++) {
      const url = this.baseUrls[i];
      try {
        const response = await fetchWithTimeout(
          url,
          {
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
          },
          timeoutMs,
        );

        if (!response.ok) {
          throw await httpErrorFromResponse(response);
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

    // All URLs failed (includes JSON-RPC errors like invalid tx, not only network failures)
    const errorMessages = errors.map(e => e.message).join('; ');
    throw new Error(`All RPC endpoints failed. Tried: ${this.baseUrls.join(', ')}. Errors: ${errorMessages}`);
  }

  /**
   * Call RPC method on all nodes in parallel and return the best result
   * For chain info, returns the node with the highest block height
   * For other queries, returns the first successful response
   */
  private async callAll<T>(
    method: string,
    params: unknown[] = [],
    selector?: (results: T[]) => T,
    timeoutMs: number = RPC_CALL_ALL_TIMEOUT_MS,
  ): Promise<T> {
    const promises = this.baseUrls.map(async (url) => {
      try {
        const response = await fetchWithTimeout(
          url,
          {
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
          },
          timeoutMs,
        );

        if (!response.ok) {
          throw await httpErrorFromResponse(response);
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
      throw new Error(`All RPC endpoints failed. Tried: ${this.baseUrls.join(', ')}. Errors: ${errorMessages}`);
    }

    // Use selector if provided, otherwise return first successful result
    if (selector) {
      return selector(successful.map(r => r.result));
    }

    return successful[0].result;
  }

  // ========== Account Methods ==========
  
  async getBalance(address: string): Promise<bigint> {
    const raw = await this.call<number | string>('account_getBalance', [address]);
    return typeof raw === 'bigint' ? raw : BigInt(String(raw));
  }

  async getNonce(address: string): Promise<number> {
    return this.call<number>('account_getNonce', [address]);
  }

  async getAccountInfo(address: string): Promise<AccountInfo> {
    const raw = await this.call<{ address: string; balance: number | string; nonce: number }>(
      'account_getInfo',
      [address],
    );
    const bal =
      typeof raw.balance === 'bigint'
        ? raw.balance
        : typeof raw.balance === 'string'
          ? BigInt(raw.balance)
          : BigInt(Math.trunc(Number(raw.balance)));
    return { address: raw.address, balance: bal, nonce: raw.nonce };
  }

  // ========== Chain Methods ==========

  /**
   * Query every RPC URL in parallel; return the first non-null block.
   * `chain_getInfo` uses max height across nodes — any single node may return `null` for `chain_getBlock(h)`
   * if it is behind, while another has the full chain. Sequential "try until null" still preferred rpc1 first;
   * parallel avoids one slow/failing node hiding another that has data.
   */
  async getBlock(height: number): Promise<Block | null> {
    const outcomes = await Promise.all(
      this.baseUrls.map((url) =>
        this.jsonRpcRequest<Block | null>(url, 'chain_getBlock', [height], RPC_BLOCK_BODY_TIMEOUT_MS),
      ),
    );
    return outcomes.find((b) => b != null) ?? null;
  }

  /** Same as {@link getBlock}: first successful non-null among all configured RPC URLs. */
  async getLatestBlock(): Promise<Block | null> {
    const outcomes = await Promise.all(
      this.baseUrls.map((url) =>
        this.jsonRpcRequest<Block | null>(url, 'chain_getLatestBlock', [], RPC_BLOCK_BODY_TIMEOUT_MS),
      ),
    );
    return outcomes.find((b) => b != null) ?? null;
  }

  /**
   * Latest full block via `GET {VITE_API_URL}/chain/latest-block` (same CORS as `/chain/info`).
   * Use when browser POST to `/node-rpc` fails or returns empty bodies.
   */
  async getLatestBlockFromApi(): Promise<Block | null> {
    if (isDevelopment) return null;
    const base = apiBaseTrimmed();
    if (!base) return null;
    try {
      const response = await fetchWithTimeout(
        `${base}/chain/latest-block`,
        { method: 'GET', headers: { Accept: 'application/json' } },
        RPC_BLOCK_BODY_TIMEOUT_MS,
      );
      if (!response.ok) return null;
      const data = (await response.json()) as unknown;
      if (!data || typeof data !== 'object') return null;
      const o = data as Record<string, unknown>;
      if (!('header' in o)) return null;
      return data as Block;
    } catch {
      return null;
    }
  }

  /** Single JSON-RPC POST; returns null on error / jsonrpc error / null result (no throw). */
  private async jsonRpcRequest<T>(
    url: string,
    method: string,
    params: unknown[],
    timeoutMs: number = RPC_FETCH_TIMEOUT_MS,
  ): Promise<T | null> {
    try {
      const response = await fetchWithTimeout(
        url,
        {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            jsonrpc: '2.0',
            id: this.requestId++,
            method,
            params,
          }),
        },
        timeoutMs,
      );
      if (!response.ok) return null;
      const data: RpcResponse<T> = await response.json();
      if (data.error) return null;
      if (data.result === undefined) return null;
      return data.result as T;
    } catch {
      return null;
    }
  }

  async getBlockHeader(height: number): Promise<BlockHeader | null> {
    return this.call<BlockHeader | null>('chain_getBlockHeader', [height]);
  }

  /** Prefer max `best_height` across RPC URLs (same as previous `getChainInfo` JSON-RPC path). */
  private async fetchChainInfoViaJsonRpc(): Promise<ChainInfo> {
    try {
      return await this.callAll<ChainInfo>(
        'chain_getInfo',
        [],
        (results) =>
          results.reduce((best, current) =>
            current.best_height > best.best_height ? current : best,
          ),
        RPC_CALL_ALL_CHAIN_INFO_TIMEOUT_MS,
      );
    } catch {
      return this.call<ChainInfo>('chain_getInfo', []);
    }
  }

  /** When `chain_getInfo` omits mining tips, try `chain_getMiningWork` (mining-enabled nodes only). */
  private async supplementChainInfoMiningFields(info: ChainInfo): Promise<ChainInfo> {
    const havePow = finiteOrUndef(info.header_pow_difficulty) != null;
    const haveNp = finiteOrUndef(info.np_problem_size) != null;
    if (havePow && haveNp) return info;
    try {
      // Single-RPC `call` (failover list) — avoids a second full `callAll` that waits on every host.
      const mw = await this.call<MiningWork>('chain_getMiningWork', [], 10_000);
      let out = info;
      if (!havePow && Number.isFinite(mw.difficulty)) {
        out = { ...out, header_pow_difficulty: mw.difficulty };
      }
      if (!haveNp) {
        const n = npProblemSizeFromMiningProblem(mw.problem);
        if (n != null) out = { ...out, np_problem_size: n };
      }
      return out;
    } catch {
      return info;
    }
  }

  /**
   * Merge `GET /chain/info` with live `chain_getInfo` from JSON-RPC so mining / W / minted fields
   * are not stuck empty when the API’s `NODE_RPC_URL` points at a non-mining or older node.
   */
  async getChainInfo(): Promise<ChainInfo> {
    const rpcPromise = this.fetchChainInfoViaJsonRpc();

    if (!isDevelopment) {
      const base = apiBaseTrimmed();
      if (base) {
        try {
          const [fromApi, rpcInfo] = await Promise.all([
            fetchChainInfoFromApi(),
            rpcPromise,
          ]);
          if (!apiChainInfoLooksEmptyTip(fromApi)) {
            return await this.supplementChainInfoMiningFields(mergeChainInfoFromRpc(fromApi, rpcInfo));
          }
          console.warn(
            '[rpc-client] /chain/info returned empty tip (height 0, no hashes); using JSON-RPC',
          );
        } catch (e) {
          console.warn('[rpc-client] /chain/info failed; using JSON-RPC', e);
        }
        return await this.supplementChainInfoMiningFields(await rpcPromise);
      }
    }

    return await this.supplementChainInfoMiningFields(await rpcPromise);
  }

  /** Next mining template from nodes with mining enabled (longest `next_height` wins in multi-RPC). */
  async getMiningWork(): Promise<MiningWork> {
    try {
      return await this.callAll<MiningWork>('chain_getMiningWork', [], (results) =>
        results.reduce((best, cur) => (cur.next_height > best.next_height ? cur : best)),
      );
    } catch {
      return this.call<MiningWork>('chain_getMiningWork', []);
    }
  }

  async submitBlock(block: Block): Promise<string> {
    // The block needs to match Rust's serialization format exactly
    // Hash and Address are serialized as byte arrays [u8; 32]
    // Convert any hex strings to byte arrays before submission
    const serializedBlock = this.serializeBlockForRpc(block);
    return this.call<string>('chain_submitBlock', [serializedBlock], RPC_SUBMIT_BLOCK_TIMEOUT_MS);
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
    return this.call<string>('transaction_submit', [txHex], RPC_SUBMIT_BLOCK_TIMEOUT_MS);
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

  async getProblemsBySubmitter(address: string): Promise<ProblemInfo[]> {
    return this.call<ProblemInfo[]>('marketplace_getProblemsBySubmitter', [address]);
  }

  async getProblemsBySolver(address: string): Promise<ProblemInfo[]> {
    return this.call<ProblemInfo[]>('marketplace_getProblemsBySolver', [address]);
  }

  async getMarketplaceStats(): Promise<MarketplaceStats> {
    return this.call<MarketplaceStats>('marketplace_getStats', []);
  }

  async submitPublicProblem(params: PublicProblemParams): Promise<string> {
    return this.call<string>('marketplace_submitPublicProblem', [params]);
  }

  async submitPrivateProblem(params: PrivateProblemParams): Promise<string> {
    return this.call<string>('marketplace_submitPrivateProblem', [params]);
  }

  async submitPrivateProblemWithWallet(
    params: PrivateProblemWalletParams,
  ): Promise<PrivateProblemSubmissionResult> {
    return this.call<PrivateProblemSubmissionResult>(
      'marketplace_submitPrivateProblemWithWallet',
      [params],
    );
  }

  async revealProblem(params: RevealParams): Promise<boolean> {
    return this.call<boolean>('marketplace_revealProblem', [params]);
  }

  async submitSolution(params: SolutionSubmissionParams): Promise<boolean> {
    return this.call<boolean>('marketplace_submitSolution', [params]);
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
