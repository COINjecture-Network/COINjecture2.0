import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  Activity,
  Award,
  Blocks,
  Calculator,
  CheckCircle,
  ChevronLeft,
  ChevronRight,
  Clock,
  Loader2,
  Network,
  Search,
  Target,
  Triangle,
  Waves,
} from "lucide-react";
import {
  Bar,
  BarChart,
  CartesianGrid,
  Legend,
  Line,
  LineChart,
  ReferenceLine,
  ResponsiveContainer,
  Scatter,
  ScatterChart,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { Navigation } from "@/components/Navigation";
import { Footer } from "@/components/Footer";
import { rpcClient, type Block, type ChainInfo } from "@/lib/rpc-client";
import {
  MetricsClient,
  type AllPoolsData,
  type ConsensusState,
  type ConvergenceMetrics,
  type SatoshiConstants,
} from "@/lib/metrics-client";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@/components/ui/accordion";
import { cn } from "@/lib/utils";

const metricsClient = new MetricsClient();

const POOL_INFO = [
  { name: "D1", label: "Genesis", tau: 0.0, d_n: 1.0, color: "hsl(var(--chart-1))" },
  { name: "D2", label: "Coupling", tau: 0.2, d_n: 0.867, color: "hsl(var(--chart-2))" },
  { name: "D3", label: "First Harmonic", tau: 0.41, d_n: 0.75, color: "hsl(var(--chart-3))" },
  { name: "D4", label: "Golden Ratio", tau: 0.68, d_n: 0.618, color: "hsl(var(--chart-4))" },
  { name: "D5", label: "Half-scale", tau: 0.98, d_n: 0.5, color: "hsl(var(--chart-5))" },
  { name: "D6", label: "Second Golden", tau: 1.36, d_n: 0.382, color: "hsl(var(--destructive))" },
  { name: "D7", label: "Quarter-scale", tau: 1.96, d_n: 0.25, color: "hsl(199 89% 48%)" },
  { name: "D8", label: "Euler", tau: 2.72, d_n: 0.146, color: "hsl(142 76% 36%)" },
] as const;

const ETA = 1 / Math.sqrt(2);
const LAMBDA = 1 / Math.sqrt(2);
const TAU_C = Math.sqrt(2);

function rewardDisplay(block: Block): string {
  const r = block.coinbase?.reward;
  if (r === undefined || r === null) return "—";
  if (typeof r === "bigint") return r.toString();
  if (typeof r === "number") return r.toLocaleString();
  return String(r);
}

function txRows(block: Block): { key: string; line: string }[] {
  if (!Array.isArray(block.transactions)) return [];
  return block.transactions.map((tx, i) => ({
    key: `tx-${i}`,
    line: typeof tx === "object" && tx !== null ? JSON.stringify(tx).slice(0, 200) : String(tx),
  }));
}

export default function Explore() {
  const [selectedHeight, setSelectedHeight] = useState<number | null>(null);
  const [searchHeight, setSearchHeight] = useState("");
  const [showPeers, setShowPeers] = useState(false);

  const { data: chainInfo } = useQuery({
    queryKey: ["explore", "chainInfo"],
    queryFn: () => rpcClient.getChainInfo(),
    refetchInterval: 10_000,
  });

  const { data: allPools, error: poolsError } = useQuery({
    queryKey: ["explore", "allPools"],
    queryFn: () => metricsClient.getAllPoolsData(),
    refetchInterval: 5_000,
    retry: 1,
  });

  const { data: consensus, error: consensusError } = useQuery({
    queryKey: ["explore", "consensus"],
    queryFn: () => metricsClient.getConsensusState(),
    refetchInterval: 5_000,
    retry: 1,
  });

  const { data: constants, error: constantsError } = useQuery({
    queryKey: ["explore", "satoshiConstants"],
    queryFn: () => metricsClient.getSatoshiConstants(),
    refetchInterval: 10_000,
    retry: 1,
  });

  const { data: metricsHeight, error: heightError } = useQuery({
    queryKey: ["explore", "metricsBlockHeight"],
    queryFn: () => metricsClient.getBlockHeight(),
    refetchInterval: 10_000,
    retry: 1,
  });

  const { data: convergence, error: convError } = useQuery({
    queryKey: ["explore", "convergence"],
    queryFn: () => metricsClient.getConvergenceMetrics(),
    refetchInterval: 5_000,
    retry: 1,
  });

  const metricsErr =
    poolsError || consensusError || constantsError || heightError || convError
      ? "Prometheus metrics unavailable (check node exporter / VITE_METRICS_URL)."
      : null;

  const onSearch = () => {
    const h = parseInt(searchHeight, 10);
    if (!Number.isNaN(h) && h >= 0) setSelectedHeight(h);
  };

  return (
    <div className="min-h-screen">
      <Navigation />
      <main className="container mx-auto space-y-8 px-4 pb-20 pt-28 sm:px-6">
        <div>
          <h1 className="font-brand text-3xl font-bold tracking-tight">Explore</h1>
          <p className="mt-1 max-w-2xl text-sm text-muted-foreground">
            Chain overview, blocks, and live Prometheus metrics (pools, consensus, oracle).
          </p>
        </div>

        {metricsErr && (
          <Alert variant="destructive">
            <AlertTitle>Metrics</AlertTitle>
            <AlertDescription>{metricsErr}</AlertDescription>
          </Alert>
        )}

        <ChainStatsBar
          chainInfo={chainInfo}
          metricsBlockHeight={metricsHeight}
          showPeers={showPeers}
          onTogglePeers={() => setShowPeers((v) => !v)}
        />

        <div className="grid gap-6 lg:grid-cols-2">
          <div className="space-y-6">
            <Card>
              <CardHeader className="pb-2">
                <CardTitle className="text-base">Block search</CardTitle>
                <CardDescription>Jump to a height from the canonical RPC tip.</CardDescription>
              </CardHeader>
              <CardContent className="flex gap-2">
                <Input
                  type="number"
                  min={0}
                  placeholder="Height"
                  value={searchHeight}
                  onChange={(e) => setSearchHeight(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && onSearch()}
                />
                <Button type="button" variant="secondary" className="shrink-0 gap-1" onClick={onSearch}>
                  <Search className="h-4 w-4" />
                  Go
                </Button>
              </CardContent>
            </Card>

            <LatestBlocksCard
              tip={chainInfo?.best_height ?? 0}
              selected={selectedHeight}
              onSelect={setSelectedHeight}
            />
          </div>

          <div>
            {selectedHeight !== null ? (
              <BlockDetailCard height={selectedHeight} maxHeight={chainInfo?.best_height ?? 0} onNavigate={setSelectedHeight} />
            ) : (
              <Card className="flex min-h-[320px] flex-col items-center justify-center text-muted-foreground">
                <Blocks className="mb-3 h-12 w-12 opacity-40" />
                <p className="text-sm">Select a block from the list or search by height.</p>
              </Card>
            )}
          </div>
        </div>

        <TimeseriesSection consensus={consensus} />

        <StakingPoolsSection allPools={allPools} />

        <Accordion type="single" collapsible className="w-full rounded-lg border bg-card">
          <AccordionItem value="advanced">
            <AccordionTrigger className="px-4 text-left font-semibold">
              Advanced — wavefunction, oracle, reward calculator
            </AccordionTrigger>
            <AccordionContent className="space-y-6 px-4 pb-6">
              <WavefunctionPanel consensus={consensus} />
              <OraclePanel constants={constants} convergence={convergence} />
              <CalculatorPanel consensus={consensus} />
            </AccordionContent>
          </AccordionItem>
        </Accordion>
      </main>
      <Footer />
    </div>
  );
}

function ChainStatsBar({
  chainInfo,
  metricsBlockHeight,
  showPeers,
  onTogglePeers,
}: {
  chainInfo: ChainInfo | undefined;
  metricsBlockHeight: number | undefined;
  showPeers: boolean;
  onTogglePeers: () => void;
}) {
  return (
    <Card>
      <CardHeader className="pb-2">
        <CardTitle className="flex items-center gap-2 text-base">
          <Activity className="h-4 w-4" />
          Chain stats
        </CardTitle>
      </CardHeader>
      <CardContent className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-5">
        <StatPill icon={<Blocks className="h-4 w-4" />} label="Best height" value={chainInfo ? chainInfo.best_height.toLocaleString() : "…"} />
        <button
          type="button"
          onClick={onTogglePeers}
          className="flex items-center justify-between rounded-lg border bg-muted/30 px-3 py-2 text-left transition hover:bg-muted/50"
        >
          <StatPill icon={<Network className="h-4 w-4" />} label="Peers" value={chainInfo != null ? String(chainInfo.peer_count) : "…"} />
          <Badge variant="outline" className="text-[10px]">
            {showPeers ? "hide" : "info"}
          </Badge>
        </button>
        <StatPill
          icon={<Award className="h-4 w-4" />}
          label="Cumulative work"
          value={chainInfo?.best_cumulative_work ? String(chainInfo.best_cumulative_work).slice(0, 14) + "…" : "—"}
        />
        <StatPill
          icon={<Award className="h-4 w-4" />}
          label="NP size (next)"
          value={chainInfo?.np_problem_size != null ? String(chainInfo.np_problem_size) : "—"}
        />
        <StatPill
          icon={<Activity className="h-4 w-4" />}
          label="Metrics height"
          value={metricsBlockHeight != null ? metricsBlockHeight.toLocaleString() : "—"}
        />
      </CardContent>
      {showPeers && (
        <CardContent className="border-t pt-0">
          <p className="text-xs text-muted-foreground">
            Peer count comes from <code className="rounded bg-muted px-1">chain_getInfo</code>. LibP2P topology is
            node-specific.
          </p>
        </CardContent>
      )}
      <CardContent className="grid gap-2 border-t pt-4 text-xs">
        <div>
          <div className="text-muted-foreground">Best hash</div>
          <code className="break-all text-[11px]">{chainInfo?.best_hash ?? "…"}</code>
        </div>
        <div>
          <div className="text-muted-foreground">Genesis</div>
          <code className="break-all text-[11px]">{chainInfo?.genesis_hash ?? "…"}</code>
        </div>
        <div>
          <div className="text-muted-foreground">Chain ID</div>
          <code className="text-[11px]">{chainInfo?.chain_id ?? "…"}</code>
        </div>
      </CardContent>
    </Card>
  );
}

function StatPill({ icon, label, value }: { icon: React.ReactNode; label: string; value: string }) {
  return (
    <div className="rounded-lg border bg-muted/20 px-3 py-2">
      <div className="flex items-center gap-2 text-muted-foreground">
        {icon}
        <span className="text-xs font-medium uppercase tracking-wide">{label}</span>
      </div>
      <div className="mt-1 font-mono text-lg font-semibold tabular-nums">{value}</div>
    </div>
  );
}

function LatestBlocksCard({
  tip,
  selected,
  onSelect,
}: {
  tip: number;
  selected: number | null;
  onSelect: (h: number) => void;
}) {
  const heights = useMemo(() => {
    const n = Math.min(10, tip + 1);
    return Array.from({ length: n }, (_, i) => tip - i);
  }, [tip]);

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">Latest blocks</CardTitle>
        <CardDescription>Click a row to inspect header and PoUW fields.</CardDescription>
      </CardHeader>
      <CardContent className="space-y-2">
        {heights.map((h) => (
          <BlockListRow key={h} height={h} selected={selected === h} onClick={() => onSelect(h)} />
        ))}
      </CardContent>
    </Card>
  );
}

function BlockListRow({ height, selected, onClick }: { height: number; selected: boolean; onClick: () => void }) {
  const { data: block } = useQuery({
    queryKey: ["explore", "block", height],
    queryFn: () => rpcClient.getBlock(height),
    staleTime: 15_000,
  });

  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "flex w-full items-center justify-between rounded-lg border px-3 py-2 text-left text-sm transition",
        selected ? "border-primary bg-primary/5" : "border-border hover:bg-muted/40",
      )}
    >
      <div>
        <div className="font-medium">#{height}</div>
        <div className="text-xs text-muted-foreground">
          {block ? `${block.transactions.length} tx · work ${block.header.work_score.toFixed(2)}` : <Loader2 className="inline h-3 w-3 animate-spin" />}
        </div>
      </div>
      {block && (
        <span className="text-xs text-muted-foreground">{new Date(block.header.timestamp * 1000).toLocaleTimeString()}</span>
      )}
    </button>
  );
}

