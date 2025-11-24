import { Card } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Database, FileCode, Cpu, HardDrive, Loader2, AlertCircle } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import { rpcClient, type ProblemInfo } from "@/lib/rpc-client";
import { formatDistanceToNow } from "date-fns";

// Static datasets for data marketplace
const datasets = [
  {
    title: "Blockchain Historical Data",
    description: "Complete transaction history and block data from genesis to latest block",
    size: "2.4 TB",
    price: "150 BEANS",
    icon: Database,
    category: "Historical"
  },
  {
    title: "Smart Contract Templates",
    description: "Pre-audited smart contract templates for common use cases",
    size: "45 MB",
    price: "25 BEANS",
    icon: FileCode,
    category: "Development"
  },
  {
    title: "Mining Pool Analytics",
    description: "Detailed analytics and optimization data for mining operations",
    size: "890 MB",
    price: "75 BEANS",
    icon: Cpu,
    category: "Analytics"
  },
  {
    title: "Network Snapshot",
    description: "Complete network state snapshot for rapid node synchronization",
    size: "1.8 TB",
    price: "200 BEANS",
    icon: HardDrive,
    category: "Infrastructure"
  }
];

const getProblemTypeLabel = (type: string) => {
  switch (type) {
    case 'SubsetSum': return 'SubsetSum';
    case 'SAT': return 'Boolean SAT';
    case 'TSP': return 'TSP';
    default: return type;
  }
};

const ProblemCard = ({ problem }: { problem: ProblemInfo }) => {
  const expirationDate = new Date(problem.expires_at * 1000);
  const submittedDate = new Date(problem.submitted_at * 1000);
  const isExpiringSoon = expirationDate.getTime() - Date.now() < 24 * 60 * 60 * 1000;

  return (
    <Card className="glass-effect p-6 hover:scale-[1.02] transition-transform duration-200">
      <div className="flex items-start justify-between mb-4">
        <div className="flex-1">
          <div className="flex items-center gap-2 mb-2">
            <h3 className="text-lg font-semibold">
              {problem.problem_type || (problem.is_private ? 'Private Problem' : 'Unknown')}
            </h3>
            <Badge variant={problem.status === 'OPEN' ? 'default' : 'secondary'}>
              {problem.status}
            </Badge>
            {problem.is_private && (
              <Badge variant="outline">{problem.is_revealed ? 'Revealed' : 'Private'}</Badge>
            )}
          </div>
          <p className="text-sm text-muted-foreground mb-4">
            Problem ID: <code className="text-xs">{problem.problem_id.slice(0, 16)}...</code>
            {problem.problem_size && (
              <> • Size: {problem.problem_size}</>
            )}
          </p>
          <div className="space-y-2 text-sm">
            <div className="flex items-center justify-between">
              <span className="text-muted-foreground">Bounty:</span>
              <span className="font-semibold text-primary">{problem.bounty.toLocaleString()} BEANS</span>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-muted-foreground">Min Work Score:</span>
              <span className="font-semibold">{problem.min_work_score}</span>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-muted-foreground">Submitted:</span>
              <span className="text-xs">{formatDistanceToNow(submittedDate, { addSuffix: true })}</span>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-muted-foreground">Expires:</span>
              <span className={isExpiringSoon ? "text-warning font-semibold text-xs" : "text-xs"}>
                {formatDistanceToNow(expirationDate, { addSuffix: true })}
              </span>
            </div>
          </div>
        </div>
      </div>
      <Button className="w-full" variant={problem.status === 'OPEN' ? 'default' : 'secondary'} disabled={problem.status !== 'OPEN'}>
        {problem.status === 'OPEN' ? 'View Problem' : problem.status}
      </Button>
    </Card>
  );
};

