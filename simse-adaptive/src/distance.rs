use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum DistanceMetric {
	#[default]
	Cosine,
	Euclidean,
	DotProduct,
	Manhattan,
}

impl DistanceMetric {
	/// Returns a distance function (lower = more similar for Cosine/Euclidean/Manhattan,
	/// more negative = more similar for DotProduct).
	pub fn distance_fn(self) -> fn(&[f32], &[f32]) -> f64 {
		match self {
			Self::Cosine => cosine_distance,
			Self::Euclidean => euclidean_distance,
			Self::DotProduct => dot_product_distance,
			Self::Manhattan => manhattan_distance,
		}
	}

	/// Returns a similarity score (higher = more similar). Range depends on metric.
	/// Cosine: [-1, 1], DotProduct: unbounded, Euclidean/Manhattan: 1/(1+d).
	pub fn similarity(self, a: &[f32], b: &[f32]) -> f64 {
		match self {
			Self::Cosine => cosine_similarity_score(a, b),
			Self::DotProduct => dot_product_similarity(a, b),
			Self::Euclidean => {
				let d = euclidean_distance(a, b);
				1.0 / (1.0 + d)
			}
			Self::Manhattan => {
				let d = manhattan_distance(a, b);
				1.0 / (1.0 + d)
			}
		}
	}
}

// ---------------------------------------------------------------------------
// Cosine
// ---------------------------------------------------------------------------

pub fn cosine_similarity_score(a: &[f32], b: &[f32]) -> f64 {
	if a.len() != b.len() || a.is_empty() {
		return 0.0;
	}
	let (mut dot, mut na, mut nb) = (0.0f64, 0.0f64, 0.0f64);
	for i in 0..a.len() {
		let ai = a[i] as f64;
		let bi = b[i] as f64;
		dot += ai * bi;
		na += ai * ai;
		nb += bi * bi;
	}
	let denom = na.sqrt() * nb.sqrt();
	if denom == 0.0 {
		return 0.0;
	}
	let r = dot / denom;
	if !r.is_finite() {
		return 0.0;
	}
	r.clamp(-1.0, 1.0)
}

pub fn cosine_distance(a: &[f32], b: &[f32]) -> f64 {
	1.0 - cosine_similarity_score(a, b)
}

pub fn compute_magnitude(embedding: &[f32]) -> f64 {
	let mut sum = 0.0f64;
	for &v in embedding {
		let vf = v as f64;
		sum += vf * vf;
	}
	sum.sqrt()
}

pub fn cosine_similarity_with_magnitude(a: &[f32], b: &[f32], mag_a: f64, mag_b: f64) -> f64 {
	if a.len() != b.len() || a.is_empty() {
		return 0.0;
	}
	let denom = mag_a * mag_b;
	if denom == 0.0 {
		return 0.0;
	}
	let mut dot = 0.0f64;
	for i in 0..a.len() {
		dot += (a[i] as f64) * (b[i] as f64);
	}
	let r = dot / denom;
	if !r.is_finite() {
		return 0.0;
	}
	r.clamp(-1.0, 1.0)
}

/// Backwards-compat alias.
pub use cosine_similarity_score as cosine_similarity;

// ---------------------------------------------------------------------------
// Euclidean (L2)
// ---------------------------------------------------------------------------

pub fn euclidean_distance(a: &[f32], b: &[f32]) -> f64 {
	if a.len() != b.len() {
		return 0.0;
	}
	let mut sum = 0.0f64;
	for i in 0..a.len() {
		let d = (a[i] as f64) - (b[i] as f64);
		sum += d * d;
	}
	sum.sqrt()
}

// ---------------------------------------------------------------------------
// Dot Product
// ---------------------------------------------------------------------------

pub fn dot_product_distance(a: &[f32], b: &[f32]) -> f64 {
	-dot_product_similarity(a, b)
}

pub fn dot_product_similarity(a: &[f32], b: &[f32]) -> f64 {
	if a.len() != b.len() {
		return 0.0;
	}
	let mut dot = 0.0f64;
	for i in 0..a.len() {
		dot += (a[i] as f64) * (b[i] as f64);
	}
	dot
}

// ---------------------------------------------------------------------------
// Manhattan (L1)
// ---------------------------------------------------------------------------

