// ---------------------------------------------------------------------------
// Quantization — scalar and binary vector quantization
// ---------------------------------------------------------------------------
//
// Provides two quantization strategies for compressing f32 vectors:
//
// - **Scalar quantization** (ScalarQuantizer): maps each f32 dimension to a
//   u8 value in [0, 255], achieving 4x compression. Approximate similarity
//   is computed by decoding back to f32 first.
//
// - **Binary quantization** (BinaryQuantizer): sign-bit quantization that
//   packs each dimension into a single bit (positive -> 1, non-positive -> 0),
//   achieving 32x compression. Similarity uses Hamming distance with hardware
//   popcnt via `count_ones()`.
// ---------------------------------------------------------------------------

use serde::{Deserialize, Serialize};

use crate::adaptive::distance::{cosine_similarity_score, euclidean_distance};

// ---------------------------------------------------------------------------
// Quantization enum
// ---------------------------------------------------------------------------

/// Quantization strategy for stored vectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Quantization {
	#[default]
	None,
	Scalar,
	Binary,
}

// ---------------------------------------------------------------------------
// Scalar quantization — f32 to u8 (4x compression)
// ---------------------------------------------------------------------------

/// A scalar-quantized vector: each f32 dimension mapped to u8 in [0, 255].
#[derive(Debug, Clone)]
pub struct ScalarQuantized {
	/// Quantized data, one u8 per dimension.
	pub data: Vec<u8>,
	/// Minimum value across all dimensions before quantization.
	pub min: f32,
	/// Scale factor: `(max - min) / 255.0`. If all values are identical,
	/// scale is set to 1.0 to avoid division by zero.
	pub scale: f32,
	/// Number of original dimensions.
	pub dimensions: usize,
}

/// Scalar quantizer: fit min/max from input, then encode to u8.
pub struct ScalarQuantizer;

impl ScalarQuantizer {
	/// Find min/max across all dimensions, map each value to [0, 255].
	///
	/// For uniform vectors (min == max), all values encode to 0 and scale
	/// is set to 1.0.
	pub fn fit_encode(values: &[f32]) -> ScalarQuantized {
		if values.is_empty() {
			return ScalarQuantized {
				data: Vec::new(),
				min: 0.0,
				scale: 1.0,
				dimensions: 0,
			};
		}

		let mut min_val = f32::INFINITY;
		let mut max_val = f32::NEG_INFINITY;
		for &v in values {
			if v < min_val {
				min_val = v;
			}
			if v > max_val {
				max_val = v;
			}
		}

		let range = max_val - min_val;
		let scale = if range == 0.0 { 1.0 } else { range / 255.0 };

		let mut data = Vec::with_capacity(values.len());
		for &v in values {
			if range == 0.0 {
				data.push(0);
			} else {
				let normalized = ((v - min_val) / scale).round();
				data.push(normalized.clamp(0.0, 255.0) as u8);
			}
		}

		ScalarQuantized {
			data,
			min: min_val,
			scale,
			dimensions: values.len(),
		}
	}
}

impl ScalarQuantized {
	/// Decode back to f32 (lossy).
	///
	/// Each u8 value is mapped back: `min + (value as f32) * scale`.
	pub fn decode(&self) -> Vec<f32> {
		self.data
			.iter()
			.map(|&v| self.min + (v as f32) * self.scale)
			.collect()
	}

	/// Approximate cosine similarity between two scalar-quantized vectors.
	///
	/// Decodes both vectors and computes exact cosine on the decoded values.
	/// Returns 0.0 if dimensions mismatch.
	pub fn approximate_cosine(&self, other: &ScalarQuantized) -> f64 {
		if self.dimensions != other.dimensions {
			return 0.0;
		}
		let a = self.decode();
		let b = other.decode();
		cosine_similarity_score(&a, &b)
	}

	/// Approximate Euclidean (L2) distance between two scalar-quantized vectors.
	///
	/// Decodes both vectors and computes exact Euclidean distance on the
	/// decoded values. Returns 0.0 if dimensions mismatch.
	pub fn approximate_euclidean(&self, other: &ScalarQuantized) -> f64 {
		if self.dimensions != other.dimensions {
			return 0.0;
		}
		let a = self.decode();
		let b = other.decode();
		euclidean_distance(&a, &b)
	}
}

// ---------------------------------------------------------------------------
// Binary quantization — f32 to 1-bit sign (32x compression)
// ---------------------------------------------------------------------------

/// Binary quantizer: sign-bit quantization packed into u64 words.
pub struct BinaryQuantizer;

