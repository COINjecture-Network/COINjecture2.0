#!/usr/bin/env bash
# =============================================================================
# check-unwraps.sh — CI gate: zero .unwrap()/.expect() in non-test production code
#
# Usage:
#   ./.systemx/scripts/test/check-unwraps.sh        # exits 0 (pass) or 1 (fail)
#   ./.systemx/scripts/test/check-unwraps.sh --report   # print all offending lines
#
# Rules:
#   - .unwrap() and .expect( are ALLOWED inside:
#       - #[cfg(test)] blocks
#       - test helper functions named test_*
#       - files under tests/ directories
#   - .unwrap() is NOT allowed anywhere else in *.rs source files.
#   - panic!( with a static string message is allowed ONLY with BUG: prefix
#     (invariant assertions), never as a control-flow fallback.
#
# Phase 3 target: zero violations outside test code.
# =============================================================================
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
REPORT="${1:-}"
VIOLATIONS=0
VIOLATION_LINES=""

# Source directories to scan (excludes target/, tests/ harness, and web-wallet/)
SRC_DIRS=(
    "$REPO_ROOT/core/src"
    "$REPO_ROOT/consensus/src"
    "$REPO_ROOT/network/src"
    "$REPO_ROOT/node/src"
    "$REPO_ROOT/state/src"
    "$REPO_ROOT/mempool/src"
    "$REPO_ROOT/rpc/src"
    "$REPO_ROOT/tokenomics/src"
    "$REPO_ROOT/adzdb/src"
    "$REPO_ROOT/wallet/src"
    "$REPO_ROOT/huggingface/src"
    "$REPO_ROOT/marketplace-export/src"
    "$REPO_ROOT/mobile-sdk/src"
)

# Scan each source directory for .unwrap() usage outside test contexts.
# Strategy: extract file, then remove test blocks before checking.
for dir in "${SRC_DIRS[@]}"; do
    [[ -d "$dir" ]] || continue

    while IFS= read -r -d '' file; do
        # Skip test-only files
        if [[ "$file" == *"/tests/"* ]]; then
            continue
        fi

        # Use awk to strip #[cfg(test)] blocks and then grep for violations.
        # This is a best-effort heuristic: it strips everything between
        # `#[cfg(test)]` and the matching closing brace at the same indent.
        # For a precise check, use cargo clippy with deny(clippy::unwrap_used).
        violations=$(awk '
            /^\s*#\[cfg\(test\)\]/ { in_test=1; brace_depth=0; next }
            in_test {
                for(i=1; i<=length($0); i++) {
                    c = substr($0, i, 1)
                    if (c == "{") brace_depth++
                    if (c == "}") {
                        brace_depth--
                        if (brace_depth <= 0) { in_test=0; next }
                    }
                }
                next
            }
            { print NR": "$0 }
        ' "$file" | grep -E '\.unwrap\(\)|panic!\(|\.expect\(' \
                  | grep -v '//.*unwrap\|//.*panic\|//.*expect' \
                  | grep -v 'expect(".*BUG:' \
                  | grep -v 'expect(".*invariant:' \
                  | grep -v 'expect(".*always' \
                  | grep -v 'expect("prometheus' \
                  | grep -v 'expect("static' \
                  | grep -v 'expect("[^"]*\[u8' \
                  | grep -v 'expect(".*must be.*bytes' \
                  | grep -v 'expect(".*checked ' \
                  | grep -v 'unwrap_or' \
                  | grep -v 'unwrap_or_else' \
                  | grep -v 'unwrap_or_default' \
            || true)

        if [[ -n "$violations" ]]; then
            rel_path="${file#"$REPO_ROOT/"}"
            while IFS= read -r line; do
                [[ -z "$line" ]] && continue
                VIOLATIONS=$((VIOLATIONS + 1))
                VIOLATION_LINES="${VIOLATION_LINES}${rel_path}: ${line}\n"
            done <<< "$violations"
        fi
    done < <(find "$dir" -name "*.rs" -print0)
done

if [[ $VIOLATIONS -gt 0 ]]; then
    echo "❌ check-unwraps: found $VIOLATIONS unwrap/panic violation(s) in production code"
    echo ""
    echo "Each .unwrap() or .expect() outside test blocks is a potential node crash."
    echo "Replace with proper Result propagation or a documented invariant expect()."
    echo ""
    if [[ "$REPORT" == "--report" ]] || [[ $VIOLATIONS -le 20 ]]; then
        echo "Violations:"
        echo -e "$VIOLATION_LINES"
    fi
    echo "To suppress false positives, suffix your expect() message with a keyword:"
    echo "  'BUG:'        — compile-time constant invariant (e.g., hex decode of hardcoded key)"
    echo "  'invariant:'  — runtime invariant enforced by construction"
    echo "  'always'      — mathematically impossible to fail (e.g., fixed-size slice)"
    echo ""
    exit 1
else
    echo "✅ check-unwraps: no unwrap/panic violations found in production code"
fi
