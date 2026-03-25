//! Benchmarks for block serialization, deserialization, and header hashing.
//!
//! Run with: `cargo bench -p coinject-core --bench block_bench`

use coinject_core::{
    Block, BlockHeader, CoinbaseTransaction, Commitment, Hash, ProblemType, Solution,
    SolutionReveal, Transaction, BLOCK_VERSION_STANDARD,
};
use coinject_core::crypto::KeyPair;
use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};

fn zero_address() -> coinject_core::Address {
    coinject_core::Address::from_bytes([0u8; 32])
}

fn zero_commitment() -> Commitment {
    Commitment {
        hash: Hash::ZERO,
        problem_hash: Hash::ZERO,
    }
}

fn make_header(height: u64) -> BlockHeader {
    BlockHeader {
        version: BLOCK_VERSION_STANDARD,
        height,
        prev_hash: Hash::ZERO,
        timestamp: 1_700_000_000 + height as i64,
        transactions_root: Hash::ZERO,
        solutions_root: Hash::ZERO,
        commitment: zero_commitment(),
        work_score: 42.0,
        miner: zero_address(),
        nonce: 12345,
        solve_time_us: 1000,
        verify_time_us: 50,
        time_asymmetry_ratio: 20.0,
        solution_quality: 0.95,
        complexity_weight: 1.0,
        energy_estimate_joules: 0.001,
    }
}

fn make_block(height: u64, tx_count: usize) -> Block {
    let header = make_header(height);
    let coinbase = CoinbaseTransaction::new(zero_address(), 1_000_000, height);
    let keypair = KeyPair::generate();
    let addr = keypair.address();
    let transactions: Vec<Transaction> = (0..tx_count)
        .map(|i| {
            Transaction::new_transfer(
                addr,
                zero_address(),
                1000 + i as u128,
                100,
                i as u64,
                &keypair,
            )
        })
        .collect();
    let solution_reveal = SolutionReveal::new(
        ProblemType::SAT { variables: 10, clauses: vec![] },
        Solution::SAT(vec![true; 10]),
        zero_commitment(),
    );
    Block {
        header,
        coinbase,
        transactions,
        solution_reveal,
    }
}

fn bench_header_hash(c: &mut Criterion) {
    let header = make_header(1000);
    c.bench_function("block_header_hash_bincode", |b| {
        b.iter(|| black_box(header.hash()));
    });
    c.bench_function("block_header_hash_json", |b| {
        b.iter(|| black_box(header.hash_from_json()));
    });
}

fn bench_block_serialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("block_serialize_bincode");
    for tx_count in [0usize, 10, 100] {
        let block = make_block(1, tx_count);
        let bytes = bincode::serialize(&block).unwrap();
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.bench_with_input(
            format!("{}_txs", tx_count),
            &block,
            |b, blk| {
                b.iter(|| bincode::serialize(black_box(blk)).unwrap());
            },
        );
    }
    group.finish();
}

fn bench_block_deserialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("block_deserialize_bincode");
    for tx_count in [0usize, 10, 100] {
        let block = make_block(1, tx_count);
        let bytes = bincode::serialize(&block).unwrap();
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.bench_with_input(
            format!("{}_txs", tx_count),
            &bytes,
            |b, raw| {
                b.iter(|| bincode::deserialize::<Block>(black_box(raw)).unwrap());
            },
        );
    }
    group.finish();
}

criterion_group!(
    block_benches,
    bench_header_hash,
    bench_block_serialize,
    bench_block_deserialize
);
criterion_main!(block_benches);
