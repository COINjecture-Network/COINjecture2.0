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
  LineChart,
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
import { API_BASE } from "@/lib/api/client";

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
    storage_path?: string | null;
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

const getReadableProblemLabel = (type: string | null, isPrivate: boolean) => {
  if (!type) {
    return isPrivate ? "Private Problem" : "Unknown";
  }

  if (type.startsWith("SubsetSum")) {
    return "SubsetSum";
  }

  if (type.startsWith("SAT")) {
    return "Boolean SAT";
  }

  if (type.startsWith("TSP")) {
    return "TSP";
  }

  if (type.startsWith("Custom")) {
    return "Custom Problem";
  }

  return type;
};

function getComplexityLabel(problem: ProblemInfo) {
  const size = typeof problem.problem_size === "number" ? problem.problem_size : null;

  if (size === null) {
    return "Network sized";
  }

  if (size <= 8) {
    return "Small";
  }

  if (size <= 64) {
    return "Medium";
  }

  if (size <= 512) {
    return "Large";
  }

  return "High complexity";
}

const ProblemCard = ({ problem }: { problem: ProblemInfo }) => {
  const expirationDate = new Date(problem.expires_at * 1000);
  const submittedDate = new Date(problem.submitted_at * 1000);
  const isExpiringSoon = expirationDate.getTime() - Date.now() < 24 * 60 * 60 * 1000;
  const statusLabel = problem.status === "OPEN" ? "Live bounty" : problem.status;
  const minWorkLabel = typeof problem.min_work_score === "number" ? problem.min_work_score.toString() : "Open";
  const urgencyLabel = isExpiringSoon ? "Closing soon" : "Plenty of runway";
  const readableProblemLabel = getReadableProblemLabel(problem.problem_type, problem.is_private);
  const canSolveInLab = Boolean(problem.problem) && (!problem.is_private || problem.is_revealed);

  return (
    <Card className="border-border/70 bg-background hover:border-primary/40 hover:shadow-[0_18px_60px_rgba(0,0,0,0.12)] transition-all">
      <CardContent className="p-6 space-y-5">
        <div className="flex flex-col gap-4 xl:flex-row xl:items-start xl:justify-between">
          <div className="min-w-0">
            <div className="mb-2 flex flex-wrap items-center gap-2">
              <Badge variant="outline" className="border-primary/20 text-primary">
                Solver demand
              </Badge>
              <Badge variant={problem.status === "OPEN" ? "default" : "secondary"}>
                {statusLabel}
              </Badge>
              {problem.is_private && (
                <Badge variant="outline">{problem.is_revealed ? "Revealed" : "Private"}</Badge>
              )}
            </div>
            <h3 className="text-lg font-semibold leading-snug break-words">{readableProblemLabel}</h3>
            <p className="text-sm text-muted-foreground">
              Problem ID <code className="text-xs">{problem.problem_id.slice(0, 16)}...</code>
            </p>
          </div>
          <div className="rounded-xl border border-primary/20 bg-primary/10 px-3 py-2 text-left xl:min-w-[144px] xl:text-right">
            <div className="text-[11px] uppercase tracking-[0.14em] text-muted-foreground">Payout</div>
            <div className="text-lg font-semibold text-primary">{problem.bounty.toLocaleString()} BEANS</div>
            <div className="text-[11px] text-muted-foreground mt-1">Available now</div>
          </div>
        </div>

        <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 text-sm">
          <MetricPill label="Difficulty floor" value={minWorkLabel} />
          <MetricPill label="Complexity" value={getComplexityLabel(problem)} />
          <MetricPill label="Submitted" value={formatDistanceToNow(submittedDate, { addSuffix: true })} />
          <MetricPill
            label="Window"
            value={urgencyLabel}
            tone={isExpiringSoon ? "warning" : "default"}
          />
        </div>

        <div className="rounded-xl border border-border/60 bg-muted/20 p-4 text-sm">
          <div className="text-[11px] uppercase tracking-[0.16em] text-muted-foreground mb-2">Buyer brief</div>
          <p className="text-foreground leading-relaxed">
            This listing is live on-chain right now. Inspect the structure, estimate the effort, and jump into Solver Lab when the reward looks worth taking.
          </p>
          <div className="mt-3 text-muted-foreground">
            Closes {formatDistanceToNow(expirationDate, { addSuffix: true })}
          </div>
        </div>

        <div className="flex flex-col sm:flex-row gap-3">
          <Button asChild className="sm:flex-1" disabled={problem.status !== "OPEN" || !canSolveInLab}>
            <Link
              to={`/solver-lab?problemId=${encodeURIComponent(problem.problem_id)}`}
              state={{ selectedBounty: problem }}
            >
              {canSolveInLab ? "Solve this bounty" : "Await reveal"}
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
          <Card className="border-border/60 bg-background transition-all hover:border-primary/40 hover:shadow-[0_16px_48px_rgba(0,0,0,0.10)]">
            <CardContent className="p-5">
              <div className="flex items-start justify-between gap-3 mb-3">
                <div>
                  <div className="text-[11px] uppercase tracking-[0.16em] text-muted-foreground mb-1">
                    Verified inventory
                  </div>
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
              <div className="mt-3 flex items-center justify-between text-xs">
                <span className="text-muted-foreground">Preview the payload and metrics before export.</span>
                <span className="text-primary">Inspect listing</span>
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

  const handleDatasetDownload = (slug: string) => {
    window.open(`${API_BASE}/marketplace/datasets/${encodeURIComponent(slug)}/download`, "_blank", "noopener,noreferrer");
  };

  const liveSolutionCount = solutionSets.length.toString();
  const topSolution = solutionSets[0];
  const featuredProblem = problems?.[0];
  const featuredDataset = normalizedDatasets[0];
  const bountyCtaLabel = isAuthenticated ? "Open Solver Lab" : "Register and solve";
  const solutionState = getDataState(solutionSetsLoading, solutionSetsError, solutionSets.length);
  const datasetState = getDataState(datasetsLoading, datasetsError, normalizedDatasets.length);

  return (
    <section id="marketplace" className="py-20">
      <div className="container mx-auto px-6">
        <Card className="overflow-hidden border-primary/20 mb-12 bg-[radial-gradient(circle_at_top_left,rgba(147,51,234,0.14),transparent_34%),radial-gradient(circle_at_top_right,rgba(59,130,246,0.12),transparent_28%),linear-gradient(135deg,rgba(255,255,255,0.98),rgba(248,250,252,0.96))] text-foreground dark:bg-[radial-gradient(circle_at_top_left,rgba(147,51,234,0.22),transparent_34%),radial-gradient(circle_at_top_right,rgba(59,130,246,0.18),transparent_28%),linear-gradient(135deg,rgba(14,14,24,0.96),rgba(10,10,16,0.92))] dark:text-white">
          <div className="grid gap-8 px-6 py-8 md:grid-cols-[1.1fr_0.9fr] md:px-8">
            <div>
              <div className="flex flex-wrap items-center gap-2 mb-4">
                <Badge variant="secondary" className="text-xs bg-foreground/5 text-foreground border-foreground/10 dark:bg-white/10 dark:text-white dark:border-white/10">Live market</Badge>
                <Badge variant="outline" className="text-xs border-foreground/15 text-foreground/80 dark:border-white/20 dark:text-white/90">
                  {isAuthenticated ? "Signed in and ready to act" : "Browse free, then unlock"}
                </Badge>
              </div>
              <h2 className="text-4xl md:text-5xl font-bold tracking-tight mb-4 leading-tight">
                The live market for <span className="text-primary">solver payouts, verified outputs, and premium chain data</span>
              </h2>
              <p className="text-muted-foreground dark:text-white/75 max-w-3xl leading-relaxed mb-6 text-lg">
                High-signal bounties. Freshly verified solution inventory. Productized datasets with proof and provenance.
              </p>
              <div className="grid grid-cols-1 sm:grid-cols-3 gap-4 mb-5">
                <SignalStat
                  label="Open listings"
                  value={statsLoading ? "Loading" : stats ? stats.open_problems.toLocaleString() : "Live"}
                  dark
                />
                <SignalStat label="Fresh outputs" value={solutionSetsLoading ? "Loading" : liveSolutionCount} dark />
                <SignalStat
                  label="Market mode"
                  value={isAuthenticated ? "Buyer active" : "Preview mode"}
                  dark
                />
              </div>
              <div className="flex flex-wrap gap-3 text-sm text-foreground/80 dark:text-white/80">
                <MarketChip icon={Zap} text="Highest payout first" />
                <MarketChip icon={ShieldCheck} text="Chain-backed proof" />
                <MarketChip icon={Sparkles} text="Fresh inventory every block" />
              </div>
            </div>
            <div className="grid gap-4">
              <FeatureSpotlight
                eyebrow="Featured opportunity"
                title={featuredProblem ? getProblemTypeLabel(featuredProblem.problem_type || "Unknown") : "Live bounty feed"}
                value={featuredProblem ? `${featuredProblem.bounty.toLocaleString()} BEANS` : "Loading"}
                description={
                  featuredProblem
                    ? `${featuredProblem.status === "OPEN" ? "Open now" : featuredProblem.status} • closes ${formatDistanceToNow(new Date(featuredProblem.expires_at * 1000), { addSuffix: true })}`
                    : "Loading the highest-signal solver opportunity."
                }
                tone="bounty"
              />
              <FeatureSpotlight
                eyebrow="Featured product"
                title={featuredDataset?.title || "Dataset catalog"}
                value={featuredDataset?.latest_snapshot?.version || "Preview"}
                description={
                  featuredDataset
                    ? `${featuredDataset.latest_snapshot?.row_count?.toLocaleString() || "Growing"} rows • ${featuredDataset.latest_snapshot?.status || "Preview ready"}`
                    : "Loading the first premium chain dataset."
                }
                tone="dataset"
              />
            </div>
          </div>
        </Card>

        <Tabs value={marketplaceTab} onValueChange={setMarketplaceTab}>
          <div className="flex flex-col gap-5 md:flex-row md:items-end md:justify-between mb-8">
            <div>
              <h3 className="text-3xl font-bold">Choose your side of the market</h3>
              <p className="text-muted-foreground">
                Hunt payouts on one side. Shop verified chain products on the other.
              </p>
            </div>
            <TabsList className="w-full md:w-auto h-auto grid grid-cols-2 rounded-2xl border border-border/60 bg-muted/30 p-1.5 md:min-w-[360px] md:shadow-sm">
              <TabsTrigger
                value="bounties"
                className="rounded-xl px-6 py-4 text-base font-semibold data-[state=active]:bg-background data-[state=active]:shadow-sm"
              >
                Earn
              </TabsTrigger>
              <TabsTrigger
                value="data"
                className="rounded-xl px-6 py-4 text-base font-semibold data-[state=active]:bg-background data-[state=active]:shadow-sm"
              >
                Buy
              </TabsTrigger>
            </TabsList>
          </div>

          <TabsContent value="bounties" className="space-y-6">
            <div className="grid gap-6 xl:grid-cols-[0.95fr_1.05fr]">
              <Card className="border-border/70 bg-background">
                <CardHeader>
                  <div className="mb-2 flex flex-wrap items-center gap-2">
                    <Badge variant="secondary">Active listings</Badge>
                    <Badge variant="outline">Solver-side marketplace</Badge>
                  </div>
                  <CardTitle className="text-2xl xl:text-3xl leading-tight">Earn from live demand</CardTitle>
                  <CardDescription className="leading-relaxed">
                    Start with the biggest payout that matches your solving style. Move fast when a listing is heating up.
                  </CardDescription>
                </CardHeader>
                <CardContent className="space-y-4">
                  <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 text-sm">
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
                  <Card className="p-8">
                    <div className="flex items-center justify-center py-8">
                      <Loader2 className="h-8 w-8 animate-spin text-primary" />
                    </div>
                    <p className="text-center text-sm text-muted-foreground">Loading the latest on-chain bounty feed…</p>
                  </Card>
                )}

                {problemsError && (
                  <Card className="border-warning/30 bg-warning/5">
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
                  <Card className="p-8 text-center">
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
              <Card className="border-border/70 bg-background">
                <CardHeader>
                  <div className="flex items-center gap-2 mb-2">
                    <Badge variant="secondary">Featured inventory</Badge>
                    <Badge variant="outline" className="border-primary/30 text-primary">
                      {isAuthenticated ? "Buyer tools active" : "Preview mode"}
                    </Badge>
                  </div>
                  <CardTitle className="text-3xl">Buy into the network’s output layer</CardTitle>
                  <CardDescription className="leading-relaxed">
                    The proof is live below. The products are the snapshots, feeds, and structured exports built from it.
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

                  <div className="flex flex-col sm:flex-row gap-3">
                    <Button
                      className="sm:flex-1"
                      onClick={() =>
                        isAuthenticated && featuredDataset
                          ? handleDatasetDownload(featuredDataset.slug)
                          : openRegistration()
                      }
                    >
                      {isAuthenticated ? "Download featured dataset" : "Create free account"}
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
                onPrimaryAction={() =>
                  isAuthenticated && featuredDataset
                    ? handleDatasetDownload(featuredDataset.slug)
                    : openRegistration()
                }
                onSecondaryAction={openRegistration}
              />
            </div>

            <Card className="max-w-6xl mx-auto border-border/70 bg-background">
              <CardHeader className="gap-4 md:flex-row md:items-end md:justify-between">
                <div>
                  <CardTitle className="text-xl">Verified solution sets</CardTitle>
                  <CardDescription>
                    Recent on-chain solutions shown like browsable inventory, sortable by value and type.
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
              <Card className="w-full max-w-4xl border-primary/20 bg-gradient-to-br from-primary/10 via-background to-background">
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
                    <Button
                      onClick={() =>
                        isAuthenticated && featuredDataset
                          ? handleDatasetDownload(featuredDataset.slug)
                          : openRegistration()
                      }
                    >
                      {isAuthenticated ? "Download featured dataset" : "Create free account"}
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

            <Card className="max-w-6xl mx-auto border-border/70 bg-background">
              <CardHeader>
                <CardTitle className="text-xl">Dataset catalog</CardTitle>
                <CardDescription>
                  Productized snapshots built from the live chain output above.
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
                    <Card key={dataset.slug} className="border-border/70 bg-background hover:border-primary/40 hover:shadow-[0_18px_60px_rgba(0,0,0,0.12)] transition-all">
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
                            {isAuthenticated ? "Browse product" : "Preview product"}
                          </Badge>
                        </div>
                        <CardDescription className="text-sm leading-relaxed">
                          {dataset.description || "Marketplace-ready dataset snapshot from COINjecture chain activity."}
                        </CardDescription>
                      </CardHeader>
                      <CardContent className="space-y-5">
                        <div className="flex items-center justify-between rounded-xl border border-border/60 bg-muted/20 px-4 py-3">
                          <div>
                            <div className="text-[11px] uppercase tracking-[0.16em] text-muted-foreground">Starting price</div>
                            <div className="text-xl font-semibold text-foreground">
                              {dataset.price > 0 ? `${dataset.price.toLocaleString()} ${dataset.currency}` : "Free preview"}
                            </div>
                          </div>
                          <div className="text-right text-xs text-muted-foreground">
                            <div>{snapshot?.status || "Preview ready"}</div>
                            <div>{snapshot?.checksum ? `Checksum ${snapshot.checksum.slice(0, 8)}…` : "Chain-backed snapshot"}</div>
                          </div>
                        </div>

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
                            Why teams would buy this
                          </div>
                          <p className="text-sm text-foreground leading-relaxed">
                            {datasetUseCase(dataset.slug)}
                          </p>
                        </div>

                        <div className="flex flex-col sm:flex-row gap-3">
                          <Button className="sm:flex-1" onClick={() => handleDatasetDownload(dataset.slug)}>
                            <Download className="h-4 w-4" />
                            Download dataset
                          </Button>
                          <Button
                            variant="outline"
                            className="sm:flex-1"
                            onClick={() => handleDatasetDownload(dataset.slug)}
                          >
                            <Zap className="h-4 w-4" />
                            Download JSON
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

function SignalStat({ label, value, dark = false }: { label: string; value: string; dark?: boolean }) {
  return (
    <div
      className={cn(
        "rounded-xl border p-4",
        dark ? "border-white/10 bg-white/5" : "border-foreground/10 bg-background/75",
      )}
    >
      <div className={cn("text-xs uppercase tracking-[0.16em] mb-1", dark ? "text-white/60" : "text-foreground/55")}>
        {label}
      </div>
      <div className={cn("text-lg font-semibold", dark ? "text-white" : "text-foreground")}>{value}</div>
    </div>
  );
}

function MarketChip({
  icon: Icon,
  text,
}: {
  icon: typeof Zap | typeof ShieldCheck | typeof Sparkles;
  text: string;
}) {
  return (
    <div className="inline-flex items-center gap-2 rounded-full border border-foreground/10 bg-foreground/5 px-3 py-2 dark:border-white/10 dark:bg-white/5">
      <Icon className="h-4 w-4 text-primary" />
      <span>{text}</span>
    </div>
  );
}

function FeatureSpotlight({
  eyebrow,
  title,
  value,
  description,
  tone,
}: {
  eyebrow: string;
  title: string;
  value: string;
  description: string;
  tone: "bounty" | "dataset";
}) {
  return (
    <div
      className={cn(
        "rounded-2xl border p-5 backdrop-blur-sm",
        tone === "bounty"
          ? "border-primary/25 bg-primary/10 text-foreground dark:border-primary/30 dark:bg-primary/10 dark:text-white"
          : "border-foreground/10 bg-background/60 text-foreground dark:border-white/10 dark:bg-white/5 dark:text-white",
      )}
    >
      <div className="text-[11px] uppercase tracking-[0.18em] text-foreground/55 dark:text-white/60 mb-2">{eyebrow}</div>
      <div className="text-xl font-semibold leading-tight">{title}</div>
      <div className="mt-3 text-3xl font-bold text-primary">{value}</div>
      <div className="mt-2 text-sm text-muted-foreground dark:text-white/70 leading-relaxed">{description}</div>
    </div>
  );
}

function MerchFeature({
  icon: Icon,
  title,
  body,
}: {
  icon: typeof Sparkles | typeof Zap | typeof Database | typeof ShieldCheck;
  title: string;
  body: string;
}) {
  return (
    <div className="rounded-xl border border-border/60 bg-background/75 p-4">
      <div className="flex items-center gap-2 mb-2">
        <div className="rounded-lg bg-primary/10 p-2">
          <Icon className="h-4 w-4 text-primary" />
        </div>
        <div className="font-medium">{title}</div>
      </div>
      <p className="text-sm text-muted-foreground leading-relaxed">{body}</p>
    </div>
  );
}

function ShelfRow({
  label,
  value,
  note,
}: {
  label: string;
  value: string;
  note: string;
}) {
  return (
    <div className="rounded-xl border border-border/60 bg-muted/20 px-4 py-3">
      <div className="flex items-start justify-between gap-4">
        <div>
          <div className="text-[11px] uppercase tracking-[0.16em] text-muted-foreground">{label}</div>
          <div className="mt-1 text-lg font-semibold text-foreground">{value}</div>
        </div>
        <div className="max-w-[180px] text-right text-xs text-muted-foreground leading-relaxed">{note}</div>
      </div>
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
