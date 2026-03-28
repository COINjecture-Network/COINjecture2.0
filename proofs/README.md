# Formal Proofs

This directory contains machine-checked mathematical proofs that verify COINjecture's theoretical foundations.

## Eigenverse (git submodule)

**Repository**: [github.com/beanapologist/Eigenverse](https://github.com/beanapologist/Eigenverse)

450 theorems verified in Lean 4 with zero `sorry` — covering:

- **Algebra** (127 theorems): mu^8=1 closure, Silver ratio, coherence C(r)<=1, Z/8Z memory
- **Geometry** (141 theorems): Rotation matrices, unit circle orbit, hyperbolic Pythagorean identity
- **Physics** (159 theorems): c=1/sqrt(mu0*eps0), alpha~1/137, Koide formula, Lorentz geometry
- **Quantum** (120 theorems): Floquet time crystals, gravity-quantum duality, Theorem Q
- **Chemistry** (44 theorems): NIST atomic weights, Ohm-coherence duality

### Building the proofs

```bash
cd proofs/eigenverse/formal-lean/
lake exe cache get   # download Mathlib cache (~5 min)
lake build           # verify all 450 theorems
lake exe formalLean  # print theorem summary
```

### Connection to COINjecture

The mu-balance primitive (mu = e^(i*3pi/4) = (-1+i)/sqrt(2)) proven in Eigenverse is the foundation for:

- **Consensus**: The `work_score` formula uses coherence function C(r) = 2r/(1+r^2)
- **Tokenomics**: Dimensional pool scales derive from D_n = e^(-eta*tau_n) where eta = 1/sqrt(2)
- **Stability**: Lyapunov convergence guarantees eta = lambda = 1/sqrt(2) (critical damping)

Every mathematical claim in the COINjecture whitepaper can be traced to a specific Lean 4 theorem in this submodule.
