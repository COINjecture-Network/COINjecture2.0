# DARQ AGI Mode — COINjecture 2.0 Remediation Loop

**Spec v1.4** · Supersedes v1.3 · Target: `COINjecture-Network/COINjecture2.0`
Baseline anchored at `28c50a12` · **Commit over `loop/LOOP_SPEC.md`**

---

## §0.2 Changelog — v1.3 → v1.4

Landed as **P-000-B**, docs-only on `feat/p000-loop-scaffolding`, from findings made while building
P-023.

| v1.3 said | Reality (Cycle 2, P-023) | v1.4 |
|---|---|---|
| The Dependabot/`cargo audit` gap is **inter**-ecosystem: cargo audit sees Rust, the gap is JS+Python | **There is an intra-Rust gap too.** `quinn-proto` seen by both; `crossbeam-epoch` by cargo audit only; `rand` by Dependabot only. Neither scanner is a superset. | **P-002 must UNION both scanners.** Mechanism of the omission is unexplained and flagged as such |
| Dependency alerts span "all ecosystems" | **npm had no `dependabot.yml` entry at all** while holding 21 of 23 alerts. Security updates ran anyway, so it looked covered. | **DARQ-023** registered; closed by **P-023** (PR #56) |
| CodeQL scans python, so python is watched | CodeQL scans python **code**. There is **no python dependency manifest anywhere**, so nothing watches python **dependencies**. | **DARQ-024** registered — not closable by config |
| — | `rand` sits near key generation, so its advisory needed sizing before anyone panicked | **GHSA-cq8v-f236-94qc analysed in full (§8): a soundness bug, NOT an RNG defect. No keys implicated.** |

**Ordering rule established:** **P-023 lands before P-022.** `dependabot.yml` is read from the default
branch at run time, so configuring first means the first successful run after the unblock is already
governed. Unblock first and the backlog arrives ungoverned. *(Same shape as provisioning a
destination before flipping a switch.)*

**Grouping principle established** (ruled by Al, recorded in `dependabot.yml`): **group where items
are individually unremarkable; separate where each deserves its own merge decision.** Grouping trades
review *volume* for review *granularity*. Right for a wave of npm transitive bumps; wrong where one PR
would couple unrelated subsystems. Applied: npm grouped, **cargo security updates ungrouped**.

---

## §0.1 Changelog — v1.2 → v1.3

Landed as **P-000-A**, a docs-only amendment packet on `feat/p000-loop-scaffolding`, authorized by
Al after the Phase −1 state detection that followed a mid-session network fault.

| v1.2 said | Reality (Cycle 1, Phase D + Phase −1) | v1.3 |
|---|---|---|
| **D12** — green before merge, *no exceptions* | D12 as written forbids the merge Al ruled for. PR #55 is green everywhere except a Security Audit failure it did not introduce and cannot fix in scope. Holding it would keep the security gate dark to preserve a rule whose purpose is a live security gate. | **D12 rewritten to the bounded-exception form**, with both precedents recorded |
| **GATE-1** — stranded genesis balances *and* live testnet state | `node/src/genesis.rs:47` sets `initial_supply: 0`. **There are no genesis allocations** — verified at HEAD, no allocation/premine/balance-insert code exists in that file. | **GATE-1 halved.** Only the live-state question survives |
| Lean count 89 **unreconciled**, 586 untested hypothesis | 89 **re-measured untruncated and confirmed** under three independent commands. Both submodules are unpopulated, so 89 counts only the 19 tracked in-repo `.lean` files. | §2 records the confirmed count *and the exact command*; 586 stays unreconciled but is now a sharper question |
| Dependency alerts sit open, cause unexamined | **Dependabot has not run successfully on `main` since at least 2026-07-13.** Root cause found: a dangling submodule pin aborts the clone before any ecosystem logic runs. Zero Dependabot PRs exist. | **DARQ-022** registered, assigned to P-002 |
| Three workflows still float (noted in passing) | An unpinned `release.yml` means a release can be cut on a compiler CI never tested. | **P-002-H2** registered |
| DARQ-021 unsized, unassigned | It is the highest-value open question in the queue: it decides whether an arbitrary-theft path exists. | **P-021-V** (verification) and **P-021** (fix, GATE-2) registered |

**The through-line of this revision:** three separate mechanisms were found silently disabling
security machinery — a floating toolchain darkening the audit job (DARQ-020), a dangling submodule
darkening the dependency updater (DARQ-022), and a validator calling the weaker of two available
checks (DARQ-021). None announced itself. Each was found while looking for something else. **Treat
"the gate is present" and "the gate is running" as different claims requiring separate evidence.**

---

## §0 Changelog — v1.1 → v1.2

| v1.1 said | Reality (Cycle 1 discovery) | v1.2 |
|---|---|---|
| P-002 builds a CI security pipeline | **CodeQL already runs** (GitHub default setup — actions, javascript-typescript, python, rust; no workflow file). **A Security Audit job already exists** in `ci.yml`. | P-002 **amends**. Adding CodeQL would be pure duplication. |
| ~14 Dependabot alerts vs 2 cargo-audit findings | **Four numbers now**: 18 (API) / 19 (push banner) / ~14 (stale spec) / 2 (cargo audit) | §9 rewritten — and the gap is **not** a counting error, see below |
| CI is a working pipeline to add gates to | **CI is dark.** Lint red on main; `test`, `build`, `security` all `needs: lint`. No test signal and no audit signal on any PR. | **P-002-H** preempts the queue |
| The floating toolchain is a nuisance | It is the **root cause**. `RUST_TOOLCHAIN: stable` means any new clippy lint reds the pipeline — and therefore darkens the security gate — with zero code changes. | **D11** added |

**The dependency insight that changes P-002's meaning:** CodeQL scans
*javascript-typescript and python*. COINjecture is not a pure-Rust repo. `cargo audit` sees only the
Rust graph — its 2 findings are Rust/RustSec. The remaining ~16 alerts are almost certainly JS and
Python dependencies that `cargo audit` is structurally blind to. **The dependency blind spot is
multi-ecosystem, not a Rust gap.** "Fixed the 2 Rust advisories" must never be recorded as
"dependencies clean."

---

## §1 Seat Configuration

| Seat | Who | Role |
|---|---|---|
| **BUILDER** (empirical) | Claude Code | Only seat with repo access. Ground truth for the others. |
| **ADVERSARY** | Pre-loaded (Codex + third-party), re-invoked on demand | **Fix-verification only** — pointed at a patch, never the codebase. |
| **SYNTHESIS** | Claude / Opus | Arbitrates, rules on flags, holds gates, writes go/no-go. |

Adversary pass **mandatory** for P-003, P-004, P-005. **Skipped** elsewhere — logged in the LEDGER
as a deliberate deviation with a one-line reason.

---

## §2 Ground Truth — anchored to `28c50a12`

**The repo at this SHA is ground truth. Numbers carried in memory are not.**

- Clone: `COINjecture2.0-network` (org remote). HEAD and `Cargo.lock` SHA-256 match the Codex scan
  baseline exactly — zero drift.
- **982** tests passing / 0 failed / 4 ignored (49 test binaries) · **89** Lean theorems / 19 files ·
  **15** crates
- ✅ **Lean 89 re-measured and CONFIRMED in Cycle 1 (P-000-A)** — see "Measurement discipline" below.
  The 586 figure remains **unreconciled**, not corrected. Hypothesis, now mechanically concrete:
  `.gitmodules` registers `proofs/eigenverse` → `beanapologist/Eigenverse`, and that submodule is
  **unpopulated** in this worktree. 89 therefore counts only the repo's own tracked `.lean` files. A
  count taken with that submodule checked out would include Eigenverse's theorems. **That is a
  testable next step, not a conclusion — do not record 586 as explained until someone runs it.**
- ⚠️ **CORRECTED in Cycle 1 — the 936 figure carried by v1.0/v1.1 was wrong.** It was a builder
  measurement error, not a code change: the Cycle 0 baseline command ended in `| head -60`, which
  truncated the stream at 60 matching lines, and the total was summed from the 30 `test result:`
  lines that survived. The captured output is exactly 61 lines and stops mid-suite. **982 / 0 / 4 is
  the first correctly measured total**, taken under the pinned toolchain (P-002-H, run 30842995637).
  Note 982 *exceeds* the original 951 claim rather than falling short of it, so the Cycle 0 report's
  "−15 tests" discrepancy was an artifact of the same truncation and should be disregarded.
  The true total under the *old* 1.91 toolchain was never established, so the 936→982 delta cannot
  be attributed to the toolchain change — the evidence points entirely at the truncation.

### Measurement discipline *(New — v1.3)*

**Any count derived from a piped, `head`ed, or `tail`ed stream is suspect by default.** The 936-test
figure was not a wrong measurement, it was a *truncated* one — `| head -60` cut the stream and the
total was summed from the 30 `test result:` lines that survived. It then propagated through three
spec revisions as if it were data, and generated a phantom "−15 tests" regression signal that cost
real analysis time. A truncated count does not announce itself; it looks exactly like a count.

Rules:

1. **State the exact command next to any number you record.** A number without its command is a
   rumour. Every figure in this section carries its command below.
2. **Never `head`/`tail` a stream you are about to sum.** Redirect to a file and count the file, or
   use a counter that cannot truncate.
3. **A count that changes when the regex changes is not yet a measurement.** Run at least two
   independent formulations and reconcile them *before* recording, not after.
4. **Re-measure inherited numbers before trusting them**, even ones this spec previously asserted.
   §2 says the repo at the SHA is ground truth; that binds this document too.

**Baseline: 982 passed / 0 failed / 4 ignored**, 49 test binaries, under pinned 1.97.1. Hosted
reports 1964 / 0 / 8 — exactly 2×, because tarpaulin re-runs the suite. **That doubling is expected;
do not file it as a discrepancy.**

**Lean: 89 theorems across 19 tracked `.lean` files — CONFIRMED (P-000-A).** Re-measured after the
936 defect put every same-session count in doubt. Three independent formulations agree:

```bash
git ls-files '*.lean' | xargs grep -hcE '\b(theorem|lemma)\b'          | awk '{s+=$1} END {print s}'   # 89
git ls-files '*.lean' | xargs grep -ohE '\b(theorem|lemma)\b' | wc -l                                  # 89
git ls-files '*.lean' | xargs grep -hcE '^[[:space:]]*(private |protected |nonrec |@\[[^]]*\] *)*(theorem|lemma)\b' | awk '{s+=$1} END {print s}'   # 89
```

A fourth, naive formulation — line-start with no prefix allowance — returns **88**, undercounting a
single declaration that carries a modifier or attribute prefix. **That one-count sensitivity is
exactly what rule 3 exists to catch**, and it is why 89 is recorded as confirmed rather than
assumed: the agreement of three formulations is the evidence, not any single run.

**⚠️ Clone hazard — now a supply-chain dependency, not just a workspace nuisance.** A second clone at
`C:\Users\LEET\COINjecture2.0` (Quigles1337 remote) is also v4.8.4 / 15 crates but is **dirty with 20
live worktrees and has no `lean4/`**. Beyond the local confusion, **the org repo depends on that fork
in-tree**: `.gitmodules` registers `latest-upstream` → `Quigles1337/COINjecture2.0`, pinned at
`6a32fbfc7094fe82c02a91b231b52798c9f42972`, **a commit that no longer exists in that remote**. That
dangling pin is the root cause of DARQ-022. Renaming or deleting the local clone does not fix it —
the submodule entry is committed to `main` and must be resolved in the repo.

**Remediation state on `main`: zero.** SEC-PR-001 exists as branch `ff6e65c4`, not an ancestor of
main. SEC-PR-002…005 are unimplemented specs in `first-five-prs.md`.

**Audit reliability, calibrated.** Both audits' *findings* have held; their *coordinates* have not.
The third-party report cites `core/src/crypto.rs:423` in a 397-line file and missed a third address
derivation. **Every uncited specific in either report is a hypothesis.**

---

## §3 Guardrails

**D1 — Approved packets only.** One packet, one concern, one branch, one cycle.

**D2 — Integer money only.** Any `f64` near a balance, fee, or supply figure is a Critical you
created. *(`work_score` is `f64` today — see GATE-3.)*

**D3 — Checked arithmetic on money and nonce paths.** `checked_add` / `checked_sub` returning
errors. **Never `saturating_*` on balances** — saturation silently destroys supply.

**D4 — Never set a CI gate below the measured baseline.** Inventory first, gate at the measured
value, tighten as its own PR.

**D5 — Consensus-affecting changes are hard forks.** Gated (§4). Anything touching `validator.rs`,
fork choice, or block validation is consensus code even when the change looks cosmetic.

**D6 — No deployment.** Box C access unresolved; parked as a known gap. Repo work only.

**D7 — One root cause per packet.** Report "N findings / M root causes."

**D8 — Small reviewable changes**, green against the current verified baseline.

**D9 — Windows-authored scripts need the exec bit set in the git index** (`git update-index
--chmod=+x`) **and must be invoked through `bash`.** Al develops on Windows; CI runs Linux.

**D10 — PRs, not direct-to-main.** COINjecture is shared with Sarah and has an established PR
workflow.

**D11 — Pin the toolchain. No floating `stable`.** *(New — Cycle 1.)* A floating toolchain means
every upstream lint release can red the pipeline with zero code changes, and because `security`
`needs: lint`, that **silently disables the security gate**. There must be exactly one source of
truth for the toolchain version — `rust-toolchain.toml` and any CI env var must not disagree.

**D12 — Green before merge, with one bounded exception.** *(Amended v1.3, ruled by Al. Supersedes
the "no exceptions" form of v1.2.)*

No PR merges with a failure **it introduced**, or with a failure **it could have fixed within its own
legitimate scope**. A pre-existing failure that is (a) **correctly attributed**, (b) **tracked to a
named packet**, and (c) **outside the current packet's scope** does not block a PR that strictly
improves the pipeline's honesty.

**The test is not "is it red?" but "is this red fixable inside this packet?"** If yes, fix it — the
exception does not apply and never applies to convenience. If no, and it has a named owner, merge the
improvement.

*Both precedents, recorded so the boundary is drawn by cases and not by adjectives:*

| | **PR #54 — HELD** | **PR #55 — MERGED** |
|---|---|---|
| The red | `Lint` — two clippy lints under a floating toolchain | `Security Audit` — RUSTSEC-2026-0185, RUSTSEC-2026-0204 |
| Introduced by the PR? | No — #54 changed zero `.rs` files | No — #55 is what made the job run at all |
| **Fixable in scope?** | **Yes** — a two-line syntax hotfix (P-002-H) | **No** — needs dependency triage: a `quinn-proto` ≥0.11.15 / `crossbeam-epoch` ≥0.9.20 bump, lockfile churn, and possibly a minor/major version review of its own |
| Named owner | P-002-H, created for it | P-002, already scoped for it |
| **Ruling** | **Hold.** Cheap fix available → fix it, don't merge over it | **Merge.** The alternative is holding a security-gate restoration hostage to the advisories it exists to surface |

**The asymmetry is the whole rule.** #54's red was cheap, so "fix it" was the honest answer. #55's
red is a genuine finding that #55 itself *uncovered* — the job had not executed on any PR for 51
days. Refusing #55 would leave the audit dark in order to avoid seeing what the audit reports.

**A job reporting a true failure beats a job reporting nothing.** A red that tells the truth is a
working gate; a green that never ran is not. When those two conflict, prefer the one that produces
signal — and log the deferred red in `LEDGER.md` against its owning packet, so "tracked to a named
packet" stays a fact and does not decay into a phrase.

---

## §4 Gates

### GATE-1 — C3 is consensus-breaking ⚠️ *(narrowed in v1.3 — the genesis half does not exist)*

> **NARROWED — half this gate was answered by reading the code, not by asking a human.**
>
> `node/src/genesis.rs:47` sets `initial_supply: 0`, commented *"Zero initial supply - tokens created
> through mining rewards only."* Verified at HEAD: that file contains **no allocation, premine,
> initial-balance, or genesis-balance-insert code of any kind** — greps for `allocation`, `premine`,
> `initial_balance`, `genesis_balance`, and for any balance/account map insertion all return empty.
>
> **There are no genesis allocations. The "stranded genesis balances" half of GATE-1 does not exist,
> and never did.** No balance can be stranded at an address that was never credited.
>
> This does **not** weaken DARQ-004 (C3). Three incompatible address derivations are still CONFIRMED
> and still consensus-breaking on the transaction path. What changed is the *blast radius* and the
> *cost of the decision*: there is no premined state to migrate, so the reset-vs-migration question
> collapses to the far cheaper one below.
>
> ⚠️ Note for whoever runs P-003-V's successor: the P-003-V method as written in §11 opens with
> *"obtain a genesis-allocated address and its private key."* **That step is now known to be
> unsatisfiable** — re-scope the repro around a mined or transacted address before running it.

**What survives, and it is one question, not two:** is there **live testnet state from mining or
transactions** — balances anyone cares about? That is narrower than v1.2's framing, it is Sarah's to
answer from the droplet, and reset-vs-migration is correspondingly cheaper either way.

Three derivations confirmed, not the two reported:

| Derivation | Site | Notes |
|---|---|---|
| Raw 32-byte pubkey | `core/src/types.rs:46` | **on the consensus tx path** |
| SHA-256(pubkey) | wallet + genesis | |
| BLAKE3(pubkey) | validator's own keystore | **missed by the audit entirely** |

`Address::from_pubkey` is the natural canonical helper and is already public, but only `core` calls
it. Fix shape: **one helper, four open-coded call sites.**

**Al + Sarah decide:** is there live **mined or transacted** testnet state with balances anyone cares
about? **Chain reset** (clean, correct, pre-mainnet, costs history) or **migration** (preserves
state, materially more code and risk)? With no genesis allocations, reset is materially cheaper than
v1.2 assumed.

**Split the question — this was conflated in v1.1:**

| | Question | Who | Blocked by | Status |
|---|---|---|---|---|
| **Genesis state** | Are genesis-allocated balances stranded? | — | — | ✅ **ANSWERED — no allocations exist** (v1.3) |
| **Local repro** | Does the split actually break spending at runtime? | **Builder** (P-003-V) | nothing | ⚠️ Method needs re-scoping — see the note above |
| **Live state** | Is there deployed mined/transacted state anyone cares about? | **Sarah** | droplet access | ⛔ Open — the only surviving half |

The local repro is a builder task, not a human one, and needs no droplet.

### GATE-2 — C1 is a hard fork

Changing block validation means old and new nodes disagree. Coordinated restart, not rolling
upgrade. Confirm testnet topology before P-004 opens.

### GATE-3 — C2 is a protocol design decision, not a patch ⚠️

The audit says *"recompute work score from verified inputs."* **There are no verified inputs.**
`solve_time` is miner wall-clock; no other node can check it. Current validation is exactly
`is_finite() && >= 0` with `min_work_score: 0.0`. A `WorkScoreCalculator` exists, but its only
non-test consumer is the miner computing the value it then self-declares.

Options for Sarah — input, not recommendation:

1. **Fork choice on header-hash PoW.** Safest; reduces to standard Nakamoto. Useful work becomes a
   *validity gate* rather than the weight metric. Costs the PoUW narrative.
2. **Score derived from the canonical problem instance.** Makes `work_score` a pure function of
   consensus-visible data; fork choice becomes cumulative deterministic difficulty — close to what
   Bitcoin actually does. Preserves PoUW framing.
3. **Clamp against the canonical problem.** Weakest — still trusts a range, still gameable inside it.

**Why Sarah specifically:** consensus and fork-handling are her Rust domain, *and* there are Lean
theorems in this tree that may encode assumptions about fork choice. Changing the metric could
invalidate proofs. She is the only person who can see both sides.

---

## §5 Packet Queue

| ID | Scope | Gate | Status |
|---|---|---|---|
| **P-000** | Commit `loop/` + `LOOP_SPEC.md` | none | 🟡 PR #54 open, awaiting #55 then rebase |
| **P-000-A** | **Spec v1.3 amendments** — D12, GATE-1, §2 discipline, new packets, DARQ-022 | none | 🟡 **This packet.** Docs-only, folds into PR #54 |
| **P-001** | Registry + verification of C3/C1/C2 | — | ✅ complete (Cycle 0) |
| **P-002-H** | **CI hotfix — pin toolchain, clear two lints** | none | 🔵 PR #55 draft, hosted verified — **Al merges** |
| **P-002-H2** | **Pin `release.yml`, `api-server-ci.yml`, `lean4.yml`** | none | ⚪ **Small, ungated, anytime** |
| **P-023** | **Dependabot config — npm coverage + grouping** (DARQ-023) | none | 🔵 **PR #56 draft.** Lands BEFORE P-022 by design |
| **P-022** | DARQ-022 fix — resolve the `latest-upstream` submodule | none | ⛔ Blocked on Al's fix-shape ruling. **Run AFTER P-023** |
| **P-024** | DARQ-024 — Python dependency governance (needs a manifest first) | none | ⛔ **Unassigned — Al's decision**, not a config change |
| **P-002** | `deny.toml` + `cargo-deny` + multi-ecosystem reconciliation + **DARQ-022** | none | Blocked on P-000 merge |
| **P-003-V** | C3 local repro — ⚠️ **method invalidated by the GATE-1 narrowing, re-scope first** | none | Ran in Cycle 1; re-scope needed |
| **P-021-V** | **DARQ-021 apply-path verification — does the debit site index `from`?** | none | 🔴 **Highest value in the queue — do first once #55/#54 land** |
| **P-021** | DARQ-021 fix — enforce from/pubkey binding on all ingest paths | **GATE-2** (hard fork) | ⛔ Blocked until P-021-V sizes it |
| **P-003** | C3 address derivation unification | **GATE-1** | Blocked. Adversary mandatory. |
| **P-004** | C1 canonical problem regeneration | **GATE-2** | Blocked. Adversary mandatory. |
| **P-004-D** | C2 fork-choice metric — **design packet** | **GATE-3** | Blocked on Sarah. Not a build packet. |
| **P-005** | C4, C5, C6, M4 + `rpc/src/server.rs:1583` — ledger apply path | none | After P-002. Adversary mandatory. |
| **P-006** | SEC-PR-002 transport gate **+** C7 per-account authz **+** H8, H10 | none | One PR — same files, same concern. |
| **P-007** | H6, H7 — escrow signature verification | none | |
| **P-008** | H1–H5, M8, M9 — gossip auth + bounded ingress | none | Codex "bounded ingress" (~25) |
| **P-009** | H9, H11, M10 + telemetry amplification (DARQ-019) | none | Verify the 64 MiB / 300 s figures first |
| **P-010** | M1, M2, M5, M6, L1–L5 — sweep | none | |
| **P-011** | ~668 `unwrap`/`expect` on reachable paths | none | Network layer alone: 329 + 28 |

### Ordering — the deadlock, resolved

P-000 can't go green until clippy is fixed → the clippy fix was P-002's business → P-002's STEP 0
refuses to start until P-000 merges. Al ruled **fix, don't merge over red** [D12]:

1. **P-002-H** branches from `main`, goes green, merges → main green, security gate live again
2. **P-000** rebases onto green main → goes green → merges
   *(No conflict is possible: P-000 is docs-only, P-002-H is src+config only. Verified in Phase −1 by
   file-level set intersection of the two diffs: **empty**.)*
3. **P-002** starts, re-scoped per §0
4. **P-003-V** runs in parallel with any of the above — it depends on nothing

**Amended by v1.3 at step 1:** P-002-H merges **despite** its red Security Audit, per the bounded
exception in D12. The original ordering assumed a fully green #55 was achievable inside P-002-H's
scope; it is not — the red is the two RUSTSEC advisories, which are P-002's work. **Waiting for green
here would deadlock the queue for the second time, on the same shape of reasoning as the first.**

**After step 2, the queue re-prioritises: P-021-V goes first, ahead of P-002.** DARQ-021 is
unsized, and it is the only open item that could turn out to be an arbitrary-theft path. Sizing it
is cheap (read-only) and it dominates every scheduling decision downstream — if it confirms, it
outranks C3 and expands P-005; if it does not, the queue is unchanged and the cost was one
verification packet.

### P-002 re-scope

Not "build a pipeline." The real delta is now: **write `deny.toml`** (without it `cargo-deny` cannot
run at all), **wire `cargo-deny` into the existing job**, and **reconcile the four dependency
numbers across ecosystems**. Do **not** add CodeQL. Do **not** duplicate the existing Security Audit
job.

### P-002 sharpened again *(v1.3)*

`cargo deny check` is **already present in `ci.yml` and toothless twice over**: it carries
`continue-on-error: true`, *and* it never executes at all because `cargo audit` fails first in the
same job. Either defect alone would make the gate decorative; both together mean it has never once
produced a verdict. **P-002 must do all three of the following, or the gate stays decorative:**

1. **Add `deny.toml`.** Without it `cargo-deny` cannot run at all — this is why "wire up cargo-deny"
   was never sufficient as a scope.
2. **Remove `continue-on-error: true`.** A gate that cannot fail the build is documentation.
3. **Resolve the two RUSTSEC advisories** — patch-level bump where available
   (`quinn-proto` ≥ 0.11.15, `crossbeam-epoch` ≥ 0.9.20); otherwise a `deny.toml` ignore **with a
   written reason and an expiry date**. Never a silent ignore, and never an ignore without a date —
   an undated ignore is how an advisory becomes permanent.

Plus **DARQ-022** (Dependabot has not run successfully on `main`), **DARQ-023** (npm unconfigured —
closed by P-023), and the multi-ecosystem reconciliation. "Fixed the 2 Rust advisories" must never be
recorded as "dependencies clean."

#### ⚠️ P-002 must UNION both scanners — the inter-ecosystem framing above was incomplete *(v1.4)*

v1.3 framed the Dependabot/`cargo audit` gap as purely **inter**-ecosystem: `cargo audit` sees Rust,
Dependabot sees everything, the difference is JS and Python. **That is wrong. There is an
intra-Rust gap as well.** Measured at `b1aaf59b` during P-023:

| Crate | Version in `Cargo.lock` | `cargo audit` (RustSec) | Dependabot (GHSA) |
|---|---|---|---|
| `quinn-proto` | 0.11.14 | ✅ RUSTSEC-2026-0185 | ✅ GHSA-4w2j-m93h-cj5j → 0.11.15 |
| `crossbeam-epoch` | 0.9.18 | ✅ RUSTSEC-2026-0204 | ❌ **not reported** |
| `rand` | 0.8.5 | ❌ **not reported** | ✅ GHSA-cq8v-f236-94qc → 0.8.6 |

All three versions verified present in `Cargo.lock`. **Each scanner reports exactly two of the three.
Neither is a superset of the other.**

**Therefore P-002's Rust scope is the UNION, not either list.** Resolving "the two RUSTSEC
advisories" as originally written would leave `rand` 0.8.5 untouched, because `cargo audit` never
names it. Conversely, working only from the Dependabot alert list would leave `crossbeam-epoch`
untouched.

⚠️ **The mechanism of the discrepancy is NOT explained, and P-002 should not proceed as if it were.**
`rand`'s advisory exists in RustSec as **RUSTSEC-2026-0097** — the same database `cargo audit` loaded
1186 advisories from on that run — and `cargo audit` *does* surface `informational = unsound`
advisories, as it did for `anyhow` (RUSTSEC-2026-0190) in the same output. So "it's only
informational" does **not** account for the omission. Confirmed from the full audit log, not a
partial grep.

*Untested hypothesis, recorded as a hypothesis:* RUSTSEC-2026-0097 carries `affected functions`
metadata (`rand::thread_rng`, `rand::rng`) which RUSTSEC-2026-0190 may not, and `cargo audit` may
filter on it. **Do not act on this until it is tested.** Until the mechanism is known, assume
`cargo audit` may be silently omitting other advisories on the same grounds — which makes the union
requirement a floor, not a ceiling.

**Consequence for D12 bookkeeping:** the LEDGER entry for PR #55 records "the two advisories"
accurately *as a record of what the Security Audit job reported* — which is what it claims to be —
but that list understates Rust exposure by one crate. **Annotate it; do not rewrite it.** The
standing obligation in that entry (P-002 must re-enumerate from scratch rather than trust the
recorded list) is now empirically vindicated: the list was already incomplete when it was written.

⚠️ **Sequencing note:** DARQ-022 should be settled *before* the reconciliation, not after. Until
Dependabot runs, the 18/19 alert counts are stale by an unknown margin — they reflect whatever the
last successful run saw, which was at least three weeks before this writing. Reconciling against a
frozen number produces a confident wrong answer.

---

## §6 Phase Protocol

`loop/STATE.md` carries `CYCLE`, `PHASE`, `PACKET`, `BRANCH`, `CAPACITY_FLAG`.

| Phase | Meaning | Exit |
|---|---|---|
| **A** | Reconcile / verify — read-only | Registry written, STOP + report |
| **B** | Build — active packet on a branch | Green build + tests, diff ferried |
| **C** | Adversary — fix-verification only | BLOCK / MERGE-WITH-NOTES / CLEAN |
| **D** | Synthesis — Al + Opus | Gate ruling, LEDGER entry, merge or return to B |

Phase C conditional (P-003, P-004, P-005). `CAPACITY_FLAG: remediation-priority` STOPs the loop.

---

## §7 Registry Schema + Canonical Root-Cause Map

```
DARQ-ID       | DARQ-001
Title         | Fork choice trusts miner-supplied work parameters
Severity      | Critical | High | Medium | Low | Informational
Category      | Logic | Integer | Auth | P2P | Crypto | Panic | Serde | SupplyChain | AsyncRace
Root cause    | RC-01
Third-party   | C1, C2
Codex         | program "deterministic transition validation" (partial)
Location      | <verified path:line at HEAD — or DRIFTED / NOT-FOUND / ALREADY-FIXED / UNVERIFIED>
Verified at   | <commit SHA>
Status        | Open | In Progress | Resolved | Accepted Risk | Won't Fix
Packet        | P-004
Notes         | <audit citation errors, composition risks, blockers>
```

**Verdicts — use exactly these, never guess:** `CONFIRMED` · `DRIFTED` (record new location) ·
`ALREADY-FIXED` (record the SHA) · `NOT-FOUND` (false positive — say why) · `NEEDS-HUMAN`.

A `NOT-FOUND` is a **valuable** result. Human audits fail by under-reporting false positives just as
automated ones fail by over-reporting them.

### 36 findings / 18 root causes · DARQ-001…019

| RC | Root cause | Findings | Packet |
|---|---|---|---|
| RC-01 | Consensus trusts miner-supplied work parameters | C1, C2 | P-004 / P-004-D |
| RC-02 | Validation bypass on direct apply paths | M3 | P-005 |
| RC-03 | Dual bincode/JSON hashing — acceptance ambiguity | M7 | P-004 |
| RC-04 | Inconsistent address derivation (×3) | C3 | P-003 |
| RC-05 | Ledger apply path performs no validation | C4, C5, C6, M4, NEW-1 | P-005 |
| RC-06 | Unauthenticated state-mutating endpoints | C7, H8 | P-006 |
| RC-07 | Client IP taken from spoofable headers | H10 | P-006 |
| RC-08 | Escrow signature verification incomplete | H6, H7 | P-007 |
| RC-09 | Gossip accepted without verified sender | H1, H2, M8 | P-008 |
| RC-10 | Unbounded peer / mempool ingress | H3, H4, H5, M9 | P-008 |
| RC-11 | Missing rate limits on public API | H11, L5 | P-009 |
| RC-12 | Attacker-drivable disk / log amplification | NEW-2, L3 | P-009 |
| RC-13 | Panic on malformed consensus input | M1, M2, M6 | P-010 |
| RC-14 | Non-atomic multi-table writes | M5 | P-010 |
| RC-15 | Query injection via unencoded interpolation | H9 | P-009 |
| RC-16 | Information disclosure / IDOR | M10 | P-009 |
| RC-17 | Key and credential handling weaknesses | L1, L2 | P-010 |
| RC-18 | Unchecked time / expiry arithmetic | L4 | P-010 |

**DARQ-019** is a composition (RC-11 × RC-12) and correctly carries its own ID without inventing a
19th root cause.

---

## §8 New Findings

**DARQ-NEW-1** · Medium · Integer · `rpc/src/server.rs:1583` — uncited C5-class unchecked
arithmetic. Folds into RC-05 / P-005.

**DARQ-NEW-2** · Medium alone · `node/src/validator.rs:102–130` — debug telemetry writes JSON to
disk on **every bad-parent block**. Attacker-drivable. **Confirmed at HEAD.**

**DARQ-019 (NEW-3)** · **High — PROVISIONAL** · Composition of NEW-2 × H11

> `/node-rpc` accepts **64 MiB** `chain_submitBlock` bodies with a **300-second timeout** and **no
> rate limiting** (H11). Every bad-parent block triggers a **disk write** (NEW-2).
>
> ⇒ Unauthenticated remote attacker → unlimited 64 MiB submissions → unbounded disk writes.
> **Disk-exhaustion DoS assembled from two separately-filed findings.**

⚠️ **Severity is provisional.** The amplifier half (NEW-2) is confirmed at HEAD. The ingest half —
the 64 MiB and 300 s figures — is **carried from the third-party report and unverified at this
HEAD**. Given that report's demonstrated citation errors, verify before P-009 locks the severity.
Fixing either half mitigates; fixing H11 is cheaper.

**DARQ-NEW-4** · Operational · `.github/workflows/ci.yml` — floating `RUST_TOOLCHAIN: stable` with
`needs: lint` fan-out means an upstream lint release silently disables the security gate. Closed by
P-002-H + D11.

**DARQ-021 (NEW-5)** · **Critical — UNSIZED** · `node/src/validator.rs:169`, `mempool/src/pool.rs:160`
Block validation and mempool admission both call `verify_signature()`, which checks the Ed25519
signature **only**. The `from == public_key.to_address()` binding lives solely in
`Transaction::is_valid()` (`core/src/transaction.rs:437-439`), reached only via `Block::verify()`
(`core/src/block.rs:215`) — **which `node/src` never calls.** Every ingest route uses
`validate_block_with_options`. Runtime probe (P-003-V): a transfer naming an arbitrary victim as
`from`, signed by an attacker key, returns `verify_signature = true`. **Severity deliberately
withheld pending P-021-V** — it turns entirely on whether the ledger apply path debits `tx.from` or
an address derived from the signing key. Do not assign a severity before that is traced.

**DARQ-022 (NEW-6)** · Operational · `.gitmodules` + submodule pin — **Dependabot has not run
successfully on `main`, across every ecosystem, for at least three weeks**

> Found during Phase −1 state detection, while enumerating CI runs on `main` for an unrelated reason.
>
> Every `Dependabot Updates` run on `main` fails — **npm_and_yarn *and* cargo**, both `/web-wallet`
> and `/web/coinjecture-evolved-main` and `/.` — from at least 2026-07-13 through 2026-08-02.
> **Root cause, read from the run log, is not ecosystem-specific and not a dependency problem at
> all:**
>
> ```
> Submodule 'latest-upstream' (https://github.com/Quigles1337/COINjecture2.0.git) registered
> fatal: remote error: upload-pack: not our ref 6a32fbfc7094fe82c02a91b231b52798c9f42972
> fatal: Fetched in submodule path 'latest-upstream', but it did not contain 6a32fbfc...
> ```
>
> `.gitmodules` registers `latest-upstream` → `Quigles1337/COINjecture2.0`, and the tree pins it at
> `6a32fbfc7094fe82c02a91b231b52798c9f42972`. **That commit is not reachable in that remote** —
> confirmed independently with `git ls-remote`, which does not list it. Dependabot clones with
> submodules, fails at clone time, and aborts **before any ecosystem update logic runs**. That is why
> the failure is total rather than partial.
>
> **Consequence — this is the part that matters.** Dependabot's automated fix PRs are not being
> generated. Confirmed by observation: the open-PR list contains **zero** Dependabot PRs (only #46,
> #48, #54, #55, all authored by Al). **This is a strong candidate explanation for why 18–19 alerts
> sit open** — not neglect, but a broken updater that fails silently in a workflow nobody reads
> because it is "expected to be noisy."
>
> ⚠️ **The 18/19 alert counts are therefore stale by an unknown margin**, frozen at whatever the last
> successful run saw. Any reconciliation of the four dependency numbers (§9) that treats them as
> current will produce a confident wrong answer. Settle DARQ-022 first.
>
> **Same class as DARQ-020, different mechanism.** DARQ-020: a floating toolchain darkens the audit
> *job*. DARQ-022: a dangling submodule darkens the dependency *updater*. In both cases an unrelated
> failure disabled a security mechanism, in both cases nothing announced it, and in both cases the
> workflow was "running" the whole time. **Assigned to P-002.**
>
> **Fix shape — needs a decision, not just a commit.** Three options, in increasing order of
> commitment: (a) update the pin to a commit that exists in the fork; (b) remove the `latest-upstream`
> submodule entirely, if it serves no build purpose — nothing in the workspace appears to consume it;
> (c) re-point it at the org repo. **(b) is the likely right answer and it is Al's call**, because it
> also severs the org repo's in-tree dependency on the personal fork — see the clone hazard in §2.
> Note that `proofs/eigenverse` is a second submodule and is **not** implicated in the failure; do not
> remove it while fixing this.

**DARQ-023 (NEW-7)** · Operational · `.github/dependabot.yml` — npm held **21 of 23 open alerts** with
**no `updates` entry at all**. Security updates ran for npm regardless (they do not require an entry),
so the ecosystem looked covered while version updates — the routine bumps that pre-empt alerts — were
never generated. **"Dependabot is running for npm" and "npm is configured" were different claims.**
Closed by **P-023** (PR #56).

**DARQ-024 (NEW-8)** · Operational · no Python manifest — 17 `.py` files import `requests` and
`huggingface_hub`; no `requirements.txt`, `pyproject.toml`, `setup.py`, `Pipfile` or lockfile exists.
**CodeQL scans the Python code while nothing watches the Python dependencies.** Not closable by
configuration — with no manifest there is nothing to parse. Needs a manifest first, which is a real
change (it pins currently-implicit versions and touches the HF scripts and the test harness).
**Unassigned; Al's decision.**

### Advisory sizing — `rand` GHSA-cq8v-f236-94qc is NOT an RNG defect *(v1.4)*

*Belongs to P-002's Rust union scope (see §5). Not related to DARQ-021, which is the sender-binding
finding.*

Pulled during P-023 because `rand` sits near key generation on this chain and an RNG defect there
would be critical. **It is not one. Recorded in full so nobody re-opens this question from the crate
name alone.**

**RUSTSEC-2026-0097 / GHSA-cq8v-f236-94qc — "Rand is unsound with a custom logger using
`rand::rng()`".** Classification **INFO / Unsound**, severity **low**, **no CVE**, no CVSS. It is a
*soundness* bug — safe code can trigger Undefined Behaviour — **not a defect in randomness quality,
entropy, or predictability.** The mechanism is an aliased mutable reference: `ThreadRng`'s
`RngCore`/`TryRng` methods cast `*mut BlockRng<ReseedingCore>` to `&mut`, and reentrancy produces two
live mutable references, violating Stacked Borrows.

**It requires all five of these simultaneously:** the `log` *and* `thread_rng` features enabled; a
**custom logger** defined; that logger calling `rand::rng()`/`thread_rng()` and invoking RNG methods;
the `ThreadRng` reseeding *during* that logger call (every 64 kB); and trace-level logging, or
warn-level with `getrandom` failing to supply a seed.

**Are already-generated keys implicated? No — on four independent grounds, any one of which suffices:**

1. **Wrong failure mode.** The bug cannot produce weak, biased or predictable output. It produces UB
   through aliasing. There is no path from this defect to a guessable key.
2. **The affected copy is not in the crypto path.** `rand 0.8.5` is **transitive only**, reachable
   from exactly six third-party crates — `jsonrpsee-core`, `jsonwebtoken`, `num-bigint-dig`,
   `rust_decimal`, `soketto`, `tungstenite`. RPC transport, JWT, decimals, bigints. **No first-party
   crate depends on it.**
3. **First-party `rand` is 0.9.4, which is not affected.** The workspace declares `rand = "0.9"`, and
   the advisory patches the 0.9 line at **0.9.3**. `0.9.4 ≥ 0.9.3`. So even where first-party code
   does use `rand`, it is not the vulnerable copy.
4. **Key generation does not use the affected functions at all.** Every `SigningKey::generate` site
   passes **`OsRng`** (`api-server/src/crypto.rs:41,56`, `consensus/src/coordinator/commit.rs:255,309`,
   and the test sites). `ed25519-dalek 2.2.0` depends on **`rand_core 0.6.4`**, not on `rand`. The
   advisory's affected functions are `rand::thread_rng` and `rand::rng` — neither is `OsRng`.

**And the precondition is unmet anyway:** a repo-wide search for `impl log::Log`, `set_boxed_logger`
and `log::set_logger` finds **no custom logger implementation**. Condition 2 of five does not hold, so
the bug is unreachable in this codebase as written.

**On "signing nonces" specifically:** Ed25519 signing is *deterministic* by construction (RFC 8032) —
the per-signature nonce is derived by hashing the private key with the message, **not sampled from an
RNG**. There is no RNG consumer on the Ed25519 signing path to be affected. Key *generation* consumes
randomness; signing does not.

**Verdict: patch it as routine hygiene, not as an incident.** It stays in P-002's union scope because
it is a real advisory against a version in the tree — the fix is a transitive bump to `rand` 0.8.6 —
but there is **no key rotation, no re-issuance, and no retroactive exposure**. Nothing generated by
this chain is called into question by it.

---

## §9 Dependency Baseline — multi-ecosystem

`cargo-audit 0.22.2` and `cargo-deny 0.20.2` installed. **No `deny.toml`, so deny cannot run.**
CodeQL runs via GitHub default setup (actions, javascript-typescript, python, rust). A Security
Audit job exists in `ci.yml` — currently never executing, because `needs: lint`.

Four numbers for the same question:

| Source | Count | Scope |
|---|---|---|
| Dependabot (API) | 18 | all ecosystems |
| Dependabot (push banner) | 19 | all ecosystems |
| v1.1 spec | ~14 | stale |
| `cargo audit` | **2** | Rust/RustSec only |

**These are not in conflict — they measure different graphs.** `cargo audit` is structurally blind
to JS and Python dependencies. The ~16 difference is the multi-ecosystem blind spot, and it is
larger than the Rust one.

Rust advisories:
- **RUSTSEC-2026-0185** — `quinn-proto`, High 7.5. **QUIC — the network layer of a P2P chain.**
  Postdates my knowledge cutoff. **Pull the advisory text before triaging.**
- **RUSTSEC-2026-0204** — `crossbeam-epoch`

---

## §10 PHASE B — P-002-H BUILDER PROMPT (CI hotfix)

```text
You are the BUILDER seat of the DARQ AGI Mode loop for COINjecture 2.0.
Read loop/LOOP_SPEC.md — it binds you. This prompt adds packet scope only.

PACKET: P-002-H — CI hotfix: pin toolchain, clear two lints
CYCLE: 1 · PHASE: B · BRANCH: fix/ci-pin-toolchain-clippy

WHY THIS PREEMPTS THE QUEUE
main is red. Lint fails; test, build and security all `needs: lint`, so the
Security Audit job has not run on any PR since stable advanced. The pipeline
is dark. A dark security gate outranks the packet queue. Al ruled: fix, do
not merge over red [D12].

STEP 0 — STATE DETECTION
- Confirm the clone is COINjecture2.0-network. If the remote is Quigles1337,
  STOP — wrong clone.
- If loop/STATE.md shows PHASE: B with PACKET: P-002-H, resume from BRANCH.
- P-000 (PR #54) is open and NOT merged. That is expected. This packet
  branches from main, NOT from feat/p000-loop-scaffolding.
- Create fix/ci-pin-toolchain-clippy from current main.

STEP 1 — ESTABLISH THE DARK WINDOW (read-only; do this first)
Determine when CI went dark and what merged blind:
  gh run list --branch main --workflow ci.yml --limit 50
Identify the last green run and the first red run; record both SHAs and dates.
Then git log between them: what merged while lint was failing?

Report the window and the merged commits. Do NOT act on it — this is
information for Al, not a task. If PRs #46/#47/#48 or the five Dependabot
bumps fall inside the window, say so explicitly: they merged with no test
signal and no audit signal.

STEP 2 — PIN THE TOOLCHAIN (root cause; the lints are symptoms) [D11]
Add rust-toolchain.toml at the repo root pinning the CURRENT stable version —
the one hosted CI is actually running. Determine it; do not assume a number.
Include components = ["rustfmt", "clippy"].

CRITICAL — ONE SOURCE OF TRUTH. ci.yml currently sets RUST_TOOLCHAIN: stable.
That env var and rust-toolchain.toml must not disagree. Either make ci.yml
consume the toolchain file, or set the env var to the identical pinned
version. State which you chose and why.

TEAM IMPACT — say this plainly in the report: rust-toolchain.toml changes the
toolchain for EVERY developer, not just CI. rustup will auto-download the
pinned version for Sarah and anyone else on this repo. That is the intended
effect — local finally matches hosted — but it is not a CI-only change and
must not be described as one.

STEP 3 — CLEAR THE TWO LINTS
  clippy::unneeded_wildcard_pattern     — node/src/validator.rs:642
  clippy::useless_borrows_in_formatting — wallet/src/commands/marketplace.rs:33

HARD GUARDRAIL: both fixes must be PROVABLY BEHAVIOUR-PRESERVING — syntax
only. validator.rs is consensus code [D5]. If either fix requires changing
logic rather than syntax, STOP AND REPORT: it is not a hotfix and does not
belong on this branch.

For each, quote before and after and state in one line why the change cannot
alter behaviour.

Then run the FULL clippy sweep under the pinned toolchain and report the
COMPLETE warning list, not just these two. -D warnings fails the job, but
clippy still enumerates everything before failing. If the pinned version
flags more than these two, report the full set and STOP before fixing — the
packet needs re-scoping.

STEP 4 — VERIFY THE WHOLE PIPELINE, NOT JUST LINT
The 936-test baseline was measured under an older local toolchain. NOBODY has
run this suite under current stable, because test needs: lint and lint has
been red. Expect surprises downstream; do not panic if they appear.

Locally, under the pinned toolchain: cargo fmt --check, cargo clippy
-D warnings, cargo build, cargo test. Report results against the
936 passing / 0 failed / 4 ignored baseline. Any deviation is a FINDING to
report, not a thing to fix on this branch.

Then push and confirm on the HOSTED runner that lint, test, build AND the
existing Security Audit job all execute and report. Getting the audit job to
actually run is the real deliverable of this packet — a green lint that still
leaves the audit dark is a failed packet.

STEP 5 — REPORT
Write loop/reports/C1-hotfix-builder.md:
  1. Dark window — dates, SHAs, what merged blind
  2. Pinned version, where the pin lives, how the two-sources problem resolved
  3. Both lint fixes: before/after + behaviour-preservation argument
  4. Complete clippy output under the pinned toolchain
  5. Local results vs the 936 baseline
  6. Hosted run ID and per-job status, especially Security Audit
  7. 2-4 things you want a second opinion on rather than let stand
  8. What you did NOT do and why

Open as a DRAFT PR. Do not merge — that is Phase D. Set PHASE: D.

GUARDRAILS
- Scope: rust-toolchain.toml, ci.yml if needed, exactly two lint fixes.
  Nothing else. No deny.toml (that is P-002), no dependency bumps, no CodeQL.
- Behaviour-preserving only. Consensus code is involved [D5].
- Draft PR, no merge, no direct-to-main [D10].

STOP CONDITIONS — report rather than work around
- More than the two known lints fire under the pinned toolchain
- Either lint fix would require a behaviour change
- Tests deviate from 936 / 0 / 4 under the pinned toolchain
- The hosted Security Audit job still does not run after lint goes green
```

---

## §11 PHASE A — P-003-V BUILDER PROMPT (C3 local repro, parallel)

```text
You are the BUILDER seat of the DARQ AGI Mode loop for COINjecture 2.0.
Read loop/LOOP_SPEC.md — it binds you.

PACKET: P-003-V — C3 runtime verification (local genesis spend test)
PHASE: A — verification. READ-ONLY with respect to the repo.

This packet is independent. It does not need CI, P-000, P-002-H, or any
server access. It can run in parallel with anything.

WHY THIS EXISTS
Static analysis proved three incompatible address derivations exist. It did
NOT prove nothing reconciles them at runtime — a translation layer, a
compatibility shim, or a lookup that tries multiple derivations would change
the picture entirely. Only execution settles it.

THE HYPOTHESIS TO TEST
Genesis credits a balance to address A = SHA-256(pubkey). The consensus
transaction path derives the sender's address as B = raw 32-byte pubkey. If
nothing reconciles them, a genesis-allocated balance is visible in state and
UNSPENDABLE by the key that owns it.

METHOD
1. Start a local node from genesis (cargo run — testnet/dev config, fresh
   data dir, no external peers).
2. Obtain a genesis-allocated address and its private key from the genesis
   config / keystore.
3. Query the balance at that address. Record what the node reports.
4. Sign and submit a transfer from that address.
5. Record EXACTLY what happens: accepted, rejected, error text, and which
   address the validator actually looked up.

Instrument if needed — a temporary debug print of the derived address on the
validation path is fine. Do NOT commit it.

REPORT loop/reports/C1-p003v-builder.md:
  - Verdict: CONFIRMED (spend fails as predicted) / NOT-FOUND (spend succeeds
    — something reconciles them; explain what) / NEEDS-HUMAN (couldn't run)
  - The exact failure mode and error text
  - Which derivation each side actually used at runtime
  - Whether ANY reconciliation layer exists anywhere in the path
  - If CONFIRMED: does the balance appear in state queries while being
    unspendable? That determines whether the bug is silent or visible.

GUARDRAILS
- No src/ commits. Temporary local instrumentation only, reverted after.
- Do not attempt to FIX C3. That is P-003 and it is gated [GATE-1].
- Local node only. No droplet, no deployed network, no external peers.
- A NOT-FOUND verdict is valuable, not a failure — it would mean the audit
  and the static verification both missed a reconciliation layer.
```

---

## §11.1 PHASE A — P-021-V BUILDER PROMPT (DARQ-021 apply-path verification)

*Committed here so it never needs re-pasting. This is the first packet to run once #55 and #54 land.*

```text
You are the BUILDER seat of the DARQ AGI Mode loop for COINjecture 2.0.
Read loop/LOOP_SPEC.md — it binds you.

PACKET: P-021-V — DARQ-021 apply-path verification
PHASE: A — verification. READ-ONLY. No fix.

WHAT THE LAST SESSION FOUND (while probing C3, not while looking for this)
verify_signature() validates the signature only. The binding
    from == public_key.to_address()
lives solely in is_valid(). Block::verify() is the only caller of is_valid()
and is never invoked anywhere in node/src. Both block validation
(validator.rs:169) and mempool admission (pool.rs:160) call ONLY
verify_signature(). Every ingest route uses validate_block_with_options.

The probe demonstrated directly: a transaction whose `from` is an arbitrary
victim address the signing key does not own returns verify_signature = true.

THE ONE QUESTION THAT SETS SEVERITY
Does the ledger apply path debit the `from` field, or an address derived from
the signing public key?

  debits `from`            -> arbitrary theft from any account. Critical.
                              Outranks C3 and everything else open.
  debits derived address   -> not theft. Impact is spoofed attribution or
                              accounting corruption. Size it honestly.

Do not assume. Trace it. Do not assign severity before step 2 is answered.

METHOD
1. Map every path from ingest (RPC and gossip) through mempool admission and
   block application to the point where a balance is mutated.
2. At each debit site, record EXACTLY which value indexes the account:
   tx.from, or something derived from tx.public_key. path:line.
3. Record which derivation the debit site uses — raw, SHA-256, or BLAKE3.
   C3 and DARQ-021 interact: the debit site may use a different derivation
   than is_valid() does, and that changes what the exploit actually yields.
4. Determine whether ANY path reaching balance mutation enforces the
   from/pubkey binding by any means.
5. Only if 1-4 show a theft path, write a local probe demonstrating it end to
   end. Same discipline as P-003-V: written, run, deleted, never committed.

ALSO SETTLE
  - Network reachability: is this reachable from the RPC or gossip boundary,
    or only from a local API? Trace inward from the ingest boundary.
  - Do mempool admission and block application differ in exposure? Both were
    named; they may not be equally reachable.
  - Distinctness from M3: M3 is callers bypassing the validator. DARQ-021 is
    the validator performing the weaker of two checks that both exist.
    Confirm they are distinct and that fixing M3 as written would NOT close
    DARQ-021.

REPORT loop/reports/C2-p021v-builder.md
  1. Verdict: CONFIRMED-THEFT / CONFIRMED-NON-THEFT / NOT-FOUND / NEEDS-HUMAN
  2. The exact debit site, path:line, and what indexes the account
  3. Which derivation the debit site uses, and the C3 interaction
  4. Network reachability, per ingest route
  5. Proposed severity WITH reasoning, never a bare label
  6. Fix shape: one check in one place, or N ingest sites?
  7. Consensus impact: adding a validation check means old nodes accept what
     new nodes reject. Confirm whether this is a hard fork [D5, GATE-2].
  8. 2-4 things you want a second opinion on

GUARDRAILS
  - Read-only. Do NOT fix. The fix is consensus-affecting and gated.
  - Probe code temporary, deleted after, never committed.
  - Severity is determined by the debit site, not by intuition.
  - If you cannot settle the debit site from the code, NEEDS-HUMAN is the
    correct verdict. Do not reason toward a severity.
```

---

## §12 Al's Track — decisions, no code

*Updated v1.3. Items resolved by evidence are struck through with the resolution, not deleted — the
record of what was once open is part of the audit trail.*

1. ☑ ~~**Confirm the clone**~~ — **RESOLVED.** `origin` is `COINjecture-Network/COINjecture2.0`,
   re-confirmed in Phase −1. All verdicts stand. *(The second clone is now item 5, and it grew teeth.)*
2. ☐ **Merge PR #55 (P-002-H)** — ruled: **merge despite the red Security Audit**, per the amended
   D12 above. The red is the two RUSTSEC advisories, owned by P-002. This unblocks everything.
3. ☐ **Merge PR #54 (P-000 + P-000-A)** after #55, once the builder rebases it and hosted CI confirms.
   Docs-only; conflict is impossible (verified by diff intersection).
4. ☐ **GATE-1 to Sarah — now materially narrower.** No genesis allocations exist, so the only
   surviving question is live **mined or transacted** testnet state. Reset is cheaper than assumed.
5. ☐ **GATE-3 to Sarah** — the three fork-choice options in §4, with the Lean-proof interaction
   flagged explicitly.
6. ☑ ~~**Reconcile the Lean count**~~ — **half resolved.** 89 is re-measured and CONFIRMED under
   three independent commands (§2). 586 remains unreconciled, but is now a concrete testable
   question: `proofs/eigenverse` is a registered, **unpopulated** submodule. ☐ Someone should
   populate it and re-count. That is a 5-minute task, not a decision.
7. ☐ **Decide DARQ-022's fix shape — this one is genuinely yours.** The `latest-upstream` submodule
   points at the personal fork and its pin is dangling, which has silently disabled Dependabot across
   all ecosystems for at least three weeks. Options in §8: re-pin, **remove**, or re-point at the org
   repo. Removing it also severs the org repo's in-tree dependency on the personal fork — which
   subsumes what used to be item 5 below.
8. ☐ **Clean up or rename the second clone** — no longer cosmetic. See item 7; the local directory is
   the smaller half of this problem, and fixing the local clone alone does **not** fix `main`.
9. ☑ ~~**Review the dark window**~~ — **RESOLVED: nothing to review.** Last CI run 2026-06-12 (green,
   `28c50a12`); next 2026-08-02 (red, PR #54). **Zero commits to `main` in between.** Rust 1.97.1
   landed inside a 51-day dormancy, so `main` went retroactively red without a commit. Nothing merged
   blind; nothing needs re-verification.
10. ☐ **Authorize P-021-V** once #54 lands — §11.1 carries the full prompt. It is read-only, ungated,
    and it is the only open item that could turn out to be arbitrary theft.
