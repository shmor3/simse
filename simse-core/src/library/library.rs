//! High-level Library wrapping `Store` with automatic embedding,
//! event publishing, shelf management, and compendium (LLM summarization).
//!
//! The `Library` struct holds a `Store` behind `Arc<Mutex<_>>` so it
//! can be shared across shelves and async tasks. All mutating operations
//! acquire the lock, perform work, and release it before awaiting embeddings
//! or text generation so the lock is never held across `.await` points.

use std::collections::{HashMap, HashSet};
use std::mem;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use simse_adaptive_engine::store::{StoreConfig, Store};
use simse_adaptive_engine::types::{
	AdvancedLookup, DateRange, DuplicateCheckResult, DuplicateCluster, Entry, Lookup, MetadataFilter,
	LearnerProfile, Recommendation, RecommendOptions, SearchOptions, TextLookup, TextSearchOptions,
	TopicInfo,
};

use crate::error::{LibraryErrorCode, SimseError};
use crate::events::EventBus;

use super::query_dsl::parse_query;
use super::shelf::Shelf;

// ---------------------------------------------------------------------------
// Traits
// ---------------------------------------------------------------------------

/// Async embedding provider — converts text into embedding vectors.
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
	/// Embed one or more text inputs using the given model identifier.
	///
	/// Returns one embedding vector per input string.
	async fn embed(
		&self,
		input: &[String],
		model: &str,
	) -> Result<Vec<Vec<f32>>, SimseError>;
}

/// Async text generation provider — used for compendium (summarization).
#[async_trait]
pub trait TextGenerationProvider: Send + Sync {
	/// Generate text from a prompt with an optional system prompt.
	async fn generate(
		&self,
		prompt: &str,
		system_prompt: Option<&str>,
	) -> Result<String, SimseError>;
}

// ---------------------------------------------------------------------------
// CompendiumOptions / CompendiumResult
// ---------------------------------------------------------------------------

/// Options for creating a compendium (summarization of multiple entries).
#[derive(Debug, Clone)]
pub struct CompendiumOptions {
	/// IDs of entries to summarize (minimum 2).
	pub ids: Vec<String>,
	/// Custom instruction prompt for the summarization.
	pub prompt: Option<String>,
	/// Optional system prompt passed to the text generation provider.
	pub system_prompt: Option<String>,
	/// If true, delete the original entries after summarization.
	pub delete_originals: bool,
	/// Additional metadata to attach to the compendium entry.
	pub metadata: HashMap<String, String>,
}

/// Result of a compendium operation.
#[derive(Debug, Clone)]
pub struct CompendiumResult {
	/// ID of the newly created compendium entry.
	pub compendium_id: String,
	/// The generated summary text.
	pub text: String,
	/// IDs of the source entries that were summarized.
	pub source_ids: Vec<String>,
	/// Whether the originals were deleted.
	pub deleted_originals: bool,
}

// ---------------------------------------------------------------------------
// LibraryConfig
// ---------------------------------------------------------------------------

/// Configuration for the Library.
#[derive(Debug, Clone)]
pub struct LibraryConfig {
	/// Model identifier passed to the embedding provider.
	pub embedding_agent: String,
	/// Default maximum results for search operations.
	pub max_results: usize,
	/// Default similarity threshold for search operations.
	pub similarity_threshold: f64,
}

impl Default for LibraryConfig {
	fn default() -> Self {
		Self {
			embedding_agent: "default".to_string(),
			max_results: 10,
			similarity_threshold: 0.0,
		}
	}
}

// ---------------------------------------------------------------------------
// Library
// ---------------------------------------------------------------------------

/// High-level vector library with automatic embedding, shelf management,
/// and compendium support.
///
/// Wraps a `Store` with:
/// - Automatic text-to-embedding conversion via `EmbeddingProvider`
/// - Event publishing via `EventBus`
/// - Shelf (agent-scoped partition) caching
/// - Compendium (LLM summarization) via `TextGenerationProvider`
pub struct Library {
	store: Arc<Mutex<Store>>,
	embedder: Arc<dyn EmbeddingProvider>,
	config: LibraryConfig,
	event_bus: Option<EventBus>,
	text_generator: Mutex<Option<Arc<dyn TextGenerationProvider>>>,
	initialized: Mutex<bool>,
	shelf_cache: Mutex<HashMap<String, Shelf>>,
}

