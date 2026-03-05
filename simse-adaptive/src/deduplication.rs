// ---------------------------------------------------------------------------
// Deduplication — detect and group near-duplicate vector entries
// ---------------------------------------------------------------------------
//
// Pure functions that operate on Volume arrays. No side effects,
// no external dependencies beyond cosine similarity.
// ---------------------------------------------------------------------------

use crate::cosine::cosine_similarity;
use crate::types::{DuplicateCheckResult, DuplicateVolumes, Volume};

// ---------------------------------------------------------------------------
// Single-entry duplicate check
// ---------------------------------------------------------------------------

/// Check whether `new_embedding` is a near-duplicate of any existing volume.
///
/// Returns the closest match above `threshold`, or a non-duplicate result
/// if no match is found. Linear scan — O(N).
pub fn check_duplicate(
	new_embedding: &[f32],
	volumes: &[Volume],
	threshold: f64,
) -> DuplicateCheckResult {
	let mut best_entry: Option<&Volume> = None;
	let mut best_similarity = f64::NEG_INFINITY;

	for volume in volumes {
		if volume.embedding.len() != new_embedding.len() {
			continue;
		}
		let sim = cosine_similarity(new_embedding, &volume.embedding);
		if sim >= threshold && sim > best_similarity {
			best_similarity = sim;
			best_entry = Some(volume);
		}
	}

	match best_entry {
		Some(vol) => DuplicateCheckResult {
			is_duplicate: true,
			existing_volume: Some(vol.clone()),
			similarity: Some(best_similarity),
		},
		None => DuplicateCheckResult {
			is_duplicate: false,
			existing_volume: None,
			similarity: None,
		},
	}
}

// ---------------------------------------------------------------------------
// Group duplicate detection
// ---------------------------------------------------------------------------