function BlockDetailCard({
  height,
  maxHeight,
  onNavigate,
}: {
  height: number;
  maxHeight: number;
  onNavigate: (h: number) => void;
}) {
  const { data: block, isLoading, error } = useQuery({
    queryKey: ["explore", "block", height],
    queryFn: () => rpcClient.getBlock(height),
  });

  if (isLoading) {
    return (
      <Card className="flex min-h-[280px] items-center justify-center">
        <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
      </Card>
    );
  }

  if (error || !block) {
    return (
      <Card className="border-destructive/50 p-6 text-center text-sm text-destructive">
        Block not found or RPC error.
      </Card>
    );
  }

  const solveMs = block.header.solve_time_us / 1000;
  const verifyMs = block.header.verify_time_us / 1000;

  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
        <CardTitle className="text-base">Block #{height}</CardTitle>
        <div className="flex gap-1">
          <Button type="button" size="icon" variant="outline" disabled={height <= 0} onClick={() => onNavigate(height - 1)}>
            <ChevronLeft className="h-4 w-4" />
          </Button>
          <Button type="button" size="icon" variant="outline" disabled={height >= maxHeight} onClick={() => onNavigate(height + 1)}>
            <ChevronRight className="h-4 w-4" />
          </Button>
        </div>
      </CardHeader>
      <CardContent className="space-y-3 text-sm">
        <DetailRow icon={<Clock className="h-4 w-4" />} label="Timestamp" value={new Date(block.header.timestamp * 1000).toLocaleString()} />
        <DetailRow icon={<Award className="h-4 w-4" />} label="Work score" value={block.header.work_score.toFixed(4)} />
        <DetailRow label="Coinbase reward" value={rewardDisplay(block)} />
        <DetailRow label="Nonce" value={String(block.header.nonce)} />
        <DetailRow label="Version" value={String(block.header.version)} />
        <div className="rounded-lg border border-primary/30 bg-primary/5 p-3">
          <div className="mb-2 text-xs font-semibold uppercase tracking-wide text-primary">PoUW</div>
          <div className="grid gap-1 text-xs">
            <RowKv label="Solve" value={`${solveMs.toFixed(3)} ms (${block.header.solve_time_us} μs)`} />
            <RowKv label="Verify" value={`${verifyMs.toFixed(3)} ms (${block.header.verify_time_us} μs)`} />
            <RowKv label="Asymmetry" value={`${block.header.time_asymmetry_ratio.toFixed(2)}×`} />
            <RowKv label="Quality" value={`${(block.header.solution_quality * 100).toFixed(2)}%`} />
            <RowKv label="Energy" value={`${block.header.energy_estimate_joules.toFixed(2)} J`} />
          </div>
        </div>
        <DetailRow label="Prev hash" value={<code className="break-all text-[10px]">{block.header.prev_hash}</code>} />
        <DetailRow label="Miner" value={<code className="break-all text-[10px]">{block.header.miner}</code>} />
        <div>
          <div className="mb-1 text-xs text-muted-foreground">Transactions ({block.transactions.length})</div>
          {txRows(block).length === 0 ? (
            <p className="text-xs text-muted-foreground">No transactions</p>
          ) : (
            <ul className="max-h-40 space-y-1 overflow-y-auto rounded border bg-muted/20 p-2 font-mono text-[10px]">
              {txRows(block).map((r) => (
                <li key={r.key} className="break-all">
                  {r.line}
                </li>
              ))}
            </ul>
          )}
        </div>
      </CardContent>
    </Card>
  );
}