impl Library {
	/// Create a new Library wrapping a fresh `Store`.
	///
	/// Call [`initialize`](Library::initialize) before use.
	pub fn new(
		embedder: Arc<dyn EmbeddingProvider>,
		config: LibraryConfig,
		store_config: StoreConfig,
		event_bus: Option<EventBus>,
	) -> Self {
		let store = Store::new(store_config);
		Self {
			store: Arc::new(Mutex::new(store)),
			embedder,
			config,
			event_bus,
			text_generator: Mutex::new(None),
			initialized: Mutex::new(false),
			shelf_cache: Mutex::new(HashMap::new()),
		}
	}

	/// Create a Library from an existing `Store` wrapped in `Arc<Mutex<_>>`.
	pub fn from_store(
		store: Arc<Mutex<Store>>,
		embedder: Arc<dyn EmbeddingProvider>,
		config: LibraryConfig,
		event_bus: Option<EventBus>,
	) -> Self {
		Self {
			store,
			embedder,
			config,
			event_bus,
			text_generator: Mutex::new(None),
			initialized: Mutex::new(false),
			shelf_cache: Mutex::new(HashMap::new()),
		}
	}

	// -----------------------------------------------------------------------
	// Internal helpers
	// -----------------------------------------------------------------------

	fn ensure_initialized(&self) -> Result<(), SimseError> {
		let init = self.initialized.lock().unwrap_or_else(|e| e.into_inner());
		if !*init {
			return Err(SimseError::library(
				LibraryErrorCode::NotInitialized,
				"Library has not been initialized. Call initialize() first.",
			));
		}
		Ok(())
	}

	async fn get_embedding(&self, text: &str) -> Result<Vec<f32>, SimseError> {
		let results = self
			.embedder
			.embed(&[text.to_string()], &self.config.embedding_agent)
			.await?;

		let embedding = results
			.into_iter()
			.next()
			.ok_or_else(|| {
				SimseError::library(
					LibraryErrorCode::EmbeddingFailed,
					"Embedding provider returned no embeddings",
				)
			})?;

		if embedding.is_empty() {
			return Err(SimseError::library(
				LibraryErrorCode::EmbeddingFailed,
				"Embedding provider returned an empty embedding vector",
			));
		}

		Ok(embedding)
	}

	fn publish(&self, event_type: &str, payload: serde_json::Value) {
		if let Some(ref bus) = self.event_bus {
			bus.publish(event_type, payload);
		}
	}

	// -----------------------------------------------------------------------
	// Lifecycle
	// -----------------------------------------------------------------------

	/// Initialize the library and its underlying store.
	///
	/// If a `storage_path` was set in `StoreConfig`, persisted data is loaded
	/// from disk.
	pub fn initialize(&self, storage_path: Option<&str>) -> Result<(), SimseError> {
		{
			let init = self.initialized.lock().unwrap_or_else(|e| e.into_inner());
			if *init {
				return Ok(());
			}
		}

		{
			let mut store = self.store.lock().unwrap_or_else(|e| e.into_inner());
			let taken = mem::replace(&mut *store, Store::new(StoreConfig::default()));
			*store = taken.initialize(storage_path)?;
		}

		{
			let mut init = self.initialized.lock().unwrap_or_else(|e| e.into_inner());
			*init = true;
		}

		Ok(())
	}

	/// Dispose the library: save if dirty, then clean up resources.
	pub fn dispose(&self) -> Result<(), SimseError> {
		{
			let mut store = self.store.lock().unwrap_or_else(|e| e.into_inner());
			let taken = mem::replace(&mut *store, Store::new(StoreConfig::default()));
			*store = taken.dispose()?;
		}

		{
			let mut cache = self.shelf_cache.lock().unwrap_or_else(|e| e.into_inner());
			cache.clear();
		}

		{
			let mut init = self.initialized.lock().unwrap_or_else(|e| e.into_inner());
			*init = false;
		}

		Ok(())
	}

