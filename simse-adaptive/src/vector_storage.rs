use std::collections::HashMap;

/// Structure-of-Arrays vector storage for cache-friendly distance computation.
/// Embeddings stored contiguously: [v0_d0, v0_d1, ..., v1_d0, v1_d1, ...]
pub struct VectorStorage {
	ids: Vec<String>,
	embeddings: Vec<f32>,
	id_to_index: HashMap<String, usize>,
	dimensions: usize,
}

impl VectorStorage {
	pub fn new(dimensions: usize) -> Self {
		Self {
			ids: Vec::new(),
			embeddings: Vec::new(),
			id_to_index: HashMap::new(),
			dimensions,
		}
	}

	pub fn insert(&mut self, id: &str, embedding: &[f32]) {
		debug_assert_eq!(embedding.len(), self.dimensions);

		if let Some(&idx) = self.id_to_index.get(id) {
			// Update existing entry in-place.
			let start = idx * self.dimensions;
			self.embeddings[start..start + self.dimensions]
				.copy_from_slice(embedding);
			return;
		}

		let idx = self.ids.len();
		self.ids.push(id.to_string());
		self.embeddings.extend_from_slice(embedding);
		self.id_to_index.insert(id.to_string(), idx);
	}

	pub fn insert_batch(&mut self, entries: &[(&str, &[f32])]) {
		for &(id, embedding) in entries {
			self.insert(id, embedding);
		}
	}

	/// Remove an entry by id. Uses swap-remove for O(1) removal while
	/// keeping embeddings contiguous.
	pub fn remove(&mut self, id: &str) -> bool {
		let Some(&idx) = self.id_to_index.get(id) else {
			return false;
		};

		let last_idx = self.ids.len() - 1;

		if idx != last_idx {
			// Swap the last entry into the vacated slot.
			let last_id = self.ids[last_idx].clone();

			self.ids.swap(idx, last_idx);

			let src_start = last_idx * self.dimensions;
			let dst_start = idx * self.dimensions;
			for d in 0..self.dimensions {
				self.embeddings[dst_start + d] = self.embeddings[src_start + d];
			}

			*self.id_to_index.get_mut(&last_id).unwrap() = idx;
		}

		self.ids.pop();
		self.embeddings.truncate(self.ids.len() * self.dimensions);
		self.id_to_index.remove(id);
		true
	}

	pub fn get(&self, id: &str) -> Option<&[f32]> {
		let &idx = self.id_to_index.get(id)?;
		let start = idx * self.dimensions;
		Some(&self.embeddings[start..start + self.dimensions])
	}

	pub fn contains(&self, id: &str) -> bool {
		self.id_to_index.contains_key(id)
	}

	pub fn iter(&self) -> impl Iterator<Item = (&str, &[f32])> {
		self.ids.iter().enumerate().map(move |(i, id)| {
			let start = i * self.dimensions;
			(id.as_str(), &self.embeddings[start..start + self.dimensions])
		})
	}

	pub fn len(&self) -> usize {
		self.ids.len()
	}

	pub fn is_empty(&self) -> bool {
		self.ids.is_empty()
	}

	pub fn clear(&mut self) {
		self.ids.clear();
		self.embeddings.clear();
		self.id_to_index.clear();
	}

	pub fn raw_embeddings(&self) -> &[f32] {
		&self.embeddings
	}

	pub fn dimensions(&self) -> usize {
		self.dimensions
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn insert_and_get() {
		let mut vs = VectorStorage::new(3);
		vs.insert("a", &[1.0, 2.0, 3.0]);
		assert_eq!(vs.len(), 1);
		assert_eq!(vs.get("a"), Some(&[1.0f32, 2.0, 3.0][..]));
	}

	#[test]
	fn insert_batch_and_get() {
		let mut vs = VectorStorage::new(2);
		vs.insert_batch(&[("a", &[1.0, 2.0]), ("b", &[3.0, 4.0])]);
		assert_eq!(vs.len(), 2);
		assert_eq!(vs.get("a"), Some(&[1.0f32, 2.0][..]));
		assert_eq!(vs.get("b"), Some(&[3.0f32, 4.0][..]));
	}

	#[test]
	fn remove() {
		let mut vs = VectorStorage::new(2);
		vs.insert("a", &[1.0, 2.0]);
		vs.insert("b", &[3.0, 4.0]);
		vs.remove("a");
		assert_eq!(vs.len(), 1);
		assert_eq!(vs.get("a"), None);
		assert_eq!(vs.get("b"), Some(&[3.0f32, 4.0][..]));
	}

	#[test]
	fn iter_embeddings() {
		let mut vs = VectorStorage::new(2);
		vs.insert("a", &[1.0, 2.0]);
		vs.insert("b", &[3.0, 4.0]);
		let all: Vec<_> = vs.iter().collect();
		assert_eq!(all.len(), 2);
	}

	#[test]
	fn clear() {
		let mut vs = VectorStorage::new(2);
		vs.insert("a", &[1.0, 2.0]);
		vs.clear();
		assert_eq!(vs.len(), 0);
	}

	#[test]
	fn contiguous_layout() {
		let mut vs = VectorStorage::new(3);
		vs.insert("a", &[1.0, 2.0, 3.0]);
		vs.insert("b", &[4.0, 5.0, 6.0]);
		// Internal buffer should be contiguous [1,2,3,4,5,6]
		assert_eq!(vs.raw_embeddings().len(), 6);
	}
}
