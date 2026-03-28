import type { ProblemType } from "@/lib/rpc-client";

function isPlainObject(v: unknown): v is Record<string, unknown> {
  return v !== null && typeof v === "object" && !Array.isArray(v);
}

export function parseNetworkProblem(text: string): { ok: true; value: ProblemType } | { ok: false; error: string } {
  let raw: unknown;
  try {
    raw = JSON.parse(text);
  } catch {
    return { ok: false, error: "Invalid JSON" };
  }

  if (!isPlainObject(raw)) {
    return { ok: false, error: "Problem must be a JSON object" };
  }

  const keys = Object.keys(raw).filter((k) => raw[k as keyof typeof raw] !== undefined);
  const problemKeys = keys.filter((k) => ["SubsetSum", "SAT", "TSP", "Custom"].includes(k));
  if (problemKeys.length !== 1) {
    return {
      ok: false,
      error: "Provide exactly one problem key: SubsetSum, SAT, or TSP (Custom not supported in Solver Lab)",
    };
  }

  const k = problemKeys[0];
  if (k === "Custom") {
    return { ok: false, error: "Custom problems are not editable in Solver Lab" };
  }

  if (k === "SubsetSum") {
    const v = raw.SubsetSum;
    if (!isPlainObject(v)) {
      return { ok: false, error: "SubsetSum must be an object with numbers[] and target" };
    }
    if (!Array.isArray(v.numbers) || !v.numbers.every((n) => typeof n === "number" && Number.isFinite(n))) {
      return { ok: false, error: "SubsetSum.numbers must be a number array" };
    }
    if (typeof v.target !== "number" || !Number.isFinite(v.target)) {
      return { ok: false, error: "SubsetSum.target must be a finite number" };
    }
    return {
      ok: true,
      value: {
        SubsetSum: {
          numbers: v.numbers.map((n) => Math.trunc(n)),
          target: Math.trunc(v.target),
        },
      },
    };
  }

  if (k === "SAT") {
    const v = raw.SAT;
    if (!isPlainObject(v)) {
      return { ok: false, error: "SAT must be an object" };
    }
    const variables = v.variables;
    if (typeof variables !== "number" || !Number.isInteger(variables) || variables < 0) {
      return { ok: false, error: "SAT.variables must be a nonnegative integer" };
    }
    if (!Array.isArray(v.clauses)) {
      return { ok: false, error: "SAT.clauses must be an array" };
    }
    const clauses: { literals: number[] }[] = [];
    for (let i = 0; i < v.clauses.length; i++) {
      const c = v.clauses[i];
      if (isPlainObject(c) && Array.isArray(c.literals)) {
        if (!c.literals.every((lit) => typeof lit === "number" && Number.isInteger(lit) && lit !== 0)) {
          return { ok: false, error: `SAT.clauses[${i}]: literals must be nonzero integers (DIMACS 1..n)` };
        }
        clauses.push({ literals: c.literals });
        continue;
      }
      if (Array.isArray(c) && c.every((lit) => typeof lit === "number" && Number.isInteger(lit) && lit !== 0)) {
        clauses.push({ literals: c as number[] });
        continue;
      }
      return {
        ok: false,
        error: `SAT.clauses[${i}]: use { "literals": [1,-2,3] } or a literal array [1,-2,3]`,
      };
    }
    return { ok: true, value: { SAT: { variables, clauses } } };
  }

  const v = raw.TSP;
  if (!isPlainObject(v)) {
    return { ok: false, error: "TSP must be an object" };
  }
  const cities = v.cities;
  if (typeof cities !== "number" || !Number.isInteger(cities) || cities < 0) {
    return { ok: false, error: "TSP.cities must be a nonnegative integer" };
  }
  if (!Array.isArray(v.distances)) {
    return { ok: false, error: "TSP.distances must be a square matrix (number[][])" };
  }
  if (v.distances.length !== cities) {
    return { ok: false, error: "TSP.distances row count must equal cities" };
  }
  const distances: number[][] = [];
  for (let i = 0; i < cities; i++) {
    const row = v.distances[i];
    if (!Array.isArray(row) || row.length !== cities) {
      return { ok: false, error: `TSP.distances[${i}] must have length ${cities}` };
    }
    if (!row.every((x) => typeof x === "number" && Number.isFinite(x) && x >= 0)) {
      return { ok: false, error: "TSP distances must be nonnegative finite numbers" };
    }
    distances.push(row.map((x) => x));
  }
  return { ok: true, value: { TSP: { cities, distances } } };
}

/** Deep sort object keys for stable JSON comparison (RPC vs editor shapes). */
function sortKeysDeep(v: unknown): unknown {
  if (v === null || typeof v !== "object") return v;
  if (Array.isArray(v)) return v.map(sortKeysDeep);
  const o = v as Record<string, unknown>;
  const out: Record<string, unknown> = {};
  for (const k of Object.keys(o).sort()) {
    out[k] = sortKeysDeep(o[k]);
  }
  return out;
}

function stableProblemJson(p: ProblemType): string {
  return JSON.stringify(sortKeysDeep(p));
}

/** True when `instance.json` matches the next-block mining template from `chain_getMiningWork`. */
export function problemTypesEqual(a: ProblemType, b: ProblemType): boolean {
  return stableProblemJson(a) === stableProblemJson(b);
}