	// -----------------------------------------------------------------------
	// Write operations
	// -----------------------------------------------------------------------

	/// Embed text and add it to the library.
	///
	/// Returns the generated entry ID.
	pub async fn add(
		&self,
		text: &str,
		metadata: HashMap<String, String>,
	) -> Result<String, SimseError> {
		self.ensure_initialized()?;

		if text.trim().is_empty() {
			return Err(SimseError::library(
				LibraryErrorCode::EmptyText,
				"Cannot add empty or whitespace-only text to library",
			));
		}

		let embedding = self.get_embedding(text).await?;

		let id = {
			let mut store = self.store.lock().unwrap_or_else(|e| e.into_inner());
			let taken = mem::replace(&mut *store, Store::new(StoreConfig::default()));
			let (new_store, id) = taken.add(text.to_string(), embedding, metadata)?;
			*store = new_store;
			id
		};

		self.publish(
			crate::events::event_types::LIBRARY_STORE,
			serde_json::json!({ "id": id, "contentLength": text.len() }),
		);

		Ok(id)
	}

	/// Embed multiple texts and add them in batch.
	///
	/// Returns the generated entry IDs.
	pub async fn add_batch(
		&self,
		entries: &[(&str, HashMap<String, String>)],
	) -> Result<Vec<String>, SimseError> {
		self.ensure_initialized()?;

		if entries.is_empty() {
			return Ok(Vec::new());
		}

		// Validate no empty texts
		for (i, (text, _)) in entries.iter().enumerate() {
			if text.trim().is_empty() {
				return Err(SimseError::library(
					LibraryErrorCode::EmptyText,
					format!(
						"Cannot add empty or whitespace-only text to library (batch index {})",
						i
					),
				));
			}
		}

		// Embed all texts at once
		let texts: Vec<String> = entries.iter().map(|(t, _)| t.to_string()).collect();
		let embeddings = self
			.embedder
			.embed(&texts, &self.config.embedding_agent)
			.await?;

		if embeddings.len() < entries.len() {
			return Err(SimseError::library(
				LibraryErrorCode::EmbeddingFailed,
				format!(
					"Embedding provider returned {} embeddings for {} inputs",
					embeddings.len(),
					entries.len()
				),
			));
		}

		// Validate each embedding
		for (i, emb) in embeddings.iter().enumerate() {
			if emb.is_empty() {
				return Err(SimseError::library(
					LibraryErrorCode::EmbeddingFailed,
					format!(
						"Embedding provider returned an empty embedding vector at index {}",
						i
					),
				));
			}
		}

		// Build batch entries for the store
		let store_entries: Vec<simse_adaptive_engine::store::AddEntry> = entries
			.iter()
			.zip(embeddings.into_iter())
			.map(|((text, meta), emb)| simse_adaptive_engine::store::AddEntry {
				text: text.to_string(),
				embedding: emb,
				metadata: meta.clone(),
			})
			.collect();

		let ids = {
			let mut store = self.store.lock().unwrap_or_else(|e| e.into_inner());
			let taken = mem::replace(&mut *store, Store::new(StoreConfig::default()));
			let (new_store, ids) = taken.add_batch(store_entries)?;
			*store = new_store;
			ids
		};

		Ok(ids)
	}

	// -----------------------------------------------------------------------
	// Search
	// -----------------------------------------------------------------------

	/// Embed a query string and perform vector similarity search.
	pub async fn search(
		&self,
		query: &str,
		max_results: Option<usize>,
		threshold: Option<f64>,
	) -> Result<Vec<Lookup>, SimseError> {
		self.ensure_initialized()?;

		if query.trim().is_empty() {
			return Ok(Vec::new());
		}

		let embedding = self.get_embedding(query).await?;

		let results = {
			let mut store = self.store.lock().unwrap_or_else(|e| e.into_inner());
			let taken = mem::replace(&mut *store, Store::new(StoreConfig::default()));
			let (new_store, results) = taken.search(
				&embedding,
				max_results.unwrap_or(self.config.max_results),
				threshold.unwrap_or(self.config.similarity_threshold),
			)?;
			*store = new_store;
			results
		};

		self.publish(
			crate::events::event_types::LIBRARY_SEARCH,
			serde_json::json!({
				"query": query,
				"resultCount": results.len(),
			}),
		);

		Ok(results)
	}

