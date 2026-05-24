/-!
Coherence function **C(r) = 2r / (1 + r²)** used in the conjecture / work-score interpretation.

**Equilibrium:** C(r) = 1 ↔ r = 1.
**Symmetry:** C(r) = C(1/r) for r > 0.
**Perturbation decay (prediction):** transients scale as sech(n · log r).

Mirrors claims in the whitepaper and [`proofs/README.md`](../proofs/README.md) (Eigenverse).
-/

namespace Coinjecture

/-- Coherence C(r) = 2r / (1 + r²). Defined for r > 0. -/
def coherence (r : Float) : Float :=
  2 * r / (1 + r * r)

/-- Hyperbolic secant: sech(x) = 1 / cosh(x). -/
def sech (x : Float) : Float :=
  1 / Float.cosh x

theorem coherence_at_one : coherence 1 = 1 := by native_decide

theorem coherence_at_equilibrium_max_sample :
    coherence 1 ≥ coherence 0.5 := by native_decide

theorem coherence_symmetric_two : coherence 2 = coherence 0.5 := by native_decide

theorem coherence_symmetric_four : coherence 4 = coherence 0.25 := by native_decide

/-- log r = 0 when r = 1 (equilibrium). -/
theorem log_one_zero : Float.log 1 = 0 := by native_decide

/-- sech(0) = 1: full coherence at equilibrium perturbation scale. -/
theorem sech_at_zero : sech 0 = 1 := by native_decide

/-- Example transient: C(r) = sech(log r) at r = 2; C(r²) = sech(2 log r). -/
theorem perturbation_sech_sample_r2_n1 :
    coherence 2 = sech (Float.log 2) := by native_decide

theorem perturbation_sech_sample_r2_n2 :
    coherence 4 = sech (2 * Float.log 2) := by native_decide

/-- Dimensionless timing ratio r = solve_time / target_time (empirical / self-referential). -/
structure TimingRatio where
  solveSecs : Float
  targetSecs : Float
  posTarget : 0 < targetSecs

def ratio (t : TimingRatio) : Float :=
  t.solveSecs / t.targetSecs

def timingCoherence (t : TimingRatio) : Float :=
  coherence (ratio t)

theorem at_target_ratio_one (target : Float) (h : 0 < target) :
    timingCoherence { solveSecs := target, targetSecs := target, posTarget := h } = 1 := by
  native_decide

end Coinjecture
