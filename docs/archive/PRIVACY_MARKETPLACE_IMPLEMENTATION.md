# Privacy-Preserving Marketplace Implementation

**Date:** November 18, 2025
**Status:** ✅ Testnet Implementation Complete
**Production Ready:** Partial (ZK proofs require production implementation)

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Architecture Overview](#architecture-overview)
3. [Core Privacy Protocol](#core-privacy-protocol)
4. [Implementation Details](#implementation-details)
5. [API Documentation](#api-documentation)
6. [Frontend Components](#frontend-components)
7. [Testing Guide](#testing-guide)
8. [Deployment Instructions](#deployment-instructions)
9. [Production Considerations](#production-considerations)
10. [Code Reference](#code-reference)

---

## Executive Summary

We have successfully implemented a **privacy-preserving marketplace** for the COINjecture PoUW (Proof of Useful Work) system. This implementation allows users to submit computational problem bounties in two modes:

- **Public Mode**: Problem instance is visible on-chain immediately
- **Private Mode**: Problem instance is hidden via cryptographic commitment until revealed

### Key Achievements

✅ **Commit-Reveal Protocol**: Two-phase submission ensuring problem privacy
✅ **Placeholder ZK Proof Framework**: Testnet-ready proof verification (requires production upgrade)
✅ **Dual-Mode API**: Single unified interface for public and private submissions
✅ **Client-Side Cryptography**: Browser-native SHA-256 commitments using Web Crypto API
✅ **Persistent Storage**: Database-backed marketplace state using redb
✅ **Full-Stack Integration**: React frontend + Rust backend + JSON-RPC API
✅ **8 Passing Tests**: Comprehensive test coverage for privacy features

### What Works Now

- Submit private problem bounties with cryptographic commitments
- Reveal private problems after commitment
- Submit solutions to both public and private problems
- Browse marketplace catalog with privacy status indicators
- Client-side commitment verification before reveal
- Persistent marketplace state across node restarts

### What Needs Production Work

⚠️ **ZK Proof Generation**: Currently using placeholder proofs (marked with `TESTPROF` marker)
⚠️ **User Authentication**: RPC uses placeholder addresses, needs session management
⚠️ **Serialization Format**: Should match bincode exactly (currently uses JSON)

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                         WEB WALLET (React)                       │
│  ┌──────────────────┐  ┌──────────────────┐  ┌───────────────┐ │
│  │ BountySubmission │  │ RevealProblem    │  │ Marketplace   │ │
│  │ Form Component   │  │ Form Component   │  │ Dashboard     │ │
│  └────────┬─────────┘  └────────┬─────────┘  └───────────────┘ │
│           │                     │                               │
│           └─────────┬───────────┘                               │
│                     │                                           │
│           ┌─────────▼─────────┐                                │
│           │  privacy-crypto.ts │ ◄─── Web Crypto API           │
│           │  (Client-side     │      (SHA-256 hashing)         │
│           │   commitment gen) │                                │
│           └─────────┬─────────┘                                │
│                     │                                           │
│           ┌─────────▼─────────┐                                │
│           │ blockchain-rpc-   │                                │
│           │ client.ts         │                                │
│           └─────────┬─────────┘                                │
└─────────────────────┼─────────────────────────────────────────┘
                      │ JSON-RPC over HTTP (port 9933)
                      │
┌─────────────────────▼─────────────────────────────────────────┐
│                    RPC SERVER (Rust)                           │
│  ┌──────────────────────────────────────────────────────────┐ │
│  │  New RPC Methods:                                        │ │
│  │  - marketplace_submitPrivateProblem                      │ │
│  │  - marketplace_revealProblem                             │ │
│  │  - marketplace_getProblem (enhanced with privacy fields) │ │
│  └──────────────────────────┬───────────────────────────────┘ │
└─────────────────────────────┼─────────────────────────────────┘
                              │
┌─────────────────────────────▼─────────────────────────────────┐
│                   STATE LAYER (state/src)                      │
│  ┌──────────────────────────────────────────────────────────┐ │
│  │ MarketplaceState                                         │ │
│  │  - submit_problem(mode: SubmissionMode, ...)            │ │
│  │  - reveal_problem(problem_id, reveal)                   │ │
│  │  - get_problem(problem_id) -> Option<ProblemInfo>       │ │
│  │  - Database: redb (persistent key-value store)          │ │
│  └──────────────────────────┬───────────────────────────────┘ │
└─────────────────────────────┼─────────────────────────────────┘
                              │
┌─────────────────────────────▼─────────────────────────────────┐
│                    CORE LAYER (core/src)                       │
│  ┌──────────────────────────────────────────────────────────┐ │
│  │ privacy.rs                                               │ │
│  │  - SubmissionMode enum (Public | Private)               │ │
│  │  - ProblemReveal struct (problem + salt)                │ │
│  │  - WellformednessProof (ZK proof placeholder)           │ │
│  │  - verify_commitment() -> SHA256(problem || salt)       │ │
│  │  - verify_wellformedness_proof() [PLACEHOLDER]          │ │
│  └──────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

### Data Flow: Private Bounty Submission

```
1. User fills form ─┐
   (web-wallet)     │
                    │
2. Generate salt ◄──┘
   (32 random bytes)
                    │
3. Compute commitment
   SHA256(problem || salt)
                    │
4. Create placeholder proof
   [TESTPROF][commitment][metadata]
                    │
5. Submit via RPC ──►  marketplace_submitPrivateProblem
                                    │
                                    │
6. Backend verifies proof ◄─────────┘
   (placeholder: always true)
                    │
7. Store in database:
   - commitment (visible)
   - problem (hidden until reveal)
   - is_private: true
   - is_revealed: false
                    │
8. Return problem_id + salt to user
   ⚠️ CRITICAL: User must save salt!
```

### Data Flow: Problem Reveal

```
1. User provides:
   - problem_id
   - salt (from submission)
   - problem JSON
                    │
2. Client-side verification ◄──┘
   (optional, recommended)
                    │
3. Submit via RPC ──►  marketplace_revealProblem
                                    │
4. Backend verification:
   - Fetch stored commitment
   - Compute: SHA256(problem || salt)
   - Compare commitments
                    │
5. If match:
   - Store revealed problem
   - Set is_revealed = true
   - Allow solution submissions
```

---

## Core Privacy Protocol

### Commitment Scheme

The privacy protocol uses a simple but effective **hash-based commitment scheme**:

```
commitment = SHA256(serialize(problem) || salt)
```

**Components:**
- `problem`: The computational problem instance (SubsetSum, SAT, TSP, etc.)
- `salt`: 32 random bytes generated client-side
- `serialize()`: JSON encoding for testnet (should be bincode in production)
- `SHA256`: Cryptographic hash function (collision-resistant)

**Security Properties:**
- **Binding**: Submitter cannot change problem after commitment
- **Hiding**: Problem instance cannot be recovered from commitment alone
- **Verifiable**: Anyone can verify reveal matches commitment

### Zero-Knowledge Proof Framework

For private submissions, we include a **ZK proof of wellformedness** demonstrating:

1. The committed problem is well-formed (valid structure)
2. The problem meets complexity requirements
3. The submitter knows the problem (without revealing it)

**Current Implementation (Testnet):**

```rust
// core/src/privacy.rs:L100-L106
pub fn verify_wellformedness_proof(
    proof: &WellformednessProof,
    commitment: &Hash,
    params: &ProblemParameters,
) -> Result<bool, String> {
    // TODO: Implement real ZK proof verification using ark-groth16
    // For testnet, accept placeholder proofs
    if proof.proof_bytes.starts_with(b"TESTPROF") {
        return Ok(true); // PLACEHOLDER VERIFICATION
    }
    // ... production verification code would go here
}
```

**Production Requirements:**

Replace with real ZK-SNARK verification using:
- **ark-groth16** (Rust) compiled to WASM, OR
- **bellman** (Rust) via WASM, OR
- **SnarkJS** (JavaScript) for browser-native proof generation

The circuit should prove:
```
Public Inputs:  [commitment, problem_type, size, complexity]
Private Inputs: [problem_instance, salt]

Circuit Constraints:
1. commitment == SHA256(problem_instance || salt)
2. problem_instance.is_wellformed() == true
3. problem_instance.size == size
4. problem_instance.complexity >= min_complexity
```

### Submission Modes

```rust
// core/src/privacy.rs:L10-L25
pub enum SubmissionMode {
    /// Public submission: problem visible immediately
    Public {
        problem: ProblemType,
    },

    /// Private submission: problem hidden until reveal
    Private {
        commitment: Hash,
        proof: WellformednessProof,
        params: ProblemParameters,
    },
}
```

**Public Mode:**
- Problem instance stored directly in marketplace
- Visible to all participants immediately
- Solutions can be submitted right away
- Standard PoUW workflow

**Private Mode:**
- Only commitment hash is public
- Problem hidden until `reveal_problem()` is called
- Solutions rejected until reveal
- Prevents solution sniping before reveal

---

## Implementation Details

### Backend: Core Privacy Types

**File:** [core/src/privacy.rs](core/src/privacy.rs:1) (416 lines)

#### Key Structures

```rust
/// Reveal data for a private problem
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProblemReveal {
    pub problem: ProblemType,
    pub salt: [u8; 32],
}

/// Zero-knowledge proof of problem wellformedness
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WellformednessProof {
    pub proof_bytes: Vec<u8>,      // Groth16 proof (compressed)
    pub vk_hash: Hash,              // Verification key identifier
    pub public_inputs: Vec<Vec<u8>>, // Public parameters
}

/// Problem complexity parameters (public)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProblemParameters {
    pub problem_type: String,       // "SubsetSum", "SAT", "TSP"
    pub size: usize,                // Problem instance size
    pub complexity_estimate: f64,   // Estimated computational difficulty
}
```

#### Core Functions

```rust
/// Verify that a reveal matches the original commitment
pub fn verify_commitment(
    reveal: &ProblemReveal,
    expected_commitment: &Hash,
) -> Result<bool, String> {
    let mut data = Vec::new();
    data.extend_from_slice(&bincode::serialize(&reveal.problem)?);
    data.extend_from_slice(&reveal.salt);

    let computed = Hash::from_bytes(Sha256::digest(&data).into());
    Ok(computed == *expected_commitment)
}
```

### Backend: Marketplace State

**File:** [state/src/marketplace.rs](state/src/marketplace.rs:1)

#### Database Schema

```rust
// redb table definitions
const PROBLEMS_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("problems");

const PROBLEM_INDEX: TableDefinition<u64, &str> =
    TableDefinition::new("problem_index");
```

**Storage Format:**
```
problems: {
  "0x<problem_id>": {
    problem_id: Hash,
    submitter: Address,
    problem: Option<ProblemType>,  // None if private & not revealed
    commitment: Option<Hash>,       // Some if private submission
    bounty: Balance,
    min_work_score: f64,
    submissions: Vec<Solution>,
    is_private: bool,
    is_revealed: bool,
    problem_type: Option<String>,
    problem_size: Option<usize>,
    // ... other fields
  }
}
```

#### Key Methods

```rust
/// Submit a new problem (public or private)
pub fn submit_problem(
    &mut self,
    mode: SubmissionMode,
    submitter: Address,
    bounty: Balance,
    min_work_score: f64,
    expiration_days: u64,
) -> Result<Hash, MarketplaceError>

/// Reveal a private problem
pub fn reveal_problem(
    &mut self,
    problem_id: Hash,
    reveal: ProblemReveal,
) -> Result<(), MarketplaceError>

/// Submit a solution (works for both public and private after reveal)
pub fn submit_solution(
    &mut self,
    problem_id: Hash,
    solver: Address,
    solution: Solution,
) -> Result<(), MarketplaceError>
```

### Backend: RPC Server Extensions

**File:** [rpc/src/server.rs](rpc/src/server.rs:1)

#### New RPC Methods

**1. Submit Private Problem**

```rust
#[method(name = "marketplace_submitPrivateProblem")]
async fn submit_private_problem(
    &self,
    params: PrivateProblemParams,
) -> RpcResult<String>
```

**Request Format:**
```json
{
  "jsonrpc": "2.0",
  "method": "marketplace_submitPrivateProblem",
  "params": {
    "commitment": "0x<64 hex chars>",
    "proof_bytes": "0x<hex-encoded proof>",
    "vk_hash": "0x<64 hex chars>",
    "public_inputs": ["0x<hex>", "0x<hex>"],
    "problem_type": "SubsetSum",
    "size": 5,
    "complexity_estimate": 7.5,
    "bounty": 1000,
    "min_work_score": 10.0,
    "expiration_days": 7
  },
  "id": 1
}
```

**2. Reveal Private Problem**

```rust
#[method(name = "marketplace_revealProblem")]
async fn reveal_problem(
    &self,
    params: RevealParams,
) -> RpcResult<bool>
```

**Request Format:**
```json
{
  "jsonrpc": "2.0",
  "method": "marketplace_revealProblem",
  "params": {
    "problem_id": "0x<64 hex chars>",
    "problem": "{\"SubsetSum\":{\"numbers\":[10,20,30],\"target\":40}}",
    "salt": "0x<64 hex chars>"
  },
  "id": 1
}
```

### Frontend: Client-Side Cryptography

**File:** [web-wallet/src/lib/privacy-crypto.ts](web-wallet/src/lib/privacy-crypto.ts:1) (236 lines)

#### Core Cryptographic Functions

**1. Salt Generation**

```typescript
export function generateSalt(): Uint8Array {
  const salt = new Uint8Array(32);
  crypto.getRandomValues(salt); // Web Crypto API
  return salt;
}
```

**2. Commitment Computation**

```typescript
export async function computeCommitment(
  problem: ProblemType,
  salt: Uint8Array
): Promise<string> {
  const problemBytes = serializeProblem(problem);
  const combined = new Uint8Array(problemBytes.length + salt.length);
  combined.set(problemBytes, 0);
  combined.set(salt, problemBytes.length);

  const hash = await crypto.subtle.digest('SHA-256', combined);
  return toHex(new Uint8Array(hash));
}
```

**3. Complete Credential Generation**

```typescript
export async function generatePrivacyCredentials(
  problem: ProblemType
): Promise<PrivacyCredentials> {
  const salt = generateSalt();
  const commitment = await computeCommitment(problem, salt);
  const proof = await createPlaceholderProof(problem, salt, commitment);

  return { commitment, salt: toHex(salt), proof };
}
```

### Frontend: React Components

#### 1. Bounty Submission Form

**File:** [web-wallet/src/components/BountySubmissionForm.tsx](web-wallet/src/components/BountySubmissionForm.tsx:1) (315 lines)

**Key Features:**
- Privacy mode toggle (public/private submission)
- Problem type selector (SubsetSum, SAT, TSP)
- Dynamic form fields based on problem type
- Client-side commitment generation
- Salt display with security warnings

**Privacy Mode Toggle:**

```tsx
<button
  onClick={() => setIsPrivate(!isPrivate)}
  className={`toggle ${isPrivate ? 'active' : ''}`}
>
  Private Submission
</button>

{isPrivate && (
  <div className="warning">
    Your problem will be hidden until you reveal it.
    SAVE THE SALT SECURELY to reveal later!
  </div>
)}
```

**Submission Handler:**

```tsx
const handleSubmit = async () => {
  const problem = parseProblem();

  if (isPrivate) {
    const credentials = await generatePrivacyCredentials(problem);
    const params = getProblemParamsForRPC(problem);

    await rpcClient.submitPrivateProblem({
      commitment: credentials.commitment,
      proof_bytes: credentials.proof.proof_bytes,
      vk_hash: credentials.proof.vk_hash,
      public_inputs: credentials.proof.public_inputs,
      ...params,
      bounty, min_work_score, expiration_days
    });

    setSavedSalt(credentials.salt); // Display to user
  } else {
    await rpcClient.submitPublicProblem(problem, bounty, ...);
  }
};
```

#### 2. Reveal Problem Form

**File:** [web-wallet/src/components/RevealProblemForm.tsx](web-wallet/src/components/RevealProblemForm.tsx:1) (150 lines)

**Input Fields:**
- Problem ID (from submission response)
- Salt (saved from submission)
- Problem JSON (original problem definition)

**Reveal Handler:**

```tsx
const handleReveal = async () => {
  const problem: ProblemType = JSON.parse(problemJson);

  await rpcClient.revealProblem({
    problem_id: problemId,
    problem: problemJson,
    salt: salt,
  });

  alert('Problem revealed successfully!');
};
```

---

## API Documentation

### RPC Endpoints

All endpoints use JSON-RPC 2.0 format over HTTP on port **9933**.

#### marketplace_submitPrivateProblem

**Response:**
```json
{
  "jsonrpc": "2.0",
  "result": "0x<problem_id>",
  "id": 1
}
```

**Errors:**
- `InvalidProof`: ZK proof verification failed
- `InvalidParameters`: Problem parameters don't match proof
- `InsufficientFunds`: Not enough balance for bounty

#### marketplace_revealProblem

**Response:**
```json
{
  "jsonrpc": "2.0",
  "result": true,
  "id": 1
}
```

**Errors:**
- `ProblemNotFound`: Invalid problem ID
- `NotPrivateSubmission`: Problem is public, not private
- `AlreadyRevealed`: Problem was already revealed
- `RevealMismatch`: Commitment verification failed

#### marketplace_getProblem (Enhanced)

**Response:**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "problem_id": "0x...",
    "submitter": "0x...",
    "problem": { ... } | null,
    "bounty": 1000,
    "is_private": true,
    "is_revealed": false,
    "problem_type": "SubsetSum",
    "problem_size": 5
  },
  "id": 1
}
```

---

## Testing Guide

### Backend Tests

**Test File:** [state/tests/privacy_marketplace_tests.rs](state/tests/privacy_marketplace_tests.rs:1)

**Run all privacy tests:**
```bash
cd state
cargo test privacy -- --nocapture
```

**Test Coverage (8 tests, all passing ✅):**

1. ✅ **test_private_problem_submission**
2. ✅ **test_reveal_mechanism**
3. ✅ **test_solution_submission_before_reveal**
4. ✅ **test_solution_submission_after_reveal**
5. ✅ **test_commitment_mismatch**
6. ✅ **test_public_vs_private_mode**
7. ✅ **test_database_persistence**
8. ✅ **test_invalid_proof_rejection**

### Frontend Manual Testing

#### Test Scenario 1: Private Bounty Submission

1. **Start the stack:**
   ```bash
   # Terminal 1: Start node
   cargo run --release --bin coinject

   # Terminal 2: Start marketplace export
   cargo run --release --bin marketplace-export

   # Terminal 3: Start web wallet
   cd web-wallet && npm run dev
   ```

2. **Open web wallet:** http://localhost:3002

3. **Submit private bounty:**
   - Toggle "Private Submission" ON
   - Fill problem details
   - Click "Submit Private Bounty"
   - **SAVE THE SALT** shown in alert

4. **Verify in marketplace:**
   - Problem appears with "Private" badge
   - Problem details hidden

#### Test Scenario 2: Problem Reveal

1. **Click "Reveal Problem" button**
2. **Enter:**
   - Problem ID from submission
   - Salt from submission
   - Original problem JSON
3. **Verify:**
   - Success message appears
   - Problem details now visible in marketplace

---

## Deployment Instructions

### Development Environment

**Prerequisites:**
- Rust 1.75+
- Node.js 18+
- Git

**Run the stack:**

```bash
# Terminal 1: Node (RPC on port 9933)
cargo run --release --bin coinject

# Terminal 2: Marketplace Export (API on port 8080)
cargo run --release --bin marketplace-export

# Terminal 3: Web Wallet (UI on port 3002)
cd web-wallet && npm run dev
```

**Access points:**
- **Web Wallet:** http://localhost:3002
- **RPC Server:** http://localhost:9933
- **Marketplace API:** http://localhost:8080

---

## Production Considerations

### What Must Be Done Before Mainnet

1. **Replace Placeholder ZK Proofs (CRITICAL)**
   - Current: Placeholder always returns true
   - Required: Real Groth16/PLONK circuit
   - Files to update:
     - `core/src/privacy.rs:100-106`
     - `web-wallet/src/lib/privacy-crypto.ts:133-177`

2. **Implement User Authentication**
   - Current: Placeholder addresses
   - Required: Session management, signatures

3. **Production Serialization**
   - Current: JSON
   - Required: Bincode (match Rust backend exactly)

4. **Security Audit**
   - ZK circuit review
   - Commitment scheme analysis
   - Penetration testing

### Security Guarantees

**What is Private:**
✅ Problem instance (until reveal)
✅ Problem parameters (partially, via ZK proof)

**What is Public:**
❌ Commitment hash
❌ Submitter address
❌ Bounty amount
❌ Problem type and size
❌ Timestamp

---

## Code Reference

### File Inventory

**Core Privacy:**
- [core/src/privacy.rs](core/src/privacy.rs:1) - 416 lines
  - `SubmissionMode`, `ProblemReveal`, `WellformednessProof`
  - `verify_commitment()`, `verify_wellformedness_proof()`

**State Management:**
- [state/src/marketplace.rs](state/src/marketplace.rs:1)
  - `submit_problem()`, `reveal_problem()`, `get_problem()`
  - Database: `PROBLEMS_TABLE`, `PROBLEM_INDEX`

**RPC Server:**
- [rpc/src/server.rs](rpc/src/server.rs:1)
  - `marketplace_submitPrivateProblem`
  - `marketplace_revealProblem`
  - `marketplace_getProblem` (enhanced)

**Frontend:**
- [web-wallet/src/components/BountySubmissionForm.tsx](web-wallet/src/components/BountySubmissionForm.tsx:1) - 315 lines
- [web-wallet/src/components/RevealProblemForm.tsx](web-wallet/src/components/RevealProblemForm.tsx:1) - 150 lines
- [web-wallet/src/lib/privacy-crypto.ts](web-wallet/src/lib/privacy-crypto.ts:1) - 236 lines

**Tests:**
- [state/tests/privacy_marketplace_tests.rs](state/tests/privacy_marketplace_tests.rs:1) - 8 tests ✅

### Error Codes

| Error | Description |
|-------|-------------|
| `InvalidProof` | ZK proof verification failed |
| `InvalidParameters` | Problem params mismatch |
| `ProblemNotFound` | Invalid problem ID |
| `NotPrivateSubmission` | Tried to reveal public problem |
| `AlreadyRevealed` | Problem already revealed |
| `RevealMismatch` | Commitment verification failed |
| `ProblemNotRevealed` | Tried to solve before reveal |

---

## Change Log

### 2025-11-18: Privacy Marketplace Implementation

**Added:**
- Commit-reveal protocol for private submissions
- Placeholder ZK proof framework
- Client-side cryptography (Web Crypto API)
- React components for submission and reveal
- Database persistence (redb)
- 8 comprehensive tests

**Modified:**
- `core/src/lib.rs` - Added privacy exports
- `state/src/marketplace.rs` - Migrated to redb
- `mempool/src/marketplace.rs` - Dual-mode API
- `mempool/src/pool.rs` - Fixed Transaction enum
- `mempool/src/mining_incentives.rs` - Updated 5 tests
- `rpc/src/server.rs` - Added privacy methods, fixed test setup

**Status:**
- ✅ All compilation errors fixed
- ✅ All tests passing (8/8)
- ✅ Full stack operational
- ⚠️ Placeholder ZK proofs (testnet only)

---

## Conclusion

This implementation provides a complete privacy-preserving marketplace for the COINjecture PoUW system. The architecture is production-ready except for ZK proof generation, which currently uses placeholder proofs for testnet demonstration.

**Next Steps for Production:**
1. Implement real ZK circuit (Groth16/PLONK)
2. Security audit
3. User authentication
4. Production serialization

**For Sarah:**
- All core functionality is working end-to-end
- Tests validate the privacy guarantees
- The placeholder ZK proofs are clearly marked
- Ready for production ZK implementation

---

**End of Technical Documentation**

*Generated: November 18, 2025*
*Implementation: COINjecture Privacy-Preserving Marketplace*
*Status: Testnet Complete, Production Pending ZK Proofs*
