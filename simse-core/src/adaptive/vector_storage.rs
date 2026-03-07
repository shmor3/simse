// ---------------------------------------------------------------------------
// VectorStorage — Structure-of-Arrays contiguous embedding storage
// ---------------------------------------------------------------------------
//
// Stores embeddings in a single contiguous `Vec<f32>` buffer for cache-friendly
// sequential scans. Each embedding occupies a fixed-width slot of `dimensions`
// floats. IDs are tracked in a parallel `Vec<String>` and a `HashMap` provides
// O(1) lookup from ID to index.
//
// Removal uses swap-remove: the last entry is moved into the vacated slot so
// the buffer stays contiguous with no holes.
// ---------------------------------------------------------------------------

use std::collections::HashMap;

/// Structure-of-Arrays vector storage for cache-friendly distance computation.
/// Embeddings stored contiguously: `[v0_d0, v0_d1, ..., v1_d0, v1_d1, ...]`
pub struct VectorStorage {
	ids: Vec<String>,
	embeddings: Vec<f32>,
	id_to_index: HashMap<String, usize>,
	dimensions: usize,
}

impl VectorStorage {
	/// Create a new, empty `VectorStorage` with the given dimensionality.
	#[inline]
	pub fn new(dimensions: usize) -> Self {
		Self {
			ids: Vec::new(),
			embeddings: Vec::new(),
			id_to_index: HashMap::new(),
			dimensions,
		}
	}

	/// Insert a single embedding.
	///
	/// If an entry with the same `id` already exists it is overwritten in-place.
	///
	/// # Panics
	///
	/// Panics if `embedding.len() != self.dimensions`.
	#[inline]
	pub fn insert(&mut self, id: &str, embedding: &[f32]) {
		assert_eq!(
			embedding.len(),
			self.dimensions,
			"embedding length {} does not match storage dimensions {}",
			embedding.len(),
			self.dimensions,
		);

		if let Some(&idx) = self.id_to_index.get(id) {
			// Overwrite existing slot.
			let start = idx * self.dimensions;
			self.embeddings[start..start + self.dimensions].copy_from_slice(embedding);
		} else {
			// Append new entry.
			let idx = self.ids.len();
			self.ids.push(id.to_owned());
			self.embeddings.extend_from_slice(embedding);
			self.id_to_index.insert(id.to_owned(), idx);
		}
	}

	/// Insert a batch of `(id, embedding)` pairs.
	///
	/// Equivalent to calling [`insert`](Self::insert) for each entry but
	/// pre-reserves capacity for the common case where all IDs are new.
	///
	/// # Panics
	///
	/// Panics if any embedding length does not match `self.dimensions`.
	pub fn insert_batch(&mut self, entries: &[(&str, &[f32])]) {
		// Reserve for the worst case (all new entries).
		self.ids.reserve(entries.len());
		self.embeddings.reserve(entries.len() * self.dimensions);
		self.id_to_index.reserve(entries.len());

		for &(id, embedding) in entries {
			self.insert(id, embedding);
		}
	}

	/// Remove an entry by ID, returning `true` if it existed.
	///
	/// Uses swap-remove: the last entry is moved into the vacated slot so the
	/// embeddings buffer remains contiguous with no holes.
	pub fn remove(&mut self, id: &str) -> bool {
		let Some(idx) = self.id_to_index.remove(id) else {
			return false;
		};

		let last = self.ids.len() - 1;

		if idx != last {
			// Move last entry into the vacated slot.

			// 1. Copy the last embedding over the removed one.
			let dst_start = idx * self.dimensions;
			let src_start = last * self.dimensions;
			// copy_within handles overlapping ranges (not the case here, but safe).
			self.embeddings
				.copy_within(src_start..src_start + self.dimensions, dst_start);

			// 2. Move the last ID into the vacated slot.
			self.ids.swap(idx, last);

			// 3. Update the index map for the moved entry.
			self.id_to_index.insert(self.ids[idx].clone(), idx);
		}

		// Pop the last slot (now either the removed entry or a duplicate of the
		// swapped entry).
		self.ids.pop();
		self.embeddings.truncate(last * self.dimensions);

		true
	}

