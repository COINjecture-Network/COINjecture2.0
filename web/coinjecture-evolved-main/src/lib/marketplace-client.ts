// Marketplace export API — same paths as web-wallet `marketplace-client`.
// Dev: Vite proxies `/marketplace` → export service. Prod: set `VITE_MARKETPLACE_EXPORT_URL`.

export interface CatalogStats {
  total_problems: number;
  by_type: {
    subset_sum: number;
    sat: number;
    tsp: number;
    custom: number;
  };
  work_score_stats: {
    min: number;
    max: number;
    avg: number;
    median: number;
  };
  time_asymmetry_stats: {
    min: number;
    max: number;
    avg: number;
    median: number;
  };
  latest_height: number;
  total_energy_joules: number;
}

export interface MarketplaceCatalogEntry {
  problem_id: string;
  provenance: {
    block_height: number;
    block_hash: string;
    miner_address: string;
    timestamp: string;
    commitment_hash: string;
  };
  metrics: {
    work_score: number;
    solve_time_ms: number;
    verify_time_ms: number;
    time_asymmetry_ratio: number;
    solution_quality: number;
    complexity_weight: number;
    energy_estimate_joules: number;
    space_asymmetry_ratio?: number;
  };
  problem: unknown;
  solution: unknown;
  metadata: {
    exported_at: string;
    export_version: string;
    protocol_version: number;
  };
}

export interface SearchCriteria {
  problem_type?: string;
  min_work_score?: number;
  max_work_score?: number;
  min_time_asymmetry?: number;
  min_solution_quality?: number;
  height_range?: [number, number];
  offset?: number;
  limit?: number;
}

export interface SearchResults {
  entries: MarketplaceCatalogEntry[];
  total_count: number;
  offset: number;
  limit: number;
}

function marketplaceBaseUrl(): string {
  const raw = (import.meta.env.VITE_MARKETPLACE_EXPORT_URL as string | undefined)?.trim();
  if (raw) return raw.replace(/\/$/, "");
  return "/marketplace";
}

function httpErrorDetail(response: Response): string {
  const bit = response.statusText?.trim();
  return bit ? `${response.status} ${bit}` : `HTTP ${response.status}`;
}

export class MarketplaceClient {
  private baseUrl: string;

  constructor(baseUrl?: string) {
    this.baseUrl = baseUrl ?? marketplaceBaseUrl();
  }

  async getStats(): Promise<CatalogStats> {
    const response = await fetch(`${this.baseUrl}/api/v1/stats`);
    if (!response.ok) {
      throw new Error(`Failed to fetch stats: ${httpErrorDetail(response)}`);
    }
    return response.json();
  }

  async search(criteria: SearchCriteria = {}): Promise<SearchResults> {
    const response = await fetch(`${this.baseUrl}/api/v1/catalog/search`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify(criteria),
    });
    if (!response.ok) {
      throw new Error(`Failed to search: ${httpErrorDetail(response)}`);
    }
    return response.json();
  }

  async getEntry(problemId: string): Promise<MarketplaceCatalogEntry> {
    const response = await fetch(`${this.baseUrl}/api/v1/catalog/${encodeURIComponent(problemId)}`);
    if (!response.ok) {
      throw new Error(`Failed to fetch entry: ${httpErrorDetail(response)}`);
    }
    return response.json();
  }

  async healthCheck(): Promise<{ status: string; version: string; entries_count: number }> {
    const response = await fetch(`${this.baseUrl}/health`);
    if (!response.ok) {
      throw new Error(`Health check failed: ${httpErrorDetail(response)}`);
    }
    return response.json();
  }
}