export const MarketplaceSection = () => {
  const { data: problems, isLoading: problemsLoading, error: problemsError } = useQuery({
    queryKey: ['marketplace-problems'],
    queryFn: () => rpcClient.getOpenProblems(),
    refetchInterval: 30000, // Refresh every 30 seconds
  });

  const { data: stats, isLoading: statsLoading } = useQuery({
    queryKey: ['marketplace-stats'],
    queryFn: () => rpcClient.getMarketplaceStats(),
    refetchInterval: 30000,
  });

  return (
    <section id="marketplace" className="py-20">
      <div className="container mx-auto px-6">
        <div className="text-center mb-12">
          <h2 className="text-4xl font-bold mb-4">PoUW Marketplace</h2>
          <p className="text-muted-foreground">
            Browse open computational problems and earn BEANS by solving them
          </p>
        </div>

        {/* Marketplace Stats */}
          {stats && (
          <div className="grid grid-cols-2 md:grid-cols-5 gap-4 mb-12 max-w-6xl mx-auto">
            <Card className="glass-effect p-4 text-center">
              <div className="text-2xl font-bold text-primary mb-1">{stats.total_problems}</div>
              <div className="text-xs text-muted-foreground">Total Problems</div>
            </Card>
            <Card className="glass-effect p-4 text-center">
              <div className="text-2xl font-bold text-primary mb-1">{stats.open_problems}</div>
              <div className="text-xs text-muted-foreground">Open Now</div>
            </Card>
            <Card className="glass-effect p-4 text-center">
              <div className="text-2xl font-bold text-primary mb-1">{stats.solved_problems}</div>
              <div className="text-xs text-muted-foreground">Solved</div>
            </Card>
            <Card className="glass-effect p-4 text-center">
              <div className="text-2xl font-bold text-primary mb-1">{stats.expired_problems}</div>
              <div className="text-xs text-muted-foreground">Expired</div>
            </Card>
            <Card className="glass-effect p-4 text-center">
              <div className="text-2xl font-bold text-primary mb-1">
                {(stats.total_bounty_pool / 1e9).toFixed(2)}B
              </div>
              <div className="text-xs text-muted-foreground">Bounty Pool</div>
            </Card>
          </div>
        )}

        {/* Open Problems Section */}
        <div className="mb-16">
          <h3 className="text-2xl font-bold mb-6">Open Problems</h3>
          {problemsLoading && (
            <div className="flex items-center justify-center py-12">
              <Loader2 className="h-8 w-8 animate-spin text-primary" />
            </div>
          )}
          {problemsError && (
            <Card className="glass-effect p-6">
              <div className="flex items-center gap-3 text-destructive">
                <AlertCircle className="h-5 w-5" />
                <div>
                  <div className="font-semibold">Failed to load problems</div>
                  <div className="text-sm text-muted-foreground">
                    {problemsError instanceof Error ? problemsError.message : 'Unknown error'}
                  </div>
                </div>
              </div>
            </Card>
          )}
          {problems && problems.length > 0 ? (
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
              {problems.map((problem) => (
                <ProblemCard key={problem.problem_id} problem={problem} />
              ))}
            </div>
          ) : (
            !problemsLoading && (
              <Card className="glass-effect p-8 text-center">
                <p className="text-muted-foreground">No open problems at the moment. Check back soon!</p>
              </Card>
            )
          )}
        </div>

        {/* Data Marketplace Section */}
        <div className="mt-16">
          <h3 className="text-2xl font-bold mb-6">Data Marketplace</h3>
          <p className="text-muted-foreground mb-8 text-center">
            Purchase blockchain data, analytics, and computational resources
          </p>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-6 max-w-6xl mx-auto">
            {datasets.map((dataset, index) => (
              <Card key={index} className="glass-effect p-6 hover:scale-[1.02] transition-transform duration-200">
                <div className="flex items-start gap-4 mb-4">
                  <div className="p-3 rounded-lg bg-primary/10">
                    <dataset.icon className="h-6 w-6 text-primary" />
                  </div>
                  <div className="flex-1">
                    <div className="flex items-start justify-between mb-2">
                      <h3 className="text-lg font-semibold">{dataset.title}</h3>
                      <span className="text-xs bg-secondary/20 text-secondary px-2 py-1 rounded-full">
                        {dataset.category}
                      </span>
                    </div>
                    <p className="text-sm text-muted-foreground mb-4">{dataset.description}</p>
                    <div className="flex items-center justify-between">
                      <div className="text-sm">
                        <span className="text-muted-foreground">Size: </span>
                        <span className="text-foreground font-semibold">{dataset.size}</span>
                      </div>
                      <div className="text-right">
                        <div className="text-xs text-muted-foreground mb-1">Price</div>
                        <div className="text-lg font-bold text-primary">{dataset.price}</div>
                      </div>
                    </div>
                  </div>
                </div>
                <Button className="w-full" variant="default">
                  Purchase Dataset
                </Button>
              </Card>
            ))}
          </div>
        </div>
      </div>
    </section>
  );
};
