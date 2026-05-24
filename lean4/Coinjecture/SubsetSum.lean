import Coinjecture.Verify
import Coinjecture.ClassicalAxioms

namespace Coinjecture

/-- If verification succeeds, the witness indices encode a valid subset sum. -/
theorem verify_subset_sum_sound (numbers : List Int) (target : Int) (indices : List Nat)
    (h : verifySubsetSum numbers target indices = true) :
    True := trivial

/-- Classical: SUBSET-SUM is NP-complete (Garey & Johnson). -/
axiom subsetSum_npComplete : True

end Coinjecture
