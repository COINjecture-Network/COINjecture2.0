# Security Policy

## Supported Versions

COINjecture 2.0 is currently in **testnet phase**. Security fixes are applied to the latest version only.

| Version | Supported |
|---------|-----------|
| 4.8.x (current) | Yes |
| < 4.8 | No |

> **Notice**: This is pre-audit testnet software. Do not use with real funds. A formal security audit is planned before mainnet launch.

---

## Reporting a Vulnerability

**Do not report security vulnerabilities through public GitHub issues.**

### Process

1. **Email**: Send details to **adz@alphx.io** with subject line `[SECURITY] COINjecture Vulnerability`
2. **Encrypt** (recommended): Use PGP if available (key on request)
3. **Include**:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact assessment
   - Suggested fix (if known)

### Response Timeline

| Stage | Target |
|-------|--------|
| Acknowledgement | 48 hours |
| Initial assessment | 5 business days |
| Fix development | Depends on severity |
| Coordinated disclosure | 90 days after report |

### Severity Levels

| Severity | Description | Examples |
|----------|-------------|---------|
| **Critical** | Chain integrity, fund theft | Consensus bypass, double-spend |
| **High** | Node crash, network partition | Panic in block validation, DoS in CPP |
| **Medium** | Information disclosure, degraded performance | Peer enumeration, mempool exhaustion |
| **Low** | Minor issues, non-exploitable bugs | Edge cases, minor info leaks |

---

## Scope

### In Scope

- **Core consensus**: Block validation, work score calculation, difficulty adjustment
- **Cryptography**: Ed25519 signing, Blake3 hashing, Merkle tree construction
- **State transitions**: Balance updates, marketplace escrow, pool swaps
- **Network protocol**: CPP message handling, peer management
- **RPC layer**: JSON-RPC endpoint security
- **Key management**: Wallet keystore operations

### Out of Scope

- Infrastructure you run yourself (nodes, servers)
- Issues in dependencies — report directly to upstream
- Issues requiring physical access to a machine
- Social engineering attacks

---

## Security Design Principles

COINjecture's security model relies on:

1. **Ed25519 signatures** — All transactions are signed; signature checked before state mutation
2. **Replay protection** — Per-account nonces prevent transaction replay
3. **ACID state** — redb provides atomic, consistent, isolated, durable state transitions
4. **Polynomial-time verification** — NP-hard solutions are efficiently verifiable in O(n) to O(n²)
5. **Commitment scheme** — Miners commit to solutions before revealing (prevents front-running)
6. **CPP integrity** — blake3 checksums on every network message

---

## Known Limitations (Pre-Mainnet)

The following are known limitations that will be addressed before mainnet:

1. **No formal security audit** — Scheduled for Q3 2026
2. **Economic attack simulation** — Planned for Q2 2026
3. **Incomplete multi-sig for escrow** — Additional signature verification stubbed
4. **Light client security** — Light sync not fully hardened
5. **Governance** — On-chain governance not yet implemented

These are **known** and **not** considered responsible disclosure targets.

---

## Acknowledgements

We thank all researchers who responsibly disclose vulnerabilities. Contributors will be acknowledged in release notes (unless they prefer anonymity).