function DetailRow({ icon, label, value }: { icon?: React.ReactNode; label: string; value: React.ReactNode }) {
  return (
    <div className="flex flex-wrap items-start justify-between gap-2 border-b border-border/60 py-2 last:border-0">
      <div className="flex items-center gap-2 text-muted-foreground">
        {icon}
        <span>{label}</span>
      </div>
      <div className="max-w-[min(100%,28rem)] text-right font-medium">{value}</div>
    </div>
  );
}

function RowKv({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex justify-between gap-4">
      <span className="text-muted-foreground">{label}</span>
      <span className="font-mono tabular-nums">{value}</span>
    </div>
  );
}

function TimeseriesSection({ consensus }: { consensus: ConsensusState | undefined }) {
  const historicalData = useMemo(() => {
    if (!consensus) return [];
    const data: {
      tau: number;
      magnitude: number;
      phase: number;
      d1: number;
      d2: number;
      d3: number;
      totalSupply: number;
    }[] = [];
    const currentTau = consensus.tau;
    const points = 50;
    for (let i = 0; i < points; i++) {
      const tau = (currentTau / Math.max(points - 1, 1)) * i;
      const magnitude = Math.exp(-ETA * tau);
      const phase = LAMBDA * tau;
      const d1 = 1_000_000 * (1 + tau * 0.1) * Math.exp(-ETA * 0.0);
      const d2 = 1_000_000 * (1 + tau * 0.1) * Math.exp(-ETA * 0.2);
      const d3 = 1_000_000 * (1 + tau * 0.1) * Math.exp(-ETA * 0.41);
      data.push({
        tau: Number(tau.toFixed(4)),
        magnitude: Number(magnitude.toFixed(4)),
        phase: Number(phase.toFixed(4)),
        d1: Math.round(d1),
        d2: Math.round(d2),
        d3: Math.round(d3),
        totalSupply: Math.round(d1 + d2 + d3),
      });
    }
    return data;
  }, [consensus]);

  if (historicalData.length === 0) {
    return (
      <Card>
        <CardHeader>
          <CardTitle className="text-base">Timeseries</CardTitle>
          <CardDescription>Loads when consensus τ is available from Prometheus.</CardDescription>
        </CardHeader>
      </Card>
    );
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">Timeseries</CardTitle>
        <CardDescription>Modelled pool evolution vs τ (same construction as web-wallet).</CardDescription>
      </CardHeader>
      <CardContent className="h-[360px] w-full">
        <ResponsiveContainer width="100%" height="100%">
          <LineChart data={historicalData}>
            <CartesianGrid strokeDasharray="3 3" className="stroke-muted" />
            <XAxis dataKey="tau" tick={{ fontSize: 11 }} />
            <YAxis tick={{ fontSize: 11 }} />
            <Tooltip contentStyle={{ fontSize: 12 }} />
            <Legend />
            <Line type="monotone" dataKey="d1" name="D1" stroke="hsl(var(--chart-1))" strokeWidth={2} dot={false} />
            <Line type="monotone" dataKey="d2" name="D2" stroke="hsl(var(--chart-2))" strokeWidth={2} dot={false} />
            <Line type="monotone" dataKey="d3" name="D3" stroke="hsl(var(--chart-3))" strokeWidth={2} dot={false} />
          </LineChart>
        </ResponsiveContainer>
      </CardContent>
    </Card>
  );
}