	/// Retrieve the embedding for the given ID as a slice into the contiguous
	/// buffer, or `None` if the ID is not present.
	#[inline]
	pub fn get(&self, id: &str) -> Option<&[f32]> {
		let &idx = self.id_to_index.get(id)?;
		let start = idx * self.dimensions;
		Some(&self.embeddings[start..start + self.dimensions])
	}

	/// Returns `true` if the storage contains an entry with the given ID.
	#[inline]
	pub fn contains(&self, id: &str) -> bool {
		self.id_to_index.contains_key(id)
	}

	/// Iterate over `(id, embedding_slice)` pairs.
	#[inline]
	pub fn iter(&self) -> impl Iterator<Item = (&str, &[f32])> {
		self.ids.iter().enumerate().map(move |(i, id)| {
			let start = i * self.dimensions;
			(id.as_str(), &self.embeddings[start..start + self.dimensions])
		})
	}

	/// The number of stored entries.
	#[inline]
	pub fn len(&self) -> usize {
		self.ids.len()
	}

	/// Returns `true` if the storage contains no entries.
	#[inline]
	pub fn is_empty(&self) -> bool {
		self.ids.is_empty()
	}

	/// Remove all entries.
	#[inline]
	pub fn clear(&mut self) {
		self.ids.clear();
		self.embeddings.clear();
		self.id_to_index.clear();
	}

	/// The full contiguous embedding buffer.
	///
	/// Layout: `[v0_d0, v0_d1, ..., v1_d0, v1_d1, ...]` where each vector
	/// occupies `dimensions` consecutive floats.
	#[inline]
	pub fn raw_embeddings(&self) -> &[f32] {
		&self.embeddings
	}

	/// The dimensionality of each stored embedding.
	#[inline]
	pub fn dimensions(&self) -> usize {
		self.dimensions
	}
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;

	// -- insert and get -------------------------------------------------------

	#[test]
	fn insert_and_get_single() {
		let mut vs = VectorStorage::new(3);
		vs.insert("a", &[1.0, 2.0, 3.0]);

		assert_eq!(vs.len(), 1);
		assert!(!vs.is_empty());
		assert_eq!(vs.get("a"), Some([1.0, 2.0, 3.0].as_slice()));
	}

	#[test]
	fn insert_multiple_and_get() {
		let mut vs = VectorStorage::new(2);
		vs.insert("x", &[1.0, 2.0]);
		vs.insert("y", &[3.0, 4.0]);
		vs.insert("z", &[5.0, 6.0]);

		assert_eq!(vs.len(), 3);
		assert_eq!(vs.get("x"), Some([1.0, 2.0].as_slice()));
		assert_eq!(vs.get("y"), Some([3.0, 4.0].as_slice()));
		assert_eq!(vs.get("z"), Some([5.0, 6.0].as_slice()));
	}

	#[test]
	fn get_nonexistent_returns_none() {
		let vs = VectorStorage::new(2);
		assert_eq!(vs.get("missing"), None);
	}

	// -- insert_batch ---------------------------------------------------------

	#[test]
	fn insert_batch_and_get() {
		let mut vs = VectorStorage::new(2);
		vs.insert_batch(&[("a", &[1.0, 2.0]), ("b", &[3.0, 4.0]), ("c", &[5.0, 6.0])]);

		assert_eq!(vs.len(), 3);
		assert_eq!(vs.get("a"), Some([1.0, 2.0].as_slice()));
		assert_eq!(vs.get("b"), Some([3.0, 4.0].as_slice()));
		assert_eq!(vs.get("c"), Some([5.0, 6.0].as_slice()));
	}

	// -- duplicate id insert overwrites --------------------------------------

	#[test]
	fn duplicate_id_insert_overwrites() {
		let mut vs = VectorStorage::new(3);
		vs.insert("a", &[1.0, 2.0, 3.0]);
		vs.insert("a", &[7.0, 8.0, 9.0]);

		assert_eq!(vs.len(), 1);
		assert_eq!(vs.get("a"), Some([7.0, 8.0, 9.0].as_slice()));
	}

