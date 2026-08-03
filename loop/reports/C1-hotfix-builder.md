# Cycle 1 — P-002-H BUILDER report (CI hotfix)

Packet: **P-002-H** · Branch: `fix/ci-pin-toolchain-clippy` · PR **#55** (draft)
Base: `main` @ `28c50a12` · Date: 2026-08-03

---

## 1. The dark window

| | |
|---|---|
| Last CI run of any kind | **2026-06-12** — `main` @ `28c50a12`, **green** |
| Rust 1.97.1 released | **2026-07-14** (`rustc 1.97.1 (8bab26f4f 2026-07-14)`) |
| Next CI run of any kind | **2026-08-02** — PR #54 (P-000), **red** |
| Commits to `main` in the window | **zero** — `main` is still `28c50a12` |
| CI runs on *any* branch in the window | **zero** |

**Nothing merged blind.** The spec's STEP 1 asked what merged while lint was failing; the answer is
nothing. The repo was dormant for 51 days, stable moved underneath it, and `main` went
*retroactively* red without a single commit. PR #54 was simply the first thing to run CI since, and
it surfaced the breakage.

This is the benign version of the finding. **§12 item 6 resolves to "nothing to re-verify."**
PRs #46/#47/#48 and the Dependabot bumps are all far outside the window (#47 merged 2026-04-25).

---

## 2. The pin

**Root cause was the floating channel, not the lints.** Three sources all said `stable`:

| Source | Before | After |
|---|---|---|
| `rust-toolchain.toml` → `[toolchain].channel` | `stable` | **`1.97.1`** |
| `ci.yml` → `env.RUST_TOOLCHAIN` | `stable` | **`1.97.1`** |
| `ci.yml` → per-job `toolchain:` input | `${{ env.RUST_TOOLCHAIN \|\| 'stable' }}` | unchanged — now resolves to the pin |

⚠️ **`rust-toolchain.toml` already existed** (added by `136c64dc`, "for local rustup"). The spec's
STEP 2 said to *add* it. It was there — and it was floating too, so it was part of the problem
rather than a mitigation.

**How the two-sources problem is resolved:** I kept both files carrying the literal but made
divergence *impossible to merge*. The existing per-job `Guard` step (present in all four jobs) now:

1. asserts `RUST_TOOLCHAIN` is set;
2. parses `[toolchain].channel` out of `rust-toolchain.toml`;
3. **fails the job if they differ**, with an explicit `::error::` naming both values;
4. **fails the job if the channel is floating** (`stable`, `beta`, `nightly*`).

I chose a mechanical assertion over "make ci.yml read the file" because `dtolnay/rust-toolchain@v1`
requires an explicit `toolchain:` input, so the env var cannot simply be deleted. An enforced
invariant is stronger than an undocumented action behaviour, and it also blocks the *original*
defect — someone setting either source back to `stable`. **Verified working on the hosted runner:
the Guard step passed under the pin.**

### ⚠️ This is not a CI-only change

`rust-toolchain.toml` governs **every developer's** toolchain. rustup will auto-download 1.97.1 for
Sarah and anyone else working in this repo, on their next `cargo` invocation in this directory.
That is the intended effect — local finally matches hosted, which is precisely what would have
caught this class of failure before CI did — but it must not be described as a CI-only change in
the PR discussion.

---

## 3. The two lint fixes

Both are syntax-only and provably behaviour-preserving.

### `node/src/validator.rs:641` — `clippy::unneeded_wildcard_pattern`

```rust
-  coinject_core::ChannelType::UnilateralClose {
-      balance_a: _,
-      balance_b: _,
-      ..
-  } => {
+  coinject_core::ChannelType::UnilateralClose { .. } => {
```

`ChannelType::UnilateralClose` has four fields (`core/src/transaction.rs:149-154`: `sequence`,
`balance_a`, `balance_b`, `dispute_proof`). The old pattern bound `balance_a` and `balance_b` to `_`
— which binds nothing — and `..` already covered `sequence` and `dispute_proof`. The new pattern
matches the same variant and binds the same nothing. **The arm body is untouched** (it is entirely
`TODO` comments). This is consensus code, so I kept strictly to pattern syntax [D5]; `{ .. }` is
already the idiom used for this same variant at `transaction.rs:688` and `:693`.

