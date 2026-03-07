use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use rand::RngExt;

use simse_core::adaptive::distance::*;
use simse_core::adaptive::index::*;
use simse_core::adaptive::quantization::*;

fn random_vectors(n: usize, dims: usize) -> Vec<Vec<f32>> {
	let mut rng = rand::rng();
	(0..n)
		.map(|_| (0..dims).map(|_| rng.random::<f32>() * 2.0 - 1.0).collect())
		.collect()
}

fn bench_distance_metrics(c: &mut Criterion) {
	let mut group = c.benchmark_group("distance");
	for dims in [128, 384, 768] {
		let a: Vec<f32> = (0..dims).map(|i| (i as f32) * 0.001).collect();
		let b: Vec<f32> = (0..dims).map(|i| ((dims - i) as f32) * 0.001).collect();

		group.bench_with_input(BenchmarkId::new("cosine", dims), &dims, |bench, _| {
			bench.iter(|| DistanceMetric::Cosine.similarity(&a, &b));
		});
		group.bench_with_input(BenchmarkId::new("euclidean", dims), &dims, |bench, _| {
			bench.iter(|| DistanceMetric::Euclidean.similarity(&a, &b));
		});
		group.bench_with_input(BenchmarkId::new("dot_product", dims), &dims, |bench, _| {
			bench.iter(|| DistanceMetric::DotProduct.similarity(&a, &b));
		});
		group.bench_with_input(BenchmarkId::new("manhattan", dims), &dims, |bench, _| {
			bench.iter(|| DistanceMetric::Manhattan.similarity(&a, &b));
		});
	}
	group.finish();
}

fn bench_flat_search(c: &mut Criterion) {
	let mut group = c.benchmark_group("flat_search");
	let dims = 384;

	for n in [100, 1_000, 10_000] {
		let vectors = random_vectors(n, dims);
		let query: Vec<f32> = (0..dims).map(|i| (i as f32) * 0.001).collect();

		let mut idx = FlatIndex::new(dims);
		for (i, v) in vectors.iter().enumerate() {
			idx.insert(&format!("v{i}"), v);
		}

		group.bench_with_input(BenchmarkId::new("cosine", n), &n, |bench, _| {
			bench.iter(|| idx.search(&query, 10, DistanceMetric::Cosine));
		});
	}
	group.finish();
}

fn bench_quantization(c: &mut Criterion) {
	let mut group = c.benchmark_group("quantization");
	let dims = 384;
	let v: Vec<f32> = (0..dims).map(|i| (i as f32) * 0.01 - 1.92).collect();

	group.bench_function("scalar_encode_384d", |bench| {
		bench.iter(|| ScalarQuantizer::fit_encode(&v));
	});

	group.bench_function("binary_encode_384d", |bench| {
		bench.iter(|| BinaryQuantizer::encode(&v));
	});

	let qa = ScalarQuantizer::fit_encode(&v);
	let qb = ScalarQuantizer::fit_encode(&v);
	group.bench_function("scalar_approx_cosine", |bench| {
		bench.iter(|| qa.approximate_cosine(&qb));
	});

	let ba = BinaryQuantizer::encode(&v);
	let bb = BinaryQuantizer::encode(&v);
	group.bench_function("binary_hamming", |bench| {
		bench.iter(|| BinaryQuantizer::hamming_distance(&ba, &bb));
	});

	group.finish();
}

criterion_group!(
	benches,
	bench_distance_metrics,
	bench_flat_search,
	bench_quantization
);
criterion_main!(benches);
