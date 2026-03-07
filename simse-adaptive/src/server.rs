// ---------------------------------------------------------------------------
// AdaptiveServer — JSON-RPC dispatcher
// ---------------------------------------------------------------------------
//
// Routes incoming JSON-RPC 2.0 requests (NDJSON over stdin) to Store
// operations.  Follows the same pattern as the VFS server: a main `run()` loop,
// a `dispatch()` match, `with_store` / `with_store_mut` helpers, and
// free-standing handler functions for each method.
// ---------------------------------------------------------------------------

use std::collections::HashMap;
use std::io::{self, BufRead};
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use tokio::sync::mpsc;
use tracing::info;

use crate::pcn::encoder::InputEvent;
use crate::error::AdaptiveError;
use crate::pcn::config::PcnConfig;
use crate::persistence::{load_snapshot, save_snapshot};
use crate::pcn::predictor::Predictor;
use crate::context_format::{format_context, ContextFormatOptions};
use crate::protocol::*;
use crate::query_dsl::parse_query;
use crate::pcn::snapshot::ModelSnapshot;
use crate::store::{AddEntry, DuplicateBehavior, StoreConfig, Store};
use crate::pcn::trainer::TrainingWorker;
use crate::transport::NdjsonTransport;
use crate::types::Lookup;

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

/// JSON-RPC server that dispatches requests to a [`Store`] and PCN predictor.
pub struct AdaptiveServer {
	transport: NdjsonTransport,
	// Vector store
	store: Option<Store>,
	// PCN fields
	snapshot: Arc<RwLock<ModelSnapshot>>,
	predictor: Option<Predictor>,
	event_tx: Option<mpsc::Sender<InputEvent>>,
	pcn_initialized: bool,
	pcn_config: Option<PcnConfig>,
	embedding_dim: usize,
}

impl AdaptiveServer {
	/// Create a new server with the given transport.
	pub fn new(transport: NdjsonTransport) -> Self {
		Self {
			transport,
			store: None,
			snapshot: Arc::new(RwLock::new(ModelSnapshot::empty())),
			predictor: None,
			event_tx: None,
			pcn_initialized: false,
			pcn_config: None,
			embedding_dim: 0,
		}
	}

	/// Main loop: read JSON-RPC messages from stdin, dispatch to handlers.
	pub async fn run(&mut self) -> Result<(), AdaptiveError> {
		let stdin = io::stdin();
		let reader = stdin.lock();

		for line_result in reader.lines() {
			let line = line_result?;
			if line.trim().is_empty() {
				continue;
			}

			let request: JsonRpcRequest = match serde_json::from_str(&line) {
				Ok(r) => r,
				Err(e) => {
					tracing::error!("Failed to parse request: {}", e);
					continue;
				}
			};

			self.dispatch(request).await;
		}

		Ok(())
	}

	// ── Dispatch ──────────────────────────────────────────────────────────