	#[test]
	fn duplicate_id_insert_batch_overwrites() {
		let mut vs = VectorStorage::new(2);
		vs.insert_batch(&[("a", &[1.0, 2.0]), ("a", &[9.0, 8.0])]);

		assert_eq!(vs.len(), 1);
		assert_eq!(vs.get("a"), Some([9.0, 8.0].as_slice()));
	}

	// -- remove ---------------------------------------------------------------

	#[test]
	fn remove_single_entry() {
		let mut vs = VectorStorage::new(2);
		vs.insert("a", &[1.0, 2.0]);

		assert!(vs.remove("a"));
		assert_eq!(vs.len(), 0);
		assert!(vs.is_empty());
		assert_eq!(vs.get("a"), None);
		assert!(vs.raw_embeddings().is_empty());
	}

	#[test]
	fn remove_nonexistent_returns_false() {
		let mut vs = VectorStorage::new(2);
		vs.insert("a", &[1.0, 2.0]);

		assert!(!vs.remove("missing"));
		assert_eq!(vs.len(), 1);
	}

	#[test]
	fn remove_swap_keeps_data_contiguous() {
		let mut vs = VectorStorage::new(2);
		vs.insert("a", &[1.0, 2.0]);
		vs.insert("b", &[3.0, 4.0]);
		vs.insert("c", &[5.0, 6.0]);

		// Remove the first entry; "c" (last) should be swapped into slot 0.
		assert!(vs.remove("a"));

		assert_eq!(vs.len(), 2);
		assert!(!vs.contains("a"));

		// Both remaining entries must be retrievable and correct.
		assert_eq!(vs.get("b"), Some([3.0, 4.0].as_slice()));
		assert_eq!(vs.get("c"), Some([5.0, 6.0].as_slice()));

		// Raw buffer must be exactly 2 * dimensions long (no holes).
		assert_eq!(vs.raw_embeddings().len(), 2 * 2);
	}

	#[test]
	fn remove_middle_element() {
		let mut vs = VectorStorage::new(3);
		vs.insert("a", &[1.0, 2.0, 3.0]);
		vs.insert("b", &[4.0, 5.0, 6.0]);
		vs.insert("c", &[7.0, 8.0, 9.0]);

		// Remove middle; "c" swaps into slot 1.
		assert!(vs.remove("b"));

		assert_eq!(vs.len(), 2);
		assert_eq!(vs.get("a"), Some([1.0, 2.0, 3.0].as_slice()));
		assert_eq!(vs.get("c"), Some([7.0, 8.0, 9.0].as_slice()));
		assert_eq!(vs.raw_embeddings().len(), 2 * 3);
	}

	#[test]
	fn remove_last_element_no_swap_needed() {
		let mut vs = VectorStorage::new(2);
		vs.insert("a", &[1.0, 2.0]);
		vs.insert("b", &[3.0, 4.0]);

		// Remove the last entry — no swap needed.
		assert!(vs.remove("b"));

		assert_eq!(vs.len(), 1);
		assert_eq!(vs.get("a"), Some([1.0, 2.0].as_slice()));
		assert_eq!(vs.raw_embeddings().len(), 2);
	}

	#[test]
	fn remove_all_entries_one_by_one() {
		let mut vs = VectorStorage::new(2);
		vs.insert("a", &[1.0, 2.0]);
		vs.insert("b", &[3.0, 4.0]);
		vs.insert("c", &[5.0, 6.0]);

		assert!(vs.remove("b"));
		assert!(vs.remove("a"));
		assert!(vs.remove("c"));

		assert!(vs.is_empty());
		assert_eq!(vs.len(), 0);
		assert!(vs.raw_embeddings().is_empty());
	}

	// -- contains -------------------------------------------------------------

	#[test]
	fn contains_present_and_absent() {
		let mut vs = VectorStorage::new(2);
		vs.insert("a", &[1.0, 2.0]);

		assert!(vs.contains("a"));
		assert!(!vs.contains("b"));
	}

	// -- iter -----------------------------------------------------------------

