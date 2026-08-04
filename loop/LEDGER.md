# LEDGER

Append-only record of work actually **merged**. One row per merged packet.
Authority: `loop/LOOP_SPEC.md` §6 (Phase D writes the entry).

| Date | Cycle | Packet | RC / DARQ | Findings closed | PR | Merge SHA | Tests before → after | Adversary |
|---|---|---|---|---|---|---|---|---|
| 2026-08-03 | 1 | **P-002-H** | DARQ-020 (NEW-4) | **1** — DARQ-020 closed | [#55](https://github.com/COINjecture-Network/COINjecture2.0/pull/55) | `b1aaf59b` | 982 → 982 / 0 failed / 4 ignored | `SKIPPED — CI-config hotfix; both src changes are provably syntax-only, no behaviour to adversarially verify` |
| 2026-08-04 | 1 | **P-000 + P-000-A + P-000-B** | — (process) · registers DARQ-023, DARQ-024 | 0 — process packet | [#54](https://github.com/COINjecture-Network/COINjecture2.0/pull/54) | `28007c36` | 982 → 982 / 0 failed / 4 ignored | `SKIPPED — docs-only; no behaviour to adversarially verify` |

Cycle 0 (P-001) was read-only, so it has no row.

Per §1, the Adversary column records `CLEAN` / `MERGE-WITH-NOTES` / `BLOCK` for P-003, P-004 and
P-005, and `SKIPPED — <one-line reason>` everywhere else.

### P-002-H — merged under the amended D12 bounded exception ⚠️ deferred red

**This is the first application of the amended D12, and D12 requires the deferred red be logged
against its owning packet. This section is that log. It is not a formality — it is the mechanism that
keeps "tracked to a named packet" a fact rather than a phrase.**

| | |
|---|---|
| Merged | 2026-08-03T20:26:32Z, merge commit `b1aaf59b`, head `26ff50a8` |
| Green | Lint · Test (default) · Test (adzdb) · Build (release) ×2 · Docker Build · Analyze ×4 (actions, js-ts, python, rust) · CodeQL |
| **Red at merge** | **`Security Audit` — merged anyway, deliberately** |

**The deferred red, itemised and owned:**

| Advisory | Crate | Severity | Fix available | Owner |
|---|---|---|---|---|
| [RUSTSEC-2026-0185](https://rustsec.org/advisories/RUSTSEC-2026-0185) | `quinn-proto` 0.11.14 | **7.5 High** — remote memory exhaustion via unbounded out-of-order stream reassembly | ≥ 0.11.15 | **P-002** |
| [RUSTSEC-2026-0204](https://rustsec.org/advisories/RUSTSEC-2026-0204) | `crossbeam-epoch` 0.9.18 | Invalid pointer deref in `fmt::Pointer` for `Atomic`/`Shared` | ≥ 0.9.20 | **P-002** |

Plus 3 *allowed* warnings that do not fail the job and are not deferred red: `bincode` 1.3.3
unmaintained (RUSTSEC-2025-0141), `anyhow` 1.0.102 unsound (RUSTSEC-2026-0190), `spin` 0.9.8 yanked.

**D12's three conditions, checked individually rather than asserted:**

1. **Correctly attributed** — ✅ Not introduced by P-002-H. The opposite: the `Security Audit` job had
   not executed on *any* PR for 51 days because `needs: lint` gated it behind a red Lint. P-002-H is
   what made it run. **These advisories were already true and already invisible.**
2. **Tracked to a named packet** — ✅ P-002, scoped for exactly this before P-002-H existed. Recorded
   here with advisory IDs, affected versions and fix versions so the successor packet inherits facts,
   not a pointer.
3. **Outside the current packet's scope** — ✅ P-002-H was scoped to `rust-toolchain.toml`, `ci.yml`,
   and two syntax-only lint fixes. Resolving these requires dependency bumps, lockfile churn, and a
   `quinn-proto` review that touches the QUIC layer of a P2P chain. Folding that into a hotfix would
   have broken D1 and D8 both.

**Why this was the right call and not merely a permitted one:** holding #55 would have kept the
security gate dark in order to avoid seeing what the security gate reports. The pipeline was
*already* failing these advisories — silently, for 51 days. #55 changed nothing about the repo's
security posture except making it **legible**. A job reporting a true failure beats a job reporting
nothing.

**Standing obligation created by this entry:** `main` is now red on `Security Audit` and will stay
red until P-002 lands. That red is **expected and accounted for** — it is not a new signal and must
not be re-diagnosed as one. **But it also must not become wallpaper.** If P-002 slips, a *third*
advisory could appear and be mistaken for the two already logged here. P-002 must therefore
re-enumerate `cargo audit` output from scratch rather than assume this list is still complete.

> ⚠️ **VINDICATED, 2026-08-04 — and faster than expected.** That obligation was written as a
> precaution. It was already necessary when written. P-023's inventory found a **third** vulnerable
> Rust crate, `rand` 0.8.5 (GHSA-cq8v-f236-94qc / RUSTSEC-2026-0097), which `cargo audit` does **not**
> report at all. The list above was incomplete on the day it was recorded.
>
> **Do not rewrite the list above.** It is accurate as what it claims to be — a record of what the
> `Security Audit` job reported. This annotation is the correction. See `LOOP_SPEC.md` §5 "P-002 must
> UNION both scanners" and §8 for the `rand` sizing (it is a soundness bug, not an RNG defect; no keys
> implicated).

---

### P-000 + P-000-A + P-000-B — merged under the amended D12 bounded exception ⚠️ deferred red

**Second application of amended D12.** Merged 2026-08-04T01:26:31Z as `28007c36` — a true merge
commit (parents `b1aaf59b` + `7eac7915`), not a squash.

| | |
|---|---|
| Green at merge | Lint · Test (default) · Test (adzdb) · Build (release) ×2 · Docker Build · Analyze ×4 · CodeQL |
| **Red at merge** | **`Security Audit`** — the same two advisories, unchanged |

Three packets rode in this one PR, all docs-only under `loop/`:

- **P-000** — the loop scaffolding and `LOOP_SPEC.md`. **This is what ends the re-pasting problem:**
  the spec is now on `main` and future prompts reference it by path.
- **P-000-A** — spec v1.3: D12 amended to the bounded-exception form, GATE-1 halved, §2 measurement
  discipline, P-021-V / P-021 / P-002-H2 registered, DARQ-022 registered, P-002 sharpened.
- **P-000-B** — spec v1.4: P-002 amended to union both scanners, DARQ-023 and DARQ-024 registered,
  the `rand` advisory sized, the P-023-before-P-022 ordering rule and Al's grouping principle recorded.

**D12's three conditions, checked individually rather than asserted:**

1. **Correctly attributed** — ✅ Docs-only under `loop/`; it changed no `Cargo.toml`, no `Cargo.lock`,
   and no `.rs` file, so it cannot affect `cargo audit`. It inherited this red from `main` at the
   moment it rebased onto `b1aaf59b`. **A cleaner case than #55's:** #55 at least touched CI config,
   whereas this PR touches no code at all.
2. **Tracked to a named packet** — ✅ P-002, itemised in the #55 entry above with advisory IDs and fix
   versions — **now with the `rand` annotation added, because that list proved incomplete.**
3. **Outside the current packet's scope** — ✅ a documentation packet cannot bump `quinn-proto`.

**Verification performed before merge, not assumed:** the rebase onto `b1aaf59b` replayed 7/7 commits
with zero conflicts, and `loop/` was confirmed **byte-identical** pre- and post-rebase by tree diff
rather than by inspection. Six of the jobs listed green above had **never executed on this PR before**
— they were all `skipping` behind a red Lint until #55 landed the toolchain pin. The hosted suite
returned **1964 / 0 / 8**, exactly 2× the 982 / 0 / 4 baseline (tarpaulin re-runs the suite), summed
from all 85 `test result:` lines with no truncation per §2.

**Consequence:** `main` remains red on `Security Audit` until P-002 lands. Expected, logged, owned.

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
