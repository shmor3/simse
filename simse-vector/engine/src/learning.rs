// ---------------------------------------------------------------------------
// Adaptive Learning Engine
// ---------------------------------------------------------------------------
//
// Observes search patterns and adapts the memory system in real time:
//
// 1. Relevance feedback — tracks which entries are retrieved by diverse
//    queries, boosting consistently-relevant entries.
// 2. Adaptive weight profiles — shifts vector/recency/frequency weights
//    based on which signals best predict useful results.
// 3. Interest embedding — maintains a decayed average of recent query
//    embeddings representing the user's evolving interests.
// 4. Per-topic profiles — tracks weights, interest embeddings, and query
//    counts independently per topic, falling back to global state when
//    a topic has insufficient data (< 10 queries).
//
// All state is serializable for persistence via base64-encoded Float32 LE
// embeddings.
// ---------------------------------------------------------------------------

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::cosine::cosine_similarity;
use crate::types::{PatronProfile, QueryRecord, RequiredWeightProfile};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const SEVEN_DAYS_MS: f64 = 7.0 * 24.0 * 60.0 * 60.0 * 1000.0;
const MIN_WEIGHT: f64 = 0.05;
const MAX_WEIGHT: f64 = 0.9;
const BOOST_MIN: f64 = 0.8;
const BOOST_MAX: f64 = 1.2;
const TOPIC_QUERY_THRESHOLD: usize = 10;

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Configuration for the adaptive patron learning engine.
pub struct LearningOptions {
	pub enabled: bool,
	pub max_query_history: usize,
	pub query_decay_ms: f64,
	pub weight_adaptation_rate: f64,
	pub interest_boost_weight: f64,
}

impl Default for LearningOptions {
	fn default() -> Self {
		Self {
			enabled: true,
			max_query_history: 50,
			query_decay_ms: SEVEN_DAYS_MS,
			weight_adaptation_rate: 0.05,
			interest_boost_weight: 0.15,
		}
	}
}

// ---------------------------------------------------------------------------
// Internal types
// ---------------------------------------------------------------------------

/// Per-entry implicit feedback tracking.
struct FeedbackEntry {
	query_count: usize,
	total_retrievals: usize,
	last_query_timestamp: u64,
	/// Sample of distinct query embeddings that found this entry (for diversity).
	query_embeddings: Vec<Vec<f32>>,
}

/// Per-entry explicit relevance feedback counts.
struct ExplicitFeedback {
	positive: usize,
	negative: usize,
}

/// Per-topic mutable learning state.
struct TopicState {
	weights: RequiredWeightProfile,
	interest_embedding: Option<Vec<f32>>,
	query_count: usize,
	query_history: Vec<QueryRecord>,
}