	#[test]
	fn iter_all_entries() {
		let mut vs = VectorStorage::new(2);
		vs.insert("a", &[1.0, 2.0]);
		vs.insert("b", &[3.0, 4.0]);

		let mut entries: Vec<_> = vs.iter().map(|(id, e)| (id.to_owned(), e.to_vec())).collect();
		entries.sort_by(|a, b| a.0.cmp(&b.0));

		assert_eq!(entries.len(), 2);
		assert_eq!(entries[0], ("a".to_owned(), vec![1.0, 2.0]));
		assert_eq!(entries[1], ("b".to_owned(), vec![3.0, 4.0]));
	}

	#[test]
	fn iter_empty_storage() {
		let vs = VectorStorage::new(4);
		assert_eq!(vs.iter().count(), 0);
	}

	// -- clear ----------------------------------------------------------------

	#[test]
	fn clear_removes_everything() {
		let mut vs = VectorStorage::new(2);
		vs.insert("a", &[1.0, 2.0]);
		vs.insert("b", &[3.0, 4.0]);

		vs.clear();

		assert!(vs.is_empty());
		assert_eq!(vs.len(), 0);
		assert!(vs.raw_embeddings().is_empty());
		assert_eq!(vs.get("a"), None);
		assert_eq!(vs.get("b"), None);
	}

	// -- contiguous layout verification --------------------------------------

	#[test]
	fn contiguous_layout_after_inserts() {
		let mut vs = VectorStorage::new(3);
		vs.insert("a", &[1.0, 2.0, 3.0]);
		vs.insert("b", &[4.0, 5.0, 6.0]);

		let raw = vs.raw_embeddings();
		assert_eq!(raw.len(), 6);

		// The order must match insertion order.
		assert_eq!(&raw[0..3], &[1.0, 2.0, 3.0]);
		assert_eq!(&raw[3..6], &[4.0, 5.0, 6.0]);
	}

	#[test]
	fn contiguous_layout_after_remove() {
		let mut vs = VectorStorage::new(2);
		vs.insert("a", &[1.0, 2.0]);
		vs.insert("b", &[3.0, 4.0]);
		vs.insert("c", &[5.0, 6.0]);

		vs.remove("a"); // "c" swaps to slot 0

		let raw = vs.raw_embeddings();
		assert_eq!(raw.len(), 4); // exactly 2 entries * 2 dims

		// After swap-remove of "a": slot 0 = former "c", slot 1 = "b" unchanged.
		assert_eq!(vs.get("c"), Some([5.0, 6.0].as_slice()));
		assert_eq!(vs.get("b"), Some([3.0, 4.0].as_slice()));
	}

	// -- empty storage operations --------------------------------------------

	#[test]
	fn empty_storage_properties() {
		let vs = VectorStorage::new(5);

		assert!(vs.is_empty());
		assert_eq!(vs.len(), 0);
		assert_eq!(vs.dimensions(), 5);
		assert!(vs.raw_embeddings().is_empty());
		assert_eq!(vs.get("anything"), None);
		assert!(!vs.contains("anything"));
	}

	#[test]
	fn clear_on_empty_is_noop() {
		let mut vs = VectorStorage::new(3);
		vs.clear();
		assert!(vs.is_empty());
	}

	#[test]
	fn remove_from_empty_returns_false() {
		let mut vs = VectorStorage::new(2);
		assert!(!vs.remove("x"));
	}

	// -- dimensions -----------------------------------------------------------

	#[test]
	fn dimensions_accessor() {
		let vs = VectorStorage::new(128);
		assert_eq!(vs.dimensions(), 128);
	}

	// -- panics ---------------------------------------------------------------

	#[test]
	#[should_panic(expected = "does not match storage dimensions")]
	fn insert_wrong_dimensions_panics() {
		let mut vs = VectorStorage::new(3);
		vs.insert("a", &[1.0, 2.0]); // 2 != 3
	}

	#[test]
	#[should_panic(expected = "does not match storage dimensions")]
	fn insert_batch_wrong_dimensions_panics() {
		let mut vs = VectorStorage::new(3);
		vs.insert_batch(&[("a", &[1.0, 2.0, 3.0]), ("b", &[4.0, 5.0])]);
	}
}
