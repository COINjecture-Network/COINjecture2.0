/**
 * Metadata aligned with `consensus/src/problem_registry.rs` ProblemDescriptor values
 * (scaling exponents, limits). Used for Solver Lab hints only.
 */
export type NetworkProblemKind = "SubsetSum" | "SAT" | "TSP";

export const NETWORK_REGISTRY: Record<
  NetworkProblemKind,
  {
    label: string;
    scalingExponent: number;
    absoluteMaxSize: number;
  }
> = {
  SubsetSum: {
    label: "Subset Sum",
    scalingExponent: 0.8,
    absoluteMaxSize: 60,
  },
  SAT: {
    label: "SAT",
    scalingExponent: 0.7,
    absoluteMaxSize: 120,
  },
  TSP: {
    label: "TSP",
    scalingExponent: 0.5,
    absoluteMaxSize: 30,
  },
};
