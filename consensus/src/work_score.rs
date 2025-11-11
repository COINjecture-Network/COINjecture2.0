// Work Score Calculation Engine
// work_score = (solve_time / verify_time) × √(solve_memory / verify_memory) ×
//              problem_weight × size_factor × quality_score × energy_efficiency

use coinject_core::{ProblemType, Solution, WorkScore};
use std::time::Duration;

pub struct WorkScoreCalculator {
    base_constant: f64,
}

impl WorkScoreCalculator {
    pub fn new() -> Self {
        WorkScoreCalculator {
            base_constant: 1.0, // Adjusted dynamically
        }
    }

    /// Calculate dimensionless work score
    pub fn calculate(
        &self,
        problem: &ProblemType,
        solution: &Solution,
        solve_time: Duration,
        verify_time: Duration,
        solve_memory: usize,
        verify_memory: usize,
        energy_per_op: f64,
    ) -> WorkScore {
        // 1. Time asymmetry ratio
        let time_ratio = solve_time.as_secs_f64() / verify_time.as_secs_f64().max(0.001);

        // 2. Space asymmetry ratio
        let space_ratio = (solve_memory as f64 / verify_memory as f64).sqrt();

        // 3. Problem difficulty weight
        let problem_weight = problem.difficulty_weight();

        // 4. Solution quality (0.0 to 1.0)
        let quality_score = solution.quality(problem);

        // 5. Energy efficiency (lower energy = higher score)
        let energy_efficiency = 1.0 / (energy_per_op + 1.0);

        // Dimensionless work score
        self.base_constant
            * time_ratio
            * space_ratio
            * problem_weight
            * quality_score
            * energy_efficiency
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use coinject_core::{ProblemType, Solution};

    #[test]
    fn test_work_score_calculation() {
        let calculator = WorkScoreCalculator::new();

        let problem = ProblemType::SubsetSum {
            numbers: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            target: 25,
        };

        let solution = Solution::SubsetSum(vec![4, 5, 6, 7]); // 5 + 6 + 7 + 8 = 26... wrong
        let solution = Solution::SubsetSum(vec![3, 4, 6, 9]); // 4 + 5 + 7 + 10 = 26... still wrong
        let solution = Solution::SubsetSum(vec![2, 6, 7, 8]); // 3 + 7 + 8 + 9 = 27... argh
        let solution = Solution::SubsetSum(vec![2, 5, 6, 8]); // 3 + 6 + 7 + 9 = 25

        let solve_time = Duration::from_secs(10);
        let verify_time = Duration::from_millis(1);
        let solve_memory = 1024 * 1024;
        let verify_memory = 1024;
        let energy_per_op = 0.001;

        let score = calculator.calculate(
            &problem,
            &solution,
            solve_time,
            verify_time,
            solve_memory,
            verify_memory,
            energy_per_op,
        );

        assert!(score > 0.0);
        println!("Work score: {}", score);
    }
}