	/// Content-based text search (no embedding needed).
	pub fn text_search(
		&self,
		options: &TextSearchOptions,
	) -> Result<Vec<TextLookup>, SimseError> {
		self.ensure_initialized()?;

		if options.query.trim().is_empty() {
			return Ok(Vec::new());
		}

		let store = self.store.lock().unwrap_or_else(|e| e.into_inner());
		Ok(store.text_search(options)?)
	}

	/// Filter entries by metadata filters (logical AND).
	pub fn filter_by_metadata(
		&self,
		filters: &[MetadataFilter],
	) -> Result<Vec<Entry>, SimseError> {
		self.ensure_initialized()?;

		let store = self.store.lock().unwrap_or_else(|e| e.into_inner());
		Ok(store.filter_by_metadata(filters))
	}

	/// Filter entries by timestamp range.
	pub fn filter_by_date_range(&self, range: &DateRange) -> Result<Vec<Entry>, SimseError> {
		self.ensure_initialized()?;

		let store = self.store.lock().unwrap_or_else(|e| e.into_inner());
		Ok(store.filter_by_date_range(range))
	}

	/// Advanced / combined search with optional auto-embedding.
	///
	/// If `query_embedding` is not provided but a text query is present,
	/// the text is automatically embedded for vector search.
	pub async fn advanced_search(
		&self,
		options: &SearchOptions,
	) -> Result<Vec<AdvancedLookup>, SimseError> {
		self.ensure_initialized()?;

		let mut resolved = options.clone();

		// Auto-embed text query if no embedding provided
		if resolved.query_embedding.is_none()
			&& let Some(ref text_opts) = resolved.text {
				let trimmed = text_opts.query.trim();
				if !trimmed.is_empty()
					&& let Ok(emb) = self.get_embedding(trimmed).await {
						resolved.query_embedding = Some(emb);
					}
			}

		let mut store = self.store.lock().unwrap_or_else(|e| e.into_inner());
		let taken = mem::replace(&mut *store, Store::new(StoreConfig::default()));
		let (new_store, results) = taken.advanced_search(&resolved)?;
		*store = new_store;
		Ok(results)
	}

	/// Parse a DSL query string and run an advanced search.
	///
	/// Auto-embeds the text query for vector search and applies topic filters.
	pub async fn query(&self, dsl: &str) -> Result<Vec<AdvancedLookup>, SimseError> {
		self.ensure_initialized()?;

		let parsed = parse_query(dsl);

		let mut search_options = SearchOptions {
			query_embedding: None,
			similarity_threshold: parsed.min_score,
			text: parsed.text_search.as_ref().and_then(|ts| {
				if ts.query.is_empty() {
					None
				} else {
					Some(TextSearchOptions {
						query: ts.query.clone(),
						mode: Some(ts.mode.clone()),
						threshold: None,
					})
				}
			}),
			metadata: parsed.metadata_filters.clone(),
			date_range: None,
			max_results: None,
			rank_by: None,
			field_boosts: None,
			rank_weights: None,
			topic_filter: None,
			graph_boost: None,
		};

		// Auto-embed text query
		if let Some(ref ts) = parsed.text_search {
			let trimmed = ts.query.trim();
			if !trimmed.is_empty()
				&& let Ok(emb) = self.get_embedding(trimmed).await {
					search_options.query_embedding = Some(emb);
				}
		}

		let mut results = {
			let mut store = self.store.lock().unwrap_or_else(|e| e.into_inner());
			let taken = mem::replace(&mut *store, Store::new(StoreConfig::default()));
			let (new_store, results) = taken.advanced_search(&search_options)?;
			*store = new_store;
			results
		};

		// Apply topic filter post-hoc
		if let Some(ref topics) = parsed.topic_filter
			&& !topics.is_empty() {
				let topic_ids: HashSet<String> = {
					let store = self.store.lock().unwrap_or_else(|e| e.into_inner());
					store
						.filter_by_topic(topics)
						.into_iter()
						.map(|v| v.id)
						.collect()
				};
				results.retain(|r| topic_ids.contains(&r.entry.id));
			}

		Ok(results)
	}

