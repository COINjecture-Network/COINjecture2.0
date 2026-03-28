import { Card } from "@/components/ui/card";
import { TrendingUp, Users, Activity, Target, Award, BarChart3, Network, Loader2 } from "lucide-react";
import { LiveSolutionFeed } from "./LiveSolutionFeed";
import { useQuery } from "@tanstack/react-query";
import { rpcClient } from "@/lib/rpc-client";

export const MetricsSection = () => {
  const { data: chainInfo, isLoading: chainLoading, isError: chainError } = useQuery({
    queryKey: ['chain-info'],
    queryFn: () => rpcClient.getChainInfo(),
    refetchInterval: 10000, // Refresh every 10 seconds
  });

  const { data: marketplaceStats, isLoading: statsLoading, isError: statsError } = useQuery({
    queryKey: ['marketplace-stats'],
    queryFn: () => rpcClient.getMarketplaceStats(),
    refetchInterval: 30000, // Refresh every 30 seconds
  });

  // Avoid blocking the whole section: chain and marketplace load independently (previously OR kept spinner forever if one query hung).
  const chainPending = chainLoading && !chainInfo;
  const statsPending = statsLoading && !marketplaceStats;

  return (
    <section id="metrics" className="py-20 relative">
      <div className="container mx-auto px-6">
        <div className="text-center mb-12">
          <h2 className="text-4xl font-bold mb-4">
            Live <span className="text-primary">Network Metrics</span>
          </h2>
          <p className="text-muted-foreground">Real-time blockchain and marketplace statistics</p>
          <a 
            href="https://huggingface.co/datasets/COINjecture/NP_Solutions" 
            target="_blank" 
            rel="noopener noreferrer"
            className="text-sm text-primary hover:underline mt-2 inline-block"
          >
            View NP_Solutions Dataset on HuggingFace →
          </a>
        </div>

        {chainError && (
          <p className="text-center text-destructive text-sm mb-6 max-w-xl mx-auto">
            {import.meta.env.DEV ? (
              <>
                Could not reach JSON-RPC (dev proxy targets the first URL in{" "}
                <code className="rounded bg-muted px-1 py-0.5 text-xs">VITE_RPC_URL</code> from{" "}
                <code className="rounded bg-muted px-1 py-0.5 text-xs">.env</code>
                ). Check network/firewall, or run a node on{" "}
                <code className="rounded bg-muted px-1 py-0.5 text-xs">127.0.0.1:9933</code> and add{" "}
                <code className="rounded bg-muted px-1 py-0.5 text-xs">.env.development.local</code> with{" "}
                <code className="rounded bg-muted px-1 py-0.5 text-xs">VITE_RPC_URL=http://127.0.0.1:9933</code>
                , then restart the dev server.
              </>
            ) : (
              <>
                Could not load chain metrics. The browser calls URLs from{" "}
                <code className="rounded bg-muted px-1 py-0.5 text-xs">VITE_RPC_URL</code> (see build). If this
                persists, check those endpoints are up and return CORS for{" "}
                <code className="rounded bg-muted px-1 py-0.5 text-xs">https://coinjecture.com</code> (or use{" "}
                <code className="rounded bg-muted px-1 py-0.5 text-xs">*</code> on the RPC server).
              </>
            )}
          </p>
        )}

        {/* Chain Information */}
        {chainPending && !chainError && (
          <div className="flex justify-center py-8">
            <Loader2 className="h-8 w-8 animate-spin text-primary" aria-label="Loading chain metrics" />
          </div>
        )}

        {chainInfo && (
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-5 gap-6 mb-12">
            <Card className="glass-effect p-6">
              <div className="flex items-center gap-3 mb-2">
                <Network className="h-6 w-6 text-primary" />
                <p className="text-sm text-muted-foreground">Chain ID</p>
              </div>
              <p className="text-xl font-bold">{chainInfo.chain_id}</p>
            </Card>
            <Card className="glass-effect p-6">
              <div className="flex items-center gap-3 mb-2">
                <Activity className="h-6 w-6 text-primary" />
                <p className="text-sm text-muted-foreground">Block Height</p>
              </div>
              <p className="text-xl font-bold">{chainInfo.best_height.toLocaleString()}</p>
            </Card>
            <Card className="glass-effect p-6">
              <div className="flex items-center gap-3 mb-2">
                <Users className="h-6 w-6 text-primary" />
                <p className="text-sm text-muted-foreground">Peers</p>
              </div>
              <p className="text-xl font-bold">{chainInfo.peer_count}</p>
            </Card>
            <Card className="glass-effect p-6">
              <div className="flex items-center gap-3 mb-2">
                <Target className="h-6 w-6 text-primary" />
                <p className="text-sm text-muted-foreground">Best Hash</p>
              </div>
              <p className="text-xs font-mono break-all">{chainInfo.best_hash.slice(0, 16)}...</p>
            </Card>
            <Card className="glass-effect p-6">
              <div className="flex items-center gap-3 mb-2">
                <Award className="h-6 w-6 text-primary" />
                <p className="text-sm text-muted-foreground">Genesis</p>
              </div>
              <p className="text-xs font-mono break-all">{chainInfo.genesis_hash.slice(0, 16)}...</p>
            </Card>
          </div>
        )}

        {/* Marketplace Stats */}
        {marketplaceStats && (
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-5 gap-6 mb-12">
            <Card className="glass-effect p-6">
              <div className="flex items-center gap-3 mb-2">
                <Activity className="h-6 w-6 text-primary" />
                <p className="text-sm text-muted-foreground">Total Problems</p>
              </div>
              <p className="text-2xl font-bold">{marketplaceStats.total_problems}</p>
            </Card>
            <Card className="glass-effect p-6">
              <div className="flex items-center gap-3 mb-2">
                <Target className="h-6 w-6 text-success" />
                <p className="text-sm text-muted-foreground">Open</p>
              </div>
              <p className="text-2xl font-bold text-success">{marketplaceStats.open_problems}</p>
            </Card>
            <Card className="glass-effect p-6">
              <div className="flex items-center gap-3 mb-2">
                <Award className="h-6 w-6 text-primary" />
                <p className="text-sm text-muted-foreground">Solved</p>
              </div>
              <p className="text-2xl font-bold">{marketplaceStats.solved_problems}</p>
            </Card>
            <Card className="glass-effect p-6">
              <div className="flex items-center gap-3 mb-2">
                <BarChart3 className="h-6 w-6 text-warning" />
                <p className="text-sm text-muted-foreground">Expired</p>
              </div>
              <p className="text-2xl font-bold">{marketplaceStats.expired_problems}</p>
            </Card>
            <Card className="glass-effect p-6">
              <div className="flex items-center gap-3 mb-2">
                <TrendingUp className="h-6 w-6 text-primary" />
                <p className="text-sm text-muted-foreground">Bounty Pool</p>
              </div>
              <p className="text-2xl font-bold">{(marketplaceStats.total_bounty_pool / 1e9).toFixed(2)}B</p>
            </Card>
          </div>
        )}

        {/* Live Solution Feed */}
        <div className="mt-12">
          <LiveSolutionFeed />
        </div>
      </div>
    </section>
  );
};
