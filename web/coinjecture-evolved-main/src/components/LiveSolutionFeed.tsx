import { Card } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { useState, useEffect } from "react";
import { Activity, Zap, Target, Award, Loader2, AlertCircle } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import { rpcClient } from "@/lib/rpc-client";

interface Solution {
  block_height: number;
  problem_type: "SubsetSum" | "TSP" | "SAT";
  solver: string;
  bounty: string;
  work_score: number;
  solve_energy_joules: number;
  problem_complexity: number;
  timestamp: number;
  problem_data: {
    target?: number;
    values?: number[];
    cities?: number;
    clauses?: number;
    variables?: number;
  };
  solution_data: {
    indices?: number[];
    sum?: number;
    route?: number[];
    satisfying_assignment?: boolean[];
  };
}

export const LiveSolutionFeed = () => {
  const [solutions, setSolutions] = useState<Solution[]>([]);
  const [newSolutionIndex, setNewSolutionIndex] = useState<number | null>(null);
  const [previousHeight, setPreviousHeight] = useState<number | null>(null);

  // Fetch chain info to get latest block height
  const { data: chainInfo } = useQuery({
    queryKey: ['chain-info'],
    queryFn: () => rpcClient.getChainInfo(),
    refetchInterval: 10000, // Refresh every 10 seconds
  });

  // Fetch recent blocks and extract solution data
  useEffect(() => {
    if (!chainInfo) return;

    const fetchRecentBlocks = async () => {
      const latestHeight = chainInfo.best_height;
      const blockCount = 20; // Fetch last 20 blocks
      const startHeight = Math.max(0, latestHeight - blockCount + 1);

      const blockPromises: Promise<void>[] = [];
      const newSolutions: Solution[] = [];

      for (let height = latestHeight; height >= startHeight; height--) {
        blockPromises.push(
          rpcClient.getBlock(height).then((block) => {
            if (!block || !block.solution_reveal) return;

            const header = block.header;
            const solutionReveal = block.solution_reveal;
            const problem = solutionReveal.problem;
            const solution = solutionReveal.solution;

            // Determine problem type
            let problemType: "SubsetSum" | "TSP" | "SAT" | null = null;
            let problemData: Solution['problem_data'] = {};
            let solutionData: Solution['solution_data'] = {};

            // Parse problem and solution based on type
            const problemObj = problem as any;
            const solutionObj = solution as any;

            if (problemObj.SubsetSum) {
              problemType = "SubsetSum";
              const subsetSum = problemObj.SubsetSum;
              problemData = {
                target: subsetSum.target,
                values: subsetSum.numbers || [],
              };
              if (solutionObj.SubsetSum) {
                const indices = solutionObj.SubsetSum;
                const sum = indices.reduce((acc: number, idx: number) => acc + (subsetSum.numbers?.[idx] || 0), 0);
                solutionData = { indices, sum };
              }
            } else if (problemObj.SAT) {
              problemType = "SAT";
              const sat = problemObj.SAT;
              problemData = {
                variables: sat.variables || 0,
                clauses: Array.isArray(sat.clauses) ? sat.clauses.length : 0,
              };
              if (solutionObj.SAT) {
                const assignment = solutionObj.SAT;
                solutionData = { satisfying_assignment: assignment };
              }
            } else if (problemObj.TSP) {
              problemType = "TSP";
              const tsp = problemObj.TSP;
              problemData = {
                cities: tsp.cities || 0,
              };
              if (solutionObj.TSP) {
                const route = solutionObj.TSP;
                solutionData = { route };
              }
            }

            if (problemType) {
              // Format miner address as solver name (truncate)
              const solver = header.miner ? 
                `${header.miner.slice(0, 8)}...${header.miner.slice(-6)}` : 
                "Unknown";

              // Format bounty (work_score in millions)
              const bounty = header.work_score >= 1000000 
                ? `${(header.work_score / 1000000).toFixed(0)}M`
                : header.work_score >= 1000
                ? `${(header.work_score / 1000).toFixed(0)}K`
                : header.work_score.toFixed(0);

              newSolutions.push({
                block_height: header.height,
                problem_type: problemType,
                solver,
                bounty,
                work_score: header.work_score,
                solve_energy_joules: header.energy_estimate_joules,
                problem_complexity: header.complexity_weight,
                timestamp: header.timestamp * 1000, // Convert to milliseconds
                problem_data: problemData,
                solution_data: solutionData,
              });
            }
          }).catch(() => {
            // Silently skip blocks that fail to fetch
          })
        );
      }

      await Promise.all(blockPromises);

      // Sort by block height (newest first)
      newSolutions.sort((a, b) => b.block_height - a.block_height);

      // Check if we have a new block
      if (previousHeight !== null && latestHeight > previousHeight) {
        setNewSolutionIndex(0);
        setTimeout(() => setNewSolutionIndex(null), 2000);
      }

      setSolutions(newSolutions.slice(0, 20)); // Keep top 20
      setPreviousHeight(latestHeight);
    };

    fetchRecentBlocks();
  }, [chainInfo, previousHeight]);

  const formatTime = (timestamp: number) => {
    const seconds = Math.floor((Date.now() - timestamp) / 1000);
    if (seconds < 60) return `${seconds}s ago`;
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `${minutes}m ago`;
    return `${Math.floor(minutes / 60)}h ago`;
  };

  return (
    <Card className="glass-effect p-6">
      <div className="flex items-center gap-2 mb-4">
        <Activity className="h-5 w-5 text-primary animate-pulse" />
        <h3 className="text-xl font-semibold">Live Solution Feed</h3>
        <Badge variant="secondary" className="ml-auto">
          <span className="w-2 h-2 rounded-full bg-success mr-2 animate-pulse" />
          Live
        </Badge>
      </div>

      <div className="space-y-3 max-h-[600px] overflow-y-auto">
        {!chainInfo && (
          <div className="flex items-center justify-center py-12">
            <Loader2 className="h-8 w-8 animate-spin text-primary" />
          </div>
        )}
        {chainInfo && solutions.length === 0 && (
          <div className="text-center py-12 text-muted-foreground">
            <AlertCircle className="h-8 w-8 mx-auto mb-4 opacity-50" />
            <p>No solutions found in recent blocks</p>
            <p className="text-sm mt-2">Blocks will appear here as they are mined</p>
          </div>
        )}
        {solutions.map((solution, index) => (
          <Card 
            key={`${solution.block_height}-${solution.timestamp}`}
            className={`p-4 transition-all duration-500 ${
              index === newSolutionIndex 
                ? 'bg-primary/10 border-primary animate-pulse' 
                : 'hover:bg-accent/50'
            }`}
          >
            <div className="space-y-3">
              {/* Header */}
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <Badge variant="outline" className="terminal-font">
                    Block #{solution.block_height}
                  </Badge>
                  <Badge variant="secondary">{solution.problem_type}</Badge>
                </div>
                <span className="text-xs text-muted-foreground">
                  {formatTime(solution.timestamp)}
                </span>
              </div>

              {/* Problem Details */}
              <div className="bg-muted/30 rounded-lg p-3 space-y-2">
                <div className="flex items-center gap-2 text-sm">
                  <Target className="h-4 w-4 text-warning" />
                  <span className="text-muted-foreground">Problem:</span>
                  {solution.problem_type === "SubsetSum" && (
                    <span className="terminal-font text-foreground">
                      Find subset summing to {solution.problem_data.target}
                    </span>
                  )}
                  {solution.problem_type === "TSP" && (
                    <span className="terminal-font text-foreground">
                      Optimize route for {solution.problem_data.cities} cities
                    </span>
                  )}
                  {solution.problem_type === "SAT" && (
                    <span className="terminal-font text-foreground">
                      Satisfy {solution.problem_data.clauses} clauses with {solution.problem_data.variables} variables
                    </span>
                  )}
                </div>
                {solution.problem_type === "SubsetSum" && solution.problem_data.values && (
                  <div className="text-xs text-muted-foreground terminal-font">
                    Values: [{solution.problem_data.values.join(", ")}]
                  </div>
                )}
              </div>

              {/* Solution Details */}
              <div className="bg-success/10 rounded-lg p-3 space-y-2 border border-success/20">
                <div className="flex items-center gap-2 text-sm">
                  <Award className="h-4 w-4 text-success" />
                  <span className="text-muted-foreground">Solution:</span>
                  {solution.problem_type === "SubsetSum" && solution.solution_data.indices && (
                    <span className="terminal-font text-success">
                      Indices {solution.solution_data.indices.join(", ")} → Sum: {solution.solution_data.sum}
                    </span>
                  )}
                  {solution.problem_type === "TSP" && solution.solution_data.route && (
                    <span className="terminal-font text-success">
                      Route: {solution.solution_data.route.join(" → ")}
                    </span>
                  )}
                  {solution.problem_type === "SAT" && (
                    <span className="terminal-font text-success">
                      Satisfying assignment found
                    </span>
                  )}
                </div>
                <div className="flex items-center gap-2 text-xs text-muted-foreground">
                  <span>Solver: <span className="text-foreground font-medium">{solution.solver}</span></span>
                </div>
              </div>

              {/* Metrics */}
              <div className="grid grid-cols-4 gap-2 text-xs">
                <div className="flex items-center gap-1">
                  <Award className="h-3 w-3 text-primary" />
                  <span className="text-muted-foreground">Bounty:</span>
                  <span className="font-semibold text-primary">{solution.bounty}</span>
                </div>
                <div className="flex items-center gap-1">
                  <Target className="h-3 w-3 text-warning" />
                  <span className="text-muted-foreground">Work:</span>
                  <span className="font-semibold">{solution.work_score}</span>
                </div>
                <div className="flex items-center gap-1">
                  <Zap className="h-3 w-3 text-success" />
                  <span className="text-muted-foreground">Energy:</span>
                  <span className="font-semibold">{solution.solve_energy_joules.toFixed(2)}J</span>
                </div>
                <div className="flex items-center gap-1">
                  <Activity className="h-3 w-3 text-secondary" />
                  <span className="text-muted-foreground">Complexity:</span>
                  <span className="font-semibold">{solution.problem_complexity.toFixed(2)}</span>
                </div>
              </div>
            </div>
          </Card>
        ))}
      </div>

      <div className="mt-4 text-center">
        <a 
          href="https://huggingface.co/datasets/COINjecture/NP_Solutions" 
          target="_blank" 
          rel="noopener noreferrer"
          className="text-xs text-primary hover:underline"
        >
          View full NP_Solutions dataset on HuggingFace →
        </a>
      </div>
    </Card>
  );
};
