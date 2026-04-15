import { Card } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { useState, useEffect, useRef } from "react";
import { Activity, Zap, Target, Award, Loader2, AlertCircle } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import { rpcClient } from "@/lib/rpc-client";
import { bytesToHex } from "@noble/hashes/utils";
import {
  blockRewardFromTruncWorkAndParentW,
  formatBeans,
  formatWorkScoreBits,
  parseBalance,
  parseU128DecimalString,
  truncatedHeaderWorkScoreU128,
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

/**
 * Parent cumulative work for height `h` is `W_tip − w_h − Σ w_i (h<i≤tip)` with `w` = truncated stored
 * `header.work_score` (same as node DB), not PoUW display bits.
 */
function applyTokenomicsRewardsToSolutions(
  solutions: Solution[],
  workByHeight: Map<number, bigint>,
  tipHeight: number,
  wTip: bigint,
): void {
  for (const s of solutions) {
    if (s.block_height <= 0) continue;
    let tail = 0n;
    let complete = true;
    for (let hi = s.block_height + 1; hi <= tipHeight; hi++) {
      const w = workByHeight.get(hi);
      if (w === undefined) {
        complete = false;
        break;
      }
      tail += w;
    }
    const wSelf = workByHeight.get(s.block_height);
    if (wSelf === undefined) complete = false;
    if (!complete) continue;
    const parentW = wTip - wSelf - tail;
    if (parentW < 0n) continue;
    s.parent_w_emission = parentW;
    // Integer ⌊w/W⌋ is often 0 once W ≫ w_trunc — do not replace on-chain coinbase in `reward_beans`.
    s.formula_reward_beans = blockRewardFromTruncWorkAndParentW(wSelf, parentW);
  }
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
  /** Parsed on-chain coinbase `reward` (authoritative mint for this block). */
  reward_beans: bigint;
  /** Model mint atoms `⌊w_trunc·S/W_parent⌋` when W_parent is derivable (S=10¹²); informational vs coinbase. */
  formula_reward_beans: bigint | null;
  /**
   * Integer truncated from on-chain `header.work_score` — this is what the node sums into cumulative W
   * (`work_score.max(0) as u64`). When this is 0, parent W does not grow and rewards can repeat.
   */
  w_contrib_chain: bigint;
  /** Parent cumulative W (Σ chain rows before this block) when derivable from tip W + loaded heights. */
  parent_w_emission: bigint | null;
  /** Parsed `coinbase.reward` from RPC JSON when present (may match or differ from formula display). */
  coinbase_beans: bigint | null;
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
    refetchInterval: 5000,
  });

  const BLOCK_FETCH_CONCURRENCY = 4;

  const {
    data: feedData,
    isError: feedQueryError,
    error: feedQueryErrorDetail,
    failureCount: feedFailureCount,
  } = useQuery({
    queryKey: ['live-solution-feed', chainInfo?.best_height, chainInfo?.best_hash],
    enabled: !!chainInfo,
    refetchInterval: 8000,
    queryFn: async () => {
      // `/chain/info` (or parallel chain_getInfo) may report a higher tip than the node behind
      // `VITE_API_URL/node-rpc` or a single lagging RPC. chain_getBlock(h) then returns null for
      // every h > that node's tip → "fetched 0". Cap heights to the RPC tip we can actually read.
      const apiBest = chainInfo!.best_height;
      const tipBlock = await rpcClient.getLatestBlock();
      let latestHeight = apiBest;
      if (tipBlock?.header) {
        const th = headerHeight(tipBlock.header);
        if (th !== null) {
          latestHeight = Math.min(apiBest, th);
        }
      }

      const blockCount = 20;
      const startHeight = Math.max(0, latestHeight - blockCount + 1);
      const heights: number[] = [];
      for (let h = latestHeight; h >= startHeight; h--) {
        heights.push(h);
      }

      const newSolutions: Solution[] = [];
      const workByHeight = new Map<number, bigint>();
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
        workByHeight.set(
          height,
          truncatedHeaderWorkScoreU128(header.work_score ?? header.workScore),
        );
        const bits = effectiveHeaderWorkScoreBits(header);
        const wRow = truncatedHeaderWorkScoreU128(header.work_score ?? header.workScore);
        const fromCb = blockRaw ? coinbaseRewardBeansFromBlock(blockRaw) : null;
        const rewardBeans = fromCb !== null ? fromCb : 0n;
        newSolutions.push({
          block_height: height,
          problem_type: "Custom",
          solver: formatMinerShort(header.miner),
          reward_beans: rewardBeans,
          formula_reward_beans: null,
          w_contrib_chain: wRow,
          parent_w_emission: null,
          coinbase_beans: fromCb,
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
        const raw = block as unknown as Record<string, unknown>;
        const headerRaw = raw.header as Record<string, unknown> | undefined;
        if (!headerRaw) return;

        const height = headerHeight(headerRaw);
        if (height === null) return;

        workByHeight.set(
          height,
          truncatedHeaderWorkScoreU128(headerRaw.work_score ?? headerRaw.workScore),
        );

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
        const wRow = truncatedHeaderWorkScoreU128(headerRaw.work_score ?? headerRaw.workScore);
        const fromCb = coinbaseRewardBeansFromBlock(raw);
        const rewardBeans = fromCb !== null ? fromCb : 0n;

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
          formula_reward_beans: null,
          w_contrib_chain: wRow,
          parent_w_emission: null,
          coinbase_beans: fromCb,
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

      // If every chain_getBlock(height) returned null, reuse RPC tip from above, then REST fallback.
      if (blocksFetched === 0 && tipBlock) {
        processBlock(tipBlock);
      }
      if (blocksFetched === 0) {
        const viaApi = await rpcClient.getLatestBlockFromApi();
        if (viaApi) processBlock(viaApi);
      }

      const uniqueByHeight = Array.from(
        new Map(newSolutions.map((s) => [s.block_height, s])).values(),
      ).sort((a, b) => b.block_height - a.block_height);

      const wTip = parseU128DecimalString(chainInfo!.best_cumulative_work);
      if (wTip !== null) {
        applyTokenomicsRewardsToSolutions(uniqueByHeight, workByHeight, latestHeight, wTip);
      }

      let feedHint: string | null = null;
      if (uniqueByHeight.length === 0 && blocksFetched > 0 && blocksWithReveal === 0) {
        feedHint =
          "Blocks loaded but none included solution_reveal. Nodes may omit bodies or RPC may strip them.";
      } else if (uniqueByHeight.length === 0 && blocksFetched > 0 && blocksWithReveal > 0) {
        feedHint =
          "Blocks include solution_reveal but problem/solution JSON was not recognized (see browser console).";
      } else if (uniqueByHeight.length === 0 && blocksFetched === 0) {
        feedHint =
          "Could not load block bodies (chain_getBlock / chain_getLatestBlock / API /chain/latest-block). " +
          "Confirm VITE_API_URL, API NODE_RPC_URL, and that /node-rpc and /chain/latest-block reach a synced node.";
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
        {chainInfo && feedQueryError && (
          <div className="text-center py-8 text-destructive text-sm space-y-2 px-4">
            <AlertCircle className="h-8 w-8 mx-auto opacity-80" />
            <p className="font-medium">Could not load the live feed</p>
            <p className="text-muted-foreground break-words max-w-xl mx-auto">
              {feedQueryErrorDetail instanceof Error
                ? feedQueryErrorDetail.message
                : String(feedQueryErrorDetail ?? 'Unknown error')}
            </p>
            {feedFailureCount > 0 ? (
              <p className="text-xs text-muted-foreground">Failed attempts: {feedFailureCount}</p>
            ) : null}
          </div>
        )}
        {chainInfo && !feedQueryError && solutions.length === 0 && (
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

              {/* Metrics — emission uses Σ on-chain header work_score integers, not PoUW bits alone */}
              <div className="grid grid-cols-2 sm:grid-cols-4 gap-2 text-xs">
                <div className="flex flex-col gap-0.5 sm:col-span-1">
                  <span className="text-muted-foreground flex items-center gap-1">
                    <Award className="h-3 w-3 text-primary shrink-0" />
                    Reward (BEANS)
                  </span>
                  <span className="font-semibold text-primary tabular-nums">
                    {formatBeans(solution.reward_beans)}
                  </span>
                  {solution.parent_w_emission != null && solution.formula_reward_beans != null ? (
                    <span
                      className="text-[10px] text-muted-foreground leading-tight"
                      title="Mint = ⌊w_trunc·S / W_parent⌋ ledger atoms (S=10¹²). Display BEANS = mint / S. w_trunc is integer header work, not fractional PoUW bits."
                    >
                      Model mint: {formatBeans(solution.formula_reward_beans)} BEANS
                      <span className="opacity-80">
                        {" "}
                        ({solution.formula_reward_beans.toLocaleString()} atoms, w_trunc=
                        {solution.w_contrib_chain.toString()}, W_parent={solution.parent_w_emission.toString()})
                      </span>
                    </span>
                  ) : solution.block_height > 0 ? (
                    <span className="text-[10px] text-muted-foreground leading-tight">
                      W_parent not derived (tip/window mismatch); reward from block coinbase only.
                    </span>
                  ) : null}
                  {solution.coinbase_beans != null &&
                  solution.formula_reward_beans != null &&
                  solution.coinbase_beans !== solution.formula_reward_beans ? (
                    <span className="text-[10px] text-amber-600/90 dark:text-amber-400/90 leading-tight">
                      Differs from model ⌊w·S÷W⌋ (legacy blocks or RPC mismatch).
                    </span>
                  ) : null}
                </div>
                <div className="flex flex-col gap-0.5">
                  <span className="text-muted-foreground flex items-center gap-1">
                    <Target className="h-3 w-3 text-warning shrink-0" />
                    Work (bits)
                  </span>
                  <span className="font-semibold tabular-nums" title="log₂(solve/verify) × quality from header / PoUW estimate">
                    {formatWorkScoreBits(solution.work_score_bits)}
                  </span>
                  <span
                    className="text-[10px] text-muted-foreground leading-tight"
                    title="Chain ΣW uses (header.work_score as u64); values &lt; 1 truncate to 0 even when PoUW bits (above) are non-zero."
                  >
                    ΣW row: +{solution.w_contrib_chain.toString()}
                    {solution.w_contrib_chain === 0n
                      ? " — no integer work added to W (stored score truncates to 0)"
                      : ""}
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
