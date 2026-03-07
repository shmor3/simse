// ---------------------------------------------------------------------------
// Topic Catalog — hierarchical topic classification with normalization
// ---------------------------------------------------------------------------
//
// Manages a tree of topics with fuzzy matching (Levenshtein), aliases,
// volume tracking, merge, and relocate operations.
// ---------------------------------------------------------------------------

use crate::text_search::levenshtein_similarity;
use crate::types::TopicCatalogSection;

// ---------------------------------------------------------------------------
// TopicCatalog
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct TopicCatalog {
	similarity_threshold: f64,
	/// topic -> Set<volumeId>
	topic_to_volumes: im::HashMap<String, im::HashSet<String>>,
	/// volumeId -> topic
	volume_to_topic: im::HashMap<String, String>,
	/// alias -> canonical topic
	aliases: im::HashMap<String, String>,
	/// topic -> Set<child topic>
	topic_to_children: im::HashMap<String, im::HashSet<String>>,
}

impl TopicCatalog {
	/// Create a new `TopicCatalog`. `similarity_threshold` defaults to 0.85.
	pub fn new(similarity_threshold: f64) -> Self {
		Self {
			similarity_threshold,
			topic_to_volumes: im::HashMap::new(),
			volume_to_topic: im::HashMap::new(),
			aliases: im::HashMap::new(),
			topic_to_children: im::HashMap::new(),
		}
	}

	/// Ensure a topic and all its ancestors exist in the catalog.
	///
	/// For "code/rust/async", creates "code", "code/rust", "code/rust/async"
	/// and registers parent-child relationships.
	fn ensure_topic_exists(mut self, topic: &str) -> Self {
		let normalized = topic.to_lowercase();
		let normalized = normalized.trim();

		if !self.topic_to_volumes.contains_key(normalized) {
			self.topic_to_volumes = self.topic_to_volumes.update(normalized.to_string(), im::HashSet::new());
		}

		// Ensure all ancestors exist
		let parts: Vec<&str> = normalized.split('/').collect();
		for i in 1..parts.len() {
			let parent = parts[..i].join("/");
			let child = parts[..=i].join("/");

			if !self.topic_to_volumes.contains_key(&parent) {
				self.topic_to_volumes = self.topic_to_volumes.update(parent.clone(), im::HashSet::new());
			}

			let children = self.topic_to_children.get(&parent).cloned().unwrap_or_default();
			let children = children.update(child);
			self.topic_to_children = self.topic_to_children.update(parent, children);
		}
		self
	}

	/// Resolve a proposed topic to a canonical name.
	///
	/// Normalization pipeline:
	/// 1. Lowercase + trim
	/// 2. Check aliases
	/// 3. Check exact match
	/// 4. Fuzzy match (Levenshtein >= threshold)
	/// 5. Register as new topic
	pub fn resolve(self, proposed_topic: &str) -> (Self, String) {
		let normalized = proposed_topic.to_lowercase();
		let normalized = normalized.trim().to_string();

		// 1. Check aliases
		if let Some(canonical) = self.aliases.get(&normalized).cloned() {
			return (self, canonical);
		}

		// 2. Check exact match
		if self.topic_to_volumes.contains_key(&normalized) {
			return (self, normalized);
		}

		// 3. Check similarity against existing topics
		let mut best_match: Option<String> = None;
		let mut best_score: f64 = 0.0;
		for existing in self.topic_to_volumes.keys() {
			let score = levenshtein_similarity(&normalized, existing);
			if score >= self.similarity_threshold && score > best_score {
				best_score = score;
				best_match = Some(existing.clone());
			}
		}

		if let Some(matched) = best_match {
			return (self, matched);
		}

		// 4. Register as new topic
		let s = self.ensure_topic_exists(&normalized);
		(s, normalized)
	}

	/// Register a volume under a topic. Resolves the topic first and removes
	/// the volume from any previous topic.
	pub fn register_volume(self, volume_id: &str, topic: &str) -> Self {
		let (mut s, canonical) = self.resolve(topic);

		// Remove from old topic if exists
		if let Some(old_topic) = s.volume_to_topic.get(volume_id).cloned() {
			if let Some(vols) = s.topic_to_volumes.get(&old_topic).cloned() {
				let vols = vols.without(volume_id);
				s.topic_to_volumes = s.topic_to_volumes.update(old_topic, vols);
			}
		}

		if let Some(vols) = s.topic_to_volumes.get(&canonical).cloned() {
			let vols = vols.update(volume_id.to_string());
			s.topic_to_volumes = s.topic_to_volumes.update(canonical.clone(), vols);
		}
		s.volume_to_topic = s.volume_to_topic.update(volume_id.to_string(), canonical);
		s
	}

	/// Remove a volume from whatever topic it belongs to.
	pub fn remove_volume(mut self, volume_id: &str) -> Self {
		if let Some(topic) = self.volume_to_topic.get(volume_id).cloned() {
			self.volume_to_topic = self.volume_to_topic.without(volume_id);
			if let Some(vols) = self.topic_to_volumes.get(&topic).cloned() {
				let vols = vols.without(volume_id);
				self.topic_to_volumes = self.topic_to_volumes.update(topic, vols);
			}
		}
		self
	}

