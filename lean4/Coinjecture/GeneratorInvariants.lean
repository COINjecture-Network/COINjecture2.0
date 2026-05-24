import Coinjecture.Verify

namespace Coinjecture

/--
Miner-generated SubsetSum: `target` equals sum of a non-empty proper subset of `numbers`.
(Mirrors `consensus/src/miner.rs` generation.)
-/
structure SubsetSumInstance where
  numbers : List Int
  target : Int
  witnessIndices : List Nat

def subsetSumInstanceValid (p : SubsetSumInstance) : Bool :=
  verifySubsetSum p.numbers p.target p.witnessIndices

/--
Miner-generated 3-SAT: hidden assignment satisfies all clauses (70% biased literals).
-/
structure Sat3Instance where
  variables : Nat
  clauses : List (List Int)
  witnessAssignment : List Bool

def sat3InstanceValid (p : Sat3Instance) : Bool :=
  verifySat p.variables p.clauses p.witnessAssignment

end Coinjecture
