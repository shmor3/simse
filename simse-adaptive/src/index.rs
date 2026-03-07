use std::cmp::Ordering;

use crate::distance::DistanceMetric;
use crate::vector_storage::VectorStorage;

/// Result of an index search: (id, similarity_score).
pub type SearchResult = (String, f64);

/// Backend trait for vector indexing strategies.
pub trait IndexBackend: Send + Sync {
	fn insert(&mut self, id: &str, embedding: &[f32]);
	fn insert_batch(&mut self, entries: &[(&str, &[f32])]);
	fn remove(&mut self, id: &str);
	fn search(&self, query: &[f32], k: usize, metric: DistanceMetric) -> Vec<SearchResult>;
	fn len(&self) -> usize;
	fn is_empty(&self) -> bool {
		self.len() == 0
	}
	fn contains(&self, id: &str) -> bool;
	fn rebuild(&mut self);
}

/// Brute-force flat index using VectorStorage.
/// Optimal for small collections (<=1K vectors). O(N) search.
pub struct FlatIndex {
	storage: VectorStorage,
}

impl FlatIndex {
	pub fn new(dimensions: usize) -> Self {
		Self {
			storage: VectorStorage::new(dimensions),
		}
	}
}

impl IndexBackend for FlatIndex {
	fn insert(&mut self, id: &str, embedding: &[f32]) {
		self.storage.insert(id, embedding);
	}

	fn insert_batch(&mut self, entries: &[(&str, &[f32])]) {
		self.storage.insert_batch(entries);
	}

	fn remove(&mut self, id: &str) {
		self.storage.remove(id);
	}

	fn search(&self, query: &[f32], k: usize, metric: DistanceMetric) -> Vec<SearchResult> {
		let mut results: Vec<SearchResult> = self
			.storage
			.iter()
			.map(|(id, embedding)| {
				let score = metric.similarity(query, embedding);
				(id.to_string(), score)
			})
			.collect();

		results.sort_by(|a, b| {
			b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal)
		});

		results.truncate(k);
		results
	}

	fn len(&self) -> usize {
		self.storage.len()
	}

	fn contains(&self, id: &str) -> bool {
		self.storage.contains(id)
	}

	fn rebuild(&mut self) {
		// FlatIndex has no auxiliary structure to rebuild.
		// This is a no-op — the storage is always consistent.
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn make_flat() -> FlatIndex {
		FlatIndex::new(3)
	}

	#[test]
	fn flat_insert_and_search() {
		let mut idx = make_flat();
		idx.insert("a", &[1.0, 0.0, 0.0]);
		idx.insert("b", &[0.0, 1.0, 0.0]);
		idx.insert("c", &[0.9, 0.1, 0.0]);
		let results = idx.search(&[1.0, 0.0, 0.0], 2, DistanceMetric::Cosine);
		assert_eq!(results.len(), 2);
		assert_eq!(results[0].0, "a"); // most similar (exact match)
		assert_eq!(results[1].0, "c"); // second most similar
	}

	#[test]
	fn flat_remove() {
		let mut idx = make_flat();
		idx.insert("a", &[1.0, 0.0, 0.0]);
		idx.insert("b", &[0.0, 1.0, 0.0]);
		idx.remove("a");
		assert_eq!(idx.len(), 1);
		let results = idx.search(&[1.0, 0.0, 0.0], 10, DistanceMetric::Cosine);
		assert_eq!(results.len(), 1);
		assert_eq!(results[0].0, "b");
	}

	#[test]
	fn flat_batch_insert() {
		let mut idx = make_flat();
		idx.insert_batch(&[
			("a", &[1.0, 0.0, 0.0]),
			("b", &[0.0, 1.0, 0.0]),
			("c", &[0.0, 0.0, 1.0]),
		]);
		assert_eq!(idx.len(), 3);
		let results = idx.search(&[1.0, 0.0, 0.0], 10, DistanceMetric::Cosine);
		assert_eq!(results.len(), 3);
		assert_eq!(results[0].0, "a");
	}

	#[test]
	fn flat_euclidean_search() {
		let mut idx = make_flat();
		idx.insert("a", &[0.0, 0.0, 0.0]);
		idx.insert("b", &[1.0, 1.0, 1.0]);
		let results = idx.search(&[0.0, 0.0, 0.0], 2, DistanceMetric::Euclidean);
		// "a" should be first — distance 0 => similarity 1/(1+0) = 1.0
		// "b" has distance sqrt(3) => similarity 1/(1+sqrt(3)) ~ 0.366
		assert_eq!(results[0].0, "a");
		assert!((results[0].1 - 1.0).abs() < 1e-6);
		assert_eq!(results[1].0, "b");
	}

	#[test]
	fn flat_dot_product_search() {
		let mut idx = make_flat();
		idx.insert("a", &[1.0, 0.0, 0.0]);
		idx.insert("b", &[0.5, 0.5, 0.0]);
		idx.insert("c", &[0.0, 0.0, 1.0]);
		let results = idx.search(&[1.0, 0.0, 0.0], 3, DistanceMetric::DotProduct);
		// dot(query, a) = 1.0, dot(query, b) = 0.5, dot(query, c) = 0.0
		assert_eq!(results[0].0, "a");
		assert!((results[0].1 - 1.0).abs() < 1e-6);
		assert_eq!(results[1].0, "b");
		assert!((results[1].1 - 0.5).abs() < 1e-6);
		assert_eq!(results[2].0, "c");
		assert!(results[2].1.abs() < 1e-6);
	}

	#[test]
	fn flat_empty_search() {
		let idx = make_flat();
		let results = idx.search(&[1.0, 0.0, 0.0], 10, DistanceMetric::Cosine);
		assert!(results.is_empty());
	}

	#[test]
	fn flat_search_k_larger_than_entries() {
		let mut idx = make_flat();
		idx.insert("a", &[1.0, 0.0, 0.0]);
		idx.insert("b", &[0.0, 1.0, 0.0]);
		idx.insert("c", &[0.0, 0.0, 1.0]);
		let results = idx.search(&[1.0, 0.0, 0.0], 100, DistanceMetric::Cosine);
		assert_eq!(results.len(), 3);
	}

	#[test]
	fn flat_contains() {
		let mut idx = make_flat();
		assert!(!idx.contains("a"));
		idx.insert("a", &[1.0, 0.0, 0.0]);
		assert!(idx.contains("a"));
		assert!(!idx.contains("b"));
		idx.remove("a");
		assert!(!idx.contains("a"));
	}

	#[test]
	fn flat_rebuild() {
		let mut idx = make_flat();
		idx.insert("a", &[1.0, 0.0, 0.0]);
		idx.insert("b", &[0.0, 1.0, 0.0]);
		idx.rebuild();
		// Rebuild should not break anything.
		assert_eq!(idx.len(), 2);
		let results = idx.search(&[1.0, 0.0, 0.0], 2, DistanceMetric::Cosine);
		assert_eq!(results.len(), 2);
		assert_eq!(results[0].0, "a");
	}

	#[test]
	fn flat_len() {
		let mut idx = make_flat();
		assert_eq!(idx.len(), 0);
		assert!(idx.is_empty());
		idx.insert("a", &[1.0, 0.0, 0.0]);
		assert_eq!(idx.len(), 1);
		assert!(!idx.is_empty());
		idx.insert("b", &[0.0, 1.0, 0.0]);
		assert_eq!(idx.len(), 2);
		idx.remove("a");
		assert_eq!(idx.len(), 1);
	}
}
