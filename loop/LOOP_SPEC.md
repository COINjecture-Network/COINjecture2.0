# DARQ AGI Mode — COINjecture 2.0 Remediation Loop

**Spec v1.1** · Supersedes v1.0 · Target: `COINjecture-Network/COINjecture2.0` @ `28c50a12`
**Commit to `loop/LOOP_SPEC.md` as packet P-000, before anything else.**

---

## §0 Changelog — what v1.0 got wrong

Recorded rather than quietly patched, because these are the kind of errors that recur.

| v1.0 claim | Reality (per C0 verification) | Fix in v1.1 |
|---|---|---|
| SEC-PR-002…005 are existing code to dedupe against | They are **specs** in `first-five-prs.md` marked *do not implement without authorization*. Only SEC-PR-001 is built, and it is unmerged. `main` carries zero security remediation. | §2 corrected; P-006 now *implements* SEC-PR-002 rather than reconciling with it |
| Baseline: 951 tests, 586 Lean theorems | **936 / 89** at `28c50a12`, suite green. No counting convention reaches 586 from this tree. | Baseline re-anchored to the SHA; §2 |
| ~14 dependency vulnerabilities | **2** (`cargo audit`), tooling already installed | P-002 destaged — but see the revised caveat, which moved rather than vanished |
| The prompt in §8 is sufficient | The builder never received §7, so it designed the registry schema itself and invented its own packet numbering | **P-000 commits the spec.** Future prompts reference it by path instead of re-pasting. |
| Cluster count should be ~14–15 | I did the clustering myself and got **18**. The builder's 19 was fine. | §7 carries the canonical 18-row map; the pushback is withdrawn |

**The structural lesson:** a prompt that is not self-contained *and* a spec that is not committed
leaves the builder inventing schema. P-000 exists to end that permanently.

---

## §1 Seat Configuration

| Seat | Who | Role |
|---|---|---|
| **BUILDER** (empirical) | Claude Code | Only seat with repo access. Ground truth for the others. |
| **ADVERSARY** | Pre-loaded (Codex + third-party), re-invoked on demand | **Fix-verification only** — pointed at a patch, never at the codebase. |
| **SYNTHESIS** | Claude / Opus | Arbitrates, rules on flags, holds gates, writes go/no-go. |

Adversary pass is **mandatory** for P-003, P-004, P-005 (consensus / address / ledger).
**Skipped** elsewhere — logged in the LEDGER as a deliberate deviation with a one-line reason.

---

## §2 Ground Truth — anchored to `28c50a12`

**The repo at this SHA is ground truth. Numbers carried in memory are not.**

- Clone: `COINjecture2.0-network` (org remote). HEAD and `Cargo.lock` SHA-256 both match the Codex
  scan baseline exactly — zero drift.
- **936** tests passing / 0 failed / 4 ignored · **89** Lean theorems across 19 files · **15** crates
- Lean count is **unreconciled**, not corrected. 586 appears nowhere in this tree. One untested
  hypothesis: 586 may be *Eigenverse's* count, mislabelled as COINjecture's. Do not adopt either
  number until someone checks.

**⚠️ Clone hazard.** A second clone at `C:\Users\LEET\COINjecture2.0` (Quigles1337 remote) is also
v4.8.4 / 15 crates, but is **dirty with 20 live worktrees and has no `lean4/`**. It is a
verdict-against-the-wrong-tree waiting to happen. Clean it up or rename it unmistakably.

**Remediation state on `main`: zero.** SEC-PR-001 exists as branch `ff6e65c4`, not an ancestor of
main. SEC-PR-002…005 are unimplemented specs.

**Audit reliability, now calibrated.** Both audits' *findings* have held; their *coordinates* have
not. The third-party report cites `core/src/crypto.rs:423` in a 397-line file, and missed a third
address derivation entirely. **Every uncited specific in either report is a hypothesis.**

---

## §3 Guardrails

**D1 — Approved packets only.** One packet per branch, per cycle.

**D2 — Integer money only.** Any `f64` near a balance, fee, or supply figure is a Critical you
created. *(Note: `work_score` is `f64` today — see P-004-D.)*