	async fn dispatch(&mut self, req: JsonRpcRequest) {
		let id = req.id;
		let result = match req.method.as_str() {
			// -- Lifecycle -----------------------------------------------
			"store/initialize" => self.handle_initialize(req.params),
			"store/dispose" => self.with_state_transition(|s| {
				let s = s.dispose()?;
				Ok((s, serde_json::json!({})))
			}),
			"store/save" => self.with_state_transition(|s| {
				let s = s.save()?;
				Ok((s, serde_json::json!({})))
			}),

			// -- CRUD ----------------------------------------------------
			"store/add" => self.with_state_transition(|s| handle_add(s, req.params)),
			"store/addBatch" => self.with_state_transition(|s| handle_add_batch(s, req.params)),
			"store/delete" => self.with_state_transition(|s| handle_delete(s, req.params)),
			"store/deleteBatch" => self.with_state_transition(|s| handle_delete_batch(s, req.params)),
			"store/clear" => self.with_state_transition(|s| {
				let s = s.clear();
				Ok((s, serde_json::json!({})))
			}),
			"store/getById" => self.with_state_transition(|s| handle_get_by_id(s, req.params)),
			"store/getAll" => self.with_state(|s| {
				let volumes = s.get_all();
				Ok(serde_json::json!({ "entries": volumes }))
			}),

			// -- Search --------------------------------------------------
			"store/search" => self.with_state_transition(|s| handle_search(s, req.params)),
			"store/textSearch" => self.with_state(|s| handle_text_search(s, req.params)),
			"store/advancedSearch" => {
				self.with_state_transition(|s| handle_advanced_search(s, req.params))
			}
			"store/filterByMetadata" => {
				self.with_state(|s| handle_filter_by_metadata(s, req.params))
			}
			"store/filterByDateRange" => {
				self.with_state(|s| handle_filter_by_date_range(s, req.params))
			}
			"store/filterByTopic" => self.with_state(|s| handle_filter_by_topic(s, req.params)),
			"store/getTopics" => self.with_state(|s| {
				let topics = s.get_topics();
				Ok(serde_json::json!({ "topics": topics }))
			}),

			// -- Recommendation ------------------------------------------
			"store/recommend" => self.with_state(|s| handle_recommend(s, req.params)),

			// -- Deduplication -------------------------------------------
			"store/checkDuplicate" => {
				self.with_state(|s| handle_check_duplicate(s, req.params))
			}
			"store/findDuplicates" => {
				self.with_state(|s| handle_find_duplicates(s, req.params))
			}

			// -- Size / Dirty --------------------------------------------
			"store/size" => self.with_state(|s| Ok(serde_json::json!({ "count": s.size() }))),
			"store/isDirty" => {
				self.with_state(|s| Ok(serde_json::json!({ "dirty": s.is_dirty() })))
			}

			// -- Catalog -------------------------------------------------
			"catalog/resolve" => self.with_state_transition(|s| handle_catalog_resolve(s, req.params)),
			"catalog/relocate" => {
				self.with_state_transition(|s| handle_catalog_relocate(s, req.params))
			}
			"catalog/merge" => self.with_state_transition(|s| handle_catalog_merge(s, req.params)),
			"catalog/sections" => self.with_state(|s| {
				let sections = s.catalog_sections();
				Ok(serde_json::json!({ "sections": sections }))
			}),
			"catalog/volumes" => self.with_state(|s| handle_catalog_entries(s, req.params)),

			// -- Learning ------------------------------------------------
			"learning/recordQuery" => {
				self.with_state_transition(|s| handle_record_query(s, req.params))
			}
			"learning/recordFeedback" => {
				self.with_state_transition(|s| handle_record_feedback(s, req.params))
			}
			"learning/profile" => self.with_state(|s| {
				let profile = s.get_profile();
				Ok(serde_json::json!({ "profile": profile }))
			}),

			// -- Query DSL -----------------------------------------------
			"query/parse" => handle_query_parse(req.params),

			// -- Prompt injection ----------------------------------------
			"format/memoryContext" => handle_format_context(req.params),

			// -- Graph ---------------------------------------------------
			"graph/neighbors" => self.with_state(|s| handle_graph_neighbors(s, req.params)),
			"graph/traverse" => self.with_state(|s| handle_graph_traverse(s, req.params)),

			// -- PCN lifecycle -------------------------------------------
			"pcn/initialize" => self.handle_pcn_initialize(req.params),
			"pcn/dispose" => self.handle_pcn_dispose(),
			"pcn/health" => self.handle_pcn_health(),

			// -- PCN feed ------------------------------------------------
			"feed/event" => self.handle_feed_event(req.params),

			// -- PCN predict ---------------------------------------------
			"predict/confidence" => self.handle_predict_confidence(req.params),
			"predict/anomalies" => self.handle_predict_anomalies(req.params),

			// -- PCN model -----------------------------------------------
			"model/stats" => self.handle_model_stats(),
			"model/snapshot" => self.handle_model_snapshot(req.params),
			"model/restore" => self.handle_model_restore(req.params),
			"model/reset" => self.handle_model_reset(),

			// -- Unknown -------------------------------------------------
			_ => {
				self.transport.write_error(
					id,
					METHOD_NOT_FOUND,
					format!("Unknown method: {}", req.method),
					None,
				);
				return;
			}
		};

		match result {
			Ok(value) => self.transport.write_response(id, value),
			Err(e) => self.transport.write_error(
				id,
				ADAPTIVE_ERROR,
				e.to_string(),
				Some(e.to_json_rpc_error()),
			),
		}
	}

	// ── Store accessors ───────────────────────────────────────────────────

