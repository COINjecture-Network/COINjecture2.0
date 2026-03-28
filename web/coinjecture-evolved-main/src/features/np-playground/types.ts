import type { ProblemType, SolutionType } from "@/lib/rpc-client";

export type { ProblemType, SolutionType };

/** UI-facing result after running the same solvers as `mining.ts` / block `solution_reveal` */
export type SolverRunResult = {
  ok: boolean;
  timeMs: number;
  solution: SolutionType | null;
  /** Human-readable lines (solver notes, verification hints) */
  log: string[];
};
