import type { NetworkProblemKind } from "./networkRegistry";

/** `ProblemType` JSON as accepted by RPC / `mining.generateProblem` / `core::problem::ProblemType` */
export const DEFAULT_PROBLEM_JSON: Record<NetworkProblemKind, string> = {
  SubsetSum: JSON.stringify(
    {
      SubsetSum: {
        numbers: [3, 34, 4, 12, 5, 2],
        target: 15,
      },
    },
    null,
    2
  ),
  SAT: JSON.stringify(
    {
      SAT: {
        variables: 4,
        clauses: [
          { literals: [1, -2, 3] },
          { literals: [-1, 2, -3] },
          { literals: [2, 3, 4] },
          { literals: [-2, -3, 4] },
        ],
      },
    },
    null,
    2
  ),
  TSP: JSON.stringify(
    {
      TSP: {
        cities: 5,
        distances: [
          [0, 10, 15, 20, 25],
          [10, 0, 35, 25, 30],
          [15, 35, 0, 30, 20],
          [20, 25, 30, 0, 15],
          [25, 30, 20, 15, 0],
        ],
      },
    },
    null,
    2
  ),
};
