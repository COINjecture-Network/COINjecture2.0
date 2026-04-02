import { Card } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { useState, useEffect, useRef } from "react";
import { Activity, Zap, Target, Award, Loader2, AlertCircle } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import { rpcClient } from "@/lib/rpc-client";
import { bytesToHex } from "@noble/hashes/utils";
import {
  blockRewardFromWorkScore,
  formatBeans,
  formatWorkScoreBits,
  parseBalance,
  workScoreBitsFromPouw,
} from "@/lib/chain-metrics";

/** RPC may send `problem` as object or (rarely) JSON string. */
function unwrapJsonObject(value: unknown): Record<string, unknown> | null {
  if (value == null) return null;
  if (typeof value === "string") {
    try {
      const o = JSON.parse(value) as unknown;
      return typeof o === "object" && o !== null && !Array.isArray(o) ? (o as Record<string, unknown>) : null;
    } catch {
      return null;
    }
  }
  if (typeof value === "object" && !Array.isArray(value)) return value as Record<string, unknown>;
  return null;
}

/** Same as unwrap for `solution` (may be JSON string, or raw index array for SubsetSum). */
function unwrapSolutionValue(value: unknown): unknown {
  if (value == null) return value;
  if (typeof value === "string") {
    try {
      return JSON.parse(value) as unknown;
    } catch {
      return value;
    }
  }
  return value;
}

function headerHeight(header: unknown): number | null {
  if (!header || typeof header !== "object") return null;
  const h = header as Record<string, unknown>;
  const raw = h.height ?? h.Height;
  const n = typeof raw === "number" ? raw : typeof raw === "string" ? Number(raw) : NaN;
  return Number.isFinite(n) ? n : null;
}

function headerWorkScore(header: unknown): number {
  if (!header || typeof header !== "object") return NaN;
  const h = header as Record<string, unknown>;
  const raw = h.work_score ?? h.workScore;
  return typeof raw === "number" ? raw : typeof raw === "string" ? Number(raw) : NaN;
}

/**
 * Header `work_score` is authoritative when non-zero. Some nodes/blocks stored 0 while PoUW
 * fields (solve/verify/quality) still reflect real work — recompute bits with the same formula
 * as `consensus/src/work_score.rs` + `chain-metrics.ts`.
 */
function effectiveHeaderWorkScoreBits(header: Record<string, unknown>): number {
  const stored = headerWorkScore(header);
  if (Number.isFinite(stored) && stored > 0) {
    return stored;
  }
  const solveUs =
    typeof header.solve_time_us === "number"
      ? header.solve_time_us
      : Number(header.solve_time_us ?? 0);
  const verifyUs =
    typeof header.verify_time_us === "number"
      ? header.verify_time_us
      : Number(header.verify_time_us ?? 0);
  const quality =
    typeof header.solution_quality === "number"
      ? header.solution_quality
      : Number(header.solution_quality ?? 0);
  const recomputed = workScoreBitsFromPouw(solveUs, verifyUs, quality);
  if (Number.isFinite(recomputed) && recomputed > 0) {
    return recomputed;
  }
  return Number.isFinite(stored) ? stored : 0;
}

function coinbaseRewardBeansFromBlock(raw: Record<string, unknown>): bigint | null {
  const cb = raw.coinbase;
  if (cb == null || typeof cb !== "object" || Array.isArray(cb)) return null;
  return parseBalance((cb as Record<string, unknown>).reward);
}

function numish(v: unknown): number | null {
  if (typeof v === "number" && Number.isFinite(v)) return v;
  if (typeof v === "string" && v.trim() !== "") {
    const n = Number(v);
    return Number.isFinite(n) ? n : null;
  }
  return null;
}

function formatMinerShort(miner: unknown): string {
  if (typeof miner === "string" && miner.length >= 16) {
    return `${miner.slice(0, 8)}...${miner.slice(-6)}`;
  }
  if (Array.isArray(miner) && miner.length >= 8) {
    const hex = bytesToHex(Uint8Array.from(miner as number[]));
    return `${hex.slice(0, 8)}...${hex.slice(-6)}`;
  }
  return "Unknown";
}

