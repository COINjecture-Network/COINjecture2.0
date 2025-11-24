import { Card } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Copy, Check, ExternalLink } from "lucide-react";
import { useState } from "react";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";

const API_BASE = import.meta.env.VITE_RPC_URL || "http://localhost:9933";

const endpoints = [
  {
    method: "GET",
    path: "/v1/metrics/dashboard",
    description: "Comprehensive blockchain metrics with dynamic gas data",
    example: `{
  "totalBlocks": 12847,
  "activeMiners": 1284,
  "networkHashrate": "245 TH/s",
  "avgGasPrice": "125000"
}`
  },
  {
    method: "GET",
    path: "/v1/data/block/latest",
    description: "Latest block with dynamic gas calculation",
    example: `{
  "index": 12847,
  "hash": "0x7f9c...",
  "timestamp": 1703001234,
  "reward": 2.5
}`
  },
  {
    method: "POST",
    path: "/v1/ingest/block",
    description: "Submit blocks with IPFS-based gas calculation",
    example: `{
  "minerAddress": "0xBEANS0HYM75LZ",
  "data": "...",
  "nonce": 12345
}`
  },
  {
    method: "GET",
    path: "/v1/rewards/{address}",
    description: "User rewards & wallet data with gas integration",
    example: `{
  "address": "0xBEANS0HYM75LZ",
  "balance": 127.50,
  "pending": 2.5
}`
  }
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
          <p className="text-muted-foreground mb-6">RESTful API with dynamic gas calculation system</p>
          <div className="flex items-center justify-center gap-4">
            <code className="text-sm bg-terminal-bg text-terminal-text px-4 py-2 rounded-lg terminal-font">
              {API_BASE}
            </code>
            <Button variant="outline" size="sm">
              Test Connection <ExternalLink className="ml-2 h-3 w-3" />
            </Button>
          </div>
        </div>

        <div className="max-w-5xl mx-auto space-y-6">
          {endpoints.map((endpoint, index) => (
            <Card key={index} className="glass-effect overflow-hidden">
              <div className="p-6">
                <div className="flex flex-wrap items-start gap-4 mb-4">
                  <span className={`text-xs font-semibold px-3 py-1 rounded-full ${
                    endpoint.method === "GET" ? "bg-primary/20 text-primary" : "bg-secondary/20 text-secondary"
                  }`}>
                    {endpoint.method}
                  </span>
                  <code className="text-sm text-foreground terminal-font flex-1">
                    {endpoint.path}
                  </code>
                </div>
                <p className="text-sm text-muted-foreground mb-4">{endpoint.description}</p>
                
                <Tabs defaultValue="response" className="w-full">
                  <TabsList className="grid w-full max-w-md grid-cols-2">
                    <TabsTrigger value="response">Response</TabsTrigger>
                    <TabsTrigger value="curl">cURL</TabsTrigger>
                  </TabsList>
                  <TabsContent value="response" className="mt-4">
                    <div className="relative">
                      <pre className="bg-terminal-bg text-terminal-text p-4 rounded-lg overflow-x-auto text-xs terminal-font">
                        {endpoint.example}
                      </pre>
                      <button
                        onClick={() => copyCode(endpoint.example, index)}
                        className="absolute top-2 right-2 p-2 hover:bg-muted/20 rounded transition-colors"
                      >
                        {copiedIndex === index ? (
                          <Check className="h-4 w-4 text-success" />
                        ) : (
                          <Copy className="h-4 w-4 text-muted-foreground" />
                        )}
                      </button>
                    </div>
                  </TabsContent>
                  <TabsContent value="curl" className="mt-4">
                    <div className="relative">
                      <pre className="bg-terminal-bg text-terminal-text p-4 rounded-lg overflow-x-auto text-xs terminal-font">
                        {`curl -X ${endpoint.method} \\
  "${API_BASE}${endpoint.path}" \\
  -H "Content-Type: application/json"`}
                      </pre>
                      <button
                        onClick={() => copyCode(`curl -X ${endpoint.method} "${API_BASE}${endpoint.path}" -H "Content-Type: application/json"`, index + 100)}
                        className="absolute top-2 right-2 p-2 hover:bg-muted/20 rounded transition-colors"
                      >
                        {copiedIndex === index + 100 ? (
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
          ))}
        </div>
      </div>
    </section>
  );
};
