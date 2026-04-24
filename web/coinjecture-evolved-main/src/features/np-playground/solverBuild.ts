import type { ProblemType } from "@/lib/rpc-client";

/**
 * Build a classic worker script that inlines user solver sources.
 * Execution uses a Blob URL + Worker (no `new Function`), so solver code stays
 * in an isolated worker thread without dynamic Function compilation.
 */
export function buildUserSolverWorkerScript(files: Record<string, string>): string {
  const ss = files["solvers/subset-sum.js"] ?? "";
  const sat = files["solvers/sat.js"] ?? "";
  const tsp = files["solvers/tsp.js"] ?? "";

  return `
"use strict";
${ss}
${sat}
${tsp}
if (typeof solveSubsetSum !== "function") throw new Error("Define function solveSubsetSum in solvers/subset-sum.js");
if (typeof solveSAT !== "function") throw new Error("Define function solveSAT in solvers/sat.js");
if (typeof solveTSP !== "function") throw new Error("Define function solveTSP in solvers/tsp.js");
self.onmessage = function (e) {
  var problem = e.data.problem;
  try {
    var t0 = performance.now();
    var solution = null;
    if (problem.SubsetSum) {
      var r0 = solveSubsetSum(problem.SubsetSum.numbers, problem.SubsetSum.target);
      solution = r0 == null ? null : { SubsetSum: r0 };
    } else if (problem.SAT) {
      var r1 = solveSAT(problem.SAT.variables, problem.SAT.clauses);
      solution = r1 == null ? null : { SAT: r1 };
    } else if (problem.TSP) {
      var r2 = solveTSP(problem.TSP.cities, problem.TSP.distances);
      solution = r2 == null ? null : { TSP: r2 };
    }
    self.postMessage({
      ok: true,
      solution: solution,
      timeMs: performance.now() - t0,
    });
  } catch (err) {
    self.postMessage({
      ok: false,
      error: err instanceof Error ? err.message : String(err),
    });
  }
};
`;
}

export type UserSolverWorkerHandle = {
  worker: Worker;
  revokeObjectUrl: () => void;
};

/** Spawn a dedicated worker whose script embeds the given solver files. */
export function createUserSolverWorker(files: Record<string, string>): UserSolverWorkerHandle {
  const script = buildUserSolverWorkerScript(files);
  const blob = new Blob([script], { type: "text/javascript;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const worker = new Worker(url);
  return {
    worker,
    revokeObjectUrl: () => URL.revokeObjectURL(url),
  };
}

/** @internal Message shape posted into the user-solver worker. */
export type UserSolverWorkerIn = { problem: ProblemType };