	/// Move a volume from its current topic to a new one.
	pub fn relocate(self, volume_id: &str, new_topic: &str) -> Self {
		let s = self.remove_volume(volume_id);
		s.register_volume(volume_id, new_topic)
	}

	/// Merge all volumes from `source` into `target` and create an alias.
	pub fn merge(self, source: &str, target: &str) -> Self {
		let src_norm = source.to_lowercase();
		let src_norm = src_norm.trim().to_string();
		let (mut s, tgt_norm) = self.resolve(target);

		// Collect volume IDs from source
		let volume_ids: Vec<String> = s
			.topic_to_volumes
			.get(&src_norm)
			.map(|set| set.iter().cloned().collect())
			.unwrap_or_default();

		if volume_ids.is_empty() && !s.topic_to_volumes.contains_key(&src_norm) {
			return s;
		}

		// Ensure target exists
		if !s.topic_to_volumes.contains_key(&tgt_norm) {
			s = s.ensure_topic_exists(&tgt_norm);
		}

		// Move all volumes
		for volume_id in &volume_ids {
			if let Some(vols) = s.topic_to_volumes.get(&tgt_norm).cloned() {
				let vols = vols.update(volume_id.clone());
				s.topic_to_volumes = s.topic_to_volumes.update(tgt_norm.clone(), vols);
			}
			s.volume_to_topic = s.volume_to_topic.update(volume_id.clone(), tgt_norm.clone());
		}

		// Clear source
		if s.topic_to_volumes.contains_key(&src_norm) {
			s.topic_to_volumes = s.topic_to_volumes.update(src_norm.clone(), im::HashSet::new());
		}

		// Add alias
		s.aliases = s.aliases.update(src_norm, tgt_norm);
		s
	}

	/// List all topics with parent/children/volume count.
	pub fn sections(&self) -> Vec<TopicCatalogSection> {
		let mut result = Vec::new();
		for (topic, vols) in &self.topic_to_volumes {
			let parts: Vec<&str> = topic.split('/').collect();
			let parent = if parts.len() > 1 {
				Some(parts[..parts.len() - 1].join("/"))
			} else {
				None
			};
			let children = self
				.topic_to_children
				.get(topic)
				.map(|c| c.iter().cloned().collect::<Vec<_>>())
				.unwrap_or_default();
			result.push(TopicCatalogSection {
				topic: topic.clone(),
				parent,
				children,
				entry_count: vols.len(),
			});
		}
		result
	}

	/// Return the volume IDs for a topic (lowercase + trimmed).
	pub fn volumes(&self, topic: &str) -> Vec<String> {
		let normalized = topic.to_lowercase();
		let normalized = normalized.trim();
		self.topic_to_volumes
			.get(normalized)
			.map(|s| s.iter().cloned().collect())
			.unwrap_or_default()
	}

	/// Add a manual alias from `alias` to `canonical`.
	pub fn add_alias(mut self, alias: &str, canonical: &str) -> Self {
		self.aliases = self.aliases.update(
			alias.to_lowercase().trim().to_string(),
			canonical.to_lowercase().trim().to_string(),
		);
		self
	}

	/// Return the topic a volume belongs to, if any.
	pub fn get_topic_for_volume(&self, volume_id: &str) -> Option<String> {
		self.volume_to_topic.get(volume_id).cloned()
	}
}

impl Default for TopicCatalog {
	fn default() -> Self {
		Self::new(0.85)
	}
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn resolve_creates_new_topic() {
		let catalog = TopicCatalog::default();
		let (catalog, topic) = catalog.resolve("Rust");
		assert_eq!(topic, "rust");
		let sections = catalog.sections();
		assert!(sections.iter().any(|s| s.topic == "rust"));
	}

	#[test]
	fn resolve_returns_exact_match() {
		let catalog = TopicCatalog::default();
		let (catalog, _) = catalog.resolve("rust");
		let (_, topic) = catalog.resolve("Rust");
		assert_eq!(topic, "rust");
	}

	#[test]
	fn resolve_fuzzy_matches() {
		let catalog = TopicCatalog::default();
		let (catalog, _) = catalog.resolve("programming");
		// "programing" (one 'm') should fuzzy-match "programming"
		let (_, topic) = catalog.resolve("programing");
		assert_eq!(topic, "programming");
	}

	#[test]
	fn resolve_does_not_fuzzy_match_below_threshold() {
		let catalog = TopicCatalog::new(0.95);
		let (catalog, _) = catalog.resolve("rust");
		// "ruby" is too different at threshold 0.95
		let (_, topic) = catalog.resolve("ruby");
		assert_ne!(topic, "rust");
		assert_eq!(topic, "ruby");
	}

	#[test]
	fn resolve_checks_aliases() {
		let catalog = TopicCatalog::default();
		let (catalog, _) = catalog.resolve("javascript");
		let catalog = catalog.add_alias("js", "javascript");
		let (_, topic) = catalog.resolve("js");
		assert_eq!(topic, "javascript");
	}

