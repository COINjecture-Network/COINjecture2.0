/-!
Appendix E — eight dimensional pools D(τ) = e^(−τ/√2) = e^(−η·τ).

Mirrors [`core/src/dimensional.rs`](../core/src/dimensional.rs) `TAU_POINTS` and
`DimensionalScales::scale_at_tau`.
-/

import Coinjecture.DesignAxioms

namespace Coinjecture

/-- Exponential decay scale D(τ) = e^(−η·τ). -/
def poolScale (tau : Float) : Float :=
  Float.exp (-eta * tau)

/-- Table 1 dimensionless τₙ samples (D1–D8). -/
def tauPoints : List Float :=
  [0.00, 0.20, 0.41, 0.68, 0.98, 1.36, 1.96, 2.72]

/-- Recover τ from target scale: τ = −√2 · ln(D). -/
def tauFromScale (d : Float) : Float :=
  -tauC * Float.log d

def goldenInv : Float := (Float.sqrt 5 - 1) / 2

theorem pool_genesis : poolScale 0 = 1 := by native_decide

theorem pool_d4_near_golden : (poolScale 0.68 - goldenInv).abs < 0.01 := by native_decide

theorem pool_d5_half : poolScale 0.98 = 0.5 := by native_decide

theorem pool_d7_quarter : (poolScale 1.96 - 0.25).abs < 0.01 := by native_decide

theorem tau_from_phi_inv :
    (tauFromScale goldenInv - 0.68).abs < 0.05 := by native_decide

/-- Fixed ln(φ)/ln(2) ratio (design constant relationship). -/
def logPhiOverLogTwo : Float := Float.log goldenInv / Float.log 2

theorem log_phi_log_two_sample : (logPhiOverLogTwo + 0.694).abs < 0.01 := by native_decide

end Coinjecture