### `wallet/src/commands/marketplace.rs:30` — `clippy::useless_borrows_in_formatting`

```rust
-  println!(
-      "{}. Problem #{}",
-      i + 1,
-      &problem.problem_id[0..16].dimmed()
-  );
+  println!("{}. Problem #{}", i + 1, problem.problem_id[0..16].dimmed());
```

`impl Display for &T` forwards to `T`, and `println!` takes all arguments by reference internally,
so the formatted bytes are identical. The one-line collapse is **rustfmt's doing**, not mine: after
removing the `&` the call fits within the width limit, and `cargo fmt --check` then demanded it.
That reformat is a consequence of the fix, not an independent change — worth knowing when reading
the diff.

---

## 4. Complete clippy output under the pinned toolchain

```
$ cargo +1.97.1 clippy --workspace --all-targets --all-features --locked
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2m 22s
```

**Zero warnings, zero errors, exit 0.** Exactly the two known lints existed under 1.97.1 — no
others. The re-run using CI's exact invocation (`cargo clippy --all-targets --all-features --
-D warnings`) is also clean. `cargo fmt --all -- --check` is clean, and produced exactly one diff
hunk before I applied it — in the file I had just edited, confirming the repo is otherwise
fmt-clean.

**No STOP condition fired here.** The packet did not need re-scoping.

---

## 5. 🚨 The 936 baseline is wrong, and it was my error

**Measured under the pinned toolchain: 982 passed / 0 failed / 4 ignored, 49 test binaries.**

Not 936/30. The suite is green either way, but the number in `LOOP_SPEC.md` §2 is wrong and I put
it there.

**Cause:** my Cycle 0 baseline command ended in `| head -60`. That truncated the stream at 60
matching lines, and I summed the 30 `test result:` lines that survived. The captured output file is
exactly 61 lines and ends mid-suite on a `test result:` line — the remaining ~19 binaries never
reached the summary. **936 was a truncation artifact, never a measurement.**

| | Value |
|---|---|
| Spec §2 baseline (wrong) | 936 / 0 / 4 across 30 bins |
| **True, under 1.97.1** | **982 / 0 / 4 across 49 bins** |
| Original claim in the Cycle 0 order | 951 |

Note 982 **exceeds** the original 951 claim rather than falling short of it, which is consistent
with tests having been added since 951 was recorded. The Cycle 0 report's "−15 tests" discrepancy
was therefore an artifact of my own truncation, not a real shortfall — that part of the C0 report
should be disregarded.

I have **not** established what the true total is under the *old* 1.91 toolchain, so I cannot say
whether 1.97.1 changed the count at all. The honest statement is: 982 is the first correctly
measured total, taken under the pinned toolchain.

**Recommendation:** amend §2 of the spec to 982 / 0 / 4 and drop the "provisional" caveat, since
this run *is* the measurement under current stable that the caveat was waiting for.

---

## 6. Hosted run — the Security Audit job is alive

Run **30842995637** on `fix/ci-pin-toolchain-clippy`:

| Job | Before (PR #54) | Now |
|---|---|---|
| **Lint** | ❌ fail | ✅ **pass** (1m49s) |
| Test (default) | ⏭️ skipped | ▶️ executing |
| Test (adzdb) | ⏭️ skipped | ▶️ executing |
| Build (release) ×2 | ⏭️ skipped | ▶️ executing |
| **Security Audit** | ⏭️ **skipped (dark)** | ▶️ **RAN** — ❌ fails |

Final: **Lint ✅ · Test (default) ✅ · Test (adzdb) ✅ · Build (release) ×2 ✅ · Security Audit ❌ ·
CodeQL + Analyze ×4 ✅.** Every job that was dark now executes.

**Cross-platform confirmation of the corrected baseline:** the hosted `Test (default)` log sums to
1964 passed / 0 failed / 8 ignored — exactly **2×** my local figure, because the job runs
`cargo test --all` and then `cargo tarpaulin --all`, which re-runs the whole suite. The single-run
hosted total is therefore **982 passed / 0 failed / 4 ignored**, matching the local Windows
measurement exactly. **982 is confirmed on both platforms; 936 is definitively wrong.**
| CodeQL / Analyze ×4 | ✅ pass | ✅ pass |

**The packet's stated deliverable is met: the Security Audit job executes again.** Its step-level
result:

```
success  Guard — toolchain is set, pinned, and agrees with rust-toolchain.toml
success  Install cargo-audit
success  Install cargo-deny
failure  Run cargo-audit
skipped  Run cargo-deny (licenses, bans, advisories)
```

It fails on `cargo audit`, which exits non-zero on the two known RUSTSEC advisories
(`quinn-proto` RUSTSEC-2026-0185, `crossbeam-epoch` RUSTSEC-2026-0204). **That failure is genuine,
pre-existing, and out of this packet's scope** — the guardrails forbid dependency bumps, and
advisory triage is P-002 STEP 3.

**Two discoveries here that change P-002:**

1. **`cargo deny check` is already wired into `ci.yml`** with `continue-on-error: true`, and
   `cargo-deny` is already installed by the job. P-002's delta is therefore smaller than the spec
   assumes on the wiring side — but the step is currently **toothless twice over**: it has
   `continue-on-error`, *and* it never executes at all because the preceding `cargo audit` step
   fails first. P-002 must add `deny.toml` **and** remove `continue-on-error` **and** resolve the
   audit failure, or the deny gate stays decorative.
2. **The Security Audit job cannot go green until the two advisories are triaged.** So this packet
   restores the *signal* but cannot restore *green*.

---

## 7. Wanted: a second opinion

1. **D12 vs. reality.** D12 says green before merge, no exceptions. P-002-H makes Lint green but
   Security Audit now reports red where it previously reported nothing. Strictly, P-002-H cannot
   satisfy D12 — and neither can anything else until P-002 triages the advisories. Options: fold
   the advisory triage into this packet (scope expansion on a hotfix), accept a known-red Security
   Audit as the merge state, or run P-002 before merging either PR. **I did not decide this.** My
   inclination is to merge P-002-H anyway — a job that reports a true failure is strictly better
   than a job that reports nothing — but that is a D12 amendment, and D12 is Al's rule.
2. **Pinning to a patch version.** `1.97.1` will go stale; the repo will sit on it until someone
   bumps deliberately. That is the intent, but it trades "surprise breakage" for "silent staleness."
   A scheduled job that tests against `stable` and opens an issue on divergence would give early
   warning without gating merges. Out of scope here; worth a P-002 decision.
3. **Three workflows still float.** `api-server-ci.yml`, `release.yml` and `lean4.yml` all still use
   `stable`. **`release.yml` is the one that matters** — unpinned release builds are not
   reproducible, and a release could be cut on a different compiler than CI ever tested. Also,
   `api-server-ci.yml` and `release.yml` set `RUST_TOOLCHAIN: stable` and then *hardcode*
   `toolchain: stable`, so their env var is decorative. Left untouched [D1]; recommend a follow-up.
4. **The `..` fix is on consensus code.** I am confident it is behaviour-preserving, but D5 says
   consensus code is consensus code even when the change looks cosmetic. A second reader on that
   one-line diff costs nothing.

---

## 8. What I did NOT do, and why

- **Did not fix the two RUSTSEC advisories.** Guardrails forbid dependency bumps; that is P-002
  STEP 3, which also requires pulling the advisory text for `quinn-proto` first.
- **Did not write `deny.toml`.** Explicitly P-002.
- **Did not remove `continue-on-error` from the deny step.** Same reason — reported instead.
- **Did not touch `api-server-ci.yml`, `release.yml`, or `lean4.yml`.** Out of the packet's stated
  scope [D1]. Reported in §7.
- **Did not merge.** Draft PR #55; merging is Phase D.
- **Did not amend `LOOP_SPEC.md` §2 with the corrected 982 figure.** The spec lives on the P-000
  branch and this packet branches from `main`; editing it here would create the exact conflict the
  ordering plan was designed to avoid. Flagged in §5 for whoever lands P-000.
- **Did not establish the true test total under the old 1.91 toolchain**, so I cannot attribute the
  936→982 delta to the toolchain change versus my truncation bug. The evidence points entirely at
  the truncation, but I did not prove the counterfactual.
- **Did not run P-003-V.** Independent packet; not started, to keep this branch to one concern.

---

**STATE: CYCLE 1, PHASE D, PACKET P-002-H, PR #55 draft. Builder stops here.**
