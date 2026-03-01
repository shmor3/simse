// ---------------------------------------------------------------------------
// VolumeStore — core state manager
// ---------------------------------------------------------------------------
//
// Integrates all sub-modules (cosine, text_search, inverted_index, cataloging,
// deduplication, recommendation, learning, topic_catalog, persistence,
// text_cache) into a single stateful struct with full CRUD, search,
// recommendation, and persistence capabilities.
//
// Ports stacks.ts (914 lines), stacks-search.ts (490 lines), and
// stacks-recommend.ts (203 lines) from TypeScript into Rust.
// ---------------------------------------------------------------------------

use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use uuid::Uuid;

use crate::cataloging::{MagnitudeCache, MetadataIndex, TopicIndex};
use crate::cosine::compute_magnitude;
use crate::deduplication;
use crate::error::VectorError;
use crate::graph::{GraphConfig, GraphIndex};
use crate::inverted_index::InvertedIndex;
use crate::learning::{LearningEngine, LearningOptions};
use crate::persistence::{self, AccessStats};
use crate::recommendation::{
	compute_recommendation_score, frequency_score, normalize_weights, recency_score,
	RecommendationScoreInput,
};
use crate::text_cache::TextCache;
use crate::text_search::{self, matches_all_metadata_filters};
use crate::topic_catalog::TopicCatalog;
use crate::types::{
	AdvancedLookup, DateRange, DuplicateCheckResult, DuplicateVolumes, Lookup,
	MetadataFilter, Recommendation, RecommendOptions, RecommendationScores, ScoreBreakdown,
	SearchOptions, TextLookup, TextSearchOptions, TopicCatalogSection, TopicInfo, Volume,
};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Controls how duplicate volumes are handled during add.
#[derive(Debug, Clone, PartialEq)]
pub enum DuplicateBehavior {
	/// Silently skip the duplicate; return the existing volume's ID.
	Skip,
	/// Log a warning and skip (returns existing ID, dirty bit unchanged).
	Warn,
	/// Return a `VectorError::Duplicate` error.
	Error,
}

impl Default for DuplicateBehavior {
	fn default() -> Self {
		Self::Skip
	}
}

/// Configuration for a `VolumeStore`.
pub struct StoreConfig {
	pub storage_path: Option<String>,
	pub duplicate_threshold: f64,
	pub duplicate_behavior: DuplicateBehavior,
	pub max_regex_pattern_length: usize,
	pub learning_enabled: bool,
	pub learning_options: LearningOptions,
	pub recency_half_life_ms: f64,
	pub topic_catalog_threshold: f64,
	pub graph_config: GraphConfig,
}

impl Default for StoreConfig {
	fn default() -> Self {
		Self {
			storage_path: None,
			duplicate_threshold: 0.95,
			duplicate_behavior: DuplicateBehavior::Skip,
			max_regex_pattern_length: 500,
			learning_enabled: false,
			learning_options: LearningOptions::default(),
			recency_half_life_ms: 30.0 * 24.0 * 60.0 * 60.0 * 1000.0,
			topic_catalog_threshold: 0.85,
			graph_config: GraphConfig::default(),
		}
	}
}

// ---------------------------------------------------------------------------
// AddEntry — batch add input
// ---------------------------------------------------------------------------

/// Input for `add_batch`.
pub struct AddEntry {
	pub text: String,
	pub embedding: Vec<f32>,
	pub metadata: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// VolumeStore
// ---------------------------------------------------------------------------

/// Central stateful store for volumes (vector entries).
pub struct VolumeStore {
	volumes: Vec<Volume>,
	topic_index: TopicIndex,
	metadata_index: MetadataIndex,
	magnitude_cache: MagnitudeCache,
	inverted_index: InvertedIndex,
	topic_catalog: TopicCatalog,
	graph_index: GraphIndex,
	learning_engine: Option<LearningEngine>,
	text_cache: Option<TextCache>,
	access_stats: HashMap<String, AccessStats>,
	config: StoreConfig,
	initialized: bool,
	dirty: bool,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn current_timestamp_ms() -> u64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.unwrap_or_default()
		.as_millis() as u64
}

// ---------------------------------------------------------------------------
// Macro for ensuring initialization
// ---------------------------------------------------------------------------

macro_rules! ensure_initialized {
	($self:expr) => {
		if !$self.initialized {
			return Err(VectorError::NotInitialized);
		}
	};
}

// ---------------------------------------------------------------------------
// Implementation
// ---------------------------------------------------------------------------

impl VolumeStore {
	// -- Lifecycle -----------------------------------------------------------

	/// Create a new `VolumeStore` with default empty state. Not yet initialized.
	pub fn new(config: StoreConfig) -> Self {
		let learning_engine = if config.learning_enabled {
			Some(LearningEngine::new(LearningOptions {
				enabled: config.learning_options.enabled,
				max_query_history: config.learning_options.max_query_history,
				query_decay_ms: config.learning_options.query_decay_ms,
				weight_adaptation_rate: config.learning_options.weight_adaptation_rate,
				interest_boost_weight: config.learning_options.interest_boost_weight,
			}))
		} else {
			None
		};

		let graph_index = GraphIndex::new(config.graph_config.clone());

		Self {
			volumes: Vec::new(),
			topic_index: TopicIndex::new(5, &[]),
			metadata_index: MetadataIndex::new(),
			magnitude_cache: MagnitudeCache::new(),
			inverted_index: InvertedIndex::new(),
			topic_catalog: TopicCatalog::new(config.topic_catalog_threshold),
			graph_index,
			learning_engine,
			text_cache: Some(TextCache::default()),
			access_stats: HashMap::new(),
			config,
			initialized: false,
			dirty: false,
		}
	}

	/// Initialize the store. If a storage path is provided (or was set in
	/// config), load persisted data from disk and rebuild all indexes.
	pub fn initialize(&mut self, storage_path: Option<&str>) -> Result<(), VectorError> {
		// Use the provided path, or fall back to config
		let effective_path = storage_path
			.map(|s| s.to_string())
			.or_else(|| self.config.storage_path.clone());

		if let Some(ref path) = effective_path {
			self.config.storage_path = Some(path.clone());

			let data = persistence::load_from_directory(path).map_err(|e| match e {
				persistence::PersistenceError::Io(io) => VectorError::Io(io),
				persistence::PersistenceError::Corruption(msg) => VectorError::Corruption(msg),
				persistence::PersistenceError::Serialization(msg) => {
					VectorError::Serialization(msg)
				}
			})?;

			self.volumes = data.entries;
			self.access_stats = data.access_stats;

			// Restore learning state if present
			if let (Some(engine), Some(state)) =
				(self.learning_engine.as_mut(), data.learning_state.as_ref())
			{
				engine.restore(state);
			}

			// Restore graph state: explicit edges from persistence,
			// then rebuild implicit similarity edges
			if let Some(gs) = data.graph_state {
				self.graph_index =
					GraphIndex::from_state(gs, self.config.graph_config.clone());
			}
		}

		// Rebuild all indexes from current volumes
		self.rebuild_indexes();

		// Rebuild implicit similarity edges for the graph
		for i in 0..self.volumes.len() {
			for j in (i + 1)..self.volumes.len() {
				let sim = crate::cosine::cosine_similarity(
					&self.volumes[i].embedding,
					&self.volumes[j].embedding,
				);
				let ts = self.volumes[i].timestamp.max(self.volumes[j].timestamp);
				self.graph_index.add_similarity_edge(
					&self.volumes[i].id,
					&self.volumes[j].id,
					sim,
					ts,
				);
			}
		}

		self.initialized = true;
		self.dirty = false;

		Ok(())
	}

	/// Save if dirty, then clean up resources.
	pub fn dispose(&mut self) -> Result<(), VectorError> {
		if self.dirty {
			self.save()?;
		}
		self.text_cache = None;
		Ok(())
	}

