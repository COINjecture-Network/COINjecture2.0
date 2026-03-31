// API response types for the COINjecture API server

export interface BlockEvent {
  height: number;
  hash: string;
  timestamp: string;
  tx_count: number;
  miner: string;
  work_score: number;
}

export interface MempoolEvent {
  pending_count: number;
  total_size_bytes: number;
  oldest_tx_age_seconds: number;
}

export interface ChainInfo {
  network: string;
  height: number | null;
  syncing: boolean;
  peer_count: number | null;
  version: string;
}

export interface TradingPair {
  id: string;
  base_token: string;
  quote_token: string;
  is_active: boolean;
}

export interface Trade {
  price: string;
  quantity: string;
  executed_at: string;
  buyer_wallet?: string;
  seller_wallet?: string;
}

export interface PouwTask {
  id: string;
  title: string;
  problem_class: string;
  bounty_amount: string;
  bounty_token: string;
  status: string;
  deadline: string;
  min_work_score?: number;
  max_assignments?: number;
}

export interface SolutionSet {
  id: string;
  block_height: number;
  block_hash: string;
  created_at?: string | null;
  problem_id?: string | null;
  problem_type: string;
  solution_type: string;
  miner?: string | null;
  work_score?: number | null;
  solve_time_us?: number | null;
  verify_time_us?: number | null;
  time_asymmetry_ratio?: number | null;
  solution_quality?: number | null;
  complexity_weight?: number | null;
  energy_estimate_joules?: number | null;
  quality_band?: string | null;
  raw_problem?: unknown;
  raw_solution?: unknown;
  raw_solution_reveal?: unknown;
}
