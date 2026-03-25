# .systemx — COINjecture 2.0 Planning & Tracking Index

This directory contains all planning, status, and log artifacts for the COINjecture 2.0 production readiness program.

---

## Plans

| File | Description | Status |
|------|-------------|--------|
| [plans/launch-checklist.md](plans/launch-checklist.md) | 72-item pre-mainnet checklist (53% complete) | Active |
| [plans/executive-summary.md](plans/executive-summary.md) | All 20 phases summarized with risk assessment | Complete |

---

## Status Reports

| File | Description | Date |
|------|-------------|------|
| [status/reports/final-audit.md](status/reports/final-audit.md) | Phase 20 final audit — fmt, clippy, tests, known issues | 2026-03-25 |

---

## Changelogs

| File | Description | Date |
|------|-------------|------|
| [logs/changelog/phase-19-20.md](logs/changelog/phase-19-20.md) | Phase 19 & 20 — Documentation + Final Audit | 2026-03-25 |
| [logs/changelog/phase-8.md](logs/changelog/phase-8.md) | Phase 8 — Unit Testing Infrastructure | 2026-03-25 |

---

## Scripts

| File | Description |
|------|-------------|
| [scripts/test/run_tests.sh](scripts/test/run_tests.sh) | Run workspace tests; `--coverage` flag for tarpaulin |

---

## Quick Reference

### Current State (2026-03-25)

- **Version**: 4.8.4
- **Tests**: 665 passing, 0 failures
- **Clippy**: 0 warnings
- **Formatting**: Clean
- **Launch readiness**: 53% (38/72 checklist items)

### Critical Blockers for Mainnet

1. Formal security audit (Q3 2026)
2. EscrowTransaction multi-sig completion
3. Economic attack simulation (Q2 2026)
4. Public bootstrap infrastructure (≥3 nodes)
5. `cargo-audit` clean
6. State size limits defined

### Phase History

| Phases | Title | Branch |
|--------|-------|--------|
| 1–7, 9–11, 13–17 | Core, State, Network, RPC, Metrics | main |
| 8 | Unit Testing | claude/dazzling-bohr |
| 12 | Web Wallet Security | main |
| 18–20 | Consensus Refinement, Docs, Audit | claude/exciting-knuth |
