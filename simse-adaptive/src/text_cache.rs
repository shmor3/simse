// ---------------------------------------------------------------------------
// LRU Text Cache
// ---------------------------------------------------------------------------
//
// Keeps frequently accessed entry texts in memory to avoid repeated disk
// reads during search result hydration. Evicts by entry count and total
// byte budget (whichever limit is hit first).
// ---------------------------------------------------------------------------

use im::{HashMap, Vector};

// ---------------------------------------------------------------------------
// TextCache
// ---------------------------------------------------------------------------

/// An LRU text cache with both entry-count and byte-budget limits.
///
/// Internally uses an `im::Vector<(String, String)>` for ordering (oldest first)
/// and an `im::HashMap<String, usize>` for O(1) lookups by ID.
#[derive(Clone)]
pub struct TextCache {
	max_entries: usize,
	max_bytes: usize,
	/// Ordered entries: oldest first, newest last. Each element is (id, text).
	entries: Vector<(String, String)>,
	/// Maps id -> index in `entries`.
	index: HashMap<String, usize>,
	total_bytes: usize,
}

impl TextCache {
	/// Create a new `TextCache`.
	///
	/// * `max_entries` — maximum number of cached entries (default 500)
	/// * `max_bytes` — maximum total UTF-8 bytes (default 5,242,880 = 5 MB)
	pub fn new(max_entries: usize, max_bytes: usize) -> Self {
		Self {
			max_entries,
			max_bytes,
			entries: Vector::new(),
			index: HashMap::new(),
			total_bytes: 0,
		}
	}

	/// Rebuild the `index` map from `entries`. Called after removals that
	/// shift positions.
	fn rebuild_index(entries: &Vector<(String, String)>) -> HashMap<String, usize> {
		let mut index = HashMap::new();
		for (i, (id, _)) in entries.iter().enumerate() {
			index = index.update(id.clone(), i);
		}
		index
	}

	/// Evict the oldest entries until both limits are satisfied.
	fn evict(self) -> Self {
		let mut entries = self.entries;
		let mut total_bytes = self.total_bytes;

		while entries.len() > self.max_entries || total_bytes > self.max_bytes {
			if entries.is_empty() {
				break;
			}
			let (_, text) = entries.remove(0);
			total_bytes -= text.len();
		}
		let index = Self::rebuild_index(&entries);
		Self { entries, index, total_bytes, ..self }
	}

	/// Get a cached text by entry ID. Returns `None` on miss.
	/// On hit, promotes the entry to most-recently-used.
	pub fn get(self, id: &str) -> (Self, Option<String>) {
		let idx = match self.index.get(id).copied() {
			Some(idx) => idx,
			None => return (self, None),
		};
		let mut entries = self.entries;
		let (entry_id, text) = entries.remove(idx);
		entries.push_back((entry_id, text.clone()));
		let index = Self::rebuild_index(&entries);
		(Self { entries, index, ..self }, Some(text))
	}

	/// Put a text into the cache, promoting it to most-recently-used.
	/// If the id already exists, its old value is replaced.
	pub fn put(self, id: &str, text: &str) -> Self {
		let mut entries = self.entries;
		let mut total_bytes = self.total_bytes;
		let mut index = self.index;

		// Remove old entry if exists
		if let Some(&idx) = index.get(id) {
			let (_, old_text) = entries.remove(idx);
			total_bytes -= old_text.len();
			index = Self::rebuild_index(&entries);
		}

		let bytes = text.len();
		total_bytes += bytes;
		let new_idx = entries.len();
		entries.push_back((id.to_string(), text.to_string()));
		index = index.update(id.to_string(), new_idx);

		let result = Self { entries, index, total_bytes, ..self };
		result.evict()
	}

	/// Remove a specific entry from the cache.
	pub fn remove(self, id: &str) -> (Self, bool) {
		if let Some(&idx) = self.index.get(id) {
			let mut entries = self.entries;
			let (_, text) = entries.remove(idx);
			let total_bytes = self.total_bytes - text.len();
			let index = Self::rebuild_index(&entries);
			(Self { entries, index, total_bytes, ..self }, true)
		} else {
			(self, false)
		}
	}

	/// Clear all cached entries.
	pub fn clear(self) -> Self {
		Self {
			entries: Vector::new(),
			index: HashMap::new(),
			total_bytes: 0,
			..self
		}
	}

	/// Number of entries currently in the cache.
	pub fn size(&self) -> usize {
		self.entries.len()
	}

	/// Total UTF-8 bytes currently used by cached texts.
	pub fn bytes(&self) -> usize {
		self.total_bytes
	}
}

impl Default for TextCache {
	fn default() -> Self {
		Self::new(500, 5_242_880)
	}
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn put_and_get() {
		let cache = TextCache::new(10, 1_000_000);
		let cache = cache.put("a", "hello");
		let (cache, val) = cache.get("a");
		assert_eq!(val, Some("hello".to_string()));
		assert_eq!(cache.size(), 1);
		assert_eq!(cache.bytes(), 5);
	}

	#[test]
	fn get_miss_returns_none() {
		let cache = TextCache::new(10, 1_000_000);
		let (_cache, val) = cache.get("missing");
		assert_eq!(val, None);
	}

