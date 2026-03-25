//! Benchmarks for consensus operations: work score calculation, difficulty adjustment.
//!
//! Run with: `cargo bench -p coinject-consensus --bench consensus_bench`

use coinject_consensus::work_score::WorkScoreCalculator;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;

fn bench_work_score_calculation(c: &mut Criterion) {
    let calc = WorkScoreCalculator::new();

    let mut group = c.benchmark_group("work_score_calc");
    // Vary solve_time to exercise different asymmetry ratios
    for asymmetry in [2u64, 16, 256, 4096, 65536] {
        let solve_time = Duration::from_millis(asymmetry);
        let verify_time = Duration::from_millis(1);
        let quality = 0.95f64;

        group.bench_with_input(
            BenchmarkId::from_parameter(asymmetry),
            &(solve_time, verify_time, quality),
            |b, &(st, vt, q)| {
                b.iter(|| calc.calculate(black_box(st), black_box(vt), black_box(q)));
            },
        );
    }
    group.finish();
}

fn bench_work_score_batch(c: &mut Criterion) {
    let calc = WorkScoreCalculator::new();
    let inputs: Vec<(Duration, Duration, f64)> = (0..1000)
        .map(|i| {
            (
                Duration::from_millis(10 + i % 100),
                Duration::from_millis(1),
                0.9 + (i as f64 % 10.0) * 0.01,
            )
        })
        .collect();

    c.bench_function("work_score_batch_1000", |b| {
        b.iter(|| {
            inputs
                .iter()
                .map(|(st, vt, q)| {
                    calc.calculate(black_box(*st), black_box(*vt), black_box(*q))
                })
                .sum::<f64>()
        });
    });
}

criterion_group!(
    consensus_benches,
    bench_work_score_calculation,
    bench_work_score_batch
);
criterion_main!(consensus_benches);
