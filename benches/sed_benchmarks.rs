use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use rand::Rng;
use rand_xoshiro::rand_core::SeedableRng;
use rand_xoshiro::Xoshiro256PlusPlus;
use std::hint::black_box;
use tree_statistics::lb::sed::{bounded_string_edit_distance, BerghelRoachSed};

/// Generate a random sequence of integers
fn generate_random_sequence(len: usize, max_val: i32, seed: u64) -> Vec<i32> {
    let mut rng = Xoshiro256PlusPlus::seed_from_u64(seed);
    (0..len).map(|_| rng.random_range(0..max_val)).collect()
}

/// Generate a sequence with some similarity to another sequence
fn generate_similar_sequence(base: &[i32], edit_distance: usize, seed: u64) -> Vec<i32> {
    let mut rng = Xoshiro256PlusPlus::seed_from_u64(seed);
    let mut result = base.to_vec();

    for _ in 0..edit_distance {
        let operation = rng.random_range(0..3);
        match operation {
            0 => {
                // Substitution
                if !result.is_empty() {
                    let idx = rng.random_range(0..result.len());
                    result[idx] = rng.random_range(0..100);
                }
            }
            1 => {
                // Insertion
                let idx = rng.random_range(0..=result.len());
                result.insert(idx, rng.random_range(0..100));
            }
            2 => {
                // Deletion
                if !result.is_empty() {
                    let idx = rng.random_range(0..result.len());
                    result.remove(idx);
                }
            }
            _ => unreachable!(),
        }
    }

    result
}

fn bench_bounded_string_edit_distance(c: &mut Criterion) {
    let mut group = c.benchmark_group("bounded_string_edit_distance");

    // Test with different sequence lengths
    for size in [10, 50, 100, 500, 1000].iter() {
        let s1 = generate_random_sequence(*size, 100, 42);
        let s2 = generate_similar_sequence(&s1, size / 10, 123);
        let k = size / 5;

        group.bench_with_input(
            BenchmarkId::new("size", size),
            &(&s1, &s2, k),
            |b, (s1, s2, k)| {
                b.iter(|| {
                    bounded_string_edit_distance(black_box(s1), black_box(s2), black_box(*k))
                });
            },
        );
    }

    group.finish();
}

fn bench_bounded_edit_distance_varying_k(c: &mut Criterion) {
    let mut group = c.benchmark_group("bounded_edit_distance_varying_k");

    let size = 200;
    let s1 = generate_random_sequence(size, 100, 42);
    let s2 = generate_similar_sequence(&s1, 20, 123);

    // Test with different thresholds
    for k in [3, 5, 10, 20].iter() {
        group.bench_with_input(
            BenchmarkId::new("threshold", k),
            &(&s1, &s2, *k),
            |b, (s1, s2, k)| {
                b.iter(|| {
                    bounded_string_edit_distance(black_box(s1), black_box(s2), black_box(*k))
                });
            },
        );
    }

    group.finish();
}

fn bench_berghel_roach_compute_distance(c: &mut Criterion) {
    let mut group = c.benchmark_group("berghel_roach_compute_distance");

    // Test with different sequence lengths
    for size in [10, 50, 100, 500, 1000].iter() {
        let s1 = generate_random_sequence(*size, 100, 42);
        let s2 = generate_similar_sequence(&s1, size / 10, 123);
        let threshold = (size / 5) as i32;

        group.bench_with_input(
            BenchmarkId::new("size", size),
            &(&s1, &s2, threshold),
            |b, (s1, s2, threshold)| {
                let mut br = BerghelRoachSed::initialize_query(s1, *threshold);
                b.iter(|| black_box(br.compute_distance(black_box(s2))));
            },
        );
    }

    group.finish();
}

fn bench_berghel_roach_varying_threshold(c: &mut Criterion) {
    let mut group = c.benchmark_group("berghel_roach_varying_threshold");

    let size = 200;
    let s1 = generate_random_sequence(size, 100, 42);
    let s2 = generate_similar_sequence(&s1, 20, 123);

    // Test with different thresholds
    for threshold in [3, 5, 10, 20].iter() {
        group.bench_with_input(
            BenchmarkId::new("threshold", threshold),
            &(&s1, &s2, *threshold as i32),
            |b, (s1, s2, threshold)| {
                let mut br = BerghelRoachSed::initialize_query(s1, *threshold);
                b.iter(|| black_box(br.compute_distance(black_box(s2))));
            },
        );
    }

    group.finish();
}

fn bench_berghel_roach_reinitialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("berghel_roach_reinitialization");

    let size = 200;
    let threshold = 40;

    // Benchmark the cost of reinitializing with new query
    group.bench_function("reinitialize_query", |b| {
        let s1 = generate_random_sequence(size, 100, 42);
        let s2 = generate_random_sequence(size, 100, 123);
        let mut br = BerghelRoachSed::initialize_query(&s1, threshold);

        b.iter(|| {
            br.reinitialize_query(black_box(&s2), black_box(threshold));
        });
    });

    // Compare reinitialization vs creating new instance
    group.bench_function("create_new_instance", |b| {
        let s1 = generate_random_sequence(size, 100, 42);

        b.iter(|| {
            black_box(BerghelRoachSed::initialize_query(
                black_box(&s1),
                black_box(threshold),
            ))
        });
    });

    group.finish();
}

fn bench_comparison_both_methods(c: &mut Criterion) {
    let mut group = c.benchmark_group("comparison_both_methods");

    let size = 200;
    let s1 = generate_random_sequence(size, 100, 42);
    let s2 = generate_similar_sequence(&s1, 10, 123);
    let threshold = 12;

    group.bench_function("bounded_string_edit_distance", |b| {
        b.iter(|| {
            bounded_string_edit_distance(
                black_box(&s1),
                black_box(&s2),
                black_box(threshold as usize),
            )
        });
    });

    group.bench_function("berghel_roach_compute_distance", |b| {
        let mut br = BerghelRoachSed::initialize_query(&s1, threshold);
        b.iter(|| black_box(br.compute_distance(black_box(&s2))));
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_bounded_string_edit_distance,
    bench_bounded_edit_distance_varying_k,
    bench_berghel_roach_compute_distance,
    bench_berghel_roach_varying_threshold,
    bench_berghel_roach_reinitialization,
    bench_comparison_both_methods,
);
criterion_main!(benches);
