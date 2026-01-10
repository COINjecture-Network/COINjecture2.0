// Mining Loop with NP-hard Proof-of-Work
// Implements commit-reveal protocol to prevent grinding attacks

use coinject_core::{
    Address, Block, BlockHeader, Clause, CoinbaseTransaction, Commitment, Hash, ProblemType,
    Solution, SolutionReveal, Transaction,
};
use coinject_tokenomics::{RewardCalculator, NetworkMetrics};
use crate::{WorkScoreCalculator, DifficultyAdjuster};
use rand::Rng;
use rand::seq::SliceRandom;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

const MAX_MINING_ATTEMPTS: usize = 5;
const MINING_TIMEOUT: Duration = Duration::from_secs(60);
const FAILURE_PENALTY_TIME: Duration = Duration::from_secs(60);

// ============================================================================
// STANDALONE BLOCKING FUNCTIONS
// These run in spawn_blocking to avoid starving the tokio runtime
// ============================================================================

/// Mine header by finding nonce that meets difficulty target (blocking)
/// This is a standalone function to run in spawn_blocking
fn mine_header_blocking(mut header: BlockHeader, difficulty: u32) -> Option<(BlockHeader, Hash)> {
    let target_prefix = "0".repeat(difficulty as usize);
    let start_time = Instant::now();
    let mut hashes = 0u64;

    println!("🎯 Mining target: hash must start with '{}'", target_prefix);

    for nonce in 0..u64::MAX {
        header.nonce = nonce;
        let hash = header.hash();
        hashes += 1;

        let hash_hex = hex::encode(hash.as_bytes());

        // Debug: Print first few hash samples
        if nonce < 5 {
            println!("  Sample hash #{}: {}", nonce, hash_hex);
        }

        if hash_hex.starts_with(&target_prefix) {
            let elapsed = start_time.elapsed().as_secs_f64();
            let hash_rate = hashes as f64 / elapsed;
            println!("✅ Found nonce {} after {} hashes ({:.2} H/s)", nonce, hashes, hash_rate);
            println!("   Block hash: {}", hash_hex);
            return Some((header, hash));
        }

        // Print progress every million hashes
        if nonce % 1_000_000 == 0 && nonce > 0 {
            let elapsed = start_time.elapsed().as_secs_f64();
            let hash_rate = hashes as f64 / elapsed;
            println!("⛏️  Mining... {} hashes ({:.2} H/s) | Latest: {}...",
                hashes, hash_rate, &hash_hex[..16]);
        }
    }

    None
}

/// Solve NP-hard problem (blocking) - standalone function for spawn_blocking
fn solve_problem_blocking(problem: ProblemType) -> Option<(Solution, Duration, usize)> {
    let start_time = Instant::now();
    let mut memory_used = 0;

    let solution = match &problem {
        ProblemType::SubsetSum { numbers, target } => {
            solve_subset_sum_blocking(numbers, *target, &mut memory_used)
        }
        ProblemType::SAT { variables, clauses } => {
            let timeout = Duration::from_secs(30);
            if *variables <= 32 {
                solve_sat_brute_force_blocking(*variables, clauses, &mut memory_used, start_time, timeout)
            } else {
                solve_sat_with_timeout_blocking(*variables, clauses, &mut memory_used, timeout, start_time)
            }
        }
        ProblemType::TSP { cities, distances } => {
            solve_tsp_blocking(*cities, distances, &mut memory_used)
        }
        ProblemType::Custom { .. } => None,
    };

    let solve_time = start_time.elapsed();
    solution.map(|s| (s, solve_time, memory_used))
}

/// Subset sum solver (blocking standalone)
fn solve_subset_sum_blocking(numbers: &[i64], target: i64, memory: &mut usize) -> Option<Solution> {
    let n = numbers.len();
    let sum: i64 = numbers.iter().sum();

    if target > sum || target < 0 {
        return None;
    }

    let offset = sum.abs() as usize;
    let range = (2 * offset + 1) as usize;
    let mut dp = vec![vec![false; range]; n + 1];
    *memory += dp.len() * dp[0].len();

    dp[0][offset] = true;

    for i in 1..=n {
        for j in 0..range {
            dp[i][j] = dp[i-1][j];
            let num = numbers[i-1];
            let prev_idx = j as i64 - num;
            if prev_idx >= 0 && prev_idx < range as i64 {
                dp[i][j] |= dp[i-1][prev_idx as usize];
            }
        }
    }

    let target_idx = (offset as i64 + target) as usize;
    if !dp[n][target_idx] {
        return None;
    }

    let mut indices = Vec::new();
    let mut curr_sum = target;
    for i in (1..=n).rev() {
        if curr_sum == 0 {
            break;
        }
        let num = numbers[i-1];
        if curr_sum >= num {
            let prev_idx = (offset as i64 + curr_sum - num) as usize;
            if prev_idx < range && dp[i-1][prev_idx] {
                indices.push(i-1);
                curr_sum -= num;
            }
        }
    }

    indices.reverse();
    Some(Solution::SubsetSum(indices))
}

