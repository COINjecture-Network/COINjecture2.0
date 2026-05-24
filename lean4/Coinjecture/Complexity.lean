/-!
Dimensionless complexity bookkeeping for PoUW (not a full Mathlib formalization).
-/

namespace Coinjecture

/-- Problem is checkable in time polynomial in input length (witness verification). -/
structure PolyVerifiable (α : Type) where
  verify : α → List Bool → Bool
  /-- Spec: verification visits each witness element at most once (linear scan). -/
  linearWitness : True

/-- NP decision predicate (existential witness). -/
structure NpDecision (α : Type) extends PolyVerifiable α where
  witnessOk : α → List Bool → Bool := verify

end Coinjecture
