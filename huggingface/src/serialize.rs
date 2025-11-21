// Problem and Solution Serialization
// Converts ProblemType and Solution to JSON format for Hugging Face Dataset

use coinject_core::{ProblemType, Solution, SubmissionMode, ProblemReveal};
use serde_json::{json, Value};
use base64::{Engine as _, engine::general_purpose::STANDARD};

/// Serialize problem to JSON with schema-normalized structure
///
/// Uses discriminated union to maintain type consistency across heterogeneous problem types.
/// Enables efficient filtering, querying, and schema validation in downstream pipelines.
pub fn serialize_problem(problem: &ProblemType) -> Result<Value, SerializationError> {
    match problem {
        ProblemType::SubsetSum { numbers, target } => {
            Ok(json!({
                "type": "SubsetSum",
                "subset_sum": {
                    "numbers": numbers,
                    "target": target
                },
                "sat": null,
                "tsp": null,
                "custom": null
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
                "type": "SAT",
                "subset_sum": null,
                "sat": {
                    "variables": variables,
                    "clauses": clauses_json
                },
                "tsp": null,
                "custom": null
            }))
        }
        ProblemType::TSP { cities, distances } => {
            Ok(json!({
                "type": "TSP",
                "subset_sum": null,
                "sat": null,
                "tsp": {
                    "cities": cities,
                    "distances": distances
                },
                "custom": null
            }))
        }
        ProblemType::Custom { problem_id, data } => {
            let data_b64 = STANDARD.encode(data);
            Ok(json!({
                "type": "Custom",
                "subset_sum": null,
                "sat": null,
                "tsp": null,
                "custom": {
                    "problem_id": hex::encode(problem_id.as_bytes()),
                    "data": data_b64
                }
            }))
        }
    }
}

/// Serialize solution to JSON with schema-normalized structure
///
/// Maintains consistent column types across all records by using a discriminated union.
/// This ensures HuggingFace dataset viewer can infer types correctly and enables
/// efficient querying, ML training, and forensic analytics.
pub fn serialize_solution(solution: &Solution) -> Result<Value, SerializationError> {
    match solution {
        Solution::SubsetSum(indices) => {
            Ok(json!({
                "type": "SubsetSum",
                "indices": indices,
                "assignments": null,
                "tour": null,
                "custom": null
            }))
        }
        Solution::SAT(assignments) => {
            Ok(json!({
                "type": "SAT",
                "indices": null,
                "assignments": assignments,
                "tour": null,
                "custom": null
            }))
        }
        Solution::TSP(tour) => {
            Ok(json!({
                "type": "TSP",
                "indices": null,
                "assignments": null,
                "tour": tour,
                "custom": null
            }))
        }
        Solution::Custom(data) => {
            let data_b64 = STANDARD.encode(data);
            Ok(json!({
                "type": "Custom",
                "indices": null,
                "assignments": null,
                "tour": null,
                "custom": data_b64
            }))
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