**D3 — Checked arithmetic on money and nonce paths.** `checked_add` / `checked_sub` returning
errors. **Never `saturating_*` on balances** — saturation silently destroys supply.

**D4 — Never set a CI gate below the measured baseline.** Inventory first, gate at the measured
value, tighten as its own PR. This is the v1.0 caveat generalised: it was never really about
`cargo audit`, it was about clippy and geiger too. See P-002.

**D5 — Consensus-affecting changes are hard forks.** Gated (§4).

**D6 — No deployment.** Box C access unresolved; parked as a known gap. Repo work only.

**D7 — One root cause per packet.** Report "N findings / M root causes."

**D8 — Small reviewable changes**, green against baseline 936.

**D9 — Windows-authored scripts need the exec bit set in the git index and must be invoked through
`bash`.** BEANlet lost a hosted-CI run to exactly this (`exit 126`), and it was the third defect in
that arc that local green never caught. Al develops on Windows; CI runs on Linux.

**D10 — PRs, not direct-to-main.** BEANlet's C0 committed to main because that repo was empty.
COINjecture is shared with Sarah and has an established PR workflow.

---

## §4 Gates

### GATE-1 — C3 is genesis- *and* consensus-breaking ⚠️ (escalated since v1.0)

Verification found **three** derivations, not two:

| Derivation | Site | Notes |
|---|---|---|
| Raw 32-byte pubkey | `core/src/types.rs:46` | **on the consensus tx path** |
| SHA-256(pubkey) | wallet + genesis | |
| BLAKE3(pubkey) | validator's own keystore | **missed by the audit entirely** |

`Address::from_pubkey` is the natural canonical helper and is already public — but only `core` calls
it. Fix shape: **one helper, four open-coded call sites.**

**Al + Sarah must decide:** is there live testnet state with balances anyone cares about? Chain
reset (clean, correct, pre-mainnet, costs history) or migration (preserves state, materially more
code and risk)?

**Cheapest confirmation, no code:** try to spend from a genesis-allocated address. One transaction.

### GATE-2 — C1 is a hard fork

Changing block validation means old and new nodes disagree. Coordinated restart, not rolling
upgrade. Confirm testnet topology before P-004 opens.

### GATE-3 — C2 is a protocol design decision, not a patch ⚠️ NEW

The audit says *"recompute work score from verified inputs."* **There are no verified inputs.**
`solve_time` is miner wall-clock; no other node can check it. Current validation is exactly
`is_finite() && >= 0`, with `min_work_score: 0.0`. A `WorkScoreCalculator` exists, but its only
non-test consumer is the miner computing the value it then self-declares.

Options, as input to Sarah — not a recommendation:

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

## §5 Packet Queue (canonical numbering — supersedes anything the C0 builder invented)

| ID | Scope | Gate | Status |
|---|---|---|---|
| **P-000** | Commit `loop/` scaffolding + `LOOP_SPEC.md` | none | **Do first** — trivial, unblocks self-bootstrapping |
| **P-001** | Registry + verification of C3/C1/C2 | — | ✅ complete (Cycle 0) |
| **P-002** | CI security pipeline + `deny.toml` | none | **Ready — next build packet** |
| **P-003** | C3 address derivation unification | **GATE-1** | Blocked. Adversary pass mandatory. |
| **P-004** | C1 canonical problem regeneration | **GATE-2** | Blocked. Adversary pass mandatory. |
| **P-004-D** | C2 fork-choice metric — **design packet** | **GATE-3** | Blocked on Sarah. Not a build packet. |
| **P-005** | C4, C5, C6, M4 + `rpc/src/server.rs:1583` — ledger apply path | none | Ready after P-002. Adversary mandatory. |
| **P-006** | SEC-PR-002 transport gate **+** C7 per-account authz **+** H8, H10 | none | Ready. One PR — same files, same concern. |
| **P-007** | H6, H7 — escrow signature verification | none | |
| **P-008** | H1–H5, M8, M9 — gossip auth + bounded ingress | none | Codex "bounded ingress" (~25) |
| **P-009** | H9, H11, M10 + telemetry amplification | none | Includes DARQ-NEW-3 (§8) |
| **P-010** | M1, M2, M5, M6, L1–L5 — sweep | none | |
| **P-011** | ~668 `unwrap`/`expect` on reachable paths | none | Network layer alone: 329 + 28 |

