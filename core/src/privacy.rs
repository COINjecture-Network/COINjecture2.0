// Privacy-Preserving Marketplace Infrastructure
// Enables optional problem commitment with ZK well-formedness proofs
//
// Design Philosophy:
// - Two-mode system: Public (current behavior) and Private (commitment-based)
// - ZK proofs ensure committed problems are well-formed without revealing instance
// - Maintains PoUW asymmetry measurement while adding privacy layer

use crate::{Hash, ProblemType};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Submission mode for marketplace bounties
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SubmissionMode {
    /// Public mode: Problem instance is fully visible on-chain
    /// Use case: Open problems, competitions, public bounties
    Public { problem: ProblemType },

    /// Private mode: Only commitment to problem is visible
    /// Use case: Proprietary problems, sensitive optimization, private bounties
    Private {
        /// Commitment to problem instance
        /// commitment = H(problem_instance || salt)
        problem_commitment: Hash,

        /// Zero-knowledge proof that committed problem is well-formed
        /// Proves: "I know a valid ProblemType P such that H(P || salt) = commitment"
        /// Without revealing P itself
        zk_wellformed_proof: WellformednessProof,

        /// Problem parameters that can be public (for solver estimation)
        /// E.g., "SubsetSum with 1000 numbers", without revealing the numbers
        public_params: ProblemParameters,
    },
}

/// Problem parameters that can be revealed without leaking the instance
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProblemParameters {
    /// Problem type identifier (SubsetSum, SAT, TSP, etc.)
    pub problem_type: String,

    /// Problem size/complexity (number of variables, cities, etc.)
    pub size: usize,

    /// Estimated minimum work score
    pub complexity_estimate: f64,
}

/// Zero-knowledge proof that a committed problem is well-formed
///
/// Institutional Implementation Notes:
/// - For production: Use ark-groth16, bellman, or halo2
/// - For testnet: Placeholder proof with commitment verification
/// - Proof circuit must verify:
///   1. Problem conforms to ProblemType structure
///   2. Problem parameters match public_params
///   3. Commitment matches H(problem || salt)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WellformednessProof {
    /// Proof bytes (circuit-specific encoding)
    /// For Groth16: ~200 bytes
    /// For PLONK: ~400-800 bytes
    /// For Halo2: recursive proof, ~1-2KB
    pub proof_bytes: Vec<u8>,

    /// Verification key hash (ensures proof uses correct circuit)
    pub vk_hash: Hash,

    /// Public inputs to the circuit
    pub public_inputs: Vec<Vec<u8>>,
}

impl WellformednessProof {
    /// Create a new well-formedness proof
    ///
    /// # Arguments
    /// * `problem` - The problem instance (private witness)
    /// * `salt` - Random salt for commitment
    /// * `public_params` - Public parameters to prove consistency
    ///
    /// # Returns
    /// Result containing proof and commitment
    pub fn create(
        problem: &ProblemType,
        salt: &[u8; 32],
        public_params: &ProblemParameters,
    ) -> Result<(Self, Hash), PrivacyError> {
        // Compute commitment
        let commitment = Self::compute_commitment(problem, salt);

        // TODO: Implement full ZK circuit proof generation
        // For now: Create placeholder proof that stores commitment
        // Production: Use ark-groth16 or bellman to prove:
        //   1. problem is valid ProblemType
        //   2. problem.type == public_params.problem_type
        //   3. problem.size == public_params.size
        //   4. H(problem || salt) == commitment

        let proof = WellformednessProof {
            proof_bytes: Self::create_placeholder_proof(problem, salt, public_params)?,
            vk_hash: Self::get_verification_key_hash(),
            public_inputs: vec![
                commitment.as_bytes().to_vec(),
                bincode::serialize(&public_params)
                    .map_err(|_| PrivacyError::SerializationFailed)?,
            ],
        };

        Ok((proof, commitment))
    }

