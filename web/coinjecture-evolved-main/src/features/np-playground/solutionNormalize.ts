import type { ProblemType, SolutionType } from "@/lib/rpc-client";

/** Best-effort cast from user solver return value to RPC solution shape */
export function normalizeSolution(problem: ProblemType, raw: unknown): SolutionType | null {
  if (raw == null || typeof raw !== "object") return null;
  const o = raw as Record<string, unknown>;

  if (problem.SubsetSum) {
    const idx = o.SubsetSum;
    if (Array.isArray(idx) && idx.every((x) => typeof x === "number" && Number.isInteger(x))) {
      return { SubsetSum: idx as number[] };
    }
  }
  if (problem.SAT) {
    const sat = o.SAT;
    if (Array.isArray(sat) && sat.every((x) => typeof x === "boolean")) {
      return { SAT: sat as boolean[] };
    }
  }
  if (problem.TSP) {
    const tsp = o.TSP;
    if (Array.isArray(tsp) && tsp.every((x) => typeof x === "number" && Number.isInteger(x))) {
      return { TSP: tsp as number[] };
    }
  }
  return null;
}
