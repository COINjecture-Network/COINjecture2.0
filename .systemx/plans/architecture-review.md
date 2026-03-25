# Architecture Review — COINjecture2.0

> **Status:** Initial review complete | **Date:** 2026-03-24

## Overview

COINjecture2.0 is a 13-crate Rust workspace implementing a blockchain platform with a custom Conjecture Propagation Protocol (CPP) consensus mechanism. The architecture is modular, well-separated, and uses production-grade cryptographic libraries.

## Crate Dependency Graph

```
                    ┌─────────┐
                    │  node   │ (binary entry point)
                    └────┬────┘
          ┌──────┬───────┼───────┬──────────┐
          │      │       │       │          │
     ┌────▼──┐ ┌─▼───┐ ┌▼────┐ ┌▼────────┐ ┌▼──────┐
     │  rpc  │ │ net │ │ mem │ │consensus│ │ state │
     └───┬───┘ └──┬──┘ │pool │ └────┬────┘ └───┬───┘
         │        │    └──┬──┘      │           │
         │        │       │         │           │
         └────────┴───────┴────┬────┴───────────┘
                               │
                          ┌────▼────┐
                          │  core   │ (types, crypto, tx, block)
                          └────┬────┘
                               │
                          ┌────▼────┐
                          │  adzdb  │ (storage layer)
                          └─────────┘

  Auxiliary:  tokenomics ← core
              wallet ← core, rpc (client)
              huggingface ← core
              marketplace-export ← core, state
              mobile-sdk ← core
```

## Strengths

- Clean separation of concerns across crates
- Custom CPP protocol with elegant equilibrium-based flow control
- Production-grade crypto: ed25519-dalek, blake3, @noble (web wallet)
- Working Docker testnet with 4-node orchestration
- ACID-compliant storage via redb

## Areas for Improvement

See [production-readiness-plan.md](production-readiness-plan.md) for the full 20-phase remediation plan addressing all 43 audit findings.

## Key Decisions

- **CPP over libp2p:** Custom protocol chosen for tighter consensus integration and equilibrium flow control
- **redb over RocksDB:** Pure Rust, simpler deployment, ACID-compliant, no C++ dependency
- **ed25519-dalek:** Industry-standard, audited, pure Rust implementation
- **Workspace architecture:** 13 crates enables parallel compilation and clear boundaries
