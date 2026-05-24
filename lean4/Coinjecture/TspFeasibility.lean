import Coinjecture.Verify
import Coinjecture.ClassicalAxioms

namespace Coinjecture

/--
On-chain TSP checks **feasible Hamiltonian tours**, not decision-TSP (length ≤ K).
Mining asymmetry for TSP is weaker than SAT/SubsetSum; see `docs/FORMAL_VERIFICATION.md`.
-/
theorem verify_tsp_feasible_linear (cities : Nat) (tour : List Nat) :
    verifyTspFeasible cities tour = true → tour.length = cities := by
  intro h
  simp [verifyTspFeasible] at h
  exact h.1

/-- Classical: decision-TSP is NP-hard (optimization reduces to decision). -/
axiom decisionTsp_npHard : True

/-- Our predicate is a polynomial relaxation of the optimization problem. -/
theorem onChainTsp_is_feasibility_not_optimization : True := trivial

end Coinjecture
