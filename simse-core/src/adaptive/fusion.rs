// ---------------------------------------------------------------------------
// Fusion — MMR reranking and RRF hybrid search fusion
// ---------------------------------------------------------------------------
//
// Provides two result fusion strategies for search/retrieval:
//
// - **MMR (Maximal Marginal Relevance)**: Reranks candidates to balance
//   relevance and diversity. Useful when search results cluster around
//   similar embeddings and you want broader topic coverage.
//
// - **RRF (Reciprocal Rank Fusion)**: Merges multiple ranked lists into
//   a single ranking. Useful for hybrid search where you combine results
//   from vector similarity, BM25 text search, and other signals.
// ---------------------------------------------------------------------------

use std::collections::HashMap;

use crate::adaptive::distance::DistanceMetric;

// ---------------------------------------------------------------------------
// MMR — Maximal Marginal Relevance
// ---------------------------------------------------------------------------

/// Maximal Marginal Relevance reranking for diversity.
///
/// Iteratively selects candidates that balance relevance to the query with
/// diversity from already-selected results:
///
/// ```text
/// MMR(d) = lambda * sim(query, d) - (1 - lambda) * max(sim(d, d_selected))
/// ```
///
/// # Arguments
///
/// * `candidates` — Vec of `(id, embedding, relevance_score)`. The
///   `relevance_score` is not used directly in the MMR formula; instead,
///   similarity is recomputed via the metric for consistency. The score
///   field is reserved for future use or caller reference.
/// * `query` — The query embedding vector.
/// * `k` — Maximum number of results to return.
/// * `lambda` — Trade-off parameter in `[0.0, 1.0]`:
///   - `1.0` = pure relevance (no diversity penalty)
///   - `0.0` = pure diversity (no relevance reward)
/// * `metric` — Distance metric used to compute similarity.
///
/// # Returns
///
/// Vec of `(id, mmr_score)` sorted by MMR score descending, with at most
/// `k` entries.
pub fn mmr_rerank(
	candidates: &[(String, Vec<f32>, f64)],
	query: &[f32],
	k: usize,
	lambda: f64,
	metric: DistanceMetric,
) -> Vec<(String, f64)> {
	if candidates.is_empty() || k == 0 {
		return Vec::new();
	}

	let n = candidates.len();
	let limit = k.min(n);

	// Pre-compute query-candidate similarities
	let query_sims: Vec<f64> = candidates
		.iter()
		.map(|(_, emb, _)| metric.similarity(query, emb))
		.collect();

	let mut selected: Vec<usize> = Vec::with_capacity(limit);
	let mut is_selected = vec![false; n];
	let mut result: Vec<(String, f64)> = Vec::with_capacity(limit);

	for _ in 0..limit {
		let mut best_idx: Option<usize> = None;
		let mut best_score = f64::NEG_INFINITY;

		for i in 0..n {
			if is_selected[i] {
				continue;
			}

			let relevance = query_sims[i];

			// Compute max similarity to any already-selected candidate
			let max_sim_to_selected = if selected.is_empty() {
				0.0
			} else {
				selected
					.iter()
					.map(|&j| metric.similarity(&candidates[i].1, &candidates[j].1))
					.fold(f64::NEG_INFINITY, f64::max)
			};

			let mmr_score = lambda * relevance - (1.0 - lambda) * max_sim_to_selected;

			if mmr_score > best_score {
				best_score = mmr_score;
				best_idx = Some(i);
			}
		}

		if let Some(idx) = best_idx {
			is_selected[idx] = true;
			selected.push(idx);
			result.push((candidates[idx].0.clone(), best_score));
		} else {
			break;
		}
	}

	result
}

// ---------------------------------------------------------------------------
// RRF — Reciprocal Rank Fusion
// ---------------------------------------------------------------------------

