# PACKETS — live status tracker

**Authority: `loop/LOOP_SPEC.md` §5.** That table is canonical. This file tracks *state*, not scope.
The packet numbering the Cycle 0 builder invented (its own P-001/P-002/P-003) is **retired** —
superseded by §5.

| ID | Scope | Gate | Status | Branch / PR |
|---|---|---|---|---|
| **P-000** | Commit `loop/` scaffolding + `LOOP_SPEC.md` | none | 🔵 **Draft PR open** — awaiting Al | `feat/p000-loop-scaffolding` |
| **P-001** | Registry + verification of C3/C1/C2 | — | ✅ **Complete** (Cycle 0) | — (read-only, no branch) |
| **P-002** | CI security pipeline + `deny.toml` | none | ⛔ **Blocked on P-000 merging** | `feat/p002-ci-security-pipeline` |
| **P-003** | C3 address derivation unification | **GATE-1** | ⛔ Blocked. Adversary mandatory. | |
| **P-004** | C1 canonical problem regeneration | **GATE-2** | ⛔ Blocked. Adversary mandatory. | |
| **P-004-D** | C2 fork-choice metric — design packet | **GATE-3** | ⛔ Blocked on Sarah. Not a build packet. | |
| **P-005** | C4, C5, C6, M4 + NEW-1 — ledger apply path | none | ⚪ Ready after P-002. Adversary mandatory. | |
| **P-006** | SEC-PR-002 transport gate + C7 authz + H8, H10 | none | ⚪ Ready. One PR. | |
| **P-007** | H6, H7 — escrow signature verification | none | ⚪ | |
| **P-008** | H1–H5, M8, M9 — gossip auth + bounded ingress | none | ⚪ | |
| **P-009** | H9, H11, M10 + telemetry amplification (incl. NEW-3) | none | ⚪ | |
| **P-010** | M1, M2, M5, M6, L1–L5 — sweep | none | ⚪ | |
| **P-011** | ~668 `unwrap`/`expect` on reachable paths | none | ⚪ | |

Legend: ✅ complete · 🔵 in review · ⛔ blocked · ⚪ ready/unstarted

## Adversary pass (§1)

Mandatory for **P-003, P-004, P-005**. Skipped elsewhere — each skip logged in `LEDGER.md` as a
deliberate deviation with a one-line reason.

## Open gate questions

Carried from §4. These are decisions, not tasks:

- **GATE-1 (P-003)** — is there live testnet state with balances anyone cares about? Reset or
  migrate? Cheapest confirmation is one transaction: try to spend from a genesis-allocated address.
- **GATE-2 (P-004)** — testnet topology; hard fork needs a coordinated restart, not a rolling upgrade.
- **GATE-3 (P-004-D)** — fork-choice metric. Three options in §4. Sarah's call, and it may interact
  with Lean theorems in `lean4/` that encode fork-choice assumptions.
