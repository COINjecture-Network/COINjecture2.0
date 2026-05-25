import Mathlib

set_option linter.unusedSimpArgs false
set_option linter.unusedTactic false

/-!
# Complex decomposition — Euclidean and Lorentzian charts

Two honest coordinate systems on ℂ \\ {0} (and off the light cone for Lorentzian):

**Euclidean** (total on `z ≠ 0`): radial `log ‖z‖`, angular `arg z`.
**Lorentzian** (off the light cone `Re² = Im²`): hyperbolic modulus `½ log |Re² − Im²|`,
rapidity from `(Re ± Im)` ratio — chart selected by `qSign`.

**Anchors** `±1 ± i` (rescaled to modulus √2) lie on the light cone: Euclidean coordinates
are defined, but `|Re² − Im²| = 0` so the Lorentzian radial coordinate is singular. That is the
formal obstruction, not metaphor.

Network reading (see `Coinjecture/Coherence.lean`): timing ratio `r = t_solve / t_target`
with equilibrium `r = 1` and `C(r) = 2r/(1+r²)`; imaginary axis = self-referential target,
real axis = empirical solve time.
-/

namespace Framework

open Complex Real

/-! ## Anchor: diagonal vertices; on the light cone -/

/-- The four framework anchors at `z⁴ = −4`: `±1 ± i`. -/
inductive Anchor : Type
  | Q1 | Q2 | Q3 | Q4
  deriving DecidableEq, Repr

noncomputable def Anchor.value : Anchor → ℂ
  | .Q1 =>  1 + I
  | .Q2 => -1 + I
  | .Q3 => -1 - I
  | .Q4 =>  1 - I

@[simp] theorem Anchor.abs_value (a : Anchor) : ‖a.value‖ = Real.sqrt 2 := by
  rcases a with _ | _ | _ | _ <;> norm_num [Complex.normSq, Complex.norm_def, Anchor.value]

theorem Anchor.value_ne_zero (a : Anchor) : a.value ≠ 0 := by
  cases a <;> simp [Anchor.value, Complex.ext_iff]

/-! ## Sign and octant (Lorentzian chart indicator) -/

inductive Sign : Type
  | pos | zero | neg
  deriving DecidableEq, Repr

noncomputable def Sign.ofReal (x : ℝ) : Sign :=
  if x > 0 then .pos else if x = 0 then .zero else .neg

/-- Minkowski invariant `Q(z) = Re² − Im²`. Zero on the light cone. -/
noncomputable def qInvariant (z : ℂ) : ℝ :=
  z.re ^ 2 - z.im ^ 2

def onLightCone (z : ℂ) : Prop :=
  qInvariant z = 0

def offLightCone (z : ℂ) : Prop :=
  qInvariant z ≠ 0

structure OctantAddress where
  reSign : Sign
  imSign : Sign
  /-- Sign of `Re² − Im²`: timelike-Re (.pos), timelike-Im (.neg), or light cone (.zero). -/
  qSign  : Sign
  deriving Repr

noncomputable def octantAddress (z : ℂ) : OctantAddress :=
  { reSign := Sign.ofReal z.re
    imSign := Sign.ofReal z.im
    qSign  := Sign.ofReal (qInvariant z) }

/-- Every anchor value lies on the light cone. -/
theorem anchor_on_lightCone (a : Anchor) : onLightCone a.value := by
  rcases a with _ | _ | _ | _ <;> norm_num [onLightCone, qInvariant, Anchor.value]

/-! ## Anchor · coherence factorization -/

noncomputable def anchorOf (z : ℂ) : Anchor :=
  match Sign.ofReal z.re, Sign.ofReal z.im with
  | .pos, .pos => .Q1
  | .pos, .neg => .Q4
  | .neg, .pos => .Q2
  | .neg, .neg => .Q3
  | .pos, .zero => .Q1
  | .neg, .zero => .Q2
  | .zero, .pos => .Q1
  | .zero, .neg => .Q4
  | .zero, .zero => .Q1

noncomputable def coherenceOf (z : ℂ) : ℂ :=
  z / (anchorOf z).value

theorem decompose (z : ℂ) :
    z = (anchorOf z).value * coherenceOf z := by
  unfold coherenceOf
  rw [mul_div_cancel₀ _ (Anchor.value_ne_zero _)]

/-! ## Euclidean chart (defined for all `z`, total at `z ≠ 0`) -/

/-- Euclidean radial: `log ‖z‖` (0 at origin by convention). -/
noncomputable def euclRadial (z : ℂ) : ℝ :=
  if z = 0 then 0 else Real.log ‖z‖

/-- Euclidean angular: `arg z`. -/
noncomputable def euclAngular (z : ℂ) : ℝ :=
  Complex.arg z

/-- Legacy names (deprecated in docs; kept as abbreviations). -/
noncomputable abbrev witnessLog := euclRadial
noncomputable abbrev observerArg := euclAngular

