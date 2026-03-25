#!/usr/bin/env bash
# =============================================================================
# COINjecture — Phase 8 Test Runner
# .systemx/scripts/test/run_tests.sh
# =============================================================================
# Runs the full workspace test suite and optionally measures coverage.
# Usage:
#   ./run_tests.sh                  # all tests, default output
#   ./run_tests.sh --coverage       # add cargo-tarpaulin coverage report
#   ./run_tests.sh --crate core     # test a single crate only
#   RUST_LOG=debug ./run_tests.sh   # verbose logging during tests
#
# Requirements:
#   - Rust toolchain (cargo)
#   - cargo-tarpaulin (optional, for --coverage)

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$REPO_ROOT"

COVERAGE=false
CRATE=""

# Parse flags
while [[ $# -gt 0 ]]; do
  case "$1" in
    --coverage) COVERAGE=true; shift ;;
    --crate)    CRATE="$2"; shift 2 ;;
    *) echo "Unknown flag: $1" >&2; exit 1 ;;
  esac
done

echo "============================================================"
echo "  COINjecture Unit Test Suite"
echo "  Repo: $REPO_ROOT"
echo "============================================================"
echo ""

# ── Run tests ────────────────────────────────────────────────────────────────

if [[ -n "$CRATE" ]]; then
  echo "▶ Running tests for crate: $CRATE"
  cargo test -p "$CRATE" -- --nocapture
else
  echo "▶ Running all workspace tests..."
  cargo test --workspace -- --nocapture
fi

echo ""
echo "✅ All tests passed."
echo ""

# ── Optional coverage ────────────────────────────────────────────────────────

if [[ "$COVERAGE" == true ]]; then
  echo "▶ Running cargo-tarpaulin for coverage..."

  if ! command -v cargo-tarpaulin &>/dev/null; then
    echo "⚠  cargo-tarpaulin not found. Install with:"
    echo "   cargo install cargo-tarpaulin"
    exit 1
  fi

  cargo tarpaulin \
    --workspace \
    --exclude coinject-node \
    --exclude coinject-wallet \
    --exclude coinject-mobile-sdk \
    --timeout 180 \
    --out Html \
    --output-dir .systemx/coverage \
    -- --test-threads 1

  echo ""
  echo "📊 Coverage report: .systemx/coverage/tarpaulin-report.html"
fi

echo ""
echo "============================================================"
echo "  Done."
echo "============================================================"
