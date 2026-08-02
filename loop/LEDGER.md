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
| Tests passing | 936 (+4 ignored, 0 failed, 30 test binaries) |
| Lean theorems | 89 (`theorem`/`lemma` decls across 19 `.lean` files) |
| Workspace crates | 15 |
| `cargo audit` | 2 vulnerabilities, 3 allowed warnings |
| `Cargo.lock` SHA-256 | `9930a209663dd812d03dd654d5ea8f850152667de455191b7c4645eb1cdb1bea` |

Two of these disagree with numbers previously carried in memory (951 tests, 586 Lean theorems).
Per §2 the repo at this SHA is ground truth; the Lean gap is recorded as **unreconciled**, not
corrected. Do not treat either as a regression signal without reading §2 first.

Clippy, `cargo fmt` and `cargo geiger` baselines are **not yet measured** — that is STEP 2 of P-002,
and per D4 no CI gate may be set until they are.
