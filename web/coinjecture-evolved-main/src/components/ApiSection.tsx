import { Card } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Copy, Check, ExternalLink } from "lucide-react";
import { useState } from "react";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";

import { getDefaultRpcBaseUrls } from "@/lib/rpc-client";

/** Display / examples: first RPC URL the app uses (dev: /api/rpc proxy; prod: VITE_RPC_URL). */
const API_BASE = getDefaultRpcBaseUrls()[0];

interface RpcEndpoint {
  category: string;
  method: string;
  description: string;
  params: string;
  requestExample: string;
  responseExample: string;
}

const endpoints: RpcEndpoint[] = [
  // Account Methods
  {
    category: "Account",
    method: "account_getBalance",
    description: "Get account balance for an address",
    params: "address: string (hex-encoded 64-char address)",
    requestExample: JSON.stringify({
      jsonrpc: "2.0",
      id: 1,
      method: "account_getBalance",
      params: ["a1b2c3d4e5f6..."],
    }, null, 2),
    responseExample: JSON.stringify({
      jsonrpc: "2.0",
      id: 1,
      result: 1000000000,
    }, null, 2),
  },
  {
    category: "Account",
    method: "account_getNonce",
    description: "Get account nonce for transaction ordering",
    params: "address: string",
    requestExample: JSON.stringify({
      jsonrpc: "2.0",
      id: 2,
      method: "account_getNonce",
      params: ["a1b2c3d4e5f6..."],
    }, null, 2),
    responseExample: JSON.stringify({
      jsonrpc: "2.0",
      id: 2,
      result: 42,
    }, null, 2),
  },
  {
    category: "Account",
    method: "account_getInfo",
    description: "Get complete account information (balance and nonce)",
    params: "address: string",
    requestExample: JSON.stringify({
      jsonrpc: "2.0",
      id: 3,
      method: "account_getInfo",
      params: ["a1b2c3d4e5f6..."],
    }, null, 2),
    responseExample: JSON.stringify({
      jsonrpc: "2.0",
      id: 3,
      result: {
        address: "a1b2c3d4e5f6...",
        balance: 1000000000,
        nonce: 42,
      },
    }, null, 2),
  },
  // Chain Methods
  {
    category: "Chain",
    method: "chain_getInfo",
    description: "Get blockchain information (height, hash, peers)",
    params: "none",
    requestExample: JSON.stringify({
      jsonrpc: "2.0",
      id: 4,
      method: "chain_getInfo",
      params: [],
    }, null, 2),
    responseExample: JSON.stringify({
      jsonrpc: "2.0",
      id: 4,
      result: {
        chain_id: "coinjecture-netb",
        best_height: 12345,
        best_hash: "0xabcd...",
        genesis_hash: "0x0000...",
        peer_count: 3,
      },
    }, null, 2),
  },
  {
    category: "Chain",
    method: "chain_getBlock",
    description: "Get block by height",
    params: "height: number",
    requestExample: JSON.stringify({
      jsonrpc: "2.0",
      id: 5,
      method: "chain_getBlock",
      params: [12345],
    }, null, 2),
    responseExample: JSON.stringify({
      jsonrpc: "2.0",
      id: 5,
      result: {
        header: {
          height: 12345,
          prev_hash: "0xabcd...",
          timestamp: 1703001234,
          work_score: 150.5,
        },
        transactions: [],
        solution_reveal: {
          problem: { SubsetSum: { numbers: [1, 2, 3], target: 5 } },
          solution: { SubsetSum: [0, 2] },
        },
      },
    }, null, 2),
  },
  {
    category: "Chain",
    method: "chain_getLatestBlock",
    description: "Get the latest block in the chain",
    params: "none",
    requestExample: JSON.stringify({
      jsonrpc: "2.0",
      id: 6,
      method: "chain_getLatestBlock",
      params: [],
    }, null, 2),
    responseExample: JSON.stringify({
      jsonrpc: "2.0",
      id: 6,
      result: {
        header: {
          height: 12345,
          prev_hash: "0xabcd...",
          timestamp: 1703001234,
          work_score: 150.5,
        },
        transactions: [],
        solution_reveal: {
          problem: { SubsetSum: { numbers: [1, 2, 3], target: 5 } },
          solution: { SubsetSum: [0, 2] },
        },
      },
    }, null, 2),
  },
  // Transaction Methods
  {
    category: "Transaction",
    method: "transaction_submit",
    description:
      "Submit a signed transaction. Accepts hex-encoded bincode (CLI / Rust `hex::encode(bincode::serialize(&tx)?)`) or a JSON object string for web-wallet flows. Verifies signature server-side.",
    params: "tx_hex: string — hex (bincode) or JSON starting with '{'",
    requestExample: JSON.stringify(
      {
        jsonrpc: "2.0",
        method: "transaction_submit",
        params: ["<signed_marketplace_tx_hex>"],
        id: 1,
      },
      null,
      2
    ),
    responseExample: JSON.stringify(
      {
        jsonrpc: "2.0",
        id: 1,
        result: "<tx_hash_hex>",
      },
      null,
      2
    ),
  },
  {
    category: "Transaction",
    method: "transaction_getStatus",
    description: "Get transaction status by hash",
    params: "tx_hash: string",
    requestExample: JSON.stringify({
      jsonrpc: "2.0",
      id: 8,
      method: "transaction_getStatus",
      params: ["0xtxhash..."],
    }, null, 2),
    responseExample: JSON.stringify({
      jsonrpc: "2.0",
      id: 8,
      result: {
        tx_hash: "0xtxhash...",
        status: "confirmed",
        block_height: 12345,
      },
    }, null, 2),
  },
  // Marketplace Methods (rpc/src/server.rs — JSON-RPC)
  {
    category: "Marketplace",
    method: "marketplace_getOpenProblems",
    description: "Returns all open problems (`Vec<ProblemInfo>`). Matches `CoinjectureRpcServer::get_open_problems`.",
    params: "none",
    requestExample: JSON.stringify(
      {
        jsonrpc: "2.0",
        method: "marketplace_getOpenProblems",
        params: [],
        id: 1,
      },
      null,
      2
    ),
    responseExample: JSON.stringify(
      {
        jsonrpc: "2.0",
        id: 1,
        result: [
          {
            problem_id: "<hex>",
            submitter: "<hex_address>",
            bounty: 1000000,
            min_work_score: 50.0,
            status: "OPEN",
            submitted_at: 1703001234,
            expires_at: 1703087634,
            is_private: false,
            problem_type: "SubsetSum(8)",
            problem_size: 8,
            is_revealed: true,
          },
        ],
      },
      null,
      2
    ),
  },
  {
    category: "Marketplace",
    method: "marketplace_getProblem",
    description: "Get a single problem by ID (`Option<ProblemInfo>`).",
    params: "problem_id: string (hex problem id)",
    requestExample: JSON.stringify(
      {
        jsonrpc: "2.0",
        method: "marketplace_getProblem",
        params: ["<problem_id_hex>"],
        id: 1,
      },
      null,
      2
    ),
    responseExample: JSON.stringify(
      {
        jsonrpc: "2.0",
        id: 1,
        result: {
          problem_id: "<problem_id_hex>",
          submitter: "<hex_address>",
          bounty: 1000,
          min_work_score: 10.0,
          status: "OPEN",
          submitted_at: 1703001234,
          expires_at: 1703087634,
          is_private: false,
          problem_type: "SubsetSum(8)",
          problem_size: 8,
          is_revealed: true,
        },
      },
      null,
      2
    ),
  },
  {
    category: "Marketplace",
    method: "marketplace_getStats",
    description: "Aggregate marketplace counters (`coinject_state::MarketplaceStats`).",
    params: "none",
    requestExample: JSON.stringify(
      {
        jsonrpc: "2.0",
        method: "marketplace_getStats",
        params: [],
        id: 1,
      },
      null,
      2
    ),
    responseExample: JSON.stringify(
      {
        jsonrpc: "2.0",
        id: 1,
        result: {
          total_problems: 100,
          open_problems: 25,
          solved_problems: 60,
          expired_problems: 10,
          cancelled_problems: 5,
          total_bounty_pool: 5000000000,
        },
      },
      null,
      2
    ),
  },
  {
    category: "Marketplace",
    method: "marketplace_submitPrivateProblem",
    description: "Submit a private bounty: commitment + ZK proof (`PrivateProblemParams`). Returns problem_id string.",
    params: "params: PrivateProblemParams object",
    requestExample: JSON.stringify(
      {
        jsonrpc: "2.0",
        method: "marketplace_submitPrivateProblem",
        params: [
          {
            commitment: "<hex>",
            proof_bytes: "<hex>",
            vk_hash: "<hex>",
            public_inputs: ["<hex>"],
            problem_type: "SubsetSum",
            size: 8,
            complexity_estimate: 10.0,
            bounty: 1000,
            min_work_score: 10.0,
            expiration_days: 30,
          },
        ],
        id: 1,
      },
      null,
      2
    ),
    responseExample: JSON.stringify(
      {
        jsonrpc: "2.0",
        id: 1,
        result: "<problem_id_hex>",
      },
      null,
      2
    ),
  },
  {
    category: "Marketplace",
    method: "marketplace_revealProblem",
    description: "Reveal problem for a private bounty (`RevealParams`: JSON ProblemType + 32-byte salt hex).",
    params: "params: RevealParams",
    requestExample: JSON.stringify(
      {
        jsonrpc: "2.0",
        method: "marketplace_revealProblem",
        params: [
          {
            problem_id: "<problem_id_hex>",
            problem: '{"SubsetSum":{"numbers":[15,22,14,26,32,9,16,8],"target":53}}',
            salt: "<64_hex_chars>",
          },
        ],
        id: 1,
      },
      null,
      2
    ),
    responseExample: JSON.stringify(
      {
        jsonrpc: "2.0",
        id: 1,
        result: true,
      },
      null,
      2
    ),
  },
  {
    category: "Marketplace",
    method: "marketplace_submitPublicSubsetSum",
    description:
      "Phase-2 MVP: post a public SubsetSum instance + escrow (`PublicSubsetSumParams`). Bounty is locked when accepted.",
    params: "params: PublicSubsetSumParams",
    requestExample: JSON.stringify(
      {
        jsonrpc: "2.0",
        method: "marketplace_submitPublicSubsetSum",
        params: [
          {
            numbers: [15, 22, 14, 26, 32, 9, 16, 8],
            target: 53,
            bounty: 1000,
            min_work_score: 10.0,
            expiration_days: 30,
            submitter: "<your_address_hex>",
          },
        ],
        id: 1,
      },
      null,
      2
    ),
    responseExample: JSON.stringify(
      {
        jsonrpc: "2.0",
        id: 1,
        result: "<problem_id_hex>",
      },
      null,
      2
    ),
  },
  {
    category: "Marketplace",
    method: "marketplace_submitSolution",
    description:
      "Submit indices solving a public SubsetSum listing (`SolutionSubmissionParams`). Node verifies in polynomial time and settles bounty when valid.",
    params: "params: SolutionSubmissionParams",
    requestExample: JSON.stringify(
      {
        jsonrpc: "2.0",
        method: "marketplace_submitSolution",
        params: [
          {
            problem_id: "<problem_id_hex>",
            selected_indices: [0, 1, 6],
            solver: "<solver_address_hex>",
          },
        ],
        id: 1,
      },
      null,
      2
    ),
    responseExample: JSON.stringify(
      {
        jsonrpc: "2.0",
        id: 1,
        result: true,
      },
      null,
      2
    ),
  },
];

