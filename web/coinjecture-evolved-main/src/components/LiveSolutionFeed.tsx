import { Card } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { useEffect, useState } from "react";
import { Activity, Zap, Target, Award } from "lucide-react";

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

  // Simulate live feed with mock data based on actual dataset structure
  useEffect(() => {
    const mockSolutions: Solution[] = [
      {
        block_height: 40,
        problem_type: "SubsetSum",
        solver: "NPOptimizer",
        bounty: "369M",
        work_score: 369,
        solve_energy_joules: 2.89,
        problem_complexity: 5.2,
        timestamp: Date.now() - 2000,
        problem_data: { target: 15823, values: [3241, 5892, 2184, 4506] },
        solution_data: { indices: [0, 2, 3], sum: 15823 }
      },
      {
        block_height: 39,
        problem_type: "TSP",
        solver: "AlgoSolver",
        bounty: "342M",
        work_score: 342,
        solve_energy_joules: 2.71,
        problem_complexity: 4.8,
        timestamp: Date.now() - 45000,
        problem_data: { cities: 25 },
        solution_data: { route: [0, 3, 7, 12, 18, 24] }
      },
      {
        block_height: 38,
        problem_type: "SAT",
        solver: "ComplexityKing",
        bounty: "318M",
        work_score: 318,
        solve_energy_joules: 2.54,
        problem_complexity: 5.1,
        timestamp: Date.now() - 98000,
        problem_data: { clauses: 120, variables: 50 },
        solution_data: { satisfying_assignment: [true, false, true] }
      }
    ];

    setSolutions(mockSolutions);

    // Simulate new solutions arriving
    const interval = setInterval(() => {
      const problemTypes: ("SubsetSum" | "TSP" | "SAT")[] = ["SubsetSum", "TSP", "SAT"];
      const selectedType = problemTypes[Math.floor(Math.random() * 3)];
      
      const newSolution: Solution = {
        block_height: 40 + solutions.length + 1,
        problem_type: selectedType,
        solver: ["NPOptimizer", "AlgoSolver", "ComplexityKing", "Anonymous"][Math.floor(Math.random() * 4)],
        bounty: `${Math.floor(Math.random() * 400 + 200)}M`,
        work_score: Math.floor(Math.random() * 300 + 150),
        solve_energy_joules: Math.random() * 2 + 1.5,
        problem_complexity: Math.random() * 2 + 3.5,
        timestamp: Date.now(),
        problem_data: selectedType === "SubsetSum" 
          ? {
              target: Math.floor(Math.random() * 20000 + 5000),
              values: Array.from({ length: Math.floor(Math.random() * 5 + 3) }, () => 
                Math.floor(Math.random() * 5000 + 1000)
              )
            }
          : selectedType === "TSP"
          ? { cities: Math.floor(Math.random() * 15 + 15) }
          : { clauses: Math.floor(Math.random() * 100 + 80), variables: Math.floor(Math.random() * 40 + 30) },
        solution_data: selectedType === "SubsetSum"
          ? { indices: [0, 1], sum: Math.floor(Math.random() * 20000 + 5000) }
          : selectedType === "TSP"
          ? { route: Array.from({ length: 6 }, (_, i) => i * 4) }
          : { satisfying_assignment: [true, false, true] }
      };

      setSolutions(prev => [newSolution, ...prev].slice(0, 10));
      setNewSolutionIndex(0);
      setTimeout(() => setNewSolutionIndex(null), 2000);
    }, 8000);

    return () => clearInterval(interval);
  }, []);

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
