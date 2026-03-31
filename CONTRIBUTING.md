# Contributing to COINjecture 2.0

Thank you for your interest in contributing to COINjecture 2.0 — a WEB4 Layer 1 blockchain implementing Proof of Useful Work.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Environment](#development-environment)
- [Project Structure](#project-structure)
- [Making Changes](#making-changes)
- [Commit Conventions](#commit-conventions)
- [Pull Request Process](#pull-request-process)
- [Code Style](#code-style)
- [Testing Requirements](#testing-requirements)
- [Review Guidelines](#review-guidelines)

---

## Code of Conduct

This project follows our [Code of Conduct](CODE_OF_CONDUCT.md). By participating, you agree to uphold it.

---

## Getting Started

1. **Fork** the repository on GitHub
2. **Clone** your fork locally:
   ```bash
   git clone https://github.com/YOUR_USERNAME/COINjecture2.0.git
   cd COINjecture2.0
   ```
3. **Add upstream** remote:
   ```bash
   git remote add upstream https://github.com/Quigles1337/COINjecture2.0.git
   ```
4. **Create a feature branch**:
   ```bash
   git checkout -b feat/your-feature-name
   ```

---

## Development Environment

### Requirements

- **Rust 1.88+** — install via [rustup.rs](https://rustup.rs/)
- **Docker** (optional) — for running the 4-node testnet
- **cargo-audit** (recommended) — `cargo install cargo-audit`
- **cargo-tarpaulin** (optional) — `cargo install cargo-tarpaulin` for coverage

### Initial Setup

```bash
# Verify Rust version
rustc --version  # should be 1.88+

# Build the workspace
cargo build

# Run all tests
cargo test --workspace

# Check formatting
cargo fmt --check

# Run lints
cargo clippy --workspace -- -D warnings
```

---

## Project Structure

```
COINjecture2.0/
├── core/           # Primitive types, cryptography, transactions, blocks
├── consensus/      # PoUW mining engine, difficulty adjustment, work scoring
├── network/        # CPP P2P protocol (custom TCP on port 707)
├── state/          # ACID-compliant redb state management
├── mempool/        # Transaction pool, fee market, marketplace cache
├── rpc/            # JSON-RPC HTTP/WebSocket server
├── tokenomics/     # Dimensional economics, emission, reward distribution
├── node/           # Full node binary (coinject)
├── wallet/         # CLI wallet binary
├── adzdb/          # Custom append-only block database
├── huggingface/    # HuggingFace dataset integration
├── mobile-sdk/     # Mobile SDK bindings
└── marketplace-export/  # Marketplace data export utilities
```

Key invariants:
- **`core`** has no internal dependencies — it is the foundation
- **`state`** depends only on `core` and `adzdb`
- **`consensus`** depends on `core` only
- **`network`** depends on `core` only (no libp2p — pure stdlib TCP)
- **`node`** is the integration point that wires all crates together

---

## Making Changes

### Branching Strategy

| Branch type | Naming convention | Example |
|-------------|-------------------|---------|
| Feature | `feat/<short-description>` | `feat/add-tsp-solver` |
| Bug fix | `fix/<short-description>` | `fix/merkle-root-mismatch` |
| Refactor | `refactor/<short-description>` | `refactor/work-score-formula` |
| Documentation | `docs/<short-description>` | `docs/consensus-explained` |
| Test | `test/<short-description>` | `test/property-tests-consensus` |

### Keep Changes Focused

- One logical change per PR
- Avoid mixing refactoring with feature work
- Keep PRs small and reviewable (< 500 lines preferred)

---

## Commit Conventions

This project uses [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <short description>

[optional body]

[optional footer]
```

### Types

| Type | When to use |
|------|-------------|
| `feat` | New feature |
| `fix` | Bug fix |
| `refactor` | Code restructuring (no behavior change) |
| `test` | Adding or fixing tests |
| `docs` | Documentation only |
| `perf` | Performance improvement |
| `chore` | Build, CI, dependency updates |
| `style` | Formatting only (no logic change) |

### Scopes

Use the crate name as scope: `core`, `consensus`, `network`, `state`, `mempool`, `rpc`, `node`, `wallet`, `tokenomics`.

### Examples

```
feat(consensus): add log2 bit-equivalent work score formula
fix(network): handle partial TCP reads in CPP protocol decoder
refactor(state): extract marketplace escrow logic into separate module
test(consensus): add property tests for difficulty adjustment
docs(core): add doc comments to all public types
```

---

## Pull Request Process

1. **Sync** with upstream before opening a PR:
   ```bash
   git fetch upstream
   git rebase upstream/main
   ```

2. **Ensure all checks pass locally**:
   ```bash
   cargo fmt --check
   cargo clippy --workspace -- -D warnings
   cargo test --workspace
   ```

3. **Open the PR** against `main` with:
   - A clear title following commit conventions
   - Description explaining _what_ and _why_ (not _how_)
   - Reference any related issues (`Closes #123`)

4. **Address review comments** — push fixup commits, do not force-push mid-review

5. **Squash on merge** — the maintainer will squash commits for a clean history

### PR Checklist

- [ ] Tests added/updated for the change
- [ ] `cargo fmt --check` passes
- [ ] `cargo clippy --workspace -- -D warnings` passes
- [ ] `cargo test --workspace` passes
- [ ] Doc comments updated for any changed public API
- [ ] CHANGELOG entry added if user-visible change

---

## Code Style

### Formatting

- Run `cargo fmt` before every commit — no exceptions
- CI will fail on unformatted code

### Lints

- Zero clippy warnings in the workspace (`-D warnings`)
- Suppress specific lints with `#[allow(...)]` + a comment explaining why

### Doc Comments

All **public** types, functions, and modules must have `///` doc comments. Format:

```rust
/// One-line summary (imperative mood: "Calculate", "Return", "Verify").
///
/// Extended description if the one-liner isn't enough.
///
/// # Arguments
/// * `foo` - What foo is
///
/// # Returns
/// Description of the return value.
///
/// # Errors
/// When this can fail.
///
/// # Panics
/// When this can panic (try to avoid).
pub fn my_function(foo: u32) -> Result<u64, Error> { ... }
```

### Error Handling

- Use `thiserror` for library crates (typed errors)
- Use `anyhow` for binary crates (`node`, `wallet`) where error context matters
- Never use `unwrap()` in library code — return `Result` or `Option`
- `unwrap()` is acceptable in tests and one-time setup code

### Async

- All I/O is `async` using `tokio`
- Don't block in async contexts — use `tokio::task::spawn_blocking` for CPU work
- Prefer `tokio::sync` primitives over `std::sync` in async code

### Consensus-Critical Code

Any code that affects block hashing, work score calculation, or state transitions must:
- Use only integer arithmetic (no floats) where consensus determinism is required
- Be clearly documented with `// CONSENSUS-CRITICAL` comment
- Have property tests verifying determinism across inputs

---

## Testing Requirements

### Test Coverage Expectations

| Crate | Minimum coverage |
|-------|-----------------|
| `core` | 80% |
| `consensus` | 75% |
| `state` | 70% |
| `network` | 60% |

### Test Types

1. **Unit tests** — in `#[cfg(test)]` modules within source files
2. **Integration tests** — in `<crate>/tests/` directory
3. **Property tests** — use `proptest` for consensus-critical invariants
4. **E2E tests** — in `tests/harness/` using the local test harness

### Running Tests

```bash
# All tests
cargo test --workspace

# Specific crate
cargo test -p coinject-consensus

# With output (useful for debugging)
cargo test --workspace -- --nocapture

# Property tests (more iterations)
PROPTEST_CASES=1000 cargo test --workspace

# Coverage (requires cargo-tarpaulin)
cargo tarpaulin --workspace --out Html
```

---

## Review Guidelines

### As an Author

- Explain non-obvious decisions in comments or PR description
- Keep PRs focused — split unrelated changes
- Respond to all review comments, even if just acknowledging

### As a Reviewer

- Be constructive and specific — propose alternatives when blocking
- Distinguish blocking issues from suggestions (`nit:` prefix for minor style)
- Check for:
  - Correctness of consensus-critical paths
  - Proper error handling (no silent failures)
  - Test coverage for new behavior
  - Documentation for new public API
  - No performance regressions in hot paths (block validation, peer routing)

---

## Security

If you discover a security vulnerability, **do not open a public issue**. Please follow the process in [SECURITY.md](SECURITY.md).
