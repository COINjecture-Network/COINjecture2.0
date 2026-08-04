# C3 BUILDER REPORT — P-023, dependabot configuration

**Cycle 2 · Phase B · Packet P-023 · Branch `fix/p023-dependabot-config` · Draft PR #56**
Base `b1aaf59b` (post-#55 main) · Commit `0bcf5cdc` · Gate: none

> ✅ **RESOLVED 2026-08-04 — this report now lives in `loop/reports/`.** PR #54 merged as `28007c36`,
> so `loop/` is on `main` and this file was moved out of the scratchpad, where it had been the only
> copy and was not in git.
>
> **§0 and §6 below are preserved as written, not rewritten.** They record what was true at the time
> the packet ran, which is the point of a builder report. Their current status:
>
> - **§0** — the blocker is **closed**. #54 was genuinely unmerged when P-023 ran; it merged
>   afterwards. The reasoning for refusing to write a LEDGER entry for a merge that had not happened
>   stands as recorded.
> - **§6** — the drafted LEDGER entry has **landed**. See `loop/LEDGER.md`, the
>   "P-000 + P-000-A + P-000-B" section. It was expanded on landing with the pre-merge verification
>   detail and the `rand` annotation, both of which post-date this draft.

---

## §0 BLOCKER — the packet premise is false. PR #54 is NOT merged.

**The prompt states "PR #54 (P-000) is MERGED under amended D12. The spec is on main."** It is not.
Verified across five independent routes before any other work:

| Route | Result |
|---|---|
| `gh api repos/.../pulls/54` | `merged=false`, `merged_at=null`, `state=open` |
| `git ls-tree origin/main` | **no `loop/` directory on `main`** |
| `git log origin/main --grep='#54'` | no merge commit — empty |
| `git log origin/main --first-parent` | tip is `b1aaf59b` (#55), previous is `28c50a12` (#53) |
| PR #54 timeline events | no `merged` and no `closed` event; last event is my own `head_ref_force_pushed` at 20:36:08Z |

`main` = `b1aaf59b611677699bd7919127cca78d7640a0c7` — the #55 merge, nothing after it.

### Two consequences, both acted on

**1. The LEDGER entry was NOT written.** `LEDGER.md` is *"an append-only record of work actually
merged."* Writing a second bounded-exception entry for a merge that has not occurred would put a
false fact into the one file whose entire purpose is being trustworthy about what landed. It would
also have to be written to a file that does not exist on `main` and cannot ride this packet's branch.
**The entry is drafted and ready in §6 below; it lands the moment #54 actually merges.**

**2. "Future prompts reference the spec by path" does not yet hold.** `loop/LOOP_SPEC.md` v1.3 exists
only on `feat/p000-loop-scaffolding`. Until #54 merges, the spec must still be carried in-prompt or
read from that branch. **The re-pasting problem is not closed.**

**P-023 itself was unaffected** — it touches only `.github/dependabot.yml` and has no dependency on
`loop/` or on #54. It was therefore built in full rather than blocked on the bad premise.

---

## §1 STEP 1 — measured before configuring [D4]

### 1a. `.github/dependabot.yml` as it stands on `main`, verbatim

```yaml
version: 2

updates:
  # ── Rust / Cargo dependencies ──────────────────────────────────────────────
  - package-ecosystem: cargo
    directory: /
    schedule:
      interval: weekly
      day: monday
      time: "06:00"
      timezone: UTC
    open-pull-requests-limit: 10
    labels:
      - dependencies
      - rust
    commit-message:
      prefix: "chore(deps)"
      include: scope
    # Group all patch updates to reduce noise
    groups:
      patch-updates:
        patterns: ["*"]
        update-types: ["minor", "patch"]
    ignore:
      # Pin major versions manually — breaking changes require review
      - dependency-name: tokio
        update-types: ["version-update:semver-major"]
      - dependency-name: serde
        update-types: ["version-update:semver-major"]
      - dependency-name: jsonrpsee
        update-types: ["version-update:semver-major"]
      - dependency-name: redb
        update-types: ["version-update:semver-major"]

  # ── GitHub Actions ─────────────────────────────────────────────────────────
  - package-ecosystem: github-actions
    directory: /
    schedule:
      interval: weekly
      day: monday
      time: "06:00"
      timezone: UTC
    open-pull-requests-limit: 5
    labels:
      - dependencies
      - github-actions
    commit-message:
      prefix: "chore(ci)"
      include: scope
    groups:
      actions-updates:
        patterns: ["*"]

  # ── Docker base images ─────────────────────────────────────────────────────
  - package-ecosystem: docker
    directory: /
    schedule:
      interval: weekly
      day: monday
      time: "06:00"
      timezone: UTC
    open-pull-requests-limit: 3
    labels:
      - dependencies
      - docker
    commit-message:
      prefix: "chore(docker)"
      include: scope
```

Three entries: cargo, github-actions, docker. **npm absent.**

### 1b. Every package manifest, by ecosystem and directory

Discovered from tracked files on `main` (`git ls-files`, so no `node_modules` contamination).

| Ecosystem | Directory | Manifests | Pre-existing entry? |
|---|---|---|---|
| **cargo** | `/` | `Cargo.toml` + 15 member manifests, one root `Cargo.lock` | ✅ yes |
| **npm** | `/web-wallet` | `package.json`, `package-lock.json` | ❌ **none** |
| **npm** | `/web/coinjecture-evolved-main` | `package.json`, `package-lock.json` | ❌ **none** |
| **github-actions** | `/` | `ci.yml`, `api-server-ci.yml`, `lean4.yml`, `release.yml` | ✅ yes |
| **docker** | `/` | `Dockerfile`, `Dockerfile.api` | ✅ yes |
| docker (archived) | `/docker/archive` | 6 × `Dockerfile.*` | ❌ deliberately not configured |
| **python** | — | **no manifest exists** | ❌ **impossible** |

**The two known npm roots are confirmed, and there are no others.** Exactly two tracked
`package.json` files exist, both with lockfiles.

Cargo needs no per-member entries: `directory: /` resolves the whole workspace through the single
root `Cargo.lock`. Per-member entries would be redundant and would fragment grouping.

**Python is a coverage gap that configuration cannot close.** 17 `.py` files under `scripts/` and
`tests/harness/` import third-party packages — `requests`, `huggingface_hub` — and there is **no**
`requirements.txt`, `pyproject.toml`, `setup.py`, `Pipfile`, or lockfile anywhere in the repo. With
no manifest, Dependabot has nothing to parse, so no `pip` entry is possible. Those dependencies are
entirely ungoverned, and **CodeQL scans Python code while nothing at all watches Python
dependencies.** Closing this needs a manifest first; it is not a config change. Candidate
**DARQ-024**.

### 1c. Open alerts by ecosystem — from the API, not the banner

**Exact query:**
```bash
gh api "repos/COINjecture-Network/COINjecture2.0/dependabot/alerts?state=open&per_page=100" --paginate
```

**Total open: 23** (2026-08-03, post-#55).

| Ecosystem | Count | high | medium | low |
|---|---|---|---|---|
| npm | **21** | 11 | 9 | 1 |
| rust | **2** | 1 | 0 | 1 |

By manifest:

| Manifest | Alerts | Distinct packages | Patch available |
|---|---|---|---|
| `web/coinjecture-evolved-main/package-lock.json` | 11 | 6 | 10 yes / **1 no** |
| `web-wallet/package-lock.json` | 10 | 6 | 9 yes / **1 no** |
| `Cargo.lock` | 2 | 2 | 2 yes |

**Drift on 2026-08-03 alone: 19 → 20 → 23.** Recorded as observed. I cannot distinguish newly
published advisories from a GitHub rescan completing from this data, and say so rather than pick one.
What is certain is the direction: **it rose ~21% in a few hours while the updater was dead.**

### 1d. Ecosystems with no Dependabot entry

- **npm — has manifests, had no entry.** This is DARQ-023 and P-023 closes it. Note the asymmetry
  that made it invisible: npm *security* updates were already running (that is why failing runs are
  named `npm_and_yarn in /web-wallet` despite npm being absent from the config), so the ecosystem
  looked covered. Only *version* updates were missing — the routine bumps that would have pre-empted
  several of these 21 alerts before they ever became alerts.
- **python — has third-party dependencies, no manifest, so no entry is possible.** Not closable here.

---

## §2 STEP 2 — the configuration, and the reasoning behind each choice

Full diff: `C3-diff.patch` (149 insertions, 3 deletions, one file).

### Schedules, per ecosystem

| Ecosystem | Schedule | Reasoning |
|---|---|---|
| cargo | weekly, **Monday** 06:00 UTC | Unchanged. 519 crate deps and a Rust bump can reach consensus code — these want one deliberate review slot, not a daily trickle. |
| npm ×2 | weekly, **Tuesday** 06:00 UTC | **Staggered deliberately.** Two roots, 21 alerts, the largest surface. Landing npm on the same morning as three other ecosystems makes the biggest wave compete with everything else for attention. |
| github-actions | weekly, Monday | Unchanged. Low churn, low risk. |
| docker | weekly, Monday | Unchanged. Base images move slowly. |

Staggering affects **version** updates only — security updates fire on advisory publication, not on
this schedule. It is a review-ergonomics choice, not a security one, and is described as such.

### `open-pull-requests-limit`, and the documentation question — SETTLED

**The stop condition "the security-vs-version-update limit question cannot be settled from
documentation" did NOT fire.** GitHub's documentation is explicit:

> *"Security update pull requests are not subject to this limit and do not count toward it. There is
> no limit on the number of open pull requests for security updates."*

**`open-pull-requests-limit` applies to VERSION updates only.** No value in this file — including the
ones I set — can prevent a security update PR from being created. That is what makes the limits
below safe to set at all.

| Ecosystem | Limit | Reasoning |
|---|---|---|
| cargo | 10 | Unchanged. |
| npm ×2 | 10 each | Largest surface, two roots; generous enough not to starve version updates behind the backlog. |
| github-actions | 5 | Unchanged. |
| docker | 3 | Unchanged. |

### Grouping — the actual throttle

Grouping **batches, it never drops**: a grouped update still arrives, as one PR instead of N. Every
ecosystem now carries two groups:

- **`applies-to: version-updates`** — `patterns: ["*"]`, `update-types: [minor, patch]`. Majors are
  deliberately **excluded** so they arrive individually and get judged on their own merits.
- **`applies-to: security-updates`** — `patterns: ["*"]`. **This is the line that governs the
  post-DARQ-022 backlog.** Without it, 14 distinct vulnerable packages produce ~14 PRs at once. With
  it, 3.

### `ignore` — verified, unchanged, and now guarded by a comment

This is where the real risk was, and it is **not** where the prompt anticipated it.

Two documented statements are in apparent tension:

> *"You can configure Dependabot to ignore those dependencies when it opens pull requests for version
> updates **and security updates**."*

> *"`update-types` only affects **version** updates, not **security** updates. Security updates will
> always be created regardless of the `update-types` setting."*

**Resolution: an `ignore` entry with a bare `dependency-name` blocks both. An `ignore` entry scoped
with `update-types:` blocks version updates only.** All four pre-existing cargo ignores — tokio,
serde, jsonrpsee, redb — carry `update-types: ["version-update:semver-major"]`. **They are therefore
safe: none of them can suppress a security update.**

That safety is **conditional and fragile**, so the config now says so in a header comment:

> ⚠️ **DO NOT delete `update-types:` from an ignore rule.** Without it the rule becomes a blanket
> ignore that WILL suppress security updates for that crate — silently, with no failing job to notice
> it.

A one-key deletion turning a review policy into a silent security-update suppressor is precisely the
DARQ-020 / DARQ-022 failure shape: a mechanism that keeps appearing to work while no longer working.

**P-023 adds no `ignore` rules and removes none** [D1]. **The stop condition "any config you would
write could suppress a security update" did NOT fire.**

### Machine-checked before commit

```
BLANKET IGNORES (would suppress security updates): NONE — every ignore is scoped by update-types
SECURITY-UPDATE GROUP COVERAGE: OK on all 5 entries
ECOSYSTEM COVERAGE: cargo=['/'], npm=['/web-wallet','/web/coinjecture-evolved-main'],
                    github-actions=['/'], docker=['/']
```

### Two entries, not `directories:`

`directories` (plural) is documented and would express the two npm roots in one entry. I used **two
separate `directory:` entries anyway.** This config has to parse correctly on the *first* run after
the DARQ-022 unblock; a parse error there costs another blind window, and the whole point of
sequencing P-023 before P-022 is that the first run is already governed. **~25 lines of duplication
is the cheaper risk than a novel key on the one run that matters.** If Al prefers the compact form,
it is a two-minute change.

---

## §3 STEP 3 — PREDICTION for the first successful post-DARQ-022 run

On record so the actual result is *checkable* rather than merely observed.

### Security updates — fire immediately on unblock, independent of schedule

| Root | Alerts | Distinct vulnerable packages | **Predicted PRs** |
|---|---|---|---|
| cargo `/` | 2 | 2 (`quinn-proto` → 0.11.15, `rand` → 0.8.6) | **1** grouped |
| npm `/web-wallet` | 10 | 6 | **1** grouped |
| npm `/web/coinjecture-evolved-main` | 11 | 6 | **1** grouped |
| github-actions `/` | 0 | 0 | 0 |
| docker `/` | 0 | 0 | 0 |

**Predicted total: 3 security PRs.** Ungrouped this would be **14** — one per distinct vulnerable
package. That reduction is the packet's entire value.

**Predicted to remain open afterwards: 2 alerts** — one in each npm root has **no patched version
available**. Dependabot cannot fix them; they need manual triage or an accepted-risk decision. **If
the alert count drops to 0, that is a signal something is wrong, not a success.**

### Version updates — fire on schedule

- **Monday:** cargo → 1 grouped minor/patch PR + N individual majors (excluding the 4 ignored);
  github-actions → 1 grouped PR; docker → 1 grouped PR.
- **Tuesday:** each npm root → 1 grouped minor/patch PR + N individual majors.

**N is not predicted.** It depends on resolving each dependency graph against current registry state,
which I have not done and will not guess at. Bounded above by the limits: cargo ≤10, npm ≤10 per
root, actions ≤5, docker ≤3.

### Falsifiers — what would prove this prediction wrong

1. More than 3 security PRs → grouping is not applying to security updates as documented.
2. Fewer than 3 → an ecosystem is not being scanned at all; re-check coverage.
3. 0 alerts remaining → the 2 no-patch alerts were closed by something other than a fix.
4. Any `Dockerfile.api` bump appearing → resolves the open Docker question *in favour of* coverage.
5. No Docker PR ever, despite a stale base image → suggests `Dockerfile.api` is **not** covered.

---

## §4 FINDING — cargo audit and Dependabot disagree about Rust. ⚠️ Material to P-002.

Not sought; surfaced while computing the prediction.

| Crate | Version in `Cargo.lock` | cargo audit (RustSec) | Dependabot (GHSA) |
|---|---|---|---|
| `quinn-proto` | 0.11.14 | ✅ RUSTSEC-2026-0185 | ✅ GHSA-4w2j-m93h-cj5j → 0.11.15 |
| `crossbeam-epoch` | 0.9.18 | ✅ RUSTSEC-2026-0204 | ❌ **not reported** |
| `rand` | **0.8.5** | ❌ **not reported** | ✅ GHSA-cq8v-f236-94qc → 0.8.6 |

**All three versions verified present in `Cargo.lock`.** Each scanner reports exactly two, and
**neither reports all three.**

**Why this matters beyond bookkeeping:**

1. **P-002 is scoped to "resolve the two RUSTSEC advisories."** Doing exactly that leaves `rand`
   0.8.5 unpatched, because `cargo audit` never mentions it. **P-002's scope, as written, is
   incomplete.**
2. **The LEDGER entry for #55 lists "the two advisories."** That is accurate *as a record of what the
   Security Audit job reported* — which is what it claims to be — but it understates Rust exposure by
   one crate. It should be annotated, not corrected, when #54 lands.
3. **§9's reconciliation premise needs revising.** It assumes the Dependabot-vs-cargo-audit gap is
   *inter*-ecosystem: "cargo audit sees Rust, the ~16 gap is JS/Python." Reality: there is an
   **intra-ecosystem gap too.** Two Rust scanners, three Rust vulnerabilities, two-out-of-three each.
   **Neither tool is a superset of the other, so P-002 must union them rather than pick one.**

This is the third instance of the same pattern this cycle: a gate that appears to be reporting
completely, and is not.

---

## §5 GUARDRAIL COMPLIANCE

| Guardrail | Status |
|---|---|
| `.github/dependabot.yml` only | ✅ verified — `git status` showed exactly one modified file before commit |
| No submodule change (that is P-022) | ✅ `.gitmodules` and both gitlinks untouched |
| `.dockerignore` / rsync excludes left in place | ✅ untouched — dead no-ops, they stay [D1] |
| Draft PR, no merge [D10, D12] | ✅ **PR #56, draft** |
| One packet, one concern [D1] | ✅ no ignore rules added or removed; Docker coverage unchanged |
| Ferry `C3-builder.md` + `C3-diff.patch` | ✅ both in scratchpad — see §0 for why not `loop/reports/` |

**Stop conditions — none fired:**
- *More package roots than the config can cleanly cover* — no. Five entries cover every ecosystem
  that has a manifest. Python has no manifest, which is a gap but not an uncoverable root.
- *Limit question unsettleable from docs* — no. Settled and quoted (§2).
- *Any config could suppress a security update* — no. Verified three ways: limits do not apply to
  security updates; groups batch rather than drop; every `ignore` is scoped by `update-types`.

---

## §6 DRAFT — the LEDGER entry, ready for when #54 actually merges

**Not written to `LEDGER.md`.** Held here until the merge it describes has occurred.

```markdown
| 2026-08-0X | 1 | **P-000 + P-000-A** | — (loop scaffolding) | 0 — process packet | [#54](...) | `<merge-sha>` | 982 → 982 / 0 / 4 | `SKIPPED — docs-only; no behaviour to verify` |
```

### P-000 — merged under the amended D12 bounded exception ⚠️ deferred red

**Second application of amended D12.**

| | |
|---|---|
| Green at merge | Lint · Test ×2 · Build ×2 · Docker Build · Analyze ×4 · CodeQL |
| **Red at merge** | **`Security Audit`** — RUSTSEC-2026-0185, RUSTSEC-2026-0204 |

**D12's three conditions, checked individually:**

1. **Correctly attributed** — ✅ P-000 is docs-only under `loop/`; it cannot affect `cargo audit`. It
   inherited this red from `main` at the moment it rebased onto `b1aaf59b`. **Cleaner than #55's
   case: #55 at least touched CI config, whereas P-000 touches no code at all.**
2. **Tracked to a named packet** — ✅ P-002, already itemised in the #55 entry above with advisory
   IDs and fix versions. ⚠️ **Annotate against P-023's §4 finding: `rand` 0.8.5 is a third vulnerable
   Rust crate that `cargo audit` does not report. P-002 must union both scanners.**
3. **Outside the current packet's scope** — ✅ a documentation packet cannot bump `quinn-proto`.

**Consequence:** `main` remains red on `Security Audit` until P-002 lands. Expected, logged, owned —
and per the standing obligation from the #55 entry, **P-002 must re-enumerate from scratch rather
than trust the recorded list.** §4 above is the first vindication of that obligation: the list was
already incomplete.

---

## §7 Two-to-four things I want a second opinion on

1. **Is grouping security updates the right call at all?** It is the packet's main throttle and it
   cannot suppress — but it does mean one PR carrying 6 package bumps, where a reviewer might rubber-
   stamp the batch in a way they would not rubber-stamp six individual PRs. Grouping trades review
   *volume* for review *granularity*. I judged volume the bigger risk given a 21-alert backlog. **If
   Al disagrees, dropping the three `applies-to: security-updates` groups is a three-line change** —
   and it should be Al's call, not mine, because it is a judgement about how the team reviews rather
   than a technical constraint.

2. **The four cargo `ignore` rules deserve a decision now that their mechanics are documented.** They
   are safe today. But "safe because a sub-key is present" is a fragile invariant guarded only by a
   comment I wrote. Options: leave as-is; add a CI assertion that every `ignore` carries
   `update-types`; or drop the ignores entirely and handle majors by review. **I did not act — out of
   scope [D1] — but this is now a known conditional hazard rather than an unexamined one.**

3. **Docker `Dockerfile.api`.** I left coverage exactly as it was and flagged it, because widening it
   speculatively would be a scope change made on an assumption. But if `Dockerfile.api` is the API
   server's production image, an uncovered base image there is a real gap. **The first successful run
   settles it empirically at zero cost — worth explicitly checking rather than forgetting.**

4. **Python has no manifest and real third-party dependencies.** Creating one is a genuine change
   (pins versions, may affect the HF scripts and the test harness), so it is its own packet, not a
   config line. **Worth registering as DARQ-024 before it is forgotten** — it is currently the only
   ecosystem in the repo with *zero* dependency governance of any kind.

---

## §8 What I did NOT do, and why

- **Did not write the LEDGER entry** — the merge it describes has not happened (§0). Drafted in §6.
- **Did not merge anything.** PR #56 is a draft.
- **Did not touch `.gitmodules`, the gitlinks, `.dockerignore`, or the rsync exclude** — that is
  P-022 [D1].
- **Did not remove or modify any `ignore` rule** — pre-existing, out of scope, now documented.
- **Did not widen Docker coverage** — unverifiable today; flagged instead of guessed.
- **Did not add a `pip` entry** — impossible, no manifest exists.
- **Did not configure `docker/archive/`** — archived artifacts, not runtime images; would be noise.
- **Did not predict the major-version PR count** — would require resolving each dependency graph
  against live registry state. An invented number would be exactly the kind of unmeasured figure §2's
  measurement discipline exists to prevent.
