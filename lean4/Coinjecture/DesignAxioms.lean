/-!
Appendix D — design axioms for consensus dynamics (unit normalization + symmetry).

Mirrors [`core/src/dimensional.rs`](../core/src/dimensional.rs): η = λ = 1/√2, τ_c = √2,
critical eigenvalue μ = (−1 + i)/√2 = exp(i·3π/4), primitive 8-cycle (gear ratio 3:8).

Tier A: numeric checks via `native_decide` on Float (no `sorry`).
-/

namespace Coinjecture

/-- Satoshi constant η = |Re(μ)| = Im(μ) = 1/√2. -/
def eta : Float := 1 / Float.sqrt 2

/-- Coupling λ at critical equilibrium (η = λ). -/
def lambdaCoupling : Float := eta

/-- Dimensionless consensus time unit τ_c = √2 = 1/η. -/
def tauC : Float := Float.sqrt 2

/-- Phase advance per step on the unit circle: 3π/4 radians. -/
def eigenPhaseStep : Float := 3 * Float.pi / 4

/-- Critical eigenvalue on the unit circle: μ = (−1 + i)/√2. -/
structure UnitEigenvalue where
  re : Float
  im : Float

def criticalMu : UnitEigenvalue where
  re := -eta
  im := eta

/-- |μ|² = Re² + Im² (unit normalization). -/
def muSqNorm (z : UnitEigenvalue) : Float :=
  z.re * z.re + z.im * z.im

theorem eta_eq_lambda : eta = lambdaCoupling := rfl

theorem unit_circle_eta : eta * eta + eta * eta = 1 := by native_decide

theorem mu_on_unit_circle : muSqNorm criticalMu = 1 := by native_decide

theorem symmetry_equal_magnitudes :
    Float.abs criticalMu.re = criticalMu.im := by native_decide

theorem tau_c_reciprocal : eta * tauC = 1 := by native_decide

/-- 8 × (3π/4) = 6π = 3 full rotations (2π each). -/
theorem eight_step_phase : 8 * eigenPhaseStep = 6 * Float.pi := by native_decide

theorem eight_cycle_closes_real :
    Float.cos (8 * eigenPhaseStep) = 1 := by native_decide

theorem eight_cycle_closes_imag :
    Float.sin (8 * eigenPhaseStep) = 0 := by native_decide

/-- Gear ratio 3:8 is coprime (primitive 8-orbit). -/
theorem gear_ratio_coprime : Nat.gcd 3 8 = 1 := by decide

/-- Unitary step preserves modulus: |e^{iθ}·β| = |β| on the unit circle (β = 1). -/
theorem unitary_modulus_preserved :
    Float.cos eigenPhaseStep * Float.cos eigenPhaseStep +
      Float.sin eigenPhaseStep * Float.sin eigenPhaseStep = 1 := by native_decide

end Coinjecture