const CURL_BASE = `curl -X POST ${API_BASE} -H "Content-Type: application/json" -d`;

const MARKETPLACE_CURL_EXAMPLES = `${CURL_BASE} '{
  "jsonrpc": "2.0",
  "method": "marketplace_getOpenProblems",
  "params": [],
  "id": 1
}'

${CURL_BASE} '{
  "jsonrpc": "2.0",
  "method": "marketplace_getProblem",
  "params": ["<problem_id_hex>"],
  "id": 1
}'

${CURL_BASE} '{
  "jsonrpc": "2.0",
  "method": "marketplace_getStats",
  "params": [],
  "id": 1
}'

${CURL_BASE} '{
  "jsonrpc": "2.0",
  "method": "transaction_submit",
  "params": ["<signed_marketplace_tx_hex>"],
  "id": 1
}'`;

const RUST_MARKETPLACE_TX_EXAMPLES = `// Low-level: serialize \`Transaction::Marketplace\` and submit hex (matches node bincode path).
// See core/src/transaction.rs — \`MarketplaceTransaction\` and RPC \`transaction_submit\`.

use coinject_core::{ProblemType, Transaction, MarketplaceTransaction};

// --- Example: submitting a problem (SubsetSum → escrow) ---
let problem = ProblemType::SubsetSum {
    numbers: vec![15, 22, 14, 26, 32, 9, 16, 8],
    target: 53,
};

let tx = Transaction::Marketplace(
    MarketplaceTransaction::new_problem_submission(
        problem,
        your_address,
        1000,     // bounty
        10.0,     // min work score
        30,       // expiration (days)
        10,       // fee
        nonce,
        &keypair,
    )
);
rpc_client
    .submit_transaction(hex::encode(bincode::serialize(&tx)?))
    .await?;

// --- Example: submitting a solution (verified + bounty path in same block) ---
let solution = coinject_core::Solution::SubsetSum(vec![0, 1, 6]); // 15+22+16 = 53

let tx = Transaction::Marketplace(
    MarketplaceTransaction::new_solution_submission(
        problem_id,
        solution,
        solver_address,
        10,       // fee
        nonce,
        &keypair,
    )
);
rpc_client
    .submit_transaction(hex::encode(bincode::serialize(&tx)?))
    .await?;`;

