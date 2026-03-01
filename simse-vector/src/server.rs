// ---------------------------------------------------------------------------
// VectorServer — JSON-RPC dispatcher
// ---------------------------------------------------------------------------
//
// Routes incoming JSON-RPC 2.0 requests (NDJSON over stdin) to VolumeStore
// operations.  Follows the same pattern as the VFS server: a main `run()` loop,
// a `dispatch()` match, `with_store` / `with_store_mut` helpers, and
// free-standing handler functions for each method.
// ---------------------------------------------------------------------------

use std::collections::HashMap;
use std::io::{self, BufRead};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;

use crate::error::VectorError;
use crate::prompt_injection::{format_memory_context, PromptInjectionOptions};
use crate::protocol::*;
use crate::query_dsl::parse_query;
use crate::store::{AddEntry, DuplicateBehavior, StoreConfig, VolumeStore};
use crate::transport::NdjsonTransport;
use crate::types::Lookup;

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

/// JSON-RPC server that dispatches requests to a [`VolumeStore`].
pub struct VectorServer {
	transport: NdjsonTransport,
	store: Option<VolumeStore>,
}

impl VectorServer {
	/// Create a new server with the given transport.  The store is created
	/// lazily when `store/initialize` is called.
	pub fn new(transport: NdjsonTransport) -> Self {
		Self {
			transport,
			store: None,
		}
	}

	/// Main loop: read JSON-RPC messages from stdin, dispatch to handlers.
	pub fn run(&mut self) -> Result<(), VectorError> {
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

			self.dispatch(request);
		}