impl BinaryQuantizer {
	/// Sign-bit quantization: positive -> 1, negative/zero -> 0.
	///
	/// Packed into u64 words (64 dimensions per word). If the number of
	/// dimensions is not a multiple of 64, the last word has unused high bits
	/// set to 0.
	pub fn encode(values: &[f32]) -> Vec<u64> {
		if values.is_empty() {
			return Vec::new();
		}

		let num_words = values.len().div_ceil(64);
		let mut result = vec![0u64; num_words];

		for (i, &v) in values.iter().enumerate() {
			if v > 0.0 {
				let word_idx = i / 64;
				let bit_idx = i % 64;
				result[word_idx] |= 1u64 << bit_idx;
			}
		}

		result
	}

	/// Hamming distance between two binary-quantized vectors.
	///
	/// Counts the number of differing bits using `count_ones()`, which maps
	/// to hardware `popcnt` on x86 and `vcnt` on ARM.
	///
	/// If the slices have different lengths, only the overlapping portion is
	/// compared; extra words in the longer slice contribute all their set bits
	/// to the distance.
	pub fn hamming_distance(a: &[u64], b: &[u64]) -> u32 {
		let min_len = a.len().min(b.len());
		let max_len = a.len().max(b.len());
		let mut dist: u32 = 0;

		for i in 0..min_len {
			dist += (a[i] ^ b[i]).count_ones();
		}

		// Extra words in the longer slice: every set bit is a difference.
		let longer = if a.len() > b.len() { a } else { b };
		for i in min_len..max_len {
			dist += longer[i].count_ones();
		}

		dist
	}

	/// Hamming similarity: `1.0 - (hamming_distance / total_dims)`.
	///
	/// Returns a value in [0.0, 1.0] where 1.0 means identical and 0.0 means
	/// fully opposite. `total_dims` is the original number of dimensions
	/// (not the number of u64 words).
	///
	/// Returns 1.0 if `total_dims` is 0 (vacuously identical).
	pub fn hamming_similarity(a: &[u64], b: &[u64], total_dims: usize) -> f64 {
		if total_dims == 0 {
			return 1.0;
		}
		let dist = Self::hamming_distance(a, b) as f64;
		1.0 - (dist / total_dims as f64)
	}
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;

	// -- Quantization enum ---------------------------------------------------

	#[test]
	fn quantization_default_is_none() {
		assert_eq!(Quantization::default(), Quantization::None);
	}

	#[test]
	fn quantization_serde_roundtrip() {
		for (variant, expected_json) in [
			(Quantization::None, "\"none\""),
			(Quantization::Scalar, "\"scalar\""),
			(Quantization::Binary, "\"binary\""),
		] {
			let json = serde_json::to_string(&variant).unwrap();
			assert_eq!(json, expected_json, "serialize {:?}", variant);
			let parsed: Quantization = serde_json::from_str(&json).unwrap();
			assert_eq!(parsed, variant, "deserialize {:?}", variant);
		}
	}

	// -- Scalar: encode/decode roundtrip -------------------------------------

	#[test]
	fn scalar_encode_decode_roundtrip() {
		let values = vec![0.0, 0.5, 1.0, -1.0, 0.25, -0.75, 0.9, -0.3];
		let quantized = ScalarQuantizer::fit_encode(&values);

		assert_eq!(quantized.dimensions, values.len());
		assert_eq!(quantized.data.len(), values.len());

		let decoded = quantized.decode();
		assert_eq!(decoded.len(), values.len());

		for (i, (&original, &restored)) in values.iter().zip(decoded.iter()).enumerate() {
			assert!(
				(original - restored).abs() < 0.01,
				"dimension {} differs: original={}, decoded={}",
				i,
				original,
				restored
			);
		}
	}

	#[test]
	fn scalar_encode_decode_wide_range() {
		let values = vec![-100.0, -50.0, 0.0, 50.0, 100.0];
		let quantized = ScalarQuantizer::fit_encode(&values);
		let decoded = quantized.decode();

		for (i, (&original, &restored)) in values.iter().zip(decoded.iter()).enumerate() {
			// Wide range: 200 / 255 ~ 0.784 per step, so tolerance ~1.0
			assert!(
				(original - restored).abs() < 1.0,
				"dimension {} differs: original={}, decoded={}",
				i,
				original,
				restored
			);
		}
	}

	// -- Scalar: approximate cosine ------------------------------------------

