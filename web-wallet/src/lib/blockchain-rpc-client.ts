// Blockchain RPC Client for submitting bounties and interacting with the marketplace
// Interfaces with the node RPC on port 9933

export interface ProblemInfo {
  problem_id: string;
  submitter: string;
  bounty: number;
  min_work_score: number;
  expires_at: number;
  status: 'Open' | 'Solved' | 'Expired';
  solver: string | null;
  solution_hash: string | null;
  submitted_at: number;
  is_private: boolean;
  problem_type: string | null;
  problem_size: number | null;
  is_revealed: boolean;
}

export interface ProblemType {
  SubsetSum?: {
    numbers: number[];
    target: number;
  };
  SAT?: {
    variables: number;
    clauses: Array<{ literals: number[] }>;
  };
  TSP?: {
    cities: number;
    distances: number[][];
  };
}

export interface PrivateProblemParams {
  commitment: string;
  proof_bytes: string;
  vk_hash: string;
  public_inputs: string[];
  problem_type: string;
  size: number;
  complexity_estimate: number;
  bounty: number;
  min_work_score: number;
  expiration_days: number;
}

export interface RevealParams {
  problem_id: string;
  problem: ProblemType;
  salt: string;
}

export class BlockchainRPCClient {
  private endpoint: string;
  private requestId: number;

  constructor(endpoint: string = 'http://127.0.0.1:9933') {
    this.endpoint = endpoint;
    this.requestId = 0;
  }

  private async call(method: string, params: any[] = []): Promise<any> {
    const response = await fetch(this.endpoint, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        jsonrpc: '2.0',
        id: ++this.requestId,
        method,
        params,
      }),
    });

    if (!response.ok) {
      throw new Error(`RPC request failed: ${response.statusText}`);
    }

    const result = await response.json();

    if (result.error) {
      throw new Error(`RPC error: ${result.error.message || JSON.stringify(result.error)}`);
    }

    return result.result;
  }

  // Marketplace methods
  async getMarketplaceStats() {
    return this.call('marketplace_getStats');
  }

  async getOpenProblems(): Promise<ProblemInfo[]> {
    return this.call('marketplace_getOpenProblems');
  }

  async getProblem(problemId: string): Promise<ProblemInfo | null> {
    return this.call('marketplace_getProblem', [problemId]);
  }

  async submitPublicProblem(
    problem: ProblemType,
    bounty: number,
    minWorkScore: number,
    expirationDays: number
  ): Promise<string> {
    return this.call('marketplace_submitPublicProblem', [
      {
        problem,
        bounty,
        min_work_score: minWorkScore,
        expiration_days: expirationDays,
      },
    ]);
  }

  async submitPrivateProblem(params: PrivateProblemParams): Promise<string> {
    return this.call('marketplace_submitPrivateProblem', [params]);
  }

  async revealProblem(params: RevealParams): Promise<boolean> {
    return this.call('marketplace_revealProblem', [params]);
  }

  async submitSolution(
    problemId: string,
    solution: any
  ): Promise<boolean> {
    return this.call('marketplace_submitSolution', [
      {
        problem_id: problemId,
        solution,
      },
    ]);
  }

  // Wallet methods
  async getBalance(address: string): Promise<number> {
    return this.call('wallet_getBalance', [address]);
  }

  async getAddress(): Promise<string> {
    return this.call('wallet_getAddress');
  }
}
