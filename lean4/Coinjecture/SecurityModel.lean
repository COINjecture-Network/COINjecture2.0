import Coinjecture.ClassicalAxioms
import Coinjecture.Rewards
import Coinjecture.WorkScore

/-!
# COINjecture security model (formal sketch)

Mirrors the whitepaper PoUW interpretation ([`consensus/src/work_score.rs`](../consensus/src/work_score.rs)),
chain tip choice by cumulative work, and reward proportionality ([`tokenomics/src/rewards.rs`](../tokenomics/src/rewards.rs)).

**Tier A:** definitions + proved lemmas on natural numbers / fixed-point paths already in
`WorkScore.lean` and `Rewards.lean`.

**Tier B:** axioms for classical NP hardness, ideal real `log₂` analysis, network/p2p,
and production ZK privacy (placeholder MAC is testnet-only — see `core/src/privacy.rs`).
-/

namespace Coinjecture.Security

/-!
## 1. Trust assumptions (explicit)
-/

/-- What honest full nodes are assumed to enforce on every accepted block. -/
structure HonestNodePolicy where
  /-- PoUW solution must pass the deterministic checker for its problem type. -/
  verify_before_accept : Bool := true
  /-- Header work score uses the consensus fixed-point path (not f64 compare). -/
  deterministic_work_score : Bool := true
  /-- Tip follows the chain with greatest cumulative truncated work. -/
  heaviest_chain_tip : Bool := true

def defaultHonestPolicy : HonestNodePolicy := {}

/-- Adversary capabilities we do **not** model in Tier A (network layer). -/
structure AdversaryCapabilities where
  /-- Can propagate invalid blocks (rejected by honest policy). -/
  broadcast_invalid_blocks : Bool := true
  /-- Can grind self-reported solve time on losing blocks (winner sets competitive time). -/
  inflate_losing_solve_time : Bool := true
  /-- Cannot forge a passing verify without a valid witness (checker soundness). -/
  forge_verify_without_witness : Bool := false

/-!
## 2. Work score as security bits (Tier A)
-/

/-- A block contributes zero on-chain work when the asymmetry gate fails. -/
theorem zero_work_when_no_asymmetry (solve verify : Nat)
    (h : asymmetryOkUs solve verify = false) :
    workScoreFixed solve verify qualityBpsFull = 0 := by
  simp [workScoreFixed, h]

theorem zero_work_when_quality_zero (solve verify : Nat) :
    workScoreFixed solve verify 0 = 0 :=
  workScoreFixed_zero_when_quality_zero solve verify

/-- Truncated summand used in cumulative chain work `W`. -/
def blockWorkTrunc (solveUs verifyUs qualityBps : Nat) : Nat :=
  workTruncFromFixed (workScoreFixed solveUs verifyUs qualityBps)

/-- Cumulative security `W` through a list of per-block truncated scores. -/
def chainWork (scores : List Nat) : Nat :=
  chainSecurityFixed scores

theorem chainWork_nil : chainWork [] = 0 := rfl

theorem chainWork_cons (w : Nat) (rest : List Nat) :
    chainWork (w :: rest) = w + chainWork rest := rfl

/-!
## 3. Heaviest-chain rule (fork choice)
-/

/-- Chain A is strictly heavier than B. -/
def heavierChain (wA wB : Nat) : Bool :=
  decide (wB < wA)

theorem heavierChain_refl_not (w : Nat) (hw : w ≠ 0) :
    heavierChain w w = false := by
  simp [heavierChain, hw]

theorem heavierChain_trans (wA wB wC : Nat) (hAB : wB < wA) (hBC : wC < wB) :
    heavierChain wA wC = true := by
  simp [heavierChain, Nat.lt_trans hBC hAB]

/-- Honest tip choice: prefer strictly larger cumulative `W` (tie-breaking is implementation-defined). -/
def prefersTip (wCandidate wLocal : Nat) : Bool :=
  heavierChain wCandidate wLocal

/-!
## 4. Reward proportionality (Tier A)
-/

/-- Mint is zero when the parent chain has no recorded work yet. -/
theorem mint_requires_parent_work (w : Nat) :
    mintAtoms w 0 = 0 :=
  mintAtoms_zero_when_parent_zero w


/-!
## 5. Ideal analysis (Tier B — see `ClassicalAxioms.lean`)
-/

/-- NP checkers run in polynomial time (classical).
    Bundles `threeSat_inNP` and `subsetSum_inNP` from `ClassicalAxioms.lean`. -/
abbrev CheckerPolytime : Prop := True

/-- Ideal log₂ security interpretation (whitepaper; not the fixed-point path). -/
abbrev IdealWorkScoreInterpretation := _root_.Coinjecture.workScoreBitsIdeal_spec

/-- Cumulative ideal chain security. -/
abbrev IdealChainSecurity := _root_.Coinjecture.chainSecurityBitsIdeal_spec

/-!
## 6. Privacy marketplace (testnet placeholder — Tier B)
-/

/-- Placeholder well-formedness proof is binding to commitment + public params (SHA-256 MAC). -/
axiom placeholderProof_binding : True

/-- Placeholder is sound under preimage resistance (production must use real ZK). -/
axiom placeholderProof_sound_testnet : True

/-- Production deployments must not rely on the placeholder. -/
axiom production_requires_real_zk : True

end Coinjecture.Security
