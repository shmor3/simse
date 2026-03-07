use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use hnsw_rs::hnsw::Hnsw;
use rayon::prelude::*;

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
		let n = self.storage.len();
		if n == 0 {
			return vec![];
		}

		let dims = self.storage.dimensions();
		let raw = self.storage.raw_embeddings();

		// Collect IDs up front so we can index by position in the parallel loop
		let ids: Vec<&str> = self.storage.iter().map(|(id, _)| id).collect();

		let mut results: Vec<SearchResult> = (0..n)
			.into_par_iter()
			.map(|i| {
				let embedding = &raw[i * dims..(i + 1) * dims];
				let score = metric.similarity(query, embedding);
				(ids[i].to_string(), score)
			})
			.collect();

		results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));

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

// ---------------------------------------------------------------------------
// HnswIndex — approximate nearest-neighbor index wrapping hnsw_rs
// ---------------------------------------------------------------------------

/// Configuration for the HNSW index.
pub struct HnswConfig {
	/// Maximum number of bi-directional connections per node (default 16).
	pub m: usize,
	/// Search width during graph construction (default 200).
	pub ef_construction: usize,
	/// Search width at query time (default 50).
	pub ef_search: usize,
}

impl Default for HnswConfig {
	fn default() -> Self {
		Self {
			m: 16,
			ef_construction: 200,
			ef_search: 50,
		}
	}
}

/// Cosine distance adapter for hnsw_rs.
///
/// hnsw_rs requires a distance metric (lower = more similar). This wraps
/// our `crate::distance::cosine_distance` which returns `1.0 - cosine_similarity`,
/// giving a value in [0.0, 2.0].
#[derive(Clone, Copy)]
struct CosineDistanceAdapter;

impl hnsw_rs::anndists::dist::distances::Distance<f32> for CosineDistanceAdapter {
	fn eval(&self, a: &[f32], b: &[f32]) -> f32 {
		crate::distance::cosine_distance(a, b) as f32
	}
}

/// Approximate nearest-neighbor index backed by an HNSW graph (hnsw_rs).
///
/// The HNSW graph is built using cosine distance internally. When a different
/// `DistanceMetric` is requested at query time, the graph is still traversed
/// using cosine distance (which produces a good candidate set for any metric
/// on normalized-ish data), and the final results are **re-ranked** using the
/// requested metric. This avoids generic proliferation while keeping results
/// accurate for typical embedding workloads.
///
/// # Soft deletion
///
/// `hnsw_rs` does not support removing nodes from the graph. Instead, `remove()`
/// marks the ID as deleted in a `removed` set. `search()` filters out removed
/// entries, over-fetching candidates to compensate. Call `rebuild()` to create
/// a fresh graph without the removed entries.
pub struct HnswIndex {
	config: HnswConfig,
	dimensions: usize,
	/// Forward map: string ID -> internal integer index.
	id_to_idx: HashMap<String, usize>,
	/// Reverse map: internal integer index -> string ID.
	idx_to_id: HashMap<usize, String>,
	/// Stored vectors keyed by internal index (needed for rebuild).
	vectors: HashMap<usize, Vec<f32>>,
	/// Next internal index to assign.
	next_idx: usize,
	/// Soft-deleted IDs (still in graph until rebuild).
	removed: HashSet<String>,
	/// The underlying HNSW graph. `None` only before the first insert.
	graph: Option<Hnsw<'static, f32, CosineDistanceAdapter>>,
}

impl HnswIndex {
	/// Create a new HNSW index with the given dimensionality and default config.
	pub fn new(dimensions: usize) -> Self {
		Self::with_config(dimensions, HnswConfig::default())
	}

	/// Create a new HNSW index with explicit configuration.
	pub fn with_config(dimensions: usize, config: HnswConfig) -> Self {
		Self {
			config,
			dimensions,
			id_to_idx: HashMap::new(),
			idx_to_id: HashMap::new(),
			vectors: HashMap::new(),
			next_idx: 0,
			removed: HashSet::new(),
			graph: None,
		}
	}

