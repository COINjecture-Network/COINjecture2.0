# Cycle 0 — BUILDER report

Seat: BUILDER (read-only). Date: 2026-08-02.

---

## 1. Ground truth

| | Value |
|---|---|
| Repo | `C:\Users\LEET\COINjecture2.0-network` |
| Remote | `https://github.com/COINjecture-Network/COINjecture2.0.git` |
| **HEAD SHA** | **`28c50a122f2caab70582e8215b670b0ddc4d236d`** |
| Branch / worktree | `main`, in sync with `origin/main`, **clean** |
| `Cargo.lock` SHA-256 | `9930a209663dd812d03dd654d5ea8f850152667de455191b7c4645eb1cdb1bea` |

**Zero drift from the Codex baseline.** The Codex triage plan binds its scan to revision
`28c50a12…` and `Cargo.lock` SHA-256 `9930a209…`. Both match exactly. Every verdict below is
against the same tree both audits examined.

### Baseline vs expected

| Metric | Expected | Measured | Δ |
|---|---|---|---|
| Passing tests | 951 | **936** (+4 ignored, 0 failed, 30 binaries) | **−15** |
| Lean theorems | 586 | **89** | **−497** |
| Crates | 15 | **15** | ✅ |

I have **not** adopted either number.

- **Tests (−15).** `cargo test --workspace --locked --no-fail-fast` → 936 passed / 0 failed /
  4 ignored across 30 test binaries. The suite is *green*; the count is simply lower. 873
  `#[test]`/`#[tokio::test]` attributes exist in source, and parameterised/generated cases make up
  the rest. A 15-test gap is consistent with 951 having been measured on a different revision or
  with a different feature set — but I cannot confirm that, so it is reported as a discrepancy.
- **Lean (−497).** This one is not a rounding difference. `lean4/` contains **19 `.lean` files**
  with **89** `theorem`/`lemma` declarations total; the largest single file has 14. There is no
  plausible counting convention that reaches 586 from this tree. Either the 586 figure is from a
  different artifact (the `DARQ-LV-001_COINjecture_v2.6_Lean_Audit.pdf` in `~/Downloads` is a
  candidate — it is a *v2.6* audit, not v4.8.4), or the Lean corpus shrank. **This needs a human
  to reconcile; I did not go hunting for it.**

### ⚠️ Repo identification — read this

There are **two** COINjecture 2.0 clones on this machine, both at workspace version `4.8.4`, both
with 15 crates:

| | `COINjecture2.0-network` ← **target** | `COINjecture2.0` |
|---|---|---|
| Remote | `COINjecture-Network/COINjecture2.0` | `Quigles1337/COINjecture2.0` |
| HEAD | `28c50a12` | `e91da7b` |
| `lean4/` | present | **absent** |
| Worktree | clean | **dirty** (`M Dockerfile`) |
| Extra worktrees | none | **20** `claude/*` worktrees |

I selected `-network` on three independent signals: it is the only clone containing the
`fix/api-jwt-fail-closed @ ff6e65c4` branch that the project record names as SEC-PR-001; it is the
only clone whose HEAD **and** `Cargo.lock` hash match the Codex scan baseline; and it is the only
one with a Lean corpus at all. **If Al intended the `Quigles1337` clone, every verdict below is
void** — that tree is dirty, has 20 live worktrees, and its `node/src/chain.rs` and
`tokenomics/src/rewards.rs` differ substantially in size.

---

## 2. Existing-work inventory

### Branches matching `fix/*`, `sec-*`, `SEC-PR-*` (after `git fetch --all --prune`)

| Branch | Where | HEAD | Files touched | Intent |
|---|---|---|---|---|
| `fix/api-jwt-fail-closed` | local only | `ff6e65c4` | `api-server/src/config.rs` (+246), `jwt.rs` (+43), `main.rs` (+3), `tests/config_startup.rs` (+44) | SEC-PR-001 — reject empty/placeholder `SUPABASE_JWT_SECRET` at startup before bind |
| `origin/fix/readme-dimensional-pool-label` | remote | `b8f3d999` | README only | docs |
| `origin/fix/readme-marketplace-atomicity-language` | remote | `bb3f838a` | README only | docs |

