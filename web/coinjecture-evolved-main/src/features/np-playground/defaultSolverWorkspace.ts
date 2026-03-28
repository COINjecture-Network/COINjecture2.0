/**
 * Starter workspace: same algorithms as `mining.ts`, split into owned files (Remix-style).
 * Users edit JavaScript — top-level must be `function solveSubsetSum` / `solveSAT` / `solveTSP`
 * (function declarations so they compose when run together).
 */

export const WORKSPACE_FILES = [
  "solvers/subset-sum.js",
  "solvers/sat.js",
  "solvers/tsp.js",
  "instance.json",
] as const;

export type WorkspaceFilePath = (typeof WORKSPACE_FILES)[number];

export const DEFAULT_WORKSPACE_FILES: Record<WorkspaceFilePath, string> = {
  "solvers/subset-sum.js": `/**
 * Subset Sum — nonnegative integers. Return indices or null.
 * Verified on-chain like core::problem::Solution::SubsetSum.
 */
function solveSubsetSum(numbers, target) {
  const n = numbers.length;
  if (n === 0) return target === 0 ? [] : null;
  if (!numbers.every((x) => Number.isInteger(x) && x >= 0)) return null;
  const sum = numbers.reduce((a, b) => a + b, 0);
  if (target < 0 || target > sum || !Number.isInteger(target)) return null;

  const dp = Array.from({ length: n + 1 }, () => new Array(sum + 1).fill(false));
  dp[0][0] = true;
  for (let i = 1; i <= n; i++) {
    const w = numbers[i - 1];
    for (let s = 0; s <= sum; s++) {
      dp[i][s] = dp[i - 1][s];
      if (s >= w) dp[i][s] = dp[i][s] || dp[i - 1][s - w];
    }
  }
  if (!dp[n][target]) return null;

  const indices = [];
  let s = target;
  for (let i = n; i >= 1; i--) {
    const w = numbers[i - 1];
    if (dp[i - 1][s]) continue;
    if (s >= w && dp[i - 1][s - w]) {
      indices.push(i - 1);
      s -= w;
    }
  }
  indices.reverse();
  return indices;
}
`,

  "solvers/sat.js": `/**
 * SAT — DIMACS literals (±1..n). Return assignment (boolean[]) or null.
 * Clauses: { literals: number[] } (same as mining / rpc).
 */
function solveSAT(variables, clauses) {
  const maxAttempts = Math.min(1 << Math.min(variables, 20), 1000000);
  for (let attempt = 0; attempt < maxAttempts; attempt++) {
    const assignment = [];
    for (let i = 0; i < variables; i++) {
      assignment.push(((attempt >> i) & 1) === 1);
    }
    const ok = clauses.every((clause) => {
      const lits = clause.literals != null ? clause.literals : clause;
      return lits.some((literal) => {
        const varIdx = Math.abs(literal) - 1;
        if (varIdx >= assignment.length) return false;
        const value = assignment[varIdx];
        return literal > 0 ? value : !value;
      });
    });
    if (ok) return assignment;
  }
  return null;
}
`,

  "solvers/tsp.js": `/**
 * TSP — distance matrix. Nearest-neighbor tour (same strategy as mining.ts).
 * Return city order (permutation) or null.
 */
function solveTSP(cities, distances) {
  if (cities === 0) return null;
  const tour = [];
  const visited = new Array(cities).fill(false);
  let current = 0;
  tour.push(current);
  visited[current] = true;
  for (let i = 1; i < cities; i++) {
    let nearest = null;
    let minDist = Infinity;
    for (let next = 0; next < cities; next++) {
      if (!visited[next]) {
        const dist = distances[current][next];
        if (dist < minDist) {
          minDist = dist;
          nearest = next;
        }
      }
    }
    if (nearest === null) break;
    current = nearest;
    tour.push(current);
    visited[current] = true;
  }
  return tour.length === cities ? tour : null;
}
`,

  "instance.json": `{
  "SubsetSum": {
    "numbers": [3, 34, 4, 12, 5, 2],
    "target": 15
  }
}
`,
};

const STORAGE_KEY = "coinjecture:solver-lab-workspace-v3";

export function loadWorkspaceFromStorage(): Record<WorkspaceFilePath, string> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return { ...DEFAULT_WORKSPACE_FILES };
    const parsed = JSON.parse(raw) as Record<string, string>;
    const next = { ...DEFAULT_WORKSPACE_FILES };
    for (const k of WORKSPACE_FILES) {
      if (typeof parsed[k] === "string") next[k] = parsed[k];
    }
    return next;
  } catch {
    return { ...DEFAULT_WORKSPACE_FILES };
  }
}

export function saveWorkspaceToStorage(files: Record<WorkspaceFilePath, string>) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(files));
  } catch {
    /* quota */
  }
}

export function resetWorkspaceDefaults(): Record<WorkspaceFilePath, string> {
  return { ...DEFAULT_WORKSPACE_FILES };
}
