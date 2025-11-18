// Comprehensive Integration Tests for Privacy-Preserving Marketplace
// Tests the full lifecycle of private bounty submissions

use coinject_core::{
    Address, Balance, Hash, ProblemType, Solution,
    SubmissionMode, ProblemReveal, WellformednessProof, ProblemParameters,
};
use coinject_state::{MarketplaceState, ProblemStatus};
use std::sync::Arc;
use tempfile::TempDir;

/// Helper to create test marketplace state
fn create_test_marketplace() -> (MarketplaceState, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db = Arc::new(redb::Database::create(temp_dir.path().join("test.db")).unwrap());
    let marketplace = MarketplaceState::from_db(db).unwrap();
    (marketplace, temp_dir)
}

/// Helper to create a test problem and ZK proof
fn create_test_private_problem() -> (ProblemType, [u8; 32], WellformednessProof, Hash) {
    let problem = ProblemType::SubsetSum {
        numbers: vec![10, 20, 30, 40, 50],
        target: 60,
    };

    let salt = [42u8; 32];

    let public_params = ProblemParameters {
        problem_type: "SubsetSum".to_string(),
        size: 5,
        complexity_estimate: 15.0,
    };

    let (proof, commitment) = WellformednessProof::create(&problem, &salt, &public_params)
        .expect("Failed to create proof");

    (problem, salt, proof, commitment)
}

#[test]
fn test_private_bounty_full_lifecycle() {
    let (marketplace, _temp_dir) = create_test_marketplace();

    // 1. Create private problem submission
    let (problem, salt, proof, commitment) = create_test_private_problem();

    let submitter = Address::from_bytes([1u8; 32]);
    let bounty = 1000;
    let min_work_score = 10.0;

    let public_params = ProblemParameters {
        problem_type: "SubsetSum".to_string(),
        size: 5,
        complexity_estimate: 15.0,
    };

    let submission_mode = SubmissionMode::Private {
        problem_commitment: commitment,
        zk_wellformed_proof: proof,
        public_params,
    };

    // 2. Submit private bounty
    let problem_id = marketplace
        .submit_problem(submission_mode, submitter, bounty, min_work_score, 7)
        .expect("Failed to submit private problem");

    // 3. Verify problem is stored with private status
    let stored_problem = marketplace
        .get_problem(&problem_id)
        .expect("Failed to get problem")
        .expect("Problem not found");

    assert_eq!(stored_problem.problem_id, problem_id);
    assert_eq!(stored_problem.bounty, bounty);
    assert_eq!(stored_problem.status, ProblemStatus::Open);
    assert!(stored_problem.problem_reveal.is_none()); // Not revealed yet
    assert!(matches!(stored_problem.submission_mode, SubmissionMode::Private { .. }));

    // 4. Attempt to submit solution before reveal (should fail)
    let solver = Address::from_bytes([2u8; 32]);
    let solution = Solution::SubsetSum(vec![1, 3]); // Indices: numbers[1]=20 + numbers[3]=40 = 60

    let result = marketplace.submit_solution(problem_id, solver, solution.clone());
    assert!(result.is_err()); // Should fail because problem not revealed

    // 5. Reveal the problem
    let reveal = ProblemReveal::new(problem.clone(), salt);

    marketplace
        .reveal_problem(problem_id, reveal)
        .expect("Failed to reveal problem");

    // 6. Verify problem is now revealed
    let revealed_problem = marketplace
        .get_problem(&problem_id)
        .expect("Failed to get problem")
        .expect("Problem not found");

    assert!(revealed_problem.problem_reveal.is_some());
    let revealed = revealed_problem.problem_reveal.as_ref().unwrap();
    assert_eq!(revealed.problem, problem);

    // 7. Submit solution after reveal (should succeed)
    marketplace
        .submit_solution(problem_id, solver, solution)
        .expect("Failed to submit solution");

    // 8. Verify problem is solved
    let solved_problem = marketplace
        .get_problem(&problem_id)
        .expect("Failed to get problem")
        .expect("Problem not found");

    assert_eq!(solved_problem.status, ProblemStatus::Solved);
    assert_eq!(solved_problem.solver, Some(solver));

    // 9. Claim bounty
    let (claimed_solver, claimed_bounty) = marketplace
        .claim_bounty(problem_id)
        .expect("Failed to claim bounty");

    assert_eq!(claimed_solver, solver);
    assert_eq!(claimed_bounty, bounty);
}