	// -----------------------------------------------------------------------
	// Accessors
	// -----------------------------------------------------------------------

	/// Get an entry by ID.
	pub fn get_by_id(&self, id: &str) -> Result<Option<Entry>, SimseError> {
		self.ensure_initialized()?;

		let mut store = self.store.lock().unwrap_or_else(|e| e.into_inner());
		let taken = mem::replace(&mut *store, Store::new(StoreConfig::default()));
		let (new_store, vol) = taken.get_by_id(id);
		*store = new_store;
		Ok(vol)
	}

	/// Return all entries.
	pub fn get_all(&self) -> Result<Vec<Entry>, SimseError> {
		self.ensure_initialized()?;

		let store = self.store.lock().unwrap_or_else(|e| e.into_inner());
		Ok(store.get_all())
	}

	/// Get all topics.
	pub fn get_topics(&self) -> Result<Vec<TopicInfo>, SimseError> {
		self.ensure_initialized()?;

		let store = self.store.lock().unwrap_or_else(|e| e.into_inner());
		Ok(store.get_topics())
	}

	/// Filter entries by topic names.
	pub fn filter_by_topic(&self, topics: &[String]) -> Result<Vec<Entry>, SimseError> {
		self.ensure_initialized()?;

		let store = self.store.lock().unwrap_or_else(|e| e.into_inner());
		Ok(store.filter_by_topic(topics))
	}

	// -----------------------------------------------------------------------
	// Recommendation
	// -----------------------------------------------------------------------

	/// Embed a query and compute recommendations.
	pub async fn recommend(
		&self,
		query: &str,
		options: Option<RecommendOptions>,
	) -> Result<Vec<Recommendation>, SimseError> {
		self.ensure_initialized()?;

		if query.trim().is_empty() {
			return Ok(Vec::new());
		}

		let embedding = self.get_embedding(query).await?;

		let mut opts = options.unwrap_or(RecommendOptions {
			query_embedding: None,
			weights: None,
			max_results: None,
			min_score: None,
			metadata: None,
			topics: None,
			date_range: None,
		});
		opts.query_embedding = Some(embedding);

		let results = {
			let store = self.store.lock().unwrap_or_else(|e| e.into_inner());
			store.recommend(&opts)?
		};

		self.publish(
			crate::events::event_types::LIBRARY_RECOMMEND,
			serde_json::json!({
				"query": query,
				"resultCount": results.len(),
			}),
		);

		Ok(results)
	}

	// -----------------------------------------------------------------------
	// Deduplication
	// -----------------------------------------------------------------------

	/// Find groups of near-duplicate entries.
	pub fn find_duplicates(
		&self,
		threshold: Option<f64>,
	) -> Result<Vec<DuplicateCluster>, SimseError> {
		self.ensure_initialized()?;

		let store = self.store.lock().unwrap_or_else(|e| e.into_inner());
		Ok(store.find_duplicates(threshold))
	}

	/// Embed text and check if it would be a duplicate.
	pub async fn check_duplicate(&self, text: &str) -> Result<DuplicateCheckResult, SimseError> {
		self.ensure_initialized()?;

		if text.trim().is_empty() {
			return Ok(DuplicateCheckResult {
				is_duplicate: false,
				existing_entry: None,
				similarity: None,
			});
		}

		let embedding = self.get_embedding(text).await?;

		let store = self.store.lock().unwrap_or_else(|e| e.into_inner());
		Ok(store.check_duplicate(&embedding))
	}

	// -----------------------------------------------------------------------
	// Compendium
	// -----------------------------------------------------------------------

