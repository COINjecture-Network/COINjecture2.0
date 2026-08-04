# REGISTRY — DARQ AGI Mode / COINjecture 2.0

**41 findings / 18 root causes.**

Schema authority: `loop/LOOP_SPEC.md` §7. Root-cause map: §7 canonical table (v1.3).
Verified at: `28c50a122f2caab70582e8215b670b0ddc4d236d` unless a row says otherwise.

> Supersedes the provisional 19-row schema the Cycle 0 builder designed when §7 was unavailable.
> DARQ-IDs are **one per root cause** (the §7 example maps `DARQ-001 → RC-01 → C1, C2`).
> Exceptions: `DARQ-019` is a composition spanning two root causes; `DARQ-020`, `DARQ-022`,
> `DARQ-023` and `DARQ-024` are **operational** findings with no code root cause. Each carries its
> own ID per §8 without inventing a 19th root cause.

## Verdict vocabulary (§7 — use exactly these)

`CONFIRMED` · `DRIFTED` (record new location) · `ALREADY-FIXED` (record the SHA) ·
`NOT-FOUND` (false positive — say why) · `NEEDS-HUMAN` (unsettleable by reading code)

A `NOT-FOUND` is a **valuable** result.

## Register

| DARQ-ID | Title | Sev | Category | RC | Third-party | Location | Verdict | Status | Packet |
|---|---|---|---|---|---|---|---|---|---|
| DARQ-001 | Consensus trusts miner-supplied work parameters | Critical | Logic | RC-01 | C1, C2 | `node/src/validator.rs:137-143`; `core/src/validation.rs:341-346`; `node/src/validator.rs:158`, `:179`; `node/src/service/fork.rs:463`; `node/src/chain.rs:476` | **CONFIRMED** | Open | P-004 / P-004-D |
| DARQ-002 | Validation bypass on direct apply paths | Medium | Logic | RC-02 | M3 | UNVERIFIED | — | Open | P-005 |
| DARQ-003 | Dual bincode/JSON hashing — acceptance ambiguity | Medium | Serde | RC-03 | M7 | UNVERIFIED | — | Open | P-004 |
| DARQ-004 | Inconsistent address derivation (×3) | Critical | Crypto | RC-04 | C3 | raw `core/src/types.rs:46-48`; SHA-256 `wallet/src/keystore.rs:416`, `node/src/genesis.rs:37-43`; BLAKE3 `node/src/keystore.rs:108-113`, `state/src/escrows.rs:469-474` | **CONFIRMED** | Open | P-003 |
| DARQ-005 | Ledger apply path performs no validation | Critical | Integer | RC-05 | C4, C5, C6, M4, NEW-1 | UNVERIFIED (NEW-1 observed at `rpc/src/server.rs:1583`) | — | Open | P-005 |
| DARQ-006 | Unauthenticated state-mutating endpoints | Critical | Auth | RC-06 | C7, H8 | UNVERIFIED (C7 shape observed at `rpc/src/server.rs:1556`+) | — | Open | P-006 |
| DARQ-007 | Client IP taken from spoofable headers | High | Auth | RC-07 | H10 | UNVERIFIED | — | Open | P-006 |
| DARQ-008 | Escrow signature verification incomplete | High | Crypto | RC-08 | H6, H7 | UNVERIFIED | — | Open | P-007 |
| DARQ-009 | Gossip accepted without verified sender | High | P2P | RC-09 | H1, H2, M8 | UNVERIFIED | — | Open | P-008 |
| DARQ-010 | Unbounded peer / mempool ingress | High | P2P | RC-10 | H3, H4, H5, M9 | UNVERIFIED | — | Open | P-008 |
| DARQ-011 | Missing rate limits on public API | High | Auth | RC-11 | H11, L5 | UNVERIFIED | — | Open | P-009 |
| DARQ-012 | Attacker-drivable disk / log amplification | Medium | Panic | RC-12 | NEW-2, L3 | `node/src/validator.rs:102-130` (NEW-2, observed) | — | Open | P-009 |
| DARQ-013 | Panic on malformed consensus input | Medium | Panic | RC-13 | M1, M2, M6 | UNVERIFIED | — | Open | P-010 |
| DARQ-014 | Non-atomic multi-table writes | Medium | Logic | RC-14 | M5 | UNVERIFIED | — | Open | P-010 |
| DARQ-015 | Query injection via unencoded interpolation | High | Auth | RC-15 | H9 | UNVERIFIED | — | Open | P-009 |
| DARQ-016 | Information disclosure / IDOR | Medium | Auth | RC-16 | M10 | UNVERIFIED | — | Open | P-009 |
| DARQ-017 | Key and credential handling weaknesses | Low | Crypto | RC-17 | L1, L2 | UNVERIFIED | — | Open | P-010 |
| DARQ-018 | Unchecked time / expiry arithmetic | Low | Integer | RC-18 | L4 | UNVERIFIED | — | Open | P-010 |
| DARQ-019 | Disk-exhaustion DoS composed from NEW-2 × H11 | **High (PROVISIONAL)** | Composition | RC-11 × RC-12 | NEW-3 | composition of `node/src/validator.rs:102-130` and the unrated `/node-rpc` route | — | Open | P-009 |
| DARQ-020 | Floating CI toolchain silently disables the security gate | Operational | SupplyChain | — (operational) | NEW-4 | `.github/workflows/ci.yml` `RUST_TOOLCHAIN: stable` + `needs: lint` fan-out | **CONFIRMED** | In Progress | P-002-H |
| DARQ-021 | Block validation checks the signature but not the sender binding; `is_valid()` is dead on the block path | **Critical (UNSIZED)** | Auth | RC-02 (extends) | NEW-5 | `node/src/validator.rs:169` and `mempool/src/pool.rs:160` call `verify_signature()`; the binding lives only in `Transaction::is_valid()` (`core/src/transaction.rs:437-439`), reached only from `core/src/block.rs:215`, which `node/src` never calls | **NEEDS-HUMAN** (needs its own packet) | Open | **P-021-V** → P-021 |
| DARQ-022 | Dangling `latest-upstream` submodule pin aborts every Dependabot run — updater dark across all ecosystems | Operational | SupplyChain | — (operational) | NEW-6 | `.gitmodules` `latest-upstream` → `Quigles1337/COINjecture2.0` pinned at `6a32fbfc7094fe82c02a91b231b52798c9f42972`, unreachable in that remote | **CONFIRMED** | Open | P-002 |
| DARQ-023 | npm had no Dependabot entry at all while holding 21 of 23 open alerts | Operational | SupplyChain | — (operational) | NEW-7 | `.github/dependabot.yml` — cargo/github-actions/docker configured, **npm absent**; manifests at `web-wallet/package.json`, `web/coinjecture-evolved-main/package.json` | **CONFIRMED** | In Progress | **P-023** (PR #56) |
| DARQ-024 | Python dependencies are wholly ungoverned — third-party imports with no manifest anywhere | Operational | SupplyChain | — (operational) | NEW-8 | 17 `.py` files under `scripts/` and `tests/harness/` import `requests` and `huggingface_hub`; **no `requirements.txt`, `pyproject.toml`, `setup.py`, `Pipfile` or lockfile exists in the repo** | **CONFIRMED** | Open | **unassigned — Al's decision** |

## Codex program cross-reference

| DARQ-ID | Codex program |
|---|---|
| DARQ-001, 002, 003, 004, 005, 006, 008, 013, 015 | P1 — authoritative deterministic transition validation |
| DARQ-005, 014 | P2 — journaled atomic state and reorg handling |
| DARQ-007, 010, 011, 012 | P3 — bounded ingress |
| DARQ-017 | P4 — fail-closed configuration and key custody |
| DARQ-009 | P5 — one authenticated peer transport |

Codex SEC-PR status: SEC-PR-001 built as `fix/api-jwt-fail-closed` @ `ff6e65c4`, **unmerged**
(not an ancestor of `main`). SEC-PR-002…005 are **unimplemented specs**. SEC-PR-002 is now scoped
into P-006 alongside C7, per §5 merge rationale.

## Coverage

| | Count |
|---|---|
| Root causes with a verified Location | **3** of 18 (DARQ-001, 004, and partially 012) + DARQ-020, 022, 023, 024 (operational) |
| Findings covered by those | 9 of 41 (C1, C2, C3, NEW-2, NEW-4, NEW-5, NEW-6, NEW-7, NEW-8) |
| Rows carrying `UNVERIFIED` | 15 |

Only C1/C2/C3 were assigned for verification in Cycle 0. Everything else is seeded from audit text
and is **not** independently confirmed at this HEAD.

## Notes

**DARQ-001** — C1 and C2 share a root cause but not a fix. C1 is implementable: a deterministic
generator already exists at `consensus/src/miner.rs:663`, called only from mining paths
(`miner.rs:824`, `node/src/service/mining.rs:23`, `node/src/service/mod.rs:1019`). The blocker is
structural — it is an `async` method borrowing `self.difficulty_adjuster`, so it must be lifted to a
free function over `(height, prev_hash, difficulty_params)` and the difficulty input made
consensus-visible, or validators regenerate a different instance than the miner did. C2 has no
implementable fix until GATE-3 resolves: `solve_time` is miner wall-clock and is not verifiable by
any other node. `WorkScoreCalculator` (`consensus/src/work_score.rs:94`) exists but its only
non-test consumer is the miner computing the value it self-declares. Hence the P-004 / P-004-D split.

**DARQ-004** — the audit reported two derivations; there are **three**, and the one on the consensus
transaction path (`core/src/transaction.rs:438, 510, 589, 664, 798, 931, 1100`, all
`if self.from != self.public_key.to_address()`) is the **raw** variant. The validator's own keystore
address is BLAKE3, so a validator's on-chain identity is not the address the tx path derives for it.
`Address::from_pubkey` is the natural canonical helper and is already public API, but only `core`
calls it — fix shape is one helper plus four open-coded call sites. Genesis- and consensus-breaking.
The audit's cited SHA-256 site `core/src/crypto.rs:423` **does not exist** (file is 397 lines).

**DARQ-005** — NEW-1 (`rpc/src/server.rs:1583`, `let new_balance = current_balance + bounty;`) is an
uncited instance of the C5 class on an RPC payout path, outside C5's stated locations. Per D3, the
fix is `checked_add`/`checked_sub` returning errors — **never `saturating_*` on balances**.

**DARQ-006** — do not treat SEC-PR-002 as covering C7. The transport bearer-key gate in
`rpc/src/middleware.rs` authenticates the *connection*; C7 is missing per-*account* authorization in
`rpc/src/server.rs`. After SEC-PR-002 lands, any valid key holder can still name an arbitrary
`solver` and take the bounty.

**DARQ-019** — registered with its own ID per §8 but assigned **no new root cause**; it is the
composition of RC-11 and RC-12. Fixing either half mitigates it; H11 (rate limiting) is the cheaper
half. The `/node-rpc` 64 MiB body limit and 300 s timeout are carried from the third-party report
and are **not** independently verified at this HEAD — verify before scoping P-009.

**DARQ-020** — NEW-4, found while landing P-000. `ci.yml` sets `RUST_TOOLCHAIN: stable` (floating)
and runs `clippy -- -D warnings`; `test`, `build` and `security` all declare `needs: lint`. An
upstream lint release therefore darkens the security gate with zero code changes. Confirmed
empirically: a PR touching **zero** `.rs` files fails Lint while `main`'s last run was green.
No code root cause — it is a pipeline-topology defect. Closed by P-002-H + D11.

**DARQ-021** — found during P-003-V, not sought. `Transaction::verify_signature()` checks only the
Ed25519 signature; the `from == public_key.to_address()` binding is in `Transaction::is_valid()`.
Runtime probe: a transfer naming an arbitrary **victim** as `from`, signed by an attacker key,
returns `verify_signature = true`. Block validation (`node/src/validator.rs:169`) and mempool
admission (`mempool/src/pool.rs:160`) both call only `verify_signature()`. `Block::verify()` —
the one path that calls `is_valid()` — is **never invoked anywhere in `node/src`**; every ingest
route uses `validate_block_with_options`. Adjacent to M3 but distinct: M3 is *callers bypassing the
validator*, this is *the validator performing the weaker of two available checks*, so fixing M3 as
written would not fix it. **Not traced to the apply path and deliberately unsized** — needs its own
verification packet before anyone assigns severity. If it holds it outranks C3 and expands P-005.

**DARQ-022** — NEW-6, found during Phase −1 while enumerating CI runs on `main` for an unrelated
reason. `.gitmodules` registers `latest-upstream` → `Quigles1337/COINjecture2.0`; the tree pins it at
`6a32fbfc7094fe82c02a91b231b52798c9f42972`, which **does not exist in that remote** (`git ls-remote`
does not list it). Dependabot clones with submodules and therefore fails at clone time — *before* any
ecosystem update logic runs — which is why the failure is total rather than ecosystem-specific.
Confirmed failing across **npm_and_yarn and cargo**, in `/web-wallet`, `/web/coinjecture-evolved-main`
and `/.`, on every run from at least 2026-07-13 through 2026-08-02.

**Consequence:** no Dependabot fix PRs are being generated — the open-PR list contains zero
(only #46, #48, #54, #55, all Al's). This is a strong candidate explanation for the 18–19 open alerts.
⚠️ **Those counts are stale by an unknown margin** and must not be reconciled against until the
updater runs. No code root cause — like DARQ-020 it is a pipeline-topology defect, and it is the same
*class* of failure: an unrelated fault silently disabling a security mechanism while the workflow
appears to run. Assigned to P-002; fix shape needs Al's decision (§8).

**Audit coordinate reliability** — both audits' *findings* have held; their *coordinates* have not.
Treat every `UNVERIFIED` Location as a hypothesis, not a fact.

**DARQ-023** — NEW-7, found during P-023's STEP 1 inventory [D4]. `.github/dependabot.yml` configured
cargo, github-actions and docker, but **not npm** — while npm held **21 of the 23 open alerts**
(10 in `web-wallet/package-lock.json`, 11 in `web/coinjecture-evolved-main/package-lock.json`).

**Why it stayed invisible:** npm *security* updates were already running — that is why failing runs
are named `npm_and_yarn in /web-wallet` despite npm being absent from the config. Security updates do
not require an `updates` entry; **version updates do.** So the ecosystem looked covered while the
routine minor/patch bumps that would have pre-empted several of those 21 alerts were never generated.
**"Dependabot is running for npm" and "npm is configured" were different claims, and only the first
was true.** Closed by P-023 (PR #56).

**DARQ-024** — NEW-8, found during the same inventory. 17 Python files under `scripts/` and
`tests/harness/` import third-party packages (`requests`, `huggingface_hub`), and **no dependency
manifest of any kind exists anywhere in the repo** — no `requirements.txt`, `pyproject.toml`,
`setup.py`, `Pipfile`, or lockfile.

**Not closable by configuration.** With no manifest there is nothing for Dependabot to parse, so no
`pip` entry is possible in `dependabot.yml`; P-023 could not fix this and did not try. The asymmetry
is the finding: **CodeQL scans the Python *code* (it is one of the four configured Analyze jobs),
while nothing at all watches the Python *dependencies*.** Python is currently the only ecosystem in
this repo with **zero** dependency governance — not weak governance, none.

Creating a manifest is a real change: it pins versions that are currently floating implicitly, and it
can affect both the HuggingFace scripts and the test harness. **That makes it its own packet, and the
decision is Al's** — the options are to add a manifest and configure `pip`, to declare the scripts
out of scope and record it as accepted risk, or to vendor them elsewhere.

**Provenance note — the operational findings keep arriving sideways.** DARQ-020 was found while
landing a docs-only PR; DARQ-021 while probing C3; DARQ-022 while listing CI runs during crash
recovery; DARQ-023 and DARQ-024 while inventorying manifests for an unrelated config packet. **Five
for five, none was found by looking for it** — which is itself the finding. Nothing in this repo
watches for gaps in its own supervision, so every one of them has surfaced as a side effect of
someone doing something else. That is not a sustainable detection strategy, and it is worth a packet
of its own.