No `sec-*` or `SEC-PR-*` branch exists anywhere. Worktrees: **one** (the main checkout). The
worktree the SEC-PR-001 fix report was authored in
(`C:\Users\LEET\OneDrive\Documents\COINjecture2.0-sec-pr-001`) **no longer exists** and is not
registered — consistent with the OneDrive repo eviction; `git worktree prune --dry-run` is clean.

### 🚩 SEC-PR-002 … 005 do not exist as code

The premise in the order — that SEC-PR-001..005 "already exist from the Codex remediation" — is
**half true**. Only SEC-PR-001 was implemented. The other four are *specifications only*, in
`…-triage/first-five-prs.md`, under the header:

> *"Do not implement from this document without explicit authorization."*

The SEC-PR-001 fix report confirms it directly:

> *"No sibling finding or SEC-PR-002 through SEC-PR-005 work was included."*

**And SEC-PR-001 itself is unmerged** — `git merge-base --is-ancestor ff6e65c4 main` → false.
There is no security remediation on `main` at all.

### Overlap flag: SEC-PR-002 ↔ C7 — **PARTIAL, and it does not close C7**

The order anticipated that SEC-PR-002 ("make node RPC auth fail-closed") likely overlaps C7
(unauthenticated RPC marketplace/faucet methods). **It does not.** They fix different layers:

- **SEC-PR-002** targets `rpc/src/middleware.rs` `SecurityConfig` — a *transport* bearer-key gate
  (`RPC_REQUIRE_AUTH`, `RPC_API_KEYS`). Its finding IDs are `csf_c35bded78cd790abb52fe9b1`
  (auth default-open) and `csf_8dc6eb9ab34e150bf775be40` (empty key set fails open).
- **C7** is about *per-method principal binding*. `marketplace_submitSolution` takes the payee as
  a caller-supplied string and credits it (`rpc/src/server.rs:1556`+):

```rust
async fn submit_solution(&self, params: SolutionSubmissionParams) -> RpcResult<bool> {
    Self::validate_str_len(&params.solver, 256, "solver")?;
    let solver = self.parse_address(&params.solver)?;          // ← caller picks the payee
    …
    let (solver_addr, bounty) = self.state.marketplace_state.claim_bounty(problem_id)…;
    let current_balance = self.state.account_state.get_balance(&solver_addr);
    let new_balance = current_balance + bounty;                 // ← credited, no signature
```

A shared API key authenticates the *connection*, not the *account*. Once SEC-PR-002 lands, any
holder of a valid RPC key — every legitimate client — can still name an arbitrary `solver` and
take the bounty. **C7 needs its own packet.** Recording it as "covered by SEC-PR-002" would be a
cross-reference error that silently drops a Critical.

---

## 3. Verification verdicts

### (a) C3 — Inconsistent address derivation → **CONFIRMED** (worse than reported)

The audit reported two derivations. There are **three**.

**Derivation 1 — raw pubkey (identity).** `core/src/types.rs:46-48`:

```rust
pub fn from_pubkey(pubkey: &[u8; 32]) -> Self {
    Address(*pubkey)
}
```

Reached from `core/src/crypto.rs:29-31` (`KeyPair::address`) and `:51-53`
(`PublicKey::to_address`), both of which just delegate:

```rust
pub fn to_address(&self) -> Address {
    Address::from_pubkey(&self.0)
}
```

**Derivation 2 — SHA-256.** `wallet/src/keystore.rs:416` and `node/src/genesis.rs:37-43`:

```rust
// Derive address using SHA256 (same as wallet/src/keystore.rs derive_address)
let mut hasher = Sha256::new();
hasher.update(&public_key_bytes);
let address_hash = hasher.finalize();
```

**Derivation 3 — BLAKE3 (not in the audit).** `node/src/keystore.rs:108-113` and
`state/src/escrows.rs:469-474`:

```rust
/// Derive an `Address` from a 32-byte ed25519 public key using BLAKE3.
///
/// This matches the derivation used by the node's validator keystore.
fn address_from_pubkey(pubkey: &[u8; 32]) -> Address {
    let hash = blake3::hash(pubkey);
```