	#[test]
	fn scalar_approximate_cosine_matches_exact() {
		let a = vec![1.0, 2.0, 3.0, 4.0, 5.0];
		let b = vec![5.0, 4.0, 3.0, 2.0, 1.0];

		let exact = cosine_similarity_score(&a, &b);

		let qa = ScalarQuantizer::fit_encode(&a);
		let qb = ScalarQuantizer::fit_encode(&b);
		let approx = qa.approximate_cosine(&qb);

		assert!(
			(exact - approx).abs() < 0.05,
			"exact={}, approx={}, diff={}",
			exact,
			approx,
			(exact - approx).abs()
		);
	}

	#[test]
	fn scalar_approximate_cosine_identical_vectors() {
		let v = vec![1.0, 2.0, 3.0];
		let qa = ScalarQuantizer::fit_encode(&v);
		let qb = ScalarQuantizer::fit_encode(&v);
		let sim = qa.approximate_cosine(&qb);
		assert!(
			(sim - 1.0).abs() < 0.05,
			"identical vectors should have cosine ~1.0, got {}",
			sim
		);
	}

	#[test]
	fn scalar_approximate_cosine_dimension_mismatch() {
		let qa = ScalarQuantizer::fit_encode(&[1.0, 2.0]);
		let qb = ScalarQuantizer::fit_encode(&[1.0, 2.0, 3.0]);
		assert_eq!(qa.approximate_cosine(&qb), 0.0);
	}

	// -- Scalar: approximate Euclidean ---------------------------------------

	#[test]
	fn scalar_approximate_euclidean_reasonable_accuracy() {
		let a = vec![0.0, 0.0, 0.0];
		let b = vec![3.0, 4.0, 0.0];

		let exact = euclidean_distance(&a, &b);

		let qa = ScalarQuantizer::fit_encode(&a);
		let qb = ScalarQuantizer::fit_encode(&b);
		let approx = qa.approximate_euclidean(&qb);

		// Euclidean distance is 5.0; allow reasonable tolerance
		assert!(
			(exact - approx).abs() < 0.5,
			"exact={}, approx={}, diff={}",
			exact,
			approx,
			(exact - approx).abs()
		);
	}

	#[test]
	fn scalar_approximate_euclidean_identical() {
		let v = vec![1.0, 2.0, 3.0];
		let qa = ScalarQuantizer::fit_encode(&v);
		let qb = ScalarQuantizer::fit_encode(&v);
		let dist = qa.approximate_euclidean(&qb);
		assert!(
			dist < 0.1,
			"identical vectors should have distance ~0, got {}",
			dist
		);
	}

	#[test]
	fn scalar_approximate_euclidean_dimension_mismatch() {
		let qa = ScalarQuantizer::fit_encode(&[1.0, 2.0]);
		let qb = ScalarQuantizer::fit_encode(&[1.0, 2.0, 3.0]);
		assert_eq!(qa.approximate_euclidean(&qb), 0.0);
	}

	// -- Scalar: edge cases --------------------------------------------------

	#[test]
	fn scalar_uniform_values() {
		// All values identical: min == max, scale = 1.0
		let values = vec![5.0, 5.0, 5.0, 5.0];
		let quantized = ScalarQuantizer::fit_encode(&values);

		assert_eq!(quantized.scale, 1.0);
		// All encoded values should be 0 (no spread)
		for &v in &quantized.data {
			assert_eq!(v, 0, "uniform values should encode to 0");
		}

		let decoded = quantized.decode();
		for (i, &d) in decoded.iter().enumerate() {
			assert!(
				(d - 5.0).abs() < 0.01,
				"uniform decode dim {} = {}, expected 5.0",
				i,
				d
			);
		}
	}

	#[test]
	fn scalar_single_dimension() {
		let values = vec![42.0];
		let quantized = ScalarQuantizer::fit_encode(&values);
		assert_eq!(quantized.dimensions, 1);
		assert_eq!(quantized.data.len(), 1);

		let decoded = quantized.decode();
		assert!(
			(decoded[0] - 42.0).abs() < 0.01,
			"single dim decoded to {}, expected 42.0",
			decoded[0]
		);
	}

	#[test]
	fn scalar_empty_vector() {
		let quantized = ScalarQuantizer::fit_encode(&[]);
		assert_eq!(quantized.dimensions, 0);
		assert_eq!(quantized.data.len(), 0);
		assert!(quantized.decode().is_empty());
	}

