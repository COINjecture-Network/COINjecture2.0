# COINjecture2.0 — Roadmap

> **Status:** Draft | **Date:** 2026-03-24

## Timeline Overview

```
Month 1-2:  ████████████████  P0 Security (Phases 1-5)
Month 2-3:  ████████████████  P1 Quality (Phases 6-12)
Month 4-5:  ████████████████  P2 Features (Phases 13-18)
Month 5-6:  ████████████████  P3 Polish (Phase 19)
Month 6-8:  ████████████████  Launch Prep (Phase 20)
```

## Milestones

### M1: Security Baseline (End of Month 2)
- All P0 critical security fixes complete (Phases 1-5)
- Encrypted keystores, RPC auth, TLS, error hardening
- Gate: Zero critical audit findings open

### M2: Testnet Public Access (End of Month 3)
- P1 quality improvements complete (Phases 6-12)
- Input validation, logging, testing, CI/CD, Docker hardened
- Gate: All CI checks pass, 60%+ code coverage

### M3: Feature Complete (End of Month 5)
- P2 features complete (Phases 13-18)
- Database hardened, performance validated, load tested
- Protocol versioning, governance MVP, bridge design
- Gate: 1000 TPS sustained, 24hr soak test passed

### M4: Documentation Complete (End of Month 6)
- All documentation written and published
- Developer quickstart, operator guide, API reference
- Gate: New developer can build and run in 15 minutes

### M5: Mainnet Ready (End of Month 8)
- External audit complete with zero critical/high findings
- All phases complete, launch checklist signed off
- Gate: Phase 20 success criteria met

## Dependencies

See the dependency graph in [production-readiness-plan.md](production-readiness-plan.md) for phase ordering constraints.
