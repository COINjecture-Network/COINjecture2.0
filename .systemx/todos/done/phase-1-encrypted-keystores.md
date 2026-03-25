# DONE: Encrypted Keystore Implementation

**Completed:** 2026-03-24
**Tasks:** 1.1, 1.2, 1.6

## What was done
- `node/src/keystore.rs`: AES-256-GCM encrypted binary format (CIKV v1)
- `wallet/src/keystore.rs`: AES-256-GCM encrypted key files (CKWV v1), split
  public metadata (cleartext JSON) from private key (encrypted .key file)
- argon2id KDF (m=64MiB, t=3, p=1) for password-based key derivation
- `ValidatorKey::secret_key` zeroized on drop
- Password from `COINJECT_KEYSTORE_PASSWORD` env var
- Full unit test coverage including wrong-password rejection, magic header check,
  round-trip, debug redaction