	/// Serialize and write all data to disk.
	pub fn save(&mut self) -> Result<(), VectorError> {
		let path = match &self.config.storage_path {
			Some(p) => p.clone(),
			None => return Ok(()), // no path, nothing to save
		};

		let learning_state = self.learning_engine.as_ref().map(|e| e.serialize());
		let graph_state = self.graph_index.serialize();

		persistence::save_to_directory(
			&path,
			&self.volumes,
			&self.access_stats,
			learning_state.as_ref(),
			Some(&graph_state),
		)
		.map_err(|e| match e {
			persistence::PersistenceError::Io(io) => VectorError::Io(io),
			persistence::PersistenceError::Corruption(msg) => VectorError::Corruption(msg),
			persistence::PersistenceError::Serialization(msg) => {
				VectorError::Serialization(msg)
			}
		})?;

		self.dirty = false;
		Ok(())
	}

	// -- Index management ----------------------------------------------------

	fn index_volume(&mut self, vol: &Volume) {
		self.topic_index
			.add_entry(&vol.id, &vol.text, &vol.metadata);
		self.metadata_index.add_entry(&vol.id, &vol.metadata);
		self.magnitude_cache.set(&vol.id, &vol.embedding);
		self.inverted_index.add_entry(&vol.id, &vol.text);

		// Register in topic catalog if the volume has a topic in metadata
		if let Some(topic) = vol.metadata.get("topic") {
			self.topic_catalog.register_volume(&vol.id, topic);
		}
	}

	fn deindex_volume(&mut self, vol: &Volume) {
		self.topic_index.remove_entry(&vol.id);
		self.metadata_index.remove_entry(&vol.id, &vol.metadata);
		self.magnitude_cache.remove(&vol.id);
		self.inverted_index.remove_entry(&vol.id, &vol.text);
		self.topic_catalog.remove_volume(&vol.id);

		// Remove from text cache
		if let Some(cache) = self.text_cache.as_mut() {
			cache.remove(&vol.id);
		}
	}

	fn rebuild_indexes(&mut self) {
		self.topic_index.clear();
		self.metadata_index.clear();
		self.magnitude_cache.clear();
		self.inverted_index.clear();

		for vol in &self.volumes {
			self.topic_index
				.add_entry(&vol.id, &vol.text, &vol.metadata);
			self.metadata_index.add_entry(&vol.id, &vol.metadata);
			self.magnitude_cache.set(&vol.id, &vol.embedding);
			self.inverted_index.add_entry(&vol.id, &vol.text);

			if let Some(topic) = vol.metadata.get("topic") {
				self.topic_catalog.register_volume(&vol.id, topic);
			}
		}
	}

	// -- Access tracking -----------------------------------------------------

	fn track_access(&mut self, id: &str) {
		let now = current_timestamp_ms();
		let stats = self
			.access_stats
			.entry(id.to_string())
			.or_insert(AccessStats {
				access_count: 0,
				last_accessed: now,
			});
		stats.access_count += 1;
		stats.last_accessed = now;
	}

	// -- Fast cosine ---------------------------------------------------------

	fn fast_cosine(
		&self,
		query_embedding: &[f32],
		query_mag: f64,
		vol: &Volume,
	) -> Option<f64> {
		if vol.embedding.len() != query_embedding.len() {
			return None;
		}
		let entry_mag = self
			.magnitude_cache
			.get(&vol.id)
			.unwrap_or_else(|| compute_magnitude(&vol.embedding));
		if entry_mag == 0.0 {
			return None;
		}
		let mut dot: f64 = 0.0;
		for i in 0..query_embedding.len() {
			dot += query_embedding[i] as f64 * vol.embedding[i] as f64;
		}
		let raw = dot / (query_mag * entry_mag);
		if raw.is_finite() {
			Some(raw.clamp(-1.0, 1.0))
		} else {
			None
		}
	}

	// -- CRUD ----------------------------------------------------------------

	/// Add a single volume. Returns the generated UUID.
	pub fn add(
		&mut self,
		text: String,
		embedding: Vec<f32>,
		metadata: HashMap<String, String>,
	) -> Result<String, VectorError> {
		ensure_initialized!(self);

		if text.is_empty() {
			return Err(VectorError::EmptyText);
		}
		if embedding.is_empty() {
			return Err(VectorError::EmptyEmbedding);
		}

		// Check for duplicates
		if self.config.duplicate_threshold < 1.0 {
			let dup_result = deduplication::check_duplicate(
				&embedding,
				&self.volumes,
				self.config.duplicate_threshold,
			);
			if dup_result.is_duplicate {
				match self.config.duplicate_behavior {
					DuplicateBehavior::Skip | DuplicateBehavior::Warn => {
						if let Some(ref existing) = dup_result.existing_volume {
							return Ok(existing.id.clone());
						}
					}
					DuplicateBehavior::Error => {
						return Err(VectorError::Duplicate(
							dup_result.similarity.unwrap_or(1.0),
						));
					}
				}
			}
		}

		let id = Uuid::new_v4().to_string();
		let now = current_timestamp_ms();

		let volume = Volume {
			id: id.clone(),
			text,
			embedding,
			metadata,
			timestamp: now,
		};

		self.index_volume(&volume);
		self.volumes.push(volume);

		// Wire into graph index: parse explicit edges from rel:* metadata
		// and create implicit similarity edges with existing volumes.
		// We need to borrow the newly pushed volume immutably, so use index.
		let new_idx = self.volumes.len() - 1;
		self.graph_index.parse_metadata_edges(
			&self.volumes[new_idx].id,
			&self.volumes[new_idx].metadata,
			self.volumes[new_idx].timestamp,
		);

		let new_mag = compute_magnitude(&self.volumes[new_idx].embedding);
		for i in 0..new_idx {
			let existing_mag = self
				.magnitude_cache
				.get(&self.volumes[i].id)
				.unwrap_or_else(|| compute_magnitude(&self.volumes[i].embedding));
			let sim = crate::cosine::cosine_similarity_with_magnitude(
				&self.volumes[new_idx].embedding,
				&self.volumes[i].embedding,
				new_mag,
				existing_mag,
			);
			self.graph_index.add_similarity_edge(
				&self.volumes[new_idx].id,
				&self.volumes[i].id,
				sim,
				self.volumes[new_idx].timestamp,
			);
		}

		self.dirty = true;

		Ok(id)
	}

	/// Batch add multiple entries. Returns a list of generated UUIDs.
	pub fn add_batch(
		&mut self,
		entries: Vec<AddEntry>,
	) -> Result<Vec<String>, VectorError> {
		ensure_initialized!(self);

		let mut ids = Vec::with_capacity(entries.len());
		for entry in entries {
			let id = self.add(entry.text, entry.embedding, entry.metadata)?;
			ids.push(id);
		}
		Ok(ids)
	}

	/// Delete a volume by ID. Returns true if found and removed.
	pub fn delete(&mut self, id: &str) -> bool {
		if !self.initialized {
			return false;
		}

		if let Some(pos) = self.volumes.iter().position(|v| v.id == id) {
			let vol = self.volumes.remove(pos);
			self.deindex_volume(&vol);
			self.access_stats.remove(id);
			self.graph_index.remove_node(id);
			self.dirty = true;

			// Prune learning engine
			if let Some(engine) = self.learning_engine.as_mut() {
				let valid_ids: HashSet<String> =
					self.volumes.iter().map(|v| v.id.clone()).collect();
				engine.prune_entries(&valid_ids);
			}

			true
		} else {
			false
		}
	}

	/// Delete multiple volumes by ID. Returns the count of actually removed volumes.
	pub fn delete_batch(&mut self, ids: &[String]) -> usize {
		if !self.initialized {
			return 0;
		}

		let mut count = 0;
		for id in ids {
			if self.delete(id) {
				count += 1;
			}
		}
		count
	}