pub fn manhattan_distance(a: &[f32], b: &[f32]) -> f64 {
	if a.len() != b.len() {
		return 0.0;
	}
	let mut sum = 0.0f64;
	for i in 0..a.len() {
		sum += ((a[i] as f64) - (b[i] as f64)).abs();
	}
	sum
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn cosine_identical_vectors() {
		let a = vec![1.0f32, 2.0, 3.0];
		assert!((cosine_distance(&a, &a) - 0.0).abs() < 1e-6);
		assert!((cosine_similarity_score(&a, &a) - 1.0).abs() < 1e-6);
	}

	#[test]
	fn cosine_orthogonal() {
		let a = vec![1.0f32, 0.0];
		let b = vec![0.0f32, 1.0];
		assert!((cosine_similarity_score(&a, &b)).abs() < 1e-6);
		assert!((cosine_distance(&a, &b) - 1.0).abs() < 1e-6);
	}

	#[test]
	fn cosine_opposite() {
		let a = vec![1.0f32, 0.0];
		let b = vec![-1.0f32, 0.0];
		assert!((cosine_similarity_score(&a, &b) + 1.0).abs() < 1e-6);
	}

	#[test]
	fn cosine_empty() {
		assert_eq!(cosine_similarity_score(&[], &[]), 0.0);
	}

	#[test]
	fn cosine_mismatched() {
		assert_eq!(cosine_similarity_score(&[1.0], &[1.0, 2.0]), 0.0);
	}

	#[test]
	fn cosine_zero_magnitude() {
		assert_eq!(cosine_similarity_score(&[0.0, 0.0], &[1.0, 2.0]), 0.0);
	}

	#[test]
	fn euclidean_known() {
		let a = vec![0.0f32, 0.0];
		let b = vec![3.0f32, 4.0];
		assert!((euclidean_distance(&a, &b) - 5.0).abs() < 1e-6);
	}

	#[test]
	fn euclidean_identical() {
		let a = vec![1.0f32, 2.0, 3.0];
		assert!(euclidean_distance(&a, &a).abs() < 1e-10);
	}

	#[test]
	fn dot_product_known() {
		let a = vec![1.0f32, 2.0, 3.0];
		let b = vec![4.0f32, 5.0, 6.0];
		// 1*4 + 2*5 + 3*6 = 32
		assert!((dot_product_similarity(&a, &b) - 32.0).abs() < 1e-6);
	}

	#[test]
	fn dot_product_distance_negates() {
		let a = vec![1.0f32, 2.0];
		let b = vec![3.0f32, 4.0];
		assert!((dot_product_distance(&a, &b) + dot_product_similarity(&a, &b)).abs() < 1e-10);
	}

	#[test]
	fn manhattan_known() {
		let a = vec![1.0f32, 2.0, 3.0];
		let b = vec![4.0f32, 6.0, 3.0];
		// |1-4| + |2-6| + |3-3| = 3 + 4 + 0 = 7
		assert!((manhattan_distance(&a, &b) - 7.0).abs() < 1e-6);
	}

	#[test]
	fn distance_fn_dispatch() {
		let a = vec![1.0f32, 0.0];
		let b = vec![0.0f32, 1.0];
		let f = DistanceMetric::Cosine.distance_fn();
		let d = f(&a, &b);
		assert!((d - 1.0).abs() < 1e-6); // cosine distance = 1 - 0 = 1
	}

	#[test]
	fn similarity_fn_dispatch() {
		let a = vec![1.0f32, 2.0, 3.0];
		let sim = DistanceMetric::Cosine.similarity(&a, &a);
		assert!((sim - 1.0).abs() < 1e-6);
	}

	#[test]
	fn compute_magnitude_known() {
		let a = vec![3.0f32, 4.0];
		assert!((compute_magnitude(&a) - 5.0).abs() < 1e-6);
	}

	#[test]
	fn with_magnitude_matches_without() {
		let a = vec![1.0f32, 2.0, 3.0];
		let b = vec![4.0f32, 5.0, 6.0];
		let direct = cosine_similarity_score(&a, &b);
		let mag_a = compute_magnitude(&a);
		let mag_b = compute_magnitude(&b);
		let cached = cosine_similarity_with_magnitude(&a, &b, mag_a, mag_b);
		assert!((direct - cached).abs() < 1e-10);
	}
}
