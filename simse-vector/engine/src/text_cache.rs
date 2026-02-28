// ---------------------------------------------------------------------------
// LRU Text Cache
// ---------------------------------------------------------------------------
//
// Keeps frequently accessed entry texts in memory to avoid repeated disk
// reads during search result hydration. Evicts by entry count and total
// byte budget (whichever limit is hit first).
//
// Ported from the TypeScript implementation in src/text-cache.ts.
// ---------------------------------------------------------------------------

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// TextCache
// ---------------------------------------------------------------------------

/// An LRU text cache with both entry-count and byte-budget limits.
///
/// Internally uses a `Vec<(String, String)>` for ordering (oldest first)
/// and a `HashMap<String, usize>` for O(1) lookups by ID.
pub struct TextCache {
	max_entries: usize,
	max_bytes: usize,
	/// Ordered entries: oldest first, newest last. Each element is (id, text).
	entries: Vec<(String, String)>,
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
			entries: Vec::new(),
			index: HashMap::new(),
			total_bytes: 0,
		}
	}

	/// Rebuild the `index` map from `entries`. Called after removals that
	/// shift positions.
	fn rebuild_index(&mut self) {
		self.index.clear();
		for (i, (id, _)) in self.entries.iter().enumerate() {
			self.index.insert(id.clone(), i);
		}
	}

	/// Evict the oldest entries until both limits are satisfied.
	fn evict(&mut self) {
		while self.entries.len() > self.max_entries || self.total_bytes > self.max_bytes {
			if self.entries.is_empty() {
				break;
			}
			let (_, text) = self.entries.remove(0);
			self.total_bytes -= text.len();
		}
		self.rebuild_index();
	}

	/// Get a cached text by entry ID. Returns `None` on miss.
	/// On hit, promotes the entry to most-recently-used.
	pub fn get(&mut self, id: &str) -> Option<String> {
		let idx = self.index.get(id).copied()?;
		let (entry_id, text) = self.entries.remove(idx);
		self.entries.push((entry_id, text.clone()));
		self.rebuild_index();
		Some(text)
	}

	/// Put a text into the cache, promoting it to most-recently-used.
	/// If the id already exists, its old value is replaced.
	pub fn put(&mut self, id: &str, text: &str) {
		// Remove old entry if exists
		if let Some(idx) = self.index.remove(id) {
			let (_, old_text) = self.entries.remove(idx);
			self.total_bytes -= old_text.len();
			self.rebuild_index();
		}

		let bytes = text.len();
		self.total_bytes += bytes;
		let new_idx = self.entries.len();
		self.entries.push((id.to_string(), text.to_string()));
		self.index.insert(id.to_string(), new_idx);

		self.evict();
	}

	/// Remove a specific entry from the cache.
	pub fn remove(&mut self, id: &str) -> bool {
		if let Some(idx) = self.index.remove(id) {
			let (_, text) = self.entries.remove(idx);
			self.total_bytes -= text.len();
			self.rebuild_index();
			true
		} else {
			false
		}
	}

	/// Clear all cached entries.
	pub fn clear(&mut self) {
		self.entries.clear();
		self.index.clear();
		self.total_bytes = 0;
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
		let mut cache = TextCache::new(10, 1_000_000);
		cache.put("a", "hello");
		assert_eq!(cache.get("a"), Some("hello".to_string()));
		assert_eq!(cache.size(), 1);
		assert_eq!(cache.bytes(), 5);
	}

	#[test]
	fn get_miss_returns_none() {
		let mut cache = TextCache::new(10, 1_000_000);
		assert_eq!(cache.get("missing"), None);
	}

	#[test]
	fn put_replaces_existing() {
		let mut cache = TextCache::new(10, 1_000_000);
		cache.put("a", "hello");
		cache.put("a", "world!");
		assert_eq!(cache.get("a"), Some("world!".to_string()));
		assert_eq!(cache.size(), 1);
		assert_eq!(cache.bytes(), 6); // "world!" is 6 bytes
	}

	#[test]
	fn evict_by_entry_count() {
		let mut cache = TextCache::new(3, 1_000_000);
		cache.put("a", "1");
		cache.put("b", "2");
		cache.put("c", "3");
		cache.put("d", "4");
		// "a" should be evicted (oldest)
		assert_eq!(cache.size(), 3);
		assert_eq!(cache.get("a"), None);
		assert_eq!(cache.get("b"), Some("2".to_string()));
		assert_eq!(cache.get("c"), Some("3".to_string()));
		assert_eq!(cache.get("d"), Some("4".to_string()));
	}

	#[test]
	fn evict_by_byte_budget() {
		// Each "hello" is 5 bytes, max is 12 => at most 2 entries fit
		let mut cache = TextCache::new(100, 12);
		cache.put("a", "hello");
		cache.put("b", "hello");
		assert_eq!(cache.size(), 2);
		assert_eq!(cache.bytes(), 10);

		cache.put("c", "hello");
		// Now 15 bytes > 12, so "a" (oldest) should be evicted
		assert_eq!(cache.size(), 2);
		assert_eq!(cache.bytes(), 10);
		assert_eq!(cache.get("a"), None);
	}

	#[test]
	fn get_promotes_to_mru() {
		let mut cache = TextCache::new(3, 1_000_000);
		cache.put("a", "1");
		cache.put("b", "2");
		cache.put("c", "3");

		// Access "a" to promote it
		cache.get("a");

		// Insert "d" — should evict "b" (now the oldest)
		cache.put("d", "4");
		assert_eq!(cache.get("a"), Some("1".to_string()));
		assert_eq!(cache.get("b"), None);
	}

	#[test]
	fn remove_entry() {
		let mut cache = TextCache::new(10, 1_000_000);
		cache.put("a", "hello");
		cache.put("b", "world");
		assert!(cache.remove("a"));
		assert_eq!(cache.get("a"), None);
		assert_eq!(cache.size(), 1);
		assert_eq!(cache.bytes(), 5); // "world" = 5 bytes
	}

	#[test]
	fn remove_nonexistent_returns_false() {
		let mut cache = TextCache::new(10, 1_000_000);
		assert!(!cache.remove("missing"));
	}

	#[test]
	fn clear_empties_cache() {
		let mut cache = TextCache::new(10, 1_000_000);
		cache.put("a", "hello");
		cache.put("b", "world");
		cache.clear();
		assert_eq!(cache.size(), 0);
		assert_eq!(cache.bytes(), 0);
		assert_eq!(cache.get("a"), None);
	}

	#[test]
	fn default_limits() {
		let cache = TextCache::default();
		assert_eq!(cache.max_entries, 500);
		assert_eq!(cache.max_bytes, 5_242_880);
	}

	#[test]
	fn utf8_byte_counting() {
		let mut cache = TextCache::new(100, 1_000_000);
		// "cafe" with accent: "caf\u{00e9}" is 5 UTF-8 bytes (c=1, a=1, f=1, e-acute=2)
		let text = "caf\u{00e9}";
		cache.put("a", text);
		assert_eq!(cache.bytes(), text.len()); // Rust .len() returns UTF-8 bytes
		assert_eq!(cache.bytes(), 5);
	}

	#[test]
	fn multiple_evictions_for_large_entry() {
		// Max 10 bytes, insert entries of 3 bytes each, then one of 8 bytes
		let mut cache = TextCache::new(100, 10);
		cache.put("a", "aaa"); // 3 bytes, total 3
		cache.put("b", "bbb"); // 3 bytes, total 6
		cache.put("c", "ccc"); // 3 bytes, total 9
		cache.put("d", "dddddddd"); // 8 bytes, total 17 => evict a, b, c => total 8
		assert_eq!(cache.size(), 1);
		assert_eq!(cache.bytes(), 8);
		assert_eq!(cache.get("a"), None);
		assert_eq!(cache.get("b"), None);
		assert_eq!(cache.get("c"), None);
		assert_eq!(cache.get("d"), Some("dddddddd".to_string()));
	}
}