function StakingPoolsSection({ allPools }: { allPools: AllPoolsData | undefined }) {
  if (!allPools) {
    return (
      <Card>
        <CardHeader>
          <CardTitle className="text-base">Staking pools (D1–D8)</CardTitle>
        </CardHeader>
        <CardContent className="text-sm text-muted-foreground">Waiting for pool metrics…</CardContent>
      </Card>
    );
  }

  const maxTotal = Math.max(...POOL_INFO.map((p) => allPools[p.name]?.total ?? 0), 1);

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">Staking pools</CardTitle>
        <CardDescription>Liquidity and unlock fraction per dimensional pool.</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        {POOL_INFO.map((pool) => {
          const d = allPools[pool.name];
          const pct = maxTotal > 0 ? (d.total / maxTotal) * 100 : 0;
          return (
            <div key={pool.name} className="space-y-1">
              <div className="flex justify-between text-xs">
                <span className="font-medium">
                  {pool.name} · {pool.label}
                </span>
                <span className="text-muted-foreground">
                  {(d.unlockFraction * 100).toFixed(1)}% unlocked · yield {d.yieldRate.toFixed(4)}
                </span>
              </div>
              <div className="h-2 overflow-hidden rounded-full bg-muted">
                <div className="h-full rounded-full transition-all" style={{ width: `${pct}%`, background: pool.color }} />
              </div>
              <div className="flex justify-between font-mono text-[11px] text-muted-foreground">
                <span>locked {d.locked.toLocaleString()}</span>
                <span>total {d.total.toLocaleString()}</span>
              </div>
            </div>
          );
        })}
      </CardContent>
      <CardContent className="h-[300px] border-t pt-4">
        <LockedUnlockedChart allPools={allPools} />
      </CardContent>
    </Card>
  );
}

