import { Card } from "@/components/ui/card";
import { TrendingUp, Users, Activity, Zap, Cpu, Target, Award, BarChart3, Network, Loader2 } from "lucide-react";
import { LiveSolutionFeed } from "./LiveSolutionFeed";
import { useQuery } from "@tanstack/react-query";
import { rpcClient } from "@/lib/rpc-client";

const metrics = [
  {
    label: "Total Solutions",
    value: "40",
    change: "+15.2%",
    icon: Activity,
    color: "text-primary"
  },
  {
    label: "Problem Types",
    value: "3",
    change: "SubsetSum, TSP, SAT",
    icon: Users,
    color: "text-success"
  },
  {
    label: "Avg Work Score",
    value: "184.5",
    change: "+18.3%",
    icon: Target,
    color: "text-warning"
  },
  {
    label: "Total BEANS Awarded",
    value: "7.38B",
    change: "+22.4%",
    icon: Award,
    color: "text-secondary"
  }
];

const energyMetrics = [
  {
    label: "Avg Energy Efficiency",
    value: "0.98",
    subtitle: "Near-optimal performance",
    icon: Zap
  },
  {
    label: "Avg Solve Energy",
    value: "2.14 J",
    subtitle: "Per solution",
    icon: Cpu
  },
  {
    label: "Problem Complexity",
    value: "3.2-5.8",
    subtitle: "Range distribution",
    icon: BarChart3
  },
  {
    label: "Solution Quality",
    value: "100%",
    subtitle: "Verification rate",
    icon: Target
  }
];

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

        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
          {metrics.map((metric) => (
            <Card key={metric.label} className="glass-effect p-6 hover:scale-105 transition-transform duration-200">
              <div className="flex items-start justify-between mb-4">
                <metric.icon className={`h-8 w-8 ${metric.color}`} />
                <span className="text-xs text-success">{metric.change}</span>
              </div>
              <div>
                <p className="text-3xl font-bold mb-1">{metric.value}</p>
                <p className="text-sm text-muted-foreground">{metric.label}</p>
              </div>
            </Card>
          ))}
        </div>

        {/* Energy Efficiency Metrics */}
        <div className="mt-12">
          <h3 className="text-2xl font-bold mb-6 text-center">Energy & Performance Metrics</h3>
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
            {energyMetrics.map((metric) => (
              <Card key={metric.label} className="glass-effect p-6">
                <div className="flex items-center gap-3 mb-3">
                  <metric.icon className="h-6 w-6 text-primary" />
                  <p className="text-sm text-muted-foreground">{metric.label}</p>
                </div>
                <p className="text-2xl font-bold mb-1">{metric.value}</p>
                <p className="text-xs text-muted-foreground">{metric.subtitle}</p>
              </Card>
            ))}
          </div>
        </div>

        {/* Live Solution Feed */}
        <div className="mt-12">
          <LiveSolutionFeed />
        </div>

        {/* Additional Stats */}
        <div className="mt-12 grid grid-cols-1 lg:grid-cols-2 gap-6">
          <Card className="glass-effect p-6">
            <h3 className="text-xl font-semibold mb-4">Recent Solutions</h3>
            <div className="space-y-3">
              {[
                { block: 40, bounty: "369M", work: 369, energy: "2.89J", type: "SubsetSum" },
                { block: 39, bounty: "342M", work: 342, energy: "2.71J", type: "TSP" },
                { block: 38, bounty: "318M", work: 318, energy: "2.54J", type: "SAT" },
                { block: 37, bounty: "295M", work: 295, energy: "2.38J", type: "SubsetSum" },
                { block: 36, bounty: "273M", work: 273, energy: "2.23J", type: "TSP" }
              ].map((solution, i) => (
                <div key={solution.block} className="flex items-center justify-between text-sm">
                  <div className="flex items-center gap-3">
                    <div className="w-2 h-2 rounded-full bg-primary" />
                    <span className="text-muted-foreground">#{solution.block} • {solution.type}</span>
                  </div>
                  <span className="text-xs text-muted-foreground">Work: {solution.work}</span>
                  <span className="text-xs text-warning">{solution.energy}</span>
                  <span className="text-terminal-text terminal-font">{solution.bounty} BEANS</span>
                </div>
              ))}
            </div>
          </Card>

          <Card className="glass-effect p-6">
            <h3 className="text-xl font-semibold mb-4">Top Solvers</h3>
            <div className="space-y-3">
              {[
                { solver: "NPOptimizer", solutions: 15, beans: "2.89B", avgWork: 193 },
                { solver: "AlgoSolver", solutions: 12, beans: "2.31B", avgWork: 187 },
                { solver: "ComplexityKing", solutions: 8, beans: "1.54B", avgWork: 195 },
                { solver: "Anonymous", solutions: 5, beans: "640M", avgWork: 180 }
              ].map((solver, i) => (
                <div key={solver.solver} className="flex items-center justify-between text-sm">
                  <div className="flex items-center gap-3">
                    <span className="text-muted-foreground">#{i + 1}</span>
                    <div>
                      <p className="text-foreground font-medium">{solver.solver}</p>
                      <p className="text-xs text-muted-foreground">{solver.solutions} solutions • Avg work: {solver.avgWork}</p>
                    </div>
                  </div>
                  <span className="text-primary font-semibold">{solver.beans}</span>
                </div>
              ))}
            </div>
          </Card>
        </div>
      </div>
    </section>
  );
};
