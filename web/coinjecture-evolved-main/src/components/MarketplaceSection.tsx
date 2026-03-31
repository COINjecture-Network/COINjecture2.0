import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { apiFetch } from "@/lib/api/client";
import { cn } from "@/lib/utils";
import { useAuth } from "@/lib/auth";
import type { SolutionSet } from "@/lib/api/types";
import {
  Activity,
  AlertCircle,
  ArrowRight,
  BrainCircuit,
  ChevronRight,
  Cpu,
  Database,
  Download,
  Loader2,
  ShieldCheck,
  Sparkles,
  Zap,
} from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import { rpcClient, type ProblemInfo } from "@/lib/rpc-client";
import { formatDistanceToNow } from "date-fns";
import { Link } from "react-router-dom";
import { useMemo, useState } from "react";

type DatasetCatalogItem = {
  id?: string;
  slug: string;
  dataset_type: string;
  title: string;
  description?: string | null;
  price: number;
  currency: string;
  visibility?: string;
  metadata?: Record<string, unknown> | null;
  latest_snapshot?: {
    id?: string;
    version?: string;
    row_count?: number;
    end_height?: number;
    checksum?: string | null;
    status?: string;
  } | null;
};

const fallbackDatasets: DatasetCatalogItem[] = [
  {
    slug: "marketplace-events-by-block",
    dataset_type: "marketplace_events",
    title: "Marketplace Events By Block",
    description: "Track problem submissions, solver activity, and payouts block by block.",
    price: 0,
    currency: "BEANS",
  },
  {
    slug: "problem-submissions-and-solutions",
    dataset_type: "problem_activity",
    title: "Problem Submissions And Solutions",
    description: "A labeled stream of NP-hard problem statements, solver attempts, and verified outcomes.",
    price: 0,
    currency: "BEANS",
  },
  {
    slug: "bounty-payout-history",
    dataset_type: "bounty_payouts",
    title: "Bounty Payout History",
    description: "Audit how value moves through the network as useful work gets rewarded.",
    price: 0,
    currency: "BEANS",
  },
  {
    slug: "trading-and-liquidity-activity",
    dataset_type: "trading_activity",
    title: "Trading And Liquidity Activity",
    description: "Study emerging market behavior as computational work becomes a priced asset.",
    price: 0,
    currency: "BEANS",
  },
];

const datasetVisuals: Record<string, { icon: typeof Database; badgeClass: string; eyebrow: string }> = {
  marketplace_events: {
    icon: Database,
    badgeClass: "bg-accent-blue/15 text-accent-blue border-accent-blue/30",
    eyebrow: "Live marketplace tape",
  },
  problem_activity: {
    icon: BrainCircuit,
    badgeClass: "bg-[#D4537E]/15 text-[#D4537E] border-[#D4537E]/30",
    eyebrow: "Reasoning corpus",
  },
  bounty_payouts: {
    icon: ShieldCheck,
    badgeClass: "bg-accent-emerald/15 text-accent-emerald border-accent-emerald/30",
    eyebrow: "Reward and audit trail",
  },
  trading_activity: {
    icon: LineChart,
    badgeClass: "bg-warning/15 text-warning border-warning/30",
    eyebrow: "Market structure signal",
  },
};

const getProblemTypeLabel = (type: string) => {
  switch (type) {
    case "SubsetSum":
      return "SubsetSum";
    case "SAT":
      return "Boolean SAT";
    case "TSP":
      return "TSP";
    default:
      return type;
  }
};

