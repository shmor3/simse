// ---------------------------------------------------------------------------
// Distance — unified distance/similarity metrics for vector operations
// ---------------------------------------------------------------------------
//
// Provides four distance metrics (Cosine, Euclidean, DotProduct, Manhattan)
// with both distance and similarity functions. All functions accept `&[f32]`
// inputs and return `f64` for precision.
//
// Cosine similarity uses f64 internal arithmetic, clamped to [-1, 1], and
// returns 0.0 for zero-magnitude or empty/mismatched vectors.
// ---------------------------------------------------------------------------

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// DistanceMetric enum
// ---------------------------------------------------------------------------

/// Supported distance metrics for vector similarity search.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistanceMetric {
	#[default]
	Cosine,
	Euclidean,
	DotProduct,
	Manhattan,
}

/// Function pointer type for distance functions.
pub type DistanceFn = fn(&[f32], &[f32]) -> f64;

impl DistanceMetric {
	/// Returns a function pointer for the raw distance function of this metric.
	///
	/// Uses SIMD-accelerated paths when available (AVX2+FMA on x86_64,
	/// NEON on aarch64), with scalar fallback.
	///
	/// - Cosine: `1.0 - simd_cosine_similarity`
	/// - Euclidean: `simd_euclidean_distance`
	/// - DotProduct: negated `simd_dot_product`
	/// - Manhattan: `simd_manhattan_distance`
	#[inline]
	pub fn distance_fn(self) -> DistanceFn {
		match self {
			Self::Cosine => simd_cosine_distance,
			Self::Euclidean => simd_euclidean_distance,
			Self::DotProduct => simd_dot_product_distance,
			Self::Manhattan => simd_manhattan_distance,
		}
	}

	/// Compute the similarity score between two vectors using this metric.
	///
	/// Uses SIMD-accelerated paths when available (AVX2+FMA on x86_64,
	/// NEON on aarch64), with scalar fallback.
	///
	/// Higher values always mean more similar:
	/// - Cosine: clamped cosine similarity in [-1, 1]
	/// - Euclidean: `1.0 / (1.0 + distance)`
	/// - DotProduct: raw dot product (higher = more similar)
	/// - Manhattan: `1.0 / (1.0 + distance)`
	#[inline]
	pub fn similarity(self, a: &[f32], b: &[f32]) -> f64 {
		match self {
			Self::Cosine => simd_cosine_similarity(a, b),
			Self::Euclidean => {
				let d = simd_euclidean_distance(a, b);
				1.0 / (1.0 + d)
			}
			Self::DotProduct => simd_dot_product(a, b),
			Self::Manhattan => {
				let d = simd_manhattan_distance(a, b);
				1.0 / (1.0 + d)
			}
		}
	}
}

// ---------------------------------------------------------------------------
// Cosine
// ---------------------------------------------------------------------------