	#[test]
	fn ensure_topic_exists_creates_hierarchy() {
		let catalog = TopicCatalog::default();
		let (catalog, _) = catalog.resolve("code/rust/async");
		let sections = catalog.sections();
		let topics: Vec<&str> = sections.iter().map(|s| s.topic.as_str()).collect();
		assert!(topics.contains(&"code"));
		assert!(topics.contains(&"code/rust"));
		assert!(topics.contains(&"code/rust/async"));

		// Check parent-child relationships
		let code_section = sections.iter().find(|s| s.topic == "code").unwrap();
		assert!(code_section.children.contains(&"code/rust".to_string()));

		let rust_section = sections.iter().find(|s| s.topic == "code/rust").unwrap();
		assert!(rust_section.children.contains(&"code/rust/async".to_string()));
		assert_eq!(rust_section.parent, Some("code".to_string()));

		let async_section = sections
			.iter()
			.find(|s| s.topic == "code/rust/async")
			.unwrap();
		assert_eq!(async_section.parent, Some("code/rust".to_string()));
	}

	#[test]
	fn register_and_get_topic_for_volume() {
		let catalog = TopicCatalog::default();
		let catalog = catalog.register_volume("vol-1", "rust");
		assert_eq!(
			catalog.get_topic_for_volume("vol-1"),
			Some("rust".to_string())
		);
		let vols = catalog.volumes("rust");
		assert!(vols.contains(&"vol-1".to_string()));
	}

	#[test]
	fn register_volume_moves_from_old_topic() {
		let catalog = TopicCatalog::default();
		let catalog = catalog.register_volume("vol-1", "rust");
		let catalog = catalog.register_volume("vol-1", "go");
		assert_eq!(
			catalog.get_topic_for_volume("vol-1"),
			Some("go".to_string())
		);
		assert!(catalog.volumes("rust").is_empty());
		assert!(catalog.volumes("go").contains(&"vol-1".to_string()));
	}

	#[test]
	fn remove_volume() {
		let catalog = TopicCatalog::default();
		let catalog = catalog.register_volume("vol-1", "rust");
		let catalog = catalog.remove_volume("vol-1");
		assert_eq!(catalog.get_topic_for_volume("vol-1"), None);
		assert!(catalog.volumes("rust").is_empty());
	}

	#[test]
	fn relocate_volume() {
		let catalog = TopicCatalog::default();
		let catalog = catalog.register_volume("vol-1", "rust");
		let catalog = catalog.relocate("vol-1", "go");
		assert_eq!(
			catalog.get_topic_for_volume("vol-1"),
			Some("go".to_string())
		);
		assert!(catalog.volumes("rust").is_empty());
		assert!(catalog.volumes("go").contains(&"vol-1".to_string()));
	}

	#[test]
	fn merge_topics() {
		let catalog = TopicCatalog::default();
		let catalog = catalog.register_volume("vol-1", "javascript");
		let catalog = catalog.register_volume("vol-2", "javascript");
		let catalog = catalog.register_volume("vol-3", "typescript");

		let catalog = catalog.merge("javascript", "typescript");

		// All volumes should be under typescript
		let ts_vols = catalog.volumes("typescript");
		assert!(ts_vols.contains(&"vol-1".to_string()));
		assert!(ts_vols.contains(&"vol-2".to_string()));
		assert!(ts_vols.contains(&"vol-3".to_string()));

		// javascript should be empty
		assert!(catalog.volumes("javascript").is_empty());

		// Alias should redirect
		let (_, resolved) = catalog.resolve("javascript");
		assert_eq!(resolved, "typescript");
	}

	#[test]
	fn sections_returns_all_topics() {
		let catalog = TopicCatalog::default();
		let catalog = catalog.register_volume("vol-1", "rust");
		let catalog = catalog.register_volume("vol-2", "go");
		let catalog = catalog.register_volume("vol-3", "go");

		let sections = catalog.sections();
		let rust_section = sections.iter().find(|s| s.topic == "rust").unwrap();
		assert_eq!(rust_section.entry_count, 1);

		let go_section = sections.iter().find(|s| s.topic == "go").unwrap();
		assert_eq!(go_section.entry_count, 2);
	}

	#[test]
	fn volumes_empty_for_unknown_topic() {
		let catalog = TopicCatalog::default();
		assert!(catalog.volumes("nonexistent").is_empty());
	}

	#[test]
	fn remove_nonexistent_volume_is_noop() {
		let catalog = TopicCatalog::default();
		// Should not panic
		let _catalog = catalog.remove_volume("nonexistent");
	}

	#[test]
	fn normalize_trims_whitespace() {
		let catalog = TopicCatalog::default();
		let catalog = catalog.register_volume("vol-1", "  Rust  ");
		assert_eq!(
			catalog.get_topic_for_volume("vol-1"),
			Some("rust".to_string())
		);
		let vols = catalog.volumes("  Rust  ");
		assert!(vols.contains(&"vol-1".to_string()));
	}
}