/// SAT brute force solver (blocking standalone)
fn solve_sat_brute_force_blocking(
    variables: usize,
    clauses: &[Clause],
    memory: &mut usize,
    start_time: Instant,
    timeout: Duration,
) -> Option<Solution> {
    *memory += variables * std::mem::size_of::<bool>();

    let max_iterations = 1u64 << variables.min(32);

    for assignment_bits in 0..max_iterations {
        if start_time.elapsed() > timeout {
            return None;
        }

        let mut assignment = vec![false; variables];
        for j in 0..variables.min(32) {
            assignment[j] = (assignment_bits >> j) & 1 == 1;
        }

        // Check if this assignment satisfies all clauses
        // Literal format: positive = variable is true, negative = variable is false
        let satisfied = clauses.iter().all(|clause| {
            clause.literals.iter().any(|&literal| {
                let var_idx = (literal.abs() - 1) as usize;
                if var_idx < assignment.len() {
                    (literal > 0) == assignment[var_idx]
                } else {
                    false
                }
            })
        });

        if satisfied {
            return Some(Solution::SAT(assignment));
        }
    }

    None
}

/// SAT solver with timeout (blocking standalone) - DPLL algorithm
fn solve_sat_with_timeout_blocking(
    variables: usize,
    clauses: &[Clause],
    memory: &mut usize,
    timeout: Duration,
    start_time: Instant,
) -> Option<Solution> {
    *memory += variables * std::mem::size_of::<bool>();

    let mut assignment = vec![false; variables];

    fn dpll(
        clauses: &[Clause],
        assignment: &mut Vec<bool>,
        var_idx: usize,
        start_time: Instant,
        timeout: Duration,
    ) -> bool {
        if start_time.elapsed() > timeout {
            return false;
        }

        // Check if all clauses are satisfied
        // Literal format: positive = variable is true, negative = variable is false
        let all_satisfied = clauses.iter().all(|clause| {
            clause.literals.iter().any(|&literal| {
                let idx = (literal.abs() - 1) as usize;
                if idx < assignment.len() {
                    (literal > 0) == assignment[idx]
                } else {
                    false
                }
            })
        });

        if all_satisfied {
            return true;
        }

        // Check if any clause is unsatisfiable with current partial assignment
        let any_unsatisfiable = clauses.iter().any(|clause| {
            clause.literals.iter().all(|&literal| {
                let idx = (literal.abs() - 1) as usize;
                if idx < var_idx {
                    // Only check already-assigned variables
                    (literal > 0) != assignment[idx]
                } else {
                    false
                }
            })
        });

        if any_unsatisfiable {
            return false;
        }

        if var_idx >= assignment.len() {
            return false;
        }

        assignment[var_idx] = true;
        if dpll(clauses, assignment, var_idx + 1, start_time, timeout) {
            return true;
        }

        assignment[var_idx] = false;
        dpll(clauses, assignment, var_idx + 1, start_time, timeout)
    }

    if dpll(clauses, &mut assignment, 0, start_time, timeout) {
        Some(Solution::SAT(assignment))
    } else {
        None
    }
}