	#[test]
	fn scalar_min_max_boundaries() {
		// Minimum value should encode to 0, maximum to 255
		let values = vec![-10.0, 10.0];
		let quantized = ScalarQuantizer::fit_encode(&values);
		assert_eq!(quantized.data[0], 0);
		assert_eq!(quantized.data[1], 255);
	}

	// -- Binary: encode bit patterns -----------------------------------------

	#[test]
	fn binary_encode_known_patterns() {
		// First 4 values: positive, negative, positive, zero
		// Bits (LSB first): 1, 0, 1, 0 -> 0b0101 = 5
		let values = vec![1.0, -1.0, 0.5, 0.0];
		let encoded = BinaryQuantizer::encode(&values);
		assert_eq!(encoded.len(), 1);
		assert_eq!(encoded[0], 0b0101);
	}

	#[test]
	fn binary_encode_all_positive() {
		let values = vec![1.0; 64];
		let encoded = BinaryQuantizer::encode(&values);
		assert_eq!(encoded.len(), 1);
		assert_eq!(encoded[0], u64::MAX);
	}

	#[test]
	fn binary_encode_all_negative() {
		let values = vec![-1.0; 64];
		let encoded = BinaryQuantizer::encode(&values);
		assert_eq!(encoded.len(), 1);
		assert_eq!(encoded[0], 0);
	}

	#[test]
	fn binary_encode_all_zero() {
		// Zero is treated as non-positive -> bit 0
		let values = vec![0.0; 64];
		let encoded = BinaryQuantizer::encode(&values);
		assert_eq!(encoded.len(), 1);
		assert_eq!(encoded[0], 0);
	}

	#[test]
	fn binary_encode_empty() {
		let encoded = BinaryQuantizer::encode(&[]);
		assert!(encoded.is_empty());
	}

	// -- Binary: hamming distance --------------------------------------------

	#[test]
	fn binary_hamming_identical() {
		let a = vec![0b1010_1010u64, 0b1111_0000];
		let dist = BinaryQuantizer::hamming_distance(&a, &a);
		assert_eq!(dist, 0);
	}

	#[test]
	fn binary_hamming_known_distance() {
		// Differ in exactly 3 bits in the first word
		let a = vec![0b0000_0000u64];
		let b = vec![0b0000_0111u64];
		assert_eq!(BinaryQuantizer::hamming_distance(&a, &b), 3);
	}

	#[test]
	fn binary_hamming_all_differ() {
		let a = vec![0u64];
		let b = vec![u64::MAX];
		assert_eq!(BinaryQuantizer::hamming_distance(&a, &b), 64);
	}

	#[test]
	fn binary_hamming_different_lengths() {
		// a has 1 word, b has 2 words. Extra word in b has 8 set bits.
		let a = vec![0u64];
		let b = vec![0u64, 0xFF];
		let dist = BinaryQuantizer::hamming_distance(&a, &b);
		assert_eq!(dist, 8, "extra word's set bits should count");
	}

	#[test]
	fn binary_hamming_empty() {
		assert_eq!(BinaryQuantizer::hamming_distance(&[], &[]), 0);
	}

	#[test]
	fn binary_hamming_one_empty() {
		let a = vec![0b1111u64];
		assert_eq!(BinaryQuantizer::hamming_distance(&a, &[]), 4);
		assert_eq!(BinaryQuantizer::hamming_distance(&[], &a), 4);
	}

	// -- Binary: hamming similarity ------------------------------------------

	#[test]
	fn binary_hamming_similarity_identical() {
		let v = vec![1.0, -1.0, 0.5, -0.3, 0.0, 2.0];
		let encoded = BinaryQuantizer::encode(&v);
		let sim = BinaryQuantizer::hamming_similarity(&encoded, &encoded, v.len());
		assert!(
			(sim - 1.0).abs() < 1e-10,
			"identical vectors should have similarity 1.0, got {}",
			sim
		);
	}

	#[test]
	fn binary_hamming_similarity_fully_opposite() {
		// All positive vs all negative: every bit differs
		let a_vals = vec![1.0; 64];
		let b_vals = vec![-1.0; 64];
		let a = BinaryQuantizer::encode(&a_vals);
		let b = BinaryQuantizer::encode(&b_vals);
		let sim = BinaryQuantizer::hamming_similarity(&a, &b, 64);
		assert!(
			sim.abs() < 1e-10,
			"fully opposite should have similarity 0.0, got {}",
			sim
		);
	}

