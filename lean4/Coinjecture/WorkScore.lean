import Coinjecture.Rewards

/-!
Work score in **security bits** — mirrors [`consensus/src/work_score.rs`](../consensus/src/work_score.rs)
and the deterministic path in [`core/src/fixed_point.rs`](../core/src/fixed_point.rs).

**Ideal (analysis) form:**
```text
work_score = log₂(solve_time / verify_time) × quality     (quality ∈ [0, 1])
```

**Consensus form:** microsecond timestamps + quality in basis points, via `log2Ratio` + `applyQuality`.

Tier A: integer definitions + proved lemmas (no `sorry`, no Mathlib).
Tier B: real-valued interpretation — see `workScoreBitsIdeal_axiom` in `ClassicalAxioms.lean`.
-/

namespace Coinjecture

/-- Minimum verify-time floor (microseconds), matching `MIN_VERIFY_TIME_US`. -/
def minVerifyTimeUs : Nat := 1

/-- Minimum asymmetry multiplier: solve ≥ 2 × verify (f64 and µs paths). -/
def minAsymmetryUs : Nat := 2

/-- Fixed-point scale for deterministic work scores (`fixed_point::SCALE`). -/
def fpScale : Nat := 1_000_000

/-- Quality full scale in basis points. -/
def qualityBpsFull : Nat := 10_000

/-- Apply verify-time floor. -/
def verifyUsFloored (verifyUs : Nat) : Nat :=
  max verifyUs minVerifyTimeUs

/-- Asymmetry gate for the deterministic µs path. -/
def asymmetryOkUs (solveUs verifyUs : Nat) : Bool :=
  decide (solveUs ≥ minAsymmetryUs * verifyUsFloored verifyUs)

/-- Left shift used in `log2_ratio` (`SHIFT = 32`). -/
def fpShift : Nat := 32

/-- Bit length of a positive natural. -/
def bitLength (n : Nat) : Nat :=
  if n = 0 then 0 else Nat.log2 n + 1

/-- `log₂(numerator / denominator) × fpScale`, integer path (mirrors `fixed_point::log2_ratio`). -/
def log2Ratio (numerator denominator : Nat) : Option Nat :=
  if denominator = 0 then
    none
  else if numerator ≤ denominator then
    none
  else
    let ratioFp := (numerator <<< fpShift) / denominator
    if ratioFp = 0 then
      none
    else
      let l2 := Nat.log2 ratioFp
      if l2 < fpShift then
        none
      else
        let floorK := l2 - fpShift
        let shiftAmount := floorK
        let mantissa := ratioFp >>> shiftAmount
        let mantissaBase := 1 <<< fpShift
        let mantissaFrac := mantissa - mantissaBase
        let frac := (mantissaFrac * fpScale) >>> fpShift
        some (floorK * fpScale + frac)

/-- Apply quality in basis points (mirrors `fixed_point::apply_quality`). -/
def applyQuality (score qualityBps : Nat) : Nat :=
  if qualityBps = 0 then
    0
  else if qualityBps ≥ qualityBpsFull then
    score
  else
    (score * qualityBps) / qualityBpsFull

/-- Deterministic work score (mirrors `WorkScoreCalculator::calculate_deterministic`). -/
def workScoreFixed (solveUs verifyUs qualityBps : Nat) : Nat :=
  if qualityBps = 0 then
    0
  else if !asymmetryOkUs solveUs verifyUs then
    0
  else
    match log2Ratio solveUs (verifyUsFloored verifyUs) with
    | none => 0
    | some bits => applyQuality bits qualityBps

/-- Truncate fixed score to the u128 header summand (integer bits floor). -/
def workTruncFromFixed (score : Nat) : Nat := score / fpScale

/-- Cumulative chain security in fixed-point units (sum before converting to bits). -/
def chainSecurityFixed : List Nat → Nat
  | [] => 0
  | w :: rest => w + chainSecurityFixed rest

theorem workScoreFixed_zero_when_quality_zero (solve verify : Nat) :
    workScoreFixed solve verify 0 = 0 := rfl

theorem workScoreFixed_zero_when_trivial_asymmetry :
    workScoreFixed 1000 1000 10_000 = 0 := by
  native_decide

theorem applyQuality_zero : applyQuality 999 0 = 0 := rfl

theorem applyQuality_full (score : Nat) :
    applyQuality score qualityBpsFull = score := by
  simp [applyQuality, qualityBpsFull]

theorem applyQuality_half (score : Nat) :
    applyQuality score (qualityBpsFull / 2) = score / 2 := by
  unfold applyQuality qualityBpsFull
  have hlt : 5000 < 10000 := by decide
  simp [Nat.not_lt.mpr hlt, ite_false, ↓reduceIte]
  omega

theorem log2Ratio_none_when_not_asymmetric :
    log2Ratio 1 1 = none := rfl

theorem log2Ratio_exact_four :
    log2Ratio 4 1 = some (2 * fpScale) := by
  native_decide

theorem workScoreFixed_ten_to_one :
    workScoreFixed 10 1 10_000 = 3_250_000 := by
  native_decide

/-- End-to-end: fixed work → truncated summand → mint atoms. -/
def mintFromFixedWork (workFixed wParent : Nat) : Nat :=
  mintAtoms (workTruncFromFixed workFixed) wParent

theorem mintFromFixedWork_zero_parent (workFixed : Nat) :
    mintFromFixedWork workFixed 0 = 0 := by
  simp [mintFromFixedWork, mintAtoms_zero_when_parent_zero]

end Coinjecture