export const ApiSection = () => {
  const [copiedIndex, setCopiedIndex] = useState<number | null>(null);

  const copyCode = (code: string, index: number) => {
    navigator.clipboard.writeText(code);
    setCopiedIndex(index);
    setTimeout(() => setCopiedIndex(null), 2000);
  };

  return (
    <section id="api" className="py-20 bg-gradient-to-b from-background to-muted/20">
      <div className="container mx-auto px-6">
        <div className="text-center mb-12">
          <h2 className="text-4xl font-bold mb-4">
            API <span className="text-primary">Documentation</span>
          </h2>
          <p className="text-muted-foreground mb-6">JSON-RPC 2.0 API for COINjecture Network B</p>
          <div className="flex items-center justify-center gap-4">
            <code className="text-sm bg-terminal-bg text-terminal-text px-4 py-2 rounded-lg terminal-font">
              {API_BASE}
            </code>
            <Button variant="outline" size="sm">
              Test Connection <ExternalLink className="ml-2 h-3 w-3" />
            </Button>
          </div>
        </div>

        <div className="max-w-5xl mx-auto space-y-8">
          {["Account", "Chain", "Transaction", "Marketplace"].map((category) => {
            const categoryEndpoints = endpoints.filter(e => e.category === category);
            if (categoryEndpoints.length === 0) return null;
            
            return (
              <div key={category}>
                <h3 className="text-2xl font-bold mb-4">{category} Methods</h3>
                <div className="space-y-6">
                  {categoryEndpoints.map((endpoint, index) => {
                    const fullIndex = endpoints.indexOf(endpoint);
                    return (
                      <Card key={fullIndex} className="glass-effect overflow-hidden">
                        <div className="p-6">
                          <div className="flex flex-wrap items-start gap-4 mb-4">
                            <span className="text-xs font-semibold px-3 py-1 rounded-full bg-primary/20 text-primary">
                              JSON-RPC
                            </span>
                            <code className="text-sm text-foreground terminal-font flex-1">
                              {endpoint.method}
                            </code>
                          </div>
                          <p className="text-sm text-muted-foreground mb-2">{endpoint.description}</p>
                          <p className="text-xs text-muted-foreground mb-4">
                            <strong>Parameters:</strong> {endpoint.params}
                          </p>
                          
                          <Tabs defaultValue="request" className="w-full">
                            <TabsList className="grid w-full max-w-md grid-cols-2">
                              <TabsTrigger value="request">Request</TabsTrigger>
                              <TabsTrigger value="response">Response</TabsTrigger>
                            </TabsList>
                            <TabsContent value="request" className="mt-4">
                              <div className="relative">
                                <pre className="bg-terminal-bg text-terminal-text p-4 rounded-lg overflow-x-auto text-xs terminal-font">
                                  {endpoint.requestExample}
                                </pre>
                                <button
                                  onClick={() => copyCode(endpoint.requestExample, fullIndex)}
                                  className="absolute top-2 right-2 p-2 hover:bg-muted/20 rounded transition-colors"
                                >
                                  {copiedIndex === fullIndex ? (
                                    <Check className="h-4 w-4 text-success" />
                                  ) : (
                                    <Copy className="h-4 w-4 text-muted-foreground" />
                                  )}
                                </button>
                              </div>
                            </TabsContent>
                            <TabsContent value="response" className="mt-4">
                              <div className="relative">
                                <pre className="bg-terminal-bg text-terminal-text p-4 rounded-lg overflow-x-auto text-xs terminal-font">
                                  {endpoint.responseExample}
                                </pre>
                                <button
                                  onClick={() => copyCode(endpoint.responseExample, fullIndex + 1000)}
                                  className="absolute top-2 right-2 p-2 hover:bg-muted/20 rounded transition-colors"
                                >
                                  {copiedIndex === fullIndex + 1000 ? (
                                    <Check className="h-4 w-4 text-success" />
                                  ) : (
                                    <Copy className="h-4 w-4 text-muted-foreground" />
                                  )}
                                </button>
                              </div>
                            </TabsContent>
                          </Tabs>
                        </div>
                      </Card>
                    );
                  })}
                </div>
              </div>
            );
          })}

          <div className="mt-16 space-y-8 border-t border-border/60 pt-12">
            <div>
              <h3 className="text-2xl font-bold mb-2">Marketplace API (curl)</h3>
              <p className="text-sm text-muted-foreground mb-4">
                Same JSON-RPC surface as <code className="text-xs">rpc/src/server.rs</code>. Replace{" "}
                <code className="text-xs">{API_BASE}</code> with your node (default dev:{" "}
                <code className="text-xs">http://localhost:9933</code>).
              </p>
              <Card className="glass-effect overflow-hidden">
                <div className="p-6 relative">
                  <pre className="bg-terminal-bg text-terminal-text p-4 rounded-lg overflow-x-auto text-xs terminal-font whitespace-pre-wrap">
                    {MARKETPLACE_CURL_EXAMPLES}
                  </pre>
                  <button
                    type="button"
                    onClick={() => copyCode(MARKETPLACE_CURL_EXAMPLES, 5000)}
                    className="absolute top-8 right-8 p-2 hover:bg-muted/20 rounded transition-colors"
                    aria-label="Copy curl examples"
                  >
                    {copiedIndex === 5000 ? (
                      <Check className="h-4 w-4 text-success" />
                    ) : (
                      <Copy className="h-4 w-4 text-muted-foreground" />
                    )}
                  </button>
                </div>
              </Card>
            </div>

            <div>
              <h3 className="text-2xl font-bold mb-2">Rust: marketplace transactions via RPC</h3>
              <p className="text-sm text-muted-foreground mb-4">
                Build a <code className="text-xs">coinject_core::Transaction::Marketplace</code>, bincode-encode, hex-encode,
                and pass the string to <code className="text-xs">transaction_submit</code> (same as CLI wallet). On
                acceptance, solution verification and bounty settlement run in-block.
              </p>
              <Card className="glass-effect overflow-hidden">
                <div className="p-6 relative">
                  <pre className="bg-terminal-bg text-terminal-text p-4 rounded-lg overflow-x-auto text-xs terminal-font whitespace-pre-wrap">
                    {RUST_MARKETPLACE_TX_EXAMPLES}
                  </pre>
                  <button
                    type="button"
                    onClick={() => copyCode(RUST_MARKETPLACE_TX_EXAMPLES, 5001)}
                    className="absolute top-8 right-8 p-2 hover:bg-muted/20 rounded transition-colors"
                    aria-label="Copy Rust examples"
                  >
                    {copiedIndex === 5001 ? (
                      <Check className="h-4 w-4 text-success" />
                    ) : (
                      <Copy className="h-4 w-4 text-muted-foreground" />
                    )}
                  </button>
                </div>
              </Card>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
};