/-- Euclidean recovery: `z = exp(euclRadial + i·euclAngular)` for `z ≠ 0`. -/
theorem eucl_recover {z : ℂ} (hz : z ≠ 0) :
    z = Complex.exp ((euclRadial z : ℂ) + I * (euclAngular z : ℂ)) := by
  rw [Complex.ext_iff]
  simp +decide [Complex.exp_re, Complex.exp_im, euclRadial, euclAngular]
  rw [if_neg hz, Real.exp_log (norm_pos_iff.mpr hz)]
  rw [← Complex.norm_mul_cos_arg, ← Complex.norm_mul_sin_arg]
  exact ⟨rfl, rfl⟩

theorem alchemy_recover {z : ℂ} (hz : z ≠ 0) :
    z = Complex.exp ((witnessLog z : ℂ) + I * (observerArg z : ℂ)) :=
  eucl_recover hz

/-! ## Lorentzian chart (off the light cone) -/

/-- Lorentzian radial: `½ · log |Re² − Im²|`.
    On the light cone (`qInvariant = 0`) the chart is singular; we use `0` by convention. -/
noncomputable def lorRadial (z : ℂ) : ℝ :=
  if qInvariant z = 0 then 0 else Real.log (abs (qInvariant z)) / 2

/-- Rapidity in the timelike-Re chart (`qSign = pos`): `½ · log ((Re+Im)/(Re−Im))`. -/
noncomputable def lorRapidityReChart (z : ℂ) : ℝ :=
  if z.re = z.im then 0 else Real.log ((z.re + z.im) / (z.re - z.im)) / 2

/-- Rapidity in the timelike-Im chart (`qSign = neg`): `½ · log ((Re+Im)/(Im−Re))`. -/
noncomputable def lorRapidityImChart (z : ℂ) : ℝ :=
  if z.re = z.im then 0 else Real.log ((z.re + z.im) / (z.im - z.re)) / 2

/-- Chart-selected Lorentzian rapidity; 0 on the light cone. -/
noncomputable def lorRapidity (z : ℂ) : ℝ :=
  match Sign.ofReal (qInvariant z) with
  | .pos => lorRapidityReChart z
  | .neg => lorRapidityImChart z
  | .zero => 0

theorem octant_qSign_eq_sign (z : ℂ) :
    (octantAddress z).qSign = Sign.ofReal (qInvariant z) := rfl

/-! ## Decomposition bundle -/

structure Decomposition (z : ℂ) where
  anchor : Anchor := anchorOf z
  coherence : ℂ := coherenceOf z
  octant : OctantAddress := octantAddress z
  euclR : ℝ := euclRadial z
  euclθ : ℝ := euclAngular z
  lorR : ℝ := lorRadial z
  lorη : ℝ := lorRapidity z
  factor_correct : z = anchor.value * coherence := by
    dsimp [anchor, coherence]
    exact decompose z

noncomputable def decompositionOf (z : ℂ) : Decomposition z :=
  { factor_correct := decompose z }

theorem coherence_zero : coherenceOf 0 = 0 := by
  unfold coherenceOf; simp

theorem coherence_abs_of_unit {z : ℂ} (hz : ‖z‖ = 1) :
    ‖coherenceOf z‖ = 1 / Real.sqrt 2 := by
  rw [coherenceOf]
  norm_num [hz, Anchor.abs_value]

/-! ## μ: equilibrium on the unit circle -/

noncomputable def mu : ℂ := (-1 + I) / Real.sqrt 2

theorem mu_abs : ‖mu‖ = 1 := by
  unfold mu
  rw [Complex.norm_div]
  have h2 : (0 : ℝ) < 2 := by norm_num
  have hpos : (0 : ℝ) < Real.sqrt 2 := Real.sqrt_pos.mpr h2
  have hnum : ‖(-1 + I : ℂ)‖ = Real.sqrt 2 := by
    norm_num [Complex.norm_eq_sqrt_sq_add_sq]
  have hden : ‖((Real.sqrt 2 : ℝ) : ℂ)‖ = Real.sqrt 2 := by
    rw [Complex.norm_real, Real.norm_eq_abs, abs_of_pos hpos]
  rw [hnum, hden, div_self (ne_of_gt hpos)]

theorem mu_anchor : anchorOf mu = .Q2 := by
  unfold anchorOf mu Sign.ofReal
  norm_num [mu]

theorem mu_on_lightCone : onLightCone mu := by
  unfold onLightCone qInvariant mu
  ring_nf
  norm_num [Real.sq_sqrt (show (0 : ℝ) ≤ 2 by norm_num)]

theorem lorRadial_on_lightCone (z : ℂ) (h : onLightCone z) :
    lorRadial z = 0 := by
  unfold lorRadial onLightCone qInvariant at *
  exact if_pos h

theorem mu_unique : ∀ z : ℂ,
    ‖z‖ = 1 → -z.re = z.im → z.re < 0 → z = mu := by
  norm_num [Complex.ext_iff, mu]
  intro z h1 h2 h3
  rw [Complex.norm_def] at h1
  simp_all +decide [Complex.normSq_apply]
  ring_nf at *
  constructor <;> rw [← Real.sqrt_div_self] <;> nlinarith
    [Real.sqrt_nonneg 2, Real.sq_sqrt zero_le_two,
     Real.sqrt_nonneg (z.re ^ 2 + z.im ^ 2)]

end Framework
