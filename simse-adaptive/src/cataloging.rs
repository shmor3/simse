// ---------------------------------------------------------------------------
// Cataloging — TopicIndex, MetadataIndex, MagnitudeCache
// ---------------------------------------------------------------------------
//
// In-memory indexes to accelerate vector store lookups.
// Ports the TypeScript `cataloging.ts` (603 lines).
// ---------------------------------------------------------------------------

use std::collections::{HashMap, HashSet};

use crate::cosine::compute_magnitude;
use crate::types::TopicInfo;

// ---------------------------------------------------------------------------
// Default stop words for topic extraction
// ---------------------------------------------------------------------------

fn default_stop_words() -> HashSet<String> {
	[
		"a", "an", "and", "are", "as", "at", "be", "but", "by", "do", "for", "from", "had",
		"has", "have", "he", "her", "his", "how", "i", "if", "in", "into", "is", "it", "its",
		"my", "no", "not", "of", "on", "or", "our", "she", "so", "that", "the", "their", "them",
		"then", "there", "these", "they", "this", "to", "was", "we", "were", "what", "when",
		"which", "who", "will", "with", "you", "your",
	]
	.iter()
	.map(|s| s.to_string())
	.collect()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Get the parent path of a hierarchical topic (e.g. "code/rust" -> "code").
/// Returns `None` for root-level topics with no '/'.
fn get_parent_path(topic: &str) -> Option<String> {
	let idx = topic.rfind('/');
	idx.map(|i| topic[..i].to_string())
}

/// Create an order-independent pair key for co-occurrence tracking.
fn co_occurrence_key(a: &str, b: &str) -> String {
	if a < b {
		format!("{}\0{}", a, b)
	} else {
		format!("{}\0{}", b, a)
	}
}

/// Extract topics from text by word frequency analysis.
///
/// 1. Lowercase
/// 2. Remove non-alphanumeric characters (keep whitespace for splitting)
/// 3. Split on whitespace, filter words > 2 chars and not in stop words
/// 4. Count frequencies, sort by freq desc then alphabetically
/// 5. Take top N
fn extract_topics_from_text(
	text: &str,
	stop_words: &HashSet<String>,
	max_topics: usize,
) -> Vec<String> {
	let lowered = text.to_lowercase();
	// Remove non-alphanumeric, non-whitespace characters
	let cleaned: String = lowered
		.chars()
		.map(|c| if c.is_alphanumeric() || c.is_whitespace() { c } else { ' ' })
		.collect();

	let mut freq: HashMap<String, usize> = HashMap::new();
	for word in cleaned.split_whitespace() {
		if word.len() > 2 && !stop_words.contains(word) {
			*freq.entry(word.to_string()).or_insert(0) += 1;
		}
	}

	let mut entries: Vec<(String, usize)> = freq.into_iter().collect();
	// Sort by frequency descending, then alphabetically ascending
	entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
	entries.truncate(max_topics);
	entries.into_iter().map(|(word, _)| word).collect()
}

/// Resolve topics from metadata + text fallback.
///
/// Priority:
/// 1. `metadata["topics"]` — JSON-stringified string array (multi-topic)
/// 2. `metadata["topic"]` — single string (comma-separated supported)
/// 3. Auto-extract from text via word frequency
fn resolve_topics(
	text: &str,
	metadata: &HashMap<String, String>,
	stop_words: &HashSet<String>,
	max_topics: usize,
) -> Vec<String> {
	// 1. metadata.topics (JSON array)
	if let Some(topics_json) = metadata.get("topics") {
		if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(topics_json) {
			if let Some(arr) = parsed.as_array() {
				let topics: Vec<String> = arr
					.iter()
					.filter_map(|v| v.as_str().map(|s| s.trim().to_lowercase()))
					.filter(|s| !s.is_empty())
					.collect();
				if !topics.is_empty() {
					return topics;
				}
			}
		}
	}

	// 2. metadata.topic (single string, comma-separated)
	if let Some(topic) = metadata.get("topic") {
		let topics: Vec<String> = topic
			.split(',')
			.map(|t| t.trim().to_lowercase())
			.filter(|t| !t.is_empty())
			.collect();
		if !topics.is_empty() {
			return topics;
		}
	}

	// 3. Auto-extract from text
	extract_topics_from_text(text, stop_words, max_topics)
}

// ---------------------------------------------------------------------------
// TopicIndex
// ---------------------------------------------------------------------------

/// Tracks topics associated with entries (volumes). Topics can be hierarchical
/// (e.g. "code/rust" is a child of "code").
pub struct TopicIndex {
	/// topic -> direct entry IDs
	topic_to_entries: HashMap<String, HashSet<String>>,
	/// entry ID -> topic paths
	entry_to_topics: HashMap<String, Vec<String>>,
	/// topic -> direct child topics
	topic_to_children: HashMap<String, HashSet<String>>,
	/// pair key -> count
	co_occurrence: HashMap<String, usize>,
	/// words to ignore during auto-extraction
	stop_words: HashSet<String>,
	/// max auto-extracted topics per entry
	max_topics_per_entry: usize,
}

impl TopicIndex {
	/// Create a new TopicIndex.
	///
	/// - `max_topics`: maximum number of auto-extracted topics per entry (default 5)
	/// - `extra_stop_words`: additional words to ignore during topic extraction
	pub fn new(max_topics: usize, extra_stop_words: &[&str]) -> Self {
		let mut stop_words = default_stop_words();
		for w in extra_stop_words {
			stop_words.insert(w.to_lowercase());
		}
		Self {
			topic_to_entries: HashMap::new(),
			entry_to_topics: HashMap::new(),
			topic_to_children: HashMap::new(),
			co_occurrence: HashMap::new(),
			stop_words,
			max_topics_per_entry: max_topics,
		}
	}

	/// Ensure a topic and all its ancestors exist in the index structures.
	fn ensure_topic_exists(&mut self, topic: &str) {
		if !self.topic_to_entries.contains_key(topic) {
			self.topic_to_entries.insert(topic.to_string(), HashSet::new());
		}
		if let Some(parent) = get_parent_path(topic) {
			self.ensure_topic_exists(&parent);
			self.topic_to_children
				.entry(parent)
				.or_default()
				.insert(topic.to_string());
		}
	}

	/// Clean up a topic node if it has no direct entries and no children.
	/// Recursively cleans up ancestors.
	fn cleanup_topic(&mut self, topic: &str) {
		let has_entries = self
			.topic_to_entries
			.get(topic)
			.map_or(true, |e| !e.is_empty());
		let has_children = self
			.topic_to_children
			.get(topic)
			.map_or(false, |c| !c.is_empty());

		if !has_entries && !has_children {
			self.topic_to_entries.remove(topic);
			self.topic_to_children.remove(topic);
			if let Some(parent) = get_parent_path(topic) {
				if let Some(children) = self.topic_to_children.get_mut(&parent) {
					children.remove(topic);
				}
				self.cleanup_topic(&parent);
			}
		}
	}

	/// Increment pairwise co-occurrence counters for a set of topics.
	fn increment_co_occurrence(&mut self, topics: &[String]) {
		for i in 0..topics.len() {
			for j in (i + 1)..topics.len() {
				let key = co_occurrence_key(&topics[i], &topics[j]);
				*self.co_occurrence.entry(key).or_insert(0) += 1;
			}
		}
	}

	/// Decrement pairwise co-occurrence counters for a set of topics.
	fn decrement_co_occurrence(&mut self, topics: &[String]) {
		for i in 0..topics.len() {
			for j in (i + 1)..topics.len() {
				let key = co_occurrence_key(&topics[i], &topics[j]);
				if let Some(current) = self.co_occurrence.get(&key).copied() {
					if current <= 1 {
						self.co_occurrence.remove(&key);
					} else {
						self.co_occurrence.insert(key, current - 1);
					}
				}
			}
		}
	}

	/// Collect all entry IDs for a topic and all its descendants.
	fn collect_descendant_entries(&self, topic: &str) -> Vec<String> {
		let mut result = HashSet::new();
		if let Some(direct) = self.topic_to_entries.get(topic) {
			for id in direct {
				result.insert(id.clone());
			}
		}
		if let Some(children) = self.topic_to_children.get(topic) {
			for child in children {
				for id in self.collect_descendant_entries(child) {
					result.insert(id);
				}
			}
		}
		result.into_iter().collect()
	}

	/// Add an entry to the index, extracting topics from text and metadata.
	///
	/// If the entry already exists, it is removed first (re-indexing).
	pub fn add_entry(&mut self, id: &str, text: &str, metadata: &HashMap<String, String>) {
		// Remove existing mapping if re-indexing
		self.remove_entry(id);

		let topics = resolve_topics(text, metadata, &self.stop_words, self.max_topics_per_entry);
		self.entry_to_topics.insert(id.to_string(), topics.clone());

		for topic in &topics {
			self.ensure_topic_exists(topic);
			if let Some(set) = self.topic_to_entries.get_mut(topic) {
				set.insert(id.to_string());
			}
		}

		// Track co-occurrence between all topics on this entry
		if topics.len() > 1 {
			self.increment_co_occurrence(&topics);
		}
	}

	/// Remove an entry from the index. Cleans up empty topics.
	pub fn remove_entry(&mut self, id: &str) {
		let topics = match self.entry_to_topics.remove(id) {
			Some(t) => t,
			None => return,
		};

		// Decrement co-occurrence before removing
		if topics.len() > 1 {
			self.decrement_co_occurrence(&topics);
		}

		for topic in &topics {
			if let Some(set) = self.topic_to_entries.get_mut(topic) {
				set.remove(id);
			}
			self.cleanup_topic(topic);
		}
	}

	/// Get all entry IDs associated with a topic and its descendants.
	pub fn get_entries(&self, topic: &str) -> Vec<String> {
		let normalized = topic.to_lowercase();
		self.collect_descendant_entries(&normalized)
	}

	/// List all known topics with hierarchy info.
	pub fn get_all_topics(&self) -> Vec<TopicInfo> {
		let mut result = Vec::new();
		for (topic, entries) in &self.topic_to_entries {
			let children = self
				.topic_to_children
				.get(topic)
				.map(|c| c.iter().cloned().collect::<Vec<_>>())
				.unwrap_or_default();
			result.push(TopicInfo {
				topic: topic.clone(),
				entry_count: entries.len(),
				entry_ids: entries.iter().cloned().collect(),
				parent: get_parent_path(topic),
				children,
			});
		}
		result
	}

	/// Get topics for a specific entry.
	pub fn get_topics(&self, id: &str) -> Vec<String> {
		self.entry_to_topics.get(id).cloned().unwrap_or_default()
	}

	/// Get topics that co-occur with the given topic, sorted by count descending.
	pub fn get_related_topics(&self, topic: &str) -> Vec<(String, usize)> {
		let normalized = topic.to_lowercase();
		let mut related: HashMap<String, usize> = HashMap::new();
		for (key, count) in &self.co_occurrence {
			let parts: Vec<&str> = key.split('\0').collect();
			if parts.len() == 2 {
				if parts[0] == normalized {
					*related.entry(parts[1].to_string()).or_insert(0) += count;
				} else if parts[1] == normalized {
					*related.entry(parts[0].to_string()).or_insert(0) += count;
				}
			}
		}
		let mut result: Vec<(String, usize)> = related.into_iter().collect();
		result.sort_by(|a, b| b.1.cmp(&a.1));
		result
	}

	/// Move all entries from one topic to another. Update co-occurrence.
	pub fn merge_topic(&mut self, from: &str, to: &str) {
		let from_norm = from.to_lowercase();
		let to_norm = to.to_lowercase();

		// Collect from entries
		let from_entry_ids: Vec<String> = match self.topic_to_entries.get(&from_norm) {
			Some(entries) if !entries.is_empty() => entries.iter().cloned().collect(),
			_ => return,
		};

		// Ensure the target topic exists
		self.ensure_topic_exists(&to_norm);

		// Move each entry from `from` to `to`
		for id in &from_entry_ids {
			// Add to target topic
			if let Some(to_entries) = self.topic_to_entries.get_mut(&to_norm) {
				to_entries.insert(id.clone());
			}

			// Update the entry-to-topics mapping:
			// Clone topics out to avoid holding a mutable borrow on entry_to_topics
			// while calling co-occurrence methods that also need &mut self.
			if let Some(old_topics) = self.entry_to_topics.get(id).cloned() {
				// Decrement old co-occurrence for this entry's topic set
				if old_topics.len() > 1 {
					self.decrement_co_occurrence(&old_topics);
				}

				// Build updated topic list
				let mut new_topics = old_topics;
				if let Some(idx) = new_topics.iter().position(|t| *t == from_norm) {
					if new_topics.contains(&to_norm) {
						// Avoid duplicates: just remove
						new_topics.remove(idx);
					} else {
						new_topics[idx] = to_norm.clone();
					}
				}

				// Increment new co-occurrence for updated topic set
				if new_topics.len() > 1 {
					self.increment_co_occurrence(&new_topics);
				}

				// Write updated topics back
				self.entry_to_topics.insert(id.clone(), new_topics);
			}
		}

		// Clear the `from` topic entries and clean up
		if let Some(from_entries) = self.topic_to_entries.get_mut(&from_norm) {
			from_entries.clear();
		}
		self.cleanup_topic(&from_norm);

		// Move co-occurrence counters that reference `from` to `to`
		let keys_to_remove: Vec<String> = self
			.co_occurrence
			.keys()
			.filter(|key| {
				let parts: Vec<&str> = key.split('\0').collect();
				parts.len() == 2 && (parts[0] == from_norm || parts[1] == from_norm)
			})
			.cloned()
			.collect();

		let mut updates: HashMap<String, usize> = HashMap::new();
		for key in &keys_to_remove {
			let count = self.co_occurrence.get(key).copied().unwrap_or(0);
			let parts: Vec<&str> = key.split('\0').collect();
			let other = if parts[0] == from_norm { parts[1] } else { parts[0] };
			if other != to_norm {
				let new_key = co_occurrence_key(&to_norm, other);
				let existing = updates
					.get(&new_key)
					.copied()
					.or_else(|| self.co_occurrence.get(&new_key).copied())
					.unwrap_or(0);
				updates.insert(new_key, existing + count);
			}
		}
		for key in keys_to_remove {
			self.co_occurrence.remove(&key);
		}
		for (key, count) in updates {
			self.co_occurrence.insert(key, count);
		}
	}

	/// Get direct child topic paths (not grandchildren).
	pub fn get_children(&self, topic: &str) -> Vec<String> {
		let normalized = topic.to_lowercase();
		self.topic_to_children
			.get(&normalized)
			.map(|c| c.iter().cloned().collect())
			.unwrap_or_default()
	}

	/// Remove all entries and topics from the index.
	pub fn clear(&mut self) {
		self.topic_to_entries.clear();
		self.entry_to_topics.clear();
		self.topic_to_children.clear();
		self.co_occurrence.clear();
	}

	/// Number of distinct topics tracked.
	pub fn topic_count(&self) -> usize {
		self.topic_to_entries.len()
	}
}

impl Default for TopicIndex {
	fn default() -> Self {
		Self::new(5, &[])
	}
}

// ---------------------------------------------------------------------------
// MetadataIndex
// ---------------------------------------------------------------------------

/// O(1) lookup by (key, value) -> Set of entry IDs.
pub struct MetadataIndex {
	/// "key\0value" -> entry IDs
	kv_index: HashMap<String, HashSet<String>>,
	/// "key" -> entry IDs
	key_index: HashMap<String, HashSet<String>>,
}

impl MetadataIndex {
	pub fn new() -> Self {
		Self {
			kv_index: HashMap::new(),
			key_index: HashMap::new(),
		}
	}

	/// Create a composite key for key-value indexing.
	fn kv_key(key: &str, value: &str) -> String {
		format!("{}\0{}", key, value)
	}

	/// Add an entry's metadata to the index.
	pub fn add_entry(&mut self, id: &str, metadata: &HashMap<String, String>) {
		for (key, value) in metadata {
			// Key-value index
			let composite = Self::kv_key(key, value);
			self.kv_index
				.entry(composite)
				.or_default()
				.insert(id.to_string());

			// Key-only index
			self.key_index
				.entry(key.clone())
				.or_default()
				.insert(id.to_string());
		}
	}

	/// Remove an entry from the index.
	pub fn remove_entry(&mut self, id: &str, metadata: &HashMap<String, String>) {
		for (key, value) in metadata {
			let composite = Self::kv_key(key, value);
			if let Some(kv_set) = self.kv_index.get_mut(&composite) {
				kv_set.remove(id);
				if kv_set.is_empty() {
					self.kv_index.remove(&composite);
				}
			}

			if let Some(key_set) = self.key_index.get_mut(key) {
				key_set.remove(id);
				if key_set.is_empty() {
					self.key_index.remove(key);
				}
			}
		}
	}

	/// Get entry IDs matching an exact key-value pair.
	pub fn get_entries(&self, key: &str, value: &str) -> HashSet<String> {
		self.kv_index
			.get(&Self::kv_key(key, value))
			.cloned()
			.unwrap_or_default()
	}

	/// Get entry IDs that have a specific metadata key.
	pub fn get_entries_with_key(&self, key: &str) -> HashSet<String> {
		self.key_index.get(key).cloned().unwrap_or_default()
	}

	/// Remove all entries from the index.
	pub fn clear(&mut self) {
		self.kv_index.clear();
		self.key_index.clear();
	}
}

impl Default for MetadataIndex {
	fn default() -> Self {
		Self::new()
	}
}

// ---------------------------------------------------------------------------
// MagnitudeCache
// ---------------------------------------------------------------------------

/// Simple cache for computed L2 magnitudes.
pub struct MagnitudeCache {
	cache: HashMap<String, f64>,
}

impl MagnitudeCache {
	pub fn new() -> Self {
		Self {
			cache: HashMap::new(),
		}
	}

	/// Get the cached magnitude for an entry.
	pub fn get(&self, id: &str) -> Option<f64> {
		self.cache.get(id).copied()
	}

	/// Compute and cache the magnitude for an entry's embedding.
	pub fn set(&mut self, id: &str, embedding: &[f32]) {
		let magnitude = compute_magnitude(embedding);
		self.cache.insert(id.to_string(), magnitude);
	}

	/// Remove a cached magnitude.
	pub fn remove(&mut self, id: &str) {
		self.cache.remove(id);
	}

	/// Clear all cached magnitudes.
	pub fn clear(&mut self) {
		self.cache.clear();
	}
}

impl Default for MagnitudeCache {
	fn default() -> Self {
		Self::new()
	}
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;

	// -----------------------------------------------------------------------
	// TopicIndex tests
	// -----------------------------------------------------------------------

	#[test]
	fn topic_from_metadata_topic() {
		let mut index = TopicIndex::new(5, &[]);
		let mut metadata = HashMap::new();
		metadata.insert("topic".to_string(), "rust".to_string());
		index.add_entry("e1", "some text here", &metadata);

		let topics = index.get_topics("e1");
		assert_eq!(topics, vec!["rust"]);
	}

	#[test]
	fn topic_from_metadata_topics_json() {
		let mut index = TopicIndex::new(5, &[]);
		let mut metadata = HashMap::new();
		metadata.insert("topics".to_string(), r#"["alpha","beta"]"#.to_string());
		index.add_entry("e1", "some text here", &metadata);

		let mut topics = index.get_topics("e1");
		topics.sort();
		assert_eq!(topics, vec!["alpha", "beta"]);
	}

	#[test]
	fn topic_auto_extract() {
		let mut index = TopicIndex::new(3, &[]);
		let metadata = HashMap::new();
		// "rust" and "programming" appear twice, "language" once
		let text = "rust programming rust programming language";
		index.add_entry("e1", text, &metadata);

		let topics = index.get_topics("e1");
		// Should auto-extract top 3 frequent words (> 2 chars, not stop words)
		assert!(!topics.is_empty());
		assert!(topics.len() <= 3);
		// "rust" and "programming" should be extracted (they appear most)
		assert!(topics.contains(&"rust".to_string()));
		assert!(topics.contains(&"programming".to_string()));
	}

	#[test]
	fn topic_hierarchy() {
		let mut index = TopicIndex::new(5, &[]);
		let mut metadata = HashMap::new();
		metadata.insert("topic".to_string(), "code/rust".to_string());
		index.add_entry("e1", "some text", &metadata);

		// Should create both "code/rust" and parent "code"
		let all = index.get_all_topics();
		let topic_names: Vec<String> = all.iter().map(|t| t.topic.clone()).collect();
		assert!(topic_names.contains(&"code/rust".to_string()));
		assert!(topic_names.contains(&"code".to_string()));

		// Parent "code" should have "code/rust" as child
		let children = index.get_children("code");
		assert!(children.contains(&"code/rust".to_string()));
	}

	#[test]
	fn topic_get_entries_includes_descendants() {
		let mut index = TopicIndex::new(5, &[]);

		let mut meta1 = HashMap::new();
		meta1.insert("topic".to_string(), "code/rust".to_string());
		index.add_entry("e1", "text", &meta1);

		let mut meta2 = HashMap::new();
		meta2.insert("topic".to_string(), "code".to_string());
		index.add_entry("e2", "text", &meta2);

		// Querying "code" should return both e1 (descendant) and e2 (direct)
		let mut entries = index.get_entries("code");
		entries.sort();
		assert_eq!(entries, vec!["e1", "e2"]);

		// Querying "code/rust" should return only e1
		let entries_rust = index.get_entries("code/rust");
		assert_eq!(entries_rust, vec!["e1"]);
	}

	#[test]
	fn topic_get_children() {
		let mut index = TopicIndex::new(5, &[]);

		let mut meta1 = HashMap::new();
		meta1.insert("topic".to_string(), "code/rust".to_string());
		index.add_entry("e1", "text", &meta1);

		let mut meta2 = HashMap::new();
		meta2.insert("topic".to_string(), "code/python".to_string());
		index.add_entry("e2", "text", &meta2);

		let mut children = index.get_children("code");
		children.sort();
		assert_eq!(children, vec!["code/python", "code/rust"]);
	}

	#[test]
	fn topic_remove_cleanup() {
		let mut index = TopicIndex::new(5, &[]);

		let mut metadata = HashMap::new();
		metadata.insert("topic".to_string(), "code/rust".to_string());
		index.add_entry("e1", "text", &metadata);

		// Before removal, topics exist
		assert!(index.topic_count() >= 2); // "code" and "code/rust"

		// Remove the only entry
		index.remove_entry("e1");

		// After removal, empty topics should be cleaned up
		assert_eq!(index.topic_count(), 0);
		assert!(index.get_all_topics().is_empty());
	}

	#[test]
	fn topic_merge() {
		let mut index = TopicIndex::new(5, &[]);

		let mut meta1 = HashMap::new();
		meta1.insert("topic".to_string(), "old_topic".to_string());
		index.add_entry("e1", "text", &meta1);

		let mut meta2 = HashMap::new();
		meta2.insert("topic".to_string(), "old_topic".to_string());
		index.add_entry("e2", "text", &meta2);

		// Merge old_topic into new_topic
		index.merge_topic("old_topic", "new_topic");

		// Entries should now be under new_topic
		let mut entries = index.get_entries("new_topic");
		entries.sort();
		assert_eq!(entries, vec!["e1", "e2"]);

		// old_topic should be cleaned up
		assert!(index.get_entries("old_topic").is_empty());

		// Entry-to-topics mapping should reflect the merge
		assert_eq!(index.get_topics("e1"), vec!["new_topic"]);
		assert_eq!(index.get_topics("e2"), vec!["new_topic"]);
	}

	#[test]
	fn topic_co_occurrence() {
		let mut index = TopicIndex::new(5, &[]);

		// Entry with two topics -> co-occurrence
		let mut metadata = HashMap::new();
		metadata.insert("topics".to_string(), r#"["alpha","beta"]"#.to_string());
		index.add_entry("e1", "text", &metadata);

		let related = index.get_related_topics("alpha");
		assert_eq!(related.len(), 1);
		assert_eq!(related[0].0, "beta");
		assert_eq!(related[0].1, 1);

		// Add another entry with same pair -> count increases
		let mut metadata2 = HashMap::new();
		metadata2.insert("topics".to_string(), r#"["alpha","beta"]"#.to_string());
		index.add_entry("e2", "text", &metadata2);

		let related = index.get_related_topics("alpha");
		assert_eq!(related.len(), 1);
		assert_eq!(related[0].0, "beta");
		assert_eq!(related[0].1, 2);
	}

	#[test]
	fn topic_clear() {
		let mut index = TopicIndex::new(5, &[]);

		let mut metadata = HashMap::new();
		metadata.insert("topic".to_string(), "rust".to_string());
		index.add_entry("e1", "text", &metadata);

		assert!(index.topic_count() > 0);

		index.clear();

		assert_eq!(index.topic_count(), 0);
		assert!(index.get_all_topics().is_empty());
		assert!(index.get_topics("e1").is_empty());
	}

	#[test]
	fn topic_extra_stop_words() {
		let mut index = TopicIndex::new(5, &["rust", "code"]);
		let metadata = HashMap::new();
		let text = "rust code programming language design";
		index.add_entry("e1", text, &metadata);

		let topics = index.get_topics("e1");
		// "rust" and "code" should be filtered out as stop words
		assert!(!topics.contains(&"rust".to_string()));
		assert!(!topics.contains(&"code".to_string()));
	}

	#[test]
	fn topic_reindex_entry() {
		let mut index = TopicIndex::new(5, &[]);

		let mut meta1 = HashMap::new();
		meta1.insert("topic".to_string(), "old".to_string());
		index.add_entry("e1", "text", &meta1);
		assert_eq!(index.get_topics("e1"), vec!["old"]);

		// Re-index with new topic
		let mut meta2 = HashMap::new();
		meta2.insert("topic".to_string(), "new".to_string());
		index.add_entry("e1", "text", &meta2);
		assert_eq!(index.get_topics("e1"), vec!["new"]);

		// Old topic should be cleaned up
		assert!(index.get_entries("old").is_empty());
	}

	#[test]
	fn topic_comma_separated() {
		let mut index = TopicIndex::new(5, &[]);
		let mut metadata = HashMap::new();
		metadata.insert("topic".to_string(), "rust, python, java".to_string());
		index.add_entry("e1", "text", &metadata);

		let mut topics = index.get_topics("e1");
		topics.sort();
		assert_eq!(topics, vec!["java", "python", "rust"]);
	}

	// -----------------------------------------------------------------------
	// MetadataIndex tests
	// -----------------------------------------------------------------------

	#[test]
	fn metadata_index_basic() {
		let mut index = MetadataIndex::new();
		let mut metadata = HashMap::new();
		metadata.insert("lang".to_string(), "rust".to_string());
		metadata.insert("type".to_string(), "article".to_string());

		index.add_entry("e1", &metadata);

		let result = index.get_entries("lang", "rust");
		assert!(result.contains("e1"));
		assert_eq!(result.len(), 1);

		let result2 = index.get_entries("type", "article");
		assert!(result2.contains("e1"));
	}

	#[test]
	fn metadata_index_key_only() {
		let mut index = MetadataIndex::new();
		let mut meta1 = HashMap::new();
		meta1.insert("lang".to_string(), "rust".to_string());
		index.add_entry("e1", &meta1);

		let mut meta2 = HashMap::new();
		meta2.insert("lang".to_string(), "python".to_string());
		index.add_entry("e2", &meta2);

		let result = index.get_entries_with_key("lang");
		assert!(result.contains("e1"));
		assert!(result.contains("e2"));
		assert_eq!(result.len(), 2);
	}

	#[test]
	fn metadata_index_remove() {
		let mut index = MetadataIndex::new();
		let mut metadata = HashMap::new();
		metadata.insert("lang".to_string(), "rust".to_string());
		index.add_entry("e1", &metadata);

		assert_eq!(index.get_entries("lang", "rust").len(), 1);

		index.remove_entry("e1", &metadata);

		assert!(index.get_entries("lang", "rust").is_empty());
		assert!(index.get_entries_with_key("lang").is_empty());
	}

	#[test]
	fn metadata_index_clear() {
		let mut index = MetadataIndex::new();
		let mut metadata = HashMap::new();
		metadata.insert("lang".to_string(), "rust".to_string());
		index.add_entry("e1", &metadata);

		assert!(!index.get_entries("lang", "rust").is_empty());

		index.clear();

		assert!(index.get_entries("lang", "rust").is_empty());
		assert!(index.get_entries_with_key("lang").is_empty());
	}

	#[test]
	fn metadata_index_multiple_entries_same_key_value() {
		let mut index = MetadataIndex::new();
		let mut metadata = HashMap::new();
		metadata.insert("lang".to_string(), "rust".to_string());

		index.add_entry("e1", &metadata);
		index.add_entry("e2", &metadata);

		let result = index.get_entries("lang", "rust");
		assert!(result.contains("e1"));
		assert!(result.contains("e2"));
		assert_eq!(result.len(), 2);
	}

	// -----------------------------------------------------------------------
	// MagnitudeCache tests
	// -----------------------------------------------------------------------

	#[test]
	fn magnitude_cache_basic() {
		let mut cache = MagnitudeCache::new();
		let embedding = vec![3.0f32, 4.0];
		cache.set("e1", &embedding);

		let mag = cache.get("e1").unwrap();
		assert!((mag - 5.0).abs() < 1e-10);
	}

	#[test]
	fn magnitude_cache_missing() {
		let cache = MagnitudeCache::new();
		assert!(cache.get("nonexistent").is_none());
	}

	#[test]
	fn magnitude_cache_remove() {
		let mut cache = MagnitudeCache::new();
		cache.set("e1", &[3.0, 4.0]);
		assert!(cache.get("e1").is_some());

		cache.remove("e1");
		assert!(cache.get("e1").is_none());
	}

	#[test]
	fn magnitude_cache_clear() {
		let mut cache = MagnitudeCache::new();
		cache.set("e1", &[3.0, 4.0]);
		cache.set("e2", &[1.0, 0.0]);

		assert!(cache.get("e1").is_some());
		assert!(cache.get("e2").is_some());

		cache.clear();

		assert!(cache.get("e1").is_none());
		assert!(cache.get("e2").is_none());
	}

	#[test]
	fn magnitude_cache_overwrite() {
		let mut cache = MagnitudeCache::new();
		cache.set("e1", &[3.0, 4.0]);
		assert!((cache.get("e1").unwrap() - 5.0).abs() < 1e-10);

		cache.set("e1", &[1.0, 0.0]);
		assert!((cache.get("e1").unwrap() - 1.0).abs() < 1e-10);
	}

	// -----------------------------------------------------------------------
	// Helper function tests
	// -----------------------------------------------------------------------

	#[test]
	fn test_get_parent_path() {
		assert_eq!(get_parent_path("code/rust"), Some("code".to_string()));
		assert_eq!(
			get_parent_path("code/rust/async"),
			Some("code/rust".to_string())
		);
		assert_eq!(get_parent_path("root"), None);
	}

	#[test]
	fn test_co_occurrence_key_order_independent() {
		let key1 = co_occurrence_key("alpha", "beta");
		let key2 = co_occurrence_key("beta", "alpha");
		assert_eq!(key1, key2);
	}

	#[test]
	fn test_extract_topics_from_text() {
		let stop_words = default_stop_words();
		let topics = extract_topics_from_text(
			"rust is a great programming language and rust is fast",
			&stop_words,
			3,
		);
		// "rust" appears twice, "great", "programming", "language", "fast" once each
		assert_eq!(topics[0], "rust");
		assert!(topics.len() <= 3);
	}

	#[test]
	fn test_extract_topics_filters_short_words() {
		let stop_words = default_stop_words();
		let topics =
			extract_topics_from_text("go is ok but rust programming", &stop_words, 10);
		// "go", "is", "ok" are <= 2 chars or stop words, should be filtered
		assert!(!topics.contains(&"go".to_string()));
		assert!(!topics.contains(&"is".to_string()));
		assert!(!topics.contains(&"ok".to_string()));
	}
}
