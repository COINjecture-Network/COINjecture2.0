// Problem and Solution Serialization
// Converts ProblemType and Solution to JSON format for Hugging Face Dataset

use coinject_core::{ProblemType, Solution, SubmissionMode, ProblemReveal};
use serde_json::{json, Value};
use base64::{Engine as _, engine::general_purpose::STANDARD};

/// Serialize problem to JSON
pub fn serialize_problem(problem: &ProblemType) -> Result<Value, SerializationError> {
    match problem {
        ProblemType::SubsetSum { numbers, target } => {
            Ok(json!({
                "numbers": numbers,
                "target": target
            }))
        }
        ProblemType::SAT { variables, clauses } => {
            let clauses_json: Vec<Value> = clauses
                .iter()
                .map(|clause| {
                    json!({
                        "literals": clause.literals
                    })
                })
                .collect();

            Ok(json!({
                "variables": variables,
                "clauses": clauses_json
            }))
        }
        ProblemType::TSP { cities, distances } => {
            Ok(json!({
                "cities": cities,
                "distances": distances
            }))
        }
        ProblemType::Custom { problem_id, data } => {
            let data_b64 = STANDARD.encode(data);
            Ok(json!({
                "problem_id": hex::encode(problem_id.as_bytes()),
                "data": data_b64
            }))
        }
    }
}

/// Serialize solution to JSON
pub fn serialize_solution(solution: &Solution) -> Result<Value, SerializationError> {
    match solution {
        Solution::SubsetSum(indices) => {
            Ok(json!(indices))
        }
        Solution::SAT(assignments) => {
            Ok(json!(assignments))
        }
        Solution::TSP(tour) => {
            Ok(json!(tour))
        }
        Solution::Custom(data) => {
            let data_b64 = STANDARD.encode(data);
            Ok(json!(data_b64))
        }
    }
}

/// Extract problem from submission mode (handles private reveals)
pub fn extract_problem_from_submission(
    submission_mode: &SubmissionMode,
    reveal: Option<&ProblemReveal>,
) -> Result<Option<ProblemType>, SerializationError> {
    match submission_mode {
        SubmissionMode::Public { problem } => {
            Ok(Some(problem.clone()))
        }
        SubmissionMode::Private { .. } => {
            // For private problems, only include if revealed
            if let Some(reveal) = reveal {
                Ok(Some(reveal.problem.clone()))
            } else {
                Ok(None)
            }
        }
    }
}

/// Serialization errors
#[derive(Debug, thiserror::Error)]
pub enum SerializationError {
    #[error("Serialization failed: {0}")]
    Failed(String),
}

