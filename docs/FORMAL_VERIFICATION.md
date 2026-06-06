# Formal verification (Lean 4)

The [`lean4/`](../lean4/) package formalizes PoUW assumptions, on-chain economics,
and the COINjecture dynamical hypothesis. For Satoshi-style derivations of the same
formulas (assumptions → algebra → interpretation), see
[`CONSENSUS_CALCULATIONS.md`](CONSENSUS_CALCULATIONS.md). For security assumptions and
confirmation-depth bounds (Bitcoin §11 style), see
[`SECURITY_CALCULATIONS.md`](SECURITY_CALCULATIONS.md).

## Tier A (proved in Lean)

| Artifact | Content |
|----------|---------|
| `Coinjecture/Verify.lean` | `verifySubsetSum`, `verifySat`, `verifyTspFeasible` — mirrors [`core/src/problem.rs`](../core/src/problem.rs) |
| `Coinjecture/GeneratorInvariants.lean` | Structures for miner-generated solvable instances |
| `Coinjecture/TspFeasibility.lean` | Documents on-chain TSP as **Hamiltonian feasibility**, not decision-TSP |
| `Coinjecture/Rewards.lean` | `mintAtoms`, `mintBeans`, first-harvest and floor lemmas — [`tokenomics/src/rewards.rs`](../tokenomics/src/rewards.rs) |
| `Coinjecture/WorkScore.lean` | `workScoreFixed`, `log2Ratio`, `applyQuality` — [`consensus/src/work_score.rs`](../consensus/src/work_score.rs), [`core/src/fixed_point.rs`](../core/src/fixed_point.rs) |
| `Coinjecture/DesignAxioms.lean` | η = 1/√2, μ = (−1+i)/√2, 8-cycle closure (Appendix D) |
| `Coinjecture/ComplexDecomposition.lean` | Euclidean `(euclRadial, euclAngular)` + Lorentzian `(lorRadial, lorRapidity)`; anchors on light cone |
| `Coinjecture/Coherence.lean` | C(r) = 2r/(1+r²), sech perturbation samples, symmetry |
| `Coinjecture/DimensionalPools.lean` | D(τ) = e^(−ητ), eight pool τₙ samples (Appendix E) |
| `Coinjecture/Falsifiability.lean` | Three testable predictions + `test_conjecture`-style η tolerance |
| `Coinjecture/Fixtures.lean` | Tier C vectors aligned with Rust tests |
| `Coinjecture/SecurityModel.lean` | PoUW work bits, heaviest-chain rule, reward bounds, trust/adversary structure |
| `Coinjecture/SecurityCalculations.lean` | Catch-up exponent `q^z < p^z`, chain-work monotonicity, numeric spot-checks |

## Appendix D/E crosswalk (whitepaper → Lean → Rust)

| Whitepaper section | Claim | Lean module | Rust / on-chain |
|--------------------|-------|-------------|-----------------|
| **Appendix D — Design axioms** | \|μ\|² = 1, \|x\| = \|y\| ⇒ η = 1/√2 | `DesignAxioms.lean` (`unit_circle_eta`, `symmetry_equal_magnitudes`) | `core/src/dimensional.rs::ETA`, `LAMBDA` |
| | μ = exp(i·3π/4) = (−1+i)/√2 | `criticalMu`, `mu_on_unit_circle`; `ComplexDecomposition.mu`, `mu_unique` | `TAU_C`, phase dynamics in dimensional framework |
| | 8-step closure, gcd(3,8)=1 | `eight_cycle_closes_*`, `gear_ratio_coprime` | 8-block cycle prediction in falsifiability |
| | \|e^{iθ}β\| = \|β\| | `unitary_modulus_preserved` | — (formal invariant) |
| **Coherence** | C(r) = 2r/(1+r²), C(1)=1 | `Coherence.lean` (`coherence_at_one`) | solve/target ratio in pool metrics |
| | C(r) = C(1/r) | `coherence_symmetric_*` | work-score log-ratio symmetry (P3) |
| | C(rⁿ) = sech(n·log r) | `perturbation_sech_sample_*` | P1 sech recovery test |
| **Appendix E — Dimensional pools** | D(τ) = e^(−ητ) | `DimensionalPools.lean` (`poolScale`) | `DimensionalScales::scale_at_tau` |
| | τₙ table (D1–D8) | `tauPoints` | `TAU_POINTS` |
| | τ = −√2·ln(D) | `tauFromScale` | inverse design in dimensional docs |
| | φ⁻¹, 2⁻¹, φ⁻², 2⁻² anchors | `pool_d4_near_golden`, `pool_d5_half`, … | `DimensionalScales::calculate` |
| **Falsifiability** | P1 sech recovery | `prediction1_sech_recovery` | empirical metrics (future dashboard) |
| | P2 ~8-block oscillation | `prediction2_eight_cycle_period` | difficulty adjuster history |
| | P3 log symmetry | `prediction3_log_symmetry` | work-score distribution |
| | \|η_measured − η\| < 5% | `etaWithinTolerance` | `DimensionalPoolState::test_conjecture` |
| **Economics** | mint = ⌊w·S·K/isqrt(W)⌋ | `Rewards.lean` (`mintAtoms`, `isqrtDenom`) | `tokenomics/src/rewards.rs` |
| | work = log₂(t_s/t_v)·q | `WorkScore.lean` | `consensus/src/work_score.rs` |

