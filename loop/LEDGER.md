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

**Lean 89 — re-measured and CONFIRMED (P-000-A, Cycle 1).** After the 936 truncation defect put every
count from that session in doubt, the Lean figure was re-taken under three independent, untruncated
commands (recorded verbatim in `LOOP_SPEC.md` §2). All three return 89. A fourth, naive
line-start formulation returns 88 — the one-count sensitivity is recorded because it is the evidence
that the agreement is meaningful, not an inconsistency. **89 is no longer an inherited number.**
The 586 figure stays unreconciled; `proofs/eigenverse` is a registered but **unpopulated** submodule,
which makes the "586 is Eigenverse's count" hypothesis concrete and cheap to test.

---

## Standing notes — not findings, not merged work

Recorded here because they are facts about the *state of the work* that no code review would surface
and no `git log` on `main` would show.

### SEC-PR-001 exists on one disk and nowhere else ⚠️

`fix/api-jwt-fail-closed` @ `ff6e65c4` — "fix(api): fail closed on invalid JWT secret", authored
2026-07-17, **4 files, +318 / −18** across `api-server/src/config.rs`, `api-server/src/jwt.rs`,
`api-server/src/main.rs`, `api-server/tests/config_startup.rs`.

**It has never been pushed to any remote.** Verified in Phase −1: `git branch -r --contains ff6e65c4`
returns empty, and the branch has no upstream. It exists solely in the working clone at
`C:\Users\LEET\COINjecture2.0-network`.

**This is a single point of failure for reviewed security work.** A disk failure, a clone deletion, or
an accidental `git branch -D` loses it with no recovery path. The work is not lost-*yet* — it is
one ordinary accident away from being lost, and nothing in the current setup would notice.

**Not a code finding. Not assigned to a packet.** Pushing it is **Al's call** — it is a security
change to an authentication path and publishing it decides review timing and disclosure posture, so
the builder seat does not make it. Recorded so that the risk is a decision someone made rather than a
thing that quietly stayed true.

*(This is separate from, though adjacent to, the observation in `REGISTRY.md` that SEC-PR-001 is not
an ancestor of `main`. That note describes remediation coverage; this one describes durability.)*