    /// Verify the well-formedness proof
    ///
    /// # Arguments
    /// * `commitment` - The problem commitment to verify against
    /// * `public_params` - Public parameters that proof claims
    ///
    /// # Returns
    /// true if proof is valid, false otherwise
    pub fn verify(&self, commitment: &Hash, public_params: &ProblemParameters) -> bool {
        // Verify VK hash matches expected circuit
        if self.vk_hash != Self::get_verification_key_hash() {
            return false;
        }

        // Verify public inputs match provided values
        if self.public_inputs.len() != 2 {
            return false;
        }

        if self.public_inputs[0] != commitment.as_bytes() {
            return false;
        }

        let params_bytes = match bincode::serialize(&public_params) {
            Ok(b) => b,
            Err(_) => return false,
        };

        if self.public_inputs[1] != params_bytes {
            return false;
        }

        // TODO: Verify actual ZK proof using verification key
        // For now: Verify placeholder proof
        Self::verify_placeholder_proof(&self.proof_bytes, commitment, public_params)
    }

    /// Compute commitment to problem instance
    fn compute_commitment(problem: &ProblemType, salt: &[u8; 32]) -> Hash {
        let problem_bytes = bincode::serialize(problem).unwrap_or_default();

        let mut hasher = Sha256::new();
        hasher.update(&problem_bytes);
        hasher.update(salt);

        let hash_bytes = hasher.finalize();
        Hash::new(&hash_bytes[..])
    }

    /// Get verification key hash for the circuit
    fn get_verification_key_hash() -> Hash {
        // TODO: Replace with actual VK hash after circuit implementation
        // For testnet: Use fixed placeholder
        Hash::new(b"coinject-marketplace-wellformedness-circuit-v1")
    }

    // ========== PLACEHOLDER IMPLEMENTATION (TESTNET ONLY) ==========
    // Production deployment must replace these with real ZK proofs

    /// Create placeholder proof for testnet (NOT cryptographically secure)
    fn create_placeholder_proof(
        problem: &ProblemType,
        salt: &[u8; 32],
        _public_params: &ProblemParameters,
    ) -> Result<Vec<u8>, PrivacyError> {
        // SECURITY WARNING: This is NOT a real ZK proof
        // For testnet demonstration only

        let problem_bytes =
            bincode::serialize(problem).map_err(|_| PrivacyError::SerializationFailed)?;

        // Placeholder: Store H(problem || salt || "PLACEHOLDER")
        let mut hasher = Sha256::new();
        hasher.update(&problem_bytes);
        hasher.update(salt);
        hasher.update(b"PLACEHOLDER_PROOF_TESTNET_ONLY");

        Ok(hasher.finalize().to_vec())
    }

    /// Verify placeholder proof (NOT cryptographically secure)
    fn verify_placeholder_proof(
        _proof_bytes: &[u8],
        _commitment: &Hash,
        _public_params: &ProblemParameters,
    ) -> bool {
        // SECURITY WARNING: This accepts all proofs
        // For testnet demonstration only

        // TODO: Replace with actual ZK proof verification
        // Must use verification key and public inputs
        true
    }
}

/// Problem reveal for private bounties
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProblemReveal {
    /// The actual problem instance
    pub problem: ProblemType,

    /// Salt used in commitment
    pub salt: [u8; 32],

    /// Timestamp of reveal
    pub revealed_at: i64,
}

impl ProblemReveal {
    /// Create a new problem reveal
    pub fn new(problem: ProblemType, salt: [u8; 32]) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        ProblemReveal {
            problem,
            salt,
            revealed_at: now,
        }
    }

    /// Verify reveal matches commitment
    pub fn verify(&self, commitment: &Hash) -> bool {
        let computed = WellformednessProof::compute_commitment(&self.problem, &self.salt);
        computed == *commitment
    }
}

/// Privacy-related errors
#[derive(Debug, thiserror::Error)]
pub enum PrivacyError {
    #[error("Failed to serialize data")]
    SerializationFailed,

    #[error("Invalid commitment")]
    InvalidCommitment,

    #[error("Proof verification failed")]
    ProofVerificationFailed,

    #[error("Problem not revealed yet")]
    ProblemNotRevealed,

    #[error("Reveal does not match commitment")]
    RevealMismatch,