	/// Remove all volumes and reset all indexes.
	pub fn clear(&mut self) {
		self.volumes.clear();
		self.topic_index.clear();
		self.metadata_index.clear();
		self.magnitude_cache.clear();
		self.inverted_index.clear();
		self.access_stats.clear();
		self.graph_index = GraphIndex::new(self.config.graph_config.clone());

		if let Some(cache) = self.text_cache.as_mut() {
			cache.clear();
		}
		if let Some(engine) = self.learning_engine.as_mut() {
			engine.clear();
		}

		self.dirty = true;
	}

	// -- Search --------------------------------------------------------------

	/// Vector similarity search using cosine similarity with magnitude cache.
	pub fn search(
		&mut self,
		query_embedding: &[f32],
		max_results: usize,
		threshold: f64,
	) -> Result<Vec<Lookup>, VectorError> {
		ensure_initialized!(self);

		let query_mag = compute_magnitude(query_embedding);
		if query_mag == 0.0 {
			return Ok(Vec::new());
		}

		let mut results: Vec<Lookup> = Vec::new();

		for vol in &self.volumes {
			if let Some(score) = self.fast_cosine(query_embedding, query_mag, vol) {
				if score >= threshold {
					results.push(Lookup {
						volume: vol.clone(),
						score,
					});
				}
			}
		}

		results.sort_by(|a, b| {
			b.score
				.partial_cmp(&a.score)
				.unwrap_or(std::cmp::Ordering::Equal)
		});
		results.truncate(max_results);

		// Track access and record query for learning
		let selected_ids: Vec<String> =
			results.iter().map(|r| r.volume.id.clone()).collect();
		for id in &selected_ids {
			self.track_access(id);
		}
		if let Some(engine) = self.learning_engine.as_mut() {
			let now = current_timestamp_ms();
			engine.record_query(query_embedding, &selected_ids, None, now);
		}

		Ok(results)
	}

	/// Text search using the specified mode (fuzzy, bm25, exact, substring, regex, token).
	pub fn text_search(
		&self,
		options: &TextSearchOptions,
	) -> Result<Vec<TextLookup>, VectorError> {
		if !self.initialized {
			return Err(VectorError::NotInitialized);
		}

		let mode = options.mode.as_deref().unwrap_or("fuzzy");
		let threshold = options.threshold.unwrap_or(0.3);

		if mode == "bm25" {
			// Use the inverted index for BM25 search
			let bm25_results = self.inverted_index.bm25_search(&options.query, 1.2, 0.75);
			if bm25_results.is_empty() {
				return Ok(Vec::new());
			}

			// Normalize BM25 scores to [0, 1]
			let max_score = bm25_results
				.iter()
				.map(|r| r.score)
				.fold(f64::NEG_INFINITY, f64::max);

			let mut results: Vec<TextLookup> = Vec::new();
			for bm25_result in &bm25_results {
				let normalized = if max_score > 0.0 {
					bm25_result.score / max_score
				} else {
					0.0
				};
				if normalized >= threshold {
					if let Some(vol) = self.volumes.iter().find(|v| v.id == bm25_result.id)
					{
						results.push(TextLookup {
							volume: vol.clone(),
							score: normalized,
						});
					}
				}
			}
			return Ok(results);
		}

		// Non-BM25 modes: use score_text
		let mut results: Vec<TextLookup> = Vec::new();
		for vol in &self.volumes {
			if let Some(score) =
				text_search::score_text(&options.query, &vol.text, mode, threshold)
			{
				results.push(TextLookup {
					volume: vol.clone(),
					score,
				});
			}
		}

		results.sort_by(|a, b| {
			b.score
				.partial_cmp(&a.score)
				.unwrap_or(std::cmp::Ordering::Equal)
		});

		Ok(results)
	}

	/// Filter volumes by metadata filters.
	pub fn filter_by_metadata(&self, filters: &[MetadataFilter]) -> Vec<Volume> {
		if !self.initialized || filters.is_empty() {
			return Vec::new();
		}

		// Fast path for simple eq filters: use metadata index
		if filters.len() == 1 {
			let f = &filters[0];
			let mode = f.mode.as_deref().unwrap_or("eq");
			if mode == "eq" {
				if let Some(ref val) = f.value {
					if let Some(s) = val.as_str() {
						let ids = self.metadata_index.get_entries(&f.key, s);
						return self
							.volumes
							.iter()
							.filter(|v| ids.contains(&v.id))
							.cloned()
							.collect();
					}
				}
			}
		}

		// Linear scan for complex filters
		self.volumes
			.iter()
			.filter(|v| matches_all_metadata_filters(&v.metadata, filters))
			.cloned()
			.collect()
	}

	/// Filter volumes by timestamp range.
	pub fn filter_by_date_range(&self, range: &DateRange) -> Vec<Volume> {
		if !self.initialized {
			return Vec::new();
		}

		self.volumes
			.iter()
			.filter(|v| {
				if let Some(after) = range.after {
					if v.timestamp < after {
						return false;
					}
				}
				if let Some(before) = range.before {
					if v.timestamp > before {
						return false;
					}
				}
				true
			})
			.cloned()
			.collect()
	}

	/// Advanced search combining vector similarity, text search, metadata
	/// filters, date range, and topic filter with configurable rank modes.
	pub fn advanced_search(
		&mut self,
		options: &SearchOptions,
	) -> Result<Vec<AdvancedLookup>, VectorError> {
		ensure_initialized!(self);

		let max_results = options.max_results.unwrap_or(10);
		let rank_by = options.rank_by.as_deref().unwrap_or("average");
		let sim_threshold = options.similarity_threshold.unwrap_or(0.0);

		// Determine candidate set
		let mut candidate_ids: Option<HashSet<String>> = None;

		// Apply topic filter
		if let Some(ref topics) = options.topic_filter {
			if !topics.is_empty() {
				let mut ids = HashSet::new();
				for topic in topics {
					for id in self.topic_index.get_entries(topic) {
						ids.insert(id);
					}
				}
				candidate_ids = Some(ids);
			}
		}

		// Apply metadata filter
		if let Some(ref filters) = options.metadata {
			if !filters.is_empty() {
				let matching: HashSet<String> = self
					.volumes
					.iter()
					.filter(|v| matches_all_metadata_filters(&v.metadata, filters))
					.map(|v| v.id.clone())
					.collect();
				candidate_ids = Some(match candidate_ids {
					Some(existing) => existing.intersection(&matching).cloned().collect(),
					None => matching,
				});
			}
		}

		// Apply date range filter
		if let Some(ref date_range) = options.date_range {
			let matching: HashSet<String> = self
				.filter_by_date_range(date_range)
				.iter()
				.map(|v| v.id.clone())
				.collect();
			candidate_ids = Some(match candidate_ids {
				Some(existing) => existing.intersection(&matching).cloned().collect(),
				None => matching,
			});
		}

		// Compute vector scores
		let query_mag = options
			.query_embedding
			.as_ref()
			.map(|e| compute_magnitude(e));

		// Compute BM25 or text scores
		let text_scores: HashMap<String, f64> = if let Some(ref text_opts) = options.text {
			let mode = text_opts.mode.as_deref().unwrap_or("fuzzy");
			let threshold = text_opts.threshold.unwrap_or(0.0);

			if mode == "bm25" {
				let bm25_results =
					self.inverted_index.bm25_search(&text_opts.query, 1.2, 0.75);
				let max_score = bm25_results
					.iter()
					.map(|r| r.score)
					.fold(f64::NEG_INFINITY, f64::max);

				bm25_results
					.into_iter()
					.filter_map(|r| {
						let normalized =
							if max_score > 0.0 { r.score / max_score } else { 0.0 };
						if normalized >= threshold {
							Some((r.id, normalized))
						} else {
							None
						}
					})
					.collect()
			} else {
				let mut map = HashMap::new();
				for vol in &self.volumes {
					if let Some(score) = text_search::score_text(
						&text_opts.query,
						&vol.text,
						mode,
						threshold,
					) {
						map.insert(vol.id.clone(), score);
					}
				}
				map
			}
		} else {
			HashMap::new()
		};

		// Field boosts
		let text_boost = options
			.field_boosts
			.as_ref()
			.and_then(|b| b.text)
			.unwrap_or(1.0);
		let metadata_boost = options
			.field_boosts
			.as_ref()
			.and_then(|b| b.metadata)
			.unwrap_or(1.0);
		let _topic_boost = options
			.field_boosts
			.as_ref()
			.and_then(|b| b.topic)
			.unwrap_or(1.0);

		// Score each candidate volume
		let mut results: Vec<AdvancedLookup> = Vec::new();

		for vol in &self.volumes {
			// Filter by candidate set
			if let Some(ref ids) = candidate_ids {
				if !ids.contains(&vol.id) {
					continue;
				}
			}

			// Compute vector score
			let vector_score =
				if let (Some(ref emb), Some(qm)) = (&options.query_embedding, query_mag) {
					if qm > 0.0 {
						self.fast_cosine(emb, qm, vol)
					} else {
						None
					}
				} else {
					None
				};

			// Skip below similarity threshold for vector-only searches
			if let Some(vs) = vector_score {
				if vs < sim_threshold {
					continue;
				}
			}

			// Get text score (apply boost)
			let text_score = text_scores.get(&vol.id).map(|s| s * text_boost);

			// Apply metadata boost (multiply vector score)
			let boosted_vector = vector_score.map(|vs| vs * metadata_boost);

			// Combine scores according to rank_by
			let combined = combine_scores(
				boosted_vector,
				text_score,
				rank_by,
				options.rank_weights.as_ref(),
			);

			// Skip entries with no signal
			if combined.is_none()
				&& vector_score.is_none()
				&& text_score.is_none()
			{
				continue;
			}

			let final_score = combined.unwrap_or(0.0);

			results.push(AdvancedLookup {
				volume: vol.clone(),
				score: final_score,
				scores: ScoreBreakdown {
					vector: vector_score,
					text: text_scores.get(&vol.id).copied(),
				},
			});
		}

		results.sort_by(|a, b| {
			b.score
				.partial_cmp(&a.score)
				.unwrap_or(std::cmp::Ordering::Equal)
		});
		results.truncate(max_results);

		// Track access
		let selected_ids: Vec<String> =
			results.iter().map(|r| r.volume.id.clone()).collect();
		for id in &selected_ids {
			self.track_access(id);
		}

		Ok(results)
	}

