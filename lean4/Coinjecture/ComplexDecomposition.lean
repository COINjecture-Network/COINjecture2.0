/-
  ComplexDecomposition.lean

  A formal decomposition of complex numbers in the framework's vocabulary.

  Given a complex number z, this file provides:
    - Quadrant (or boundary status)
    - 3-bit octant address (sign Re, sign Im, sign of Re²-Im²)
    - Polar form (modulus and argument)
    - Anchor-coherence factorization z = B · C with B at a framework vertex
    - Alchemy: log z as additive coordinates (witness, observer)

  Structural facts are encoded as definitions; relationships are theorems.
  No interpretive layer (no "witness", "observer", "perpendicular collapse"
  metaphor in the formalization itself — those words live in comments).
-/

import Mathlib

namespace Framework

open Complex Real

/-! ## Anchor: The four framework anchors (informal: "diagonal vertices of the octagon") -/

/-- The four framework anchors at the diagonal vertices of the octagon.
    These are the four roots of z⁴ = -4, i.e. z ∈ {1+I, -1+I, -1-I, 1-I}.

    Corresponds to the informal `inductive Anchor` declaration:
    "Q1 = 1+I, Q2 = -1+I, Q3 = -1-I, Q4 = 1-I". -/
inductive Anchor : Type
  | Q1  -- 1 + I    (Re > 0, Im > 0)
  | Q2  -- -1 + I   (Re < 0, Im > 0)
  | Q3  -- -1 - I   (Re < 0, Im < 0)
  | Q4  -- 1 - I    (Re > 0, Im < 0)
  deriving DecidableEq, Repr

/-- The complex value of each anchor.

    Corresponds to the informal `def Anchor.value`:
    "Q1 ↦ 1+I, Q2 ↦ -1+I, Q3 ↦ -1-I, Q4 ↦ 1-I". -/
noncomputable def Anchor.value : Anchor → ℂ
  | .Q1 =>  1 + I
  | .Q2 => -1 + I
  | .Q3 => -1 - I
  | .Q4 =>  1 - I

/-
Each anchor has modulus √2.

    Corresponds to the informal `theorem Anchor.abs_value`:
    "Complex.abs a.value = Real.sqrt 2" for all anchors `a`.
-/
@[simp] theorem Anchor.abs_value (a : Anchor) : ‖a.value‖ = Real.sqrt 2 := by
  rcases a with ( _ | _ | _ | _ ) <;> norm_num [ Complex.normSq, Complex.norm_def ];
  · rw [ show Q1.value = 1 + Complex.I by rfl ] ; norm_num;
  · norm_num [ Q2, Q1, Q3, Q4, Anchor.value ];
  · norm_num [ Framework.Anchor.value ];
  · norm_num [ Q4, Anchor.value ]

/-! ## Sign: sign classification of a real number -/

/-- Sign data for a real number: positive, zero, or negative.

    Corresponds to the informal `inductive Sign`:
    "pos | zero | neg". -/
inductive Sign : Type
  | pos | zero | neg
  deriving DecidableEq, Repr

/-- Classify a real number by its sign.

    Corresponds to the informal `def Sign.ofReal`:
    "if x > 0 then .pos else if x = 0 then .zero else .neg". -/
noncomputable def Sign.ofReal (x : ℝ) : Sign :=
  if x > 0 then .pos
  else if x = 0 then .zero
  else .neg

/-! ## OctantAddress: 3-bit octant classification -/

/-- The 3-bit octant address.
    Q(z) = Re(z)² - Im(z)² determines which column dominates.

    Corresponds to the informal `structure OctantAddress`:
    "reSign, imSign, qSign" (signs of Re(z), Im(z), Re(z)²-Im(z)²). -/
structure OctantAddress where
  reSign : Sign  -- sign of Re(z)
  imSign : Sign  -- sign of Im(z)
  qSign  : Sign  -- sign of Re(z)² - Im(z)² — which column dominates
  deriving Repr

/-- Compute the octant address of a complex number.

    Corresponds to the informal `def octantAddress`:
    "reSign := sign of z.re, imSign := sign of z.im,
     qSign := sign of (z.re² - z.im²)". -/
