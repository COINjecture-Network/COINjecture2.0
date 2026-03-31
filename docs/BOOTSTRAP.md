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

The system launches with seeded defaults, then converges to empirical
operation as history accumulates.

| Parameter | Seeded Default | Rationale | Empirical Transition |
|-----------|---------------|-----------|---------------------|
| Block time | 10 seconds | Engineering choice (see below) | Fixed protocol parameter |
| Difficulty window | 20 blocks | ~200s of history | Fixed protocol parameter |
| Starting problem size | 20 | Canonical SubsetSum size unit | Adjusted after first window |
| Optimal solve time | 5.0 seconds | ~block_time / 2 | Refined from observed data |
| Size limits | (5, 50) | Safety bounds | Overridden by ProblemDescriptor |

### Why 10 Seconds?

The block time is an **engineering parameter**, not a mathematical derivation.
It was chosen for three practical constraints:

1. **Long enough for NP solving to be meaningful** — solvers need non-trivial
   time to find solutions to problems of meaningful difficulty
2. **Short enough for reasonable transaction finality** — users should not
   wait minutes for confirmation
3. **Enough overhead margin for verification plus propagation** — validators
   must verify solutions and propagate blocks before the next round

The dimensionless framework (η = 1/√2, unit circle constraint) governs the
**shape** of system dynamics — how difficulty converges, how damping behaves,
how the system avoids oscillation. It does not determine the **clock speed**.
Changing the block time changes how fast the dynamics manifest in wall-clock
time, but not the dynamics themselves.

### Separation of Concerns

| What η governs | What block time governs |
|----------------|------------------------|
| Convergence shape (critical damping) | Clock speed |
| Difficulty adaptation rate (dimensionless) | Transaction finality |
| Pool dynamics (D₁, D₂, D₃ relationships) | Mining interval |
| Stability guarantees (Lyapunov) | Propagation budget |

These are independent choices. The math tells you *how* the system behaves.
The block time tells you *how fast*.

### Storage Implications

At 10-second block intervals with a ~1 KB block header, header-only storage
is approximately 3.2 GB per year. With commodity servers commonly shipping
1 TB or more as of 2025, this is not a constraint even over decades of
operation.

## Transition to Empirical Operation

After the difficulty window fills (20 blocks ≈ 200 seconds of network history),
the system transitions from seeded defaults to empirical measurement:

1. **Blocks 0–19**: Seeded defaults govern difficulty and problem sizing
2. **Block 20+**: DifficultyAdjuster measures actual solve times from the
   trailing window and adjusts problem size to converge on the target
3. **Block 100+**: Empirical data dominates; seeded defaults have negligible
   influence

The seeded defaults are chosen so that the transition is monotonic (no
oscillation) and critically damped — the system converges to its empirical
operating point without overshooting.

## Honest Acknowledgment

During the bootstrap phase (blocks 0–19), the system does **not** satisfy the
self-referential principle. It operates on externally-seeded parameters.
This is an unavoidable property of any self-referential system — you cannot
measure yourself before you exist. The design ensures this phase is short
(~200 seconds) and that the transition to empirical operation is smooth.

## Work Score During Bootstrap

The work score formula requires no seeded defaults at all:

```
work_score = log₂(solve_time / verify_time) × quality_score
```

It measures what actually happened — how long the solver took versus how long
verification took. This makes the work score fully empirical from block 0,
even during the bootstrap phase. The only bootstrap-dependent component is
the **difficulty adjuster**, which determines what size problems to generate.

## Inflation Resistance

A miner could artificially slow their solver to inflate `solve_time`. The
racing incentive handles this: a miner who inflates solve time loses the
block to a faster competitor. The winning block's work score is therefore
the **minimum competitive** solve time, not an inflatable self-report.

During single-miner operation (bootstrap), the difficulty adjuster's target
block time serves as the inflation ceiling. The racing incentive becomes
effective when N ≥ 2 competing miners are active.

## ProblemDescriptor and the Bootstrap

Problem-type-specific parameters (scaling exponents, size ratios, absolute
limits) are defined per-descriptor via the `ProblemDescriptor` trait rather
than hardcoded in system code. During bootstrap:

- **SubsetSum** (size_ratio=1.0, max=60): The canonical reference type
- **SAT** (size_ratio=0.75, max=120): Scaled relative to SubsetSum
- **TSP** (size_ratio=0.35, max=30): Factorial complexity requires smaller sizes
- **GraphColoring** (size_ratio=0.6, max=80): Graph problems scale moderately
- **Factorization** (size_ratio=0.5, max=200): Sub-exponential, large sizes safe
- **SVP** (size_ratio=0.4, max=40): Lattice problems, moderate ceiling

These seeded values are initial estimates. The network refines problem sizing
empirically after the bootstrap window fills. New problem types registered via
`ProblemRegistry::register()` provide their own seeded defaults through the
`ProblemDescriptor` trait — no changes to consensus code required.
