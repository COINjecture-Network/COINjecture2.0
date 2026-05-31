import type { ProblemType } from "@/lib/rpc-client";
import { parseNetworkProblem } from "@/features/np-playground/parseNetworkProblem";
import type { BountyProblemKind } from "./bounty-problem-kinds";

export type BountyInstanceParseResult =
  | { ok: true; problem: ProblemType; summary: string }
  | { ok: false; error: string };

/** Every ``` / ```json fenced block in markdown (strict JSON inside). */
export function extractFencedJsonBlocks(text: string): string[] {
  const out: string[] = [];
  const re = /```(?:json)?\s*([\s\S]*?)```/gi;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text)) !== null) {
    const t = m[1].trim();
    if (t) out.push(t);
  }
  return out;
}

/** Split legacy “description” that mixed markdown + fenced instance JSON. */
export function splitLegacyDescription(description: string): {
  briefing: string;
  instanceJson: string;
} {
  const blocks = extractFencedJsonBlocks(description);
  let instanceJson = "";
  for (const block of blocks) {
    try {
      JSON.parse(block);
      instanceJson = JSON.stringify(JSON.parse(block), null, 2);
      break;
    } catch {
      /* try next block */
    }
  }
  const briefing = description.replace(/```(?:json)?[\s\S]*?```/gi, "").trim();
  return { briefing, instanceJson };
}

function flatRecordToProblemType(
  record: Record<string, unknown>,
  kind: BountyProblemKind,
): ProblemType {
  if (kind === "SubsetSum") {
    if (!Array.isArray(record.numbers) || typeof record.target !== "number") {
      throw new Error("Subset Sum requires `numbers` (array) and `target` (integer).");
    }
    const numbers = record.numbers.map((value) => {
      if (typeof value !== "number" || !Number.isFinite(value)) {
        throw new Error("Each number must be a finite integer.");
      }
      return Math.trunc(value);
    });
    if (numbers.length === 0) {
      throw new Error("Provide at least one number.");
    }
    return { SubsetSum: { numbers, target: Math.trunc(record.target) } };
  }

  if (kind === "SAT") {
    if (typeof record.variables !== "number" || !Array.isArray(record.clauses)) {
      throw new Error("SAT requires `variables` (integer) and `clauses` (array).");
    }
    const clauses = record.clauses.map((clause) => {
      const literals = Array.isArray(clause)
        ? clause
        : typeof clause === "object" &&
            clause !== null &&
            Array.isArray((clause as { literals?: unknown }).literals)
          ? (clause as { literals: unknown[] }).literals
          : null;
      if (!literals) {
        throw new Error("Each clause must be `[1,-2,3]` or `{ \"literals\": [...] }`.");
      }
      return {
        literals: literals.map((literal) => {
          if (typeof literal !== "number" || !Number.isFinite(literal)) {
            throw new Error("SAT literals must be integers.");
          }
          return Math.trunc(literal);
        }),
      };
    });
    return { SAT: { variables: Math.trunc(record.variables), clauses } };
  }

  if (kind === "TSP") {
    if (typeof record.cities !== "number" || !Array.isArray(record.distances)) {
      throw new Error("TSP requires `cities` (integer) and `distances` (matrix).");
    }
    const distances = record.distances.map((row) => {
      if (!Array.isArray(row)) {
        throw new Error("Each distance row must be an array of integers.");
      }
      return row.map((value) => {
        if (typeof value !== "number" || !Number.isFinite(value)) {
          throw new Error("Distances must be finite integers ≥ 0.");
        }
        return Math.max(0, Math.trunc(value));
      });
    });
    return { TSP: { cities: Math.trunc(record.cities), distances } };
  }

  throw new Error(`Unsupported problem kind: ${kind}`);
}

function summarizeProblem(problem: ProblemType, kind: BountyProblemKind): string {
  if (kind === "SubsetSum" && problem.SubsetSum) {
    return `${problem.SubsetSum.numbers.length} numbers · target ${problem.SubsetSum.target}`;
  }
  if (kind === "SAT" && problem.SAT) {
    return `${problem.SAT.variables} variables · ${problem.SAT.clauses.length} clauses`;
  }
  if (kind === "TSP" && problem.TSP) {
    return `${problem.TSP.cities} cities · ${problem.TSP.cities}×${problem.TSP.cities} matrix`;
  }
  return "Valid instance";
}

/**
 * Parse the instance JSON editor (flat `{ cities, distances }` or wrapped `{ TSP: {…} }`).
 */
export function parseBountyInstanceJson(
  raw: string,
  kind: BountyProblemKind,
): BountyInstanceParseResult {
  const trimmed = raw.trim();
  if (!trimmed) {
    return { ok: false, error: "Paste or load the on-chain instance JSON." };
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(trimmed);
  } catch {
    return { ok: false, error: "Invalid JSON — use strict JSON (no comments or trailing commas)." };
  }

  if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) {
    return { ok: false, error: "Instance must be a JSON object." };
  }

  const record = parsed as Record<string, unknown>;
  const wrapped = parseNetworkProblem(trimmed);
  if (wrapped.ok) {
    const matchesKind =
      (kind === "SubsetSum" && wrapped.value.SubsetSum != null) ||
      (kind === "SAT" && wrapped.value.SAT != null) ||
      (kind === "TSP" && wrapped.value.TSP != null);
    if (!matchesKind) {
      return {
        ok: false,
        error: `Wrapped JSON does not match selected type ${kind}. Use one variant key only.`,
      };
    }
    return { ok: true, problem: wrapped.value, summary: summarizeProblem(wrapped.value, kind) };
  }

  try {
    const problem = flatRecordToProblemType(record, kind);
    return { ok: true, problem, summary: summarizeProblem(problem, kind) };
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : String(e) };
  }
}

/** Resolve instance from dedicated field, or fall back to legacy description markdown. */
export function resolveBountyProblem(
  instanceJson: string,
  legacyDescription: string,
  kind: BountyProblemKind,
): BountyInstanceParseResult {
  if (instanceJson.trim()) {
    return parseBountyInstanceJson(instanceJson, kind);
  }
  const { instanceJson: extracted } = splitLegacyDescription(legacyDescription);
  if (extracted.trim()) {
    return parseBountyInstanceJson(extracted, kind);
  }
  return {
    ok: false,
    error: "Add instance JSON in Step 2, or load an example template.",
  };
}

export function problemTypeToEditorJson(problem: ProblemType): string {
  if (problem.SubsetSum) {
    return JSON.stringify(
      { numbers: problem.SubsetSum.numbers, target: problem.SubsetSum.target },
      null,
      2,
    );
  }
  if (problem.SAT) {
    return JSON.stringify(
      {
        variables: problem.SAT.variables,
        clauses: problem.SAT.clauses.map((c) => c.literals),
      },
      null,
      2,
    );
  }
  if (problem.TSP) {
    return JSON.stringify(
      { cities: problem.TSP.cities, distances: problem.TSP.distances },
      null,
      2,
    );
  }
  return "{}";
}