That doc comment is **true** — `node/src/keystore.rs` does use BLAKE3 — which makes it worse, not
better: the validator's own on-chain identity is a BLAKE3 address, while the transaction
authentication path that must recognise it derives raw. Note also that the audit's cited
SHA-256 site `core/src/crypto.rs:423` **does not exist** — that file is 397 lines and contains no
SHA-256 at all.

**Three or more sites disagree ⇒ CONFIRMED.**

**Consensus impact.** Derivation 1 is the one that matters, because it is on the transaction
authentication path — `core/src/transaction.rs` at lines 438, 510, 589, 664, 798, 931, 1100, all
of the form:

```rust
if self.from != self.public_key.to_address() {
```

So consensus authenticates against the raw-pubkey address, while the wallet shows the user a
SHA-256 address and genesis funds a SHA-256 address.

**Is there a canonical helper?** *Partly — and that is the answer to the fix-shape question.*
`Address::from_pubkey` in `core` **is** the natural canonical helper and is already public API,
but only `core` calls it. The other four sites open-code their own derivation and never reference
it. So the fix is **one helper plus four call sites**, not one function:

| Site | Current | Action |
|---|---|---|
| `core/src/types.rs:46` | raw | change the body — this is the canonical helper |
| `wallet/src/keystore.rs:416` | SHA-256 | delete, call the helper |
| `node/src/genesis.rs:38` | SHA-256 | delete, call the helper — **changes the genesis address** |
| `node/src/keystore.rs:108`, `:402` | BLAKE3 | delete, call the helper |
| `state/src/escrows.rs:469` | BLAKE3 | delete, call the helper |

This is consensus- **and** genesis-breaking. It cannot be retrofitted after a chain that matters
is started.

### (b) C1 — Miners never required to solve the assigned problem → **CONFIRMED**

Entry point is `NodeValidator::validate_block` in `node/src/validator.rs`. Step 4 verifies the
solution against the **submitted** problem, and nothing regenerates it (`:137-143`):

```rust
// 4. Validate NP-hard solution
if !block
    .solution_reveal
    .solution
    .verify(&block.solution_reveal.problem)   // ← the miner's own problem
{
    return Err(ValidationError::InvalidSolution);
}
```

Step 5 binds problem+solution+parent-hash into a commitment, which defeats *pre-mining* but does
not constrain **which** problem was used:

```rust
let epoch_salt = block.header.prev_hash;
if !block.solution_reveal.commitment.verify(
    &block.solution_reveal.problem, &block.solution_reveal.solution, &epoch_salt,
) {
```

**A canonical generator already exists and is already deterministic** —
`consensus/src/miner.rs:663`:

```rust
/// DETERMINISM: Seeded by parent hash + height to ensure all nodes generate the same problem
pub async fn generate_problem(&self, block_height: u64, prev_hash: Hash) -> ProblemType {
```

Every call site is a **mining** path — `miner.rs:824`, `node/src/service/mining.rs:23`,
`node/src/service/mod.rs:1019`, and one test at `miner.rs:1262`. The string `problem` appears in
`validator.rs` only at `:19`, `:140`, `:147`, `:150` (and unrelated marketplace code at `:750`+).
**No validation path calls it.**

**Function that would need to do it:** `NodeValidator::validate_block` in
`node/src/validator.rs`, as a new step between the current 4 and 5. The blocker is structural, not
algorithmic: `generate_problem` is an `async` method on `consensus::Miner` that borrows
`self.difficulty_adjuster`, so a validator cannot call it as-is. It must be lifted into a free
deterministic function over `(height, prev_hash, difficulty_params)` that both seats can call —
and the difficulty input must itself become consensus-visible, or validators will regenerate a
different instance than the miner did.

### (c) C2 — `work_score` self-reported and drives fork choice → **CONFIRMED**

**Validation is exactly as claimed — finite and `>= 0`, nothing more.**
`core/src/validation.rs:341-346`:

