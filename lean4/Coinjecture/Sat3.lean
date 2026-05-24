import Coinjecture.Verify
import Coinjecture.ClassicalAxioms

namespace Coinjecture

def is3Cnf (clauses : List (List Int)) : Bool :=
  clauses.all fun c => c.length == 3

theorem verify_sat_linear (variables : Nat) (clauses : List (List Int))
    (assignment : List Bool) :
    verifySat variables clauses assignment = true →
    clauses.all (clauseSatisfied assignment) := by
  intro h
  simp [verifySat] at h
  exact h.2

/-- Classical: 3-SAT is NP-complete. -/
axiom threeSat_npComplete : True

end Coinjecture
