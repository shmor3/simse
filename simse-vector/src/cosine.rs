/// Compute cosine similarity between two f32 vectors.
/// Returns 0.0 for zero-magnitude vectors or dimension mismatches.
/// Result clamped to [-1.0, 1.0].
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
	if a.len() != b.len() || a.is_empty() {
		return 0.0;
	}

	let mut dot: f64 = 0.0;
	let mut norm_a: f64 = 0.0;
	let mut norm_b: f64 = 0.0;

	for i in 0..a.len() {
		let ai = a[i] as f64;
		let bi = b[i] as f64;
		dot += ai * bi;
		norm_a += ai * ai;
		norm_b += bi * bi;
	}

	let denom = norm_a.sqrt() * norm_b.sqrt();
	if denom == 0.0 {
		return 0.0;
	}

	let result = dot / denom;
	if !result.is_finite() {
		return 0.0;
	}
	result.clamp(-1.0, 1.0)
}

/// Compute the magnitude (L2 norm) of a vector.
pub fn compute_magnitude(embedding: &[f32]) -> f64 {
	let mut sum: f64 = 0.0;
	for &v in embedding {
		let vf = v as f64;
		sum += vf * vf;
	}
	sum.sqrt()
}

/// Compute cosine similarity using pre-computed magnitudes (optimization).
/// Falls back to returning 0.0 if magnitudes are zero.
pub fn cosine_similarity_with_magnitude(a: &[f32], b: &[f32], mag_a: f64, mag_b: f64) -> f64 {
	if a.len() != b.len() || a.is_empty() {
		return 0.0;
	}

	let denom = mag_a * mag_b;
	if denom == 0.0 {
		return 0.0;
	}

	let mut dot: f64 = 0.0;
	for i in 0..a.len() {
		dot += (a[i] as f64) * (b[i] as f64);
	}

	let result = dot / denom;
	if !result.is_finite() {
		return 0.0;
	}
	result.clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn identical_vectors() {
		let v = vec![1.0f32, 2.0, 3.0];
		let sim = cosine_similarity(&v, &v);
		assert!((sim - 1.0).abs() < 1e-10);
	}

	#[test]
	fn orthogonal_vectors() {
		let a = vec![1.0f32, 0.0];
		let b = vec![0.0f32, 1.0];
		assert!((cosine_similarity(&a, &b)).abs() < 1e-10);
	}

	#[test]
	fn opposite_vectors() {
		let a = vec![1.0f32, 0.0];
		let b = vec![-1.0f32, 0.0];
		assert!((cosine_similarity(&a, &b) + 1.0).abs() < 1e-10);
	}

	#[test]
	fn empty_vectors() {
		assert_eq!(cosine_similarity(&[], &[]), 0.0);
	}

	#[test]
	fn mismatched_lengths() {
		assert_eq!(cosine_similarity(&[1.0], &[1.0, 2.0]), 0.0);
	}

	#[test]
	fn zero_magnitude() {
		let a = vec![0.0f32, 0.0];
		let b = vec![1.0f32, 2.0];
		assert_eq!(cosine_similarity(&a, &b), 0.0);
	}

	#[test]
	fn magnitude_basic() {
		let v = vec![3.0f32, 4.0];
		assert!((compute_magnitude(&v) - 5.0).abs() < 1e-10);
	}

	#[test]
	fn magnitude_empty() {
		assert_eq!(compute_magnitude(&[]), 0.0);
	}

	#[test]
	fn cosine_with_precomputed_magnitude() {
		let a = vec![1.0f32, 0.0, 0.0];
		let b = vec![0.0f32, 1.0, 0.0];
		let mag_a = compute_magnitude(&a);
		let mag_b = compute_magnitude(&b);
		let sim = cosine_similarity_with_magnitude(&a, &b, mag_a, mag_b);
		assert!(sim.abs() < 1e-10);
	}

	#[test]
	fn cosine_with_precomputed_identical() {
		let v = vec![1.0f32, 2.0, 3.0];
		let mag = compute_magnitude(&v);
		let sim = cosine_similarity_with_magnitude(&v, &v, mag, mag);
		assert!((sim - 1.0).abs() < 1e-10);
	}
}