	/// Create a compendium (summary) of multiple entries using a text generation provider.
	///
	/// Gathers the specified entries, asks the LLM to summarize them, then
	/// embeds and stores the summary as a new entry. Optionally deletes the originals.
	pub async fn compendium(
		&self,
		options: CompendiumOptions,
	) -> Result<CompendiumResult, SimseError> {
		self.ensure_initialized()?;

		let generator = {
			let tg = self
				.text_generator
				.lock()
				.unwrap_or_else(|e| e.into_inner());
			tg.clone().ok_or_else(|| {
				SimseError::library(
					LibraryErrorCode::NotInitialized,
					"Compendium requires a text generator. Call set_text_generator() first.",
				)
			})?
		};

		if options.ids.len() < 2 {
			return Err(SimseError::library(
				LibraryErrorCode::InvalidInput,
				"Compendium requires at least 2 entry IDs",
			));
		}

		// Gather source entries
		let source_entries = {
			let mut store = self.store.lock().unwrap_or_else(|e| e.into_inner());
			let mut vols = Vec::new();
			for id in &options.ids {
				let taken = mem::replace(&mut *store, Store::new(StoreConfig::default()));
				let (new_store, vol) = taken.get_by_id(id);
				*store = new_store;
				let vol = vol.ok_or_else(|| {
					SimseError::library(
						LibraryErrorCode::NotFound,
						format!("Entry \"{}\" not found for compendium", id),
					)
				})?;
				vols.push(vol);
			}
			vols
		};

		// Build prompt
		let combined_text: String = source_entries
			.iter()
			.enumerate()
			.map(|(i, v)| format!("--- Entry {} ---\n{}", i + 1, v.text))
			.collect::<Vec<_>>()
			.join("\n\n");

		let instruction = options.prompt.as_deref().unwrap_or(
			"Summarize the following entries into a single concise summary that captures all key information:",
		);

		let prompt = format!("{}\n\n{}", instruction, combined_text);

		// Generate summary
		let compendium_text = generator
			.generate(&prompt, options.system_prompt.as_deref())
			.await?;

		// Embed and store
		let compendium_embedding = self.get_embedding(&compendium_text).await?;

		let mut compendium_metadata = options.metadata.clone();
		compendium_metadata.insert(
			"summarizedFrom".to_string(),
			options.ids.join(","),
		);

		let compendium_id = {
			let mut store = self.store.lock().unwrap_or_else(|e| e.into_inner());
			let taken = mem::replace(&mut *store, Store::new(StoreConfig::default()));
			let (new_store, id) = taken.add(
				compendium_text.clone(),
				compendium_embedding,
				compendium_metadata,
			)?;
			*store = new_store;
			id
		};

		// Optionally delete originals
		if options.delete_originals {
			let mut store = self.store.lock().unwrap_or_else(|e| e.into_inner());
			let taken = mem::replace(&mut *store, Store::new(StoreConfig::default()));
			let (new_store, _count) = taken.delete_batch(&options.ids);
			*store = new_store;
		}

		self.publish(
			crate::events::event_types::LIBRARY_COMPENDIUM,
			serde_json::json!({
				"compendiumId": compendium_id,
				"sourceCount": options.ids.len(),
				"deletedOriginals": options.delete_originals,
			}),
		);

		Ok(CompendiumResult {
			compendium_id,
			text: compendium_text,
			source_ids: options.ids,
			deleted_originals: options.delete_originals,
		})
	}

	/// Set (or replace) the text generation provider used for compendium.
	pub fn set_text_generator(&self, provider: Arc<dyn TextGenerationProvider>) {
		let mut tg = self
			.text_generator
			.lock()
			.unwrap_or_else(|e| e.into_inner());
		*tg = Some(provider);
	}

	// -----------------------------------------------------------------------
	// Feedback
	// -----------------------------------------------------------------------

	/// Record explicit user feedback on whether an entry was relevant.
	pub fn record_feedback(
		&self,
		entry_id: &str,
		relevant: bool,
	) -> Result<(), SimseError> {
		self.ensure_initialized()?;

		let mut store = self.store.lock().unwrap_or_else(|e| e.into_inner());
		let taken = mem::replace(&mut *store, Store::new(StoreConfig::default()));
		*store = taken.record_feedback(entry_id, relevant);
		Ok(())
	}