interface Solution {
  block_height: number;
  problem_type: "SubsetSum" | "TSP" | "SAT" | "Custom";
  solver: string;
  /** Coinbase reward (BEANS) — from block; fallback matches tokenomics reward formula */
  reward_beans: bigint;
  /** Bit-equivalent work consensus/src/work_score.rs */
  work_score_bits: number;
  /** solve_time / verify_time — PoUW difficulty signal (header field) */
  time_asymmetry_ratio: number;
  solution_quality: number;
  solve_time_us: number;
  verify_time_us: number;
  solve_energy_joules: number;
  timestamp: number;
  problem_data: {
    target?: number;
    values?: number[];
    cities?: number;
    clauses?: number;
    variables?: number;
    custom_problem_id_hex?: string;
    /** Header-only RPC row or parser fallback */
    feed_note?: string;
  };
  solution_data: {
    indices?: number[];
    sum?: number;
    route?: number[];
    satisfying_assignment?: boolean[];
  };
}

export const LiveSolutionFeed = () => {
  const [newSolutionIndex, setNewSolutionIndex] = useState<number | null>(null);
  const prevTopHeightRef = useRef<number | null>(null);

  const { data: chainInfo } = useQuery({
    queryKey: ['chain-info'],
    queryFn: () => rpcClient.getChainInfo(),
    refetchInterval: 10000,
  });

  const BLOCK_FETCH_CONCURRENCY = 4;

  const { data: feedData } = useQuery({
    queryKey: ['live-solution-feed', chainInfo?.best_height, chainInfo?.best_hash],
    enabled: !!chainInfo,
    refetchInterval: 12000,
    queryFn: async () => {
      const latestHeight = chainInfo!.best_height;
      const blockCount = 20;
      const startHeight = Math.max(0, latestHeight - blockCount + 1);
      const heights: number[] = [];
      for (let h = latestHeight; h >= startHeight; h--) {
        heights.push(h);
      }

      const newSolutions: Solution[] = [];
      let blocksFetched = 0;
      let blocksWithReveal = 0;
      let blocksHeaderOnly = 0;
      const statsHeights = new Set<number>();

      const noteStats = (height: number, kind: "reveal" | "header") => {
        if (statsHeights.has(height)) return;
        statsHeights.add(height);
        blocksFetched += 1;
        if (kind === "reveal") blocksWithReveal += 1;
        else blocksHeaderOnly += 1;
      };

      const pushFromHeader = (
        header: Record<string, unknown>,
        height: number,
        note: string,
        blockRaw?: Record<string, unknown>,
      ) => {
        const bits = effectiveHeaderWorkScoreBits(header);
        const fromCb = blockRaw ? coinbaseRewardBeansFromBlock(blockRaw) : null;
        const rewardBeans =
          fromCb !== null && fromCb > 0n ? fromCb : blockRewardFromWorkScore(bits);
        newSolutions.push({
          block_height: height,
          problem_type: "Custom",
          solver: formatMinerShort(header.miner),
          reward_beans: rewardBeans,
          work_score_bits: bits,
          time_asymmetry_ratio: typeof header.time_asymmetry_ratio === "number" ? header.time_asymmetry_ratio : Number(header.time_asymmetry_ratio ?? NaN),
          solution_quality: typeof header.solution_quality === "number" ? header.solution_quality : Number(header.solution_quality ?? NaN),
          solve_time_us: typeof header.solve_time_us === "number" ? header.solve_time_us : Number(header.solve_time_us ?? 0),
          verify_time_us: typeof header.verify_time_us === "number" ? header.verify_time_us : Number(header.verify_time_us ?? 0),
          solve_energy_joules: typeof header.energy_estimate_joules === "number" ? header.energy_estimate_joules : Number(header.energy_estimate_joules ?? 0),
          timestamp: (typeof header.timestamp === "number" ? header.timestamp : Number(header.timestamp ?? 0)) * 1000,
          problem_data: { feed_note: note },
          solution_data: {},
        });
      };

      const processBlock = (block: Awaited<ReturnType<typeof rpcClient.getBlock>>) => {
        if (!block) return;
        try {
        const raw = block as Record<string, unknown>;
        const headerRaw = raw.header as Record<string, unknown> | undefined;
        if (!headerRaw) return;

        const height = headerHeight(headerRaw);
        if (height === null) return;

        const solutionReveal = (raw.solution_reveal ?? raw.solutionReveal) as {
          problem?: unknown;
          solution?: unknown;
        } | null;

        if (!solutionReveal || (solutionReveal.problem == null && solutionReveal.solution == null)) {
          noteStats(height, "header");
          pushFromHeader(
            headerRaw,
            height,
            "Block header received; solution_reveal missing from this RPC response (try another node or refresh).",
            raw,
          );
          return;
        }

        noteStats(height, "reveal");

        const problem = unwrapJsonObject(solutionReveal.problem) ?? (solutionReveal.problem as any);
        let solutionObj = unwrapSolutionValue(solutionReveal.solution) as any;

        /** Rust `Solution::SubsetSum` is `{"SubsetSum":[i,...]}`; some gateways send a bare index array. */
        if (Array.isArray(solutionObj) && problem && typeof problem === "object") {
          const po = problem as Record<string, unknown>;
          if (po.SubsetSum || (Array.isArray(po.numbers) && numish(po.target) != null)) {
            solutionObj = { SubsetSum: solutionObj };
          }
        }

        const bits = effectiveHeaderWorkScoreBits(headerRaw);
        const fromCb = coinbaseRewardBeansFromBlock(raw);
        const rewardBeans =
          fromCb !== null && fromCb > 0n ? fromCb : blockRewardFromWorkScore(bits);

        let problemType: "SubsetSum" | "TSP" | "SAT" | "Custom" | null = null;
        let problemData: Solution['problem_data'] = {};
        let solutionData: Solution['solution_data'] = {};

        const problemObj = problem as any;

        if (problemObj?.SubsetSum) {
          problemType = "SubsetSum";
          const subsetSum = problemObj.SubsetSum;
          const targetN = numish(subsetSum.target);
          problemData = {
            target: targetN ?? undefined,
            values: Array.isArray(subsetSum.numbers) ? subsetSum.numbers.map((x: unknown) => Number(x)) : [],
          };
          if (solutionObj?.SubsetSum) {
            const indices = solutionObj.SubsetSum as number[];
            const sum = indices.reduce((acc: number, idx: number) => acc + (subsetSum.numbers?.[idx] != null ? Number(subsetSum.numbers[idx]) : 0), 0);
            solutionData = { indices, sum };
          }
        } else if (
          problemObj &&
          typeof problemObj === "object" &&
          Array.isArray(problemObj.numbers) &&
          numish(problemObj.target) != null
        ) {
          problemType = "SubsetSum";
          const t = numish(problemObj.target)!;
          const subsetSum = { numbers: problemObj.numbers.map((x: unknown) => Number(x)), target: t };
          problemData = { target: subsetSum.target, values: subsetSum.numbers || [] };
          if (solutionObj?.SubsetSum) {
            const indices = solutionObj.SubsetSum;
            const sum = indices.reduce(
              (acc: number, idx: number) => acc + (subsetSum.numbers?.[idx] != null ? Number(subsetSum.numbers[idx]) : 0),
              0,
            );
            solutionData = { indices, sum };
          }
        } else if (problemObj?.SAT) {
          problemType = "SAT";
          const sat = problemObj.SAT;
          problemData = {
            variables: sat.variables || 0,
            clauses: Array.isArray(sat.clauses) ? sat.clauses.length : 0,
          };
          if (solutionObj?.SAT) {
            const assignment = solutionObj.SAT;
            solutionData = { satisfying_assignment: assignment };
          }
        } else if (problemObj?.TSP) {
          problemType = "TSP";
          const tsp = problemObj.TSP;
          problemData = {
            cities: tsp.cities || 0,
          };
          if (solutionObj?.TSP) {
            const route = solutionObj.TSP;
            solutionData = { route };
          }
        } else if (problemObj?.Custom) {
          problemType = "Custom";
          const c = problemObj.Custom;
          const pid = c?.problem_id;
          let problemIdHex = "";
          if (Array.isArray(pid) && pid.length > 0) {
            try {
              problemIdHex = bytesToHex(new Uint8Array(pid)) as string;
            } catch {
              problemIdHex = "";
            }
          } else if (typeof pid === "string") {
            problemIdHex = pid;
          }
          problemData = {
            custom_problem_id_hex: problemIdHex || undefined,
          };
        }

        if (!problemType) {
          problemType = "Custom";
          problemData = {
            feed_note: "Mined block (problem shape not recognized by feed parser).",
          };
        }

        const solver = formatMinerShort(headerRaw.miner);

        newSolutions.push({
          block_height: height,
          problem_type: problemType,
          solver,
          reward_beans: rewardBeans,
          work_score_bits: bits,
          time_asymmetry_ratio: typeof headerRaw.time_asymmetry_ratio === "number" ? headerRaw.time_asymmetry_ratio : Number(headerRaw.time_asymmetry_ratio ?? NaN),
          solution_quality: typeof headerRaw.solution_quality === "number" ? headerRaw.solution_quality : Number(headerRaw.solution_quality ?? NaN),
          solve_time_us: typeof headerRaw.solve_time_us === "number" ? headerRaw.solve_time_us : Number(headerRaw.solve_time_us ?? 0),
          verify_time_us: typeof headerRaw.verify_time_us === "number" ? headerRaw.verify_time_us : Number(headerRaw.verify_time_us ?? 0),
          solve_energy_joules: typeof headerRaw.energy_estimate_joules === "number" ? headerRaw.energy_estimate_joules : Number(headerRaw.energy_estimate_joules ?? 0),
          timestamp: (typeof headerRaw.timestamp === "number" ? headerRaw.timestamp : Number(headerRaw.timestamp ?? 0)) * 1000,
          problem_data: problemData,
          solution_data: solutionData,
        });
        } catch (e) {
          console.warn("[LiveSolutionFeed] skip block", e);
        }
      };

      for (let i = 0; i < heights.length; i += BLOCK_FETCH_CONCURRENCY) {
        const chunk = heights.slice(i, i + BLOCK_FETCH_CONCURRENCY);
        const blocks = await Promise.all(
          chunk.map((height) =>
            rpcClient.getBlock(height).catch(() => null),
          ),
        );
        for (const block of blocks) {
          processBlock(block);
        }
      }

      // If every chain_getBlock(height) returned null (nodes behind / flaky), try tip once.
      if (blocksFetched === 0) {
        const tip = await rpcClient.getLatestBlock();
        if (tip) processBlock(tip);
      }

      const uniqueByHeight = Array.from(
        new Map(newSolutions.map((s) => [s.block_height, s])).values(),
      ).sort((a, b) => b.block_height - a.block_height);

      let feedHint: string | null = null;
      if (uniqueByHeight.length === 0 && blocksFetched > 0 && blocksWithReveal === 0) {
        feedHint =
          "Blocks loaded but none included solution_reveal. Nodes may omit bodies or RPC may strip them.";
      } else if (uniqueByHeight.length === 0 && blocksFetched > 0 && blocksWithReveal > 0) {
        feedHint =
          "Blocks include solution_reveal but problem/solution JSON was not recognized (see browser console).";
      } else if (uniqueByHeight.length === 0 && blocksFetched === 0) {
        feedHint =
          "Could not load block bodies from any RPC in VITE_RPC_URL (chain_getBlock / chain_getLatestBlock).";
      }

      const feedStats =
        uniqueByHeight.length > 0
          ? null
          : {
              fetched: blocksFetched,
              withReveal: blocksWithReveal,
              parsedRows: uniqueByHeight.length,
              headerOnlyFallback: blocksHeaderOnly,
            };

      return {
        solutions: uniqueByHeight.slice(0, 20),
        feedHint,
        feedStats,
      };
    },
  });

  const solutions = feedData?.solutions ?? [];
  const feedHint = feedData?.feedHint ?? null;
  const feedStats = feedData?.feedStats ?? null;

  useEffect(() => {
    const top = solutions[0]?.block_height;
    if (top == null) return;
    const prev = prevTopHeightRef.current;
    if (prev !== null && top > prev) {
      setNewSolutionIndex(0);
      setTimeout(() => setNewSolutionIndex(null), 2000);
    }
    prevTopHeightRef.current = top;
  }, [solutions]);

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
          <div className="text-center py-12 text-muted-foreground space-y-3">
            <AlertCircle className="h-8 w-8 mx-auto mb-4 opacity-50" />
            <p>No solutions found in recent blocks</p>
            <p className="text-sm mt-2">Blocks will appear here as they are mined</p>
            {feedStats ? (
              <p className="text-xs font-mono max-w-xl mx-auto rounded-md border border-border/60 bg-muted/40 px-3 py-2 text-left text-foreground/90">
                RPC probe: unique heights fetched {feedStats.fetched}, with solution_reveal {feedStats.withReveal},
                header-only fallback {feedStats.headerOnlyFallback ?? 0}, rows shown {feedStats.parsedRows}
              </p>
            ) : null}
            {feedHint ? (
              <p className="text-xs max-w-lg mx-auto rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-foreground">
                {feedHint}
              </p>
            ) : null}
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
                  {solution.problem_type === "Custom" && solution.problem_data.feed_note && (
                    <span className="text-foreground text-sm">{solution.problem_data.feed_note}</span>
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
                  {solution.problem_type === "Custom" && !solution.problem_data.feed_note && (
                    <span className="terminal-font text-success">
                      Reveal committed (custom payload)
                    </span>
                  )}
                  {solution.problem_type === "Custom" && solution.problem_data.feed_note && (
                    <span className="terminal-font text-success/90 text-xs">
                      See problem line — full reveal may still be on-chain.
                    </span>
                  )}
                </div>
                <div className="flex items-center gap-2 text-xs text-muted-foreground">
                  <span>Solver: <span className="text-foreground font-medium">{solution.solver}</span></span>
                </div>
              </div>

              {/* Metrics — align with consensus/src/work_score.rs + tokenomics/src/rewards.rs */}
              <div className="grid grid-cols-2 sm:grid-cols-4 gap-2 text-xs">
                <div className="flex flex-col gap-0.5">
                  <span className="text-muted-foreground flex items-center gap-1">
                    <Award className="h-3 w-3 text-primary shrink-0" />
                    Reward (BEANS)
                  </span>
                  <span className="font-semibold text-primary tabular-nums">
                    {formatBeans(solution.reward_beans)}
                  </span>
                </div>
                <div className="flex flex-col gap-0.5">
                  <span className="text-muted-foreground flex items-center gap-1">
                    <Target className="h-3 w-3 text-warning shrink-0" />
                    Work (bits)
                  </span>
                  <span className="font-semibold tabular-nums" title="log₂(solve/verify) × quality">
                    {formatWorkScoreBits(solution.work_score_bits)}
                  </span>
                </div>
                <div className="flex flex-col gap-0.5">
                  <span className="text-muted-foreground flex items-center gap-1">
                    <Zap className="h-3 w-3 text-success shrink-0" />
                    Asymmetry
                  </span>
                  <span
                    className="font-semibold tabular-nums"
                    title="solve_time / verify_time (network-verifiable PoUW signal)"
                  >
                    {Number.isFinite(solution.time_asymmetry_ratio)
                      ? solution.time_asymmetry_ratio.toFixed(2)
                      : "—"}
                    ×
                  </span>
                </div>
                <div className="flex flex-col gap-0.5">
                  <span className="text-muted-foreground flex items-center gap-1">
                    <Activity className="h-3 w-3 text-secondary shrink-0" />
                    Quality
                  </span>
                  <span className="font-semibold tabular-nums">
                    {Number.isFinite(solution.solution_quality)
                      ? solution.solution_quality.toFixed(3)
                      : "—"}
                  </span>
                </div>
              </div>
              <div className="text-[10px] text-muted-foreground flex flex-wrap gap-x-3 gap-y-1 pt-1 border-t border-border/40">
                <span title="Transparency: solve vs verify wall time">
                  Δt: {(solution.solve_time_us / 1000).toFixed(1)} ms solve /{" "}
                  {(solution.verify_time_us / 1000).toFixed(2)} ms verify
                </span>
                <span title="Estimated energy (header, informational)">
                  Est. energy: {solution.solve_energy_joules.toFixed(1)} J
                </span>
              </div>
            </div>
          </Card>
        ))}
      </div>

    </Card>
  );
};
