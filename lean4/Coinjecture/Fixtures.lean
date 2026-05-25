import Coinjecture.Rewards
import Coinjecture.WorkScore

/-!
Tier C — Rust ↔ Lean fixture alignment.

Canonical test vectors are duplicated in
[`consensus/tests/lean_fixture_alignment.rs`](../consensus/tests/lean_fixture_alignment.rs).
Regenerate check: `./scripts/verify-formal-fixtures.sh`
-/

namespace Coinjecture.Fixtures

/-- `log2_ratio(10, 1) × fpScale` — must match Rust `fixed_point::log2_ratio`. -/
def log2TenToOne : Nat := 3_250_000

/-- `workScoreFixed 10 1 10000` (full quality bps). -/
def workScoreTenOneUs : Nat := 3_250_000

/-- `mintAtoms 16 521` atoms (tokenomics regression). -/
def mintAtoms16Over521 : Nat :=
  (16 * rewardFixedPointScale * rewardEmissionMultiplier) / 521

/-- `log2_ratio(4, 1) = 2 · fpScale`. -/
def log2FourToOne : Nat := 2 * fpScale

theorem rust_log2_ten_to_one : log2TenToOne = 3_250_000 := rfl

theorem lean_matches_rust_log2_ten :
    log2Ratio 10 1 = some log2TenToOne := by native_decide

theorem lean_matches_rust_log2_four :
    log2Ratio 4 1 = some log2FourToOne := by native_decide

theorem lean_matches_rust_work_score :
    workScoreFixed 10 1 10_000 = workScoreTenOneUs := by native_decide

theorem lean_matches_rust_mint :
    mintAtoms 16 521 = mintAtoms16Over521 := by native_decide

theorem lean_matches_rust_first_harvest :
    mintAtoms 1 1 = 50 * rewardFixedPointScale := by native_decide

end Coinjecture.Fixtures