function LockedUnlockedChart({ allPools }: { allPools: AllPoolsData }) {
  const data = POOL_INFO.map((pool) => ({
    name: pool.name,
    locked: allPools[pool.name]?.locked ?? 0,
    unlocked: allPools[pool.name]?.unlocked ?? 0,
  }));

  return (
    <ResponsiveContainer width="100%" height="100%">
      <BarChart data={data}>
        <CartesianGrid strokeDasharray="3 3" className="stroke-muted" />
        <XAxis dataKey="name" tick={{ fontSize: 11 }} />
        <YAxis tick={{ fontSize: 11 }} />
        <Tooltip formatter={(v: number) => v.toLocaleString()} />
        <Legend />
        <Bar dataKey="locked" stackId="a" name="Locked" fill="hsl(32 95% 44%)" />
        <Bar dataKey="unlocked" stackId="a" name="Unlocked" fill="hsl(142 76% 36%)" />
      </BarChart>
    </ResponsiveContainer>
  );
}

function WavefunctionPanel({ consensus }: { consensus: ConsensusState | undefined }) {
  const wavefunctionData = useMemo(() => {
    if (!consensus) return [];
    const points = 80;
    const out: { real: number; imag: number; tau: number }[] = [];
    for (let i = 0; i <= points; i++) {
      const tau = (consensus.tau / points) * i;
      const magnitude = Math.exp(-ETA * tau);
      const phase = LAMBDA * tau;
      out.push({
        tau,
        real: magnitude * Math.cos(phase),
        imag: magnitude * Math.sin(phase),
      });
    }
    return out;
  }, [consensus]);

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-base">
          <Waves className="h-4 w-4" />
          Wavefunction
        </CardTitle>
        <CardDescription>ψ(τ) in the complex plane (spiral from magnitude decay + phase).</CardDescription>
      </CardHeader>
      <CardContent className="h-[420px] w-full">
        {wavefunctionData.length === 0 ? (
          <p className="text-sm text-muted-foreground">Need consensus τ from metrics.</p>
        ) : (
          <ResponsiveContainer width="100%" height="100%">
            <ScatterChart margin={{ top: 12, right: 12, bottom: 12, left: 12 }}>
              <CartesianGrid strokeDasharray="3 3" className="stroke-muted" />
              <XAxis type="number" dataKey="real" name="Re" domain={[-1, 1]} tick={{ fontSize: 10 }} />
              <YAxis type="number" dataKey="imag" name="Im" domain={[-1, 1]} tick={{ fontSize: 10 }} />
              <Tooltip cursor={{ strokeDasharray: "3 3" }} formatter={(v: number) => v.toFixed(4)} />
              <ReferenceLine x={0} stroke="hsl(var(--muted-foreground))" />
              <ReferenceLine y={0} stroke="hsl(var(--muted-foreground))" />
              <Scatter data={wavefunctionData} fill="hsl(var(--primary))" line={{ stroke: "hsl(var(--primary))", strokeWidth: 1 }} />
            </ScatterChart>
          </ResponsiveContainer>
        )}
      </CardContent>
    </Card>
  );
}

