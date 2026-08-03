# REGISTRY — DARQ AGI Mode / COINjecture 2.0

**37 findings / 18 root causes.**

Schema authority: `loop/LOOP_SPEC.md` §7. Root-cause map: §7 canonical table (v1.2).
Verified at: `28c50a122f2caab70582e8215b670b0ddc4d236d` unless a row says otherwise.

> Supersedes the provisional 19-row schema the Cycle 0 builder designed when §7 was unavailable.
> DARQ-IDs are **one per root cause** (the §7 example maps `DARQ-001 → RC-01 → C1, C2`).
> Two exceptions: `DARQ-019` is a composition spanning two root causes, and `DARQ-020` is an
> operational finding with no code root cause. Both carry their own ID per §8 without inventing a
> 19th root cause.

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
| Root causes with a verified Location | **3** of 18 (DARQ-001, 004, and partially 012) + DARQ-020 (operational) |
| Findings covered by those | 5 of 37 (C1, C2, C3, NEW-2, NEW-4) |
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

**Audit coordinate reliability** — both audits' *findings* have held; their *coordinates* have not.
Treat every `UNVERIFIED` Location as a hypothesis, not a fact.
