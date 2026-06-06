# Security calculations

This section validates COINjecture security assumptions in the style of
[Bitcoin whitepaper — Section 11 (Calculations)](https://bitcoin.org/bitcoin.pdf):
explicit threat model, symbolic algebra, numeric examples, and a plain interpretation
of what each result means for operators.

Economic and dynamical formulas live in [`CONSENSUS_CALCULATIONS.md`](CONSENSUS_CALCULATIONS.md).
Formal structures and Tier A lemmas live in
[`lean4/Coinjecture/SecurityModel.lean`](../lean4/Coinjecture/SecurityModel.lean) and
[`lean4/Coinjecture/SecurityCalculations.lean`](../lean4/Coinjecture/SecurityCalculations.lean).

---

## 1. Threat model

We model a decentralized network of **honest full nodes** and an **adversary** who
controls a fraction of total **work-score production capacity** (not hash power).

**Honest nodes** (see `HonestNodePolicy` in Lean) are assumed to:

1. **Verify before accept** — reject blocks whose PoUW witness fails the deterministic checker.
2. **Score deterministically** — use the fixed-point work path (`workScoreFixed`), not `f64`.
3. **Follow heaviest tip** — prefer the chain with greatest cumulative truncated work `W`.

**Adversary** (see `AdversaryCapabilities` in Lean) may:

- Broadcast invalid blocks (honest nodes reject them).
- Inflate self-reported solve time on **losing** blocks (winners set competitive time).
- **Cannot** forge a passing verify without a valid witness (checker soundness — Tier B).

**Not modeled in Tier A** (deployment / network assumptions):

- Eclipse attacks, partition attacks, BGP hijacks.
- Long-range attacks without checkpointing.
- Production ZK privacy (testnet uses a placeholder MAC — see `core/src/privacy.rs`).

---

## 2. Mapping Bitcoin §11 to PoUW

| Bitcoin (hash PoW) | COINjecture (PoUW) |
|--------------------|--------------------|
| Hash power fraction `q` (attacker) | Work-score rate fraction `q` |
| Honest fraction `p = 1 − q` | `p = 1 − q` (same notation) |
| Blocks behind `z` | Blocks behind on the public tip |
| Chain length (block count) | Cumulative security `W = Σ w_trunc` |
| Proof-of-work difficulty | Asymmetry gate + `log₂(t_s/t_v) × q` |
| Longest chain rule | **Heaviest** chain rule (`W` maximal) |

**Key difference.** Bitcoin’s work is opaque hash grinding. COINjecture’s work is
**interpretable security bits**: a header score of `N` means the winner demonstrated
roughly `2^N` times more effort than verification requires (ideal analysis; on-chain
uses fixed-point truncation).

---

## 3. Per-block security bits

Given solve time `t_s`, verify time `t_v` (floor ε = 1 µs), quality `q ∈ [0, 1]`:

**Ideal form:**

```text
work_score = log₂(t_s / t_v) × q
```

**Asymmetry gate (consensus).** If `t_s < 2 · max(t_v, ε)`, then `work_score = 0`.
This encodes the NP asymmetry requirement: meaningful work needs solve at least twice verify.

**Tier A (proved in Lean).**

```text
¬asymmetryOkUs(t_s, t_v)  ⇒  workScoreFixed(...) = 0
quality = 0               ⇒  workScoreFixed(...) = 0
```

**Racing incentive (economic, not a theorem).** Miners who slow their solver lose blocks
to faster competitors; the published solve time on the winning block is the **minimum
competitive** time, not an arbitrary self-report.

| Reference | Location |
|-----------|----------|
| Ideal formula | `consensus/src/work_score.rs` (module docs) |
| Fixed-point path | `core/src/fixed_point.rs` |
| Lean | `Coinjecture/WorkScore.lean`, `SecurityModel.zero_work_when_no_asymmetry` |

---

## 4. Cumulative chain security

Each accepted block contributes integer truncated work `w_trunc` to the header.
Cumulative security through height `n`:

```text
W_n = Σᵢ₌₁ⁿ w_trunc(i)
```

**Fork choice.** Honest policy selects a tip with maximal `W` among valid chains
(tie-breaking is implementation-defined).

**Reorganization requirement.** To replace the honest tip, an adversary must produce
a **valid** alternate chain whose cumulative `W` exceeds the honest chain’s `W` from
the common ancestor forward.

**Tier A (proved in Lean).**

```text
chainWork (w :: rest) = w + chainWork rest
heavierChain wA wB  ⇔  wB < wA
```

---

## 5. Catch-up probability (adapted from Satoshi)

Satoshi models the honest chain and the attacker as independent **Poisson processes**
whose event rates are proportional to `p` and `q`. The attacker starts `z` blocks
behind. When `p > q`, the probability of ever catching up drops **exponentially** in `z`.

We adopt the same **process model** with these substitutions:

- Event rate for the honest chain ∝ `p` (honest work-score production).
- Event rate for the attacker ∝ `q`.
- “Length” advantage of `z` blocks is the initial deficit.

**Simplified bound (Satoshi’s closed form, `q < p`).**

```text
P(attacker succeeds) ≈ (q / p)^z
```

**Interpretation.** Each additional confirmation block multiplies failure probability
by roughly `q/p`. At `q = 0.1`, `p = 0.9`, ten blocks yield `(1/9)^10 ≈ 2.6 × 10⁻¹⁰`.

**Work-weighted refinement.** If the honest chain’s advantage is measured in **bits**
rather than block count, use an effective deficit `z_eff` such that the attacker must
overtake `ΔW ≈ z · w̄` average truncated work per block:

```text
P ≈ (q / p)^(ΔW / w̄) = (q / p)^z     when w̄ is stable across blocks
```

When block work varies (difficulty / problem type changes), use the **sum of per-block
work deficits** instead of raw block count — the heaviest-chain rule counts bits, not
just height.

**Algebraic core (Tier A, Lean).** For positive naturals with `q < p` and `z > 0`:

```text
q^z < p^z
```

This is the discrete backbone behind “exponential security in confirmations.”
See `SecurityCalculations.catch_up_power_lt` in Lean.

---

## 6. Numeric examples (confirmation depth)

Assume `p > q` and approximate `P ≈ (q/p)^z`. All values are **upper-bound intuition**;
live networks should add margin for latency, eclipse risk, and work-score variance.

| Attacker share `q` | Honest `p` | `q/p` | `z = 5` | `z = 10` | `z = 20` |
|--------------------|------------|-------|---------|----------|----------|
| 10% | 90% | 1/9 | 1.7×10⁻⁵ | 2.9×10⁻¹⁰ | 8.5×10⁻²⁰ |
| 20% | 80% | 1/4 | 1.0×10⁻³ | 1.0×10⁻⁶ | 1.1×10⁻¹² |
| 30% | 70% | 3/7 | 2.2×10⁻² | 4.9×10⁻⁴ | 2.4×10⁻⁷ |
| 40% | 60% | 2/3 | 1.3×10⁻¹ | 1.7×10⁻² | 2.9×10⁻⁴ |
| 49% | 51% | 49/51 | 3.4×10⁻¹ | 1.2×10⁻¹ | 1.4×10⁻² |

**Reading the table.** At 10% attacker share, **five** confirmations drive success
probability below `2×10⁻⁵`. At 49% share, even twenty confirmations leave a **non-negligible**
reorg risk — the network does **not** claim safety without an honest majority of work.

**Lean spot-checks** (exact integer inequalities `q^z < p^z`):

| Case | Statement | Lean theorem |
|------|-----------|--------------|
| 10%, 5 blocks | `1⁵ < 9⁵` | `catchup_one_ninth_pow_five` |
| 30%, 10 blocks | `3¹⁰ < 7¹⁰` | `catchup_three_seventh_pow_ten` |
| 49%, 20 blocks | `49²⁰ < 51²⁰` | `catchup_fortynine_fiftyone_pow_twenty` |

---

## 7. What PoUW asymmetry adds beyond Bitcoin’s model

| Mechanism | Security role |
|-----------|----------------|
| **NP verify** | Invalid witnesses rejected in polynomial time — `forge_verify_without_witness = false`. |
| **Asymmetry gate** | Blocks with `t_s < 2·t_v` contribute **zero** on-chain work (Lean proved). |
| **Commitment salt** | Problem tied to parent hash — pre-mining across forks requires recomputation. |
| **Racing** | Inflated solve times lose to faster honest miners on competitive blocks. |
| **Quality bps** | Suboptimal solutions scale work down (`applyQuality` monotone in Lean). |

**TSP caveat (operational).** Mining uses heuristics; verification checks tour **feasibility**
only. PoUW asymmetry for TSP is **weaker** than for SubsetSum / SAT until stronger
solvers or stricter verification are deployed.

**TSP quality (consensus).** Work score uses `quality × log₂(t_s/t_v)`. Quality is
`baseline_length / tour_length` (nearest-neighbor baseline), clamped to `[0, 1]` — not
`1/(length+1)`, which drove `w_trunc = 0` and zero BEANS on small instances.

---

## 8. Economic attacks (rewards — w/√W, v4)

Minting uses parent cumulative work `W_parent` and integer **`isqrt(W_parent)`**:

```text
mint_atoms = ⌊ w_trunc · S · K / isqrt(W_parent) ⌋
```

**Tier A (proved in Lean — `Coinjecture/Rewards.lean`, `SecurityModel.lean`).**

| Property | Statement |
|----------|-----------|
| Parent work required | `W_parent = 0 ⇒ mint = 0` |
| Block work required | `w_trunc = 0 ⇒ mint = 0` |
| Monotonic in work | `w₁ ≤ w₂ ⇒ mint(w₁) ≤ mint(w₂)` at fixed `W` |
| Floor bound | `mint · isqrt(W) ≤ w · S · K` |
| First harvest (Tier C) | `w=1`, `W=1` ⇒ `K` display BEANS (`50` with `K=50`) |

**Compared to v3 (`w/W`).** Tail decay is **`O(1/√W)`** per block rather than **`O(1/W)`** —
higher long-run issuance at the same `K`, by design on v4 (target ~1 BEANS/block average to
height 100,000). Per-block mint still **decreases** as `W` grows (denominator `isqrt(W)` increases).

**Interpretation (Tier B axioms in `ClassicalAxioms.lean`).**

- **Emission ≠ fork choice** — minted atoms are not summed into cumulative `W`.
- **Private-chain inflation** — an attacker cannot move honest balances by minting on a losing fork;
  alternate coinbase only matters if that chain becomes the **heaviest valid tip** (Sections 4–5).
- **Solve-time gaming** — more work still yields weakly more mint at fixed `W` (Lean monotonicity);
  racing still punishes inflated solve times on competitive blocks.

**v4 parameters (`coinject-network-b-v4`).** `K = 50`, `S = 10¹²`, header difficulty **5**,
NP bootstrap size **110**. Block-2 first harvest with genesis `w=10`, `W_parent=10`:
`⌊10·S·K / isqrt(10)⌋ ≈ 167` display BEANS.

---

## 9. Assumption validation matrix

| # | Assumption | Tier | How validated |
|---|------------|------|----------------|
| A1 | Honest majority of work-score rate (`p > q`) | Operational | Node distribution, monitoring |
| A2 | Verify-before-accept | A / code | `HonestNodePolicy`, `core/src/problem.rs` |
| A3 | Checker soundness | B | NP axioms + deterministic verify functions |
| A4 | Deterministic scoring | A | `workScoreFixed`, Tier C fixtures |
| A5 | Heaviest tip | A / code | `heavierChain`, fork-choice in node |
| A6 | Catch-up probability ≪ 1 at depth `z` | A (algebra) + B (Poisson) | Section 5–6, `SecurityCalculations.lean` |
| A7 | Production ZK for privacy market | **Not yet** | Placeholder axioms only — testnet |
| A8 | w/√W emission (`isqrt(W)` denominator) | A | `Rewards.mintAtoms`, `SecurityModel.mint_floor_bound` |
| A9 | Emission decoupled from fork-choice `W` | B | `ClassicalAxioms.emission_separate_from_fork_choice` |

---

## 10. Operator checklist (testnet / mainnet prep)

1. **Confirmations** — Choose `z` from Section 6 for your tolerated `q` (use pessimistic `q`).
2. **Monitor** `W` at tip vs competing tips — not block height alone.
3. **Checker parity** — Run `./scripts/verify-formal-fixtures.sh` on release candidates.
4. **TSP exposure** — Treat TSP-heavy periods as lower per-block asymmetry until solvers harden.
5. **ZK** — Do not rely on placeholder MAC proofs for production privacy claims.

---

## 11. Verification pipeline

```bash
# Tier C — Rust ↔ Lean economic/security fixtures
./scripts/verify-formal-fixtures.sh

# Lean — security + work-score lemmas
cd lean4 && lake build Coinjecture.SecurityCalculations
```

**CI:** [`.github/workflows/lean4.yml`](../.github/workflows/lean4.yml)

---

## References

- [Bitcoin whitepaper — Section 11, Calculations](https://bitcoin.org/bitcoin.pdf)
- [`CONSENSUS_CALCULATIONS.md`](CONSENSUS_CALCULATIONS.md) — η, work score, rewards, falsifiability
- [`FORMAL_VERIFICATION.md`](FORMAL_VERIFICATION.md) — Lean crosswalk
- [`SECURITY.md`](../SECURITY.md) — disclosure policy
