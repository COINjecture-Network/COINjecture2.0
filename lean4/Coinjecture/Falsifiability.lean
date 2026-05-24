/-!
Falsifiable predictions from the COINjecture dynamical hypothesis.

The network is **not** required to satisfy these at genesis; they are empirical
predictions tested as history accumulates (see `DimensionalPoolState::test_conjecture`
in [`state/src/dimensional_pools.rs`](../state/src/dimensional_pools.rs)).

1. Perturbation recovery follows **sech(n log r)**, not exponential decay.
2. Difficulty oscillation period relates to the **8-cycle** (3:8 gear).
3. Work-score log-ratios are **symmetric** about log r = 0 (C(r) = C(1/r)).
-/

import Coinjecture.Coherence
import Coinjecture.DesignAxioms

namespace Coinjecture

/-- Empirical sample for prediction testing. -/
structure EmpiricalSample where
  solveTimes : List Float
  targetTime : Float
  workLogRatios : List Float
  posTarget : 0 < targetTime

/-- Predicted recovery profile after perturbation at ratio r for n steps. -/
def predictedRecovery (r : Float) (n : Nat) : Float :=
  sech (n.toFloat * Float.log r)

/-- P1: measured recovery matches sech profile within tolerance. -/
def prediction1_sech_recovery (measured expected : Float) (tol : Float) : Bool :=
  decide ((measured - expected).abs ≤ tol)

/-- P2: oscillation period near 8-block cycle (dimensionless; tolerance in blocks). -/
def prediction2_eight_cycle_period (measuredPeriod : Nat) (tol : Nat) : Bool :=
  decide ((measuredPeriod - 8).natAbs ≤ tol)

/-- P3: work-score log-ratios symmetric about zero. -/
def prediction3_log_symmetry (ratios : List Float) : Bool :=
  ratios.all fun x => ratios.contains (-x)

/-- Composite falsification status (all three must hold to support the conjecture). -/
structure ConjecturePredictions where
  sechRecoveryOk : Bool
  eightCycleOk : Bool
  logSymmetryOk : Bool

def conjectureSupported (p : ConjecturePredictions) : Bool :=
  p.sechRecoveryOk && p.eightCycleOk && p.logSymmetryOk

/-- η measurement within 5% of 1/√2 (matches on-chain `test_conjecture` threshold). -/
def etaWithinTolerance (measured : Float) : Bool :=
  decide ((measured - eta).abs < 0.05)

end Coinjecture