/// Find groups of near-duplicate volumes using greedy clustering.
///
/// Entries are processed in timestamp order (oldest first). For each entry,
/// if it is similar enough to an existing group's representative, it joins
/// that group. Otherwise it starts a new group.
///
/// O(N^2) — intended for explicit user-triggered deduplication, not hot paths.
/// Only returns groups that have at least one duplicate.
pub fn find_duplicate_volumes(
	volumes: &[Volume],
	threshold: f64,
) -> Vec<DuplicateVolumes> {
	if volumes.len() < 2 {
		return vec![];
	}

	// Sort by timestamp (oldest first) so the representative is the original
	let mut sorted = volumes.to_vec();
	sorted.sort_by_key(|v| v.timestamp);

	struct Group {
		representative: Volume,
		duplicates: Vec<Volume>,
		total_similarity: f64,
	}

	let mut groups: Vec<Group> = Vec::new();

	for volume in sorted {
		let mut assigned = false;
		for group in &mut groups {
			if group.representative.embedding.len() != volume.embedding.len() {
				continue;
			}
			let sim =
				cosine_similarity(&group.representative.embedding, &volume.embedding);
			if sim >= threshold {
				group.duplicates.push(volume.clone());
				group.total_similarity += sim;
				assigned = true;
				break;
			}
		}
		if !assigned {
			groups.push(Group {
				representative: volume,
				duplicates: Vec::new(),
				total_similarity: 0.0,
			});
		}
	}

	groups
		.into_iter()
		.filter(|g| !g.duplicates.is_empty())
		.map(|g| {
			let avg = if g.duplicates.is_empty() {
				0.0
			} else {
				g.total_similarity / g.duplicates.len() as f64
			};
			DuplicateVolumes {
				representative: g.representative,
				duplicates: g.duplicates,
				average_similarity: avg,
			}
		})
		.collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;
	use std::collections::HashMap;

	fn make_volume(id: &str, embedding: Vec<f32>, timestamp: u64) -> Volume {
		Volume {
			id: id.to_string(),
			text: format!("text for {}", id),
			embedding,
			metadata: HashMap::new(),
			timestamp,
		}
	}

	// -- check_duplicate tests ------------------------------------------------

	#[test]
	fn check_duplicate_finds_exact_match() {
		let emb = vec![1.0, 0.0, 0.0];
		let volumes = vec![make_volume("a", vec![1.0, 0.0, 0.0], 100)];
		let result = check_duplicate(&emb, &volumes, 0.9);
		assert!(result.is_duplicate);
		assert_eq!(result.existing_volume.as_ref().unwrap().id, "a");
		assert!((result.similarity.unwrap() - 1.0).abs() < 1e-10);
	}

	#[test]
	fn check_duplicate_no_match_below_threshold() {
		let emb = vec![1.0, 0.0, 0.0];
		let volumes = vec![make_volume("a", vec![0.0, 1.0, 0.0], 100)];
		let result = check_duplicate(&emb, &volumes, 0.9);
		assert!(!result.is_duplicate);
		assert!(result.existing_volume.is_none());
		assert!(result.similarity.is_none());
	}

	#[test]
	fn check_duplicate_returns_best_match() {
		let emb = vec![1.0, 0.0, 0.0];
		let volumes = vec![
			make_volume("a", vec![0.9, 0.1, 0.0], 100),
			make_volume("b", vec![0.99, 0.01, 0.0], 200),
		];
		let result = check_duplicate(&emb, &volumes, 0.5);
		assert!(result.is_duplicate);
		assert_eq!(result.existing_volume.as_ref().unwrap().id, "b");
	}

	#[test]
	fn check_duplicate_skips_dimension_mismatch() {
		let emb = vec![1.0, 0.0, 0.0];
		let volumes = vec![make_volume("a", vec![1.0, 0.0], 100)];
		let result = check_duplicate(&emb, &volumes, 0.9);
		assert!(!result.is_duplicate);
	}

	#[test]
	fn check_duplicate_empty_volumes() {
		let emb = vec![1.0, 0.0, 0.0];
		let result = check_duplicate(&emb, &[], 0.9);
		assert!(!result.is_duplicate);
	}

	// -- find_duplicate_volumes tests -----------------------------------------

	#[test]
	fn find_duplicates_groups_similar_entries() {
		let volumes = vec![
			make_volume("a", vec![1.0, 0.0, 0.0], 100),
			make_volume("b", vec![0.99, 0.01, 0.0], 200),
			make_volume("c", vec![0.0, 1.0, 0.0], 300),
		];
		let groups = find_duplicate_volumes(&volumes, 0.9);
		assert_eq!(groups.len(), 1);
		assert_eq!(groups[0].representative.id, "a"); // oldest is representative
		assert_eq!(groups[0].duplicates.len(), 1);
		assert_eq!(groups[0].duplicates[0].id, "b");
		assert!(groups[0].average_similarity > 0.9);
	}

	#[test]
	fn find_duplicates_returns_empty_for_single_volume() {
		let volumes = vec![make_volume("a", vec![1.0, 0.0], 100)];
		assert!(find_duplicate_volumes(&volumes, 0.9).is_empty());
	}

	#[test]
	fn find_duplicates_returns_empty_when_no_duplicates() {
		let volumes = vec![
			make_volume("a", vec![1.0, 0.0, 0.0], 100),
			make_volume("b", vec![0.0, 1.0, 0.0], 200),
			make_volume("c", vec![0.0, 0.0, 1.0], 300),
		];
		assert!(find_duplicate_volumes(&volumes, 0.9).is_empty());
	}

	#[test]
	fn find_duplicates_multiple_groups() {
		let volumes = vec![
			make_volume("a1", vec![1.0, 0.0, 0.0], 100),
			make_volume("a2", vec![0.99, 0.01, 0.0], 200),
			make_volume("b1", vec![0.0, 1.0, 0.0], 300),
			make_volume("b2", vec![0.0, 0.99, 0.01], 400),
		];
		let groups = find_duplicate_volumes(&volumes, 0.9);
		assert_eq!(groups.len(), 2);
	}

	#[test]
	fn find_duplicates_oldest_is_representative() {
		let volumes = vec![
			make_volume("newer", vec![1.0, 0.0, 0.0], 500),
			make_volume("oldest", vec![0.99, 0.01, 0.0], 100),
			make_volume("middle", vec![0.98, 0.02, 0.0], 300),
		];
		let groups = find_duplicate_volumes(&volumes, 0.9);
		assert_eq!(groups.len(), 1);
		assert_eq!(groups[0].representative.id, "oldest");
		assert_eq!(groups[0].duplicates.len(), 2);
	}

	#[test]
	fn find_duplicates_empty_input() {
		assert!(find_duplicate_volumes(&[], 0.9).is_empty());
	}

	#[test]
	fn find_duplicates_average_similarity_is_correct() {
		let volumes = vec![
			make_volume("a", vec![1.0, 0.0, 0.0], 100),
			make_volume("b", vec![1.0, 0.0, 0.0], 200),
			make_volume("c", vec![1.0, 0.0, 0.0], 300),
		];
		let groups = find_duplicate_volumes(&volumes, 0.9);
		assert_eq!(groups.len(), 1);
		assert_eq!(groups[0].duplicates.len(), 2);
		// All identical, so average similarity should be 1.0
		assert!((groups[0].average_similarity - 1.0).abs() < 1e-10);
	}

	#[test]
	fn find_duplicates_skips_dimension_mismatch() {
		let volumes = vec![
			make_volume("a", vec![1.0, 0.0], 100),
			make_volume("b", vec![1.0, 0.0, 0.0], 200),
		];
		// Different dimensions, should not be grouped
		assert!(find_duplicate_volumes(&volumes, 0.9).is_empty());
	}
}
