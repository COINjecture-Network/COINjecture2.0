# DARQ AGI Mode — COINjecture 2.0 Remediation Loop

**Spec v1.2** · Supersedes v1.1 · Target: `COINjecture-Network/COINjecture2.0`
Baseline anchored at `28c50a12` · **Commit over `loop/LOOP_SPEC.md`**

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
- **936** tests passing / 0 failed / 4 ignored · **89** Lean theorems / 19 files · **15** crates
- Lean count **unreconciled**, not corrected. Untested hypothesis: 586 may be *Eigenverse's* number.
- ⚠️ **Baseline caveat (new):** 936 was measured under an **older local toolchain**. Nobody has run
  this suite under current stable, because `test` `needs: lint` and lint has been red. Treat 936 as
  provisional until P-002-H confirms it under the pinned toolchain.

**⚠️ Clone hazard.** A second clone at `C:\Users\LEET\COINjecture2.0` (Quigles1337 remote) is also
v4.8.4 / 15 crates but is **dirty with 20 live worktrees and has no `lean4/`**. Clean it up or
rename it unmistakably.

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

**D12 — Green before merge, no exceptions.** *(New — Cycle 1, ruled by Al.)* A red pipeline is never
merged past, even when the failure is pre-existing and unrelated to the PR at hand. The correct
response to unrelated red is a hotfix that preempts the queue.

---

## §4 Gates

### GATE-1 — C3 is genesis- *and* consensus-breaking ⚠️

Three derivations confirmed, not the two reported:

| Derivation | Site | Notes |
|---|---|---|
| Raw 32-byte pubkey | `core/src/types.rs:46` | **on the consensus tx path** |
| SHA-256(pubkey) | wallet + genesis | |
| BLAKE3(pubkey) | validator's own keystore | **missed by the audit entirely** |

`Address::from_pubkey` is the natural canonical helper and is already public, but only `core` calls
it. Fix shape: **one helper, four open-coded call sites.**

**Al + Sarah decide:** is there live testnet state with balances anyone cares about? **Chain reset**
(clean, correct, pre-mainnet, costs history) or **migration** (preserves state, materially more code
and risk)?

**Split the question — this was conflated in v1.1:**

| | Question | Who | Blocked by |
|---|---|---|---|
| **Local repro** | Does the split actually break spending at runtime? | **Builder** (P-003-V) | nothing |
| **Live state** | Is there deployed state with balances anyone cares about? | **Sarah** | droplet access |

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
| **P-000** | Commit `loop/` + `LOOP_SPEC.md` | none | 🟡 PR #54 open, blocked on red main |
| **P-001** | Registry + verification of C3/C1/C2 | — | ✅ complete (Cycle 0) |
| **P-002-H** | **CI hotfix — pin toolchain, clear two lints** | none | 🔴 **DO FIRST — preempts queue** |
| **P-002** | `deny.toml` + `cargo-deny` + multi-ecosystem reconciliation | none | Blocked on P-000 merge |
| **P-003-V** | **C3 local repro — spend from a genesis address** | none | **Ready — parallel, blocked by nothing** |
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
   *(No conflict is possible: P-000 is docs-only, P-002-H is src+config only.)*
3. **P-002** starts, re-scoped per §0
4. **P-003-V** runs in parallel with any of the above — it depends on nothing

### P-002 re-scope

Not "build a pipeline." The real delta is now: **write `deny.toml`** (without it `cargo-deny` cannot
run at all), **wire `cargo-deny` into the existing job**, and **reconcile the four dependency
numbers across ecosystems**. Do **not** add CodeQL. Do **not** duplicate the existing Security Audit
job.

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

## §12 Al's Track — decisions, no code

1. ☐ **Confirm the clone** — `-network` on three signals. 30 seconds. Every verdict depends on it.
2. ☐ **GATE-1 with Sarah** — live testnet state, and therefore reset vs. migration.
3. ☐ **GATE-3 to Sarah** — the three fork-choice options in §4, with the Lean-proof interaction
   flagged explicitly.
4. ☐ **Reconcile the Lean count** — is 586 actually Eigenverse's number?
5. ☐ Clean up or rename the second clone.
6. ☐ **Review the dark window** once P-002-H reports it — if anything merged blind, decide whether
   it needs re-verification now that the audit job runs again.