noncomputable def octantAddress (z : ℂ) : OctantAddress :=
  { reSign := Sign.ofReal z.re
    imSign := Sign.ofReal z.im
    qSign  := Sign.ofReal (z.re^2 - z.im^2) }

/-! ## anchorOf / coherenceOf: anchor assignment and factorization -/

/-- Pick the anchor for a complex number based on its quadrant.
    Points on the axes are assigned to a default quadrant by convention.

    Corresponds to the informal `def anchorOf`:
    "Match on (sign Re, sign Im) to assign an anchor;
     axis and origin cases default to Q1, Q2, or Q4". -/
noncomputable def anchorOf (z : ℂ) : Anchor :=
  match Sign.ofReal z.re, Sign.ofReal z.im with
  | .pos, .pos => .Q1
  | .pos, .neg => .Q4
  | .neg, .pos => .Q2
  | .neg, .neg => .Q3
  | .pos, .zero => .Q1     -- positive real axis → Q1 by convention
  | .neg, .zero => .Q2     -- negative real axis → Q2
  | .zero, .pos => .Q1     -- positive imaginary axis → Q1
  | .zero, .neg => .Q4     -- negative imaginary axis → Q4
  | .zero, .zero => .Q1    -- origin → Q1 (default; coherence undefined here)

/-- The coherence factor: z = B · C, so C = z / B.
    For z = 0, this is 0 (which respects 0 = B · 0).

    Corresponds to the informal `def coherenceOf`:
    "z / (anchorOf z).value". -/
noncomputable def coherenceOf (z : ℂ) : ℂ :=
  z / (anchorOf z).value

/-- The anchor value is always nonzero (it is one of ±1±I, all with modulus √2). -/
theorem Anchor.value_ne_zero (a : Anchor) : a.value ≠ 0 := by
  cases a <;> simp [Anchor.value, Complex.ext_iff]

/-- The fundamental factorization z = B · C.

    Corresponds to the informal `theorem decompose`:
    "z = (anchorOf z).value * coherenceOf z". -/
theorem decompose (z : ℂ) :
    z = (anchorOf z).value * coherenceOf z := by
  unfold coherenceOf
  rw [mul_div_cancel₀ _ (Anchor.value_ne_zero _)]

/-! ## Witness and Observer: additive (logarithmic) coordinates -/

/-- The witness column: log of the modulus (additive picture).

    Corresponds to the informal `def witnessLog`:
    "if z = 0 then 0 else Real.log (‖z‖)". -/
noncomputable def witnessLog (z : ℂ) : ℝ :=
  if z = 0 then 0 else Real.log ‖z‖

/-- The observer column: argument (additive picture).

    Corresponds to the informal `def observerArg`:
    "Complex.arg z". -/
noncomputable def observerArg (z : ℂ) : ℝ :=
  Complex.arg z

/-
Alchemy: the additive form of z is (witness, observer).
    Recovered as exp(witness + i·observer) for z ≠ 0.

    Corresponds to the informal `theorem alchemy_recover`:
    "z = exp(witnessLog z + I * observerArg z)" for z ≠ 0.
    The original source had a sorry; we provide a complete proof using
    the polar-form identity from Mathlib.
-/
theorem alchemy_recover {z : ℂ} (hz : z ≠ 0) :
    z = Complex.exp ((witnessLog z : ℂ) + I * (observerArg z : ℂ)) := by
      rw [ Complex.ext_iff ];
      simp +decide [ Complex.exp_re, Complex.exp_im, witnessLog, observerArg ];
      rw [ if_neg hz, Real.exp_log ( norm_pos_iff.mpr hz ) ] ; rw [ ← Complex.norm_mul_cos_arg, ← Complex.norm_mul_sin_arg ] ;
      exact ⟨ rfl, rfl ⟩

/-! ## Decomposition structure: the complete framework reading -/

/-- The complete framework reading of a complex number.
    Stores derived quantities; relationships verified as theorems.

    Corresponds to the informal `structure Decomposition`:
    "anchor, coherence, octant, witness, observer, factor_correct". -/
