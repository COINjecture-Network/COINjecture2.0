# C4 BUILDER REPORT — P-021-V, DARQ-021 apply-path verification

**Cycle 2 · Phase A · Packet P-021-V · Branch `feat/p021v-sender-binding-verification`**
Base `28007c36` (post-#54 main) · Read-only · No fix · Prompt: `LOOP_SPEC.md` §11.1

> **Filename note.** §11.1 specifies `loop/reports/C2-p021v-builder.md`. That numbering is stale — it
> was written when P-021-V was expected to run in Cycle 2, before P-023 took the `C3` slot. Filed as
> `C4-builder.md` to match the actual report sequence (C0, C1-hotfix, C1-p003v, C3, **C4**) and the
> ferry instruction. **§11.1 should be corrected in a later docs packet** — flagged, not silently
> deviated from.

---

## 1. VERDICT — **CONFIRMED-THEFT**

**The ledger apply path debits `tx.from`.** It is indexed by the attacker-controlled field, not by any
address derived from `tx.public_key`. Nothing on any ingest path enforces the binding between them.

A transaction naming an arbitrary victim as `from`, signed by an attacker's key, is accepted by every
check that actually runs and **moves the victim's balance to the attacker.**

Demonstrated end to end by a local probe (§9), not inferred.

---

## 2. THE DEBIT SITE — exactly what indexes the account

**`node/src/service/block_processing.rs:833-838`** — `apply_single_transaction`, `Transfer` arm:

```rust
// 823  balance sufficiency check — reads the VICTIM's balance
let sender_balance = state.get_balance(&transfer_tx.from);
if sender_balance < transfer_tx.amount + transfer_tx.fee { return Err(...) }

// 833  THE DEBIT
state.set_balance(
    &transfer_tx.from,                                        // <-- indexes tx.from
    sender_balance - transfer_tx.amount - transfer_tx.fee,
)?;
state.set_nonce(&transfer_tx.from, transfer_tx.nonce + 1)?;   // <-- and the nonce

// 844  the credit
let recipient_balance = state.get_balance(&transfer_tx.to);
state.set_balance(&transfer_tx.to, recipient_balance + transfer_tx.amount)?;
```

**The account is indexed by `tx.from`. Full stop.**

**The strongest single piece of evidence:** `node/src/service/block_processing.rs` contains **zero
occurrences of the string `public_key`.** The entire apply module — Transfer, TimeLock, Escrow,
Channel, TrustLine, DimensionalPoolSwap, Marketplace, and every unwind path — never reads the signing
key. Every balance read and write in it is keyed on `.from`:

| Tx type | Debit site (apply) |
|---|---|
| Transfer | `:823`, `:835`, `:840` |
| TimeLock | `:854`, `:866`, `:871` |
| Escrow | `:898`, `:909`, `:912` |
| Channel | `:611`, `:616`, `:619` (unwind), and the apply arm below it |
| TrustLine | `:701`, `:703`, `:706` |
| DimensionalPoolSwap | `:736`, `:738`, `:741` |
| Marketplace | `:758`, `:760`, `:763`, `:775` |

**This is not confined to Transfer.** Every value-moving transaction type debits `from`.

Downstream, `state/src/accounts.rs` is a pure sink: `set_balance(&Address, Balance)`,
`transfer(from, to, amount)` and `apply_block_atomically(&[(Address, Balance)], …)` all take the
address as a parameter and write it verbatim. The state layer never had the information needed to
check the binding — **the address is decided by the caller, and the caller uses `tx.from`.**

---

## 3. WHICH DERIVATION, AND THE C3 INTERACTION

`core/src/crypto.rs:51` → `core/src/types.rs:46`:

```rust
pub fn to_address(&self) -> Address { Address::from_pubkey(&self.0) }
pub fn from_pubkey(pubkey: &[u8; 32]) -> Self { Address(*pubkey) }   // RAW — no hash
```

**The derivation is RAW 32-byte public key.** Not SHA-256, not BLAKE3. This is the same raw variant
DARQ-004 (C3) identifies as the one on the consensus transaction path.

**The C3 interaction, and it cuts the opposite way from what §11.1 anticipated.** The prompt warned
that the debit site might use a *different* derivation than `is_valid()` does, which would change what
the exploit yields. It does not — because **the debit site performs no derivation at all.** It uses
`tx.from` as a literal 32-byte key into the balances table.

That makes DARQ-021 **independent of C3, not compounded by it**:

- Under C3, an address's *meaning* is ambiguous across subsystems (raw vs SHA-256 vs BLAKE3).
- Under DARQ-021, the attacker does not need to derive anything. **They name the victim's address
  literally**, in whatever encoding the ledger already stores it in. Whatever bytes are in the
  balances table, the attacker writes those bytes into `from`.

**So fixing C3 does not mitigate DARQ-021 even slightly**, and DARQ-021 does not depend on C3 being
unfixed. They are orthogonal. Any plan that treats unifying address derivation as partial mitigation
here is wrong.

---

## 4. NETWORK REACHABILITY — per ingest route

**Two independent ingest routes reach the debit. They are not equally gated, and the more dangerous
one is not gated at all.**

### Route A — RPC `submit_transaction` → mempool → miner → block → apply

`rpc/src/server.rs:889`. Accepts `tx_hex` as **either JSON or hex-bincode**. Checks, in order:

1. 256 KB size limit
2. deserialize
3. **`tx.verify_signature()`** — `server.rs:929`. Signature only.
4. `pool.add(tx)` → `mempool/src/pool.rs`: duplicate check, fee check, **`tx.verify_signature()`
   again** (`:160`), capacity check. **`pool.rs` never reads `.from` and never checks the binding.**
5. broadcast to peers

**Authentication is fail-open by default.** `rpc/src/middleware.rs:90`:

```rust
let require_auth = std::env::var("RPC_REQUIRE_AUTH").as_deref() == Ok("true");
```

`SecurityGateLayer` *is* wired in (`server.rs:1837`), but unless the operator explicitly sets
`RPC_REQUIRE_AUTH=true`, **`require_auth` is `false`** and no bearer token is required. On a default
deployment this route is reachable by **any unauthenticated party who can open a TCP connection to the
RPC port.**

*(The fail-open default is DARQ-006 / C7 territory, not DARQ-021's to fix. Recorded here because it
sets DARQ-021's reachability, and because the two compose: DARQ-006 makes the door unlocked, DARQ-021
makes what's behind it worth reaching.)*

### Route B — a miner includes the transaction directly. **Ungated by construction.**

This is the route that sets severity, and it does not depend on Route A at all.

Block validation is `BlockValidator::validate_block_with_options` (`node/src/validator.rs:82`), which
every ingest path uses. Its transaction validation is **the entire loop at `:167-174`**:

```rust
// 8. Validate all transactions
for tx in &block.transactions {
    if !tx.verify_signature() {
        return Err(ValidationError::InvalidTransaction("Invalid signature".to_string()));
    }
}
```

**That is the whole of it.** No binding check, no nonce check against state, no sender-authorization
check of any kind.

So **any miner can place a forged-`from` transfer into a block they mine and every validating node
will accept it** — no RPC access, no mempool, no authentication, nothing to bypass. `chain_submitBlock`
(`rpc/src/server.rs:446`) additionally accepts a whole `Block` over RPC, and gossip carries blocks
between peers; all of them land on the same `validate_block_with_options`.

**Mempool admission and block application therefore differ in exposure, and the asymmetry matters:**
locking down the RPC (setting `RPC_REQUIRE_AUTH=true`) closes Route A entirely and **does nothing to
Route B.** Any participant who can mine a block can steal from any funded account. **There is no
configuration that mitigates this.**

---

## 5. DOES ANY PATH ENFORCE THE BINDING? — No. The check exists and is unreachable.

The binding check is real and correct. `core/src/transaction.rs:437-439`, inside `is_valid()`:

```rust
pub fn is_valid(&self) -> bool {
    if !self.verify_signature() { return false; }
    if self.from != self.public_key.to_address() { return false; }   // <-- THE CHECK
    if validate_transfer_fields(self.amount, self.fee).is_err() { return false; }
    true
}
```

**Reachability of `is_valid()`, traced exhaustively:**

- The only non-test caller of any transaction `is_valid()` is `core/src/block.rs:215`, inside
  `Block::verify()`.
- `Block::verify()` (`core/src/block.rs:190`) has **no caller anywhere outside `core`.** A grep for
  `.verify()` across `node/`, `rpc/`, `mempool/`, `consensus/` and `api-server/`, excluding the
  unrelated `verify()` methods on commitments, solutions, Merkle proofs, light-client proofs and mesh
  identities, returns **nothing**.
- Every ingest route uses `validate_block_with_options`, which calls `verify_signature()` only.

**`Block::verify()` is dead code on every path that touches consensus, and the binding check dies with
it.** The repository contains a correct fix for its own most severe finding and never calls it.

**Why the signed message does not save it.** `signing_message()` (`transaction.rs:413-422`) commits to
`from`, `to`, `amount`, `fee`, `nonce` **and `public_key`** — so the signature covers the forged
`from`. That is irrelevant. `verify_signature()` is `self.public_key.verify(&message, &self.signature)`:
the attacker supplies the `public_key`, the `from`, and the signature, all three consistent with each
other. **Including `from` in the signed message binds the signature to the claim; it does not bind the
claim to the signer.** Only the `from == public_key.to_address()` comparison does that.

**API-level confirmation.** `TransferTransaction::new(from, to, amount, fee, nonce, keypair)`
(`transaction.rs:383`) takes `from` as a **free parameter, independent of the keypair**. Forging costs
nothing — it is the ordinary constructor called with a different first argument.

---

## 6. PROPOSED SEVERITY — **Critical**, and it outranks everything currently open

**Reasoning, not a label:**

| Criterion | Assessment |
|---|---|
| **Impact** | Direct, unbounded theft of any funded account. Bounded only by the victim's balance (the sufficiency check at `:823-824` reads the *victim's* balance, so the attacker can take all of it). |
| **Attacker prerequisites** | An Ed25519 keypair and the victim's address. **Addresses are public** — they are raw public keys and appear in every transaction and block. No key compromise, no privileged position, no race, no capital. |
| **Reachability** | Unauthenticated RPC by default (Route A) **and** any miner unconditionally (Route B). Route B has no mitigating configuration. |
| **Detectability** | Low. Forged transfers are structurally indistinguishable from legitimate ones — valid signature, valid structure, valid block. Only a `from`-vs-`public_key` comparison distinguishes them, and nothing performs it. |
| **Persistence** | State is committed to the ledger. Theft is final on confirmation. |

**Comparison to C3 (DARQ-004, Critical):** C3 makes correctly-owned funds **unspendable by their
owner** — a liveness and correctness failure. DARQ-021 makes **anyone's funds spendable by anyone
else** — an authorization failure. Both are Critical; DARQ-021 is the more urgent of the two, because
C3's damage is confinement while DARQ-021's is transfer.

**It outranks C1/C2 (DARQ-001) as well.** Those let a miner manipulate fork-choice weight. This lets
any participant empty any account.

**Recommendation: DARQ-021 becomes the highest-severity open finding in the registry.**

⚠️ **One honest caveat on scope.** I verified the *mechanism* exhaustively in code and demonstrated it
in a probe. I did **not** verify it against a running node with live consensus, and per GATE-1 it is
still unsettled whether meaningful testnet balances exist to steal. **The defect is confirmed; the
present-day financial exposure depends on that separate open question.** It does not reduce the
severity of the defect — a chain that reaches mainnet with this is catastrophic — but it should not be
reported as "funds are being stolen right now," which is not established.

---

## 7. FIX SHAPE — one check, one place. But the placement is a real decision.

**The narrow fix is one line**, in `node/src/validator.rs`, in the loop at `:167-174`:

```rust
for tx in &block.transactions {
-   if !tx.verify_signature() {
+   if !tx.is_valid() {
        return Err(ValidationError::InvalidTransaction("...".to_string()));
    }
}
```

Because `is_valid()` already calls `verify_signature()` first, this is strictly stronger and adds the
binding plus the existing field-bounds check. **One check, one place** — every ingest route funnels
through `validate_block_with_options`, so this single edit covers RPC, gossip, `chain_submitBlock` and
sync alike.

**But there are three placements, and they are not equivalent:**

1. **Block validation** (`validator.rs:169`) — **required.** Without it, Route B stays open, and Route
   B is the ungatable one.
2. **Mempool admission** (`pool.rs:160`) — **strongly recommended, not sufficient alone.** Stops
   forged transactions propagating and keeps honest miners from unknowingly including them. Purely
   local policy; **not consensus-affecting on its own.**
3. **RPC ingest** (`server.rs:929`) — optional defence in depth, better error messages at the edge.

**Do 1 and 2. 1 is the fix; 2 is hygiene that makes the network behave sanely before the fork
activates.**

⚠️ **`is_valid()` also adds `validate_transfer_fields` (amount/fee bounds).** That is a *second*
behavioural change riding along with the binding check. It is almost certainly desirable, but it is
not the same change, and under D7 (one root cause per packet) P-021 should either state explicitly
that it is adopting both, or add only the binding comparison and leave field bounds to its own packet.
**Do not let it in unremarked.**

⚠️ **The other six transaction types have their own `is_valid()` implementations**
(`transaction.rs:503, 582, 657, 791, 924, 1093`) and their own apply paths that all debit `from`.
**P-021 must confirm each of those `is_valid()` bodies actually contains the binding check** before
relying on a single call-site swap. I verified the Transfer one in full; I did not read all seven.
**That is a deliberate boundary, not an oversight — flagged for P-021 rather than assumed.**

---

## 8. CONSENSUS IMPACT — **hard fork. GATE-2 applies.** [D5]

**Confirmed.** Adding the binding check to block validation means:

- Old nodes accept blocks containing forged-`from` transactions.
- New nodes reject them.
- A block that one accepts and the other rejects is a **chain split**, by definition.

This is `validator.rs`, which D5 names explicitly as consensus code. **Coordinated restart, not a
rolling upgrade** — the same shape as GATE-2 already carries for C1.

**Two additional questions P-021 must answer before activation, which I could not settle read-only:**

1. **Does any existing block on the testnet contain a transaction where `from != public_key.to_address()`?**
   If yes, the new rule **invalidates history** and the chain cannot simply be re-validated from
   genesis — it needs either a height-gated activation or a chain reset. This is answerable only
   against live chain data, which is Sarah's access and is **the same question GATE-1 is already
   waiting on.** ⚠️ **Note the ugly possibility: such a transaction could be there innocently — an
   ordinary wallet bug — or it could be an actual theft nobody noticed. Nothing distinguishes them
   after the fact except examining the addresses.**
2. **Height-gated or immediate?** Immediate is simpler and correct for a pre-mainnet chain with no
   history worth preserving. Given GATE-1's narrowing (no genesis allocations exist), a reset is
   cheaper than v1.2 assumed — **which makes "fix it immediately and reset" a genuinely live option,
   and probably the cleanest one.**

---

## 9. THE PROBE — written, run, deleted, never committed

Per the guardrail and the P-003-V precedent. Written to `state/tests/zz_p021v_probe.rs`, executed,
then removed; `git status` confirmed clean afterwards and it never entered the index.

**Verbatim output:**

```
running 1 test
[1] verify_signature()          = true   <- validator.rs:169 / pool.rs:160
[2] is_valid()                  = false  <- transaction.rs:431, unreachable
[2a] tx.from                    = Dad34Fiq19jj7Mt2PhJjJWAqP4EMoMsK963nYT95gHEJ
[2b] public_key.to_address()    = 9YCxry2mkhuEJrL7K2B1gt6Msd99enhiVi4Fypmpc5dn
[3] before: victim=5000 attacker=0
[3] after:  victim=4000 attacker=1000

VERDICT: signature-only validation accepts a transfer that debits an
         account the signer does not control. CONFIRMED-THEFT.
test result: ok. 1 passed; 0 failed; 0 ignored
```

The probe built a transfer with `from` = victim, signed by the attacker's key, then replicated the
apply-path debit **verbatim from `block_processing.rs:823-847`**. `[2a]` vs `[2b]` shows the two
addresses are plainly different, `[1]` shows the only check that runs passes anyway, and `[3]` shows
the victim's balance moving to the attacker.

**Probe scope, stated honestly:** it exercises the real `verify_signature()`, the real `is_valid()`,
the real `TransferTransaction::new`, and the real `AccountState`. It **replicates** the apply-path
debit rather than invoking `apply_single_transaction` directly, because that function is private to
`node::service`. **The claim that the apply path debits `tx.from` rests on reading
`block_processing.rs:833-838`, quoted in §2 — not on the probe.** The probe demonstrates that the
inputs are accepted and that the debit, as written there, takes from the victim.

---

## 10. DISTINCTNESS FROM M3 — confirmed distinct, and fixing M3 would NOT close DARQ-021

**M3 / DARQ-002 (RC-02)** is *"validation bypass on direct apply paths"* — callers reaching state
mutation **without going through the validator**.

**DARQ-021 is the opposite failure.** The validator **is** invoked — every ingest route calls
`validate_block_with_options` — and it performs **the weaker of two checks that both exist in the
codebase.**

**The decisive evidence:** even at 100% validator coverage, with every caller correctly routed and
every bypass closed, `validate_block_with_options` still runs `verify_signature()` and only
`verify_signature()` (`validator.rs:169`, confirmed by reading the whole function). **Fixing M3 as
written closes bypasses and leaves the hole exactly where it is.**

They also compose in the wrong direction: M3's fix routes *more* traffic through a validator that does
not check the binding. **Fixing M3 first, alone, could create a false sense of closure.** They should
be fixed together, or DARQ-021 first.

Registry note: DARQ-021 currently sits under `RC-02 (extends)`. **That is arguably wrong now.** RC-02
is "validation bypass"; this is "validation insufficiency." I'd propose its own root cause —
**RC-19: consensus validation performs a weaker check than the one available** — but registry schema
changes are a docs packet, not this one. **Flagged, not done.**

---

## 11. THINGS I WANT A SECOND OPINION ON

1. **The one-line fix is `verify_signature()` → `is_valid()`, and it smuggles in a second change.**
   `is_valid()` also runs `validate_transfer_fields` (amount/fee bounds). Almost certainly good, but
   it is not the binding check, and D7 says one root cause per packet. **Adopt both explicitly, or add
   only the comparison?** I lean adopt-both-and-say-so — the bounds check is defensive and the fork is
   already being paid for — but that is a judgement about packet hygiene versus fork economics, and it
   should be made deliberately rather than absorbed.

2. **I verified `is_valid()` in full for `TransferTransaction` only.** The other six types each have
   their own (`transaction.rs:503, 582, 657, 791, 924, 1093`) and each apply path debits `from`. If
   any of those six omits the binding comparison, the single call-site swap silently under-fixes and
   the packet will look complete while leaving a hole. **P-021 must read all seven.** I stopped at the
   packet boundary deliberately, but I want that decision confirmed rather than inherited.

3. **The "does history contain a forged transaction?" question is genuinely unpleasant, and I want
   agreement on how to ask it.** If such a transaction exists on the testnet, it is either an innocent
   wallet bug or an unnoticed theft, and **nothing in the block distinguishes them.** Scanning for it
   is cheap and answers both the hard-fork-activation question and a "has this already been exploited"
   question nobody has asked yet. **I think it should be run before P-021 is scoped, not after** — but
   it needs chain access I do not have, and it may surface something with disclosure implications.

4. **Severity ranking against C3, and what it implies for ordering.** I placed DARQ-021 above C3 and
   above C1/C2, and I want that challenged rather than accepted. C3 is confinement (owners cannot
   spend); DARQ-021 is transfer (anyone can spend anyone's). If that ranking holds, **P-021 should
   preempt P-003 and P-004 in the queue**, and GATE-2 becomes the most urgent gate rather than GATE-1.
   That reorders a lot, so it deserves scrutiny.

---

## 12. WHAT I DID NOT DO, AND WHY

- **Did not fix anything.** Read-only packet; the fix is consensus-affecting and GATE-2 gated.
- **Did not commit the probe.** Written, run, deleted; `git status` clean, never in the index.
- **Did not read all seven `is_valid()` implementations** — Transfer only, in full. See §11.2.
- **Did not test against a running node or live chain.** Static trace plus a state-layer probe.
  The consensus-level claim rests on reading `validate_block_with_options` end to end.
- **Did not check whether existing chain history contains forged transactions** — needs chain access
  (§8, §11.3).
- **Did not re-file DARQ-021's root cause** as RC-19, though I think RC-02 is now the wrong home
  (§10). Registry schema change belongs in a docs packet.
- **Did not touch DARQ-006's fail-open `RPC_REQUIRE_AUTH` default**, though I found it while tracing
  reachability. It is P-006's, and it does not change DARQ-021's severity — Route B is ungated
  regardless.