**P-004 split rationale:** C1 is implementable — a deterministic generator already exists at
`miner.rs:663`; the blocker is structural (it's an async method borrowing the miner's difficulty
adjuster, so it must be lifted to a free function and the difficulty input made consensus-visible).
C2 has no implementable fix until GATE-3 resolves. Bundling them would block a solvable packet
behind an unsolvable one.

**P-006 merge rationale:** SEC-PR-002 fixes the transport bearer-key gate in `middleware.rs`; C7 is
missing per-account authorization in `server.rs`. **An API key authenticates the connection, not the
account** — after SEC-PR-002, any valid key holder can still name an arbitrary solver and take the
bounty. Filing C7 as covered would silently drop a Critical. Same files, same concern, one PR.

---

## §6 Phase Protocol

`loop/STATE.md` carries `CYCLE`, `PHASE`, `PACKET`, `BRANCH`, `CAPACITY_FLAG`.

| Phase | Meaning | Exit |
|---|---|---|
| **A** | Reconcile / verify — read-only | Registry written, STOP + report |
| **B** | Build — active packet on a branch | Green build + tests, diff ferried |
| **C** | Adversary — fix-verification only | BLOCK / MERGE-WITH-NOTES / CLEAN |
| **D** | Synthesis — Al + Opus | Gate ruling, LEDGER entry, merge or return to B |

Phase C is conditional (P-003, P-004, P-005 only). `CAPACITY_FLAG: remediation-priority` STOPs the
loop.

---

## §7 Registry Schema + Canonical Root-Cause Map

### Schema (the §7 that was missing)

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
Notes         | <free text — audit citation errors, composition risks, blockers>
```

**Verification verdicts — use exactly these, never guess:**
`CONFIRMED` · `DRIFTED` (record new location) · `ALREADY-FIXED` (record the SHA) ·
`NOT-FOUND` (false positive — say why) · `NEEDS-HUMAN` (unsettleable by reading code)

A `NOT-FOUND` is a **valuable** result. Human audits fail by under-reporting false positives just as
automated ones fail by over-reporting them.

### Canonical root-cause map — 36 findings / 18 root causes

| RC | Root cause | Findings | Packet |
|---|---|---|---|
| RC-01 | Consensus trusts miner-supplied work parameters | C1, C2 | P-004 / P-004-D |
| RC-02 | Validation bypass on direct apply paths | M3 | P-005 |
| RC-03 | Dual bincode/JSON hashing — acceptance ambiguity | M7 | P-004 |
| RC-04 | Inconsistent address derivation (×3) | C3 | P-003 |
| RC-05 | Ledger apply path performs no validation | C4, C5, C6, M4, **NEW-1** | P-005 |
| RC-06 | Unauthenticated state-mutating endpoints | C7, H8 | P-006 |
| RC-07 | Client IP taken from spoofable headers | H10 | P-006 |
| RC-08 | Escrow signature verification incomplete | H6, H7 | P-007 |
| RC-09 | Gossip accepted without verified sender | H1, H2, M8 | P-008 |
| RC-10 | Unbounded peer / mempool ingress | H3, H4, H5, M9 | P-008 |
| RC-11 | Missing rate limits on public API | H11, L5 | P-009 |
| RC-12 | Attacker-drivable disk / log amplification | **NEW-2**, L3 | P-009 |
| RC-13 | Panic on malformed consensus input | M1, M2, M6 | P-010 |
| RC-14 | Non-atomic multi-table writes | M5 | P-010 |
| RC-15 | Query injection via unencoded interpolation | H9 | P-009 |
| RC-16 | Information disclosure / IDOR | M10 | P-009 |
| RC-17 | Key and credential handling weaknesses | L1, L2 | P-010 |
| RC-18 | Unchecked time / expiry arithmetic | L4 | P-010 |

---

## §8 New Findings — neither audit saw these

**DARQ-NEW-1** · Medium · Integer · `rpc/src/server.rs:1583`
Uncited instance of the C5 unchecked-arithmetic class. Folds into RC-05 / P-005.

**DARQ-NEW-2** · Medium alone · Panic/Logic · `node/src/validator.rs:102–130`
Debug telemetry writes JSON to disk on **every bad-parent block**. Attacker-drivable.

**DARQ-NEW-3** · **High** · Composition · `NEW-2` × `H11`
Neither audit connected these because neither was holding both halves.

> `/node-rpc` accepts **64 MiB** `chain_submitBlock` bodies with a **300-second timeout** and
> **no rate limiting** (H11). Every bad-parent block triggers a **disk write** (NEW-2).
>
> ⇒ Unauthenticated remote attacker → unlimited 64 MiB submissions → unbounded disk writes.
> **Disk-exhaustion DoS assembled from two separately-filed findings.**

Register NEW-3 with its own ID. Fixing either half mitigates it; fixing H11 is cheaper.

---

## §9 Dependency Baseline

`cargo-audit 0.22.2` and `cargo-deny 0.20.2` installed. **No `deny.toml`, so deny cannot run.**

`cargo audit` → 2 vulnerabilities + 3 warnings:

- **RUSTSEC-2026-0185** — `quinn-proto`, High 7.5. **QUIC — your network layer, on a P2P chain.**
  This advisory postdates my knowledge cutoff. **Pull the advisory text before triaging; do not let
  me or anyone else hand-wave its severity.**
- **RUSTSEC-2026-0204** — `crossbeam-epoch`

Note: ~14 Dependabot alerts → 2 `cargo audit` findings. Different scanners, different graphs — but
that is counts inflating for the third time. Reconcile the two lists in P-002 rather than assuming
either is complete.

---

## §10 PHASE B — P-002 BUILDER PROMPT

Paste into Claude Code. **Run P-000 first** (see §11).

```text
You are the BUILDER seat of the DARQ AGI Mode loop for COINjecture 2.0.
Read loop/LOOP_SPEC.md first — it is committed and it binds you. This prompt
adds packet-specific scope only.