	// -- Recommendation ------------------------------------------------------

	/// Compute recommendations using weighted scoring with optional pre-filtering.
	pub fn recommend(
		&self,
		options: &RecommendOptions,
	) -> Result<Vec<Recommendation>, VectorError> {
		if !self.initialized {
			return Err(VectorError::NotInitialized);
		}

		let max_results = options.max_results.unwrap_or(10);
		let min_score = options.min_score.unwrap_or(0.0);

		// Get weights from learning engine or normalize user-provided
		let weights = if let Some(engine) = &self.learning_engine {
			if engine.total_queries() > 0 {
				engine.get_adapted_weights(None)
			} else {
				normalize_weights(&options.weights)
			}
		} else {
			normalize_weights(&options.weights)
		};

		// Pre-filter candidates
		let candidates: Vec<&Volume> = self
			.volumes
			.iter()
			.filter(|vol| {
				// Topic filter
				if let Some(ref topics) = options.topics {
					if !topics.is_empty() {
						let vol_topic = vol.metadata.get("topic");
						let has_matching = topics.iter().any(|t| {
							vol_topic
								.map(|vt| vt.to_lowercase() == t.to_lowercase())
								.unwrap_or(false)
						});
						if !has_matching {
							return false;
						}
					}
				}

				// Metadata filter
				if let Some(ref filters) = options.metadata {
					if !filters.is_empty()
						&& !matches_all_metadata_filters(&vol.metadata, filters)
					{
						return false;
					}
				}

				// Date range filter
				if let Some(ref date_range) = options.date_range {
					if let Some(after) = date_range.after {
						if vol.timestamp < after {
							return false;
						}
					}
					if let Some(before) = date_range.before {
						if vol.timestamp > before {
							return false;
						}
					}
				}

				true
			})
			.collect();

		if candidates.is_empty() {
			return Ok(Vec::new());
		}

		// Find max access count for frequency normalization
		let max_access: usize = self
			.access_stats
			.values()
			.map(|s| s.access_count as usize)
			.max()
			.unwrap_or(0);

		let now = current_timestamp_ms();
		let half_life = self.config.recency_half_life_ms;

		// Compute query magnitude if embedding provided
		let query_mag = options
			.query_embedding
			.as_ref()
			.map(|e| compute_magnitude(e));

		let mut results: Vec<Recommendation> = Vec::new();

		for vol in &candidates {
			// Compute vector score
			let vector_score =
				if let (Some(ref emb), Some(qm)) = (&options.query_embedding, query_mag) {
					if qm > 0.0 {
						self.fast_cosine(emb, qm, vol)
					} else {
						None
					}
				} else {
					None
				};

			// Compute recency score
			let rec_score = recency_score(vol.timestamp, half_life, now);

			// Compute frequency score
			let access_count = self
				.access_stats
				.get(&vol.id)
				.map(|s| s.access_count as usize)
				.unwrap_or(0);
			let freq_score = frequency_score(access_count, max_access);

			// Compute base recommendation score
			let input = RecommendationScoreInput {
				vector_score,
				recency_score: Some(rec_score),
				frequency_score: Some(freq_score),
			};
			let score_result = compute_recommendation_score(&input, &weights);
			let mut final_score = score_result.score;

			// Apply learning boost
			if let Some(engine) = &self.learning_engine {
				let boost = engine.compute_boost(&vol.id, &vol.embedding, None);
				final_score *= boost;
			}

			if final_score >= min_score {
				results.push(Recommendation {
					volume: (*vol).clone(),
					score: final_score,
					scores: RecommendationScores {
						vector: score_result.vector,
						recency: score_result.recency,
						frequency: score_result.frequency,
					},
				});
			}
		}

		results.sort_by(|a, b| {
			b.score
				.partial_cmp(&a.score)
				.unwrap_or(std::cmp::Ordering::Equal)
		});
		results.truncate(max_results);

		Ok(results)
	}

	// -- Accessors -----------------------------------------------------------

	/// Get a volume by ID, tracking access on hit.
	pub fn get_by_id(&mut self, id: &str) -> Option<Volume> {
		if !self.initialized {
			return None;
		}
		if let Some(vol) = self.volumes.iter().find(|v| v.id == id) {
			let vol = vol.clone();
			self.track_access(id);
			Some(vol)
		} else {
			None
		}
	}

	/// Return clones of all volumes.
	pub fn get_all(&self) -> Vec<Volume> {
		if !self.initialized {
			return Vec::new();
		}
		self.volumes.clone()
	}

	/// Get all topics from the topic index.
	pub fn get_topics(&self) -> Vec<TopicInfo> {
		if !self.initialized {
			return Vec::new();
		}
		self.topic_index.get_all_topics()
	}

	/// Filter volumes by topic, returning those whose IDs match.
	pub fn filter_by_topic(&self, topics: &[String]) -> Vec<Volume> {
		if !self.initialized || topics.is_empty() {
			return Vec::new();
		}

		let mut ids = HashSet::new();
		for topic in topics {
			for id in self.topic_index.get_entries(topic) {
				ids.insert(id);
			}
		}

		self.volumes
			.iter()
			.filter(|v| ids.contains(&v.id))
			.cloned()
			.collect()
	}

	/// Find groups of near-duplicate volumes.
	pub fn find_duplicates(&self, threshold: Option<f64>) -> Vec<DuplicateVolumes> {
		if !self.initialized {
			return Vec::new();
		}
		let thresh = threshold.unwrap_or(self.config.duplicate_threshold);
		deduplication::find_duplicate_volumes(&self.volumes, thresh)
	}

	/// Check whether a new embedding would be a duplicate.
	pub fn check_duplicate(&self, embedding: &[f32]) -> DuplicateCheckResult {
		if !self.initialized {
			return DuplicateCheckResult {
				is_duplicate: false,
				existing_volume: None,
				similarity: None,
			};
		}
		deduplication::check_duplicate(embedding, &self.volumes, self.config.duplicate_threshold)
	}

