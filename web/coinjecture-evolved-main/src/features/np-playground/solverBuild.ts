import type { ProblemType } from "@/lib/rpc-client";

/**
 * Concatenate user-owned solver files and evaluate in an isolated function scope.
 */
export function buildRunner(files: Record<string, string>) {
  const ss = files["solvers/subset-sum.js"] ?? "";
  const sat = files["solvers/sat.js"] ?? "";
  const tsp = files["solvers/tsp.js"] ?? "";

  const body = `
"use strict";
${ss}
${sat}
${tsp}
if (typeof solveSubsetSum !== "function") throw new Error("Define function solveSubsetSum in solvers/subset-sum.js");
if (typeof solveSAT !== "function") throw new Error("Define function solveSAT in solvers/sat.js");
if (typeof solveTSP !== "function") throw new Error("Define function solveTSP in solvers/tsp.js");
if (problem.SubsetSum) {
  const r = solveSubsetSum(problem.SubsetSum.numbers, problem.SubsetSum.target);
  return r == null ? null : { SubsetSum: r };
}
if (problem.SAT) {
  const r = solveSAT(problem.SAT.variables, problem.SAT.clauses);
  return r == null ? null : { SAT: r };
}
if (problem.TSP) {
  const r = solveTSP(problem.TSP.cities, problem.TSP.distances);
  return r == null ? null : { TSP: r };
}
return null;
`;

  return new Function("problem", body) as (problem: ProblemType) => unknown;
}