PACKET: P-002 — CI security pipeline + deny.toml
CYCLE: 1 · PHASE: B · BRANCH: feat/p002-ci-security-pipeline

STEP 0 — STATE DETECTION
- If loop/STATE.md says PHASE: B with PACKET: P-002 — resume from BRANCH.
- If PHASE: A and P-000 is not merged — STOP. P-000 must land first.
- If CAPACITY_FLAG: remediation-priority — STOP.
- Confirm the clone is COINjecture2.0-network at 28c50a12 (or a descendant).
  If the remote is Quigles1337, STOP — that is the wrong clone.
- Otherwise: create feat/p002-ci-security-pipeline and set PHASE: B.

STEP 1 — READ EXISTING CI BEFORE WRITING ANY
PR #47 was a CI fix that merged; CI already exists. Read every workflow under
.github/workflows/ and report what already runs. You are ADDING to a working
pipeline, not replacing it. If a gate already exists, do not duplicate it.

STEP 2 — MEASURE THE BASELINE BEFORE SETTING ANY GATE  [D4 — the core of this packet]
Do NOT assume any gate can be set to zero. This repo is ~87k LOC of existing
code; BEANlet's zero-unsafe rule was a greenfield rule and does not transfer.
Measure first, then gate at the measured value.

Report actual current counts for:
  (a) cargo clippy --all-targets --all-features 2>&1 | count warnings
      — by lint, top 10. On 87k LOC this may be in the hundreds.
  (b) cargo geiger — is it even installed? If not, say so; do not install
      without authorization. If installed, report unsafe expression counts
      per workspace crate, separating first-party from dependencies.
  (c) cargo fmt --check — how many files would change?
  (d) cargo test — confirm 936 passing / 0 failed / 4 ignored.
  (e) Is Cargo.lock committed?

For each gate, state explicitly: CAN-ENFORCE-NOW (already clean) or
BASELINE-ONLY (record the number, enforce no-regression, tighten later).
A gate set below the current baseline turns CI red on day one and makes every
later PR fight a red baseline. That is the failure mode this step prevents.