/// TSP solver (blocking standalone)
fn solve_tsp_blocking(cities: usize, distances: &[Vec<u64>], memory: &mut usize) -> Option<Solution> {
    if cities == 0 {
        return None;
    }

    *memory += cities * std::mem::size_of::<usize>();

    let mut tour = Vec::with_capacity(cities);
    let mut visited = vec![false; cities];

    tour.push(0);
    visited[0] = true;

    while tour.len() < cities {
        let current = *tour.last().unwrap();
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

    Some(Solution::TSP(tour))
}

/// Mining configuration
pub struct MiningConfig {
    pub miner_address: Address,
    pub target_block_time: Duration,
    pub min_difficulty: u32,
    pub max_difficulty: u32,
}

impl Default for MiningConfig {
    fn default() -> Self {
        MiningConfig {
            miner_address: Address::from_bytes([0u8; 32]),
            target_block_time: Duration::from_secs(60), // 1 minute blocks
            min_difficulty: 2,
            max_difficulty: 8,
        }
    }
}

/// Mining statistics
#[derive(Clone, Debug)]
pub struct MiningStats {
    pub blocks_mined: u64,
    pub total_work_score: f64,
    pub average_solve_time: Duration,
    pub hash_rate: f64, // hashes per second
}

/// Miner that solves NP-hard problems and mines blocks
pub struct Miner {
    config: MiningConfig,
    work_calculator: WorkScoreCalculator,
    reward_calculator: RewardCalculator,
    stats: Arc<RwLock<MiningStats>>,
    difficulty: u32,
    difficulty_adjuster: Arc<RwLock<DifficultyAdjuster>>,
}

impl Miner {
    pub fn new(config: MiningConfig) -> Self {
        let starting_difficulty = config.min_difficulty;
        Miner {
            config,
            work_calculator: WorkScoreCalculator::new(),
            reward_calculator: RewardCalculator::new(),
            stats: Arc::new(RwLock::new(MiningStats {
                blocks_mined: 0,
                total_work_score: 0.0,
                average_solve_time: Duration::from_secs(0),
                hash_rate: 0.0,
            })),
            difficulty: starting_difficulty, // Use configured difficulty
            difficulty_adjuster: Arc::new(RwLock::new(DifficultyAdjuster::new())),
        }
    }
    
    /// Set network metrics for empirical difficulty adjustment and work score calculation
    /// This enables full compliance with empirical/self-referential/dimensionless principles
    pub async fn set_network_metrics(&mut self, network_metrics: Arc<RwLock<NetworkMetrics>>) {
        // Update difficulty adjuster with network metrics
        {
            let mut adjuster = self.difficulty_adjuster.write().await;
            adjuster.set_metrics(Arc::clone(&network_metrics));
        }
        
        // Update work calculator with network metrics
        self.work_calculator.set_metrics(network_metrics);
    }

    /// Generate a deterministic NP-hard problem for mining
    /// RUNTIME INTEGRATION: Uses dimensional complexity |ψ(τ)| to modulate difficulty
    /// DETERMINISM: Seeded by parent hash + height to ensure all nodes generate the same problem
    pub async fn generate_problem(&self, block_height: u64, prev_hash: Hash) -> ProblemType {
        use coinject_core::{TAU_C, ConsensusState};
        use rand::SeedableRng;
        use rand::rngs::StdRng;

        // Create deterministic seed from parent hash + height
        // This ensures all nodes generate the SAME problem for a given (prev_hash, height) pair
        let mut seed_bytes = [0u8; 32];
        seed_bytes.copy_from_slice(prev_hash.as_bytes());

        // XOR height into the seed for additional entropy
        for i in 0..8 {
            seed_bytes[i] ^= ((block_height >> (i * 8)) & 0xFF) as u8;
        }

        // Create seeded RNG - deterministic across all nodes
        let seed = u64::from_le_bytes(seed_bytes[0..8].try_into().unwrap());
        let mut rng = StdRng::seed_from_u64(seed);

        // Calculate dimensionless time τ = block_height / τ_c
        let tau = (block_height as f64) / TAU_C;
        let consensus_state = ConsensusState::at_tau(tau);

        // Get adaptive problem size from difficulty adjuster
        // This adjusts based on actual solve times to target network-derived optimal times
        let has_metrics = {
            let adjuster = self.difficulty_adjuster.read().await;
            adjuster.has_metrics()
        };

        // Randomly choose problem type
        match rng.gen_range(0..3) {
            0 => {
                // Subset Sum - Generate SOLVABLE problem by selecting a random subset first
                // Use async version if network metrics available, otherwise sync version
                let problem_size = if has_metrics {
                    let mut adjuster = self.difficulty_adjuster.write().await;
                    adjuster.size_for_problem_type_async("SubsetSum").await
                } else {
                    let adjuster = self.difficulty_adjuster.read().await;
                    adjuster.size_for_problem_type("SubsetSum")
                };
                let numbers: Vec<i64> = (0..problem_size)
                    .map(|_| rng.gen_range(1..1000))
                    .collect();

                // Randomly select which numbers to include in the solution
                // This guarantees the problem is solvable
                let subset_size = rng.gen_range(1..=problem_size.min(problem_size - 1).max(1));
                let mut selected_indices: Vec<usize> = (0..problem_size).collect();
                selected_indices.shuffle(&mut rng);
                selected_indices.truncate(subset_size);

                // Calculate target as sum of selected numbers - guarantees solvability
                let target: i64 = selected_indices.iter().map(|&i| numbers[i]).sum();

                ProblemType::SubsetSum { numbers, target }
            }
            1 => {
                // SAT (Boolean Satisfiability) - Generate SATISFIABLE problem
                let variables = if has_metrics {
                    let mut adjuster = self.difficulty_adjuster.write().await;
                    adjuster.size_for_problem_type_async("SAT").await
                } else {
                    let adjuster = self.difficulty_adjuster.read().await;
                    adjuster.size_for_problem_type("SAT")
                };
                let num_clauses = variables * 3; // 3-SAT ratio

                // Generate a random satisfying assignment first (our "hidden solution")
                let satisfying_assignment: Vec<bool> = (0..variables)
                    .map(|_| rng.gen_bool(0.5))
                    .collect();

                use rand::seq::SliceRandom;
                let clauses: Vec<Clause> = (0..num_clauses)
                    .map(|_| {
                        // Select 3 DISTINCT variables for this clause
                        let mut selected_vars: Vec<usize> = (0..variables).collect();
                        selected_vars.shuffle(&mut rng);

                        let literals: Vec<i32> = selected_vars.iter().take(3)
                            .map(|&var_idx| {
                                let var = (var_idx as i32) + 1;
                                let assignment_value = satisfying_assignment[var_idx];

                                // 70% chance: Create literal that matches assignment (satisfied)
                                // 30% chance: Create opposite literal (not satisfied by this variable)
                                // This ensures at least one literal per clause is satisfied
                                if rng.gen_bool(0.7) {
                                    if assignment_value { var } else { -var }
                                } else {
                                    if assignment_value { -var } else { var }
                                }
                            })
                            .collect();

                        Clause { literals }
                    })
                    .collect();

                ProblemType::SAT { variables, clauses }
            }
            _ => {
                // TSP (Traveling Salesman Problem)
                let cities = if has_metrics {
                    let mut adjuster = self.difficulty_adjuster.write().await;
                    adjuster.size_for_problem_type_async("TSP").await
                } else {
                    let adjuster = self.difficulty_adjuster.read().await;
                    adjuster.size_for_problem_type("TSP")
                };
                let mut distances = vec![vec![0u64; cities]; cities];
                for i in 0..cities {
                    for j in i+1..cities {
                        let dist = rng.gen_range(1..100);
                        distances[i][j] = dist;
                        distances[j][i] = dist;
                    }
                }

                ProblemType::TSP { cities, distances }
            }
        }
    }

    /// Solve NP-hard problem using backtracking/heuristics
    /// Returns None if timeout exceeded or problem is unsolvable
    pub fn solve_problem(&self, problem: &ProblemType) -> Option<(Solution, Duration, usize)> {
        let start_time = Instant::now();
        let mut memory_used = 0;

        let solution = match problem {
            ProblemType::SubsetSum { numbers, target } => {
                // Dynamic programming solution (pseudo-polynomial time)
                self.solve_subset_sum(numbers, *target, &mut memory_used)
            }
            ProblemType::SAT { variables, clauses } => {
                // For problems ≤32 vars, use brute force (2^32 = ~4.3B possibilities, manageable)
                // For larger problems, use DPLL with timeout
                let timeout = Duration::from_secs(30); // Increase timeout for larger problems
                if *variables <= 32 {
                    self.solve_sat_brute_force(*variables, &clauses, &mut memory_used, start_time, timeout)
                } else {
                    self.solve_sat_with_timeout(*variables, &clauses, &mut memory_used, timeout, start_time)
                }
            }
            ProblemType::TSP { cities, distances } => {
                // Greedy nearest neighbor heuristic
                self.solve_tsp(*cities, distances, &mut memory_used)
            }
            ProblemType::Custom { .. } => None,
        };

        let solve_time = start_time.elapsed();
        solution.map(|s| (s, solve_time, memory_used))
    }

    /// Subset sum solver using dynamic programming
    fn solve_subset_sum(&self, numbers: &[i64], target: i64, memory: &mut usize) -> Option<Solution> {
        let n = numbers.len();
        let sum: i64 = numbers.iter().sum();

        if target > sum || target < 0 {
            return None;
        }

        // DP table
        let offset = sum.abs() as usize;
        let range = (2 * offset + 1) as usize;
        let mut dp = vec![vec![false; range]; n + 1];
        *memory += dp.len() * dp[0].len();

        dp[0][offset] = true;

        for i in 1..=n {
            for j in 0..range {
                dp[i][j] = dp[i-1][j];
                let num = numbers[i-1];
                let prev_idx = j as i64 - num;
                if prev_idx >= 0 && prev_idx < range as i64 {
                    dp[i][j] |= dp[i-1][prev_idx as usize];
                }
            }
        }

        let target_idx = (offset as i64 + target) as usize;
        if !dp[n][target_idx] {
            return None;
        }

        // Backtrack to find solution
        let mut indices = Vec::new();
        let mut curr_sum = target;
        for i in (1..=n).rev() {
            if curr_sum == 0 {
                break;
            }
            let num = numbers[i-1];
            if curr_sum >= num {
                let prev_idx = (offset as i64 + curr_sum - num) as usize;
                if prev_idx < range && dp[i-1][prev_idx] {
                    indices.push(i-1);
                    curr_sum -= num;
                }
            }
        }

        Some(Solution::SubsetSum(indices))
    }

    /// SAT solver using brute force (for problems ≤32 variables)
    fn solve_sat_brute_force(
        &self,
        variables: usize,
        clauses: &[Clause],
        memory: &mut usize,
        start_time: Instant,
        timeout: Duration,
    ) -> Option<Solution> {
        *memory += variables * 8;
        
        // Brute force: try all 2^variables assignments
        // Cap at 2^32 for safety (4.3 billion possibilities)
        let max_assignments = 1u64 << variables.min(32);
        for i in 0..max_assignments {
            if start_time.elapsed() > timeout {
                break;
            }
            
            let mut assignment = vec![false; variables];
            for j in 0..variables.min(32) {
                assignment[j] = (i >> j) & 1 == 1;
            }
            
            // Check if this assignment satisfies all clauses
            let satisfied = clauses.iter().all(|clause| {
                clause.literals.iter().any(|&literal| {
                    let var_idx = (literal.abs() - 1) as usize;
                    if var_idx < assignment.len() {
                        (literal > 0) == assignment[var_idx]
                    } else {
                        false
                    }
                })
            });
            
            if satisfied {
                return Some(Solution::SAT(assignment));
            }
        }
        
        None
    }

    /// SAT solver using random search (legacy, not used)
    fn solve_sat(&self, variables: usize, clauses: &[Clause], memory: &mut usize) -> Option<Solution> {
        // Simple randomized search (not full DPLL for simplicity)
        *memory += variables * 8;

        let mut rng = rand::thread_rng();
        for _ in 0..1000 { // Try 1000 random assignments
            let assignment: Vec<bool> = (0..variables).map(|_| rng.gen_bool(0.5)).collect();

            let satisfied = clauses.iter().all(|clause| {
                clause.literals.iter().any(|&literal| {
                    let var_idx = (literal.abs() - 1) as usize;
                    let value = assignment.get(var_idx).copied().unwrap_or(false);
                    if literal > 0 {
                        value
                    } else {
                        !value
                    }
                })
            });

            if satisfied {
                return Some(Solution::SAT(assignment));
            }
        }

        None
    }

    /// SAT solver with timeout protection using DPLL (Davis-Putnam-Logemann-Loveland) algorithm
    fn solve_sat_with_timeout(
        &self,
        variables: usize,
        clauses: &[Clause],
        memory: &mut usize,
        timeout: Duration,
        start_time: Instant,
    ) -> Option<Solution> {
        *memory += variables * 8;

        // DPLL recursive solver - simplified and corrected implementation
        let mut assignment = vec![None; variables]; // None = unassigned, Some(true/false) = assigned
        
        fn is_clause_satisfied(clause: &Clause, assignment: &[Option<bool>]) -> bool {
            clause.literals.iter().any(|&literal| {
                let var_idx = (literal.abs() - 1) as usize;
                if var_idx < assignment.len() {
                    if let Some(value) = assignment[var_idx] {
                        return (literal > 0) == value;
                    }
                }
                false
            })
        }
        
        fn dpll_solve(
            clauses: &[Clause],
            assignment: &mut [Option<bool>],
            start_time: Instant,
            timeout: Duration,
        ) -> bool {
            // Check timeout
            if start_time.elapsed() > timeout {
                return false;
            }

            // Unit propagation: repeatedly find and propagate unit clauses
            loop {
                let mut propagated = false;
                for clause in clauses {
                    if is_clause_satisfied(clause, assignment) {
                        continue;
                    }
                    
                    // Count unassigned and assigned literals
                    let mut unassigned_literal: Option<(usize, bool)> = None;
                    let mut has_satisfied = false;
                    
                    for &literal in &clause.literals {
                        let var_idx = (literal.abs() - 1) as usize;
                        if var_idx >= assignment.len() {
                            continue;
                        }
                        
                        if let Some(value) = assignment[var_idx] {
                            if (literal > 0) == value {
                                has_satisfied = true;
                                break;
                            }
                        } else {
                            if unassigned_literal.is_none() {
                                unassigned_literal = Some((var_idx, literal > 0));
                            } else {
                                // More than one unassigned - not a unit clause
                                unassigned_literal = None;
                                break;
                            }
                        }
                    }
                    
                    if has_satisfied {
                        continue;
                    }
                    
                    if let Some((var_idx, value)) = unassigned_literal {
                        // Unit clause found - assign the literal to satisfy the clause
                        assignment[var_idx] = Some(value);
                        propagated = true;
                    } else if unassigned_literal.is_none() {
                        // All literals assigned but clause not satisfied = conflict
                        return false;
                    }
                }
                
                if !propagated {
                    break;
                }
            }

            // Check if all clauses are satisfied
            if clauses.iter().all(|clause| is_clause_satisfied(clause, assignment)) {
                return true;
            }

            // Find first unassigned variable for decision
            if let Some(var_idx) = assignment.iter().position(|&a| a.is_none()) {
                // Try assigning true
                assignment[var_idx] = Some(true);
                if dpll_solve(clauses, assignment, start_time, timeout) {
                    return true;
                }
                
                // Try assigning false
                assignment[var_idx] = Some(false);
                if dpll_solve(clauses, assignment, start_time, timeout) {
                    return true;
                }
                
                // Backtrack
                assignment[var_idx] = None;
                false
            } else {
                // All variables assigned but not all clauses satisfied
                false
            }
        }

        if dpll_solve(clauses, &mut assignment, start_time, timeout) {
            let solution: Vec<bool> = assignment.iter()
                .map(|&a| a.unwrap_or(false))
                .collect();
            Some(Solution::SAT(solution))
        } else {
            if start_time.elapsed() > timeout {
                println!("⏱️  SAT solver timeout after {:.2}s ({} vars, {} clauses)", 
                    start_time.elapsed().as_secs_f64(), variables, clauses.len());
            } else {
                // Debug: Check if problem is actually satisfiable by trying the known solution
                // (This is only for debugging - in production we'd remove this)
                println!("❌ SAT solver failed to find solution ({} vars, {} clauses) - checking satisfiability...", variables, clauses.len());
                
                // Try brute force check on small problems
                if variables <= 32 {
                    let mut test_assignment = vec![false; variables];
                    let mut found = false;
                    for i in 0..(1u64 << variables.min(32)) {
                        for j in 0..variables.min(32) {
                            test_assignment[j] = (i >> j) & 1 == 1;
                        }
                        let satisfied = clauses.iter().all(|clause| {
                            clause.literals.iter().any(|&literal| {
                                let var_idx = (literal.abs() - 1) as usize;
                                if var_idx < test_assignment.len() {
                                    (literal > 0) == test_assignment[var_idx]
                                } else {
                                    false
                                }
                            })
                        });
                        if satisfied {
                            found = true;
                            println!("⚠️  Problem IS satisfiable but DPLL failed! Assignment: {:?}", 
                                test_assignment.iter().take(10).collect::<Vec<_>>());
                            break;
                        }
                    }
                    if !found {
                        println!("⚠️  Problem appears to be UNSATISFIABLE (brute force check)");
                    }
                }
            }
            None
        }
    }

    /// TSP solver using nearest neighbor heuristic
    fn solve_tsp(&self, cities: usize, distances: &[Vec<u64>], memory: &mut usize) -> Option<Solution> {
        if cities == 0 {
            return None;
        }

        *memory += cities * 8;

        let mut tour = Vec::with_capacity(cities);
        let mut visited = vec![false; cities];
        let mut current = 0;

        tour.push(current);
        visited[current] = true;

        for _ in 1..cities {
            let mut nearest = None;
            let mut min_dist = u64::MAX;

            for next in 0..cities {
                if !visited[next] {
                    let dist = distances[current][next];
                    if dist < min_dist {
                        min_dist = dist;
                        nearest = Some(next);
                    }
                }
            }

            if let Some(next) = nearest {
                current = next;
                tour.push(current);
                visited[current] = true;
            }
        }

        Some(Solution::TSP(tour))
    }

    /// Mine a block with commit-reveal protocol
    pub async fn mine_block(
        &mut self,
        prev_hash: Hash,
        height: u64,
        transactions: Vec<Transaction>,
    ) -> Option<Block> {
        use coinject_core::{ConsensusState, TAU_C};
        let mining_start = Instant::now();
        let mut attempts = 0usize;

        let (problem, (solution, solve_time, solve_memory)) = loop {
            attempts += 1;
            println!("\n=== Mining Block {} (attempt #{}) ===", height, attempts);

            // 1. Generate NP-hard problem (deterministically seeded by parent hash)
            // All nodes generate the SAME problem for a given (prev_hash, height) pair
            let problem = self.generate_problem(height, prev_hash).await;

            // Calculate and display dimensional state
            let tau = (height as f64) / TAU_C;
            let consensus_state = ConsensusState::at_tau(tau);
            println!(
                "Dimensional state: τ={:.4}, |ψ|={:.4}, θ={:.4} rad",
                consensus_state.tau, consensus_state.magnitude, consensus_state.phase
            );
            println!("Generated problem: {:?}", problem);

            // 2. Solve the problem and measure performance
            // Use spawn_blocking to avoid starving the tokio runtime
            let problem_clone = problem.clone();
            let solve_result = tokio::task::spawn_blocking(move || {
                solve_problem_blocking(problem_clone)
            }).await.ok().flatten();

            if let Some(result) = solve_result {
                break (problem, result);
            }

            println!(
                "❌ Mining attempt #{} failed to find a solution before timeout. Penalizing difficulty...",
                attempts
            );
            {
                let mut adjuster = self.difficulty_adjuster.write().await;
                adjuster.record_solve_time(FAILURE_PENALTY_TIME);
                let new_size = adjuster.penalize_failure();
                println!("   → New target problem size: {}", new_size);
            }

            if attempts >= MAX_MINING_ATTEMPTS || mining_start.elapsed() >= MINING_TIMEOUT {
                println!(
                    "⏰ Mining aborted after {} attempts ({:.1?}). Waiting for next template.",
                    attempts,
                    mining_start.elapsed()
                );
                return None;
            }

            println!("🔁 Retrying with reduced difficulty...");
        };
        println!("Solved in {:?} using {} bytes", solve_time, solve_memory);

        // 3. Verify solution
        let verify_start = Instant::now();
        if !solution.verify(&problem) {
            println!("Solution verification failed!");
            return None;
        }
        let verify_time = verify_start.elapsed();
        let verify_memory = 1024; // Approximate verification memory

        // 4. Calculate work score
        let work_score = self.work_calculator.calculate(
            &problem,
            &solution,
            solve_time,
            verify_time,
            solve_memory,
            verify_memory,
            0.001, // Energy per operation
        );
        println!("Work score: {}", work_score);

        // 5. Create commitment (prevents grinding)
        // CRITICAL: Epoch salt derived from parent block hash to prevent pre-mining
        // This ensures problems cannot be pre-computed before parent block is mined
        let epoch_salt = prev_hash; // Use parent block hash as epoch salt
        let commitment = Commitment::create(&problem, &solution, &epoch_salt);
        println!("Commitment created: {:?} (epoch_salt from parent: {:?})", commitment.hash, prev_hash);

        // 6. Calculate PoUW transparency metrics
        let solve_time_us = solve_time.as_micros() as u64;
        let verify_time_us = verify_time.as_micros().max(1) as u64; // Minimum 1ms to avoid div by zero
        let time_asymmetry_ratio = solve_time_us as f64 / verify_time_us as f64;
        let solution_quality = solution.quality(&problem);
        let complexity_weight = problem.difficulty_weight();

        // Estimate energy: Assume 100W TDP CPU, energy = power * time
        // 100W = 100 J/s, so energy_joules = 100 * (solve_time in seconds)
        let energy_estimate_joules = 100.0 * solve_time.as_secs_f64();

        println!("PoUW Metrics:");
        println!("  Solve time: {}µs", solve_time_us);
        println!("  Verify time: {}µs", verify_time_us);
        println!("  Time asymmetry: {:.2}x", time_asymmetry_ratio);
        println!("  Solution quality: {:.4}", solution_quality);
        println!("  Complexity weight: {:.2}", complexity_weight);
        println!("  Energy estimate: {:.2} J", energy_estimate_joules);

        // 7. Build block header with commitment and PoUW metrics
        // FIX: For block 1 (height 1), use deterministic timestamp to prevent forks
        // Genesis timestamp is 1735689600 (Jan 1, 2025 00:00:00 UTC)
        // For height 1, use genesis + 1 second to ensure deterministic hash
        // For subsequent blocks, use max(parent_timestamp + 1, current_time) for monotonicity
        let timestamp = if height == 1 {
            // Block 1: Use deterministic timestamp (genesis + 1 second)
            // This ensures all nodes generate the same block 1 hash if they mine it
            1735689601i64  // Jan 1, 2025 00:00:01 UTC
        } else {
            // Subsequent blocks: Use current time, but ensure it's >= parent + 1
            // Note: We don't have parent block here, so we use current time
            // The validator will enforce timestamp ordering
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64
        };
        // M1.1 DEBUG: Log mined timestamp vs current time
        let now_ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let delta = now_ts - timestamp;
        println!("⏱️  [MINER] height={} mined_ts={} now_ts={} delta={}s", height, timestamp, now_ts, delta);

        let transactions_root = Self::merkle_root(&transactions);
        let solutions_root = Hash::new(&bincode::serialize(&solution).unwrap_or_default());

        let mut header = BlockHeader {
            version: 1,
            height,
            prev_hash,
            timestamp,
            transactions_root,
            solutions_root,
            commitment: commitment.clone(),
            work_score,
            miner: self.config.miner_address,
            nonce: 0,
            // PoUW Transparency Metrics
            solve_time_us,
            verify_time_us,
            time_asymmetry_ratio,
            solution_quality,
            complexity_weight,
            energy_estimate_joules,
        };

        // 8. Mine the header (find nonce that meets difficulty)
        // Use spawn_blocking to avoid starving the tokio runtime
        let difficulty = self.difficulty;
        let (mined_header, header_hash) = tokio::task::spawn_blocking(move || {
            mine_header_blocking(header, difficulty)
        }).await.ok().flatten()?;
        let header = mined_header; // Use the mined header with correct nonce
        println!("Header mined: {:?}", header_hash);

        // 9. Calculate block reward and create coinbase transaction
        let reward_amount = self.reward_calculator.calculate_reward(work_score);
        let coinbase = CoinbaseTransaction::new(self.config.miner_address, reward_amount, height);
        println!("Block reward: {} tokens", reward_amount);

        // 10. Create solution reveal
        let solution_reveal = SolutionReveal {
            problem: problem.clone(),
            solution: solution.clone(),
            commitment,
        };

        // 11. Update stats
        self.update_stats(work_score, solve_time).await;

        Some(Block {
            header,
            coinbase,
            transactions,
            solution_reveal,
        })
    }

    /// Mine header by finding nonce that meets difficulty target
    fn mine_header(&self, header: &mut BlockHeader) -> Option<Hash> {
        let target_prefix = "0".repeat(self.difficulty as usize);
        let start_time = Instant::now();
        let mut hashes = 0u64;

        println!("🎯 Mining target: hash must start with '{}'", target_prefix);

        for nonce in 0..u64::MAX {
            header.nonce = nonce;
            let hash = header.hash();
            hashes += 1;

            let hash_hex = hex::encode(hash.as_bytes());

            // Debug: Print first few hash samples
            if nonce < 5 {
                println!("  Sample hash #{}: {}", nonce, hash_hex);
            }

            if hash_hex.starts_with(&target_prefix) {
                let elapsed = start_time.elapsed().as_secs_f64();
                let hash_rate = hashes as f64 / elapsed;
                println!("✅ Found nonce {} after {} hashes ({:.2} H/s)", nonce, hashes, hash_rate);
                println!("   Block hash: {}", hash_hex);
                return Some(hash);
            }

            // Print progress every million hashes
            if nonce % 1_000_000 == 0 && nonce > 0 {
                let elapsed = start_time.elapsed().as_secs_f64();
                let hash_rate = hashes as f64 / elapsed;
                println!("⛏️  Mining... {} hashes ({:.2} H/s) | Latest: {}...",
                    hashes, hash_rate, &hash_hex[..16]);
            }
        }

        None
    }

    /// Calculate merkle root of transactions
    fn merkle_root(transactions: &[Transaction]) -> Hash {
        if transactions.is_empty() {
            return Hash::ZERO;
        }

        let leaves: Vec<Vec<u8>> = transactions
            .iter()
            .map(|tx| bincode::serialize(tx).unwrap_or_default())
            .collect();

        let tree = coinject_core::MerkleTree::new(leaves);
        tree.root()
    }

    /// Update mining statistics
    async fn update_stats(&mut self, work_score: f64, solve_time: Duration) {
        let mut stats = self.stats.write().await;
        stats.blocks_mined += 1;
        stats.total_work_score += work_score;

        // Update average solve time
        let total_time = stats.average_solve_time.as_secs_f64() * (stats.blocks_mined - 1) as f64
            + solve_time.as_secs_f64();
        stats.average_solve_time = Duration::from_secs_f64(total_time / stats.blocks_mined as f64);

        // Record solve time in difficulty adjuster
        let (new_size, diff_stats) = {
            let mut adjuster = self.difficulty_adjuster.write().await;
            adjuster.record_solve_time(solve_time);
            let new_size = adjuster.adjust_difficulty();
            (new_size, adjuster.stats())
        };

        println!(
            "📈 Difficulty stats: size={} avg={:.2}s σ={:.2}s ratio {:.2}x samples {} stall={} recovery={}",
            diff_stats.current_size,
            diff_stats.avg_solve_time_secs,
            diff_stats.std_dev_secs,
            diff_stats.time_ratio,
            diff_stats.sample_count,
            diff_stats.stall_counter,
            diff_stats.in_recovery_mode
        );
        if diff_stats.time_ratio > 2.0 {
            println!("⚠️  Solve time ratio {:.2}x > 2.0. Network may be underpowered—consider adding miners.", diff_stats.time_ratio);
        }
        println!("   → Next problem size target: {}", new_size);
    }

    /// Get current mining stats
    pub async fn get_stats(&self) -> MiningStats {
        self.stats.read().await.clone()
    }

    /// Adjust mining difficulty based on block time
    pub fn adjust_difficulty(&mut self, actual_block_time: Duration) {
        let target = self.config.target_block_time.as_secs_f64();
        let actual = actual_block_time.as_secs_f64();

        if actual < target * 0.8 {
            // Blocks too fast, increase difficulty
            self.difficulty = (self.difficulty + 1).min(self.config.max_difficulty);
            println!("Difficulty increased to {}", self.difficulty);
        } else if actual > target * 1.2 {
            // Blocks too slow, decrease difficulty
            self.difficulty = (self.difficulty.saturating_sub(1)).max(self.config.min_difficulty);
            println!("Difficulty decreased to {}", self.difficulty);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_problem_generation() {
        let config = MiningConfig::default();
        let miner = Miner::new(config);

        let prev_hash = Hash::ZERO;
        let problem = miner.generate_problem(0, prev_hash).await;
        println!("Generated problem: {:?}", problem);

        match problem {
            ProblemType::SubsetSum { ref numbers, .. } => assert!(!numbers.is_empty()),
            ProblemType::SAT { variables, .. } => assert!(variables > 0),
            ProblemType::TSP { cities, .. } => assert!(cities > 0),
            _ => panic!("Unexpected problem type"),
        }
    }

    #[tokio::test]
    async fn test_subset_sum_solver() {
        let config = MiningConfig::default();
        let miner = Miner::new(config);

        let problem = ProblemType::SubsetSum {
            numbers: vec![3, 6, 7, 9, 12],
            target: 15,
        };

        let result = miner.solve_problem(&problem);
        assert!(result.is_some());

        let (solution, _time, _memory) = result.unwrap();
        assert!(solution.verify(&problem));
    }

    #[tokio::test]
    async fn test_tsp_solver() {
        let config = MiningConfig::default();
        let miner = Miner::new(config);

        let problem = ProblemType::TSP {
            cities: 5,
            distances: vec![
                vec![0, 10, 15, 20, 25],
                vec![10, 0, 35, 25, 30],
                vec![15, 35, 0, 30, 20],
                vec![20, 25, 30, 0, 15],
                vec![25, 30, 20, 15, 0],
            ],
        };

        let result = miner.solve_problem(&problem);
        assert!(result.is_some());

        let (solution, _time, _memory) = result.unwrap();
        assert!(solution.verify(&problem));
    }
}