	/// Lazily initialize the HNSW graph with a capacity hint.
	fn ensure_graph(&mut self, capacity: usize) {
		if self.graph.is_none() {
			let max_elements = capacity.max(128);
			let max_layer = 16;
			let hnsw = Hnsw::new(
				self.config.m,
				max_elements,
				max_layer,
				self.config.ef_construction,
				CosineDistanceAdapter,
			);
			self.graph = Some(hnsw);
		}
	}
}

impl IndexBackend for HnswIndex {
	fn insert(&mut self, id: &str, embedding: &[f32]) {
		debug_assert_eq!(
			embedding.len(),
			self.dimensions,
			"embedding dimension mismatch: expected {}, got {}",
			self.dimensions,
			embedding.len()
		);

		// If this ID was previously soft-deleted, un-remove it.
		self.removed.remove(id);

		// If this ID already exists, remove the old mapping and vector.
		// The old graph node becomes orphaned (filtered out by the missing
		// idx_to_id mapping). It will be purged on the next rebuild.
		if let Some(&old_idx) = self.id_to_idx.get(id) {
			self.vectors.remove(&old_idx);
			self.idx_to_id.remove(&old_idx);
		}

		self.ensure_graph(1);

		let idx = self.next_idx;
		self.next_idx += 1;

		self.id_to_idx.insert(id.to_string(), idx);
		self.idx_to_id.insert(idx, id.to_string());
		self.vectors.insert(idx, embedding.to_vec());

		if let Some(ref graph) = self.graph {
			graph.insert((embedding, idx));
		}
	}

	fn insert_batch(&mut self, entries: &[(&str, &[f32])]) {
		for &(id, embedding) in entries {
			self.insert(id, embedding);
		}
	}

	fn remove(&mut self, id: &str) {
		if self.id_to_idx.contains_key(id) {
			self.removed.insert(id.to_string());
		}
	}

	fn search(&self, query: &[f32], k: usize, metric: DistanceMetric) -> Vec<SearchResult> {
		let graph = match self.graph {
			Some(ref g) => g,
			None => return vec![],
		};

		if self.len() == 0 {
			return vec![];
		}

		// Over-fetch to account for soft-deleted entries.
		let fetch_k = (k + self.removed.len()).min(graph.get_nb_point());
		if fetch_k == 0 {
			return vec![];
		}

		let neighbours = graph.search(query, fetch_k, self.config.ef_search);

		// Map internal IDs back to string IDs, filter removed, re-rank
		// with the requested metric.
		let mut results: Vec<SearchResult> = neighbours
			.iter()
			.filter_map(|n| {
				let str_id = self.idx_to_id.get(&n.d_id)?;
				if self.removed.contains(str_id) {
					return None;
				}
				let embedding = self.vectors.get(&n.d_id)?;
				let score = metric.similarity(query, embedding);
				Some((str_id.clone(), score))
			})
			.collect();

		results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
		results.truncate(k);
		results
	}

	fn len(&self) -> usize {
		// Active entries = total inserted minus soft-deleted.
		let total_mapped = self.id_to_idx.len();
		let removed_count = self.removed.len();
		total_mapped.saturating_sub(removed_count)
	}

	fn contains(&self, id: &str) -> bool {
		self.id_to_idx.contains_key(id) && !self.removed.contains(id)
	}

