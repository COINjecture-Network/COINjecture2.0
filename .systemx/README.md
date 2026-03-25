# .systemx — COINjecture2.0 Workspace

> Central coordination hub for the COINjecture2.0 production readiness effort.
> Version 4.8.4 | 13-crate Rust workspace | CPP Consensus Protocol

---

## Quick Navigation

### Plans & Strategy
| Document | Description |
|----------|-------------|
| [Production Readiness Plan](plans/production-readiness-plan.md) | **START HERE** — 20-phase, 200+ task plan covering all audit findings |
| [Architecture Review](plans/architecture-review.md) | System architecture analysis and recommendations |
| [Roadmap](plans/roadmap.md) | High-level timeline and milestones |

### Task Tracking
| Directory | Purpose |
|-----------|---------|
| [todos/backlog/](todos/backlog/) | Tasks not yet started |
| [todos/in-progress/](todos/in-progress/) | Tasks currently being worked on |
| [todos/blocked/](todos/blocked/) | Tasks blocked by dependencies or external factors |
| [todos/done/](todos/done/) | Completed tasks (for audit trail) |

### Scripts & Automation
| Directory | Purpose |
|-----------|---------|
| [scripts/setup/](scripts/setup/) | Environment and dependency setup scripts |
| [scripts/build/](scripts/build/) | Build automation and compilation helpers |
| [scripts/deploy/](scripts/deploy/) | Deployment scripts (Docker, cloud, bare metal) |
| [scripts/test/](scripts/test/) | Test runners, coverage, fuzzing, load test tools |
| [scripts/utils/](scripts/utils/) | Miscellaneous utility scripts |

### Helpers & Templates
| Directory | Purpose |
|-----------|---------|
| [helpers/templates/](helpers/templates/) | PR templates, issue templates, ADR templates |
| [helpers/snippets/](helpers/snippets/) | Reusable code snippets for common patterns |
| [helpers/prompts/](helpers/prompts/) | AI/LLM prompts for code review, generation, etc. |

### Project Status
| Directory | Purpose |
|-----------|---------|
| [status/broken/](status/broken/) | Known broken features and modules |
| [status/working/](status/working/) | Verified working features and modules |
| [status/reports/](status/reports/) | Status reports, load test results, audit summaries |

### Documentation
| Directory | Purpose |
|-----------|---------|
| [docs/api/](docs/api/) | RPC reference, WebSocket API, transaction/block formats |
| [docs/architecture/](docs/architecture/) | System architecture, CPP protocol, tokenomics model |
| [docs/guides/](docs/guides/) | Quickstart, operator guide, testnet guide, code walkthroughs |
| [docs/decisions/](docs/decisions/) | Architecture Decision Records (ADRs) |

### Testing
| Directory | Purpose |
|-----------|---------|
| [tests/integration/](tests/integration/) | Cross-crate integration test plans and configs |
| [tests/e2e/](tests/e2e/) | End-to-end Docker testnet scenarios |
| [tests/load/](tests/load/) | Load testing scripts, configs, and results |
| [tests/fuzz/](tests/fuzz/) | Fuzz testing targets and corpus |

### Security
| Directory | Purpose |
|-----------|---------|
| [security/audit-findings/](security/audit-findings/) | Audit reports and finding details |
| [security/threat-model/](security/threat-model/) | Threat model documentation |
| [security/remediation/](security/remediation/) | Remediation plans and progress tracking |

### CI/CD
| Directory | Purpose |
|-----------|---------|
| [ci-cd/pipelines/](ci-cd/pipelines/) | Pipeline definitions and workflow configs |
| [ci-cd/configs/](ci-cd/configs/) | Tool configs (clippy, deny, tarpaulin, etc.) |
| [ci-cd/hooks/](ci-cd/hooks/) | Git hooks (secret scanning, pre-commit) |

### Logs & History
| Directory | Purpose |
|-----------|---------|
| [logs/reviews/](logs/reviews/) | Code review notes and session logs |
| [logs/changelog/](logs/changelog/) | Detailed change history and release notes |

---

## Audit Summary

The initial security audit identified:

| Severity | Count | Status |
|----------|-------|--------|
| **Critical** | 9 | Phase 1–5 |
| **High** | 16 | Phase 3–12 |
| **Medium** | 18 | Phase 13–19 |

**Positive findings:** Clean 13-crate modular architecture, elegant CPP protocol with equilibrium-based flow control, working Docker testnet (4 nodes), good recent bug-fix documentation, production-grade crypto library choices (ed25519-dalek, blake3, @noble).

---

## Workspace Crates

| # | Crate | Path | Purpose |
|---|-------|------|---------|
| 1 | `adzdb` | `adzdb/` | Custom database layer |
| 2 | `core` | `core/` | Types, crypto, transactions, blocks, commitments, privacy |
| 3 | `consensus` | `consensus/` | Mining, difficulty, work scoring, problem registry |
| 4 | `network` | `network/` | CPP protocol, peer management, reputation |
| 5 | `state` | `state/` | Accounts, escrows, channels, trustlines, marketplace |
| 6 | `mempool` | `mempool/` | Transaction pool, fee market, data pricing |
| 7 | `rpc` | `rpc/` | JSON-RPC server, WebSocket subscriptions |
| 8 | `tokenomics` | `tokenomics/` | Emission, rewards, staking, AMM, governance, deflation |
| 9 | `node` | `node/` | Full node binary, chain management, config, keystore |
| 10 | `wallet` | `wallet/` | CLI wallet, keystore, RPC client |
| 11 | `marketplace-export` | `marketplace-export/` | Marketplace data export |
| 12 | `huggingface` | `huggingface/` | AI model integration, metrics, streaming |
| 13 | `mobile-sdk` | `mobile-sdk/` | Mobile client SDK |

---

## How to Use This Workspace

1. **Start with the plan:** Read [production-readiness-plan.md](plans/production-readiness-plan.md) in full.
2. **Pick a phase:** Work through phases sequentially (1 → 20), respecting dependencies.
3. **Track progress:** Move tasks between `todos/` subdirectories as work progresses.
4. **Document decisions:** Record major decisions in `docs/decisions/` using the ADR template.
5. **Report status:** Write status updates in `status/reports/` at the end of each phase.
6. **Log changes:** Record all changes in `logs/changelog/` for audit trail.

---

*Created 2026-03-24 as part of the COINjecture2.0 production readiness initiative.*