	fn with_state<F>(&self, f: F) -> Result<serde_json::Value, AdaptiveError>
	where
		F: FnOnce(&Store) -> Result<serde_json::Value, AdaptiveError>,
	{
		match &self.store {
			Some(s) => f(s),
			None => Err(AdaptiveError::NotInitialized),
		}
	}

	fn with_state_transition<F>(&mut self, f: F) -> Result<serde_json::Value, AdaptiveError>
	where
		F: FnOnce(Store) -> Result<(Store, serde_json::Value), AdaptiveError>,
	{
		let state = self.store.take().ok_or(AdaptiveError::NotInitialized)?;
		let backup = state.clone();
		match f(state) {
			Ok((new_state, value)) => {
				self.store = Some(new_state);
				Ok(value)
			}
			Err(e) => {
				self.store = Some(backup);
				Err(e)
			}
		}
	}

	// ── Initialize ────────────────────────────────────────────────────────

	fn handle_initialize(
		&mut self,
		params: serde_json::Value,
	) -> Result<serde_json::Value, AdaptiveError> {
		let p: InitializeParams = parse_params(params)?;

		let duplicate_behavior = match p.duplicate_behavior.as_deref() {
			Some("skip") => DuplicateBehavior::Skip,
			Some("warn") => DuplicateBehavior::Warn,
			Some("error") => DuplicateBehavior::Error,
			_ => DuplicateBehavior::default(),
		};

		let config = StoreConfig {
			storage_path: p.storage_path.clone(),
			duplicate_threshold: p.duplicate_threshold.unwrap_or(0.95),
			duplicate_behavior,
			max_regex_pattern_length: p.max_regex_pattern_length.unwrap_or(500),
			learning_enabled: p.learning_enabled.unwrap_or(false),
			learning_options: Default::default(),
			recency_half_life_ms: p
				.recency_half_life_ms
				.unwrap_or(30.0 * 24.0 * 60.0 * 60.0 * 1000.0),
			topic_catalog_threshold: p.topic_catalog_threshold.unwrap_or(0.85),
			graph_config: Default::default(),
		};

		let store = Store::new(config);
		let store = store.initialize(p.storage_path.as_deref())?;
		self.store = Some(store);

		Ok(serde_json::json!({}))
	}

	// ── PCN handlers ──────────────────────────────────────────────────────

	fn handle_pcn_initialize(
		&mut self,
		params: serde_json::Value,
	) -> Result<serde_json::Value, AdaptiveError> {
		let p: PcnInitializeParams = parse_params(params)?;

		let config = p.config;
		let embedding_dim = p.embedding_dim;

		// Drop old event sender first to signal the existing training worker
		// to shut down. Without this, re-initializing the PCN would leak the
		// old worker task (it would block forever on the now-orphaned channel).
		self.event_tx = None;

		let snapshot = Arc::new(RwLock::new(ModelSnapshot::empty()));
		let (tx, rx) = mpsc::channel::<InputEvent>(config.channel_capacity);

		let worker_snapshot = snapshot.clone();
		let worker_config = config.clone();
		tokio::spawn(async move {
			let stats =
				TrainingWorker::run_batch(rx, worker_snapshot, worker_config, embedding_dim).await;
			info!(
				epochs = stats.epochs,
				total_samples = stats.total_samples,
				"Training worker exited"
			);
		});

		let predictor = Predictor::new(snapshot.clone(), config.inference_steps);

		self.snapshot = snapshot;
		self.predictor = Some(predictor);
		self.event_tx = Some(tx);
		self.pcn_config = Some(config);
		self.embedding_dim = embedding_dim;
		self.pcn_initialized = true;

		Ok(serde_json::json!({ "ok": true }))
	}

	fn handle_pcn_dispose(&mut self) -> Result<serde_json::Value, AdaptiveError> {
		self.event_tx = None;
		self.predictor = None;
		self.pcn_initialized = false;
		self.pcn_config = None;
		Ok(serde_json::json!({ "ok": true }))
	}

	fn handle_pcn_health(&self) -> Result<serde_json::Value, AdaptiveError> {
		Ok(serde_json::json!({
			"initialized": self.pcn_initialized,
			"embeddingDim": self.embedding_dim,
			"hasPredictor": self.predictor.is_some(),
			"hasEventChannel": self.event_tx.is_some(),
		}))
	}