#[test]
fn test_private_bounty_duplicate_rejection() {
    let (marketplace, _temp_dir) = create_test_marketplace();

    let (problem, _salt, proof, commitment) = create_test_private_problem();

    let submitter = Address::from_bytes([1u8; 32]);

    let public_params = ProblemParameters {
        problem_type: "SubsetSum".to_string(),
        size: 5,
        complexity_estimate: 15.0,
    };

    let submission_mode = SubmissionMode::Private {
        problem_commitment: commitment,
        zk_wellformed_proof: proof.clone(),
        public_params: public_params.clone(),
    };

    // Submit first time - should succeed
    let result1 = marketplace.submit_problem(submission_mode.clone(), submitter, 1000, 10.0, 7);
    assert!(result1.is_ok());

    // Submit again with same commitment - should fail
    let result2 = marketplace.submit_problem(submission_mode, submitter, 1000, 10.0, 7);
    assert!(result2.is_err());
}

#[test]
fn test_private_bounty_invalid_reveal() {
    let (marketplace, _temp_dir) = create_test_marketplace();

    let (problem, salt, proof, commitment) = create_test_private_problem();

    let submitter = Address::from_bytes([1u8; 32]);

    let public_params = ProblemParameters {
        problem_type: "SubsetSum".to_string(),
        size: 5,
        complexity_estimate: 15.0,
    };

    let submission_mode = SubmissionMode::Private {
        problem_commitment: commitment,
        zk_wellformed_proof: proof,
        public_params,
    };

    let problem_id = marketplace
        .submit_problem(submission_mode, submitter, 1000, 10.0, 7)
        .expect("Failed to submit problem");

    // Try to reveal with wrong problem (different target)
    let wrong_problem = ProblemType::SubsetSum {
        numbers: vec![10, 20, 30, 40, 50],
        target: 100, // Wrong target
    };

    let wrong_reveal = ProblemReveal::new(wrong_problem, salt);

    let result = marketplace.reveal_problem(problem_id, wrong_reveal);
    assert!(result.is_err()); // Should fail - reveal doesn't match commitment
}

#[test]
fn test_public_vs_private_modes() {
    let (marketplace, _temp_dir) = create_test_marketplace();

    let submitter = Address::from_bytes([1u8; 32]);

    // Create public problem
    let public_problem = ProblemType::SubsetSum {
        numbers: vec![1, 2, 3, 4, 5],
        target: 9,
    };

    let public_id = marketplace
        .submit_public_problem(public_problem.clone(), submitter, 1000, 10.0, 7)
        .expect("Failed to submit public problem");

    // Create private problem
    let (_problem, _salt, proof, commitment) = create_test_private_problem();

    let public_params = ProblemParameters {
        problem_type: "SubsetSum".to_string(),
        size: 5,
        complexity_estimate: 15.0,
    };

    let private_mode = SubmissionMode::Private {
        problem_commitment: commitment,
        zk_wellformed_proof: proof,
        public_params,
    };

    let private_id = marketplace
        .submit_problem(private_mode, submitter, 2000, 15.0, 7)
        .expect("Failed to submit private problem");

    // Verify public problem is immediately solvable
    let public_submission = marketplace
        .get_problem(&public_id)
        .unwrap()
        .unwrap();

    assert!(matches!(public_submission.submission_mode, SubmissionMode::Public { .. }));

    // Verify private problem requires reveal
    let private_submission = marketplace
        .get_problem(&private_id)
        .unwrap()
        .unwrap();

    assert!(matches!(private_submission.submission_mode, SubmissionMode::Private { .. }));
    assert!(private_submission.problem_reveal.is_none());
}

