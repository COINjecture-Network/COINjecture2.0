import { Card } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { TrendingUp, Users, Activity, Target, Award, BarChart3, Network, Loader2 } from "lucide-react";
import { LiveSolutionFeed } from "./LiveSolutionFeed";
import { useQuery } from "@tanstack/react-query";
import { rpcClient } from "@/lib/rpc-client";
import { Link } from "react-router-dom";

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
          <p className="text-muted-foreground">Real-time blockchain and marketplace statistics that prove the market is live right now.</p>
        </div>

        <div className="market-surface-strong p-6 md:p-8 mb-10">
          <div className="grid gap-5 lg:grid-cols-[1.1fr_0.9fr] lg:items-center">
            <div>
              <div className="signal-kicker">Participation layer</div>
              <h3 className="text-2xl md:text-3xl font-bold mt-2">Use the live tape to decide whether to mine, post demand, or inspect output.</h3>
              <p className="text-muted-foreground mt-3 max-w-2xl">
                Metrics should do more than reassure visitors. They should help users decide where value is moving and what action to take next.
              </p>
            </div>
            <div className="grid gap-3 sm:grid-cols-2">
              <div className="signal-card">
                <div className="signal-kicker">For miners</div>
                <div className="mt-2 font-semibold">Track open demand, bounty pool, and fresh solved output before entering Solver Lab.</div>
              </div>
              <div className="signal-card">
                <div className="signal-kicker">For submitters</div>
                <div className="mt-2 font-semibold">Use market activity to decide when to post a high-signal bounty and what reward level will attract attention.</div>
              </div>
              <div className="sm:col-span-2 flex flex-col sm:flex-row gap-3">
                <Button asChild className="sm:flex-1">
                  <Link to="/solver-lab">Start Mining</Link>
                </Button>
                <Button asChild variant="outline" className="sm:flex-1">
                  <Link to="/bounty-submit">Post A Bounty</Link>
                </Button>
              </div>
            </div>
          </div>
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
                Could not load chain metrics. The app uses your API{" "}
                <code className="rounded bg-muted px-1 py-0.5 text-xs">GET /chain/info</code> first (see{" "}
                <code className="rounded bg-muted px-1 py-0.5 text-xs">VITE_API_URL</code>), then JSON-RPC (
                <code className="rounded bg-muted px-1 py-0.5 text-xs">POST …/node-rpc</code> or{" "}
                <code className="rounded bg-muted px-1 py-0.5 text-xs">VITE_RPC_URL</code>). Ensure the API is up,
                <code className="rounded bg-muted px-1 py-0.5 text-xs">NODE_RPC_URL</code> is set on the server, and
                CORS allows this origin.
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
            <Card className="signal-card">
              <div className="flex items-center gap-3 mb-2">
                <Network className="h-6 w-6 text-primary" />
                <p className="text-sm text-muted-foreground">Chain ID</p>
              </div>
              <p className="text-xl font-bold">{chainInfo.chain_id}</p>
            </Card>
            <Card className="signal-card">
              <div className="flex items-center gap-3 mb-2">
                <Activity className="h-6 w-6 text-primary" />
                <p className="text-sm text-muted-foreground">Block Height</p>
              </div>
              <p className="text-xl font-bold">{chainInfo.best_height.toLocaleString()}</p>
            </Card>
            <Card className="signal-card">
              <div className="flex items-center gap-3 mb-2">
                <Users className="h-6 w-6 text-primary" />
                <p className="text-sm text-muted-foreground">Peers</p>
              </div>
              <p className="text-xl font-bold">{chainInfo.peer_count}</p>
            </Card>
            <Card className="signal-card">
              <div className="flex items-center gap-3 mb-2">
                <Target className="h-6 w-6 text-primary" />
                <p className="text-sm text-muted-foreground">Best Hash</p>
              </div>
              <p className="text-xs font-mono break-all">
                {chainInfo.best_hash.length >= 16
                  ? `${chainInfo.best_hash.slice(0, 16)}...`
                  : chainInfo.best_hash || "—"}
              </p>
            </Card>
            <Card className="signal-card">
              <div className="flex items-center gap-3 mb-2">
                <Award className="h-6 w-6 text-primary" />
                <p className="text-sm text-muted-foreground">Genesis</p>
              </div>
              <p className="text-xs font-mono break-all">
                {chainInfo.genesis_hash.length >= 16
                  ? `${chainInfo.genesis_hash.slice(0, 16)}...`
                  : chainInfo.genesis_hash || "—"}
              </p>
            </Card>
          </div>
        )}

        {/* Marketplace Stats */}
        {marketplaceStats && (
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-5 gap-6 mb-12">
            <Card className="signal-card">
              <div className="flex items-center gap-3 mb-2">
                <Activity className="h-6 w-6 text-primary" />
                <p className="text-sm text-muted-foreground">Total Problems</p>
              </div>
              <p className="text-2xl font-bold">{marketplaceStats.total_problems}</p>
            </Card>
            <Card className="signal-card">
              <div className="flex items-center gap-3 mb-2">
                <Target className="h-6 w-6 text-success" />
                <p className="text-sm text-muted-foreground">Open</p>
              </div>
              <p className="text-2xl font-bold text-success">{marketplaceStats.open_problems}</p>
            </Card>
            <Card className="signal-card">
              <div className="flex items-center gap-3 mb-2">
                <Award className="h-6 w-6 text-primary" />
                <p className="text-sm text-muted-foreground">Solved</p>
              </div>
              <p className="text-2xl font-bold">{marketplaceStats.solved_problems}</p>
            </Card>
            <Card className="signal-card">
              <div className="flex items-center gap-3 mb-2">
                <BarChart3 className="h-6 w-6 text-warning" />
                <p className="text-sm text-muted-foreground">Expired</p>
              </div>
              <p className="text-2xl font-bold">{marketplaceStats.expired_problems}</p>
            </Card>
            <Card className="signal-card">
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