		Ok(())
	}

	// ── Dispatch ──────────────────────────────────────────────────────────

	fn dispatch(&mut self, req: JsonRpcRequest) {
		let id = req.id;
		let result = match req.method.as_str() {
			// -- Lifecycle -----------------------------------------------
			"store/initialize" => self.handle_initialize(req.params),
			"store/dispose" => self.with_store_mut(|s| {
				s.dispose()?;
				Ok(serde_json::json!({}))
			}),
			"store/save" => self.with_store_mut(|s| {
				s.save()?;
				Ok(serde_json::json!({}))
			}),

			// -- CRUD ----------------------------------------------------
			"store/add" => self.with_store_mut(|s| handle_add(s, req.params)),
			"store/addBatch" => self.with_store_mut(|s| handle_add_batch(s, req.params)),
			"store/delete" => self.with_store_mut(|s| handle_delete(s, req.params)),
			"store/deleteBatch" => self.with_store_mut(|s| handle_delete_batch(s, req.params)),
			"store/clear" => self.with_store_mut(|s| {
				s.clear();
				Ok(serde_json::json!({}))
			}),
			"store/getById" => self.with_store_mut(|s| handle_get_by_id(s, req.params)),
			"store/getAll" => self.with_store(|s| {
				let volumes = s.get_all();
				Ok(serde_json::json!({ "volumes": volumes }))
			}),

			// -- Search --------------------------------------------------
			"store/search" => self.with_store_mut(|s| handle_search(s, req.params)),
			"store/textSearch" => self.with_store(|s| handle_text_search(s, req.params)),
			"store/advancedSearch" => {
				self.with_store_mut(|s| handle_advanced_search(s, req.params))
			}
			"store/filterByMetadata" => {
				self.with_store(|s| handle_filter_by_metadata(s, req.params))
			}
			"store/filterByDateRange" => {
				self.with_store(|s| handle_filter_by_date_range(s, req.params))
			}
			"store/filterByTopic" => self.with_store(|s| handle_filter_by_topic(s, req.params)),
			"store/getTopics" => self.with_store(|s| {
				let topics = s.get_topics();
				Ok(serde_json::json!({ "topics": topics }))
			}),

			// -- Recommendation ------------------------------------------
			"store/recommend" => self.with_store(|s| handle_recommend(s, req.params)),

			// -- Deduplication -------------------------------------------
			"store/checkDuplicate" => {
				self.with_store(|s| handle_check_duplicate(s, req.params))
			}
			"store/findDuplicates" => {
				self.with_store(|s| handle_find_duplicates(s, req.params))
			}

			// -- Size / Dirty --------------------------------------------
			"store/size" => self.with_store(|s| Ok(serde_json::json!({ "count": s.size() }))),
			"store/isDirty" => {
				self.with_store(|s| Ok(serde_json::json!({ "dirty": s.is_dirty() })))
			}

			// -- Catalog -------------------------------------------------
			"catalog/resolve" => self.with_store_mut(|s| handle_catalog_resolve(s, req.params)),
			"catalog/relocate" => {
				self.with_store_mut(|s| handle_catalog_relocate(s, req.params))
			}
			"catalog/merge" => self.with_store_mut(|s| handle_catalog_merge(s, req.params)),
			"catalog/sections" => self.with_store(|s| {
				let sections = s.catalog_sections();
				Ok(serde_json::json!({ "sections": sections }))
			}),
			"catalog/volumes" => self.with_store(|s| handle_catalog_volumes(s, req.params)),

			// -- Learning ------------------------------------------------
			"learning/recordQuery" => {
				self.with_store_mut(|s| handle_record_query(s, req.params))
			}
			"learning/recordFeedback" => {
				self.with_store_mut(|s| handle_record_feedback(s, req.params))
			}
			"learning/profile" => self.with_store(|s| {
				let profile = s.get_profile();
				Ok(serde_json::json!({ "profile": profile }))
			}),

			// -- Query DSL -----------------------------------------------
			"query/parse" => handle_query_parse(req.params),

			// -- Prompt injection ----------------------------------------
			"format/memoryContext" => handle_format_memory_context(req.params),

			// -- Graph ---------------------------------------------------
			"graph/neighbors" => self.with_store(|s| handle_graph_neighbors(s, req.params)),
			"graph/traverse" => self.with_store(|s| handle_graph_traverse(s, req.params)),

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
				VECTOR_ERROR,
				e.to_string(),
				Some(e.to_json_rpc_error()),
			),
		}
	}

	// ── Store accessors ───────────────────────────────────────────────────

	fn with_store<F>(&self, f: F) -> Result<serde_json::Value, VectorError>
	where
		F: FnOnce(&VolumeStore) -> Result<serde_json::Value, VectorError>,
	{
		match &self.store {
			Some(s) => f(s),
			None => Err(VectorError::NotInitialized),
		}
	}

	fn with_store_mut<F>(&mut self, f: F) -> Result<serde_json::Value, VectorError>
	where
		F: FnOnce(&mut VolumeStore) -> Result<serde_json::Value, VectorError>,
	{
		match &mut self.store {
			Some(s) => f(s),
			None => Err(VectorError::NotInitialized),
		}
	}

	// ── Initialize ────────────────────────────────────────────────────────

	fn handle_initialize(
		&mut self,
		params: serde_json::Value,
	) -> Result<serde_json::Value, VectorError> {
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

		let mut store = VolumeStore::new(config);
		store.initialize(p.storage_path.as_deref())?;
		self.store = Some(store);

		Ok(serde_json::json!({}))
	}
}

// ---------------------------------------------------------------------------
// Param types
// ---------------------------------------------------------------------------