function OraclePanel({
  constants,
  convergence,
}: {
  constants: SatoshiConstants | undefined;
  convergence: ConvergenceMetrics | undefined;
}) {
  if (!constants || !convergence) {
    return (
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-base">
            <Triangle className="h-4 w-4" />
            Oracle
          </CardTitle>
        </CardHeader>
        <CardContent className="text-sm text-muted-foreground">Loading oracle inputs…</CardContent>
      </Card>
    );
  }

  const measured_eta = convergence.measured_eta;
  const measured_lambda = convergence.measured_lambda;
  const theoretical_eta = convergence.theoretical_eta;
  const theoretical_lambda = convergence.theoretical_lambda;
  const SQRT_3 = Math.sqrt(3);
  const ALTITUDE = SQRT_3 / 2;
  const d1 = Math.abs(measured_lambda);
  const d2 = Math.abs(SQRT_3 * measured_eta - measured_lambda) / 2;
  const d3 = Math.abs(SQRT_3 * measured_eta + measured_lambda - SQRT_3) / 2;
  const sum = d1 + d2 + d3;
  const measured_delta = sum / ALTITUDE - 1.0;
  const d1_theo = Math.abs(theoretical_lambda);
  const d2_theo = Math.abs(SQRT_3 * theoretical_eta - theoretical_lambda) / 2;
  const d3_theo = Math.abs(SQRT_3 * theoretical_eta + theoretical_lambda - SQRT_3) / 2;
  const sum_theo = d1_theo + d2_theo + d3_theo;
  const theoretical_delta = sum_theo / ALTITUDE - 1.0;
  const scale = 400;
  const v1 = { x: 0, y: 0 };
  const v2 = { x: 1, y: 0 };
  const v3 = { x: 0.5, y: Math.sqrt(3) / 2 };
  const points = [
    { x: v1.x * scale, y: v1.y * scale },
    { x: v2.x * scale, y: v2.y * scale },
    { x: v3.x * scale, y: v3.y * scale },
  ];
  const pointX = measured_eta * scale;
  const pointY = measured_lambda * scale;
  const theoX = theoretical_eta * scale;
  const theoY = theoretical_lambda * scale;
  const isConverging = convergence.convergence_confidence > 0.8;
  const eta_converged = convergence.eta_error < 0.01;
  const lambda_converged = convergence.lambda_error < 0.01;
  const fully_converged = eta_converged && lambda_converged && isConverging;

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-base">
          {fully_converged ? <CheckCircle className="h-4 w-4 text-emerald-500" /> : <Target className="h-4 w-4" />}
          Viviani oracle
        </CardTitle>
        <CardDescription>η = λ = 1/√2 conjecture panel (ported from web-wallet).</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-4 lg:grid-cols-2">
        <div className="rounded-lg border bg-background p-2">
          <svg width="100%" height="360" viewBox="-50 -350 500 450" className="text-foreground">
            <polygon
              points={`${points[0].x},${-points[0].y} ${points[1].x},${-points[1].y} ${points[2].x},${-points[2].y}`}
              fill="none"
              stroke="hsl(var(--primary))"
              strokeWidth="2"
            />
            {points.map((p, i) => (
              <circle key={i} cx={p.x} cy={-p.y} r="4" fill="hsl(var(--primary))" />
            ))}
            <circle cx={theoX} cy={-theoY} r="8" fill="none" stroke="hsl(142 76% 36%)" strokeWidth="2" strokeDasharray="4,4" />
            <circle cx={theoX} cy={-theoY} r="2" fill="hsl(142 76% 36%)" />
            <line x1={pointX} y1={-pointY} x2={theoX} y2={-theoY} stroke="hsl(32 95% 44%)" strokeWidth="1" strokeDasharray="3,3" />
            <circle cx={pointX} cy={-pointY} r="6" fill="hsl(var(--destructive))" />
          </svg>
        </div>
        <div className="space-y-3 text-sm">
          <div className="rounded-lg border bg-muted/30 p-3 font-mono text-xs">
            <div>η measured {measured_eta.toFixed(6)} · target {theoretical_eta.toFixed(6)}</div>
            <div>λ measured {measured_lambda.toFixed(6)} · target {theoretical_lambda.toFixed(6)}</div>
            <div>R² {(convergence.convergence_confidence * 100).toFixed(1)}%</div>
            <div>Oracle Δ {convergence.measured_oracle_delta.toFixed(6)}</div>
          </div>
          <div className="rounded-lg border p-3">
            <div className="text-xs font-semibold text-muted-foreground">Viviani distances (measured)</div>
            <div className="mt-1 space-y-1 font-mono text-xs">
              <div className="flex justify-between">
                <span>d1</span>
                <span>{d1.toFixed(6)}</span>
              </div>
              <div className="flex justify-between">
                <span>d2</span>
                <span>{d2.toFixed(6)}</span>
              </div>
              <div className="flex justify-between">
                <span>d3</span>
                <span>{d3.toFixed(6)}</span>
              </div>
              <div className="flex justify-between font-medium">
                <span>Δ raw</span>
                <span>{measured_delta.toFixed(6)}</span>
              </div>
              <div className="flex justify-between text-muted-foreground">
                <span>vs theoretical Δ</span>
                <span>{theoretical_delta.toFixed(6)}</span>
              </div>
            </div>
          </div>
          <div className="text-xs text-muted-foreground">
            Unit circle check η²+λ²: <span className="font-mono">{constants.unit_circle_constraint.toFixed(6)}</span>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}