/// Reciprocal Rank Fusion: merges multiple ranked lists into a unified ranking.
///
/// For each document appearing in any list, computes:
///
/// ```text
/// score(d) = sum over lists of: 1 / (k + rank_in_list)
/// ```
///
/// Documents not present in a given list receive no contribution from that
/// list (not penalized, just no boost).
///
/// # Arguments
///
/// * `lists` — Slice of ranked lists. Each list is a slice of `(id, rank)`
///   pairs where `rank` is 1-indexed (1 = best).
/// * `k` — RRF constant (typically 60). Controls how much top ranks are
///   boosted relative to lower ranks. Higher `k` reduces the gap between
///   rank positions.
///
/// # Returns
///
/// Vec of `(id, rrf_score)` sorted by RRF score descending.
pub fn reciprocal_rank_fusion(
	lists: &[&[(String, usize)]],
	k: usize,
) -> Vec<(String, f64)> {
	if lists.is_empty() {
		return Vec::new();
	}

	let mut scores: HashMap<String, f64> = HashMap::new();

	for list in lists {
		for (id, rank) in *list {
			let contribution = 1.0 / (k as f64 + *rank as f64);
			*scores.entry(id.clone()).or_insert(0.0) += contribution;
		}
	}

	let mut result: Vec<(String, f64)> = scores.into_iter().collect();
	result.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
	result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;

	// -- MMR tests -----------------------------------------------------------

	#[test]
	fn mmr_lambda_one_is_pure_relevance() {
		// With lambda=1.0, diversity term is 0, so order = relevance order
		let candidates = vec![
			("a".into(), vec![1.0f32, 0.0], 0.9),
			("b".into(), vec![0.0, 1.0], 0.5),
			("c".into(), vec![0.8, 0.2], 0.85),
		];
		let query = vec![1.0f32, 0.0];
		let result = mmr_rerank(&candidates, &query, 3, 1.0, DistanceMetric::Cosine);
		assert_eq!(result[0].0, "a");
	}

	#[test]
	fn mmr_promotes_diversity() {
		let candidates = vec![
			("a".into(), vec![1.0f32, 0.0], 0.9),
			("c".into(), vec![0.95, 0.05], 0.89), // very similar to a
			("b".into(), vec![0.0, 1.0], 0.5),     // orthogonal to a
		];
		let query = vec![1.0f32, 0.0];
		// Low lambda = high diversity
		let result = mmr_rerank(&candidates, &query, 3, 0.3, DistanceMetric::Cosine);
		// After "a" is selected, "b" should be promoted over "c" due to diversity
		assert_eq!(result[0].0, "a");
		assert_eq!(result[1].0, "b"); // diverse pick
	}

	#[test]
	fn mmr_empty_candidates() {
		let result = mmr_rerank(&[], &[1.0], 10, 0.5, DistanceMetric::Cosine);
		assert!(result.is_empty());
	}

	#[test]
	fn mmr_k_larger_than_candidates() {
		let candidates = vec![("a".into(), vec![1.0f32], 0.9)];
		let result = mmr_rerank(&candidates, &[1.0], 10, 0.5, DistanceMetric::Cosine);
		assert_eq!(result.len(), 1);
	}

	#[test]
	fn mmr_k_zero_returns_empty() {
		let candidates = vec![("a".into(), vec![1.0f32, 0.0], 0.9)];
		let result = mmr_rerank(&candidates, &[1.0, 0.0], 0, 0.5, DistanceMetric::Cosine);
		assert!(result.is_empty());
	}

	#[test]
	fn mmr_lambda_zero_maximizes_diversity() {
		// With lambda=0.0, relevance term is 0, only diversity matters
		let candidates = vec![
			("a".into(), vec![1.0f32, 0.0], 0.9),
			("b".into(), vec![0.99, 0.01], 0.88),
			("c".into(), vec![-1.0, 0.0], 0.1), // maximally different from a
		];
		let query = vec![1.0f32, 0.0];
		let result = mmr_rerank(&candidates, &query, 3, 0.0, DistanceMetric::Cosine);
		// First pick: all have 0 relevance weight; diversity term is 0 for empty
		// selected set, so first pick is arbitrary (all have mmr_score = 0).
		// After first pick, second pick should maximize diversity.
		assert_eq!(result.len(), 3);
	}

	#[test]
	fn mmr_with_euclidean_metric() {
		let candidates = vec![
			("a".into(), vec![1.0f32, 0.0], 0.9),
			("b".into(), vec![0.0, 1.0], 0.5),
		];
		let query = vec![1.0f32, 0.0];
		let result = mmr_rerank(&candidates, &query, 2, 1.0, DistanceMetric::Euclidean);
		// With Euclidean similarity: 1/(1+0) = 1.0 for "a", 1/(1+sqrt(2)) for "b"
		assert_eq!(result[0].0, "a");
		assert_eq!(result.len(), 2);
	}

	#[test]
	fn mmr_scores_are_descending() {
		let candidates = vec![
			("a".into(), vec![1.0f32, 0.0, 0.0], 0.9),
			("b".into(), vec![0.0, 1.0, 0.0], 0.5),
			("c".into(), vec![0.0, 0.0, 1.0], 0.3),
		];
		let query = vec![1.0f32, 0.0, 0.0];
		let result = mmr_rerank(&candidates, &query, 3, 0.7, DistanceMetric::Cosine);
		for i in 1..result.len() {
			assert!(
				result[i - 1].1 >= result[i].1,
				"MMR scores not descending: {} < {}",
				result[i - 1].1,
				result[i].1
			);
		}
	}

	// -- RRF tests -----------------------------------------------------------

	#[test]
	fn rrf_basic_ranking() {
		let list_a: Vec<(String, usize)> =
			vec![("x".into(), 1), ("y".into(), 2), ("z".into(), 3)];
		let list_b: Vec<(String, usize)> =
			vec![("y".into(), 1), ("x".into(), 2), ("z".into(), 3)];
		let result = reciprocal_rank_fusion(&[&list_a, &list_b], 60);
		// x: 1/61 + 1/62, y: 1/62 + 1/61 — same score, both above z
		let z_pos = result.iter().position(|(id, _)| id == "z").unwrap();
		assert_eq!(z_pos, 2);
	}

	#[test]
	fn rrf_single_list() {
		let list: Vec<(String, usize)> = vec![("a".into(), 1), ("b".into(), 2)];
		let result = reciprocal_rank_fusion(&[&list], 60);
		assert_eq!(result[0].0, "a");
	}

	#[test]
	fn rrf_empty() {
		let result = reciprocal_rank_fusion(&[], 60);
		assert!(result.is_empty());
	}

	#[test]
	fn rrf_disjoint_lists() {
		let list_a: Vec<(String, usize)> = vec![("a".into(), 1)];
		let list_b: Vec<(String, usize)> = vec![("b".into(), 1)];
		let result = reciprocal_rank_fusion(&[&list_a, &list_b], 60);
		// Both have score 1/61, so 2 results
		assert_eq!(result.len(), 2);
	}

	#[test]
	fn rrf_score_accumulation() {
		// Document appearing in 3 lists at rank 1 should score 3/(k+1)
		let list_a: Vec<(String, usize)> = vec![("x".into(), 1)];
		let list_b: Vec<(String, usize)> = vec![("x".into(), 1)];
		let list_c: Vec<(String, usize)> = vec![("x".into(), 1)];
		let result = reciprocal_rank_fusion(&[&list_a, &list_b, &list_c], 60);
		assert_eq!(result.len(), 1);
		let expected = 3.0 / 61.0;
		assert!((result[0].1 - expected).abs() < 1e-10);
	}

	#[test]
	fn rrf_higher_rank_wins() {
		// "a" at rank 1 in one list vs "b" at rank 10 in one list
		let list: Vec<(String, usize)> = vec![("a".into(), 1), ("b".into(), 10)];
		let result = reciprocal_rank_fusion(&[&list], 60);
		assert_eq!(result[0].0, "a");
		assert!(result[0].1 > result[1].1);
	}

	#[test]
	fn rrf_empty_list_in_lists() {
		let list_a: Vec<(String, usize)> = vec![];
		let list_b: Vec<(String, usize)> = vec![("a".into(), 1)];
		let result = reciprocal_rank_fusion(&[&list_a, &list_b], 60);
		assert_eq!(result.len(), 1);
		assert_eq!(result[0].0, "a");
	}

	#[test]
	fn rrf_scores_are_descending() {
		let list_a: Vec<(String, usize)> =
			vec![("a".into(), 1), ("b".into(), 2), ("c".into(), 3)];
		let list_b: Vec<(String, usize)> =
			vec![("c".into(), 1), ("a".into(), 2), ("b".into(), 3)];
		let result = reciprocal_rank_fusion(&[&list_a, &list_b], 60);
		for i in 1..result.len() {
			assert!(
				result[i - 1].1 >= result[i].1,
				"RRF scores not descending: {} < {}",
				result[i - 1].1,
				result[i].1
			);
		}
	}
}