fn parse_params<T: serde::de::DeserializeOwned>(
	params: serde_json::Value,
) -> Result<T, VectorError> {
	serde_json::from_value(params)
		.map_err(|e| VectorError::Serialization(format!("Invalid params: {}", e)))
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
#[allow(dead_code)]
struct CheckDuplicateParams {
	embedding: Vec<f32>,
	threshold: Option<f64>,
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
	volume_id: String,
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
struct CatalogVolumesParams {
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
	volume: serde_json::Value,
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
	store: &mut VolumeStore,
	params: serde_json::Value,
) -> Result<serde_json::Value, VectorError> {
	let p: AddParams = parse_params(params)?;
	let id = store.add(p.text, p.embedding, p.metadata.unwrap_or_default())?;
	Ok(serde_json::json!({ "id": id }))
}

fn handle_add_batch(
	store: &mut VolumeStore,
	params: serde_json::Value,
) -> Result<serde_json::Value, VectorError> {
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
	let ids = store.add_batch(entries)?;
	Ok(serde_json::json!({ "ids": ids }))
}

fn handle_delete(
	store: &mut VolumeStore,
	params: serde_json::Value,
) -> Result<serde_json::Value, VectorError> {
	let p: IdParams = parse_params(params)?;
	let deleted = store.delete(&p.id);
	Ok(serde_json::json!({ "deleted": deleted }))
}

fn handle_delete_batch(
	store: &mut VolumeStore,
	params: serde_json::Value,
) -> Result<serde_json::Value, VectorError> {
	let p: IdsParams = parse_params(params)?;
	let count = store.delete_batch(&p.ids);
	Ok(serde_json::json!({ "count": count }))
}

fn handle_get_by_id(
	store: &mut VolumeStore,
	params: serde_json::Value,
) -> Result<serde_json::Value, VectorError> {
	let p: IdParams = parse_params(params)?;
	let volume = store.get_by_id(&p.id);
	Ok(serde_json::json!({ "volume": volume }))
}

fn handle_search(
	store: &mut VolumeStore,
	params: serde_json::Value,
) -> Result<serde_json::Value, VectorError> {
	let p: SearchParams = parse_params(params)?;
	let results = store.search(&p.query_embedding, p.max_results.unwrap_or(10), p.threshold.unwrap_or(0.0))?;
	Ok(serde_json::json!({ "results": results }))
}

fn handle_text_search(
	store: &VolumeStore,
	params: serde_json::Value,
) -> Result<serde_json::Value, VectorError> {
	let options: crate::types::TextSearchOptions = parse_params(params)?;
	let results = store.text_search(&options)?;
	Ok(serde_json::json!({ "results": results }))
}

fn handle_advanced_search(
	store: &mut VolumeStore,
	params: serde_json::Value,
) -> Result<serde_json::Value, VectorError> {
	let options: crate::types::SearchOptions = parse_params(params)?;
	let results = store.advanced_search(&options)?;
	Ok(serde_json::json!({ "results": results }))
}

fn handle_filter_by_metadata(
	store: &VolumeStore,
	params: serde_json::Value,
) -> Result<serde_json::Value, VectorError> {
	#[derive(Deserialize)]
	struct P {
		filters: Vec<crate::types::MetadataFilter>,
	}
	let p: P = parse_params(params)?;
	let volumes = store.filter_by_metadata(&p.filters);
	Ok(serde_json::json!({ "volumes": volumes }))
}

fn handle_filter_by_date_range(
	store: &VolumeStore,
	params: serde_json::Value,
) -> Result<serde_json::Value, VectorError> {
	let range: crate::types::DateRange = parse_params(params)?;
	let volumes = store.filter_by_date_range(&range);
	Ok(serde_json::json!({ "volumes": volumes }))
}

fn handle_filter_by_topic(
	store: &VolumeStore,
	params: serde_json::Value,
) -> Result<serde_json::Value, VectorError> {
	let p: FilterByTopicParams = parse_params(params)?;
	let volumes = store.filter_by_topic(&p.topics);
	Ok(serde_json::json!({ "volumes": volumes }))
}

fn handle_recommend(
	store: &VolumeStore,
	params: serde_json::Value,
) -> Result<serde_json::Value, VectorError> {
	let options: crate::types::RecommendOptions = parse_params(params)?;
	let results = store.recommend(&options)?;
	Ok(serde_json::json!({ "results": results }))
}

fn handle_check_duplicate(
	store: &VolumeStore,
	params: serde_json::Value,
) -> Result<serde_json::Value, VectorError> {
	let p: CheckDuplicateParams = parse_params(params)?;
	// check_duplicate doesn't take a threshold param, it uses the store's config
	let result = store.check_duplicate(&p.embedding);
	Ok(serde_json::to_value(result)
		.map_err(|e| VectorError::Serialization(e.to_string()))?)
}

fn handle_find_duplicates(
	store: &VolumeStore,
	params: serde_json::Value,
) -> Result<serde_json::Value, VectorError> {
	let p: FindDuplicatesParams = parse_params(params)?;
	let groups = store.find_duplicates(p.threshold);
	Ok(serde_json::json!({ "groups": groups }))
}

fn handle_catalog_resolve(
	store: &mut VolumeStore,
	params: serde_json::Value,
) -> Result<serde_json::Value, VectorError> {
	let p: CatalogResolveParams = parse_params(params)?;
	let resolved = store.catalog_resolve(&p.topic);
	Ok(serde_json::json!({ "resolved": resolved }))
}

fn handle_catalog_relocate(
	store: &mut VolumeStore,
	params: serde_json::Value,
) -> Result<serde_json::Value, VectorError> {
	let p: CatalogRelocateParams = parse_params(params)?;
	store.catalog_relocate(&p.volume_id, &p.new_topic);
	Ok(serde_json::json!({}))
}

fn handle_catalog_merge(
	store: &mut VolumeStore,
	params: serde_json::Value,
) -> Result<serde_json::Value, VectorError> {
	let p: CatalogMergeParams = parse_params(params)?;
	store.catalog_merge(&p.source, &p.target);
	Ok(serde_json::json!({}))
}

fn handle_catalog_volumes(
	store: &VolumeStore,
	params: serde_json::Value,
) -> Result<serde_json::Value, VectorError> {
	let p: CatalogVolumesParams = parse_params(params)?;
	let volume_ids = store.catalog_volumes(&p.topic);
	Ok(serde_json::json!({ "volumeIds": volume_ids }))
}

fn handle_record_query(
	store: &mut VolumeStore,
	params: serde_json::Value,
) -> Result<serde_json::Value, VectorError> {
	let p: RecordQueryParams = parse_params(params)?;
	store.record_query(&p.embedding, &p.selected_ids);
	Ok(serde_json::json!({}))
}

fn handle_record_feedback(
	store: &mut VolumeStore,
	params: serde_json::Value,
) -> Result<serde_json::Value, VectorError> {
	let p: RecordFeedbackParams = parse_params(params)?;
	store.record_feedback(&p.entry_id, p.relevant);
	Ok(serde_json::json!({}))
}

fn handle_query_parse(params: serde_json::Value) -> Result<serde_json::Value, VectorError> {
	let p: QueryParseParams = parse_params(params)?;
	let parsed = parse_query(&p.dsl);
	serde_json::to_value(parsed).map_err(|e| VectorError::Serialization(e.to_string()))
}

fn handle_format_memory_context(
	params: serde_json::Value,
) -> Result<serde_json::Value, VectorError> {
	let p: MemoryContextParams = parse_params(params)?;

	// Convert the lookup JSON values into Lookup structs
	let lookups: Vec<Lookup> = p
		.lookups
		.into_iter()
		.map(|l| {
			let volume = serde_json::from_value(l.volume)
				.map_err(|e| VectorError::Serialization(format!("Invalid volume: {}", e)));
			volume.map(|v| Lookup {
				volume: v,
				score: l.score,
			})
		})
		.collect::<Result<Vec<_>, _>>()?;

	let options = match p.options {
		Some(opts) => PromptInjectionOptions {
			max_results: opts.max_results,
			min_score: opts.min_score,
			format: opts.format,
			tag: opts.tag,
			max_chars: opts.max_chars,
		},
		None => PromptInjectionOptions::default(),
	};

	let now = SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.unwrap_or_default()
		.as_millis() as u64;

	let text = format_memory_context(&lookups, &options, now);
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
	store: &VolumeStore,
	params: serde_json::Value,
) -> Result<serde_json::Value, VectorError> {
	let p: GraphNeighborsParams = parse_params(params)?;
	let edge_types = p.edge_types.as_ref().map(|et| parse_edge_types(et));
	let max = p.max_results.unwrap_or(20);
	let results = store.graph_neighbors(&p.id, edge_types.as_deref(), max);
	let neighbors: Vec<serde_json::Value> = results
		.iter()
		.map(|(edge, vol)| {
			serde_json::json!({
				"volume": vol,
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
	store: &VolumeStore,
	params: serde_json::Value,
) -> Result<serde_json::Value, VectorError> {
	let p: GraphTraverseParams = parse_params(params)?;
	let edge_types = p.edge_types.as_ref().map(|et| parse_edge_types(et));
	let depth = p.depth.unwrap_or(1).min(2); // Cap at 2 hops
	let max = p.max_results.unwrap_or(50);
	let results = store.graph_traverse(&p.id, depth, edge_types.as_deref(), max);
	let nodes: Vec<serde_json::Value> = results
		.iter()
		.map(|(node, vol)| {
			serde_json::json!({
				"volume": vol,
				"depth": node.depth,
				"path": node.path,
			})
		})
		.collect();
	Ok(serde_json::json!({ "nodes": nodes }))
}
