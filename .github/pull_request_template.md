## Description

<!-- Briefly describe what this PR does and why. Link to any relevant issues. -->

Closes #

---

## Type of change

- [ ] Bug fix
- [ ] New feature
- [ ] Refactor / code cleanup
- [ ] Performance improvement
- [ ] Documentation update
- [ ] CI/tooling change
- [ ] Breaking change (requires migration notes below)

---

## Checklist

### Code quality
- [ ] `cargo fmt --all` passes (no formatting changes needed)
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` passes with zero warnings
- [ ] No `#[allow(...)]` suppressions added without justification comment

### Tests
- [ ] `cargo test --all` passes locally
- [ ] New behaviour is covered by unit or integration tests
- [ ] No tests were deleted without documented reason

### Security
- [ ] `cargo audit` reports no new advisories
- [ ] No secrets, keys, or credentials introduced
- [ ] Input validation added at all new trust boundaries

### Documentation
- [ ] Public API changes documented (doc comments updated)
- [ ] `README.md` / relevant docs updated if behaviour changed
- [ ] Changelog entry added (if user-visible change)

### Breaking changes
<!-- If this is a breaking change, describe the migration path -->

---

## Testing performed

<!-- Describe how you tested these changes. Include relevant commands, test scenarios, or screenshots. -->

```
cargo test --all
```

---

## Notes for reviewers

<!-- Anything specific you'd like reviewers to focus on or be aware of. -->
