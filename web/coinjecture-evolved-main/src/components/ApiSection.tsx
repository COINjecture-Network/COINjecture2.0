import { Card } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Copy, Check, ExternalLink } from "lucide-react";
import { useState } from "react";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";

const API_BASE = import.meta.env.VITE_RPC_URL || "http://localhost:9933";

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
    description: "Submit a signed transaction to the network",
    params: "tx_hex: string (JSON-serialized transaction)",
    requestExample: JSON.stringify({
      jsonrpc: "2.0",
      id: 7,
      method: "transaction_submit",
      params: ['{"Transfer":{"from":[1,2,3],"to":[4,5,6],"amount":1000,"fee":1500,"nonce":42,"public_key":[7,8,9],"signature":[10,11,12]}}'],
    }, null, 2),
    responseExample: JSON.stringify({
      jsonrpc: "2.0",
      id: 7,
      result: "0xtxhash...",
    }, null, 2),
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
  // Marketplace Methods
  {
    category: "Marketplace",
    method: "marketplace_getOpenProblems",
    description: "Get all open problems in the marketplace",
    params: "none",
    requestExample: JSON.stringify({
      jsonrpc: "2.0",
      id: 9,
      method: "marketplace_getOpenProblems",
      params: [],
    }, null, 2),
    responseExample: JSON.stringify({
      jsonrpc: "2.0",
      id: 9,
      result: [
        {
          problem_id: "0xabcd...",
          submitter: "0x1234...",
          bounty: 1000000,
          min_work_score: 50.0,
          status: "OPEN",
          submitted_at: 1703001234,
          expires_at: 1703087634,
          is_private: false,
          problem_type: "SubsetSum(5)",
          problem_size: 5,
          is_revealed: true,
        },
      ],
    }, null, 2),
  },
  {
    category: "Marketplace",
    method: "marketplace_getStats",
    description: "Get marketplace statistics",
    params: "none",
    requestExample: JSON.stringify({
      jsonrpc: "2.0",
      id: 10,
      method: "marketplace_getStats",
      params: [],
    }, null, 2),
    responseExample: JSON.stringify({
      jsonrpc: "2.0",
      id: 10,
      result: {
        total_problems: 100,
        open_problems: 25,
        solved_problems: 60,
        total_bounty_pool: 5000000000,
      },
    }, null, 2),
  },
];

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
          <h2 className="text-4xl font-bold mb-4">API Documentation</h2>
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
        </div>
      </div>
    </section>
  );
};