	fn handle_feed_event(
		&self,
		params: serde_json::Value,
	) -> Result<serde_json::Value, AdaptiveError> {
		self.require_pcn_initialized()?;
		let p: FeedEventParams = parse_params(params)?;
		let tx = self.event_tx.as_ref().unwrap();
		match tx.try_send(p.event) {
			Ok(()) => Ok(serde_json::json!({ "queued": true })),
			Err(mpsc::error::TrySendError::Full(_)) => {
				Ok(serde_json::json!({ "queued": false, "reason": "channel_full" }))
			}
			Err(mpsc::error::TrySendError::Closed(_)) => {
				Ok(serde_json::json!({ "queued": false, "reason": "channel_closed" }))
			}
		}
	}

	fn handle_predict_confidence(
		&self,
		params: serde_json::Value,
	) -> Result<serde_json::Value, AdaptiveError> {
		self.require_pcn_initialized()?;
		let p: PredictConfidenceParams = parse_params(params)?;
		let predictor = self.predictor.as_ref().unwrap();
		match predictor.confidence(&p.input) {
			Some(result) => Ok(serde_json::json!({
				"energy": result.energy,
				"topLatent": result.top_latent,
				"energyBreakdown": result.energy_breakdown,
				"reconstruction": result.reconstruction,
			})),
			None => Ok(serde_json::json!({
				"energy": null,
				"reason": "no_trained_model",
			})),
		}
	}

	fn handle_predict_anomalies(
		&self,
		params: serde_json::Value,
	) -> Result<serde_json::Value, AdaptiveError> {
		self.require_pcn_initialized()?;
		let p: PredictAnomaliesParams = parse_params(params)?;
		let predictor = self.predictor.as_ref().unwrap();
		let anomalies = predictor.anomalies(&p.inputs, p.top_k);
		let results: Vec<serde_json::Value> = anomalies
			.into_iter()
			.map(|(index, energy)| serde_json::json!({ "index": index, "energy": energy }))
			.collect();
		Ok(serde_json::json!({ "anomalies": results }))
	}

	fn handle_model_stats(&self) -> Result<serde_json::Value, AdaptiveError> {
		self.require_pcn_initialized()?;
		let predictor = self.predictor.as_ref().unwrap();
		let stats = predictor.model_stats();
		Ok(serde_json::json!({
			"epoch": stats.epoch,
			"totalSamples": stats.total_samples,
			"numLayers": stats.num_layers,
			"inputDim": stats.input_dim,
			"layerDims": stats.layer_dims,
			"parameterCount": stats.parameter_count,
		}))
	}

	fn handle_model_snapshot(
		&self,
		params: serde_json::Value,
	) -> Result<serde_json::Value, AdaptiveError> {
		self.require_pcn_initialized()?;
		let p: ModelSnapshotParams = parse_params(params)?;
		let snap = self.snapshot.read().unwrap();
		save_snapshot(&snap, &p.path, p.compress)?;
		Ok(serde_json::json!({
			"ok": true,
			"path": p.path,
			"compressed": p.compress,
		}))
	}

	fn handle_model_restore(
		&mut self,
		params: serde_json::Value,
	) -> Result<serde_json::Value, AdaptiveError> {
		self.require_pcn_initialized()?;
		let p: ModelRestoreParams = parse_params(params)?;
		let loaded = load_snapshot(&p.path, p.compressed)?;
		let mut snap = self.snapshot.write().unwrap();
		*snap = loaded;
		Ok(serde_json::json!({
			"ok": true,
			"path": p.path,
			"epoch": snap.epoch,
			"totalSamples": snap.total_samples,
		}))
	}

	fn handle_model_reset(&self) -> Result<serde_json::Value, AdaptiveError> {
		self.require_pcn_initialized()?;
		let mut snap = self.snapshot.write().unwrap();
		*snap = ModelSnapshot::empty();
		Ok(serde_json::json!({ "ok": true }))
	}

	fn require_pcn_initialized(&self) -> Result<(), AdaptiveError> {
		if !self.pcn_initialized {
			return Err(AdaptiveError::NotInitialized);
		}
		Ok(())
	}
}

// ---------------------------------------------------------------------------
// Param types
// ---------------------------------------------------------------------------

