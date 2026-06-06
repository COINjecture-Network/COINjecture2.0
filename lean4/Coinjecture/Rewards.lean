/-!
On-chain block rewards — mirrors [`tokenomics/src/rewards.rs`](../tokenomics/src/rewards.rs).

**Whitepaper shape:** allocation proportional to `w_trunc / √W_parent` with fixed-point scale `S`
and emission multiplier `K`.

```text
mint_atoms = ⌊ w_trunc · S · K / isqrt(W_parent) ⌋
```

Tier A: integer spec and proved lemmas (no `sorry`).
-/

namespace Coinjecture

/-- Atoms per one display BEANS (`10^12`). -/
def rewardFixedPointScale : Nat := 10 ^ 12

/-- Consensus emission multiplier `K` in `mint = ⌊w·S·K / isqrt(W)⌋`. -/
def rewardEmissionMultiplier : Nat := 50

/-- Truncated header work score (non-negative integer part of bits). -/
abbrev WorkTrunc := Nat

/-- Truncate a non-negative rational work score to the on-chain summand. -/
def headerWorkTrunc (workScoreFloor : WorkTrunc) : WorkTrunc := workScoreFloor

/-- Parent cumulative work `W` through the parent block (sum of truncated scores). -/
abbrev CumulativeWork := Nat

/-- Minted coinbase atoms for one block. Matches `RewardCalculator::calculate_block_reward`. -/
def mintAtoms (wTrunc : WorkTrunc) (wParent : CumulativeWork) : Nat :=
  if wParent = 0 then
    0
  else
    let denom := Nat.max (Nat.sqrt wParent) 1
    (wTrunc * rewardFixedPointScale * rewardEmissionMultiplier) / denom

/-- Display BEANS from atoms (exact when divisible by `S`). -/
def beansFromAtoms (atoms : Nat) : Nat :=
  atoms / rewardFixedPointScale

/-- One block's display BEANS reward. -/
def mintBeans (wTrunc : WorkTrunc) (wParent : CumulativeWork) : Nat :=
  beansFromAtoms (mintAtoms wTrunc wParent)

theorem mintAtoms_zero_when_parent_zero (w : WorkTrunc) :
    mintAtoms w 0 = 0 := rfl

theorem mintAtoms_zero_when_work_zero (wParent : CumulativeWork) (h : wParent ≠ 0) :
    mintAtoms 0 wParent = 0 := by
  simp [mintAtoms, h]

/-- **First harvest:** when `W_parent = 1` and `w_trunc ≥ 1`, mint is exactly `S·K` atoms (= `K` display BEANS). -/
theorem first_harvest (w : WorkTrunc) (hw : 1 ≤ w) :
    mintAtoms w 1 = rewardFixedPointScale * rewardEmissionMultiplier := by
  unfold mintAtoms
  simp only [if_neg (by decide : (1 : Nat) ≠ 0), Nat.sqrt_one, Nat.max_def]
  rw [Nat.mul_assoc]
  exact Nat.mul_div_cancel_left (rewardFixedPointScale * rewardEmissionMultiplier) (by decide : 0 < 1)

theorem first_harvest_beans (w : WorkTrunc) (hw : 1 ≤ w) :
    mintBeans w 1 = rewardEmissionMultiplier := by
  unfold mintBeans beansFromAtoms
  rw [first_harvest w hw, Nat.mul_div_cancel_left rewardEmissionMultiplier (by decide : 0 < rewardFixedPointScale)]

/-- Floor bound: `mint · W_parent ≤ w · S · K`. -/
theorem mintAtoms_le_numerator (w wParent : Nat) (h : wParent ≠ 0) :
    mintAtoms w wParent * wParent ≤ w * rewardFixedPointScale * rewardEmissionMultiplier := by
  unfold mintAtoms
  simp only [if_neg h]
  rw [Nat.mul_comm]
  exact Nat.mul_div_le (w * rewardFixedPointScale * rewardEmissionMultiplier) wParent

/-- Monotonic in work for fixed parent cumulative work. -/
theorem mintAtoms_mono_work {w₁ w₂ wParent : Nat}
    (hle : w₁ ≤ w₂) (hW : wParent ≠ 0) :
    mintAtoms w₁ wParent ≤ mintAtoms w₂ wParent := by
  have hmul :
      w₁ * (rewardFixedPointScale * rewardEmissionMultiplier) ≤
        w₂ * (rewardFixedPointScale * rewardEmissionMultiplier) :=
    Nat.mul_le_mul_right _ hle
  unfold mintAtoms
  simp only [if_neg hW, Nat.mul_assoc]
  exact Nat.div_le_div_right hmul

/-- Sum of truncated work scores for a prefix (parent cumulative work at height `i`). -/
def cumulativeWorkPrefix (ws : List WorkTrunc) (i : Nat) : CumulativeWork :=
  ((ws.take i).foldl (· + ·) 0)

/-- Minted atoms for block at index `i` in a chain of truncated work scores. -/
def mintAt (ws : List WorkTrunc) (i : Nat) : Nat :=
  match ws[i]? with
  | none => 0
  | some w => mintAtoms w (cumulativeWorkPrefix ws i)

/-- Total coinbase atoms minted along a finite chain. -/
def totalMinted (ws : List WorkTrunc) : Nat :=
  (List.range ws.length).foldl (fun acc i => acc + mintAt ws i) 0

theorem totalMinted_nil : totalMinted [] = 0 := rfl

/-- Rust regression: `w=16, W=521` mint matches integer formula. -/
theorem mintAtoms_sixteen_over_521 :
    mintAtoms 16 521 =
      (16 * rewardFixedPointScale * rewardEmissionMultiplier) / 22 := by
  native_decide

end Coinjecture
