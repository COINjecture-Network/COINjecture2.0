import { Card } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Copy, Check, ExternalLink } from "lucide-react";
import { useState } from "react";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";

import { getDefaultRpcBaseUrls } from "@/lib/rpc-client";

/** Display / examples: first RPC URL the app uses (dev: /api/rpc proxy; prod: VITE_RPC_URL). */
const API_BASE = getDefaultRpcBaseUrls()[0];

const CHAIN_ID = "coinject-network-b-v4";
/** 1 display BEANS = 10^12 atoms (all balances/bounties on RPC are atoms). */
const ONE_BEAN_ATOMS = 1_000_000_000_000;

const PROBLEM_INFO_EXAMPLE = {
  problem_id: "<hex>",
  submitter: "<hex_address>",
  title: "SubsetSum bounty",
  briefing: "Return indices summing to target.",
  bounty: ONE_BEAN_ATOMS,
  min_work_score: 10.0,
  status: "Open",
  submitted_at: 1703001234,
  expires_at: 1703087634,
  is_private: false,
  problem_type: "SubsetSum(8)",
  problem_size: 8,
  is_revealed: true,
};

const API_NOTES =
  `Chain ID: ${CHAIN_ID}. Balances and bounties are in atoms (${ONE_BEAN_ATOMS} = 1 BEANS). ` +
  "Hashes and addresses are hex without a 0x prefix. " +
  "Wallet-backed bounty posts deduct bounty + 1 BEANS submission fee. " +
  "Full method list: rpc/src/server.rs (mining, network, timelock, escrow, channel, faucet).";

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
    description: "Get account balance in atoms (10^12 atoms = 1 BEANS)",
    params: "address: string (64-char hex address)",
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
    description: "Chain tip, peers, sync status, and optional mining fields (when enabled)",
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
        chain_id: CHAIN_ID,
        best_height: 12345,
        best_hash: "abcd...",
        genesis_hash: "0000...",
        peer_count: 3,
        total_work: 987654,
        is_syncing: false,
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
          prev_hash: "abcd...",
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
          prev_hash: "abcd...",
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
      params: ["<tx_hash_hex>"],
    }, null, 2),
    responseExample: JSON.stringify({
      jsonrpc: "2.0",
      id: 8,
      result: {
        tx_hash: "<tx_hash_hex>",
        status: "confirmed",
        block_height: 12345,
      },
    }, null, 2),
  },
  // Marketplace Methods (rpc/src/server.rs — JSON-RPC)
  {
    category: "Marketplace",
    method: "marketplace_getOpenProblems",
    description: "List open marketplace problems (ProblemInfo). Status values: Open, Solved, Expired, Cancelled.",
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
        result: [PROBLEM_INFO_EXAMPLE],
      },
      null,
      2
    ),
  },
  {
    category: "Marketplace",
    method: "marketplace_getProblem",
    description: "Get one problem by ID. Solved listings may include solver and solution fields.",
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
        result: { ...PROBLEM_INFO_EXAMPLE, problem_id: "<problem_id_hex>" },
      },
      null,
      2
    ),
  },
  {
    category: "Marketplace",
    method: "marketplace_getStats",
    description: "Marketplace aggregate counters.",
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
    description: "Submit a private bounty (commitment + ZK proof). Advanced — prefer marketplace_submitPrivateProblemWithWallet for wallet flows.",
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
      "Post a public SubsetSum bounty (escrow + 1 BEANS fee). Generic problems: marketplace_submitPublicProblem.",
    params: "params: PublicSubsetSumParams — bounty in atoms; optional title/briefing on PublicProblemParams",
    requestExample: JSON.stringify(
      {
        jsonrpc: "2.0",
        method: "marketplace_submitPublicSubsetSum",
        params: [
          {
            numbers: [15, 22, 14, 26, 32, 9, 16, 8],
            target: 53,
            bounty: ONE_BEAN_ATOMS,
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
      "Submit a typed Solution for a listing. Verified on-node; bounty settles when valid.",
    params: "params: { problem_id, solution: Solution, solver }",
    requestExample: JSON.stringify(
      {
        jsonrpc: "2.0",
        method: "marketplace_submitSolution",
        params: [
          {
            problem_id: "<problem_id_hex>",
            solution: { SubsetSum: [0, 1, 6] },
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
        1_000_000_000_000, // bounty (atoms; 1 BEANS)
        10.0,                // min work score
        30,                  // expiration (days)
        Some("Title".into()),
        Some("Briefing".into()),
        1_000_000_000_000,   // fee: min 1 BEANS for bounty posts
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
        0,        // fee (solution submit may be 0)
        nonce,
        &keypair,
    )
);
rpc_client
    .submit_transaction(hex::encode(bincode::serialize(&tx)?))
    .await?;`;

const GHCR_PACKAGE_URL =
  "https://github.com/COINjecture-Network/COINjecture2.0/pkgs/container/coinjecture2.0";
const GHCR_NODE_IMAGE = "ghcr.io/coinjecture-network/coinjecture2.0";

const GHCR_DEPLOY_EXAMPLES = `# Prebuilt COINjecture node image (GitHub Container Registry)
# Package: ${GHCR_PACKAGE_URL}

# 1) Authenticate if the package is private (PAT needs read:packages)
docker login ghcr.io

# 2) Pull — :latest tracks main CI builds; pin :sha-<commit> for production
docker pull ${GHCR_NODE_IMAGE}:latest
# docker pull ${GHCR_NODE_IMAGE}:sha-83a635f

# 3) Mesh host .env (see repo .env.example — never commit secrets)
# COINJECT_NODE_IMAGE=${GHCR_NODE_IMAGE}:latest
# HF_TOKEN=hf_...                         # optional: stream blocks to Hugging Face
# HF_DATASET_NAME=COINjecture/NP-Solutions
# COINJECT_BOOTNODES=<peer-ip>:707

# 4) Start without compiling the node image locally
docker compose pull bootnode api-server
docker compose up -d --no-build bootnode api-server

# JSON-RPC on the bootnode container: http://<host>:9933
# REST API (separate image): http://<host>:3030`;

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
          <p className="text-muted-foreground mb-3">JSON-RPC 2.0 — Network B v4 ({CHAIN_ID})</p>
          <p className="text-sm text-muted-foreground max-w-2xl mx-auto mb-6">{API_NOTES}</p>
          <div className="flex items-center justify-center gap-4">
            <code className="text-sm bg-terminal-bg text-terminal-text px-4 py-2 rounded-lg terminal-font">
              {API_BASE}
            </code>
            <Button variant="outline" size="sm">
              Test Connection <ExternalLink className="ml-2 h-3 w-3" />
            </Button>
          </div>
        </div>

        <div id="run-a-node" className="max-w-5xl mx-auto mb-12 scroll-mt-24">
          <Card className="glass-effect overflow-hidden">
            <div className="p-6">
              <div className="flex flex-wrap items-start justify-between gap-3 mb-3">
                <div>
                  <h3 className="text-2xl font-bold mb-1">Node package (GHCR)</h3>
                  <p className="text-sm text-muted-foreground max-w-2xl">
                    Run a mesh bootnode or follower without building Rust locally. Images are published by CI to{" "}
                    <code className="text-xs">{GHCR_NODE_IMAGE}</code> — see tagged builds on GitHub Packages.
                  </p>
                </div>
                <Button variant="outline" size="sm" asChild>
                  <a href={GHCR_PACKAGE_URL} target="_blank" rel="noopener noreferrer">
                    Open package <ExternalLink className="ml-2 h-3 w-3" />
                  </a>
                </Button>
              </div>
              <div className="relative">
                <pre className="bg-terminal-bg text-terminal-text p-4 rounded-lg overflow-x-auto text-xs terminal-font whitespace-pre-wrap">
                  {GHCR_DEPLOY_EXAMPLES}
                </pre>
                <button
                  type="button"
                  onClick={() => copyCode(GHCR_DEPLOY_EXAMPLES, 4999)}
                  className="absolute top-2 right-2 p-2 hover:bg-muted/20 rounded transition-colors"
                  aria-label="Copy GHCR deploy examples"
                >
                  {copiedIndex === 4999 ? (
                    <Check className="h-4 w-4 text-success" />
                  ) : (
                    <Copy className="h-4 w-4 text-muted-foreground" />
                  )}
                </button>
              </div>
            </div>
          </Card>
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
