use crate::{Hash, ProblemType, Solution};
use serde::{Deserialize, Serialize};

/// Commitment = H(problem_params || salt || H(solution))
/// Prevents grinding by cryptographically binding to solution before mining
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Commitment {
    /// The commitment hash
    pub hash: Hash,
    /// Problem hash (for verification)
    pub problem_hash: Hash,
}

impl Commitment {
    /// Create commitment from problem, solution, and epoch salt (using bincode serialization)
    pub fn create(problem: &ProblemType, solution: &Solution, epoch_salt: &Hash) -> Self {
        let problem_hash = problem.hash();
        let solution_hash = Hash::new(&bincode::serialize(solution).unwrap_or_default());

        // commitment = H(problem_params || salt || H(solution))
        let mut commitment_data = Vec::new();
        commitment_data.extend_from_slice(problem_hash.as_bytes());
        commitment_data.extend_from_slice(epoch_salt.as_bytes());
        commitment_data.extend_from_slice(solution_hash.as_bytes());

        let hash = Hash::new(&commitment_data);

        Commitment {
            hash,
            problem_hash,
        }
    }

    /// Create commitment from problem, solution, and epoch salt (using JSON serialization)
    /// This enables client-side mining from web browsers that can't use bincode
    pub fn create_from_json(problem: &ProblemType, solution: &Solution, epoch_salt: &Hash) -> Self {
        // Hash problem using JSON serialization
        let problem_json = serde_json::to_vec(problem).unwrap_or_default();
        let problem_hash = Hash::new(&problem_json);
        
        // Hash solution using JSON serialization
        let solution_json = serde_json::to_vec(solution).unwrap_or_default();
        let solution_hash = Hash::new(&solution_json);

        // commitment = H(problem_hash_bytes || epoch_salt_bytes || solution_hash_bytes)
        let mut commitment_data = Vec::new();
        commitment_data.extend_from_slice(problem_hash.as_bytes());
        commitment_data.extend_from_slice(epoch_salt.as_bytes());
        commitment_data.extend_from_slice(solution_hash.as_bytes());

        let hash = Hash::new(&commitment_data);

        Commitment {
            hash,
            problem_hash,
        }
    }

    /// Verify that commitment matches revealed problem and solution
    /// Tries both bincode and JSON serialization to support both server-side and client-side mining
    pub fn verify(&self, problem: &ProblemType, solution: &Solution, epoch_salt: &Hash) -> bool {
        // Try bincode serialization first (server-side mining)
        let expected_bincode = Self::create(problem, solution, epoch_salt);
        if self.hash == expected_bincode.hash && self.problem_hash == expected_bincode.problem_hash {
            return true;
        }

        // Try JSON serialization (client-side mining from web browsers)
        let expected_json = Self::create_from_json(problem, solution, epoch_salt);
        self.hash == expected_json.hash && self.problem_hash == expected_json.problem_hash
    }

    /// Serialize for block header
    pub fn to_bytes(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap_or_default()
    }
}

/// Solution reveal (broadcast after valid header found)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SolutionReveal {
    pub problem: ProblemType,
    pub solution: Solution,
    pub commitment: Commitment,
}

impl SolutionReveal {
    pub fn new(problem: ProblemType, solution: Solution, commitment: Commitment) -> Self {
        SolutionReveal {
            problem,
            solution,
            commitment,
        }
    }

    /// Verify this reveal matches the commitment
    pub fn verify(&self, epoch_salt: &Hash) -> bool {
        // 1. Verify commitment matches problem + solution
        if !self.commitment.verify(&self.problem, &self.solution, epoch_salt) {
            return false;
        }

        // 2. Verify solution is correct
        self.solution.verify(&self.problem)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_commitment_protocol() {
        // Create a subset sum problem
        let problem = ProblemType::SubsetSum {
            numbers: vec![1, 2, 3, 4, 5],
            target: 9,
        };

        // Solve it
        let solution = Solution::SubsetSum(vec![1, 2, 3]); // 2 + 3 + 4 = 9

        // Create epoch salt (derived from parent block hash)
        let epoch_salt = Hash::new(b"parent_block_hash");

        // Miner creates commitment
        let commitment = Commitment::create(&problem, &solution, &epoch_salt);

        // Verify commitment
        assert!(commitment.verify(&problem, &solution, &epoch_salt));

        // Wrong solution shouldn't verify
        let wrong_solution = Solution::SubsetSum(vec![0, 1]);
        assert!(!commitment.verify(&problem, &wrong_solution, &epoch_salt));
    }

    #[test]
    fn test_solution_reveal() {
        let problem = ProblemType::SubsetSum {
            numbers: vec![5, 10, 15, 20],
            target: 25,
        };

        let solution = Solution::SubsetSum(vec![0, 2]); // 5 + 15 = 20... wait that's wrong
        let solution = Solution::SubsetSum(vec![0, 3]); // 5 + 20 = 25

        let epoch_salt = Hash::new(b"epoch_salt");
        let commitment = Commitment::create(&problem, &solution, &epoch_salt);

        let reveal = SolutionReveal::new(problem, solution, commitment);
        assert!(reveal.verify(&epoch_salt));
    }

    #[test]
    fn test_json_commitment_compatibility() {
        // Test that JSON-based commitments can be verified
        let problem = ProblemType::SubsetSum {
            numbers: vec![1, 2, 3, 4, 5],
            target: 9,
        };

        let solution = Solution::SubsetSum(vec![1, 2, 3]); // 2 + 3 + 4 = 9

        let epoch_salt = Hash::new(b"parent_block_hash");

        // Create commitment using JSON serialization (client-side)
        let commitment_json = Commitment::create_from_json(&problem, &solution, &epoch_salt);

        // Verify it can be verified (should work with both methods)
        assert!(commitment_json.verify(&problem, &solution, &epoch_salt));

        // Create commitment using bincode serialization (server-side)
        let commitment_bincode = Commitment::create(&problem, &solution, &epoch_salt);

        // They should be different (different serialization formats)
        assert_ne!(commitment_json.hash, commitment_bincode.hash);
        assert_ne!(commitment_json.problem_hash, commitment_bincode.problem_hash);

        // But both should verify correctly
        assert!(commitment_json.verify(&problem, &solution, &epoch_salt));
        assert!(commitment_bincode.verify(&problem, &solution, &epoch_salt));
    }
}