const ProblemCard = ({ problem }: { problem: ProblemInfo }) => {
  const expirationDate = new Date(problem.expires_at * 1000);
  const submittedDate = new Date(problem.submitted_at * 1000);
  const isExpiringSoon = expirationDate.getTime() - Date.now() < 24 * 60 * 60 * 1000;
  const statusLabel = problem.status === "OPEN" ? "Live bounty" : problem.status;
  const minWorkLabel = typeof problem.min_work_score === "number" ? problem.min_work_score.toString() : "Open";
  const urgencyLabel = isExpiringSoon ? "Closing soon" : "Plenty of runway";

  return (
    <Card className="glass-effect border-border/70 hover:border-primary/40 transition-colors">
      <CardContent className="p-6 space-y-5">
        <div className="flex items-start justify-between gap-3">
          <div>
            <div className="flex items-center gap-2 mb-2">
              <h3 className="text-lg font-semibold">
                {problem.problem_type || (problem.is_private ? "Private Problem" : "Unknown")}
              </h3>
              <Badge variant={problem.status === "OPEN" ? "default" : "secondary"}>
                {statusLabel}
              </Badge>
              {problem.is_private && (
                <Badge variant="outline">{problem.is_revealed ? "Revealed" : "Private"}</Badge>
              )}
            </div>
            <p className="text-sm text-muted-foreground">
              Problem ID <code className="text-xs">{problem.problem_id.slice(0, 16)}...</code>
            </p>
          </div>
          <div className="rounded-xl border border-primary/20 bg-primary/10 px-3 py-2 text-right">
            <div className="text-[11px] uppercase tracking-[0.14em] text-muted-foreground">Bounty</div>
            <div className="text-lg font-semibold text-primary">{problem.bounty.toLocaleString()} BEANS</div>
          </div>
        </div>

        <div className="grid grid-cols-2 gap-3 text-sm">
          <MetricPill label="Difficulty floor" value={minWorkLabel} />
          <MetricPill label="Problem size" value={problem.problem_size || "Network sized"} />
          <MetricPill label="Submitted" value={formatDistanceToNow(submittedDate, { addSuffix: true })} />
          <MetricPill
            label="Window"
            value={urgencyLabel}
            tone={isExpiringSoon ? "warning" : "default"}
          />
        </div>

        <div className="rounded-xl border border-border/60 bg-muted/20 p-4 text-sm">
          <div className="text-[11px] uppercase tracking-[0.16em] text-muted-foreground mb-2">Operational view</div>
          <p className="text-foreground leading-relaxed">
            This bounty is live on-chain now. Open Solver Lab to inspect the structure, test an approach, and prepare a submission.
          </p>
          <div className="mt-3 text-muted-foreground">
            Closes {formatDistanceToNow(expirationDate, { addSuffix: true })}
          </div>
        </div>

        <div className="flex flex-col sm:flex-row gap-3">
          <Button asChild className="sm:flex-1" disabled={problem.status !== "OPEN"}>
            <Link to="/solver-lab">
              Solve in Solver Lab
              <ArrowRight className="h-4 w-4" />
            </Link>
          </Button>
          <Button asChild variant="outline" className="sm:flex-1">
            <Link to="/solver-lab">
              Open workbench
              <ChevronRight className="h-4 w-4" />
            </Link>
          </Button>
        </div>
      </CardContent>
    </Card>
  );
};