	fn rebuild(&mut self) {
		// Purge removed entries from all maps.
		for id in self.removed.drain() {
			if let Some(idx) = self.id_to_idx.remove(&id) {
				self.idx_to_id.remove(&idx);
				self.vectors.remove(&idx);
			}
		}

		// Rebuild the HNSW graph from scratch with only active entries.
		let capacity = self.id_to_idx.len().max(128);
		let max_layer = 16;
		let new_graph = Hnsw::new(
			self.config.m,
			capacity,
			max_layer,
			self.config.ef_construction,
			CosineDistanceAdapter,
		);

		for (&idx, vec) in &self.vectors {
			new_graph.insert((vec.as_slice(), idx));
		}

		self.graph = Some(new_graph);
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

	// -- HnswIndex tests ----------------------------------------------------

	fn make_hnsw() -> HnswIndex {
		HnswIndex::new(3)
	}

	#[test]
	fn hnsw_insert_and_search() {
		let mut idx = make_hnsw();
		idx.insert("a", &[1.0, 0.0, 0.0]);
		idx.insert("b", &[0.0, 1.0, 0.0]);
		idx.insert("c", &[0.9, 0.1, 0.0]);
		let results = idx.search(&[1.0, 0.0, 0.0], 2, DistanceMetric::Cosine);
		assert_eq!(results.len(), 2);
		assert_eq!(results[0].0, "a"); // exact match is nearest
		assert_eq!(results[1].0, "c"); // close to query
	}

	#[test]
	fn hnsw_remove_and_search() {
		let mut idx = make_hnsw();
		idx.insert("a", &[1.0, 0.0, 0.0]);
		idx.insert("b", &[0.0, 1.0, 0.0]);
		idx.insert("c", &[0.0, 0.0, 1.0]);
		idx.remove("a");
		let results = idx.search(&[1.0, 0.0, 0.0], 10, DistanceMetric::Cosine);
		// "a" was removed, so it must not appear in results.
		assert!(!results.iter().any(|(id, _)| id == "a"));
		assert_eq!(results.len(), 2);
	}

	#[test]
	fn hnsw_batch_insert() {
		let mut idx = make_hnsw();
		idx.insert_batch(&[
			("a", &[1.0, 0.0, 0.0]),
			("b", &[0.0, 1.0, 0.0]),
			("c", &[0.0, 0.0, 1.0]),
		]);
		assert_eq!(idx.len(), 3);
		let results = idx.search(&[1.0, 0.0, 0.0], 3, DistanceMetric::Cosine);
		assert_eq!(results.len(), 3);
		assert_eq!(results[0].0, "a");
	}

	#[test]
	fn hnsw_rebuild() {
		let mut idx = make_hnsw();
		idx.insert("a", &[1.0, 0.0, 0.0]);
		idx.insert("b", &[0.0, 1.0, 0.0]);
		idx.insert("c", &[0.0, 0.0, 1.0]);
		idx.remove("b");
		assert_eq!(idx.len(), 2);

		// Rebuild compacts the index, purging "b".
		idx.rebuild();
		assert_eq!(idx.len(), 2);
		assert!(!idx.contains("b"));
		assert!(idx.contains("a"));
		assert!(idx.contains("c"));

		// Search should still work correctly after rebuild.
		let results = idx.search(&[1.0, 0.0, 0.0], 10, DistanceMetric::Cosine);
		assert_eq!(results.len(), 2);
		assert_eq!(results[0].0, "a");
	}

	#[test]
	fn hnsw_empty_search() {
		let idx = make_hnsw();
		let results = idx.search(&[1.0, 0.0, 0.0], 10, DistanceMetric::Cosine);
		assert!(results.is_empty());
	}

	#[test]
	fn hnsw_contains() {
		let mut idx = make_hnsw();
		assert!(!idx.contains("a"));
		idx.insert("a", &[1.0, 0.0, 0.0]);
		assert!(idx.contains("a"));
		assert!(!idx.contains("b"));
		idx.remove("a");
		// After soft-delete, contains returns false.
		assert!(!idx.contains("a"));
	}

	#[test]
	fn hnsw_len() {
		let mut idx = make_hnsw();
		assert_eq!(idx.len(), 0);
		assert!(idx.is_empty());
		idx.insert("a", &[1.0, 0.0, 0.0]);
		assert_eq!(idx.len(), 1);
		assert!(!idx.is_empty());
		idx.insert("b", &[0.0, 1.0, 0.0]);
		assert_eq!(idx.len(), 2);
		// Soft-delete decrements len.
		idx.remove("a");
		assert_eq!(idx.len(), 1);
	}
}