function CalculatorPanel({ consensus }: { consensus: ConsensusState | undefined }) {
  const [targetPool, setTargetPool] = useState("D7");
  const [targetUnlock, setTargetUnlock] = useState("90");

  const result = useMemo(() => {
    if (!consensus) return null;
    const poolInfo = POOL_INFO.find((p) => p.name === targetPool);
    if (!poolInfo) return null;
    const targetFraction = parseFloat(targetUnlock) / 100;
    if (Number.isNaN(targetFraction)) return null;
    const currentTau = consensus.tau;
    const requiredTau = poolInfo.tau - Math.log(1 - targetFraction) / ETA;
    const deltaTau = requiredTau - currentTau;
    const deltaBlocks = Math.round(deltaTau * TAU_C);
    const currentUnlockFraction = 1 - Math.exp(-ETA * Math.max(0, currentTau - poolInfo.tau));
    return {
      requiredTau,
      deltaTau,
      deltaBlocks,
      currentUnlockFraction,
      alreadyUnlocked: currentUnlockFraction >= targetFraction,
    };
  }, [consensus, targetPool, targetUnlock]);

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-base">
          <Calculator className="h-4 w-4" />
          Unlock calculator
        </CardTitle>
        <CardDescription>U_n(τ) = 1 − e^(−η(τ − τ_n)) — same model as web-wallet.</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-4 sm:grid-cols-2">
        <div className="space-y-3">
          <div>
            <label className="text-xs font-medium text-muted-foreground">Pool</label>
            <select
              className="mt-1 w-full rounded-md border bg-background px-2 py-2 text-sm"
              value={targetPool}
              onChange={(e) => setTargetPool(e.target.value)}
            >
              {POOL_INFO.map((p) => (
                <option key={p.name} value={p.name}>
                  {p.name} — {p.label}
                </option>
              ))}
            </select>
          </div>
          <div>
            <label className="text-xs font-medium text-muted-foreground">Target unlock %</label>
            <Input type="number" min={0} max={100} value={targetUnlock} onChange={(e) => setTargetUnlock(e.target.value)} className="mt-1" />
          </div>
        </div>
        <div className="rounded-lg border bg-muted/30 p-3 text-sm">
          {!result ? (
            <p className="text-muted-foreground">Need live τ from Prometheus metrics.</p>
          ) : result.alreadyUnlocked ? (
            <p className="text-emerald-600 dark:text-emerald-400">Already at or above target unlock.</p>
          ) : (
            <ul className="space-y-2 font-mono text-xs">
              <li>Current τ {consensus!.tau.toFixed(4)}</li>
              <li>Current unlock {(result.currentUnlockFraction * 100).toFixed(2)}%</li>
              <li>Required τ {result.requiredTau.toFixed(4)}</li>
              <li>Δτ {result.deltaTau.toFixed(4)}</li>
              <li className="font-semibold">≈ {result.deltaBlocks.toLocaleString()} blocks (τ_c = √2)</li>
            </ul>
          )}
        </div>
      </CardContent>
    </Card>
  );
}
