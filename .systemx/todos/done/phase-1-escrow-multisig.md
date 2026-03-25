# DONE: Escrow Multi-Sig Authorization

**Completed:** 2026-03-24
**Task:** 1.7 (escrow validation)

## What was done
- `state/src/escrows.rs`: Added `release_with_auth()` and `refund_with_auth()` methods
- Callers must provide (public_key, signature, timestamp) to prove ownership
- Canonical auth message: `"COINJECT_ESCROW_AUTH_V1" || escrow_id || action || ts_le64`
- Freshness check: timestamp must be within ±300s of current time (replay protection)
- `address_from_pubkey()`: BLAKE3 derivation consistent with node keystore
- Existing `can_release`/`can_refund` preserved for eligibility checks
- Tests: valid release sig, wrong-action sig rejected, valid refund after timeout
