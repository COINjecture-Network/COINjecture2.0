# Cycle 1 — P-003-V BUILDER report (C3 runtime verification)

Packet: **P-003-V** · Phase A, read-only · Base `main` @ `28c50a12` · 2026-08-03
No repo changes. Temporary probe written, run, and deleted.

---

## Verdict: **CONFIRMED** — nothing reconciles the derivations at runtime

But the method in §11 could not be run as written, and what replaced it found something worse.

---

## 1. Why the stated method was not runnable

The §11 hypothesis assumes *"Genesis credits a balance to address A = SHA-256(pubkey)."* **It does
not.** Two blockers, both static and both dispositive:

1. **`node/src/genesis.rs:47` — `initial_supply: 0`**, commented *"Zero initial supply — tokens
   created through mining rewards only."* The genesis coinbase issues **zero**. There is no
   genesis-allocated balance to spend, so "spend from a genesis address" has nothing to spend.
2. **`genesis_wallet.json` does not exist in the repo.** `genesis.rs:19` refers to it
   (*"Genesis address from genesis_wallet.json"*) but no such file is present. Only the **public**
   key constant is in-tree (`df52ac77…`). Without the private key, no spend can be signed from that
   address regardless.

So the genesis-spend walk is not merely inconvenient — it tests a state that does not exist at this
HEAD. Reporting that is more useful than forcing it.

## 2. What I ran instead

The underlying question is *"does anything reconcile the three derivations at runtime?"* That is
answerable directly, without a node, a droplet, or a genesis balance. Temporary probe at
`core/tests/zz_p003v_temp_probe.rs` — generate one keypair, derive all three addresses, then push
transactions through the real `core` API. **Deleted after the run; nothing committed.**

```
PROBE raw(consensus tx path) = CCUmVjpS39BMMg79VfjBtxi6NtUzEMABnrxaGerx4UFr
PROBE sha256(wallet+genesis) = 2yukxQiDNQFdZfEV5p4yst5PPKXKqqgaLrS9jUvSc6XG
PROBE blake3(validator ks)   = 3zg2pS5SNJkvpYM5CGb9Frp5YBgWYr8wC3rUK7CVGJeT
PROBE distinct? raw!=sha true | raw!=blake3 true | sha!=blake3 true

PROBE from=SHA256  verify_signature=true  is_valid=false
PROBE from=raw     verify_signature=true  is_valid=true
PROBE from=VICTIM  verify_signature=true  is_valid=false
```

**One key, three different addresses, no translation layer anywhere in the path.** A transfer whose
`from` is the address a wallet displays is rejected by `is_valid()` while the identical transfer
using the raw-pubkey address is accepted. C3 is confirmed at runtime, not just statically.

**Which derivation each side used:** the consensus transaction path uses **raw** — the binding check
is `self.from != self.public_key.to_address()` (`core/src/transaction.rs:438` and six siblings), and
`to_address()` is `Address::from_pubkey`, the identity function. Wallet and genesis use **SHA-256**.
The validator keystore and escrows use **BLAKE3**.

**Is the bug silent or visible?** Not established — that needs a running node with state, which the
zero-supply genesis prevents. What *is* established is that the rejection is total and silent at the
API level: `verify_signature()` returns `true` for a transaction that `is_valid()` rejects, so any
caller checking only the former sees success.

---

## 3. 🚨 DARQ-NEW-5 — the binding check is dead code on the block path

This is not what I was looking for and it is more serious than C3.

`Transaction::verify_signature()` checks **only the Ed25519 signature**. The
`from == public_key.to_address()` binding lives exclusively in `Transaction::is_valid()`
(`core/src/transaction.rs:437-439`). The probe's third line proves the consequence:

> **`from = VICTIM` (an address the signing key does not own) → `verify_signature = true`.**

Now the call sites:

| Site | Calls | Binding checked? |
|---|---|---|
| `node/src/validator.rs:169` — block validation, step 8 | `tx.verify_signature()` | ❌ **no** |
| `mempool/src/pool.rs:160` — mempool admission | `tx.verify_signature()` | ❌ **no** |
| `core/src/block.rs:215` — `Block::verify()` | `tx.is_valid()` | ✅ yes |

**`Block::verify()` is never called anywhere in `node/src`.** Every ingest path —
`service/block_processing.rs:93,113`, `service/fork.rs:1185,1371`, `service/mod.rs:867,1335` —
goes through `validator.validate_block_with_options(...)`, which performs the weaker check.

So `is_valid()` — and with it the sender-binding check, and the transfer amount/fee overflow
guard it also gates (`validate_transfer_fields`) — appears to be **dead on the block path**. A block
carrying a transaction that names an arbitrary victim as `from`, signed by the attacker's own key,
passes step 8.

**This is adjacent to M3 but distinct.** M3 says *callers bypass the validator*. This says the
**validator itself** performs the weaker of two checks that both exist in the codebase. Fixing M3
as written would not fix this.

**Not fully traced, deliberately.** I did not follow the apply path to confirm the debit actually
lands on the attacker-named `from`, and I did not check whether some later stage re-checks. Per the
guardrails I logged it rather than hunting. **It needs its own verification packet before anyone
sizes it** — but if it holds, it belongs above C3 in the queue, and it materially changes P-005's
scope (RC-05, ledger apply path).

---

## 4. Is there ANY reconciliation layer?

**No.** Searched the full conversion surface: `Address::from_pubkey`, `to_address()`,
`derive_address`, `address_from_pubkey`, and every `from_pubkey(` call site. Every one is a direct
single-algorithm derivation. There is no lookup that tries multiple derivations, no translation
table, no compatibility shim, and no alias map in `state/`. The probe confirms it empirically for
the raw↔SHA-256 pair.

---

## 5. What I did NOT do, and why

- **Did not start a node.** The zero-supply genesis makes the stated walk untestable, and the probe
  answers the actual question more directly and more cheaply.
- **Did not commit the probe.** Written, run, deleted; `git status` is clean.
- **Did not attempt to fix C3.** That is P-003 and it is gated [GATE-1].
- **Did not trace DARQ-NEW-5 to the apply path or assign it a severity.** Logged, not hunted — it
  needs its own packet.
- **Did not establish whether an unspendable balance is visible in state queries.** Blocked by the
  same zero-supply genesis; needs a mined-block scenario instead.
- **Did not touch the droplet or any deployed network.**

---

## 6. What this changes for GATE-1

The builder half of GATE-1 is now answered: **the split is real at runtime and nothing reconciles
it.** The remaining half — *is there deployed state with balances anyone cares about?* — is still
Sarah's and still needs droplet access.

One input for that decision: because genesis issues zero supply, **all** existing balances on any
live chain came from mining rewards, credited to `header.miner`. Whether those are spendable depends
on which derivation produced the miner address in the running node's config — which is worth
checking before choosing reset vs. migration, since it may mean the live chain's balances are
already unspendable and a reset costs less than it appears to.

---

**STATE: CYCLE 1, PHASE A complete for P-003-V. Builder stops here.**
