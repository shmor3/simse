// ---------------------------------------------------------------------------
// Recommendation Engine — scoring functions for memory recommendations
// ---------------------------------------------------------------------------
//
// Pure functions for computing recommendation scores combining vector
// similarity, recency, and access frequency. No side effects.
// ---------------------------------------------------------------------------

use crate::types::{RequiredWeightProfile, WeightProfile};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Default half-life for recency decay: 30 days in milliseconds.
const DEFAULT_HALF_LIFE_MS: f64 = 30.0 * 24.0 * 60.0 * 60.0 * 1000.0;

// ---------------------------------------------------------------------------
// Weight normalization
// ---------------------------------------------------------------------------

/// Normalize a partial weight profile so all components sum to 1.
/// Missing weights use defaults (0.6, 0.2, 0.2), then the whole profile
/// is scaled so the sum is exactly 1.0.
pub fn normalize_weights(weights: &Option<WeightProfile>) -> RequiredWeightProfile {
	let raw_v = weights
		.as_ref()
		.and_then(|w| w.vector)
		.unwrap_or(0.6);
	let raw_r = weights
		.as_ref()
		.and_then(|w| w.recency)
		.unwrap_or(0.2);
	let raw_f = weights
		.as_ref()
		.and_then(|w| w.frequency)
		.unwrap_or(0.2);
	let total = raw_v + raw_r + raw_f;
	if total == 0.0 {
		return RequiredWeightProfile {
			vector: 0.6,
			recency: 0.2,
			frequency: 0.2,
		};
	}
	RequiredWeightProfile {
		vector: raw_v / total,
		recency: raw_r / total,
		frequency: raw_f / total,
	}
}

// ---------------------------------------------------------------------------
// Individual scoring functions
// ---------------------------------------------------------------------------

/// Compute a recency score using exponential decay.
///
/// Returns a value between 0 and 1. Entries at `now` get 1.0, entries
/// at `half_life_ms` ago get ~0.5, older entries approach 0.
pub fn recency_score(entry_timestamp: u64, half_life_ms: f64, now: u64) -> f64 {
	let age_ms = if now > entry_timestamp {
		(now - entry_timestamp) as f64
	} else {
		0.0
	};
	let lambda = f64::ln(2.0) / half_life_ms;
	(-lambda * age_ms).exp()
}

/// Compute a recency score using the default half-life (30 days).
pub fn recency_score_default(entry_timestamp: u64, now: u64) -> f64 {
	recency_score(entry_timestamp, DEFAULT_HALF_LIFE_MS, now)
}

/// Compute a frequency score using logarithmic scaling.
///
/// Returns a value between 0 and 1. `log(1 + count) / log(1 + max)`
/// ensures diminishing returns for very high access counts.
pub fn frequency_score(access_count: usize, max_access_count: usize) -> f64 {
	if max_access_count == 0 {
		return 0.0;
	}
	(1.0 + access_count as f64).ln() / (1.0 + max_access_count as f64).ln()
}

// ---------------------------------------------------------------------------
// Combined recommendation score
// ---------------------------------------------------------------------------

/// Input signals for computing a recommendation score.
pub struct RecommendationScoreInput {
	pub vector_score: Option<f64>,
	pub recency_score: Option<f64>,
	pub frequency_score: Option<f64>,
}

/// Result of a recommendation score computation, including per-signal breakdown.
pub struct RecommendationScoreResult {
	pub score: f64,
	pub vector: Option<f64>,
	pub recency: Option<f64>,
	pub frequency: Option<f64>,
}

