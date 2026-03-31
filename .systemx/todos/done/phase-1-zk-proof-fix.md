# DONE: ZK Proof Verification Fix

**Completed:** 2026-03-24
**Task:** 1.3

## What was done
- `core/src/privacy.rs`: `verify_placeholder_proof` no longer returns `true` for all inputs
- Placeholder proof is now `SHA-256("COINJECT_TESTNET_PLACEHOLDER_V1" || commitment || params)`
- Verifier recomputes and compares via `subtle::ConstantTimeEq`
- Forged proofs with garbage bytes are now rejected
- Added `test_forged_proof_rejected` regression test