```rust
// Work score: finite and non-negative
if !work_score.is_finite() {
    return Err(ValidationError::NonFiniteWorkScore(work_score));
}
if work_score < 0.0 {
    return Err(ValidationError::NegativeWorkScore(work_score));
}
```

The only other check is a floor that is **set to zero**, `node/src/validator.rs:64` and `:158`:

```rust
min_work_score: 0.0, // Allow all work scores (PoW hash is primary validation)
…
if block.header.work_score < self.min_work_score {
```

**Read site 1 — fork choice.** Reorg is decided on summed header values,
`node/src/service/fork.rs:463`:

```rust
if peer_state.cumulative_work > local_work
```

with the accumulation at `node/src/chain.rs:476`:

```rust
let inc = (block.header.work_score.max(0.0) as u64) as u128;
let new_cum = prev_cum.saturating_add(inc);
```

**Read site 2 — issuance.** `node/src/validator.rs:179`:

```rust
let expected = RewardCalculator::new().calculate_block_reward(block.header.work_score, w);
```

feeding `mint_atoms = ⌊w_trunc · S · K / isqrt(W_parent)⌋` in `tokenomics/src/rewards.rs`.

**Every read site of `block.header.work_score`** (non-test): cumulative work / fork choice —
`chain.rs:131, 189, 242, 476, 1085`, `chain_adzdb.rs:126`, `service/fork.rs:1053, 1057, 1195,
1281, 1321, 1365, 1447, 1470`, `light_sync.rs:568`; issuance — `validator.rs:179`; threshold —
`validator.rs:158`; telemetry only — `metrics_integration.rs:115, 215`, `service/mining.rs:342`.

**Does any recomputation exist?** A calculator exists — `consensus/src/work_score.rs:94`
`WorkScoreCalculator`, with `block_work_score(problem_size, solve_time_us, verify_time_us)` — but
its only non-test consumers are `consensus/src/miner.rs:592/604` (the **miner**, computing the
value it then self-declares) and `consensus/benches/`. **No validator or fork-choice path calls
it.** The value is produced by the party it benefits and consumed without challenge.

See `PACKETS.md` for why the audit's recommended fix ("recompute from verified inputs") may not be
achievable as written — `solve_time` is miner wall-clock and is not verifiable by anyone else.

---

## 4. Registry summary