    #[error("Invalid problem parameters")]
    InvalidParameters,
}

impl SubmissionMode {
    /// Get problem if available (public mode or already revealed)
    pub fn problem(&self) -> Option<&ProblemType> {
        match self {
            SubmissionMode::Public { problem } => Some(problem),
            SubmissionMode::Private { .. } => None,
        }
    }

    /// Get problem commitment (for private mode)
    pub fn commitment(&self) -> Option<&Hash> {
        match self {
            SubmissionMode::Public { .. } => None,
            SubmissionMode::Private {
                problem_commitment, ..
            } => Some(problem_commitment),
        }
    }

    /// Check if this is a private submission
    pub fn is_private(&self) -> bool {
        matches!(self, SubmissionMode::Private { .. })
    }

    /// Get public parameters
    pub fn public_params(&self) -> Option<&ProblemParameters> {
        match self {
            SubmissionMode::Private { public_params, .. } => Some(public_params),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_public_submission_mode() {
        let problem = ProblemType::SubsetSum {
            numbers: vec![1, 2, 3, 4, 5],
            target: 9,
        };

        let mode = SubmissionMode::Public {
            problem: problem.clone(),
        };

        assert!(!mode.is_private());
        assert_eq!(mode.problem(), Some(&problem));
        assert_eq!(mode.commitment(), None);
    }

    #[test]
    fn test_private_submission_mode() {
        let problem = ProblemType::SubsetSum {
            numbers: vec![1, 2, 3, 4, 5],
            target: 9,
        };

        let salt = [42u8; 32];
        let public_params = ProblemParameters {
            problem_type: "SubsetSum".to_string(),
            size: 5,
            complexity_estimate: 10.0,
        };

        let (proof, commitment) = WellformednessProof::create(&problem, &salt, &public_params)
            .expect("Failed to create proof");

        let mode = SubmissionMode::Private {
            problem_commitment: commitment,
            zk_wellformed_proof: proof,
            public_params,
        };

        assert!(mode.is_private());
        assert_eq!(mode.problem(), None);
        assert!(mode.commitment().is_some());
    }

    #[test]
    fn test_wellformedness_proof_creation() {
        let problem = ProblemType::SubsetSum {
            numbers: vec![10, 20, 30, 40],
            target: 50,
        };

        let salt = [99u8; 32];
        let public_params = ProblemParameters {
            problem_type: "SubsetSum".to_string(),
            size: 4,
            complexity_estimate: 15.0,
        };

        let result = WellformednessProof::create(&problem, &salt, &public_params);
        assert!(result.is_ok());

        let (proof, commitment) = result.unwrap();

        // Verify proof against correct commitment
        assert!(proof.verify(&commitment, &public_params));

        // Verify proof fails with wrong commitment
        let wrong_commitment = Hash::new(b"wrong");
        assert!(!proof.verify(&wrong_commitment, &public_params));
    }

    #[test]
    fn test_problem_reveal() {
        let problem = ProblemType::SubsetSum {
            numbers: vec![1, 2, 3],
            target: 6,
        };

        let salt = [77u8; 32];
        let public_params = ProblemParameters {
            problem_type: "SubsetSum".to_string(),
            size: 3,
            complexity_estimate: 5.0,
        };

        let (_proof, commitment) = WellformednessProof::create(&problem, &salt, &public_params)
            .expect("Failed to create proof");

        // Create reveal
        let reveal = ProblemReveal::new(problem.clone(), salt);

        // Verify reveal matches commitment
        assert!(reveal.verify(&commitment));

        // Verify reveal fails with wrong commitment
        let wrong_commitment = Hash::new(b"invalid");
        assert!(!reveal.verify(&wrong_commitment));
    }

    #[test]
    fn test_commitment_determinism() {
        let problem = ProblemType::SubsetSum {
            numbers: vec![7, 14, 21],
            target: 21,
        };

        let salt = [123u8; 32];

        let commitment1 = WellformednessProof::compute_commitment(&problem, &salt);
        let commitment2 = WellformednessProof::compute_commitment(&problem, &salt);

        assert_eq!(commitment1, commitment2);
    }
}