/// Compute a weighted recommendation score combining multiple signals.
pub fn compute_recommendation_score(
	input: &RecommendationScoreInput,
	weights: &RequiredWeightProfile,
) -> RecommendationScoreResult {
	let mut score = 0.0;
	if let Some(v) = input.vector_score {
		score += v * weights.vector;
	}
	if let Some(r) = input.recency_score {
		score += r * weights.recency;
	}
	if let Some(f) = input.frequency_score {
		score += f * weights.frequency;
	}
	RecommendationScoreResult {
		score,
		vector: input.vector_score,
		recency: input.recency_score,
		frequency: input.frequency_score,
	}
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;

	// -- normalize_weights tests ----------------------------------------------

	#[test]
	fn normalize_weights_defaults_when_none() {
		let result = normalize_weights(&None);
		assert!((result.vector - 0.6).abs() < 1e-10);
		assert!((result.recency - 0.2).abs() < 1e-10);
		assert!((result.frequency - 0.2).abs() < 1e-10);
	}

	#[test]
	fn normalize_weights_custom_values() {
		let w = Some(WeightProfile {
			vector: Some(0.5),
			recency: Some(0.3),
			frequency: Some(0.2),
		});
		let result = normalize_weights(&w);
		assert!((result.vector - 0.5).abs() < 1e-10);
		assert!((result.recency - 0.3).abs() < 1e-10);
		assert!((result.frequency - 0.2).abs() < 1e-10);
	}

	#[test]
	fn normalize_weights_scales_to_sum_one() {
		let w = Some(WeightProfile {
			vector: Some(1.0),
			recency: Some(1.0),
			frequency: Some(1.0),
		});
		let result = normalize_weights(&w);
		let sum = result.vector + result.recency + result.frequency;
		assert!((sum - 1.0).abs() < 1e-10);
		assert!((result.vector - 1.0 / 3.0).abs() < 1e-10);
	}

	#[test]
	fn normalize_weights_all_zero_returns_defaults() {
		let w = Some(WeightProfile {
			vector: Some(0.0),
			recency: Some(0.0),
			frequency: Some(0.0),
		});
		let result = normalize_weights(&w);
		assert!((result.vector - 0.6).abs() < 1e-10);
		assert!((result.recency - 0.2).abs() < 1e-10);
		assert!((result.frequency - 0.2).abs() < 1e-10);
	}

	#[test]
	fn normalize_weights_partial_uses_defaults() {
		let w = Some(WeightProfile {
			vector: Some(0.8),
			recency: None,
			frequency: None,
		});
		let result = normalize_weights(&w);
		let sum = result.vector + result.recency + result.frequency;
		assert!((sum - 1.0).abs() < 1e-10);
		// vector=0.8, recency=0.2(default), frequency=0.2(default), total=1.2
		assert!((result.vector - 0.8 / 1.2).abs() < 1e-10);
	}

	// -- recency_score tests --------------------------------------------------

	#[test]
	fn recency_score_at_now_is_one() {
		let score = recency_score(1000, DEFAULT_HALF_LIFE_MS, 1000);
		assert!((score - 1.0).abs() < 1e-10);
	}

	#[test]
	fn recency_score_at_half_life_is_half() {
		let now = 1_000_000u64;
		let half_life = 100_000.0;
		let entry_time = now - half_life as u64;
		let score = recency_score(entry_time, half_life, now);
		assert!((score - 0.5).abs() < 1e-6);
	}

	#[test]
	fn recency_score_future_timestamp_returns_one() {
		// Entry timestamp in the future should return 1.0 (age=0)
		let score = recency_score(2000, DEFAULT_HALF_LIFE_MS, 1000);
		assert!((score - 1.0).abs() < 1e-10);
	}

	#[test]
	fn recency_score_decays_with_age() {
		let now = 1_000_000u64;
		let recent = recency_score(now - 1000, DEFAULT_HALF_LIFE_MS, now);
		let old = recency_score(now - 1_000_000, DEFAULT_HALF_LIFE_MS, now);
		assert!(recent > old);
	}

	// -- frequency_score tests ------------------------------------------------

	#[test]
	fn frequency_score_max_count_is_one() {
		let score = frequency_score(10, 10);
		assert!((score - 1.0).abs() < 1e-10);
	}

	#[test]
	fn frequency_score_zero_max_returns_zero() {
		assert_eq!(frequency_score(5, 0), 0.0);
	}

	#[test]
	fn frequency_score_zero_count() {
		let score = frequency_score(0, 10);
		assert_eq!(score, 0.0);
	}

	#[test]
	fn frequency_score_diminishing_returns() {
		// Going from 1 to 2 should be a bigger jump than 9 to 10
		let s1 = frequency_score(1, 100);
		let s2 = frequency_score(2, 100);
		let s9 = frequency_score(9, 100);
		let s10 = frequency_score(10, 100);
		assert!(s2 - s1 > s10 - s9);
	}

	// -- compute_recommendation_score tests -----------------------------------

	#[test]
	fn compute_score_all_signals() {
		let weights = RequiredWeightProfile {
			vector: 0.6,
			recency: 0.2,
			frequency: 0.2,
		};
		let input = RecommendationScoreInput {
			vector_score: Some(0.9),
			recency_score: Some(0.8),
			frequency_score: Some(0.5),
		};
		let result = compute_recommendation_score(&input, &weights);
		let expected = 0.9 * 0.6 + 0.8 * 0.2 + 0.5 * 0.2;
		assert!((result.score - expected).abs() < 1e-10);
		assert_eq!(result.vector, Some(0.9));
		assert_eq!(result.recency, Some(0.8));
		assert_eq!(result.frequency, Some(0.5));
	}

	#[test]
	fn compute_score_missing_signals() {
		let weights = RequiredWeightProfile {
			vector: 0.6,
			recency: 0.2,
			frequency: 0.2,
		};
		let input = RecommendationScoreInput {
			vector_score: Some(0.9),
			recency_score: None,
			frequency_score: None,
		};
		let result = compute_recommendation_score(&input, &weights);
		assert!((result.score - 0.9 * 0.6).abs() < 1e-10);
		assert!(result.recency.is_none());
	}

	#[test]
	fn compute_score_no_signals() {
		let weights = RequiredWeightProfile {
			vector: 0.6,
			recency: 0.2,
			frequency: 0.2,
		};
		let input = RecommendationScoreInput {
			vector_score: None,
			recency_score: None,
			frequency_score: None,
		};
		let result = compute_recommendation_score(&input, &weights);
		assert_eq!(result.score, 0.0);
	}
}
