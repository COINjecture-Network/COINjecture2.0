# Consensus calculations

This section presents the core COINjecture dynamical and economic formulas in the
style of [Section 11 (Calculations)](https://bitcoin.org/bitcoin.pdf) in the Bitcoin
whitepaper: explicit assumptions, step-by-step algebra, closed-form results, and a
plain interpretation of what each quantity means for the running network.

Formal proofs of the integer paths live in [`lean4/`](../lean4/). See
[`FORMAL_VERIFICATION.md`](FORMAL_VERIFICATION.md) for the Lean ↔ Rust crosswalk.

---

## 1. Setup

We model a decentralized network of competing miners whose collective behavior is
shaped by measurable quantities:

- **Block solve time** — empirical wall-clock time to produce a valid PoUW block.
- **Verify time** — network-checkable time to validate the solution (polynomial).
- **Difficulty target** — self-referential optimal block interval derived from the
  network's own median solve history.
- **Work score** — dimensionless security bits accumulated per block.

The conjecture is **not** that these quantities are fixed constants at genesis.
It is that a network built from two symmetry axioms (below) and that measures
itself continuously will **empirically converge** toward a predicted equilibrium.
That prediction is **falsifiable** by running the network and comparing measurements
to the formulas in Sections 5–7.

---

## 2. Design axioms (Appendix D)

We impose only two constraints on the consensus eigenvalue μ = x + iy.

**Axiom 1 — Unit normalization (unitarity):**

```text
|μ|² = x² + y² = 1
```

**Axiom 2 — Symmetry:**

```text
|x| = |y|
```

From symmetry, x² = y². Substituting into the unit constraint:

```text
2x² = 1   ⇒   |x| = |y| = 1/√2
```

Define the **Satoshi constant** η = |Re(μ)| = Im(μ) = 1/√2 ≈ 0.707107.

The unique balanced point on the unit circle with negative real part and positive
imaginary part is the **critical eigenvalue**:

```text
μ = exp(i · 3π/4) = (−1 + i) / √2
```

**8-cycle closure.** Each step advances phase by 3π/4 radians:

```text
8 × (3π/4) = 6π = 3 × 2π
```

The orbit closes after 8 steps having completed exactly 3 full rotations. The gear
ratio 3:8 is coprime (gcd(3, 8) = 1), so the orbit is **primitive** — it visits
all 8 positions before returning. Unitary steps preserve modulus: |e^{iθ} · β| = |β|.

**Interpretation.** η = 1/√2 is not a hard-coded protocol constant imposed on miners.
It is a **hypothesis about the fixed point** of the coupled dynamics (solve times,
difficulty oscillation, work-score ratios). The network tests whether empirical
measurements converge toward η within tolerance (Section 7).

| Quantity | Symbol | Value | Code |
|----------|--------|-------|------|
| Satoshi constant | η = λ | 1/√2 | `core/src/dimensional.rs::ETA` |
| Consensus time unit | τ_c | √2 = 1/η | `TAU_C` |
| Phase step | — | 3π/4 | `DesignAxioms.lean::eigenPhaseStep` |
| Critical μ | — | (−1+i)/√2 | `DesignAxioms.lean::criticalMu` |

---

## 3. Work score (security bits)

Given solve time t_s and verify time t_v (with floor ε = 1 µs on t_v), and quality
q ∈ [0, 1] (consensus: basis points 0–10 000):

**Ideal form:**

```text
work_score = log₂(t_s / t_v) × q
```

**Asymmetry gate.** If t_s < 2 · max(t_v, ε), the score is 0. This mirrors the NP
definition: meaningful work requires solve time at least twice verify time.

**Deterministic consensus form.** Ratios use fixed-point arithmetic with scale
10⁶ (mirrors Bitcoin's insistence on deterministic validation paths):

```text
ratio_fp = (t_s ≪ 32) / max(t_v, ε)
floor_k  = ⌊log₂(ratio_fp)⌋ − 32
bits_fp  = floor_k × 10⁶ + fractional_mantissa
work     = ⌊ bits_fp × q_bps / 10 000 ⌋
```

On-chain, the header stores the integer floor of work_score bits; cumulative chain
security is W = Σ w_trunc.

**Example (Tier C fixture).** t_s = 10 µs, t_v = 1 µs, q = 100%:

```text
log₂(10/1) ≈ 3.25 bits  →  w_trunc = 3  (fixed-point score 3_250_000)
```

**Interpretation.** A block with work_score = 40 means the winning miner demonstrated
roughly 2⁴⁰ times more search effort than verification requires. An alternative chain
must match cumulative W. Racing miners publish the **minimum competitive** solve time,
not an inflated self-report.

| Path | Reference |
|------|-----------|
| Ideal log₂ | `consensus/src/work_score.rs` |
| Fixed-point | `core/src/fixed_point.rs::log2_ratio` |
| Lean spec | `lean4/Coinjecture/WorkScore.lean` |

---

## 4. Block reward (tokenomic allocation)

Rewards follow the whitepaper's dimensionless w/W shape. Let w_trunc be this block's
integer work bits and W_parent the sum through the parent block.

**Mint formula:**

```text
mint_atoms = ⌊ w_trunc · S · K / W_parent ⌋
```

where S = 10¹² atoms per display BEANS and K = 50 is the emission multiplier.

**First harvest.** When W_parent = w_trunc ≥ 1:

```text
mint_atoms = ⌊ S · K · w / w ⌋ = S · K
```

→ exactly K display BEANS worth of atoms on the first block with w_trunc ≥ 1.

**Safety.** W_parent = 0 ⇒ mint = 0.

**Example (Tier C fixture).** w_trunc = 16, W_parent = 521:

```text
mint_atoms = ⌊ 16 × 10¹² × 50 / 521 ⌋
```

**Interpretation.** Like Bitcoin's subsidy schedule, absolute issuance is bounded by
consensus constants (S, K). Unlike a fixed subsidy, **share** is proportional to
measured work relative to total chain work — high-work blocks earn more when the
chain is young; as W grows, per-block mint decreases for equal work.

| Constant | Value | Reference |
|----------|-------|-----------|
| S | 10¹² | `tokenomics/src/rewards.rs::REWARD_FIXED_POINT_SCALE` |
| K | 50 | `REWARD_EMISSION_MULTIPLIER` |
| Lean | `mintAtoms` | `lean4/Coinjecture/Rewards.lean` |

---

## 5. Coherence and perturbation decay

Define the dimensionless timing ratio r = t_s / t_target, where t_s is empirical
solve time and t_target is the difficulty adjuster's self-referential median target.

**Coherence function:**

```text
C(r) = 2r / (1 + r²)
```

**Equilibrium.** C(1) = 1. At r = 1, solve time matches target — the system is coherent.

**Symmetry.** C(r) = C(1/r). Work-score log-ratios should be symmetric about log r = 0.

**Perturbation recovery (prediction).** After a deviation from equilibrium, coherence
should decay along a **sech profile**, not exponential decay:

```text
C(rⁿ) = sech(n · log r)     where     sech(x) = 1 / cosh(x)
```

At equilibrium (r = 1), log r = 0 and sech(0) = 1.

**Check at r = 2:**

```text
C(2) = 4/5 = 0.8
sech(log 2) ≈ 0.8
```

After two steps, C(2²) = C(4) = 8/17 ≈ 0.471 and sech(2 log 2) ≈ 0.471.

| Lean | `Coinjecture/Coherence.lean` |
| On-chain ratio | solve_secs / target_secs in pool metrics |

---

## 6. Dimensional pools (Appendix E)

Eight economic dimensions D1–D8 sample an exponential decay curve at fixed τ values.
Exchange rates between pools follow the decay structure, not a market order book.

**Decay law:**

```text
D(τ) = e^(−η · τ) = e^(−τ / √2)
```

**Inverse (design):** given target scale D_n, choose τ_n = −√2 · ln(D_n).

**Table 1 — τ samples and target scales:**

| Pool | τ_n | D(τ_n) | Design anchor |
|------|-----|--------|---------------|
| D1 | 0.00 | 1.000 | Genesis |
| D2 | 0.20 | 0.867 | Coupling |
| D3 | 0.41 | 0.750 | First harmonic |
| D4 | 0.68 | 0.618 | φ⁻¹ (golden ratio inverse) |
| D5 | 0.98 | 0.500 | 2⁻¹ |
| D6 | 1.36 | 0.382 | φ⁻² |
| D7 | 1.96 | 0.250 | 2⁻² |
| D8 | 2.72 | 0.146 | e⁻ᵉ/√² |

The constants φ⁻¹, 2⁻¹, etc. appear **by construction** in the τ table. The conjecture
claims the **network's runtime behavior** (block times, difficulty oscillation, work
distributions) converges toward the equilibrium implied by η — not that φ magically
emerges from mining alone.

The fixed ratio ln(φ) / ln(2) ≈ −0.694 governs relationships between golden and
binary scale anchors.

| Code | `core/src/dimensional.rs::TAU_POINTS`, `DimensionalScales` |
| Lean | `Coinjecture/DimensionalPools.lean` |
| State | `state/src/dimensional_pools.rs` |

---

## 7. Falsifiability

Running the network produces three testable predictions. **Failure of any one
constitutes falsification** of the dynamical conjecture.

**P1 — Sech recovery.** Perturbation recovery follows sech(n · log r), not exponential
decay. Compare measured C(rⁿ) against sech(n · log r) within tolerance.

**P2 — Eight-cycle period.** Difficulty oscillation period should relate to the
8-block primitive cycle (gear ratio 3:8) as the system converges to lowest-overhead
dynamics. Tolerance: |measured_period − 8| ≤ tol blocks.

**P3 — Log symmetry.** Work-score log-ratios symmetric about 0: for each log r in
the empirical sample, −log r appears with comparable weight (reflecting C(r) = C(1/r)).

**η convergence (auxiliary).** Measured η from network metrics within 5% of 1/√2:

```text
|η_measured − 1/√2| < 0.05
```

On-chain: `DimensionalPoolState::test_conjecture()` in
[`state/src/dimensional_pools.rs`](../state/src/dimensional_pools.rs).

| Lean predicates | `Coinjecture/Falsifiability.lean` |
| Composite pass | `conjectureSupported` (P1 ∧ P2 ∧ P3) |

---

## 8. Verification pipeline

**Local gate (Tier C — Rust ↔ Lean bit-exact fixtures):**

```bash
./scripts/verify-formal-fixtures.sh
```

**Lean only:**

```bash
cd lean4 && lake build
```

**CI:** [`.github/workflows/lean4.yml`](../.github/workflows/lean4.yml) runs the full
fixture script on changes to `lean4/`, consensus work-score/reward paths, and the
verification script itself.

---

## References

- [Bitcoin whitepaper — Section 11, Calculations](https://bitcoin.org/bitcoin.pdf)
- [`FORMAL_VERIFICATION.md`](FORMAL_VERIFICATION.md) — Appendix D/E Lean crosswalk
- [`proofs/README.md`](../proofs/README.md) — Eigenverse formalization (extended proofs)