fn parse_params<T: serde::de::DeserializeOwned>(
	params: serde_json::Value,
) -> Result<T, AdaptiveError> {
	serde_json::from_value(params)
		.map_err(|e| AdaptiveError::InvalidParams(e.to_string()))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InitializeParams {
	storage_path: Option<String>,
	duplicate_threshold: Option<f64>,
	duplicate_behavior: Option<String>,
	max_regex_pattern_length: Option<usize>,
	learning_enabled: Option<bool>,
	recency_half_life_ms: Option<f64>,
	topic_catalog_threshold: Option<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddParams {
	text: String,
	embedding: Vec<f32>,
	metadata: Option<HashMap<String, String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddBatchEntry {
	text: String,
	embedding: Vec<f32>,
	metadata: Option<HashMap<String, String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddBatchParams {
	entries: Vec<AddBatchEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdParams {
	id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdsParams {
	ids: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchParams {
	query_embedding: Vec<f32>,
	max_results: Option<usize>,
	threshold: Option<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FilterByTopicParams {
	topics: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckDuplicateParams {
	embedding: Vec<f32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FindDuplicatesParams {
	threshold: Option<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogResolveParams {
	topic: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogRelocateParams {
	entry_id: String,
	new_topic: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogMergeParams {
	source: String,
	target: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogEntriesParams {
	topic: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordQueryParams {
	embedding: Vec<f32>,
	selected_ids: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordFeedbackParams {
	entry_id: String,
	relevant: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueryParseParams {
	dsl: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphNeighborsParams {
	id: String,
	edge_types: Option<Vec<String>>,
	max_results: Option<usize>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphTraverseParams {
	id: String,
	depth: Option<usize>,
	edge_types: Option<Vec<String>>,
	max_results: Option<usize>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryContextLookup {
	entry: serde_json::Value,
	score: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryContextOptions {
	max_results: Option<usize>,
	min_score: Option<f64>,
	format: Option<String>,
	tag: Option<String>,
	max_chars: Option<usize>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryContextParams {
	lookups: Vec<MemoryContextLookup>,
	options: Option<MemoryContextOptions>,
}

// ---------------------------------------------------------------------------
// Free-standing handler functions
// ---------------------------------------------------------------------------

fn handle_add(
	store: Store,
	params: serde_json::Value,
) -> Result<(Store, serde_json::Value), AdaptiveError> {
	let p: AddParams = parse_params(params)?;
	let (store, id) = store.add(p.text, p.embedding, p.metadata.unwrap_or_default())?;
	Ok((store, serde_json::json!({ "id": id })))
}

fn handle_add_batch(
	store: Store,
	params: serde_json::Value,
) -> Result<(Store, serde_json::Value), AdaptiveError> {
	let p: AddBatchParams = parse_params(params)?;
	let entries: Vec<AddEntry> = p
		.entries
		.into_iter()
		.map(|e| AddEntry {
			text: e.text,
			embedding: e.embedding,
			metadata: e.metadata.unwrap_or_default(),
		})
		.collect();
	let (store, ids) = store.add_batch(entries)?;
	Ok((store, serde_json::json!({ "ids": ids })))
}

fn handle_delete(
	store: Store,
	params: serde_json::Value,
) -> Result<(Store, serde_json::Value), AdaptiveError> {
	let p: IdParams = parse_params(params)?;
	let (store, deleted) = store.delete(&p.id);
	Ok((store, serde_json::json!({ "deleted": deleted })))
}

fn handle_delete_batch(
	store: Store,
	params: serde_json::Value,
) -> Result<(Store, serde_json::Value), AdaptiveError> {
	let p: IdsParams = parse_params(params)?;
	let (store, count) = store.delete_batch(&p.ids);
	Ok((store, serde_json::json!({ "count": count })))
}

fn handle_get_by_id(
	store: Store,
	params: serde_json::Value,
) -> Result<(Store, serde_json::Value), AdaptiveError> {
	let p: IdParams = parse_params(params)?;
	let (store, volume) = store.get_by_id(&p.id);
	Ok((store, serde_json::json!({ "entry": volume })))
}

fn handle_search(
	store: Store,
	params: serde_json::Value,
) -> Result<(Store, serde_json::Value), AdaptiveError> {
	let p: SearchParams = parse_params(params)?;
	let (store, results) = store.search(&p.query_embedding, p.max_results.unwrap_or(10), p.threshold.unwrap_or(0.0))?;
	Ok((store, serde_json::json!({ "results": results })))
}

fn handle_text_search(
	store: &Store,
	params: serde_json::Value,
) -> Result<serde_json::Value, AdaptiveError> {
	let options: crate::types::TextSearchOptions = parse_params(params)?;
	let results = store.text_search(&options)?;
	Ok(serde_json::json!({ "results": results }))
}

fn handle_advanced_search(
	store: Store,
	params: serde_json::Value,
) -> Result<(Store, serde_json::Value), AdaptiveError> {
	let options: crate::types::SearchOptions = parse_params(params)?;
	let (store, results) = store.advanced_search(&options)?;
	Ok((store, serde_json::json!({ "results": results })))
}

fn handle_filter_by_metadata(
	store: &Store,
	params: serde_json::Value,
) -> Result<serde_json::Value, AdaptiveError> {
	#[derive(Deserialize)]
	struct P {
		filters: Vec<crate::types::MetadataFilter>,
	}
	let p: P = parse_params(params)?;
	let volumes = store.filter_by_metadata(&p.filters);
	Ok(serde_json::json!({ "entries": volumes }))
}

fn handle_filter_by_date_range(
	store: &Store,
	params: serde_json::Value,
) -> Result<serde_json::Value, AdaptiveError> {
	let range: crate::types::DateRange = parse_params(params)?;
	let volumes = store.filter_by_date_range(&range);
	Ok(serde_json::json!({ "entries": volumes }))
}

fn handle_filter_by_topic(
	store: &Store,
	params: serde_json::Value,
) -> Result<serde_json::Value, AdaptiveError> {
	let p: FilterByTopicParams = parse_params(params)?;
	let volumes = store.filter_by_topic(&p.topics);
	Ok(serde_json::json!({ "entries": volumes }))
}

fn handle_recommend(
	store: &Store,
	params: serde_json::Value,
) -> Result<serde_json::Value, AdaptiveError> {
	let options: crate::types::RecommendOptions = parse_params(params)?;
	let results = store.recommend(&options)?;
	Ok(serde_json::json!({ "results": results }))
}

fn handle_check_duplicate(
	store: &Store,
	params: serde_json::Value,
) -> Result<serde_json::Value, AdaptiveError> {
	let p: CheckDuplicateParams = parse_params(params)?;
	// check_duplicate doesn't take a threshold param, it uses the store's config
	let result = store.check_duplicate(&p.embedding);
	serde_json::to_value(result)
		.map_err(|e| AdaptiveError::Serialization(e.to_string()))
}

fn handle_find_duplicates(
	store: &Store,
	params: serde_json::Value,
) -> Result<serde_json::Value, AdaptiveError> {
	let p: FindDuplicatesParams = parse_params(params)?;
	let groups = store.find_duplicates(p.threshold);
	Ok(serde_json::json!({ "groups": groups }))
}

fn handle_catalog_resolve(
	store: Store,
	params: serde_json::Value,
) -> Result<(Store, serde_json::Value), AdaptiveError> {
	let p: CatalogResolveParams = parse_params(params)?;
	let (store, resolved) = store.catalog_resolve(&p.topic);
	Ok((store, serde_json::json!({ "resolved": resolved })))
}

fn handle_catalog_relocate(
	store: Store,
	params: serde_json::Value,
) -> Result<(Store, serde_json::Value), AdaptiveError> {
	let p: CatalogRelocateParams = parse_params(params)?;
	let store = store.catalog_relocate(&p.entry_id, &p.new_topic);
	Ok((store, serde_json::json!({})))
}

fn handle_catalog_merge(
	store: Store,
	params: serde_json::Value,
) -> Result<(Store, serde_json::Value), AdaptiveError> {
	let p: CatalogMergeParams = parse_params(params)?;
	let store = store.catalog_merge(&p.source, &p.target);
	Ok((store, serde_json::json!({})))
}

fn handle_catalog_entries(
	store: &Store,
	params: serde_json::Value,
) -> Result<serde_json::Value, AdaptiveError> {
	let p: CatalogEntriesParams = parse_params(params)?;
	let entry_ids = store.catalog_entries(&p.topic);
	Ok(serde_json::json!({ "entryIds": entry_ids }))
}

fn handle_record_query(
	store: Store,
	params: serde_json::Value,
) -> Result<(Store, serde_json::Value), AdaptiveError> {
	let p: RecordQueryParams = parse_params(params)?;
	let store = store.record_query(&p.embedding, &p.selected_ids);
	Ok((store, serde_json::json!({})))
}

fn handle_record_feedback(
	store: Store,
	params: serde_json::Value,
) -> Result<(Store, serde_json::Value), AdaptiveError> {
	let p: RecordFeedbackParams = parse_params(params)?;
	let store = store.record_feedback(&p.entry_id, p.relevant);
	Ok((store, serde_json::json!({})))
}

fn handle_query_parse(params: serde_json::Value) -> Result<serde_json::Value, AdaptiveError> {
	let p: QueryParseParams = parse_params(params)?;
	let parsed = parse_query(&p.dsl);
	serde_json::to_value(parsed).map_err(|e| AdaptiveError::Serialization(e.to_string()))
}

fn handle_format_context(
	params: serde_json::Value,
) -> Result<serde_json::Value, AdaptiveError> {
	let p: MemoryContextParams = parse_params(params)?;

	// Convert the lookup JSON values into Lookup structs
	let lookups: Vec<Lookup> = p
		.lookups
		.into_iter()
		.map(|l| {
			let parsed = serde_json::from_value(l.entry)
				.map_err(|e| AdaptiveError::Serialization(format!("Invalid entry: {}", e)));
			parsed.map(|v| Lookup {
				entry: v,
				score: l.score,
			})
		})
		.collect::<Result<Vec<_>, _>>()?;

	let options = match p.options {
		Some(opts) => ContextFormatOptions {
			max_results: opts.max_results,
			min_score: opts.min_score,
			format: opts.format,
			tag: opts.tag,
			max_chars: opts.max_chars,
		},
		None => ContextFormatOptions::default(),
	};

	let now = SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.unwrap_or_default()
		.as_millis() as u64;

	let text = format_context(&lookups, &options, now);
	Ok(serde_json::json!({ "text": text }))
}

// ---------------------------------------------------------------------------
// Graph handlers
// ---------------------------------------------------------------------------

fn parse_edge_types(raw: &[String]) -> Vec<crate::graph::EdgeType> {
	raw.iter()
		.filter_map(|s| match s.as_str() {
			"Related" => Some(crate::graph::EdgeType::Related),
			"Parent" => Some(crate::graph::EdgeType::Parent),
			"Child" => Some(crate::graph::EdgeType::Child),
			"Extends" => Some(crate::graph::EdgeType::Extends),
			"Contradicts" => Some(crate::graph::EdgeType::Contradicts),
			"Similar" => Some(crate::graph::EdgeType::Similar),
			"CoOccurs" => Some(crate::graph::EdgeType::CoOccurs),
			_ => None,
		})
		.collect()
}

fn handle_graph_neighbors(
	store: &Store,
	params: serde_json::Value,
) -> Result<serde_json::Value, AdaptiveError> {
	let p: GraphNeighborsParams = parse_params(params)?;
	let edge_types = p.edge_types.as_ref().map(|et| parse_edge_types(et));
	let max = p.max_results.unwrap_or(20);
	let results = store.graph_neighbors(&p.id, edge_types.as_deref(), max);
	let neighbors: Vec<serde_json::Value> = results
		.iter()
		.map(|(edge, vol)| {
			serde_json::json!({
				"entry": vol,
				"edge": {
					"edgeType": format!("{:?}", edge.edge_type),
					"weight": edge.weight,
					"origin": format!("{:?}", edge.origin),
				}
			})
		})
		.collect();
	Ok(serde_json::json!({ "neighbors": neighbors }))
}

fn handle_graph_traverse(
	store: &Store,
	params: serde_json::Value,
) -> Result<serde_json::Value, AdaptiveError> {
	let p: GraphTraverseParams = parse_params(params)?;
	let edge_types = p.edge_types.as_ref().map(|et| parse_edge_types(et));
	let depth = p.depth.unwrap_or(1).min(2); // Cap at 2 hops
	let max = p.max_results.unwrap_or(50);
	let results = store.graph_traverse(&p.id, depth, edge_types.as_deref(), max);
	let nodes: Vec<serde_json::Value> = results
		.iter()
		.map(|(node, vol)| {
			serde_json::json!({
				"entry": vol,
				"depth": node.depth,
				"path": node.path,
			})
		})
		.collect();
	Ok(serde_json::json!({ "nodes": nodes }))
}