// ---------------------------------------------------------------------------
// Serialization types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackSerialized {
	pub id: String,
	#[serde(rename = "queryCount")]
	pub query_count: usize,
	#[serde(rename = "totalRetrievals")]
	pub total_retrievals: usize,
	#[serde(rename = "lastQueryTimestamp")]
	pub last_query_timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedQueryRecord {
	pub embedding: String,
	pub timestamp: u64,
	#[serde(rename = "resultCount")]
	pub result_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplicitFeedbackSerialized {
	#[serde(rename = "entryId")]
	pub entry_id: String,
	#[serde(rename = "positiveCount")]
	pub positive_count: usize,
	#[serde(rename = "negativeCount")]
	pub negative_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicProfileSerialized {
	pub topic: String,
	pub weights: RequiredWeightProfile,
	#[serde(rename = "interestEmbedding")]
	pub interest_embedding: Option<String>,
	#[serde(rename = "queryCount")]
	pub query_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelatedPairSerialized {
	#[serde(rename = "entryId")]
	pub entry_id: String,
	pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationSerialized {
	#[serde(rename = "entryId")]
	pub entry_id: String,
	pub correlated: Vec<CorrelatedPairSerialized>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningState {
	pub version: u32,
	pub feedback: Vec<FeedbackSerialized>,
	#[serde(rename = "queryHistory")]
	pub query_history: Vec<SerializedQueryRecord>,
	#[serde(rename = "adaptedWeights")]
	pub adapted_weights: RequiredWeightProfile,
	#[serde(rename = "interestEmbedding")]
	pub interest_embedding: Option<String>,
	#[serde(rename = "totalQueries")]
	pub total_queries: usize,
	#[serde(rename = "lastUpdated")]
	pub last_updated: u64,
	#[serde(rename = "explicitFeedback")]
	pub explicit_feedback: Option<Vec<ExplicitFeedbackSerialized>>,
	#[serde(rename = "topicProfiles")]
	pub topic_profiles: Option<Vec<TopicProfileSerialized>>,
	pub correlations: Option<Vec<CorrelationSerialized>>,
}

// ---------------------------------------------------------------------------
// Embedding encode / decode helpers
// ---------------------------------------------------------------------------

fn encode_embedding(embedding: &[f32]) -> String {
	let bytes: Vec<u8> = embedding
		.iter()
		.flat_map(|f| f.to_le_bytes())
		.collect();
	STANDARD.encode(&bytes)
}

fn decode_embedding(encoded: &str) -> Option<Vec<f32>> {
	let bytes = STANDARD.decode(encoded).ok()?;
	if bytes.len() % 4 != 0 {
		return None;
	}
	let floats: Vec<f32> = bytes
		.chunks_exact(4)
		.map(|chunk| {
			let arr: [u8; 4] = chunk.try_into().unwrap();
			f32::from_le_bytes(arr)
		})
		.collect();
	Some(floats)
}

// ---------------------------------------------------------------------------
// LearningEngine
// ---------------------------------------------------------------------------

/// Adaptive learning engine that tracks search patterns and adapts
/// weight profiles and interest embeddings over time.
pub struct LearningEngine {
	enabled: bool,
	max_query_history: usize,
	query_decay_ms: f64,
	weight_adaptation_rate: f64,
	interest_boost_weight: f64,

	// Mutable state
	feedback: HashMap<String, FeedbackEntry>,
	explicit_feedback: HashMap<String, ExplicitFeedback>,
	query_history: Vec<QueryRecord>,
	adapted_weights: RequiredWeightProfile,
	interest_embedding: Option<Vec<f32>>,
	total_queries: usize,
	last_updated: u64,
	topic_states: HashMap<String, TopicState>,
	correlations: HashMap<String, HashMap<String, usize>>,
}

// ---------------------------------------------------------------------------
// Free helper functions (avoid borrow-checker issues with &mut self + &self)
// ---------------------------------------------------------------------------

/// Compute an interest embedding from a set of query records using
/// exponential decay weighting. Returns a unit vector.
fn compute_interest_embedding(
	query_decay_ms: f64,
	history: &[QueryRecord],
	now: u64,
) -> Option<Vec<f32>> {
	if history.is_empty() {
		return None;
	}

	let lambda = f64::ln(2.0) / query_decay_ms;
	let dim = history[0].embedding.len();
	if dim == 0 {
		return None;
	}

	let mut weighted = vec![0.0f64; dim];
	let mut total_weight = 0.0f64;

	for record in history {
		if record.embedding.len() != dim {
			continue;
		}
		let age = if now > record.timestamp {
			(now - record.timestamp) as f64
		} else {
			0.0
		};
		let w = (-lambda * age).exp();
		total_weight += w;
		for i in 0..dim {
			weighted[i] += record.embedding[i] as f64 * w;
		}
	}

	if total_weight == 0.0 {
		return None;
	}

	// Divide by total weight
	for val in &mut weighted {
		*val /= total_weight;
	}

	// Compute magnitude
	let mut mag = 0.0f64;
	for val in &weighted {
		mag += val * val;
	}
	mag = mag.sqrt();

	if mag == 0.0 {
		return None;
	}

	// Normalize to unit vector
	let result: Vec<f32> = weighted.iter().map(|v| (v / mag) as f32).collect();
	Some(result)
}

/// Adapt weights based on whether recent results tended to be
/// frequently-accessed entries. Free function to avoid borrow conflicts.
fn adapt_weights(
	feedback: &HashMap<String, FeedbackEntry>,
	weight_adaptation_rate: f64,
	current_weights: &RequiredWeightProfile,
	result_ids: &[String],
) -> RequiredWeightProfile {
	if result_ids.is_empty() {
		return current_weights.clone();
	}

	let mut high_feedback_count = 0usize;
	for id in result_ids {
		if let Some(fb) = feedback.get(id) {
			if fb.total_retrievals > 3 {
				high_feedback_count += 1;
			}
		}
	}

	let ratio = high_feedback_count as f64 / result_ids.len() as f64;
	let new_weights = if ratio > 0.5 {
		RequiredWeightProfile {
			vector: current_weights.vector,
			recency: current_weights.recency,
			frequency: current_weights.frequency + weight_adaptation_rate * 0.5,
		}
	} else {
		RequiredWeightProfile {
			vector: current_weights.vector + weight_adaptation_rate * 0.5,
			recency: current_weights.recency,
			frequency: current_weights.frequency,
		}
	};
	LearningEngine::normalize_weights_required(&new_weights)
}

// ---------------------------------------------------------------------------
// LearningEngine impl
// ---------------------------------------------------------------------------

impl LearningEngine {
	/// Create a new learning engine with the given options.
	pub fn new(options: LearningOptions) -> Self {
		Self {
			enabled: options.enabled,
			max_query_history: options.max_query_history,
			query_decay_ms: options.query_decay_ms,
			weight_adaptation_rate: options.weight_adaptation_rate,
			interest_boost_weight: options.interest_boost_weight,

			feedback: HashMap::new(),
			explicit_feedback: HashMap::new(),
			query_history: Vec::new(),
			adapted_weights: RequiredWeightProfile {
				vector: 0.6,
				recency: 0.2,
				frequency: 0.2,
			},
			interest_embedding: None,
			total_queries: 0,
			last_updated: 0,
			topic_states: HashMap::new(),
			correlations: HashMap::new(),
		}
	}

	// -- Helpers --------------------------------------------------------------

	fn clamp_weight(w: f64) -> f64 {
		w.clamp(MIN_WEIGHT, MAX_WEIGHT)
	}

	fn normalize_weights_required(weights: &RequiredWeightProfile) -> RequiredWeightProfile {
		let v = Self::clamp_weight(weights.vector);
		let r = Self::clamp_weight(weights.recency);
		let f = Self::clamp_weight(weights.frequency);
		let total = v + r + f;
		RequiredWeightProfile {
			vector: v / total,
			recency: r / total,
			frequency: f / total,
		}
	}

	fn normalize_weights_in_place(&mut self) {
		self.adapted_weights = Self::normalize_weights_required(&self.adapted_weights);
	}

	/// Compute the relevance score for an entry based on implicit retrieval
	/// counts and explicit user feedback.
	///
	/// Formula: clamp((queryCount + positive * 5 - negative * 3) / maxScale, 0, 1)
	fn compute_relevance_score(&self, entry_id: &str, fb: &FeedbackEntry) -> f64 {
		let explicit = self.explicit_feedback.get(entry_id);
		let positive = explicit.map_or(0, |e| e.positive);
		let negative = explicit.map_or(0, |e| e.negative);

		let raw_score =
			fb.query_count as f64 + positive as f64 * 5.0 - negative as f64 * 3.0;
		let max_scale = self.max_query_history as f64;
		(raw_score / max_scale).clamp(0.0, 1.0)
	}

	/// Recompute the global interest embedding from query history.
	fn recompute_interest_embedding(&mut self, now: u64) {
		self.interest_embedding =
			compute_interest_embedding(self.query_decay_ms, &self.query_history, now);
	}

	/// Adapt weights based on whether recent results tended to be
	/// frequently-accessed entries.
	fn adapt_weights_for_results(
		&self,
		current_weights: &RequiredWeightProfile,
		result_ids: &[String],
	) -> RequiredWeightProfile {
		adapt_weights(
			&self.feedback,
			self.weight_adaptation_rate,
			current_weights,
			result_ids,
		)
	}

	// -- Public API -----------------------------------------------------------

	/// Record a completed query and its result set for learning.
	pub fn record_query(
		&mut self,
		query_embedding: &[f32],
		result_ids: &[String],
		topic: Option<&str>,
		now: u64,
	) {
		if !self.enabled {
			return;
		}
		if query_embedding.is_empty() || result_ids.is_empty() {
			return;
		}

		self.total_queries += 1;
		self.last_updated = now;

		// Add to global query history (FIFO capped)
		let record = QueryRecord {
			embedding: query_embedding.to_vec(),
			timestamp: now,
			result_count: result_ids.len(),
		};
		self.query_history.push(record.clone());
		if self.query_history.len() > self.max_query_history {
			let excess = self.query_history.len() - self.max_query_history;
			self.query_history.drain(..excess);
		}

		// Update per-entry feedback
		for id in result_ids {
			if let Some(existing) = self.feedback.get_mut(id) {
				existing.total_retrievals += 1;
				existing.last_query_timestamp = now;

				// Track diverse queries: only count if this query embedding
				// is sufficiently different from previously recorded ones.
				let is_diverse = existing.query_embeddings.is_empty()
					|| existing.query_embeddings.iter().all(|prev| {
						cosine_similarity(prev, query_embedding) < 0.9
					});
				if is_diverse {
					existing.query_count += 1;
					// Keep a bounded sample of query embeddings for diversity tracking
					if existing.query_embeddings.len() < 20 {
						existing.query_embeddings.push(query_embedding.to_vec());
					}
				}
			} else {
				self.feedback.insert(
					id.clone(),
					FeedbackEntry {
						query_count: 1,
						total_retrievals: 1,
						last_query_timestamp: now,
						query_embeddings: vec![query_embedding.to_vec()],
					},
				);
			}
		}

		// Update co-occurrence correlations for each pair of result IDs
		for i in 0..result_ids.len() {
			for j in (i + 1)..result_ids.len() {
				let a = &result_ids[i];
				let b = &result_ids[j];

				let map_a = self
					.correlations
					.entry(a.clone())
					.or_insert_with(HashMap::new);
				*map_a.entry(b.clone()).or_insert(0) += 1;

				let map_b = self
					.correlations
					.entry(b.clone())
					.or_insert_with(HashMap::new);
				*map_b.entry(a.clone()).or_insert(0) += 1;
			}
		}

		// Adapt global weights
		let result_ids_owned: Vec<String> = result_ids.to_vec();
		self.adapted_weights =
			self.adapt_weights_for_results(&self.adapted_weights.clone(), &result_ids_owned);

		// Recompute global interest embedding
		self.recompute_interest_embedding(now);

		// Update per-topic state if topic provided
		if let Some(topic) = topic {
			// Compute new weights using the free function (avoids borrow conflict)
			let max_hist = self.max_query_history;
			let decay_ms = self.query_decay_ms;
			let rate = self.weight_adaptation_rate;

			let topic_state = self
				.topic_states
				.entry(topic.to_string())
				.or_insert_with(|| TopicState {
					weights: RequiredWeightProfile {
						vector: 0.6,
						recency: 0.2,
						frequency: 0.2,
					},
					interest_embedding: None,
					query_count: 0,
					query_history: Vec::new(),
				});

			topic_state.query_count += 1;

			// Add to topic query history (FIFO capped)
			topic_state.query_history.push(QueryRecord {
				embedding: query_embedding.to_vec(),
				timestamp: now,
				result_count: result_ids.len(),
			});
			if topic_state.query_history.len() > max_hist {
				let excess = topic_state.query_history.len() - max_hist;
				topic_state.query_history.drain(..excess);
			}

			// Adapt topic weights using free function
			let current_weights = topic_state.weights.clone();
			topic_state.weights = adapt_weights(
				&self.feedback,
				rate,
				&current_weights,
				&result_ids_owned,
			);

			// Recompute topic interest embedding using free function
			topic_state.interest_embedding =
				compute_interest_embedding(decay_ms, &topic_state.query_history, now);
		}
	}

	/// Record explicit user feedback on whether an entry was relevant.
	pub fn record_feedback(&mut self, entry_id: &str, relevant: bool, now: u64) {
		if !self.enabled {
			return;
		}

		let existing = self
			.explicit_feedback
			.entry(entry_id.to_string())
			.or_insert(ExplicitFeedback {
				positive: 0,
				negative: 0,
			});
		if relevant {
			existing.positive += 1;
		} else {
			existing.negative += 1;
		}
		self.last_updated = now;
	}

	/// Get the current adapted weight profile, optionally per-topic.
	/// Falls back to global weights if the topic has fewer than 10 queries.
	pub fn get_adapted_weights(&self, topic: Option<&str>) -> RequiredWeightProfile {
		if let Some(topic) = topic {
			if let Some(topic_state) = self.topic_states.get(topic) {
				if topic_state.query_count >= TOPIC_QUERY_THRESHOLD {
					return topic_state.weights.clone();
				}
			}
		}
		self.adapted_weights.clone()
	}

	/// Get the current interest embedding, optionally per-topic.
	pub fn get_interest_embedding(&self, topic: Option<&str>) -> Option<Vec<f32>> {
		if let Some(topic) = topic {
			if let Some(topic_state) = self.topic_states.get(topic) {
				if topic_state.interest_embedding.is_some() {
					return topic_state.interest_embedding.clone();
				}
			}
			return None;
		}
		self.interest_embedding.clone()
	}

	/// Compute a boost multiplier for an entry based on learning state.
	/// Clamped to [0.8, 1.2].
	pub fn compute_boost(
		&self,
		entry_id: &str,
		entry_embedding: &[f32],
		topic: Option<&str>,
	) -> f64 {
		if !self.enabled {
			return 1.0;
		}

		let mut boost = 1.0;

		// Relevance feedback component
		if let Some(fb) = self.feedback.get(entry_id) {
			let relevance = self.compute_relevance_score(entry_id, fb);
			boost += relevance * 0.1;
		}

		// Interest alignment component
		let mut effective_interest: Option<&Vec<f32>> = None;
		if let Some(topic) = topic {
			if let Some(topic_state) = self.topic_states.get(topic) {
				effective_interest = topic_state.interest_embedding.as_ref();
			}
		}
		if effective_interest.is_none() {
			effective_interest = self.interest_embedding.as_ref();
		}

		if let Some(interest) = effective_interest {
			if entry_embedding.len() == interest.len() {
				let similarity = cosine_similarity(entry_embedding, interest);
				boost += 0.0f64.max(similarity) * self.interest_boost_weight;
			}
		}

		boost.clamp(BOOST_MIN, BOOST_MAX)
	}

	/// Get entries that frequently co-appear with the given entry in query results.
	/// Sorted by strength descending.
	pub fn get_correlated_entries(&self, entry_id: &str) -> Vec<(String, usize)> {
		let map = match self.correlations.get(entry_id) {
			Some(m) => m,
			None => return vec![],
		};
		if map.is_empty() {
			return vec![];
		}

		let mut results: Vec<(String, usize)> =
			map.iter().map(|(id, &count)| (id.clone(), count)).collect();
		results.sort_by(|a, b| b.1.cmp(&a.1));
		results
	}

	/// Get the full learning profile snapshot.
	pub fn get_profile(&self) -> PatronProfile {
		PatronProfile {
			query_history: self.query_history.clone(),
			adapted_weights: self.adapted_weights.clone(),
			interest_embedding: self.interest_embedding.clone(),
			total_queries: self.total_queries,
			last_updated: self.last_updated,
		}
	}

	/// Serialize all learning state for persistence.
	pub fn serialize(&self) -> LearningState {
		let serialized_feedback: Vec<FeedbackSerialized> = self
			.feedback
			.iter()
			.map(|(id, entry)| FeedbackSerialized {
				id: id.clone(),
				query_count: entry.query_count,
				total_retrievals: entry.total_retrievals,
				last_query_timestamp: entry.last_query_timestamp,
			})
			.collect();

		let serialized_history: Vec<SerializedQueryRecord> = self
			.query_history
			.iter()
			.map(|r| SerializedQueryRecord {
				embedding: encode_embedding(&r.embedding),
				timestamp: r.timestamp,
				result_count: r.result_count,
			})
			.collect();

		let serialized_explicit: Vec<ExplicitFeedbackSerialized> = self
			.explicit_feedback
			.iter()
			.map(|(id, counts)| ExplicitFeedbackSerialized {
				entry_id: id.clone(),
				positive_count: counts.positive,
				negative_count: counts.negative,
			})
			.collect();

		let serialized_topics: Vec<TopicProfileSerialized> = self
			.topic_states
			.iter()
			.map(|(topic, state)| TopicProfileSerialized {
				topic: topic.clone(),
				weights: state.weights.clone(),
				interest_embedding: state
					.interest_embedding
					.as_ref()
					.map(|e| encode_embedding(e)),
				query_count: state.query_count,
			})
			.collect();

		let serialized_correlations: Vec<CorrelationSerialized> = self
			.correlations
			.iter()
			.map(|(entry_id, map)| {
				let correlated: Vec<CorrelatedPairSerialized> = map
					.iter()
					.map(|(corr_id, &count)| CorrelatedPairSerialized {
						entry_id: corr_id.clone(),
						count,
					})
					.collect();
				CorrelationSerialized {
					entry_id: entry_id.clone(),
					correlated,
				}
			})
			.collect();

		LearningState {
			version: 1,
			feedback: serialized_feedback,
			query_history: serialized_history,
			adapted_weights: self.adapted_weights.clone(),
			interest_embedding: self
				.interest_embedding
				.as_ref()
				.map(|e| encode_embedding(e)),
			total_queries: self.total_queries,
			last_updated: self.last_updated,
			explicit_feedback: if serialized_explicit.is_empty() {
				None
			} else {
				Some(serialized_explicit)
			},
			topic_profiles: if serialized_topics.is_empty() {
				None
			} else {
				Some(serialized_topics)
			},
			correlations: if serialized_correlations.is_empty() {
				None
			} else {
				Some(serialized_correlations)
			},
		}
	}

	/// Restore learning state from a previously serialized snapshot.
	pub fn restore(&mut self, state: &LearningState) {
		// Restore feedback
		self.feedback.clear();
		for entry in &state.feedback {
			self.feedback.insert(
				entry.id.clone(),
				FeedbackEntry {
					query_count: entry.query_count,
					total_retrievals: entry.total_retrievals,
					last_query_timestamp: entry.last_query_timestamp,
					query_embeddings: Vec::new(), // not persisted — rebuilt from future queries
				},
			);
		}

		// Restore query history
		self.query_history.clear();
		for record in &state.query_history {
			if let Some(embedding) = decode_embedding(&record.embedding) {
				self.query_history.push(QueryRecord {
					embedding,
					timestamp: record.timestamp,
					result_count: record.result_count,
				});
			}
			// Skip corrupt records
		}

		// Restore weights
		self.adapted_weights = RequiredWeightProfile {
			vector: state.adapted_weights.vector,
			recency: state.adapted_weights.recency,
			frequency: state.adapted_weights.frequency,
		};
		self.normalize_weights_in_place();

		// Restore interest embedding
		self.interest_embedding = state
			.interest_embedding
			.as_ref()
			.and_then(|e| decode_embedding(e));

		// Restore explicit feedback
		self.explicit_feedback.clear();
		if let Some(ref ef) = state.explicit_feedback {
			for entry in ef {
				self.explicit_feedback.insert(
					entry.entry_id.clone(),
					ExplicitFeedback {
						positive: entry.positive_count,
						negative: entry.negative_count,
					},
				);
			}
		}

		// Restore per-topic state
		self.topic_states.clear();
		if let Some(ref profiles) = state.topic_profiles {
			for profile in profiles {
				let weights = Self::normalize_weights_required(&RequiredWeightProfile {
					vector: profile.weights.vector,
					recency: profile.weights.recency,
					frequency: profile.weights.frequency,
				});

				let interest_embedding = profile
					.interest_embedding
					.as_ref()
					.and_then(|e| decode_embedding(e));

				self.topic_states.insert(
					profile.topic.clone(),
					TopicState {
						weights,
						interest_embedding,
						query_count: profile.query_count,
						query_history: Vec::new(), // not persisted
					},
				);
			}
		}

		// Restore correlations
		self.correlations.clear();
		if let Some(ref corrs) = state.correlations {
			for entry in corrs {
				let mut map = HashMap::new();
				for pair in &entry.correlated {
					map.insert(pair.entry_id.clone(), pair.count);
				}
				self.correlations.insert(entry.entry_id.clone(), map);
			}
		}

		self.total_queries = state.total_queries;
		self.last_updated = state.last_updated;
	}

	/// Clear all learning state.
	pub fn clear(&mut self) {
		self.feedback.clear();
		self.explicit_feedback.clear();
		self.query_history.clear();
		self.adapted_weights = RequiredWeightProfile {
			vector: 0.6,
			recency: 0.2,
			frequency: 0.2,
		};
		self.interest_embedding = None;
		self.total_queries = 0;
		self.last_updated = 0;
		self.topic_states.clear();
		self.correlations.clear();
	}

	/// Remove feedback for entries that no longer exist.
	pub fn prune_entries(&mut self, valid_ids: &HashSet<String>) {
		self.feedback.retain(|id, _| valid_ids.contains(id));
		self.explicit_feedback
			.retain(|id, _| valid_ids.contains(id));

		// Remove entire correlation maps for pruned entries
		self.correlations.retain(|id, _| valid_ids.contains(id));

		// Remove references to pruned entries from remaining correlation maps
		for map in self.correlations.values_mut() {
			map.retain(|id, _| valid_ids.contains(id));
		}
	}

	/// Total number of queries recorded.
	pub fn total_queries(&self) -> usize {
		self.total_queries
	}

	/// Whether any learning state exists.
	pub fn has_data(&self) -> bool {
		self.total_queries > 0
	}
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;

	fn default_engine() -> LearningEngine {
		LearningEngine::new(LearningOptions::default())
	}

	fn make_embedding(values: &[f32]) -> Vec<f32> {
		values.to_vec()
	}

	fn make_result_ids(ids: &[&str]) -> Vec<String> {
		ids.iter().map(|s| s.to_string()).collect()
	}

	// -- record_query tests ---------------------------------------------------

	#[test]
	fn record_query_increments_total() {
		let mut engine = default_engine();
		let emb = make_embedding(&[1.0, 0.0, 0.0]);
		let ids = make_result_ids(&["a", "b"]);
		engine.record_query(&emb, &ids, None, 1000);
		assert_eq!(engine.total_queries(), 1);
		assert!(engine.has_data());
	}

	#[test]
	fn record_query_skips_when_disabled() {
		let mut engine = LearningEngine::new(LearningOptions {
			enabled: false,
			..Default::default()
		});
		let emb = make_embedding(&[1.0, 0.0, 0.0]);
		let ids = make_result_ids(&["a"]);
		engine.record_query(&emb, &ids, None, 1000);
		assert_eq!(engine.total_queries(), 0);
		assert!(!engine.has_data());
	}

	#[test]
	fn record_query_skips_empty_embedding() {
		let mut engine = default_engine();
		engine.record_query(&[], &make_result_ids(&["a"]), None, 1000);
		assert_eq!(engine.total_queries(), 0);
	}

	#[test]
	fn record_query_skips_empty_results() {
		let mut engine = default_engine();
		let emb = make_embedding(&[1.0, 0.0, 0.0]);
		engine.record_query(&emb, &[], None, 1000);
		assert_eq!(engine.total_queries(), 0);
	}

	#[test]
	fn record_query_caps_history() {
		let mut engine = LearningEngine::new(LearningOptions {
			max_query_history: 3,
			..Default::default()
		});
		for i in 0..5 {
			let emb = make_embedding(&[i as f32, 0.0, 0.0]);
			let ids = make_result_ids(&["a"]);
			engine.record_query(&emb, &ids, None, 1000 + i * 100);
		}
		assert_eq!(engine.query_history.len(), 3);
		// Should have the last 3 entries
		assert_eq!(engine.query_history[0].timestamp, 1200);
		assert_eq!(engine.query_history[1].timestamp, 1300);
		assert_eq!(engine.query_history[2].timestamp, 1400);
	}

	// -- record_feedback tests ------------------------------------------------

	#[test]
	fn record_feedback_positive() {
		let mut engine = default_engine();
		engine.record_feedback("a", true, 1000);
		engine.record_feedback("a", true, 2000);
		let ef = engine.explicit_feedback.get("a").unwrap();
		assert_eq!(ef.positive, 2);
		assert_eq!(ef.negative, 0);
	}

	#[test]
	fn record_feedback_negative() {
		let mut engine = default_engine();
		engine.record_feedback("a", false, 1000);
		let ef = engine.explicit_feedback.get("a").unwrap();
		assert_eq!(ef.positive, 0);
		assert_eq!(ef.negative, 1);
	}

	#[test]
	fn record_feedback_disabled() {
		let mut engine = LearningEngine::new(LearningOptions {
			enabled: false,
			..Default::default()
		});
		engine.record_feedback("a", true, 1000);
		assert!(engine.explicit_feedback.is_empty());
	}

	// -- weight adaptation tests ----------------------------------------------

	#[test]
	fn weights_adapt_over_queries() {
		let mut engine = default_engine();
		let initial = engine.get_adapted_weights(None);

		// Record several queries so weights adapt
		for i in 0..10 {
			let emb = make_embedding(&[1.0, i as f32 * 0.1, 0.0]);
			let ids = make_result_ids(&["a", "b"]);
			engine.record_query(&emb, &ids, None, 1000 + i * 100);
		}

		let adapted = engine.get_adapted_weights(None);
		// Weights should have changed
		let changed = (adapted.vector - initial.vector).abs() > 1e-10
			|| (adapted.recency - initial.recency).abs() > 1e-10
			|| (adapted.frequency - initial.frequency).abs() > 1e-10;
		assert!(changed, "Weights should adapt after queries");

		// Sum should still be ~1.0
		let sum = adapted.vector + adapted.recency + adapted.frequency;
		assert!((sum - 1.0).abs() < 1e-10, "Weights should sum to 1.0");
	}

	// -- interest embedding tests ---------------------------------------------

	#[test]
	fn interest_embedding_computed_after_queries() {
		let mut engine = default_engine();
		assert!(engine.get_interest_embedding(None).is_none());

		let emb = make_embedding(&[1.0, 0.0, 0.0]);
		let ids = make_result_ids(&["a"]);
		engine.record_query(&emb, &ids, None, 1000);

		let interest = engine.get_interest_embedding(None);
		assert!(interest.is_some());
		let interest = interest.unwrap();
		assert_eq!(interest.len(), 3);

		// With a single query, interest should point same direction
		let sim = cosine_similarity(&emb, &interest);
		assert!(sim > 0.99, "Interest should align with single query");
	}

	#[test]
	fn interest_embedding_per_topic() {
		let mut engine = default_engine();

		let emb = make_embedding(&[1.0, 0.0, 0.0]);
		let ids = make_result_ids(&["a"]);
		engine.record_query(&emb, &ids, Some("rust"), 1000);

		assert!(engine.get_interest_embedding(Some("rust")).is_some());
		assert!(engine.get_interest_embedding(Some("python")).is_none());
	}

	// -- boost computation tests ----------------------------------------------

	#[test]
	fn compute_boost_default_is_clamped() {
		let engine = default_engine();
		let emb = make_embedding(&[1.0, 0.0, 0.0]);
		let boost = engine.compute_boost("unknown", &emb, None);
		// With no data, boost should be clamped to [0.8, 1.2]
		assert!(boost >= 0.8 && boost <= 1.2);
	}

	#[test]
	fn compute_boost_disabled_returns_one() {
		let engine = LearningEngine::new(LearningOptions {
			enabled: false,
			..Default::default()
		});
		let emb = make_embedding(&[1.0, 0.0, 0.0]);
		assert_eq!(engine.compute_boost("a", &emb, None), 1.0);
	}

	#[test]
	fn compute_boost_increases_with_relevance() {
		let mut engine = default_engine();
		let emb = make_embedding(&[1.0, 0.0, 0.0]);
		let ids = make_result_ids(&["a"]);

		// Record many diverse queries that retrieve "a"
		for i in 0..10 {
			let qemb = make_embedding(&[(i as f32 * 0.1).cos(), (i as f32 * 0.1).sin(), 0.0]);
			engine.record_query(&qemb, &ids, None, 1000 + i * 1000);
		}

		let boost_known = engine.compute_boost("a", &emb, None);
		let boost_unknown = engine.compute_boost("unknown", &emb, None);
		assert!(
			boost_known >= boost_unknown,
			"Known entries should get equal or higher boost"
		);
	}

	// -- serialize / restore round-trip tests ---------------------------------

	#[test]
	fn serialize_restore_round_trip() {
		let mut engine = default_engine();

		// Build up some state
		let emb1 = make_embedding(&[1.0, 0.0, 0.0]);
		let emb2 = make_embedding(&[0.0, 1.0, 0.0]);
		let ids1 = make_result_ids(&["a", "b"]);
		let ids2 = make_result_ids(&["b", "c"]);

		engine.record_query(&emb1, &ids1, Some("rust"), 1000);
		engine.record_query(&emb2, &ids2, Some("rust"), 2000);
		engine.record_feedback("a", true, 3000);
		engine.record_feedback("b", false, 3000);

		let state = engine.serialize();
		let json = serde_json::to_string(&state).unwrap();

		// Restore into a fresh engine
		let mut engine2 = default_engine();
		let state2: LearningState = serde_json::from_str(&json).unwrap();
		engine2.restore(&state2);

		assert_eq!(engine2.total_queries(), 2);
		assert_eq!(engine2.last_updated, 3000);
		assert!(engine2.has_data());

		// Check weights are preserved
		let w1 = engine.get_adapted_weights(None);
		let w2 = engine2.get_adapted_weights(None);
		assert!((w1.vector - w2.vector).abs() < 1e-6);
		assert!((w1.recency - w2.recency).abs() < 1e-6);
		assert!((w1.frequency - w2.frequency).abs() < 1e-6);

		// Check explicit feedback is preserved
		let ef = engine2.explicit_feedback.get("a").unwrap();
		assert_eq!(ef.positive, 1);
		let ef_b = engine2.explicit_feedback.get("b").unwrap();
		assert_eq!(ef_b.negative, 1);
	}

	#[test]
	fn serialize_preserves_interest_embedding() {
		let mut engine = default_engine();
		let emb = make_embedding(&[0.6, 0.8, 0.0]);
		let ids = make_result_ids(&["a"]);
		engine.record_query(&emb, &ids, None, 1000);

		let original_interest = engine.get_interest_embedding(None).unwrap();

		let state = engine.serialize();
		let mut engine2 = default_engine();
		engine2.restore(&state);

		let restored_interest = engine2.get_interest_embedding(None).unwrap();
		assert_eq!(original_interest.len(), restored_interest.len());

		// Base64 round-trip loses some precision (f64 -> f32 -> base64 -> f32)
		// but should be very close
		let sim = cosine_similarity(&original_interest, &restored_interest);
		assert!(sim > 0.999, "Interest embedding should survive round-trip");
	}

	// -- clear tests ----------------------------------------------------------

	#[test]
	fn clear_resets_all_state() {
		let mut engine = default_engine();
		let emb = make_embedding(&[1.0, 0.0, 0.0]);
		let ids = make_result_ids(&["a"]);
		engine.record_query(&emb, &ids, Some("rust"), 1000);
		engine.record_feedback("a", true, 2000);

		assert!(engine.has_data());
		engine.clear();

		assert!(!engine.has_data());
		assert_eq!(engine.total_queries(), 0);
		assert!(engine.get_interest_embedding(None).is_none());
		assert!(engine.get_correlated_entries("a").is_empty());
		assert_eq!(engine.last_updated, 0);
	}

	// -- prune tests ----------------------------------------------------------

	#[test]
	fn prune_removes_invalid_entries() {
		let mut engine = default_engine();

		let emb = make_embedding(&[1.0, 0.0, 0.0]);
		let ids = make_result_ids(&["a", "b", "c"]);
		engine.record_query(&emb, &ids, None, 1000);
		engine.record_feedback("a", true, 2000);
		engine.record_feedback("b", false, 2000);

		let valid: HashSet<String> = vec!["a".to_string()].into_iter().collect();
		engine.prune_entries(&valid);

		// "a" should still exist
		assert!(engine.feedback.contains_key("a"));
		assert!(engine.explicit_feedback.contains_key("a"));

		// "b" and "c" should be removed
		assert!(!engine.feedback.contains_key("b"));
		assert!(!engine.feedback.contains_key("c"));
		assert!(!engine.explicit_feedback.contains_key("b"));

		// Correlations involving b and c should be removed
		assert!(!engine.correlations.contains_key("b"));
		assert!(!engine.correlations.contains_key("c"));

		// Correlations from "a" should not reference b or c
		if let Some(map) = engine.correlations.get("a") {
			assert!(!map.contains_key("b"));
			assert!(!map.contains_key("c"));
		}
	}

	// -- correlation tests ----------------------------------------------------

	#[test]
	fn correlations_tracked_between_result_pairs() {
		let mut engine = default_engine();
		let emb = make_embedding(&[1.0, 0.0, 0.0]);
		let ids = make_result_ids(&["a", "b", "c"]);
		engine.record_query(&emb, &ids, None, 1000);

		let corr_a = engine.get_correlated_entries("a");
		assert_eq!(corr_a.len(), 2); // b and c
		let corr_b = engine.get_correlated_entries("b");
		assert_eq!(corr_b.len(), 2); // a and c

		// Record again to strengthen
		engine.record_query(&emb, &make_result_ids(&["a", "b"]), None, 2000);
		let corr_a2 = engine.get_correlated_entries("a");
		// Find b in correlations
		let b_strength = corr_a2.iter().find(|(id, _)| id == "b").unwrap().1;
		assert_eq!(b_strength, 2); // appeared twice together
	}

	#[test]
	fn correlations_sorted_by_strength() {
		let mut engine = default_engine();
		let emb = make_embedding(&[1.0, 0.0, 0.0]);

		// a+b appear together 3 times
		for i in 0..3 {
			engine.record_query(
				&emb,
				&make_result_ids(&["a", "b"]),
				None,
				1000 + i * 100,
			);
		}
		// a+c appear together 1 time
		engine.record_query(&emb, &make_result_ids(&["a", "c"]), None, 2000);

		let corr = engine.get_correlated_entries("a");
		assert_eq!(corr[0].0, "b"); // strongest first
		assert_eq!(corr[0].1, 3);
		assert_eq!(corr[1].0, "c");
		assert_eq!(corr[1].1, 1);
	}

	// -- topic-specific weights tests -----------------------------------------

	#[test]
	fn topic_weights_below_threshold_returns_global() {
		let mut engine = default_engine();
		let emb = make_embedding(&[1.0, 0.0, 0.0]);
		let ids = make_result_ids(&["a"]);

		// Record fewer than TOPIC_QUERY_THRESHOLD queries for a topic
		for i in 0..5 {
			engine.record_query(&emb, &ids, Some("rust"), 1000 + i * 100);
		}

		let topic_weights = engine.get_adapted_weights(Some("rust"));
		let global_weights = engine.get_adapted_weights(None);
		assert_eq!(topic_weights.vector, global_weights.vector);
		assert_eq!(topic_weights.recency, global_weights.recency);
		assert_eq!(topic_weights.frequency, global_weights.frequency);
	}

	#[test]
	fn topic_weights_above_threshold_returns_topic() {
		let mut engine = default_engine();
		let ids = make_result_ids(&["a"]);

		// Record >= TOPIC_QUERY_THRESHOLD queries for "rust"
		for i in 0..12 {
			let emb = make_embedding(&[(i as f32 * 0.3).cos(), (i as f32 * 0.3).sin(), 0.0]);
			engine.record_query(&emb, &ids, Some("rust"), 1000 + i * 100);
		}

		// Also record some global queries with different characteristics
		for i in 0..5 {
			let emb = make_embedding(&[0.0, 0.0, 1.0]);
			engine.record_query(&emb, &ids, None, 5000 + i * 100);
		}

		let topic_weights = engine.get_adapted_weights(Some("rust"));
		let global_weights = engine.get_adapted_weights(None);

		// They may differ since topic has its own adaptation path
		// At minimum, both should sum to 1.0
		let topic_sum = topic_weights.vector + topic_weights.recency + topic_weights.frequency;
		let global_sum = global_weights.vector + global_weights.recency + global_weights.frequency;
		assert!((topic_sum - 1.0).abs() < 1e-10);
		assert!((global_sum - 1.0).abs() < 1e-10);
	}

	// -- diversity tracking tests ---------------------------------------------

	#[test]
	fn diversity_check_prevents_duplicate_counting() {
		let mut engine = default_engine();
		let emb = make_embedding(&[1.0, 0.0, 0.0]);
		let ids = make_result_ids(&["a"]);

		// Same query embedding recorded multiple times
		engine.record_query(&emb, &ids, None, 1000);
		engine.record_query(&emb, &ids, None, 2000);
		engine.record_query(&emb, &ids, None, 3000);

		let fb = engine.feedback.get("a").unwrap();
		// queryCount should be 1 (only first counts as diverse)
		assert_eq!(fb.query_count, 1);
		// totalRetrievals should be 3 (all counted)
		assert_eq!(fb.total_retrievals, 3);
	}

	// -- encode/decode embedding tests ----------------------------------------

	#[test]
	fn encode_decode_embedding_round_trip() {
		let original = vec![1.0f32, -0.5, 0.0, 3.14159];
		let encoded = encode_embedding(&original);
		let decoded = decode_embedding(&encoded).unwrap();
		assert_eq!(original.len(), decoded.len());
		for (a, b) in original.iter().zip(decoded.iter()) {
			assert!((a - b).abs() < 1e-6);
		}
	}

	#[test]
	fn decode_embedding_invalid_base64() {
		assert!(decode_embedding("not-valid-base64!!!").is_none());
	}
}
