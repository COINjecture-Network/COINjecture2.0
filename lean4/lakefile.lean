import Lake
open Lake DSL

require mathlib from git
  "https://github.com/leanprover-community/mathlib4.git" @ "v4.28.0"

package «coinjecture-formal» where
  version := v!"0.1.0"

lean_lib Coinjecture where
  roots := #[`Coinjecture]