	/// Number of stored volumes.
	pub fn size(&self) -> usize {
		self.volumes.len()
	}

	/// Whether the store has unsaved changes.
	pub fn is_dirty(&self) -> bool {
		self.dirty
	}

	// -- Learning delegation -------------------------------------------------

	/// Record a completed query for learning engine adaptation.
	pub fn record_query(&mut self, embedding: &[f32], selected_ids: &[String]) {
		if let Some(engine) = self.learning_engine.as_mut() {
			let now = current_timestamp_ms();
			engine.record_query(embedding, selected_ids, None, now);
		}
	}

	/// Record explicit user feedback on an entry.
	pub fn record_feedback(&mut self, entry_id: &str, relevant: bool) {
		if let Some(engine) = self.learning_engine.as_mut() {
			let now = current_timestamp_ms();
			engine.record_feedback(entry_id, relevant, now);
		}
	}

	/// Get the current learning profile snapshot.
	pub fn get_profile(&self) -> Option<crate::types::PatronProfile> {
		self.learning_engine.as_ref().map(|e| e.get_profile())
	}

	// -- Topic catalog delegation --------------------------------------------

	/// Resolve a proposed topic to a canonical name.
	pub fn catalog_resolve(&mut self, topic: &str) -> String {
		self.topic_catalog.resolve(topic)
	}

	/// Move a volume to a new topic in the catalog.
	pub fn catalog_relocate(&mut self, volume_id: &str, new_topic: &str) {
		self.topic_catalog.relocate(volume_id, new_topic);
	}

	/// Merge one topic into another in the catalog.
	pub fn catalog_merge(&mut self, source: &str, target: &str) {
		self.topic_catalog.merge(source, target);
	}

	/// List all topic catalog sections.
	pub fn catalog_sections(&self) -> Vec<TopicCatalogSection> {
		self.topic_catalog.sections()
	}

	/// Get volume IDs under a specific topic in the catalog.
	pub fn catalog_volumes(&self, topic: &str) -> Vec<String> {
		self.topic_catalog.volumes(topic)
	}

	// -- Graph delegation ----------------------------------------------------

	/// Get graph neighbors for a volume with optional edge type filter.
	pub fn graph_neighbors(
		&self,
		id: &str,
		edge_types: Option<&[crate::graph::EdgeType]>,
		max_results: usize,
	) -> Vec<(&crate::graph::Edge, Option<&Volume>)> {
		let edges = match edge_types {
			Some(types) => self.graph_index.neighbors_by_type(id, types),
			None => self.graph_index.neighbors(id),
		};
		edges
			.into_iter()
			.take(max_results)
			.map(|edge| {
				let volume = self.volumes.iter().find(|v| v.id == edge.target_id);
				(edge, volume)
			})
			.collect()
	}

	/// BFS traversal from a volume through the graph.
	pub fn graph_traverse(
		&self,
		id: &str,
		depth: usize,
		edge_types: Option<&[crate::graph::EdgeType]>,
		max_results: usize,
	) -> Vec<(crate::graph::TraversalNode, Option<&Volume>)> {
		let nodes = self.graph_index.traverse(id, depth, edge_types, max_results);
		nodes
			.into_iter()
			.map(|node| {
				let volume = self.volumes.iter().find(|v| v.id == node.node_id);
				(node, volume)
			})
			.collect()
	}