	#[test]
	fn binary_hamming_similarity_half_differ() {
		// 64 dims: first 32 positive, last 32 negative
		let mut a_vals = vec![1.0; 32];
		a_vals.extend(vec![-1.0; 32]);

		// Opposite pattern
		let mut b_vals = vec![-1.0; 32];
		b_vals.extend(vec![1.0; 32]);

		let a = BinaryQuantizer::encode(&a_vals);
		let b = BinaryQuantizer::encode(&b_vals);
		let sim = BinaryQuantizer::hamming_similarity(&a, &b, 64);
		assert!(
			(sim - 0.0).abs() < 1e-10,
			"all bits differ, similarity should be 0.0, got {}",
			sim
		);
	}

	#[test]
	fn binary_hamming_similarity_zero_dims() {
		let sim = BinaryQuantizer::hamming_similarity(&[], &[], 0);
		assert!((sim - 1.0).abs() < 1e-10);
	}

	// -- Binary: non-multiple-of-64 dimensions -------------------------------

	#[test]
	fn binary_non_multiple_of_64_dimensions() {
		// 100 dimensions: needs 2 u64 words (ceil(100/64) = 2)
		let mut values = vec![1.0f32; 100];
		// Make the last 36 negative (dims 64..99)
		for v in values[64..].iter_mut() {
			*v = -1.0;
		}

		let encoded = BinaryQuantizer::encode(&values);
		assert_eq!(encoded.len(), 2, "100 dims should need 2 u64 words");

		// First word: all 64 bits set (dims 0..63 are positive)
		assert_eq!(encoded[0], u64::MAX);

		// Second word: dims 64..99 are negative (0), unused bits 36..63 are also 0
		assert_eq!(encoded[1], 0);
	}

	#[test]
	fn binary_non_multiple_of_64_hamming() {
		// 70 dimensions: 2 words
		let a_vals: Vec<f32> = (0..70).map(|i| if i % 2 == 0 { 1.0 } else { -1.0 }).collect();
		let b_vals: Vec<f32> = (0..70).map(|i| if i % 2 == 0 { -1.0 } else { 1.0 }).collect();

		let a = BinaryQuantizer::encode(&a_vals);
		let b = BinaryQuantizer::encode(&b_vals);

		// All 70 meaningful bits differ
		let dist = BinaryQuantizer::hamming_distance(&a, &b);
		assert_eq!(dist, 70, "all 70 dimensions should differ");

		let sim = BinaryQuantizer::hamming_similarity(&a, &b, 70);
		assert!(
			sim.abs() < 1e-10,
			"all bits differ, similarity should be 0.0, got {}",
			sim
		);
	}

	#[test]
	fn binary_65_dimensions() {
		// 65 dims -> 2 words, second word has only bit 0 used
		let mut values = vec![-1.0f32; 65];
		values[64] = 1.0; // Only the 65th dimension is positive

		let encoded = BinaryQuantizer::encode(&values);
		assert_eq!(encoded.len(), 2);
		assert_eq!(encoded[0], 0); // All negative
		assert_eq!(encoded[1], 1); // Only bit 0 of second word

		// Hamming distance against all-negative
		let all_neg = BinaryQuantizer::encode(&vec![-1.0f32; 65]);
		assert_eq!(BinaryQuantizer::hamming_distance(&encoded, &all_neg), 1);
	}

	// -- Scalar: high-dimensional roundtrip ----------------------------------

	#[test]
	fn scalar_high_dimensional() {
		// 384-dim vector (common embedding size)
		let values: Vec<f32> = (0..384).map(|i| (i as f32 / 384.0) * 2.0 - 1.0).collect();
		let quantized = ScalarQuantizer::fit_encode(&values);
		assert_eq!(quantized.dimensions, 384);

		let decoded = quantized.decode();
		for (i, (&original, &restored)) in values.iter().zip(decoded.iter()).enumerate() {
			assert!(
				(original - restored).abs() < 0.01,
				"dim {} differs: original={}, decoded={}",
				i,
				original,
				restored
			);
		}
	}

	// -- Scalar: cosine with orthogonal-ish vectors --------------------------

	#[test]
	fn scalar_approximate_cosine_orthogonal() {
		// Nearly orthogonal vectors
		let a = vec![1.0, 0.0, 0.0, 0.0];
		let b = vec![0.0, 1.0, 0.0, 0.0];

		let exact = cosine_similarity_score(&a, &b);

		let qa = ScalarQuantizer::fit_encode(&a);
		let qb = ScalarQuantizer::fit_encode(&b);
		let approx = qa.approximate_cosine(&qb);

		assert!(
			(exact - approx).abs() < 0.05,
			"exact={}, approx={}",
			exact,
			approx
		);
	}
}
