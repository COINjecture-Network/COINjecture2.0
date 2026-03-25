//! Benchmarks for cryptographic operations: sign, verify, hash.
//!
//! Run with: `cargo bench -p coinject-core --bench crypto_bench`

use coinject_core::KeyPair;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

fn bench_keygen(c: &mut Criterion) {
    c.bench_function("ed25519_keygen", |b| {
        b.iter(KeyPair::generate);
    });
}

fn bench_sign(c: &mut Criterion) {
    let keypair = KeyPair::generate();
    let message = b"coinject transaction payload for signing benchmark";

    c.bench_function("ed25519_sign_50b", |b| {
        b.iter(|| keypair.sign(message));
    });
}

fn bench_verify(c: &mut Criterion) {
    let keypair = KeyPair::generate();
    let message = b"coinject transaction payload for verify benchmark";
    let sig = keypair.sign(message);
    let pubkey = keypair.public_key();

    c.bench_function("ed25519_verify_50b", |b| {
        b.iter(|| pubkey.verify(message, &sig));
    });
}

fn bench_blake3_hash(c: &mut Criterion) {
    use blake3::Hasher;

    let mut group = c.benchmark_group("blake3_hash");
    for size in [32usize, 256, 1024, 4096, 65536] {
        let data = vec![0xABu8; size];
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &data, |b, d| {
            b.iter(|| {
                let mut h = Hasher::new();
                h.update(d);
                h.finalize()
            });
        });
    }
    group.finish();
}

fn bench_sha256_hash(c: &mut Criterion) {
    use sha2::{Digest, Sha256};

    let mut group = c.benchmark_group("sha256_hash");
    for size in [32usize, 256, 1024, 4096] {
        let data = vec![0xCDu8; size];
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &data, |b, d| {
            b.iter(|| {
                let mut h = Sha256::new();
                h.update(d);
                h.finalize()
            });
        });
    }
    group.finish();
}

criterion_group!(
    crypto_benches,
    bench_keygen,
    bench_sign,
    bench_verify,
    bench_blake3_hash,
    bench_sha256_hash
);
criterion_main!(crypto_benches);