	// -----------------------------------------------------------------------
	// Delete / clear
	// -----------------------------------------------------------------------

	/// Delete an entry by ID. Returns true if it existed and was removed.
	pub fn delete(&self, id: &str) -> Result<bool, SimseError> {
		self.ensure_initialized()?;

		let deleted = {
			let mut store = self.store.lock().unwrap_or_else(|e| e.into_inner());
			let taken = mem::replace(&mut *store, Store::new(StoreConfig::default()));
			let (new_store, deleted) = taken.delete(id);
			*store = new_store;
			deleted
		};

		if deleted {
			self.publish(
				crate::events::event_types::LIBRARY_DELETE,
				serde_json::json!({ "id": id }),
			);
		}

		Ok(deleted)
	}

	/// Delete multiple entries by ID. Returns the count actually removed.
	pub fn delete_batch(&self, ids: &[String]) -> Result<usize, SimseError> {
		self.ensure_initialized()?;

		let mut store = self.store.lock().unwrap_or_else(|e| e.into_inner());
		let taken = mem::replace(&mut *store, Store::new(StoreConfig::default()));
		let (new_store, count) = taken.delete_batch(ids);
		*store = new_store;
		Ok(count)
	}

	/// Remove all entries and reset all indexes.
	pub fn clear(&self) -> Result<(), SimseError> {
		self.ensure_initialized()?;

		{
			let mut store = self.store.lock().unwrap_or_else(|e| e.into_inner());
			let taken = mem::replace(&mut *store, Store::new(StoreConfig::default()));
			*store = taken.clear();
		}

		{
			let mut cache = self.shelf_cache.lock().unwrap_or_else(|e| e.into_inner());
			cache.clear();
		}

		Ok(())
	}

	// -----------------------------------------------------------------------
	// Properties
	// -----------------------------------------------------------------------

	/// Get the learner profile, if learning is enabled.
	pub fn learner_profile(&self) -> Option<LearnerProfile> {
		let store = self.store.lock().unwrap_or_else(|e| e.into_inner());
		store.get_profile()
	}

	/// Number of entries in the store.
	pub fn size(&self) -> usize {
		let store = self.store.lock().unwrap_or_else(|e| e.into_inner());
		store.size()
	}

	/// Whether the library has been initialized.
	pub fn is_initialized(&self) -> bool {
		let init = self.initialized.lock().unwrap_or_else(|e| e.into_inner());
		*init
	}

	/// Whether the store has unsaved changes.
	pub fn is_dirty(&self) -> bool {
		let store = self.store.lock().unwrap_or_else(|e| e.into_inner());
		store.is_dirty()
	}

	/// The embedding agent / model identifier.
	pub fn embedding_agent(&self) -> &str {
		&self.config.embedding_agent
	}

	// -----------------------------------------------------------------------
	// Shelf management
	// -----------------------------------------------------------------------

	/// Get or create a named shelf (agent-scoped partition).
	///
	/// Shelves are cached — calling `shelf("foo")` twice returns the same `Shelf`.
	pub fn shelf(self: &Arc<Self>, name: &str) -> Shelf {
		let mut cache = self.shelf_cache.lock().unwrap_or_else(|e| e.into_inner());
		if let Some(s) = cache.get(name) {
			return s.clone();
		}

		let s = Shelf::new(name.to_string(), Arc::clone(self));
		cache.insert(name.to_string(), s.clone());
		s
	}

	/// List all shelf names currently present in the store's metadata.
	pub fn shelves(&self) -> Result<Vec<String>, SimseError> {
		self.ensure_initialized()?;

		let store = self.store.lock().unwrap_or_else(|e| e.into_inner());
		let mut names = HashSet::new();
		for vol in store.get_all() {
			if let Some(shelf_name) = vol.metadata.get("shelf") {
				names.insert(shelf_name.clone());
			}
		}
		let mut result: Vec<String> = names.into_iter().collect();
		result.sort();
		Ok(result)
	}

	/// Get a reference to the underlying store (for advanced usage).
	pub fn store(&self) -> &Arc<Mutex<Store>> {
		&self.store
	}
}
