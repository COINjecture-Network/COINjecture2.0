#!/usr/bin/env bash
# Tier C gate: Lean 4 specs + Rust fixture alignment.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
echo ">>> Lean 4 (rewards, work score, axioms, fixtures)"
(cd lean4 && lake build)
echo ">>> Rust fixture tests (must match lean4/Coinjecture/Fixtures.lean)"
cargo test -p coinject-consensus lean_fixture -- --nocapture
echo ">>> OK: formal fixtures aligned"
