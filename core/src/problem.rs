use crate::Hash;
use serde::{Deserialize, Serialize};

/// NP-hard problem types supported by the network
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ProblemType {
    /// Subset Sum: Given a set of integers, find subset that sums to target
    SubsetSum { numbers: Vec<i64>, target: i64 },
    /// SAT: Boolean satisfiability problem
    SAT {
        variables: usize,
        clauses: Vec<Clause>,
    },
    /// Traveling Salesman Problem
    TSP {
        cities: usize,
        distances: Vec<Vec<u64>>,
    },
    /// User-submitted custom problem
    Custom { problem_id: Hash, data: Vec<u8> },
}

/// SAT clause (disjunction of literals)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Clause {
    pub literals: Vec<i32>, // Positive = variable, negative = negated variable
}

/// Total length of a TSP tour (returns to city 0).
fn tsp_tour_length(tour: &[usize], cities: usize, distances: &[Vec<u64>]) -> u64 {
    (0..cities)
        .map(|i| {
            let from = tour[i];
            let to = tour[(i + 1) % cities];
            distances[from][to]
        })
        .sum()
}

/// Nearest-neighbor baseline tour length (for normalizing optimization quality).
fn tsp_nearest_neighbor_length(cities: usize, distances: &[Vec<u64>]) -> u64 {
    let mut tour = Vec::with_capacity(cities);
    let mut visited = vec![false; cities];
    tour.push(0);
    visited[0] = true;

    while tour.len() < cities {
        let current = *tour.last().expect("tour non-empty");
        let mut best_next = None;
        let mut best_dist = u64::MAX;
        for next in 0..cities {
            if !visited[next] {
                let dist = distances[current][next];
                if dist < best_dist {
                    best_dist = dist;
                    best_next = Some(next);
                }
            }
        }
        if let Some(next) = best_next {
            tour.push(next);
            visited[next] = true;
        } else {
            break;
        }
    }
    tsp_tour_length(&tour, cities, distances)
}

/// Solution to an NP-hard problem
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Solution {
    /// Subset Sum solution (indices of selected numbers)
    SubsetSum(Vec<usize>),
    /// SAT solution (variable assignments: true/false)
    SAT(Vec<bool>),
    /// TSP solution (city visiting order)
    TSP(Vec<usize>),
    /// Custom solution
    Custom(Vec<u8>),
}

impl Solution {
    /// Fast verification (polynomial time)
    pub fn verify(&self, problem: &ProblemType) -> bool {
        match (self, problem) {
            (Solution::SubsetSum(indices), ProblemType::SubsetSum { numbers, target }) => {
                let sum: i64 = indices.iter().map(|&i| numbers.get(i).unwrap_or(&0)).sum();
                sum == *target
            }
            (Solution::SAT(assignment), ProblemType::SAT { variables, clauses }) => {
                if assignment.len() != *variables {
                    return false;
                }
                // Check if all clauses are satisfied
                clauses.iter().all(|clause| {
                    clause.literals.iter().any(|&lit| {
                        let var_idx = lit.unsigned_abs() as usize - 1;
                        let value = assignment.get(var_idx).unwrap_or(&false);
                        if lit > 0 {
                            *value
                        } else {
                            !*value
                        }
                    })
                })
            }
            (
                Solution::TSP(tour),
                ProblemType::TSP {
                    cities,
                    distances: _,
                },
            ) => {
                // Verify tour visits all cities exactly once
                tour.len() == *cities && {
                    let mut visited = vec![false; *cities];
                    tour.iter().all(|&city| {
                        if city >= *cities || visited[city] {
                            false
                        } else {
                            visited[city] = true;
                            true
                        }
                    })
                }
            }
            _ => false, // Type mismatch or custom
        }
    }

    /// Calculate solution quality (0.0 to 1.0)
    pub fn quality(&self, problem: &ProblemType) -> f64 {
        match (self, problem) {
            (Solution::SubsetSum(_), ProblemType::SubsetSum { .. }) if self.verify(problem) => {
                // Exact solution gets 1.0
                1.0
            }
            (Solution::SubsetSum(_), ProblemType::SubsetSum { .. }) => 0.0,
            (Solution::TSP(tour), ProblemType::TSP { cities, distances }) => {
                if !self.verify(problem) {
                    return 0.0;
                }
                let length = tsp_tour_length(tour, *cities, distances);
                let baseline = tsp_nearest_neighbor_length(*cities, distances).max(1);
                // Quality in [0, 1]: NN tour = 1.0; longer tours scale down (not 1/(length+1),
                // which zeroed w_trunc for small instances with large distance matrices).
                (baseline as f64 / length.max(baseline) as f64).clamp(0.0, 1.0)
            }
            (Solution::SAT(_), ProblemType::SAT { .. }) if self.verify(problem) => {
                // Exact solution gets 1.0
                1.0
            }
            (Solution::SAT(_), ProblemType::SAT { .. }) => 0.0,
            _ => 0.0,
        }
    }
}

/// Problem difficulty weight (affects work score)
impl ProblemType {
    pub fn difficulty_weight(&self) -> f64 {
        match self {
            ProblemType::SubsetSum { numbers, .. } => {
                // Weight based on number count and magnitude
                (numbers.len() as f64).log2()
            }
            ProblemType::SAT { variables, clauses } => {
                // Weight based on variables and clauses
                (*variables as f64) * (clauses.len() as f64).log2()
            }
            ProblemType::TSP { cities, .. } => {
                // Weight based on city count (factorial complexity)
                (*cities as f64).powi(2)
            }
            ProblemType::Custom { .. } => 1.0,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap_or_default()
    }

    pub fn hash(&self) -> Hash {
        Hash::new(&self.serialize())
    }
}
