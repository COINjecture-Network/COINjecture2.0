# Bootstrap Phase: From Seeded Defaults to Empirical Operation

## The Bootstrap Problem

COINjecture's design principles state that all parameters should be:
1. **Dimensionless** — pure ratios with no arbitrary units
2. **Self-referential** — measured against the network's own state
3. **Empirically grounded** — derived from actual network behavior

However, at genesis (block 0), the network has no history to measure against.
This creates a bootstrapping contradiction: self-referential parameters need
data that doesn't exist yet.

## Resolution: Seeded Defaults with Empirical Convergence

The system launches with **seeded defaults** derived from the mathematical
framework, not from arbitrary choices:

| Parameter | Seeded Default | Source | Empirical Transition |
|-----------|---------------|--------|---------------------|
| Target block time | 14.14s (= 10√2) | 10 × η⁻¹ where η = 1/√2 | Observed after difficulty window fills |
| Difficulty window | 20 blocks | ~283s ≈ 20 × 10√2 | Fixed (protocol parameter) |
| Starting problem size | 20 | Canonical SubsetSum size unit | Adjusted after first window |
| Optimal solve time | 5.0s | ~block_time / (√2 + 1) | Refined from empirical data |
| Size limits | (5, 50) | min_size / max_size safety bounds | Overridden by ProblemDescriptor |

### Why 14.14 seconds?

The target block time is **not arbitrary**. It equals 10√2 ≈ 14.142 seconds,
which connects to the μ-framework:

- The Satoshi constant η = 1/√2
- The characteristic time τ_c = η⁻¹ = √2
- Block time = 10 × τ_c = 10√2

The factor of 10 provides a human-scale operating interval while preserving
the mathematical relationship to the damping constant.

### Transition to Empirical Operation

After the difficulty window fills (20 blocks ≈ 4.7 minutes of network history),
the system transitions from seeded defaults to empirical measurement:

1. **Block 0 – 19**: Seeded defaults govern difficulty and problem sizing
2. **Block 20+**: DifficultyAdjuster measures actual solve times from the
   trailing window and adjusts problem size to converge on the target
3. **Block 100+**: Empirical data dominates; seeded defaults have negligible influence

The seeded defaults are chosen so that the transition is monotonic (no
oscillation) and critically damped — the system converges to its empirical
operating point without overshooting.

### Honest Acknowledgment

During the bootstrap phase (blocks 0–19), the system does NOT satisfy the
self-referential principle. It operates on externally-seeded parameters.
This is an unavoidable property of any self-referential system — you cannot
measure yourself before you exist. The design ensures this phase is short
(< 5 minutes) and that the transition to empirical operation is smooth.

## ProblemDescriptor and the Bootstrap

As of the ProblemDescriptor trait system, problem-type-specific parameters
(scaling exponents, size ratios, absolute limits) are defined per-descriptor
rather than hardcoded in system code. During bootstrap:

- **SubsetSum** (size_ratio=1.0, max=60): The canonical reference type
- **SAT** (size_ratio=0.75, max=120): Scaled relative to SubsetSum
- **TSP** (size_ratio=0.35, max=30): Factorial complexity requires smaller sizes
- **GraphColoring** (size_ratio=0.6, max=80): Graph problems scale moderately
- **Factorization** (size_ratio=0.5, max=200): Sub-exponential, large sizes safe
- **SVP** (size_ratio=0.4, max=40): Lattice problems, moderate ceiling

These seeded values are initial seeds. The network refines them empirically
after the bootstrap window fills. New problem types registered via
`ProblemRegistry::register()` provide their own seeded defaults through the
`ProblemDescriptor` trait.
