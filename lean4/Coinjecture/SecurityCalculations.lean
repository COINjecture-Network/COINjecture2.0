import Coinjecture.SecurityModel

/-!
# Security calculations (Satoshi §11 style)

Discrete backbone for “exponential security in confirmations” when the honest network
holds majority work-share (`q < p`).

Mirrors [`docs/SECURITY_CALCULATIONS.md`](../../docs/SECURITY_CALCULATIONS.md).
-/

namespace Coinjecture.Security

/-!
## Work-share parameters (basis points, 10 000 = 100%)
-/

def fullBps : Nat := 10_000

/-- Honest work-share in basis points (example: 90%). -/
def honestBpsExample : Nat := 9000

/-- Attacker work-share in basis points (example: 10%). -/
def attackBpsExample : Nat := 1000

theorem attack_share_lt_honest_example :
    attackBpsExample < honestBpsExample := by decide

theorem attack_plus_honest_example :
    attackBpsExample + honestBpsExample = fullBps := by decide

/-!
## Catch-up exponent: q^z < p^z when q < p

Satoshi’s bound P ≈ (q/p)^z is < 1 exactly when q^z < p^z for positive integers.
-/

theorem catch_up_power_lt {q p z : Nat} (hq : q < p) (hz : 0 < z) :
    q ^ z < p ^ z := by
  rcases z with _ | z
  · cases hz
  · exact Nat.pow_lt_pow_left hq (Nat.succ_ne_zero z)

/-- If attacker share (bps) is strictly less than honest share (bps), same inequality. -/
theorem catch_up_bps_lt {qBps pBps z : Nat} (hq : qBps < pBps) (hz : 0 < z) :
    qBps ^ z < pBps ^ z :=
  catch_up_power_lt hq hz

/-!
## Numeric spot-checks (Section 6 of SECURITY_CALCULATIONS.md)
-/

/-- 10% vs 90% honest, z = 5: (1/9)^5 < 1  ↔  1^5 < 9^5. -/
theorem catchup_one_ninth_pow_five : 1 ^ 5 < 9 ^ 5 := by decide

/-- 30% vs 70% honest, z = 10: 3^10 < 7^10. -/
theorem catchup_three_seventh_pow_ten : 3 ^ 10 < 7 ^ 10 := by decide

/-- 49% vs 51% honest, z = 20: 49^20 < 51^20 (marginal majority). -/
theorem catchup_fortynine_fiftyone_pow_twenty : 49 ^ 20 < 51 ^ 20 := by decide

/-- 20% vs 80% honest, z = 10: 1^10 < 4^10. -/
theorem catchup_one_fourth_pow_ten : 1 ^ 10 < 4 ^ 10 := by decide

/-!
## Link to chain work (Tier A from SecurityModel)
-/

/-- Cumulative work strictly increases when a positive summand is appended. -/
theorem chainWork_strict_mono_append (w : Nat) (rest : List Nat) (hw : 0 < w) :
    chainWork rest < chainWork (w :: rest) := by
  simp [chainWork, chainSecurityFixed, hw]

/-- Zero-work blocks do not increase cumulative W (asymmetry / quality failure). -/
theorem chainWork_append_zero (rest : List Nat) :
    chainWork (0 :: rest) = chainWork rest := by
  simp [chainWork, chainSecurityFixed]

end Coinjecture.Security