	#[test]
	fn put_replaces_existing() {
		let cache = TextCache::new(10, 1_000_000);
		let cache = cache.put("a", "hello");
		let cache = cache.put("a", "world!");
		let (cache, val) = cache.get("a");
		assert_eq!(val, Some("world!".to_string()));
		assert_eq!(cache.size(), 1);
		assert_eq!(cache.bytes(), 6); // "world!" is 6 bytes
	}

	#[test]
	fn evict_by_entry_count() {
		let cache = TextCache::new(3, 1_000_000);
		let cache = cache.put("a", "1");
		let cache = cache.put("b", "2");
		let cache = cache.put("c", "3");
		let cache = cache.put("d", "4");
		// "a" should be evicted (oldest)
		assert_eq!(cache.size(), 3);
		let (cache, val_a) = cache.get("a");
		assert_eq!(val_a, None);
		let (cache, val_b) = cache.get("b");
		assert_eq!(val_b, Some("2".to_string()));
		let (cache, val_c) = cache.get("c");
		assert_eq!(val_c, Some("3".to_string()));
		let (_cache, val_d) = cache.get("d");
		assert_eq!(val_d, Some("4".to_string()));
	}

	#[test]
	fn evict_by_byte_budget() {
		// Each "hello" is 5 bytes, max is 12 => at most 2 entries fit
		let cache = TextCache::new(100, 12);
		let cache = cache.put("a", "hello");
		let cache = cache.put("b", "hello");
		assert_eq!(cache.size(), 2);
		assert_eq!(cache.bytes(), 10);

		let cache = cache.put("c", "hello");
		// Now 15 bytes > 12, so "a" (oldest) should be evicted
		assert_eq!(cache.size(), 2);
		assert_eq!(cache.bytes(), 10);
		let (_cache, val_a) = cache.get("a");
		assert_eq!(val_a, None);
	}

	#[test]
	fn get_promotes_to_mru() {
		let cache = TextCache::new(3, 1_000_000);
		let cache = cache.put("a", "1");
		let cache = cache.put("b", "2");
		let cache = cache.put("c", "3");

		// Access "a" to promote it
		let (cache, _) = cache.get("a");

		// Insert "d" — should evict "b" (now the oldest)
		let cache = cache.put("d", "4");
		let (cache, val_a) = cache.get("a");
		assert_eq!(val_a, Some("1".to_string()));
		let (_cache, val_b) = cache.get("b");
		assert_eq!(val_b, None);
	}

	#[test]
	fn remove_entry() {
		let cache = TextCache::new(10, 1_000_000);
		let cache = cache.put("a", "hello");
		let cache = cache.put("b", "world");
		let (cache, removed) = cache.remove("a");
		assert!(removed);
		let (cache, val_a) = cache.get("a");
		assert_eq!(val_a, None);
		assert_eq!(cache.size(), 1);
		assert_eq!(cache.bytes(), 5); // "world" = 5 bytes
	}

	#[test]
	fn remove_nonexistent_returns_false() {
		let cache = TextCache::new(10, 1_000_000);
		let (_cache, removed) = cache.remove("missing");
		assert!(!removed);
	}

	#[test]
	fn clear_empties_cache() {
		let cache = TextCache::new(10, 1_000_000);
		let cache = cache.put("a", "hello");
		let cache = cache.put("b", "world");
		let cache = cache.clear();
		assert_eq!(cache.size(), 0);
		assert_eq!(cache.bytes(), 0);
		let (_cache, val_a) = cache.get("a");
		assert_eq!(val_a, None);
	}

	#[test]
	fn default_limits() {
		let cache = TextCache::default();
		assert_eq!(cache.max_entries, 500);
		assert_eq!(cache.max_bytes, 5_242_880);
	}

	#[test]
	fn utf8_byte_counting() {
		let cache = TextCache::new(100, 1_000_000);
		// "cafe" with accent: "caf\u{00e9}" is 5 UTF-8 bytes (c=1, a=1, f=1, e-acute=2)
		let text = "caf\u{00e9}";
		let cache = cache.put("a", text);
		assert_eq!(cache.bytes(), text.len()); // Rust .len() returns UTF-8 bytes
		assert_eq!(cache.bytes(), 5);
	}

	#[test]
	fn multiple_evictions_for_large_entry() {
		// Max 10 bytes, insert entries of 3 bytes each, then one of 8 bytes
		let cache = TextCache::new(100, 10);
		let cache = cache.put("a", "aaa"); // 3 bytes, total 3
		let cache = cache.put("b", "bbb"); // 3 bytes, total 6
		let cache = cache.put("c", "ccc"); // 3 bytes, total 9
		let cache = cache.put("d", "dddddddd"); // 8 bytes, total 17 => evict a, b, c => total 8
		assert_eq!(cache.size(), 1);
		assert_eq!(cache.bytes(), 8);
		let (cache, val_a) = cache.get("a");
		assert_eq!(val_a, None);
		let (cache, val_b) = cache.get("b");
		assert_eq!(val_b, None);
		let (cache, val_c) = cache.get("c");
		assert_eq!(val_c, None);
		let (_cache, val_d) = cache.get("d");
		assert_eq!(val_d, Some("dddddddd".to_string()));
	}
}