**33 findings / 19 root causes.** (7 Critical + 11 High + 10 Medium + 5 Low = 33 ✅ matches the
audit's own severity table.)

3 of 19 root causes carry a verified `Location` this cycle (RC-01/C1, RC-02/C2, RC-03/C3). The
other 16 carry `Location: UNVERIFIED` by design. Rollup to the Codex programs: P1 ×12 rows,
P2 ×2, P3 ×2, P4 ×2, P5 ×2.

---

## 5. Dependency baseline (inventory only — nothing changed)

| | |
|---|---|
| `cargo-audit` | **installed**, `cargo-audit-audit 0.22.2` |
| `cargo-deny` | **installed**, `cargo-deny 0.20.2` |
| `deny.toml` | **absent** — `cargo deny` cannot run without one |
| `.cargo/audit.toml` | **present**, with 2 suppressions |

`cargo audit` over 519 dependencies:

| Severity | ID | Crate | Fix |
|---|---|---|---|
| **High (7.5)** | RUSTSEC-2026-0185 | `quinn-proto 0.11.14` — remote memory exhaustion via unbounded out-of-order stream reassembly | `>=0.11.15` |
| Unscored vuln | RUSTSEC-2026-0204 | `crossbeam-epoch 0.9.18` — invalid pointer deref in `fmt::Pointer` | `>=0.9.20` |
| Warn (unmaintained) | RUSTSEC-2025-0141 | `bincode 1.3.3` | — |
| Warn (unsound) | RUSTSEC-2026-0190 | `anyhow 1.0.102` — `Error::downcast_mut()` | — |
| Warn (yanked) | — | `spin 0.9.8` | — |

**Result: 2 vulnerabilities, 3 allowed warnings.** Both suppressions in `.cargo/audit.toml`
(`RUSTSEC-2026-0097` rand, `RUSTSEC-2023-0071` rsa) carry written justifications. I did not
evaluate whether those justifications still hold — that is a P-002 decision, not mine.

Note for whoever picks this up: `bincode` being unmaintained is not only a dependency-hygiene
item. It is the serializer in M7 (dual bincode/JSON hashing, RC-17), so RC-17 and the `bincode`
advisory should be considered together.

---

## 6. What surprised me

1. **The order's premise about SEC-PR-002..005 is wrong.** They are specs, not branches. And
   SEC-PR-001 — the one thing that *was* built — has never been merged. `main` carries zero
   security remediation. Anyone reasoning about "what's already fixed" is currently reasoning
   about a fix set that exists only on paper and on one unmerged local branch.
2. **C3 is a three-way split, not two-way.** The audit missed the BLAKE3 derivation entirely, and
   BLAKE3 is what the *validator's own keystore* uses. The audit's cited location
   `core/src/crypto.rs:423` does not exist (file is 397 lines).
3. **The line numbers in the third-party audit have drifted or were never right.** `crypto.rs:423`
   is past EOF; `tokenomics/src/rewards.rs:47` (C2) points at
   `header_work_score_trunc_u128`, not the reward calculation, and that file is 126 lines here but
   exactly 47 in the *other* clone. Treat every unverified `Location` in the registry as a lead,
   not a fact — this is precisely why 16 rows say `UNVERIFIED`.
4. **An uncited instance of C5.** Verifying C7's overlap, I hit `rpc/src/server.rs:1583`:
   `let new_balance = current_balance + bounty;` — raw `u128` add on a bounty payout. C5 cites
   `state/src/accounts.rs` and `accounts_adzdb.rs`; this RPC path is not in its location list.
   Logged, not hunted.
5. **`validator.rs` has debug telemetry wired into a consensus path.** Lines 102-130 open a file
   and append JSON on every `InvalidPrevHash`, with hardcoded `"sessionId": "debug-session"`,
   `"runId": "run1"`, `"hypothesisId": "A"`, and a stale self-reference `"location":
   "validator.rs:85"`. A peer that can feed bad-parent blocks drives unbounded disk writes. This
   is adjacent to RC-10 but is in neither audit. Not investigated further.
6. **The Lean gap is not explainable by counting convention** — see §1.

## 7. What I did NOT do, and why

- **No branch, no code, no PR, no commit.** Cycle 0 is read-only.
- **`loop/` files are written to the working tree but NOT committed.** The order says "commit
  LOOP_SPEC.md as given", but (a) no `LOOP_SPEC.md` text was supplied, and (b) committing would
  need either a branch (forbidden) or a direct `main` commit (not authorized). `loop/` currently
  shows as untracked. Al's call.
- **`LOOP_SPEC.md` not created** — I will not invent a spec and present it as the given one. The
  `REGISTRY.md` schema is consequently *provisional*: §7 was never provided, so I designed the
  columns. Reshape before Phase B if §7 differs.
- **No dependency, CI, or `Cargo.toml`/`Cargo.lock` changes.** `cargo audit` was run read-only;
  `cargo test --locked` cannot mutate the lockfile. `cargo deny` was **not** run — `deny.toml`
  does not exist, and creating one is a P-002 decision.
- **Did not verify 30 of the 33 findings.** Only C1/C2/C3 were assigned. Their registry rows say
  `UNVERIFIED` rather than carrying the audit's line numbers as if confirmed.
- **Did not re-audit.** Two adversarial passes exist. Items in §6 are logged where I tripped over
  them, not sought.
- **Did not chase the 586-theorem or 951-test discrepancies to root.** Both are reported as
  discrepancies per the order. Reconciling them needs the provenance of those numbers, which I do
  not have.
- **Did not touch the `Quigles1337/COINjecture2.0` clone** beyond read-only identification. It is
  dirty (`M Dockerfile`) with 20 live `claude/*` worktrees. If it is in scope, that is a separate
  order — and its dirty state is a stop condition in its own right.

---

**STATE: CYCLE 0, PHASE A, awaiting-queue-approval. Builder stops here.**
