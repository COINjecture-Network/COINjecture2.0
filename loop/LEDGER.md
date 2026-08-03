# LEDGER

Append-only record of work actually **merged**. One row per merged packet.
Authority: `loop/LOOP_SPEC.md` §6 (Phase D writes the entry).

| Date | Cycle | Packet | RC / DARQ | Findings closed | PR | Merge SHA | Tests before → after | Adversary |
|---|---|---|---|---|---|---|---|---|
| — | — | — | — | — | — | — | — | — |

**No entries yet.** Cycle 0 (P-001) was read-only. Cycle 1 (P-000) is in draft PR, not merged.

Per §1, the Adversary column records `CLEAN` / `MERGE-WITH-NOTES` / `BLOCK` for P-003, P-004 and
P-005, and `SKIPPED — <one-line reason>` everywhere else.

## Baseline for future deltas

Measured at `28c50a122f2caab70582e8215b670b0ddc4d236d`:

| Metric | Value |
|---|---|
| Tests passing | **982** (+4 ignored, 0 failed, 49 test binaries) — under pinned 1.97.1 |
| Lean theorems | 89 (`theorem`/`lemma` decls across 19 `.lean` files) |
| Workspace crates | 15 |
| `cargo audit` | 2 vulnerabilities, 3 allowed warnings |
| `Cargo.lock` SHA-256 | `9930a209663dd812d03dd654d5ea8f850152667de455191b7c4645eb1cdb1bea` |

⚠️ **The 936 figure previously recorded here was wrong** — a Cycle 0 `| head -60` truncation, not a
measurement. Corrected to 982 in Cycle 1. See `LOOP_SPEC.md` §2 and `reports/C1-hotfix-builder.md` §5.

The Lean gap (89 vs 586) remains **unreconciled**, not corrected. Per §2 the repo at this SHA is
ground truth; do not treat either as a regression signal without reading §2 first.

Clippy and `cargo fmt` are **clean** under the pinned 1.97.1 toolchain as of P-002-H.
`cargo geiger` is still unmeasured — that is P-002, and per D4 no gate may be set until it is.