#[test]
fn test_reveal_public_problem_fails() {
    let (marketplace, _temp_dir) = create_test_marketplace();

    let submitter = Address::from_bytes([1u8; 32]);

    // Submit public problem
    let problem = ProblemType::SubsetSum {
        numbers: vec![1, 2, 3],
        target: 6,
    };

    let problem_id = marketplace
        .submit_public_problem(problem.clone(), submitter, 1000, 10.0, 7)
        .expect("Failed to submit public problem");

    // Try to reveal a public problem (should fail)
    let salt = [99u8; 32];
    let reveal = ProblemReveal::new(problem, salt);

    let result = marketplace.reveal_problem(problem_id, reveal);
    assert!(result.is_err()); // Should fail - not a private submission
}

#[test]
fn test_marketplace_stats_with_privacy() {
    let (marketplace, _temp_dir) = create_test_marketplace();

    let submitter = Address::from_bytes([1u8; 32]);

    // Submit public problem
    let public_problem = ProblemType::SubsetSum {
        numbers: vec![1, 2, 3],
        target: 6,
    };

    marketplace
        .submit_public_problem(public_problem, submitter, 1000, 10.0, 7)
        .expect("Failed to submit public problem");

    // Submit private problem
    let (_problem, _salt, proof, commitment) = create_test_private_problem();

    let public_params = ProblemParameters {
        problem_type: "SubsetSum".to_string(),
        size: 5,
        complexity_estimate: 15.0,
    };

    let private_mode = SubmissionMode::Private {
        problem_commitment: commitment,
        zk_wellformed_proof: proof,
        public_params,
    };

    marketplace
        .submit_problem(private_mode, submitter, 2000, 15.0, 7)
        .expect("Failed to submit private problem");

    // Check stats
    let stats = marketplace.get_stats().expect("Failed to get stats");

    assert_eq!(stats.total_problems, 2);
    assert_eq!(stats.open_problems, 2);
    assert_eq!(stats.total_bounty_pool, 3000);
}

#[test]
fn test_commitment_determinism() {
    // Verify that same problem + salt produces same commitment
    let problem = ProblemType::SubsetSum {
        numbers: vec![7, 14, 21],
        target: 21,
    };

    let salt = [123u8; 32];

    let public_params = ProblemParameters {
        problem_type: "SubsetSum".to_string(),
        size: 3,
        complexity_estimate: 5.0,
    };

    let (proof1, commitment1) = WellformednessProof::create(&problem, &salt, &public_params)
        .expect("Failed to create proof 1");

    let (proof2, commitment2) = WellformednessProof::create(&problem, &salt, &public_params)
        .expect("Failed to create proof 2");

    assert_eq!(commitment1, commitment2);
}

#[test]
fn test_different_problems_different_commitments() {
    let salt = [123u8; 32];

    let problem1 = ProblemType::SubsetSum {
        numbers: vec![1, 2, 3],
        target: 6,
    };

    let problem2 = ProblemType::SubsetSum {
        numbers: vec![1, 2, 3],
        target: 5, // Different target
    };

    let public_params1 = ProblemParameters {
        problem_type: "SubsetSum".to_string(),
        size: 3,
        complexity_estimate: 5.0,
    };

    let public_params2 = ProblemParameters {
        problem_type: "SubsetSum".to_string(),
        size: 3,
        complexity_estimate: 5.0,
    };

    let (_proof1, commitment1) = WellformednessProof::create(&problem1, &salt, &public_params1)
        .expect("Failed to create proof 1");

    let (_proof2, commitment2) = WellformednessProof::create(&problem2, &salt, &public_params2)
        .expect("Failed to create proof 2");

    assert_ne!(commitment1, commitment2); // Different problems = different commitments
}