/// Compute cosine similarity between two f32 vectors.
///
/// Returns a value in [-1.0, 1.0]. Returns 0.0 for zero-magnitude vectors,
/// empty vectors, or dimension mismatches. Uses f64 internal arithmetic
/// for precision.
#[inline]
pub fn cosine_similarity_score(a: &[f32], b: &[f32]) -> f64 {
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

/// Backward-compatible alias for `cosine_similarity_score`.
pub use cosine_similarity_score as cosine_similarity;

/// Compute cosine distance: `1.0 - cosine_similarity`.
///
/// Returns a value in [0.0, 2.0].
#[inline]
pub fn cosine_distance(a: &[f32], b: &[f32]) -> f64 {
	1.0 - cosine_similarity_score(a, b)
}

/// Compute the magnitude (L2 norm) of a vector.
#[inline]
pub fn compute_magnitude(embedding: &[f32]) -> f64 {
	let mut sum: f64 = 0.0;
	for &v in embedding {
		let vf = v as f64;
		sum += vf * vf;
	}
	sum.sqrt()
}

/// Compute cosine similarity using pre-computed magnitudes (optimization).
///
/// Falls back to returning 0.0 if magnitudes are zero or vectors are
/// empty/mismatched.
#[inline]
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

// ---------------------------------------------------------------------------
// Euclidean
// ---------------------------------------------------------------------------

/// Compute Euclidean (L2) distance between two f32 vectors.
///
/// Returns 0.0 for empty or mismatched vectors.
#[inline]
pub fn euclidean_distance(a: &[f32], b: &[f32]) -> f64 {
	if a.len() != b.len() || a.is_empty() {
		return 0.0;
	}

	let mut sum: f64 = 0.0;
	for i in 0..a.len() {
		let diff = (a[i] as f64) - (b[i] as f64);
		sum += diff * diff;
	}
	sum.sqrt()
}

// ---------------------------------------------------------------------------
// Dot Product
// ---------------------------------------------------------------------------

/// Compute raw dot product between two f32 vectors.
///
/// Higher values indicate greater similarity (for normalized vectors,
/// equivalent to cosine similarity). Returns 0.0 for empty or mismatched
/// vectors.
#[inline]
pub fn dot_product_similarity(a: &[f32], b: &[f32]) -> f64 {
	if a.len() != b.len() || a.is_empty() {
		return 0.0;
	}

	let mut dot: f64 = 0.0;
	for i in 0..a.len() {
		dot += (a[i] as f64) * (b[i] as f64);
	}
	dot
}

/// Compute dot product distance (negated dot product).
///
/// Negation makes it usable as a distance metric where lower = closer.
/// Returns 0.0 for empty or mismatched vectors.
#[inline]
pub fn dot_product_distance(a: &[f32], b: &[f32]) -> f64 {
	-dot_product_similarity(a, b)
}

// ---------------------------------------------------------------------------
// Manhattan
// ---------------------------------------------------------------------------

/// Compute Manhattan (L1) distance between two f32 vectors.
///
/// Returns 0.0 for empty or mismatched vectors.
#[inline]
pub fn manhattan_distance(a: &[f32], b: &[f32]) -> f64 {
	if a.len() != b.len() || a.is_empty() {
		return 0.0;
	}

	let mut sum: f64 = 0.0;
	for i in 0..a.len() {
		sum += ((a[i] as f64) - (b[i] as f64)).abs();
	}
	sum
}

// ---------------------------------------------------------------------------
// SIMD — AVX2 internals (x86_64)
// ---------------------------------------------------------------------------

/// Horizontally sum 8 f32 lanes in a __m256 register to a single f32.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2,fma")]
#[allow(clippy::undocumented_unsafe_blocks)]
unsafe fn avx2_hsum_ps(v: std::arch::x86_64::__m256) -> f32 {
	use std::arch::x86_64::*;
	let hi = _mm256_extractf128_ps(v, 1);
	let lo = _mm256_castps256_ps128(v);
	let sum128 = _mm_add_ps(lo, hi);
	let sum64 = _mm_add_ps(sum128, _mm_movehl_ps(sum128, sum128));
	let sum32 = _mm_add_ss(sum64, _mm_shuffle_ps(sum64, sum64, 1));
	_mm_cvtss_f32(sum32)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2,fma")]
#[allow(clippy::undocumented_unsafe_blocks)]
unsafe fn avx2_dot_product(a: &[f32], b: &[f32]) -> f64 {
	use std::arch::x86_64::*;
	unsafe {
		let n = a.len();
		let chunks = n / 8;
		let mut sum = _mm256_setzero_ps();

		for i in 0..chunks {
			let va = _mm256_loadu_ps(a.as_ptr().add(i * 8));
			let vb = _mm256_loadu_ps(b.as_ptr().add(i * 8));
			sum = _mm256_fmadd_ps(va, vb, sum);
		}

		let mut result = avx2_hsum_ps(sum) as f64;

		// Scalar tail for remaining elements
		for i in (chunks * 8)..n {
			result += (*a.get_unchecked(i) as f64) * (*b.get_unchecked(i) as f64);
		}

		result
	}
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2,fma")]
#[allow(clippy::undocumented_unsafe_blocks)]
unsafe fn avx2_cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
	use std::arch::x86_64::*;
	unsafe {
		let n = a.len();
		let chunks = n / 8;
		let mut dot_acc = _mm256_setzero_ps();
		let mut norm_a_acc = _mm256_setzero_ps();
		let mut norm_b_acc = _mm256_setzero_ps();

		for i in 0..chunks {
			let va = _mm256_loadu_ps(a.as_ptr().add(i * 8));
			let vb = _mm256_loadu_ps(b.as_ptr().add(i * 8));
			dot_acc = _mm256_fmadd_ps(va, vb, dot_acc);
			norm_a_acc = _mm256_fmadd_ps(va, va, norm_a_acc);
			norm_b_acc = _mm256_fmadd_ps(vb, vb, norm_b_acc);
		}

		let mut dot = avx2_hsum_ps(dot_acc) as f64;
		let mut norm_a = avx2_hsum_ps(norm_a_acc) as f64;
		let mut norm_b = avx2_hsum_ps(norm_b_acc) as f64;

		// Scalar tail
		for i in (chunks * 8)..n {
			let ai = *a.get_unchecked(i) as f64;
			let bi = *b.get_unchecked(i) as f64;
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
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2,fma")]
#[allow(clippy::undocumented_unsafe_blocks)]
unsafe fn avx2_euclidean_distance(a: &[f32], b: &[f32]) -> f64 {
	use std::arch::x86_64::*;
	unsafe {
		let n = a.len();
		let chunks = n / 8;
		let mut sum = _mm256_setzero_ps();

		for i in 0..chunks {
			let va = _mm256_loadu_ps(a.as_ptr().add(i * 8));
			let vb = _mm256_loadu_ps(b.as_ptr().add(i * 8));
			let diff = _mm256_sub_ps(va, vb);
			sum = _mm256_fmadd_ps(diff, diff, sum);
		}

		let mut result = avx2_hsum_ps(sum) as f64;

		// Scalar tail
		for i in (chunks * 8)..n {
			let diff = (*a.get_unchecked(i) as f64) - (*b.get_unchecked(i) as f64);
			result += diff * diff;
		}

		result.sqrt()
	}
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2,fma")]
#[allow(clippy::undocumented_unsafe_blocks)]
unsafe fn avx2_manhattan_distance(a: &[f32], b: &[f32]) -> f64 {
	use std::arch::x86_64::*;
	unsafe {
		let n = a.len();
		let chunks = n / 8;
		// Sign mask: clear the sign bit to compute absolute value
		let sign_mask = _mm256_castsi256_ps(_mm256_set1_epi32(0x7FFF_FFFFu32 as i32));
		let mut sum = _mm256_setzero_ps();

		for i in 0..chunks {
			let va = _mm256_loadu_ps(a.as_ptr().add(i * 8));
			let vb = _mm256_loadu_ps(b.as_ptr().add(i * 8));
			let diff = _mm256_sub_ps(va, vb);
			let abs_diff = _mm256_and_ps(diff, sign_mask);
			sum = _mm256_add_ps(sum, abs_diff);
		}

		let mut result = avx2_hsum_ps(sum) as f64;

		// Scalar tail
		for i in (chunks * 8)..n {
			result += ((*a.get_unchecked(i) as f64) - (*b.get_unchecked(i) as f64)).abs();
		}

		result
	}
}

// ---------------------------------------------------------------------------
// SIMD — NEON internals (aarch64)
// ---------------------------------------------------------------------------

#[cfg(target_arch = "aarch64")]
#[allow(clippy::undocumented_unsafe_blocks)]
unsafe fn neon_dot_product(a: &[f32], b: &[f32]) -> f64 {
	use std::arch::aarch64::*;
	let n = a.len();
	let chunks = n / 4;
	let mut sum = vdupq_n_f32(0.0);

	for i in 0..chunks {
		let va = vld1q_f32(a.as_ptr().add(i * 4));
		let vb = vld1q_f32(b.as_ptr().add(i * 4));
		sum = vfmaq_f32(sum, va, vb);
	}

	let mut result = vaddvq_f32(sum) as f64;

	for i in (chunks * 4)..n {
		result += (*a.get_unchecked(i) as f64) * (*b.get_unchecked(i) as f64);
	}

	result
}

#[cfg(target_arch = "aarch64")]
#[allow(clippy::undocumented_unsafe_blocks)]
unsafe fn neon_cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
	use std::arch::aarch64::*;
	let n = a.len();
	let chunks = n / 4;
	let mut dot_acc = vdupq_n_f32(0.0);
	let mut norm_a_acc = vdupq_n_f32(0.0);
	let mut norm_b_acc = vdupq_n_f32(0.0);

	for i in 0..chunks {
		let va = vld1q_f32(a.as_ptr().add(i * 4));
		let vb = vld1q_f32(b.as_ptr().add(i * 4));
		dot_acc = vfmaq_f32(dot_acc, va, vb);
		norm_a_acc = vfmaq_f32(norm_a_acc, va, va);
		norm_b_acc = vfmaq_f32(norm_b_acc, vb, vb);
	}

	let mut dot = vaddvq_f32(dot_acc) as f64;
	let mut norm_a = vaddvq_f32(norm_a_acc) as f64;
	let mut norm_b = vaddvq_f32(norm_b_acc) as f64;

	for i in (chunks * 4)..n {
		let ai = *a.get_unchecked(i) as f64;
		let bi = *b.get_unchecked(i) as f64;
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

#[cfg(target_arch = "aarch64")]
#[allow(clippy::undocumented_unsafe_blocks)]
unsafe fn neon_euclidean_distance(a: &[f32], b: &[f32]) -> f64 {
	use std::arch::aarch64::*;
	let n = a.len();
	let chunks = n / 4;
	let mut sum = vdupq_n_f32(0.0);

	for i in 0..chunks {
		let va = vld1q_f32(a.as_ptr().add(i * 4));
		let vb = vld1q_f32(b.as_ptr().add(i * 4));
		let diff = vsubq_f32(va, vb);
		sum = vfmaq_f32(sum, diff, diff);
	}

	let mut result = vaddvq_f32(sum) as f64;

	for i in (chunks * 4)..n {
		let diff = (*a.get_unchecked(i) as f64) - (*b.get_unchecked(i) as f64);
		result += diff * diff;
	}

	result.sqrt()
}

#[cfg(target_arch = "aarch64")]
#[allow(clippy::undocumented_unsafe_blocks)]
unsafe fn neon_manhattan_distance(a: &[f32], b: &[f32]) -> f64 {
	use std::arch::aarch64::*;
	let n = a.len();
	let chunks = n / 4;
	let mut sum = vdupq_n_f32(0.0);

	for i in 0..chunks {
		let va = vld1q_f32(a.as_ptr().add(i * 4));
		let vb = vld1q_f32(b.as_ptr().add(i * 4));
		let diff = vsubq_f32(va, vb);
		let abs_diff = vabsq_f32(diff);
		sum = vaddq_f32(sum, abs_diff);
	}

	let mut result = vaddvq_f32(sum) as f64;

	for i in (chunks * 4)..n {
		result += ((*a.get_unchecked(i) as f64) - (*b.get_unchecked(i) as f64)).abs();
	}

	result
}

// ---------------------------------------------------------------------------
// SIMD — public dispatch functions
// ---------------------------------------------------------------------------

/// SIMD-accelerated dot product with runtime feature detection.
///
/// Dispatches to AVX2+FMA on x86_64, NEON on aarch64, or falls back to scalar.
pub fn simd_dot_product(a: &[f32], b: &[f32]) -> f64 {
	if a.len() != b.len() || a.is_empty() {
		return 0.0;
	}

	#[cfg(target_arch = "x86_64")]
	{
		if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma") {
			// SAFETY: Feature detection guarantees AVX2+FMA availability.
			// Inputs are valid slices of equal length (checked above).
			return unsafe { avx2_dot_product(a, b) };
		}
	}

	#[cfg(target_arch = "aarch64")]
	{
		// SAFETY: NEON is mandatory on aarch64. Inputs are valid slices.
		return unsafe { neon_dot_product(a, b) };
	}

	// Scalar fallback for other architectures or missing features
	#[allow(unreachable_code)]
	dot_product_similarity(a, b)
}

/// SIMD-accelerated cosine similarity with runtime feature detection.
///
/// Returns a value in [-1.0, 1.0]. Dispatches to AVX2+FMA on x86_64,
/// NEON on aarch64, or falls back to scalar.
pub fn simd_cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
	if a.len() != b.len() || a.is_empty() {
		return 0.0;
	}

	#[cfg(target_arch = "x86_64")]
	{
		if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma") {
			// SAFETY: Feature detection guarantees AVX2+FMA availability.
			// Inputs are valid slices of equal length (checked above).
			return unsafe { avx2_cosine_similarity(a, b) };
		}
	}

	#[cfg(target_arch = "aarch64")]
	{
		// SAFETY: NEON is mandatory on aarch64. Inputs are valid slices.
		return unsafe { neon_cosine_similarity(a, b) };
	}

	#[allow(unreachable_code)]
	cosine_similarity_score(a, b)
}

/// SIMD-accelerated Euclidean distance with runtime feature detection.
///
/// Dispatches to AVX2+FMA on x86_64, NEON on aarch64, or falls back to scalar.
pub fn simd_euclidean_distance(a: &[f32], b: &[f32]) -> f64 {
	if a.len() != b.len() || a.is_empty() {
		return 0.0;
	}

	#[cfg(target_arch = "x86_64")]
	{
		if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma") {
			// SAFETY: Feature detection guarantees AVX2+FMA availability.
			// Inputs are valid slices of equal length (checked above).
			return unsafe { avx2_euclidean_distance(a, b) };
		}
	}

	#[cfg(target_arch = "aarch64")]
	{
		// SAFETY: NEON is mandatory on aarch64. Inputs are valid slices.
		return unsafe { neon_euclidean_distance(a, b) };
	}

	#[allow(unreachable_code)]
	euclidean_distance(a, b)
}

/// SIMD-accelerated Manhattan distance with runtime feature detection.
///
/// Dispatches to AVX2+FMA on x86_64, NEON on aarch64, or falls back to scalar.
pub fn simd_manhattan_distance(a: &[f32], b: &[f32]) -> f64 {
	if a.len() != b.len() || a.is_empty() {
		return 0.0;
	}

	#[cfg(target_arch = "x86_64")]
	{
		if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma") {
			// SAFETY: Feature detection guarantees AVX2+FMA availability.
			// Inputs are valid slices of equal length (checked above).
			return unsafe { avx2_manhattan_distance(a, b) };
		}
	}

	#[cfg(target_arch = "aarch64")]
	{
		// SAFETY: NEON is mandatory on aarch64. Inputs are valid slices.
		return unsafe { neon_manhattan_distance(a, b) };
	}

	#[allow(unreachable_code)]
	manhattan_distance(a, b)
}

/// SIMD-accelerated cosine distance: `1.0 - simd_cosine_similarity`.
///
/// Returns a value in [0.0, 2.0].
#[inline]
pub fn simd_cosine_distance(a: &[f32], b: &[f32]) -> f64 {
	1.0 - simd_cosine_similarity(a, b)
}

/// SIMD-accelerated dot product distance (negated dot product).
///
/// Negation makes it usable as a distance metric where lower = closer.
#[inline]
pub fn simd_dot_product_distance(a: &[f32], b: &[f32]) -> f64 {
	-simd_dot_product(a, b)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;

	// -- DistanceMetric enum -------------------------------------------------

	#[test]
	fn default_metric_is_cosine() {
		assert_eq!(DistanceMetric::default(), DistanceMetric::Cosine);
	}

	#[test]
	fn metric_serde_roundtrip() {
		let json = serde_json::to_string(&DistanceMetric::Euclidean).unwrap();
		assert_eq!(json, "\"euclidean\"");
		let parsed: DistanceMetric = serde_json::from_str(&json).unwrap();
		assert_eq!(parsed, DistanceMetric::Euclidean);
	}

	#[test]
	fn metric_serde_all_variants() {
		for (metric, expected) in [
			(DistanceMetric::Cosine, "\"cosine\""),
			(DistanceMetric::Euclidean, "\"euclidean\""),
			(DistanceMetric::DotProduct, "\"dot_product\""),
			(DistanceMetric::Manhattan, "\"manhattan\""),
		] {
			let json = serde_json::to_string(&metric).unwrap();
			assert_eq!(json, expected);
			let parsed: DistanceMetric = serde_json::from_str(&json).unwrap();
			assert_eq!(parsed, metric);
		}
	}

	#[test]
	fn distance_fn_returns_correct_function() {
		let a = vec![1.0f32, 0.0, 0.0];
		let b = vec![0.0f32, 1.0, 0.0];

		// Cosine distance for orthogonal vectors = 1.0
		let f = DistanceMetric::Cosine.distance_fn();
		assert!((f(&a, &b) - 1.0).abs() < 1e-10);

		// Euclidean distance for orthogonal unit vectors = sqrt(2)
		let f = DistanceMetric::Euclidean.distance_fn();
		assert!((f(&a, &b) - std::f64::consts::SQRT_2).abs() < 1e-10);
	}

	#[test]
	fn similarity_method_dispatches_correctly() {
		let v = vec![1.0f32, 2.0, 3.0];

		// Cosine similarity of identical vectors = 1.0
		assert!((DistanceMetric::Cosine.similarity(&v, &v) - 1.0).abs() < 1e-10);

		// Euclidean similarity of identical vectors = 1/(1+0) = 1.0
		assert!((DistanceMetric::Euclidean.similarity(&v, &v) - 1.0).abs() < 1e-10);

		// Manhattan similarity of identical vectors = 1/(1+0) = 1.0
		assert!((DistanceMetric::Manhattan.similarity(&v, &v) - 1.0).abs() < 1e-10);

		// DotProduct similarity of identical vectors = 1+4+9 = 14
		assert!((DistanceMetric::DotProduct.similarity(&v, &v) - 14.0).abs() < 1e-10);
	}

	// -- Cosine similarity ---------------------------------------------------

	#[test]
	fn cosine_identical_vectors() {
		let v = vec![1.0f32, 2.0, 3.0];
		let sim = cosine_similarity_score(&v, &v);
		assert!((sim - 1.0).abs() < 1e-10);
	}

	#[test]
	fn cosine_orthogonal_vectors() {
		let a = vec![1.0f32, 0.0];
		let b = vec![0.0f32, 1.0];
		assert!((cosine_similarity_score(&a, &b)).abs() < 1e-10);
	}

	#[test]
	fn cosine_opposite_vectors() {
		let a = vec![1.0f32, 0.0];
		let b = vec![-1.0f32, 0.0];
		assert!((cosine_similarity_score(&a, &b) + 1.0).abs() < 1e-10);
	}

	#[test]
	fn cosine_empty_vectors() {
		assert_eq!(cosine_similarity_score(&[], &[]), 0.0);
	}

	#[test]
	fn cosine_mismatched_lengths() {
		assert_eq!(cosine_similarity_score(&[1.0], &[1.0, 2.0]), 0.0);
	}

	#[test]
	fn cosine_zero_magnitude() {
		let a = vec![0.0f32, 0.0];
		let b = vec![1.0f32, 2.0];
		assert_eq!(cosine_similarity_score(&a, &b), 0.0);
	}

	#[test]
	fn cosine_clamped_to_range() {
		// Even with floating-point edge cases, result is in [-1, 1]
		let a = vec![1e38f32; 4];
		let b = vec![1e38f32; 4];
		let sim = cosine_similarity_score(&a, &b);
		assert!((-1.0..=1.0).contains(&sim));
	}

	// -- Cosine distance -----------------------------------------------------

	#[test]
	fn cosine_distance_identical() {
		let v = vec![1.0f32, 2.0, 3.0];
		assert!((cosine_distance(&v, &v)).abs() < 1e-10);
	}

	#[test]
	fn cosine_distance_orthogonal() {
		let a = vec![1.0f32, 0.0];
		let b = vec![0.0f32, 1.0];
		assert!((cosine_distance(&a, &b) - 1.0).abs() < 1e-10);
	}

	#[test]
	fn cosine_distance_opposite() {
		let a = vec![1.0f32, 0.0];
		let b = vec![-1.0f32, 0.0];
		assert!((cosine_distance(&a, &b) - 2.0).abs() < 1e-10);
	}

	// -- Magnitude -----------------------------------------------------------

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
	fn magnitude_unit_vector() {
		let v = vec![1.0f32, 0.0, 0.0];
		assert!((compute_magnitude(&v) - 1.0).abs() < 1e-10);
	}

	// -- Cosine with precomputed magnitude -----------------------------------

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

	#[test]
	fn cosine_with_precomputed_zero_magnitude() {
		let a = vec![1.0f32, 2.0];
		let b = vec![3.0f32, 4.0];
		assert_eq!(cosine_similarity_with_magnitude(&a, &b, 0.0, 5.0), 0.0);
		assert_eq!(cosine_similarity_with_magnitude(&a, &b, 5.0, 0.0), 0.0);
	}

	// -- Euclidean distance --------------------------------------------------

	#[test]
	fn euclidean_identical() {
		let v = vec![1.0f32, 2.0, 3.0];
		assert!((euclidean_distance(&v, &v)).abs() < 1e-10);
	}

	#[test]
	fn euclidean_known_distance() {
		let a = vec![0.0f32, 0.0];
		let b = vec![3.0f32, 4.0];
		assert!((euclidean_distance(&a, &b) - 5.0).abs() < 1e-10);
	}

	#[test]
	fn euclidean_unit_axes() {
		let a = vec![1.0f32, 0.0, 0.0];
		let b = vec![0.0f32, 1.0, 0.0];
		assert!((euclidean_distance(&a, &b) - std::f64::consts::SQRT_2).abs() < 1e-10);
	}

	#[test]
	fn euclidean_empty() {
		assert_eq!(euclidean_distance(&[], &[]), 0.0);
	}

	#[test]
	fn euclidean_mismatched() {
		assert_eq!(euclidean_distance(&[1.0], &[1.0, 2.0]), 0.0);
	}

	#[test]
	fn euclidean_similarity_identical() {
		let v = vec![1.0f32, 2.0, 3.0];
		let sim = DistanceMetric::Euclidean.similarity(&v, &v);
		assert!((sim - 1.0).abs() < 1e-10);
	}

	#[test]
	fn euclidean_similarity_known() {
		let a = vec![0.0f32, 0.0];
		let b = vec![3.0f32, 4.0];
		let sim = DistanceMetric::Euclidean.similarity(&a, &b);
		// 1/(1+5) = 1/6
		assert!((sim - 1.0 / 6.0).abs() < 1e-10);
	}

	// -- Dot product ---------------------------------------------------------

	#[test]
	fn dot_product_basic() {
		let a = vec![1.0f32, 2.0, 3.0];
		let b = vec![4.0f32, 5.0, 6.0];
		// 1*4 + 2*5 + 3*6 = 32
		assert!((dot_product_similarity(&a, &b) - 32.0).abs() < 1e-10);
	}

	#[test]
	fn dot_product_orthogonal() {
		let a = vec![1.0f32, 0.0];
		let b = vec![0.0f32, 1.0];
		assert!((dot_product_similarity(&a, &b)).abs() < 1e-10);
	}

	#[test]
	fn dot_product_empty() {
		assert_eq!(dot_product_similarity(&[], &[]), 0.0);
	}

	#[test]
	fn dot_product_mismatched() {
		assert_eq!(dot_product_similarity(&[1.0], &[1.0, 2.0]), 0.0);
	}

	#[test]
	fn dot_product_distance_is_negated() {
		let a = vec![1.0f32, 2.0, 3.0];
		let b = vec![4.0f32, 5.0, 6.0];
		let sim = dot_product_similarity(&a, &b);
		let dist = dot_product_distance(&a, &b);
		assert!((dist + sim).abs() < 1e-10);
	}

	#[test]
	fn dot_product_normalized_equals_cosine() {
		// For unit vectors, dot product == cosine similarity
		let a = vec![1.0f32, 0.0, 0.0];
		let b = vec![0.0f32, 1.0, 0.0];
		let dp = dot_product_similarity(&a, &b);
		let cs = cosine_similarity_score(&a, &b);
		assert!((dp - cs).abs() < 1e-10);
	}

	// -- Manhattan distance --------------------------------------------------

	#[test]
	fn manhattan_identical() {
		let v = vec![1.0f32, 2.0, 3.0];
		assert!((manhattan_distance(&v, &v)).abs() < 1e-10);
	}

	#[test]
	fn manhattan_known_distance() {
		let a = vec![0.0f32, 0.0];
		let b = vec![3.0f32, 4.0];
		// |3-0| + |4-0| = 7
		assert!((manhattan_distance(&a, &b) - 7.0).abs() < 1e-10);
	}

	#[test]
	fn manhattan_negative_values() {
		let a = vec![-1.0f32, -2.0];
		let b = vec![1.0f32, 2.0];
		// |(-1)-1| + |(-2)-2| = 2 + 4 = 6
		assert!((manhattan_distance(&a, &b) - 6.0).abs() < 1e-10);
	}

	#[test]
	fn manhattan_empty() {
		assert_eq!(manhattan_distance(&[], &[]), 0.0);
	}

	#[test]
	fn manhattan_mismatched() {
		assert_eq!(manhattan_distance(&[1.0], &[1.0, 2.0]), 0.0);
	}

	#[test]
	fn manhattan_similarity_identical() {
		let v = vec![1.0f32, 2.0, 3.0];
		let sim = DistanceMetric::Manhattan.similarity(&v, &v);
		assert!((sim - 1.0).abs() < 1e-10);
	}

	#[test]
	fn manhattan_similarity_known() {
		let a = vec![0.0f32, 0.0];
		let b = vec![3.0f32, 4.0];
		let sim = DistanceMetric::Manhattan.similarity(&a, &b);
		// 1/(1+7) = 1/8 = 0.125
		assert!((sim - 0.125).abs() < 1e-10);
	}

	// -- Backward compat alias -----------------------------------------------

	#[test]
	fn cosine_similarity_alias_works() {
		let v = vec![1.0f32, 2.0, 3.0];
		let a = cosine_similarity(&v, &v);
		let b = cosine_similarity_score(&v, &v);
		assert_eq!(a, b);
	}

	// -- Cross-metric properties --------------------------------------------

	#[test]
	fn all_metrics_handle_empty_vectors() {
		// Euclidean, DotProduct, Manhattan return 0.0 distance for empty vectors
		for metric in [
			DistanceMetric::Euclidean,
			DistanceMetric::DotProduct,
			DistanceMetric::Manhattan,
		] {
			let f = metric.distance_fn();
			assert_eq!(f(&[], &[]), 0.0, "distance_fn failed for {:?}", metric);
		}
		// Cosine distance for empty vectors = 1 - 0 = 1.0
		// (cosine_similarity returns 0.0 for empty)
		let f = DistanceMetric::Cosine.distance_fn();
		assert!((f(&[], &[]) - 1.0).abs() < 1e-10);
	}

	#[test]
	fn all_metrics_return_zero_distance_for_identical() {
		let v = vec![1.0f32, 2.0, 3.0];
		// Cosine distance = 0, Euclidean = 0, Manhattan = 0
		assert!((cosine_distance(&v, &v)).abs() < 1e-10);
		assert!((euclidean_distance(&v, &v)).abs() < 1e-10);
		assert!((manhattan_distance(&v, &v)).abs() < 1e-10);
		// DotProduct distance is negative for same-direction vectors
		assert!(dot_product_distance(&v, &v) < 0.0);
	}

	// -- SIMD vs scalar agreement -------------------------------------------

	#[test]
	fn simd_dot_matches_scalar() {
		let a: Vec<f32> = (0..384).map(|i| (i as f32) * 0.01).collect();
		let b: Vec<f32> = (0..384).map(|i| ((384 - i) as f32) * 0.01).collect();
		let scalar = dot_product_similarity(&a, &b);
		let simd = simd_dot_product(&a, &b);
		assert!((scalar - simd).abs() < 1e-3, "scalar={scalar} simd={simd}");
	}

	#[test]
	fn simd_cosine_matches_scalar() {
		let a: Vec<f32> = (0..384).map(|i| (i as f32) * 0.01).collect();
		let b: Vec<f32> = (0..384).map(|i| ((384 - i) as f32) * 0.01).collect();
		let scalar = cosine_similarity_score(&a, &b);
		let simd = simd_cosine_similarity(&a, &b);
		assert!((scalar - simd).abs() < 1e-5, "scalar={scalar} simd={simd}");
	}

	#[test]
	fn simd_euclidean_matches_scalar() {
		let a: Vec<f32> = (0..384).map(|i| (i as f32) * 0.01).collect();
		let b: Vec<f32> = (0..384).map(|i| ((384 - i) as f32) * 0.01).collect();
		let scalar = euclidean_distance(&a, &b);
		let simd = simd_euclidean_distance(&a, &b);
		assert!((scalar - simd).abs() < 1e-3, "scalar={scalar} simd={simd}");
	}

	#[test]
	fn simd_manhattan_matches_scalar() {
		let a: Vec<f32> = (0..384).map(|i| (i as f32) * 0.01).collect();
		let b: Vec<f32> = (0..384).map(|i| ((384 - i) as f32) * 0.01).collect();
		let scalar = manhattan_distance(&a, &b);
		let simd = simd_manhattan_distance(&a, &b);
		assert!((scalar - simd).abs() < 1e-3, "scalar={scalar} simd={simd}");
	}

	#[test]
	fn simd_short_vector() {
		let a = vec![1.0f32, 2.0];
		let b = vec![3.0f32, 4.0];
		let scalar = cosine_similarity_score(&a, &b);
		let simd = simd_cosine_similarity(&a, &b);
		assert!((scalar - simd).abs() < 1e-6);
	}

	#[test]
	fn simd_large_vector() {
		let a: Vec<f32> = (0..1536).map(|i| (i as f32) * 0.001).collect();
		let b: Vec<f32> = (0..1536).map(|i| ((1536 - i) as f32) * 0.001).collect();
		let scalar = cosine_similarity_score(&a, &b);
		let simd = simd_cosine_similarity(&a, &b);
		assert!((scalar - simd).abs() < 1e-4, "scalar={scalar} simd={simd}");
	}

	#[test]
	fn simd_empty_vectors() {
		assert_eq!(simd_dot_product(&[], &[]), 0.0);
		assert_eq!(simd_cosine_similarity(&[], &[]), 0.0);
		assert_eq!(simd_euclidean_distance(&[], &[]), 0.0);
		assert_eq!(simd_manhattan_distance(&[], &[]), 0.0);
	}

	#[test]
	fn simd_mismatched_lengths() {
		assert_eq!(simd_dot_product(&[1.0], &[1.0, 2.0]), 0.0);
		assert_eq!(simd_cosine_similarity(&[1.0], &[1.0, 2.0]), 0.0);
		assert_eq!(simd_euclidean_distance(&[1.0], &[1.0, 2.0]), 0.0);
		assert_eq!(simd_manhattan_distance(&[1.0], &[1.0, 2.0]), 0.0);
	}

	#[test]
	fn simd_cosine_distance_wrapper() {
		let a: Vec<f32> = (0..384).map(|i| (i as f32) * 0.01).collect();
		let b: Vec<f32> = (0..384).map(|i| ((384 - i) as f32) * 0.01).collect();
		let sim = simd_cosine_similarity(&a, &b);
		let dist = simd_cosine_distance(&a, &b);
		assert!((dist - (1.0 - sim)).abs() < 1e-10);
	}

	#[test]
	fn simd_dot_product_distance_wrapper() {
		let a: Vec<f32> = (0..384).map(|i| (i as f32) * 0.01).collect();
		let b: Vec<f32> = (0..384).map(|i| ((384 - i) as f32) * 0.01).collect();
		let sim = simd_dot_product(&a, &b);
		let dist = simd_dot_product_distance(&a, &b);
		assert!((dist + sim).abs() < 1e-10);
	}

	#[test]
	fn simd_identical_vectors() {
		let v: Vec<f32> = (0..256).map(|i| (i as f32) * 0.1).collect();
		// Cosine similarity of identical vectors should be ~1.0
		let sim = simd_cosine_similarity(&v, &v);
		assert!((sim - 1.0).abs() < 1e-5, "cosine sim of identical={sim}");
		// Euclidean distance of identical vectors should be ~0.0
		let dist = simd_euclidean_distance(&v, &v);
		assert!(dist.abs() < 1e-3, "euclidean dist of identical={dist}");
		// Manhattan distance of identical vectors should be ~0.0
		let dist = simd_manhattan_distance(&v, &v);
		assert!(dist.abs() < 1e-3, "manhattan dist of identical={dist}");
	}

	// -- DistanceMetric dispatches to SIMD ----------------------------------

	#[test]
	fn distance_metric_uses_simd_paths() {
		let a: Vec<f32> = (0..384).map(|i| (i as f32) * 0.01).collect();
		let b: Vec<f32> = (0..384).map(|i| ((384 - i) as f32) * 0.01).collect();

		// Verify DistanceMetric methods agree with direct SIMD calls
		let sim = DistanceMetric::Cosine.similarity(&a, &b);
		let simd = simd_cosine_similarity(&a, &b);
		assert!((sim - simd).abs() < 1e-10);

		let f = DistanceMetric::Euclidean.distance_fn();
		let simd = simd_euclidean_distance(&a, &b);
		assert!((f(&a, &b) - simd).abs() < 1e-10);

		let sim = DistanceMetric::DotProduct.similarity(&a, &b);
		let simd = simd_dot_product(&a, &b);
		assert!((sim - simd).abs() < 1e-10);

		let f = DistanceMetric::Manhattan.distance_fn();
		let simd = simd_manhattan_distance(&a, &b);
		assert!((f(&a, &b) - simd).abs() < 1e-10);
	}
}