## Tier C (Rust ↔ Lean fixtures)

```bash
./scripts/verify-formal-fixtures.sh
```

Runs `lake build` and `cargo test -p coinject-consensus lean_fixture`. Vectors live in
[`lean4/Coinjecture/Fixtures.lean`](../lean4/Coinjecture/Fixtures.lean) and
[`consensus/tests/lean_fixture_alignment.rs`](../consensus/tests/lean_fixture_alignment.rs).

| Fixture | Lean | Rust constant |
|---------|------|---------------|
| log₂(10/1) × 10⁶ | `log2TenToOne` | `LOG2_TEN_TO_ONE = 3_250_000` |
| log₂(4/1) × 10⁶ | `log2FourToOne` | `2 × SCALE` |
| work score (10 µs, 1 µs) | `workScoreTenOneUs` | deterministic path test |
| mint(16, 521) | `mintAtoms16Over521` | reward regression (`isqrt(521)=22`) |
| first harvest | `first_harvest` | `w=1`, `W=1` ⇒ `K` BEANS |

## Security model (`Coinjecture/SecurityModel.lean`)

Satoshi-style derivations and confirmation tables:
[`SECURITY_CALCULATIONS.md`](SECURITY_CALCULATIONS.md).

| Layer | Content |
|-------|---------|
| **Trust** | `HonestNodePolicy` — verify-before-accept, deterministic work score, heaviest tip |
| **Adversary** | `AdversaryCapabilities` — invalid broadcast allowed; forge-verify disallowed (checker soundness) |
| **PoUW (Tier A)** | Zero work without asymmetry; `chainWork` append; `applyQuality` monotone |
| **Fork choice** | `heavierChain` / `prefersTip` on cumulative truncated work `W` |
| **Rewards (Tier A)** | `mint_requires_parent_work`, `mint_floor_bound`, `mint_mono_in_work` — w/√W lemmas |
| **Tier B** | NP checkers, ideal `log₂` bits, emission/fork decoupling axioms, placeholder ZK (testnet MAC in `privacy.rs`) |

## Tier B (axiomatized)

| Axiom block | Reference |
|-------------|-----------|
| `Coinjecture/ClassicalAxioms.lean` | SUBSET-SUM, 3-SAT NP-completeness; decision-TSP NP-hard; ideal `log₂` work-score interpretation; w/√W emission economics (fork decoupling, losing-fork inertness) |

## Known gaps

1. **TSP mining** uses greedy NN + 2-opt ([`consensus/src/miner.rs`](../consensus/src/miner.rs)); verification only checks tour validity. PoUW asymmetry for TSP is weaker than SAT/SubsetSum.
2. **`ProblemDescriptor.scaling_exponent`** values are empirical seeds, not Lean-proved.
3. **P1–P3 falsifiability** predicates are specified in Lean and documented in [`CONSENSUS_CALCULATIONS.md`](CONSENSUS_CALCULATIONS.md); automated historical regression against live chain data is not yet wired to CI.

## Build

```bash
cd lean4
lake build
```

**CI:** [`.github/workflows/lean4.yml`](../.github/workflows/lean4.yml) — Lean build + Tier C fixture alignment.
