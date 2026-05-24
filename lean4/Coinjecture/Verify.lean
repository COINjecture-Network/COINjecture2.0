/-!
Executable verification specs aligned with `core/src/problem.rs` `Solution::verify`.
Tier A: polynomial-time checkers (no `sorry`).
-/

namespace Coinjecture

/-- Subset sum: indices select numbers summing to target. O(k) in |indices|. -/
def verifySubsetSum (numbers : List Int) (target : Int) (indices : List Nat) : Bool :=
  let sum := indices.foldl (fun acc i =>
    match numbers[i]? with
    | some v => acc + v
    | none => acc) 0
  sum == target

/-- 3-SAT clause: list of literals (positive = var true, negative = negated). -/
def clauseSatisfied (assignment : List Bool) (literals : List Int) : Bool :=
  literals.any fun lit =>
    let idx := Int.natAbs lit - 1
    match assignment[idx]? with
    | some v => (lit > 0) == v
    | none => false

def verifySat (variables : Nat) (clauses : List (List Int)) (assignment : List Bool) : Bool :=
  assignment.length == variables &&
  clauses.all (clauseSatisfied assignment)

/-- TSP: Hamiltonian tour (visits each city once). Not optimality. O(n). -/
def verifyTspFeasible (cities : Nat) (tour : List Nat) : Bool :=
  tour.length == cities &&
  (List.range cities).all fun c => tour.count c == 1

end Coinjecture
