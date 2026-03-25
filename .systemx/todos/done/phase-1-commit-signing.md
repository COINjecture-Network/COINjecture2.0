# DONE: Consensus Commit Signing

**Completed:** 2026-03-24
**Task:** 1.5

## What was done
- `consensus/src/coordinator/commit.rs`: `SolutionCommit` gains `public_key: [u8; 32]`
- `commit_signing_message()`: canonical 66-byte domain-separated signing payload
- `verify_commit_signature()`: ed25519 verification before accepting commits
- `CommitCollector::add_commit()`: rejects forged/tampered commits
- `consensus/src/coordinator/mod.rs`: `EpochCoordinator` accepts `Option<SigningKey>`,
  new `with_signing_key()` constructor, local commits are now signed when key is present
- Migration bypass for unsigned legacy commits (all-zero public key)
- New tests: forged sig, wrong epoch, tampered score