const SolutionSetCard = ({ solution }: { solution: SolutionSet }) => {
  const hasDetailedPayload = Boolean(
    solution.raw_problem || solution.raw_solution || solution.raw_solution_reveal,
  );

  return (
    <Dialog>
      <DialogTrigger asChild>
        <button type="button" className="w-full text-left">
          <Card className="border-border/60 bg-background/60 transition-colors hover:border-primary/40">
            <CardContent className="p-5">
              <div className="flex items-start justify-between gap-3 mb-3">
                <div>
                  <div className="text-sm font-semibold">{getProblemTypeLabel(solution.problem_type)}</div>
                  <div className="text-xs text-muted-foreground">
                    Block {solution.block_height.toLocaleString()} • {solution.quality_band || "unknown"} quality band
                  </div>
                </div>
                <Badge variant="outline">{solution.solution_type}</Badge>
              </div>
              <div className="grid grid-cols-2 gap-3 text-sm">
                <MetricPill label="Work" value={formatMetricNumber(solution.work_score)} />
                <MetricPill label="Quality" value={formatMetricNumber(solution.solution_quality)} />
                <MetricPill label="Solve us" value={formatIntegerMetric(solution.solve_time_us)} />
                <MetricPill label="Asymmetry" value={formatMetricNumber(solution.time_asymmetry_ratio)} />
              </div>
              <div className="mt-4 text-xs text-muted-foreground">
                Problem ID <code>{solution.problem_id?.slice(0, 24) || "pending"}</code>
              </div>
              <div className="mt-3 text-xs text-primary">
                Click to inspect this solution set
              </div>
            </CardContent>
          </Card>
        </button>
      </DialogTrigger>
      <DialogContent className="max-w-4xl max-h-[85vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>
            Block {solution.block_height.toLocaleString()} {getProblemTypeLabel(solution.problem_type)} solution set
          </DialogTitle>
          <DialogDescription>
            Inspect the underlying on-chain problem and revealed solution payload for this block.
          </DialogDescription>
        </DialogHeader>

        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4 text-sm">
          <MetricPill label="Work" value={formatMetricNumber(solution.work_score)} />
          <MetricPill label="Quality" value={formatMetricNumber(solution.solution_quality)} />
          <MetricPill label="Solve us" value={formatIntegerMetric(solution.solve_time_us)} />
          <MetricPill label="Asymmetry" value={formatMetricNumber(solution.time_asymmetry_ratio)} />
        </div>

        <div className="grid gap-4 md:grid-cols-2">
          <DetailPanel
            title="Problem summary"
            content={{
              block_hash: solution.block_hash,
              problem_id: solution.problem_id,
              problem_type: solution.problem_type,
              solution_type: solution.solution_type,
              miner: solution.miner,
              created_at: solution.created_at,
              quality_band: solution.quality_band,
            }}
          />
          <DetailPanel
            title="Metrics"
            content={{
              work_score: solution.work_score,
              solution_quality: solution.solution_quality,
              solve_time_us: solution.solve_time_us,
              verify_time_us: solution.verify_time_us,
              time_asymmetry_ratio: solution.time_asymmetry_ratio,
              complexity_weight: solution.complexity_weight,
              energy_estimate_joules: solution.energy_estimate_joules,
            }}
          />
        </div>

        {hasDetailedPayload ? (
          <div className="grid gap-4">
            <DetailPanel title="Raw problem" content={solution.raw_problem} />
            <DetailPanel title="Raw solution" content={solution.raw_solution} />
            <DetailPanel title="Raw solution reveal" content={solution.raw_solution_reveal} />
          </div>
        ) : (
          <div className="rounded-xl border border-warning/30 bg-warning/5 p-4 text-sm text-muted-foreground">
            This row does not include a detailed reveal payload yet, but the block metadata and scoring metrics are shown above.
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
};

export const MarketplaceSection = () => {
  const { isAuthenticated, openAuthModal } = useAuth();
  const [marketplaceTab, setMarketplaceTab] = useState("bounties");
  const [solutionSortBy, setSolutionSortBy] = useState("work_score");
  const [solutionProblemType, setSolutionProblemType] = useState("all");
  const { data: problems, isLoading: problemsLoading, error: problemsError } = useQuery({
    queryKey: ["marketplace-problems"],
    queryFn: () => rpcClient.getOpenProblems(),
    refetchInterval: 30000,
  });

  const { data: stats, isLoading: statsLoading } = useQuery({
    queryKey: ["marketplace-stats"],
    queryFn: () => rpcClient.getMarketplaceStats(),
    refetchInterval: 30000,
  });

  const {
    data: datasets = fallbackDatasets,
    isLoading: datasetsLoading,
    error: datasetsError,
  } = useQuery({
    queryKey: ["dataset-catalog"],
    queryFn: () => apiFetch<DatasetCatalogItem[]>("/marketplace/datasets"),
    retry: 1,
    staleTime: 30000,
  });
  const {
    data: solutionSets = [],
    isLoading: solutionSetsLoading,
    error: solutionSetsError,
  } = useQuery({
    queryKey: ["solution-sets", solutionSortBy, solutionProblemType],
    queryFn: () => {
      const params = new URLSearchParams({
        sort_by: solutionSortBy,
        sort_order: "desc",
        limit: "12",
      });
      if (solutionProblemType !== "all") {
        params.set("problem_type", solutionProblemType);
      }
      return apiFetch<SolutionSet[]>(`/marketplace/solution-sets?${params.toString()}`);
    },
    retry: 1,
    staleTime: 30000,
  });

  const normalizedDatasets = useMemo(
    () =>
      datasets.map((dataset) => ({
        ...dataset,
        price: 0,
      })),
    [datasets],
  );

  const openRegistration = () => openAuthModal("email", "signup");

  const handleDatasetAction = () => {
    if (isAuthenticated) {
      window.alert("Dataset exploration tools are reserved for signed-in accounts. You already have access to the live previews below.");
      return;
    }

    openRegistration();
  };

  const liveSolutionCount = solutionSets.length.toString();
  const topSolution = solutionSets[0];
  const bountyCtaLabel = isAuthenticated ? "Open Solver Lab" : "Register and solve";
  const solutionState = getDataState(solutionSetsLoading, solutionSetsError, solutionSets.length);
  const datasetState = getDataState(datasetsLoading, datasetsError, normalizedDatasets.length);

  return (
    <section id="marketplace" className="py-20">
      <div className="container mx-auto px-6">
        <Card className="glass-effect overflow-hidden border-primary/20 bg-gradient-to-br from-primary/10 via-background to-background mb-12">
          <div className="grid gap-8 px-6 py-8 md:grid-cols-[1.2fr_0.8fr] md:px-8">
            <div>
              <div className="flex flex-wrap items-center gap-2 mb-4">
                <Badge variant="secondary" className="text-xs">Live blockchain marketplace</Badge>
                <Badge variant="outline" className="text-xs border-primary/30 text-primary">
                  {isAuthenticated ? "Dataset access unlocked" : "Register to explore datasets"}
                </Badge>
              </div>
              <h2 className="text-4xl font-bold tracking-tight mb-4">
                See useful work happening live, then step in and <span className="text-primary">use the chain output</span>
              </h2>
              <p className="text-muted-foreground max-w-3xl leading-relaxed mb-6">
                COINjecture turns useful computation into a visible market. Watch active bounties, inspect fresh solution
                data, and register when you want to move from browsing into dataset exploration.
              </p>
              <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
                <SignalStat
                  label="See useful work"
                  value={statsLoading ? "Loading" : stats ? stats.open_problems.toLocaleString() : "Live"}
                />
                <SignalStat label="Explore solution data" value={solutionSetsLoading ? "Loading" : liveSolutionCount} />
                <SignalStat
                  label="Register to unlock"
                  value={isAuthenticated ? "Dataset tools enabled" : "Free account"}
                />
              </div>
            </div>
            <Card className="border-border/60 bg-background/80">
              <CardHeader className="pb-3">
                <CardTitle className="text-lg">What this page shows</CardTitle>
                <CardDescription>
                  A quick read on what the network is doing right now.
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-3 text-sm text-muted-foreground">
                <ValueBullet icon={Activity} text="Live bounty demand shows where the chain wants useful work next." />
                <ValueBullet icon={BrainCircuit} text="Verified solution sets prove the chain is producing real outputs now." />
                <ValueBullet icon={Database} text="Registration unlocks deeper dataset exploration instead of a generic download flow." />
              </CardContent>
            </Card>
          </div>
        </Card>

        <Tabs value={marketplaceTab} onValueChange={setMarketplaceTab}>
          <div className="flex flex-col gap-4 md:flex-row md:items-end md:justify-between mb-8">
            <div>
              <h3 className="text-2xl font-bold">Marketplace</h3>
              <p className="text-muted-foreground">
                Browse the network from two angles: live work opportunities and live chain output.
              </p>
            </div>
            <TabsList className="w-full md:w-auto">
              <TabsTrigger value="bounties">Bounty Marketplace</TabsTrigger>
              <TabsTrigger value="data">Data Marketplace</TabsTrigger>
            </TabsList>
          </div>

          <TabsContent value="bounties" className="space-y-6">
            <div className="grid gap-6 lg:grid-cols-[0.9fr_1.1fr]">
              <Card className="glass-effect border-border/70">
                <CardHeader>
                  <div className="flex items-center gap-2 mb-2">
                    <Badge variant="secondary">Live problem feed</Badge>
                    <Badge variant="outline">Routes into Solver Lab</Badge>
                  </div>
                  <CardTitle className="text-2xl">Solve the next useful block</CardTitle>
                  <CardDescription className="leading-relaxed">
                    Open bounties show what the network wants solved right now. Scan current work, compare urgency and reward,
                    then jump into Solver Lab to act on it.
                  </CardDescription>
                </CardHeader>
                <CardContent className="space-y-4">
                  <div className="grid grid-cols-2 gap-3 text-sm">
                    <MetricPill
                      label="Open now"
                      value={statsLoading ? "Loading" : stats ? stats.open_problems.toLocaleString() : "Live"}
                    />
                    <MetricPill
                      label="Solved"
                      value={statsLoading ? "Loading" : stats ? stats.solved_problems.toLocaleString() : "Live"}
                    />
                    <MetricPill
                      label="Bounty pool"
                      value={stats && typeof stats.total_bounty_pool === "number" ? `${(stats.total_bounty_pool / 1e9).toFixed(2)}B` : "Live"}
                    />
                    <MetricPill
                      label="Expired"
                      value={statsLoading ? "Loading" : stats ? stats.expired_problems.toLocaleString() : "Live"}
                    />
                  </div>

                  <div className="rounded-xl border border-border/60 bg-muted/20 p-4">
                    <div className="text-xs uppercase tracking-[0.16em] text-muted-foreground mb-2">What to do next</div>
                    <p className="text-sm text-foreground leading-relaxed">
                      Start with the live cards on the right. When a bounty looks attractive, open Solver Lab to inspect the
                      problem, test ideas, and work toward a submission.
                    </p>
                  </div>

                  <div className="flex flex-col sm:flex-row gap-3">
                    <Button asChild className="sm:flex-1">
                      <Link to="/solver-lab">
                        {bountyCtaLabel}
                        <ArrowRight className="h-4 w-4" />
                      </Link>
                    </Button>
                    {!isAuthenticated && (
                      <Button variant="outline" className="sm:flex-1" onClick={openRegistration}>
                        Create free account
                        <Download className="h-4 w-4" />
                      </Button>
                    )}
                  </div>
                </CardContent>
              </Card>

              <div className="space-y-4">
                {problemsLoading && (
                  <Card className="glass-effect p-8">
                    <div className="flex items-center justify-center py-8">
                      <Loader2 className="h-8 w-8 animate-spin text-primary" />
                    </div>
                    <p className="text-center text-sm text-muted-foreground">Loading the latest on-chain bounty feed…</p>
                  </Card>
                )}

                {problemsError && (
                  <Card className="glass-effect border-warning/30 bg-warning/5">
                    <CardContent className="flex items-start gap-3 p-6 text-sm">
                      <AlertCircle className="h-5 w-5 text-warning mt-0.5" />
                      <div>
                        <div className="font-semibold text-foreground">Bounty feed unavailable right now</div>
                        <div className="text-muted-foreground">
                          The network is still live, but this view could not load the current bounty list.
                        </div>
                      </div>
                    </CardContent>
                  </Card>
                )}

                {!problemsLoading && !problemsError && problems && problems.length > 0 ? (
                  <div className="grid grid-cols-1 xl:grid-cols-2 gap-4">
                    {problems.slice(0, 6).map((problem) => (
                      <ProblemCard key={problem.problem_id} problem={problem} />
                    ))}
                  </div>
                ) : null}

                {!problemsLoading && !problemsError && (!problems || problems.length === 0) && (
                  <Card className="glass-effect p-8 text-center">
                    <p className="text-muted-foreground">
                      No live bounties are listed at the moment. Open Solver Lab anyway so you are ready when the next problem lands.
                    </p>
                  </Card>
                )}
              </div>
            </div>
          </TabsContent>

          <TabsContent value="data" className="space-y-6">
            <div className="grid gap-6 lg:grid-cols-[1.1fr_0.9fr]">
              <Card className="glass-effect border-border/70">
                <CardHeader>
                  <div className="flex items-center gap-2 mb-2">
                    <Badge variant="secondary">Live chain visibility</Badge>
                    <Badge variant="outline" className="border-primary/30 text-primary">
                      {isAuthenticated ? "Signed in" : "Register to explore"}
                    </Badge>
                  </div>
                  <CardTitle className="text-2xl">The chain is producing usable outputs now</CardTitle>
                  <CardDescription className="leading-relaxed">
                    Start with the live solution browser below. It is the clearest proof that the network is active, solving work,
                    and turning blocks into structured data you can explore.
                  </CardDescription>
                </CardHeader>
                <CardContent className="space-y-4">
                  <div className="grid grid-cols-2 gap-3 text-sm">
                    <MetricPill label="Visible rows" value={solutionSetsLoading ? "Loading" : liveSolutionCount} />
                    <MetricPill
                      label="Top quality"
                      value={topSolution ? formatMetricNumber(topSolution.solution_quality) : "Live"}
                    />
                    <MetricPill
                      label="Current type"
                      value={topSolution ? getProblemTypeLabel(topSolution.problem_type) : "Mixed"}
                    />
                    <MetricPill
                      label="Access"
                      value={isAuthenticated ? "Explore datasets" : "Free account unlock"}
                    />
                  </div>

                  <div className="rounded-xl border border-border/60 bg-muted/20 p-4">
                    <div className="text-xs uppercase tracking-[0.16em] text-muted-foreground mb-2">Why register</div>
                    <p className="text-sm text-foreground leading-relaxed">
                      Signed-out visitors can preview the chain. Signed-in users get the unlock moment: guided dataset exploration,
                      richer tools, and better ways to act on what the chain is producing.
                    </p>
                  </div>

                  <div className="flex flex-col sm:flex-row gap-3">
                    <Button className="sm:flex-1" onClick={handleDatasetAction}>
                      {isAuthenticated ? "Explore dataset tools" : "Create free account"}
                      <ArrowRight className="h-4 w-4" />
                    </Button>
                    <Button variant="outline" className="sm:flex-1" asChild>
                      <Link to="/solver-lab">
                        Open Solver Lab
                        <ChevronRight className="h-4 w-4" />
                      </Link>
                    </Button>
                  </div>
                </CardContent>
              </Card>

              <RegisterToExploreCard
                isAuthenticated={isAuthenticated}
                onPrimaryAction={handleDatasetAction}
                onSecondaryAction={openRegistration}
              />
            </div>

            <Card className="glass-effect max-w-6xl mx-auto">
              <CardHeader className="gap-4 md:flex-row md:items-end md:justify-between">
                <div>
                  <CardTitle className="text-xl">Verified solution sets</CardTitle>
                  <CardDescription>
                    Recent on-chain solutions, sortable by work score, quality, and problem type.
                  </CardDescription>
                </div>
                <div className="grid gap-3 sm:grid-cols-2">
                  <div className="space-y-1">
                    <div className="text-xs uppercase tracking-[0.14em] text-muted-foreground">Sort by</div>
                    <Select value={solutionSortBy} onValueChange={setSolutionSortBy}>
                      <SelectTrigger className="min-w-[180px]">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="work_score">Work score</SelectItem>
                        <SelectItem value="solution_quality">Solution quality</SelectItem>
                        <SelectItem value="problem_type">Problem type</SelectItem>
                      </SelectContent>
                    </Select>
                  </div>
                  <div className="space-y-1">
                    <div className="text-xs uppercase tracking-[0.14em] text-muted-foreground">Problem type</div>
                    <Select value={solutionProblemType} onValueChange={setSolutionProblemType}>
                      <SelectTrigger className="min-w-[180px]">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="all">All problems</SelectItem>
                        <SelectItem value="SubsetSum">SubsetSum</SelectItem>
                        <SelectItem value="SAT">SAT</SelectItem>
                        <SelectItem value="TSP">TSP</SelectItem>
                        <SelectItem value="Custom">Custom</SelectItem>
                      </SelectContent>
                    </Select>
                  </div>
                </div>
              </CardHeader>
              <CardContent>
                {solutionState === "loading" ? (
                  <div className="space-y-3 py-8">
                    <div className="flex items-center justify-center">
                      <Loader2 className="h-6 w-6 animate-spin text-primary" />
                    </div>
                    <p className="text-center text-sm text-muted-foreground">Loading the latest verified solution output…</p>
                  </div>
                ) : solutionState === "unavailable" ? (
                  <DataStateCard
                    title="Live solution browser unavailable"
                    body="We could not reach the live solution feed right now. Try again shortly while the chain keeps producing new work."
                    tone="warning"
                  />
                ) : solutionState === "empty" ? (
                  <DataStateCard
                    title="No solution rows for this view yet"
                    body="This filter is ready, but there are no visible solution rows yet. Try another problem type or check back after more blocks land."
                  />
                ) : (
                  <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
                    {solutionSets.map((solution) => (
                      <SolutionSetCard key={solution.id} solution={solution} />
                    ))}
                  </div>
                )}
              </CardContent>
            </Card>

            <div className="flex justify-center">
              <Card className="glass-effect w-full max-w-4xl border-primary/20 bg-gradient-to-br from-primary/10 via-background to-background">
                <CardContent className="flex flex-col gap-4 p-6 md:flex-row md:items-center md:justify-between">
                  <div className="space-y-2">
                    <div className="text-sm font-semibold text-foreground">
                      {isAuthenticated ? "Your account is ready for dataset exploration" : "Create a free account to explore datasets"}
                    </div>
                    <p className="text-sm text-muted-foreground">
                      {isAuthenticated
                        ? "Stay in the live feed, then open the dataset layer when you want to go deeper."
                        : "Preview the live chain now, then register when you want guided dataset access and stronger tooling."}
                    </p>
                  </div>
                  <div className="flex flex-col gap-3 sm:flex-row">
                    <Button onClick={handleDatasetAction}>
                      {isAuthenticated ? "Explore dataset tools" : "Create free account"}
                      <ArrowRight className="h-4 w-4" />
                    </Button>
                    {!isAuthenticated && (
                      <Button variant="outline" onClick={openRegistration}>
                        Register to explore datasets
                        <Download className="h-4 w-4" />
                      </Button>
                    )}
                  </div>
                </CardContent>
              </Card>
            </div>

            <Card className="glass-effect max-w-6xl mx-auto">
              <CardHeader>
                <CardTitle className="text-xl">Dataset catalog</CardTitle>
                <CardDescription>
                  Follow-on products built from the live chain output above.
                </CardDescription>
              </CardHeader>
              <CardContent>
                {datasetState === "loading" ? (
                  <div className="space-y-3 py-8">
                    <div className="flex items-center justify-center">
                      <Loader2 className="h-8 w-8 animate-spin text-primary" />
                    </div>
                    <p className="text-center text-sm text-muted-foreground">Loading dataset previews…</p>
                  </div>
                ) : datasetState === "unavailable" ? (
                  <DataStateCard
                    title="Dataset catalog unavailable"
                    body="The live dataset catalog is temporarily unavailable. The chain preview above is still live, and the catalog will return shortly."
                    tone="warning"
                  />
                ) : datasetState === "empty" ? (
                  <DataStateCard
                    title="No dataset previews available yet"
                    body="The dataset catalog is ready, but there are no published previews to show yet."
                  />
                ) : (
                  <div className="grid grid-cols-1 xl:grid-cols-2 gap-6">
                {normalizedDatasets.map((dataset) => {
                  const visual = datasetVisuals[dataset.dataset_type] ?? datasetVisuals.marketplace_events;
                  const Icon = visual.icon;
                  const snapshot = dataset.latest_snapshot;

                  return (
                    <Card key={dataset.slug} className="border-border/70 hover:border-primary/40 transition-colors">
                      <CardHeader className="space-y-4">
                        <div className="flex items-start justify-between gap-4">
                          <div className="flex items-start gap-4">
                            <div className="p-3 rounded-xl bg-primary/10">
                              <Icon className="h-6 w-6 text-primary" />
                            </div>
                            <div>
                              <div className="text-xs uppercase tracking-[0.18em] text-muted-foreground mb-2">
                                {visual.eyebrow}
                              </div>
                              <CardTitle className="text-xl leading-tight">{dataset.title}</CardTitle>
                            </div>
                          </div>
                          <Badge variant="outline" className={cn("shrink-0", visual.badgeClass)}>
                            {isAuthenticated ? "Explore now" : "Preview + register"}
                          </Badge>
                        </div>
                        <CardDescription className="text-sm leading-relaxed">
                          {dataset.description || "Marketplace-ready dataset snapshot from COINjecture chain activity."}
                        </CardDescription>
                      </CardHeader>
                      <CardContent className="space-y-5">
                        <div className="grid grid-cols-2 gap-3 text-sm">
                          <MetricPill label="Version" value={snapshot?.version || "Preview"} />
                          <MetricPill
                            label="Rows"
                            value={typeof snapshot?.row_count === "number" ? snapshot.row_count.toLocaleString() : "Growing"}
                          />
                          <MetricPill
                            label="Height"
                            value={typeof snapshot?.end_height === "number" ? snapshot.end_height.toLocaleString() : "Live"}
                          />
                          <MetricPill label="Access" value={isAuthenticated ? "Unlocked" : "Register"} />
                        </div>

                        <div className="rounded-xl border border-border/60 bg-muted/20 p-4">
                          <div className="text-xs font-mono uppercase tracking-[0.16em] text-muted-foreground mb-2">
                            What you can do with it
                          </div>
                          <p className="text-sm text-foreground leading-relaxed">
                            {datasetUseCase(dataset.slug)}
                          </p>
                        </div>

                        <div className="flex flex-col sm:flex-row gap-3">
                          <Button className="sm:flex-1" onClick={handleDatasetAction}>
                            <Download className="h-4 w-4" />
                            {isAuthenticated ? "Explore dataset" : "Create free account"}
                          </Button>
                          <Button
                            variant="outline"
                            className="sm:flex-1"
                            onClick={isAuthenticated ? handleDatasetAction : openRegistration}
                          >
                            <Zap className="h-4 w-4" />
                            {isAuthenticated ? "Unlock dataset tools" : "Register to explore"}
                          </Button>
                        </div>
                      </CardContent>
                    </Card>
                  );
                })}
                  </div>
                )}
              </CardContent>
            </Card>
          </TabsContent>
        </Tabs>
      </div>
    </section>
  );
};

function SignalStat({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-xl border border-border/60 bg-background/70 p-4">
      <div className="text-xs uppercase tracking-[0.16em] text-muted-foreground mb-1">{label}</div>
      <div className="text-lg font-semibold">{value}</div>
    </div>
  );
}

function RegisterToExploreCard({
  isAuthenticated,
  onPrimaryAction,
  onSecondaryAction,
}: {
  isAuthenticated: boolean;
  onPrimaryAction: () => void;
  onSecondaryAction: () => void;
}) {
  return (
    <Card className="glass-effect">
      <CardHeader>
        <CardTitle className="text-lg">
          {isAuthenticated ? "Go deeper on the live dataset layer" : "Preview now, register when you want to interact"}
        </CardTitle>
        <CardDescription>
          {isAuthenticated
            ? "You can move from passive chain watching into richer dataset exploration as the tooling layer expands."
            : "The live previews are open to everyone. A free account is the unlock for exploring datasets more deeply."}
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <ValueBullet icon={Database} text="See recent outputs before you commit to any workflow." />
        <ValueBullet icon={Sparkles} text="Turn live chain visibility into structured dataset exploration." />
        <ValueBullet icon={Cpu} text="Jump from chain output to Solver Lab whenever you want to act on what you see." />
        <div className="flex flex-col gap-3">
          <Button onClick={onPrimaryAction}>
            {isAuthenticated ? "Explore dataset tools" : "Create free account"}
            <ArrowRight className="h-4 w-4" />
          </Button>
          {!isAuthenticated && (
            <Button variant="outline" onClick={onSecondaryAction}>
              Register to explore datasets
              <Download className="h-4 w-4" />
            </Button>
          )}
        </div>
      </CardContent>
    </Card>
  );
}

function DataStateCard({
  title,
  body,
  tone = "default",
}: {
  title: string;
  body: string;
  tone?: "default" | "warning";
}) {
  return (
    <div
      className={cn(
        "rounded-xl border p-6 text-sm",
        tone === "warning"
          ? "border-warning/30 bg-warning/5 text-muted-foreground"
          : "border-border/60 bg-muted/20 text-muted-foreground",
      )}
    >
      <div className="font-semibold text-foreground mb-2">{title}</div>
      <div>{body}</div>
    </div>
  );
}

function ValueBullet({
  icon: Icon,
  text,
}: {
  icon: typeof Sparkles | typeof Activity | typeof Database | typeof BrainCircuit;
  text: string;
}) {
  return (
    <div className="flex items-start gap-3">
      <div className="mt-0.5 rounded-md bg-primary/10 p-1.5">
        <Icon className="h-4 w-4 text-primary" />
      </div>
      <span>{text}</span>
    </div>
  );
}

function MetricPill({
  label,
  value,
  tone = "default",
}: {
  label: string;
  value: string;
  tone?: "default" | "warning";
}) {
  return (
    <div
      className={cn(
        "rounded-lg border border-border/60 bg-background/70 px-3 py-2",
        tone === "warning" && "border-warning/30 bg-warning/5",
      )}
    >
      <div className="text-[11px] uppercase tracking-[0.14em] text-muted-foreground">{label}</div>
      <div className="text-sm font-medium mt-1">{value}</div>
    </div>
  );
}

function datasetUseCase(slug: string) {
  switch (slug) {
    case "marketplace-events-by-block":
      return "Build dashboards around solver demand, bounty creation cadence, and network-level marketplace momentum.";
    case "problem-submissions-and-solutions":
      return "Create benchmark corpora for reasoning systems using verified problem-solution pairs with on-chain provenance.";
    case "bounty-payout-history":
      return "Audit who earned what, when rewards were released, and how useful work translated into value.";
    case "trading-and-liquidity-activity":
      return "Study how token markets respond as computation, rewards, and speculation start feeding the same flywheel.";
    default:
      return "Explore how useful work, market signals, and on-chain verification interact inside the COINjecture economy.";
  }
}

function getDataState(loading: boolean, error: unknown, count: number) {
  if (loading) return "loading";
  if (error) return "unavailable";
  if (count === 0) return "empty";
  return "ready";
}

function formatMetricNumber(value: number | null | undefined) {
  if (typeof value !== "number") {
    return "n/a";
  }
  return value.toFixed(3);
}

function formatIntegerMetric(value: number | null | undefined) {
  if (typeof value !== "number") {
    return "n/a";
  }
  return value.toLocaleString();
}

function DetailPanel({ title, content }: { title: string; content: unknown }) {
  return (
    <div className="rounded-xl border border-border/60 bg-muted/20 p-4">
      <div className="mb-2 text-xs uppercase tracking-[0.14em] text-muted-foreground">{title}</div>
      <pre className="overflow-x-auto whitespace-pre-wrap break-words text-xs text-foreground">
        {formatJsonContent(content)}
      </pre>
    </div>
  );
}

function formatJsonContent(content: unknown) {
  if (content === null || content === undefined) {
    return "Not available";
  }

  if (typeof content === "string") {
    return content;
  }

  try {
    return JSON.stringify(content, null, 2);
  } catch {
    return String(content);
  }
}