structure Decomposition (z : ℂ) where
  /-- Which framework anchor z is closest to.
      Corresponds to informal field `anchor := anchorOf z`. -/
  anchor : Anchor := anchorOf z
  /-- The coherence factor C such that z = anchor.value · C.
      Corresponds to informal field `coherence := coherenceOf z`. -/
  coherence : ℂ := coherenceOf z
  /-- The octant address (3 sign bits).
      Corresponds to informal field `octant := octantAddress z`. -/
  octant : OctantAddress := octantAddress z
  /-- Witness column value (log modulus).
      Corresponds to informal field `witness := witnessLog z`. -/
  witness : ℝ := witnessLog z
  /-- Observer column value (argument).
      Corresponds to informal field `observer := observerArg z`. -/
  observer : ℝ := observerArg z
  /-- Verification: z factors as anchor · coherence.
      Corresponds to informal field `factor_correct`. -/
  factor_correct : z = anchor.value * coherence := by
    exact decompose z

/-- The canonical decomposition of any z.

    Corresponds to the informal `def decompositionOf`:
    "the default Decomposition instance for z". -/
noncomputable def decompositionOf (z : ℂ) : Decomposition z := {}

/-! ## Edge case theorems

  The framework's decomposition has well-known degenerate cases. We
  prove the structural facts about them rather than ignoring them. -/

/-- At z = 0, the decomposition has zero coherence (and the anchor is by convention).

    Corresponds to the informal `theorem coherence_zero`:
    "coherenceOf 0 = 0". -/
theorem coherence_zero : coherenceOf 0 = 0 := by
  unfold coherenceOf; simp

/-
On the unit circle, the coherence has modulus 1/√2.

    Corresponds to the informal `theorem coherence_abs_of_unit`:
    "‖coherenceOf z‖ = 1 / √2" when ‖z‖ = 1.
-/
theorem coherence_abs_of_unit {z : ℂ} (hz : ‖z‖ = 1) :
    ‖coherenceOf z‖ = 1 / Real.sqrt 2 := by
      rw [ coherenceOf ];
      norm_num [ hz, Anchor.abs_value ]

/-! ## μ: the equilibrium point -/

/-- The framework's μ: the equilibrium at (-1+I)/√2.

    Corresponds to the informal `def mu`:
    "(-1 + I) / Real.sqrt 2". -/
noncomputable def mu : ℂ := (-1 + I) / Real.sqrt 2

/-
μ has unit modulus.

    Corresponds to the informal `theorem mu_abs`:
    "‖mu‖ = 1".
-/
theorem mu_abs : ‖mu‖ = 1 := by
  unfold mu;
  norm_num [ Norm.norm ];
  norm_num [ Complex.normSq ]

/-
μ has anchor Q2 (since Re μ < 0, Im μ > 0).

    Corresponds to the informal `theorem mu_anchor`:
    "anchorOf mu = .Q2".
-/
theorem mu_anchor : anchorOf mu = .Q2 := by
  -- Unfold the definition of `anchorOf` and `mu`. Then simplify the sign calculations.
  unfold anchorOf mu;
  simp +decide [Sign.ofReal];
   (
   norm_num +zetaDelta at *)

/-
μ is uniquely determined by three conditions: unit modulus,
    column coupling (-Re = Im), and dissipative half-plane (Re < 0).

    Corresponds to the informal `theorem mu_unique`:
    "∀ z, ‖z‖ = 1 → -z.re = z.im → z.re < 0 → z = mu".
-/
theorem mu_unique : ∀ z : ℂ,
    ‖z‖ = 1 →
    -z.re = z.im →
    z.re < 0 →
    z = mu := by
      norm_num [ Complex.ext_iff, mu ];
      intro z h1 h2 h3; rw [ Complex.norm_def ] at h1; simp_all +decide [ Complex.normSq_apply ] ; ring_nf at *;
      constructor <;> rw [ ← Real.sqrt_div_self ] <;> nlinarith [ Real.sqrt_nonneg 2, Real.sq_sqrt zero_le_two, Real.sqrt_nonneg ( z.re ^ 2 + z.im ^ 2 ) ]

end Framework