/-!
Tier B axioms — classical complexity results cited in docs, not reproved here.
References: Garey & Johnson (1979); standard Karp reductions.
-/

namespace Coinjecture

/-- SUBSET-SUM ∈ NP. -/
axiom subsetSum_inNP : True

/-- SUBSET-SUM is NP-complete. -/
axiom subsetSum_NPC : True

/-- 3-SAT ∈ NP. -/
axiom threeSat_inNP : True

/-- 3-SAT is NP-complete. -/
axiom threeSat_NPC : True

/-- Decision TSP ∈ NP. -/
axiom decisionTsp_inNP : True

/-- Decision TSP is NP-hard. -/
axiom decisionTsp_NPH : True

/-!
### Work score (ideal real analysis)

The on-chain implementation uses `workScoreFixed` in `Coinjecture/WorkScore.lean`.
The ideal security interpretation is:

`work_score = log₂(solve / verify) × quality` with quality ∈ [0, 1] and ratio ≥ 2.
-/

/-- Ideal bit-equivalent work score (analysis / whitepaper). Not executable on-chain. -/
axiom workScoreBitsIdeal_spec : True

/-- Doubling solve time adds one bit at quality 1 (ideal path). -/
axiom workScoreBitsIdeal_double_solve : True

/-- Cumulative chain security is the sum of per-block bit scores (ideal path). -/
axiom chainSecurityBitsIdeal_spec : True

/-!
### Emission economics (w/√W — v4)

The on-chain law **`mint = ⌊w·S·K / isqrt(W)⌋`** is **Tier A** in `Coinjecture/Rewards.lean`.
These axioms record interpretive claims used in [`docs/SECURITY_CALCULATIONS.md`](../docs/SECURITY_CALCULATIONS.md)
that are not reproved from first principles here.
-/

/-- Minted emission does not enter cumulative fork-choice work `W`. -/
axiom emission_separate_from_fork_choice : True

/-- Per-block mint falls as parent cumulative work grows (1/√W tail; spot-checked in Lean). -/
axiom emission_decays_with_parent_work : True

/-- An alternate chain's coinbase only affects honest balances if it becomes the heaviest valid tip. -/
axiom losing_fork_emission_inert : True

end Coinjecture