STEP 3 — TRIAGE THE TWO ADVISORIES
For RUSTSEC-2026-0185 (quinn-proto) and RUSTSEC-2026-0204 (crossbeam-epoch),
report from the local advisory DB:
  - direct or transitive dependency? which crate pulls it?
  - is there a patched version, and what is the semver distance?
  - what does the advisory actually describe? Quote the summary.

Then apply this rule and say which branch you took:
  - PATCH-level bump available AND tests stay green → take it, in this PR.
  - MINOR or MAJOR bump → DO NOT take it. Report the upgrade path and stop.
    quinn-proto is QUIC on a P2P chain's network layer; that bump deserves
    its own review, not a ride-along in a CI PR.
  - No patched version → deny.toml ignore with a written reason AND an
    expiry date. Never a silent ignore.

Separately: reconcile the ~14 open Dependabot alerts against these 2 findings.
Different scanners see different graphs. Report which alerts cargo-audit does
not see and why (transitive-only? different manifest? already patched?).

STEP 4 — WRITE deny.toml
Cover all four sections: advisories, bans, licenses, sources.
LICENSES IS THE TRAP: an allowlist written from intuition will flag dozens of
transitive crates and turn CI red immediately. Enumerate the licenses actually
present in the dep graph FIRST, then write the allowlist from that set, then
flag anything genuinely objectionable (AGPL, unlicensed, unknown) as findings
for Al — do not silently allow them and do not silently ban them.

STEP 5 — LAND THE PIPELINE
Add the security gates to CI per the measured baselines from STEP 2.
Enforce what is clean; record-and-no-regress what is not.

D9 IS NOT OPTIONAL: any script you author on Windows needs its exec bit set
in the git index (git update-index --chmod=+x) and must be invoked through
bash in the workflow. BEANlet lost a hosted run to exit 126 on exactly this.

CI must be green ON THE HOSTED RUNNER, not just locally. Push and confirm.
Three defects in the BEANlet arc were invisible to local green.

STEP 6 — REPORT
Write loop/reports/C1-builder.md:
  1. What CI already did before you touched it
  2. Baseline table: each gate, measured value, CAN-ENFORCE-NOW or BASELINE-ONLY
  3. Advisory triage + which rule branch you took + Dependabot reconciliation
  4. deny.toml decisions, especially the license set and anything flagged
  5. Hosted run ID and job status
  6. 2-4 things you want a second opinion on rather than let stand
  7. What you did NOT do and why

Ferry back C1-builder.md and C1-diff.patch. Open the PR as a draft; do not
merge. Set PHASE: D.

GUARDRAILS
- CI and deny.toml only. No src/ changes. The ONLY permitted Cargo.toml /
  Cargo.lock change is a patch-level bump per STEP 3, and you must say so.
- No gate below the measured baseline [D4].
- Draft PR, no merge, no direct-to-main [D10].
- Do not install tooling without authorization.

STOP CONDITIONS — report rather than work around
- Wrong clone, dirty worktree, or unexpected HEAD
- Test baseline differs materially from 936
- Clippy or geiger baseline so large the packet needs re-scoping
- An advisory needs a MINOR/MAJOR bump
- Any gate cannot be made green on the hosted runner
```

---

## §11 Al's Track — decisions, no code

1. ☐ **Confirm the clone** — `-network` looks right on all three signals. 30 seconds. Every verdict
   depends on it.
2. ☐ **Authorize P-000** — commit `loop/` + this spec **as a PR** [D10]. Trivial, and it ends the
   re-pasting problem permanently.
3. ☐ **GATE-1 with Sarah** — live testnet state? Plus the genesis spend test. One transaction,
   highest-value move available this week.
4. ☐ **GATE-3 to Sarah** — hand her the three fork-choice options in §4 as a design question. Flag
   the Lean-proof interaction explicitly.
5. ☐ **Reconcile the Lean count** — is 586 actually Eigenverse's number?
6. ☐ Clean up or rename the second clone before it bites someone.
