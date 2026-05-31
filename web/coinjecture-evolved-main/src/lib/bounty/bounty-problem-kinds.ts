/** NP problem families supported by the marketplace bounty wizard (extensible list). */
export type BountyProblemKind = "SubsetSum" | "SAT" | "TSP";

export interface BountyProblemKindMeta {
  value: BountyProblemKind;
  label: string;
  note: string;
  /** One-line hint shown under the instance JSON editor. */
  instanceHint: string;
}

export const BOUNTY_PROBLEM_KINDS: BountyProblemKindMeta[] = [
  {
    value: "SubsetSum",
    label: "Subset Sum",
    note: "Exact subset selection — finance, packing, resource matching.",
    instanceHint: "JSON with `numbers` (array) and `target` (integer).",
  },
  {
    value: "TSP",
    label: "TSP",
    note: "Route optimization — logistics, field service, circuit layout.",
    instanceHint: "JSON with `cities` (count) and `distances` (square integer matrix).",
  },
  {
    value: "SAT",
    label: "SAT",
    note: "Boolean satisfiability — scheduling, verification, constraints.",
    instanceHint: "JSON with `variables` (count) and `clauses` (literal arrays).",
  },
];

export function bountyKindMeta(kind: string): BountyProblemKindMeta | undefined {
  return BOUNTY_PROBLEM_KINDS.find((k) => k.value === kind);
}