	/// Direct access to the underlying graph index.
	pub fn graph_index(&self) -> &GraphIndex {
		&self.graph_index
	}
}

// ---------------------------------------------------------------------------
// Score combining helper
// ---------------------------------------------------------------------------

fn combine_scores(
	vector_score: Option<f64>,
	text_score: Option<f64>,
	rank_by: &str,
	rank_weights: Option<&crate::types::RankWeights>,
) -> Option<f64> {
	match rank_by {
		"vector" => vector_score,
		"text" => text_score,
		"multiply" => {
			let v = vector_score.unwrap_or(0.0);
			let t = text_score.unwrap_or(0.0);
			if vector_score.is_some() || text_score.is_some() {
				Some(v * t)
			} else {
				None
			}
		}
		"weighted" => {
			if let Some(weights) = rank_weights {
				let mut score = 0.0;
				let mut total_weight = 0.0;
				if let (Some(vs), Some(vw)) = (vector_score, weights.vector) {
					score += vs * vw;
					total_weight += vw;
				}
				if let (Some(ts), Some(tw)) = (text_score, weights.text) {
					score += ts * tw;
					total_weight += tw;
				}
				if total_weight > 0.0 {
					Some(score / total_weight)
				} else {
					None
				}
			} else {
				// Fallback to average if no weights provided
				match (vector_score, text_score) {
					(Some(v), Some(t)) => Some((v + t) / 2.0),
					(Some(v), None) => Some(v),
					(None, Some(t)) => Some(t),
					(None, None) => None,
				}
			}
		}
		// default: "average"
		_ => match (vector_score, text_score) {
			(Some(v), Some(t)) => Some((v + t) / 2.0),
			(Some(v), None) => Some(v),
			(None, Some(t)) => Some(t),
			(None, None) => None,
		},
	}
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;

	fn default_config() -> StoreConfig {
		StoreConfig {
			duplicate_threshold: 1.0, // Disable duplicate detection in tests by default
			..Default::default()
		}
	}

	fn learning_config() -> StoreConfig {
		StoreConfig {
			learning_enabled: true,
			learning_options: LearningOptions::default(),
			..Default::default()
		}
	}

	fn init_store(config: StoreConfig) -> VolumeStore {
		let mut store = VolumeStore::new(config);
		store.initialize(None).unwrap();
		store
	}

	fn make_embedding(values: &[f32]) -> Vec<f32> {
		values.to_vec()
	}

	fn make_metadata(pairs: &[(&str, &str)]) -> HashMap<String, String> {
		pairs
			.iter()
			.map(|(k, v)| (k.to_string(), v.to_string()))
			.collect()
	}

	// 1. new + initialize lifecycle
	#[test]
	fn test_new_and_initialize() {
		let mut store = VolumeStore::new(default_config());
		assert!(!store.initialized);
		assert_eq!(store.size(), 0);

		store.initialize(None).unwrap();
		assert!(store.initialized);
		assert_eq!(store.size(), 0);
		assert!(!store.is_dirty());
	}

	// 2. add + get_by_id round-trip
	#[test]
	fn test_add_and_get_by_id() {
		let mut store = init_store(default_config());

		let id = store
			.add(
				"hello world".to_string(),
				make_embedding(&[1.0, 0.0, 0.0]),
				HashMap::new(),
			)
			.unwrap();

		assert!(!id.is_empty());
		assert_eq!(store.size(), 1);
		assert!(store.is_dirty());

		let vol = store.get_by_id(&id).unwrap();
		assert_eq!(vol.text, "hello world");
		assert_eq!(vol.embedding, vec![1.0, 0.0, 0.0]);
	}

	// 3. add with empty text
	#[test]
	fn test_add_empty_text_error() {
		let mut store = init_store(default_config());
		let result = store.add(
			"".to_string(),
			make_embedding(&[1.0, 0.0, 0.0]),
			HashMap::new(),
		);
		assert!(matches!(result, Err(VectorError::EmptyText)));
	}

	// 4. add with empty embedding
	#[test]
	fn test_add_empty_embedding_error() {
		let mut store = init_store(default_config());
		let result = store.add("hello".to_string(), Vec::new(), HashMap::new());
		assert!(matches!(result, Err(VectorError::EmptyEmbedding)));
	}

	// 5. add_batch adds multiple
	#[test]
	fn test_add_batch() {
		let mut store = init_store(default_config());

		let entries = vec![
			AddEntry {
				text: "first".to_string(),
				embedding: make_embedding(&[1.0, 0.0, 0.0]),
				metadata: HashMap::new(),
			},
			AddEntry {
				text: "second".to_string(),
				embedding: make_embedding(&[0.0, 1.0, 0.0]),
				metadata: HashMap::new(),
			},
			AddEntry {
				text: "third".to_string(),
				embedding: make_embedding(&[0.0, 0.0, 1.0]),
				metadata: HashMap::new(),
			},
		];

		let ids = store.add_batch(entries).unwrap();
		assert_eq!(ids.len(), 3);
		assert_eq!(store.size(), 3);
	}

	// 6. delete removes volume + from indexes
	#[test]
	fn test_delete() {
		let mut store = init_store(default_config());

		let id = store
			.add(
				"test entry".to_string(),
				make_embedding(&[1.0, 0.0, 0.0]),
				make_metadata(&[("topic", "rust")]),
			)
			.unwrap();

		assert_eq!(store.size(), 1);
		assert!(store.delete(&id));
		assert_eq!(store.size(), 0);
		assert!(store.get_by_id(&id).is_none());

		// Topics should be cleaned up
		let topics = store.get_topics();
		let rust_entries: Vec<&TopicInfo> =
			topics.iter().filter(|t| t.topic == "rust").collect();
		for t in rust_entries {
			assert_eq!(t.entry_count, 0);
		}
	}

	// 7. delete_batch removes multiple
	#[test]
	fn test_delete_batch() {
		let mut store = init_store(default_config());

		let id1 = store
			.add(
				"first".to_string(),
				make_embedding(&[1.0, 0.0, 0.0]),
				HashMap::new(),
			)
			.unwrap();
		let id2 = store
			.add(
				"second".to_string(),
				make_embedding(&[0.0, 1.0, 0.0]),
				HashMap::new(),
			)
			.unwrap();
		let _id3 = store
			.add(
				"third".to_string(),
				make_embedding(&[0.0, 0.0, 1.0]),
				HashMap::new(),
			)
			.unwrap();

		let removed = store.delete_batch(&[id1, id2]);
		assert_eq!(removed, 2);
		assert_eq!(store.size(), 1);
	}

	// 8. clear empties everything
	#[test]
	fn test_clear() {
		let mut store = init_store(default_config());

		store
			.add(
				"first".to_string(),
				make_embedding(&[1.0, 0.0]),
				HashMap::new(),
			)
			.unwrap();
		store
			.add(
				"second".to_string(),
				make_embedding(&[0.0, 1.0]),
				HashMap::new(),
			)
			.unwrap();

		assert_eq!(store.size(), 2);
		store.clear();
		assert_eq!(store.size(), 0);
		assert!(store.is_dirty());
	}

	// 9. search returns sorted by score
	#[test]
	fn test_search_sorted() {
		let mut store = init_store(default_config());

		store
			.add(
				"close match".to_string(),
				make_embedding(&[0.9, 0.1, 0.0]),
				HashMap::new(),
			)
			.unwrap();
		store
			.add(
				"exact match".to_string(),
				make_embedding(&[1.0, 0.0, 0.0]),
				HashMap::new(),
			)
			.unwrap();
		store
			.add(
				"orthogonal".to_string(),
				make_embedding(&[0.0, 1.0, 0.0]),
				HashMap::new(),
			)
			.unwrap();

		let results = store
			.search(&make_embedding(&[1.0, 0.0, 0.0]), 10, 0.5)
			.unwrap();

		assert!(results.len() >= 2);
		// Should be sorted descending
		for i in 1..results.len() {
			assert!(results[i - 1].score >= results[i].score);
		}
		// First result should be the exact match
		assert_eq!(results[0].volume.text, "exact match");
	}

	// 10. text_search fuzzy mode
	#[test]
	fn test_text_search_fuzzy() {
		let mut store = init_store(default_config());

		store
			.add(
				"hello world".to_string(),
				make_embedding(&[1.0, 0.0]),
				HashMap::new(),
			)
			.unwrap();
		store
			.add(
				"goodbye world".to_string(),
				make_embedding(&[0.0, 1.0]),
				HashMap::new(),
			)
			.unwrap();
		store
			.add(
				"completely different".to_string(),
				make_embedding(&[0.5, 0.5]),
				HashMap::new(),
			)
			.unwrap();

		let results = store
			.text_search(&TextSearchOptions {
				query: "hello world".to_string(),
				mode: Some("fuzzy".to_string()),
				threshold: Some(0.5),
			})
			.unwrap();

		assert!(!results.is_empty());
		assert_eq!(results[0].volume.text, "hello world");
	}

	// 11. text_search bm25 mode
	#[test]
	fn test_text_search_bm25() {
		let mut store = init_store(default_config());

		store
			.add(
				"rust programming language systems".to_string(),
				make_embedding(&[1.0, 0.0]),
				HashMap::new(),
			)
			.unwrap();
		store
			.add(
				"python programming language scripting".to_string(),
				make_embedding(&[0.0, 1.0]),
				HashMap::new(),
			)
			.unwrap();

		let results = store
			.text_search(&TextSearchOptions {
				query: "rust programming".to_string(),
				mode: Some("bm25".to_string()),
				threshold: Some(0.0),
			})
			.unwrap();

		assert!(!results.is_empty());
		// Both should appear but rust entry should score higher
		assert_eq!(results[0].volume.text, "rust programming language systems");
	}

	// 12. filter_by_metadata with eq filter
	#[test]
	fn test_filter_by_metadata() {
		let mut store = init_store(default_config());

		store
			.add(
				"rust entry".to_string(),
				make_embedding(&[1.0, 0.0]),
				make_metadata(&[("lang", "rust")]),
			)
			.unwrap();
		store
			.add(
				"python entry".to_string(),
				make_embedding(&[0.0, 1.0]),
				make_metadata(&[("lang", "python")]),
			)
			.unwrap();

		let results = store.filter_by_metadata(&[MetadataFilter {
			key: "lang".to_string(),
			value: Some(json!("rust")),
			mode: Some("eq".to_string()),
		}]);

		assert_eq!(results.len(), 1);
		assert_eq!(results[0].text, "rust entry");
	}

	// 13. filter_by_date_range
	#[test]
	fn test_filter_by_date_range() {
		let mut store = init_store(default_config());

		let id1 = store
			.add(
				"old entry".to_string(),
				make_embedding(&[1.0, 0.0]),
				HashMap::new(),
			)
			.unwrap();
		// Manually set timestamp for testing
		if let Some(vol) = store.volumes.iter_mut().find(|v| v.id == id1) {
			vol.timestamp = 1000;
		}

		let id2 = store
			.add(
				"new entry".to_string(),
				make_embedding(&[0.0, 1.0]),
				HashMap::new(),
			)
			.unwrap();
		if let Some(vol) = store.volumes.iter_mut().find(|v| v.id == id2) {
			vol.timestamp = 5000;
		}

		// Only entries after timestamp 2000
		let results = store.filter_by_date_range(&DateRange {
			after: Some(2000),
			before: None,
		});
		assert_eq!(results.len(), 1);
		assert_eq!(results[0].text, "new entry");

		// Only entries before timestamp 2000
		let results2 = store.filter_by_date_range(&DateRange {
			after: None,
			before: Some(2000),
		});
		assert_eq!(results2.len(), 1);
		assert_eq!(results2[0].text, "old entry");
	}

	// 14. advanced_search combined
	#[test]
	fn test_advanced_search() {
		let mut store = init_store(default_config());

		store
			.add(
				"rust programming language".to_string(),
				make_embedding(&[1.0, 0.0, 0.0]),
				make_metadata(&[("lang", "rust")]),
			)
			.unwrap();
		store
			.add(
				"python programming language".to_string(),
				make_embedding(&[0.0, 1.0, 0.0]),
				make_metadata(&[("lang", "python")]),
			)
			.unwrap();

		let results = store
			.advanced_search(&SearchOptions {
				query_embedding: Some(make_embedding(&[1.0, 0.0, 0.0])),
				similarity_threshold: Some(0.0),
				text: Some(TextSearchOptions {
					query: "rust".to_string(),
					mode: Some("fuzzy".to_string()),
					threshold: Some(0.0),
				}),
				metadata: None,
				date_range: None,
				max_results: Some(10),
				rank_by: Some("average".to_string()),
				field_boosts: None,
				rank_weights: None,
				topic_filter: None,
			})
			.unwrap();

		assert!(!results.is_empty());
		// The rust entry should score highest (both vector + text match)
		assert_eq!(
			results[0].volume.text,
			"rust programming language"
		);
		assert!(results[0].scores.vector.is_some());
		assert!(results[0].scores.text.is_some());
	}

	// 15. recommend returns scored results
	#[test]
	fn test_recommend() {
		let mut store = init_store(default_config());

		store
			.add(
				"entry one".to_string(),
				make_embedding(&[1.0, 0.0, 0.0]),
				HashMap::new(),
			)
			.unwrap();
		store
			.add(
				"entry two".to_string(),
				make_embedding(&[0.0, 1.0, 0.0]),
				HashMap::new(),
			)
			.unwrap();

		let results = store
			.recommend(&RecommendOptions {
				query_embedding: Some(make_embedding(&[1.0, 0.0, 0.0])),
				weights: None,
				max_results: Some(10),
				min_score: Some(0.0),
				metadata: None,
				topics: None,
				date_range: None,
			})
			.unwrap();

		assert!(!results.is_empty());
		// Results should be sorted by score desc
		for i in 1..results.len() {
			assert!(results[i - 1].score >= results[i].score);
		}
	}

	// 16. duplicate detection — skip behavior
	#[test]
	fn test_duplicate_skip() {
		let mut store = init_store(StoreConfig {
			duplicate_threshold: 0.99,
			duplicate_behavior: DuplicateBehavior::Skip,
			..Default::default()
		});

		let id1 = store
			.add(
				"first".to_string(),
				make_embedding(&[1.0, 0.0, 0.0]),
				HashMap::new(),
			)
			.unwrap();

		// Same embedding should be skipped, returning the existing ID
		let id2 = store
			.add(
				"duplicate".to_string(),
				make_embedding(&[1.0, 0.0, 0.0]),
				HashMap::new(),
			)
			.unwrap();

		assert_eq!(id1, id2);
		assert_eq!(store.size(), 1);
	}

	// 17. duplicate detection — error behavior
	#[test]
	fn test_duplicate_error() {
		let mut store = init_store(StoreConfig {
			duplicate_threshold: 0.99,
			duplicate_behavior: DuplicateBehavior::Error,
			..Default::default()
		});

		store
			.add(
				"first".to_string(),
				make_embedding(&[1.0, 0.0, 0.0]),
				HashMap::new(),
			)
			.unwrap();

		let result = store.add(
			"duplicate".to_string(),
			make_embedding(&[1.0, 0.0, 0.0]),
			HashMap::new(),
		);

		assert!(matches!(result, Err(VectorError::Duplicate(_))));
		assert_eq!(store.size(), 1);
	}

	// 18. get_topics returns indexed topics
	#[test]
	fn test_get_topics() {
		let mut store = init_store(default_config());

		store
			.add(
				"rust entry".to_string(),
				make_embedding(&[1.0, 0.0]),
				make_metadata(&[("topic", "rust")]),
			)
			.unwrap();
		store
			.add(
				"python entry".to_string(),
				make_embedding(&[0.0, 1.0]),
				make_metadata(&[("topic", "python")]),
			)
			.unwrap();

		let topics = store.get_topics();
		let topic_names: Vec<String> = topics.iter().map(|t| t.topic.clone()).collect();
		assert!(topic_names.contains(&"rust".to_string()));
		assert!(topic_names.contains(&"python".to_string()));
	}

	// 19. filter_by_topic returns matching
	#[test]
	fn test_filter_by_topic() {
		let mut store = init_store(default_config());

		store
			.add(
				"rust entry".to_string(),
				make_embedding(&[1.0, 0.0]),
				make_metadata(&[("topic", "rust")]),
			)
			.unwrap();
		store
			.add(
				"python entry".to_string(),
				make_embedding(&[0.0, 1.0]),
				make_metadata(&[("topic", "python")]),
			)
			.unwrap();

		let results = store.filter_by_topic(&["rust".to_string()]);
		assert_eq!(results.len(), 1);
		assert_eq!(results[0].text, "rust entry");
	}

	// 20. find_duplicates finds groups
	#[test]
	fn test_find_duplicates() {
		let mut store = init_store(default_config());

		store
			.add(
				"entry a".to_string(),
				make_embedding(&[1.0, 0.0, 0.0]),
				HashMap::new(),
			)
			.unwrap();
		store
			.add(
				"entry b".to_string(),
				make_embedding(&[0.99, 0.01, 0.0]),
				HashMap::new(),
			)
			.unwrap();
		store
			.add(
				"entry c".to_string(),
				make_embedding(&[0.0, 1.0, 0.0]),
				HashMap::new(),
			)
			.unwrap();

		let groups = store.find_duplicates(Some(0.9));
		assert_eq!(groups.len(), 1);
		assert_eq!(groups[0].duplicates.len(), 1);
	}

	// 21. size and is_dirty getters
	#[test]
	fn test_size_and_dirty() {
		let mut store = init_store(default_config());

		assert_eq!(store.size(), 0);
		assert!(!store.is_dirty());

		store
			.add(
				"test".to_string(),
				make_embedding(&[1.0]),
				HashMap::new(),
			)
			.unwrap();

		assert_eq!(store.size(), 1);
		assert!(store.is_dirty());
	}

	// 22. catalog_resolve + catalog_sections
	#[test]
	fn test_catalog_resolve_and_sections() {
		let mut store = init_store(default_config());

		let topic = store.catalog_resolve("Rust Programming");
		assert_eq!(topic, "rust programming");

		// Same topic should resolve to itself
		let topic2 = store.catalog_resolve("rust programming");
		assert_eq!(topic2, "rust programming");

		let sections = store.catalog_sections();
		assert!(sections
			.iter()
			.any(|s| s.topic == "rust programming"));
	}

	// 23. save + initialize round-trip via tempdir
	#[test]
	fn test_save_load_roundtrip() {
		let dir = tempfile::tempdir().unwrap();
		let dir_path = dir.path().to_str().unwrap().to_string();

		// Create and populate a store
		let mut store = init_store(StoreConfig {
			storage_path: Some(dir_path.clone()),
			..Default::default()
		});

		let id = store
			.add(
				"persisted entry".to_string(),
				make_embedding(&[0.1, 0.2, 0.3]),
				make_metadata(&[("topic", "testing")]),
			)
			.unwrap();

		store.save().unwrap();
		assert!(!store.is_dirty());

		// Create a fresh store and load from disk
		let mut store2 = VolumeStore::new(StoreConfig {
			storage_path: Some(dir_path.clone()),
			..Default::default()
		});
		store2.initialize(None).unwrap();

		assert_eq!(store2.size(), 1);
		let vol = store2.get_by_id(&id).unwrap();
		assert_eq!(vol.text, "persisted entry");
		assert_eq!(vol.metadata.get("topic").unwrap(), "testing");
	}

	// 24. check_duplicate returns correct result
	#[test]
	fn test_check_duplicate() {
		let mut store = init_store(default_config());

		store
			.add(
				"test".to_string(),
				make_embedding(&[1.0, 0.0, 0.0]),
				HashMap::new(),
			)
			.unwrap();

		let result = store.check_duplicate(&make_embedding(&[1.0, 0.0, 0.0]));
		assert!(result.is_duplicate);

		let result2 = store.check_duplicate(&make_embedding(&[0.0, 1.0, 0.0]));
		assert!(!result2.is_duplicate);
	}

	// 25. not initialized errors
	#[test]
	fn test_not_initialized_errors() {
		let mut store = VolumeStore::new(default_config());

		assert!(matches!(
			store.add("test".to_string(), make_embedding(&[1.0]), HashMap::new()),
			Err(VectorError::NotInitialized)
		));

		assert!(matches!(
			store.search(&make_embedding(&[1.0]), 10, 0.5),
			Err(VectorError::NotInitialized)
		));

		assert!(matches!(
			store.text_search(&TextSearchOptions {
				query: "test".to_string(),
				mode: None,
				threshold: None,
			}),
			Err(VectorError::NotInitialized)
		));

		assert!(matches!(
			store.advanced_search(&SearchOptions {
				query_embedding: None,
				similarity_threshold: None,
				text: None,
				metadata: None,
				date_range: None,
				max_results: None,
				rank_by: None,
				field_boosts: None,
				rank_weights: None,
				topic_filter: None,
			}),
			Err(VectorError::NotInitialized)
		));
	}

	// 26. learning engine integration
	#[test]
	fn test_learning_engine_integration() {
		let mut store = init_store(learning_config());

		let id = store
			.add(
				"test entry".to_string(),
				make_embedding(&[1.0, 0.0, 0.0]),
				HashMap::new(),
			)
			.unwrap();

		// Record query
		store.record_query(
			&make_embedding(&[1.0, 0.0, 0.0]),
			&[id.clone()],
		);

		// Record feedback
		store.record_feedback(&id, true);

		// Profile should exist
		let profile = store.get_profile();
		assert!(profile.is_some());
		let p = profile.unwrap();
		assert_eq!(p.total_queries, 1);
	}

	// 27. text_search with exact mode
	#[test]
	fn test_text_search_exact() {
		let mut store = init_store(default_config());

		store
			.add(
				"exact match text".to_string(),
				make_embedding(&[1.0, 0.0]),
				HashMap::new(),
			)
			.unwrap();
		store
			.add(
				"no match here".to_string(),
				make_embedding(&[0.0, 1.0]),
				HashMap::new(),
			)
			.unwrap();

		let results = store
			.text_search(&TextSearchOptions {
				query: "exact match text".to_string(),
				mode: Some("exact".to_string()),
				threshold: Some(0.0),
			})
			.unwrap();

		assert_eq!(results.len(), 1);
		assert_eq!(results[0].volume.text, "exact match text");
		assert!((results[0].score - 1.0).abs() < f64::EPSILON);
	}

	// 28. advanced_search with metadata filter
	#[test]
	fn test_advanced_search_with_metadata() {
		let mut store = init_store(default_config());

		store
			.add(
				"rust systems".to_string(),
				make_embedding(&[1.0, 0.0, 0.0]),
				make_metadata(&[("lang", "rust")]),
			)
			.unwrap();
		store
			.add(
				"python scripting".to_string(),
				make_embedding(&[0.9, 0.1, 0.0]),
				make_metadata(&[("lang", "python")]),
			)
			.unwrap();

		let results = store
			.advanced_search(&SearchOptions {
				query_embedding: Some(make_embedding(&[1.0, 0.0, 0.0])),
				similarity_threshold: Some(0.0),
				text: None,
				metadata: Some(vec![MetadataFilter {
					key: "lang".to_string(),
					value: Some(json!("rust")),
					mode: Some("eq".to_string()),
				}]),
				date_range: None,
				max_results: Some(10),
				rank_by: None,
				field_boosts: None,
				rank_weights: None,
				topic_filter: None,
			})
			.unwrap();

		assert_eq!(results.len(), 1);
		assert_eq!(results[0].volume.text, "rust systems");
	}

	// 29. recommend with topic filter
	#[test]
	fn test_recommend_with_topic_filter() {
		let mut store = init_store(default_config());

		store
			.add(
				"rust entry".to_string(),
				make_embedding(&[1.0, 0.0, 0.0]),
				make_metadata(&[("topic", "rust")]),
			)
			.unwrap();
		store
			.add(
				"python entry".to_string(),
				make_embedding(&[0.0, 1.0, 0.0]),
				make_metadata(&[("topic", "python")]),
			)
			.unwrap();

		let results = store
			.recommend(&RecommendOptions {
				query_embedding: Some(make_embedding(&[1.0, 0.0, 0.0])),
				weights: None,
				max_results: Some(10),
				min_score: Some(0.0),
				metadata: None,
				topics: Some(vec!["rust".to_string()]),
				date_range: None,
			})
			.unwrap();

		assert_eq!(results.len(), 1);
		assert_eq!(results[0].volume.text, "rust entry");
	}

	// 30. catalog_relocate and catalog_volumes
	#[test]
	fn test_catalog_relocate() {
		let mut store = init_store(default_config());

		store.topic_catalog.register_volume("vol-1", "old-topic");
		assert!(store
			.catalog_volumes("old-topic")
			.contains(&"vol-1".to_string()));

		store.catalog_relocate("vol-1", "new-topic");
		assert!(store.catalog_volumes("old-topic").is_empty());
		assert!(store
			.catalog_volumes("new-topic")
			.contains(&"vol-1".to_string()));
	}

	// 31. catalog_merge
	#[test]
	fn test_catalog_merge() {
		let mut store = init_store(default_config());

		store.topic_catalog.register_volume("vol-1", "javascript");
		store.topic_catalog.register_volume("vol-2", "typescript");

		store.catalog_merge("javascript", "typescript");

		let ts_vols = store.catalog_volumes("typescript");
		assert!(ts_vols.contains(&"vol-1".to_string()));
		assert!(ts_vols.contains(&"vol-2".to_string()));
		assert!(store.catalog_volumes("javascript").is_empty());
	}

	// 32. dispose saves dirty store
	#[test]
	fn test_dispose_saves() {
		let dir = tempfile::tempdir().unwrap();
		let dir_path = dir.path().to_str().unwrap().to_string();

		let mut store = init_store(StoreConfig {
			storage_path: Some(dir_path.clone()),
			..Default::default()
		});

		store
			.add(
				"test".to_string(),
				make_embedding(&[1.0]),
				HashMap::new(),
			)
			.unwrap();

		assert!(store.is_dirty());
		store.dispose().unwrap();

		// Verify data was saved
		let mut store2 = VolumeStore::new(StoreConfig {
			storage_path: Some(dir_path),
			..Default::default()
		});
		store2.initialize(None).unwrap();
		assert_eq!(store2.size(), 1);
	}

	// 33. search with zero magnitude query returns empty
	#[test]
	fn test_search_zero_magnitude() {
		let mut store = init_store(default_config());

		store
			.add(
				"test".to_string(),
				make_embedding(&[1.0, 0.0]),
				HashMap::new(),
			)
			.unwrap();

		let results = store
			.search(&make_embedding(&[0.0, 0.0]), 10, 0.0)
			.unwrap();
		assert!(results.is_empty());
	}

	// 34. get_all returns clones
	#[test]
	fn test_get_all() {
		let mut store = init_store(default_config());

		store
			.add(
				"first".to_string(),
				make_embedding(&[1.0]),
				HashMap::new(),
			)
			.unwrap();
		store
			.add(
				"second".to_string(),
				make_embedding(&[0.0]),
				HashMap::new(),
			)
			.unwrap();

		let all = store.get_all();
		assert_eq!(all.len(), 2);
	}

	// 35. filter_by_metadata complex filter (linear scan)
	#[test]
	fn test_filter_by_metadata_contains() {
		let mut store = init_store(default_config());

		store
			.add(
				"entry a".to_string(),
				make_embedding(&[1.0, 0.0]),
				make_metadata(&[("desc", "hello world")]),
			)
			.unwrap();
		store
			.add(
				"entry b".to_string(),
				make_embedding(&[0.0, 1.0]),
				make_metadata(&[("desc", "goodbye world")]),
			)
			.unwrap();

		let results = store.filter_by_metadata(&[MetadataFilter {
			key: "desc".to_string(),
			value: Some(json!("hello")),
			mode: Some("contains".to_string()),
		}]);

		assert_eq!(results.len(), 1);
		assert_eq!(results[0].text, "entry a");
	}
}
