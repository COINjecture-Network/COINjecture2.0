import { Card } from "@/components/ui/card";
import { TrendingUp, Users, Activity, Target, Award, BarChart3, Network, Loader2 } from "lucide-react";
import { LiveSolutionFeed } from "./LiveSolutionFeed";
import { useQuery } from "@tanstack/react-query";
import { rpcClient } from "@/lib/rpc-client";

export const MetricsSection = () => {
  const { data: chainInfo, isLoading: chainLoading } = useQuery({
    queryKey: ['chain-info'],
    queryFn: () => rpcClient.getChainInfo(),
    refetchInterval: 10000, // Refresh every 10 seconds
  });

  const { data: marketplaceStats, isLoading: statsLoading } = useQuery({
    queryKey: ['marketplace-stats'],
    queryFn: () => rpcClient.getMarketplaceStats(),
    refetchInterval: 30000, // Refresh every 30 seconds
  });

  return (
    <section id="metrics" className="py-20 relative">
      <div className="container mx-auto px-6">
        <div className="text-center mb-12">
          <h2 className="text-4xl font-bold mb-4">Live Network Metrics</h2>
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

        {/* Chain Information */}
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

        {(chainLoading || statsLoading) && (
          <div className="flex items-center justify-center py-12">
            <Loader2 className="h-8 w-8 animate-spin text-primary" />
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
