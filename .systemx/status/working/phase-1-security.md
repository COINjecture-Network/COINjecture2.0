# Phase 1 — Critical Security Fixes

**Status:** COMPLETE — `cargo build` passes (1 dead_code warning, no errors)
**Branch:** `claude/affectionate-snyder`
**Updated:** 2026-03-24

## Completed
- [x] 1.1 Encrypted node keystore (AES-256-GCM + argon2id)
- [x] 1.2 Encrypted wallet keystore (AES-256-GCM + argon2id)
- [x] 1.3 Fix ZK proof placeholder — no longer always-true
- [x] 1.5 Consensus commit ed25519 signing + verification
- [x] 1.6 Zeroize secret key material on drop
- [x] 1.7 Escrow release/refund require ed25519 signatures
- [x] 2.3 CORS restricted from Any to localhost allow-list (CSRF mitigation)

## Build Verification
- [x] `cargo check` — clean (no errors)
- [x] `cargo build` — clean (1 dead_code warning on public API methods, no errors)

## Deferred to Phase 1b
- [ ] 1.7 `coinject keygen` CLI subcommand
- [ ] 1.8 `cargo deny` config
- [ ] 1.9 `cargo audit` review
- [ ] 1.10 Panic hook
- [ ] 1.11 Secret scanning pre-commit hook
- [ ] Full ZK circuit implementation

## Notes
- All changes are in the `claude/affectionate-snyder` worktree
- Backward-incompatible key format changes are intentional and documented
- Tests updated and new security regression tests added
