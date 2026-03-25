# Phase 10 — CI/CD Pipeline Hardening

**Date:** 2026-03-25
**Branch:** claude/awesome-euler
**Status:** Complete

---

## Summary

Replaced the single-job CI workflow with a hardened, multi-stage pipeline and
added a complete release automation suite.

---

## Files Created / Modified

### `.github/workflows/ci.yml` (modified)
**Before:** Single `build-and-test` job + `docker-smoke` job. Formatting check
was commented out. No security scanning, no coverage, no dependency graph.

**After:** Five-stage pipeline with explicit job dependencies:

| Stage | Job | Depends on |
|-------|-----|-----------|
| 1 | `lint` | — |
| 2 | `test` | `lint` |
| 2 | `build` | `lint` |
| 3 | `security` | `lint` |
| 4 | `docker` | `test`, `build` |

- **`lint`** — `cargo fmt --all --check` + `cargo clippy --all-targets --all-features -- -D warnings`
- **`test`** — `cargo test --all --all-features` + cargo-tarpaulin coverage → Codecov upload
- **`build`** — `cargo build --release` for `coinject-node` and `coinject-wallet`; uploads Linux amd64 artifacts (7-day retention)
- **`security`** — `cargo audit` (hard fail) + `cargo deny check` (soft fail, wires in deny.toml when added)
- **`docker`** — Buildx multi-platform, push to `ghcr.io` on branch push (skip on PRs), layer cache via `type=gha`

**Caching strategy:**
- Cargo registry index + cache + git db cached by `Cargo.lock` hash (shared across jobs via restore-keys)
- Build artifacts (`target/`) cached per job with fine-grained keys including `.rs` file hashes for test job

---

### `.github/workflows/release.yml` (new)
Triggered on `v*.*.*` tag push.

**Matrix:** 5 targets
- `x86_64-unknown-linux-gnu` (ubuntu-latest, native)
- `aarch64-unknown-linux-gnu` (ubuntu-latest, cross via `cross`)
- `x86_64-apple-darwin` (macos-latest, native)
- `aarch64-apple-darwin` (macos-latest, native)
- `x86_64-pc-windows-msvc` (windows-latest, native)

**Artifacts per target:** `coinject-node-{tag}-{suffix}[.exe]`, `coinject-wallet-{tag}-{suffix}[.exe]`, per-platform `checksums-{suffix}.txt`

**Release job:** Downloads all artifacts, merges checksums into `SHA256SUMS.txt`, extracts changelog section from `CHANGELOG.md`, publishes GitHub Release via `softprops/action-gh-release@v2`. Pre-release flag set automatically for `-alpha`/`-beta`/`-rc` tags.

**Docker release job:** Builds multi-platform image (`linux/amd64`, `linux/arm64`), pushes semver tags (`v1.2.3`, `v1.2`, `v1`, `latest`) to `ghcr.io`.

---

### `.github/pull_request_template.md` (new)
Checklist covers:
- Code quality: fmt, clippy, no unjustified `#[allow]`
- Tests: all pass, new behaviour covered, no deleted tests
- Security: audit clean, no secrets, input validation
- Documentation: doc comments, README, changelog
- Breaking changes: migration notes section

---

### `.github/dependabot.yml` (new)
Three ecosystems configured:
- **Cargo** — weekly Monday 06:00 UTC, limit 10 PRs, patch/minor grouped, major versions for `tokio`/`serde`/`jsonrpsee`/`redb` ignored (manual review)
- **GitHub Actions** — weekly, limit 5 PRs, all grouped
- **Docker** — weekly, limit 3 PRs

All commits prefixed (`chore(deps)`, `chore(ci)`, `chore(docker)`) and labelled for easy filtering.

---

## Recommended Branch Protection Rules for `main`

Apply via **GitHub → Settings → Branches → Add rule → `main`**:

| Setting | Value | Reason |
|---------|-------|--------|
| Require status checks to pass | `lint`, `test`, `build`, `security` | All CI stages must be green |
| Require branches to be up to date | ✓ | Prevent stale-branch merges |
| Require pull request reviews | 1 approving review | Human sign-off on every merge |
| Dismiss stale reviews on push | ✓ | Re-review after force-updates |
| Require review from code owners | ✓ (add CODEOWNERS) | Core files protected |
| Restrict who can push to matching branches | Admins only | No direct pushes |
| Allow force pushes | ✗ | Preserve linear history |
| Allow deletions | ✗ | Protect main from accidental deletion |
| Require signed commits | ✓ (recommended) | Provenance assurance |

---

## Tools Introduced

| Tool | Version | Purpose |
|------|---------|---------|
| `cargo-tarpaulin` | latest stable | Code coverage (Xml → Codecov) |
| `cargo-audit` | latest stable | CVE advisory scanning |
| `cargo-deny` | latest stable | License, ban, and advisory policy |
| `cross` | latest stable | Cross-compilation for ARM64 Linux |
| `taiki-e/install-action@v2` | v2 | Fast tool installation in CI |
| `docker/buildx` | v3 | Multi-platform Docker builds |
| `softprops/action-gh-release@v2` | v2 | GitHub Release creation |

---

## Next Steps

- Add `deny.toml` to configure `cargo-deny` policies (licenses, bans)
- Add `CODEOWNERS` file pointing core crates to maintainers
- Enable Codecov integration (add `CODECOV_TOKEN` secret)
- Consider adding `cargo-semver-checks` to release workflow to catch accidental API breaks
