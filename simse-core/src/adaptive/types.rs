use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::adaptive::distance::DistanceMetric;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
	pub id: String,
	pub text: String,
	pub embedding: Vec<f32>,
	pub metadata: HashMap<String, String>,
	pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lookup {
	pub entry: Entry,
	pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextLookup {
	pub entry: Entry,
	pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedLookup {
	pub entry: Entry,
	pub score: f64,
	pub scores: ScoreBreakdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreBreakdown {
	pub vector: Option<f64>,
	pub text: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateCheckResult {
	#[serde(rename = "isDuplicate")]
	pub is_duplicate: bool,
	#[serde(rename = "existingEntry")]
	pub existing_entry: Option<Entry>,
	pub similarity: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateCluster {
	pub representative: Entry,
	pub duplicates: Vec<Entry>,
	#[serde(rename = "averageSimilarity")]
	pub average_similarity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicInfo {
	pub topic: String,
	#[serde(rename = "entryCount")]
	pub entry_count: usize,
	#[serde(rename = "entryIds")]
	pub entry_ids: Vec<String>,
	pub parent: Option<String>,
	pub children: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicCatalogSection {
	pub topic: String,
	pub parent: Option<String>,
	pub children: Vec<String>,
	#[serde(rename = "entryCount")]
	pub entry_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
	pub entry: Entry,
	pub score: f64,
	pub scores: RecommendationScores,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendationScores {
	pub vector: Option<f64>,
	pub recency: Option<f64>,
	pub frequency: Option<f64>,
	pub graph: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightProfile {
	pub vector: Option<f64>,
	pub recency: Option<f64>,
	pub frequency: Option<f64>,
	pub graph: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetadataFilter {
	pub key: String,
	pub value: Option<serde_json::Value>,
	pub mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateRange {
	pub after: Option<u64>,
	pub before: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnerProfile {
	#[serde(rename = "queryHistory")]
	pub query_history: Vec<QueryRecord>,
	#[serde(rename = "adaptedWeights")]
	pub adapted_weights: RequiredWeightProfile,
	#[serde(rename = "interestEmbedding")]
	pub interest_embedding: Option<Vec<f32>>,
	#[serde(rename = "totalQueries")]
	pub total_queries: usize,
	#[serde(rename = "lastUpdated")]
	pub last_updated: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryRecord {
	pub embedding: Vec<f32>,
	pub timestamp: u64,
	#[serde(rename = "resultCount")]
	pub result_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequiredWeightProfile {
	pub vector: f64,
	pub recency: f64,
	pub frequency: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphBoost {
	pub enabled: Option<bool>,
	pub weight: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchOptions {
	#[serde(rename = "queryEmbedding")]
	pub query_embedding: Option<Vec<f32>>,
	#[serde(rename = "similarityThreshold")]
	pub similarity_threshold: Option<f64>,
	pub text: Option<TextSearchOptions>,
	pub metadata: Option<Vec<MetadataFilter>>,
	#[serde(rename = "dateRange")]
	pub date_range: Option<DateRange>,
	#[serde(rename = "maxResults")]
	pub max_results: Option<usize>,
	#[serde(rename = "rankBy")]
	pub rank_by: Option<String>,
	#[serde(rename = "fieldBoosts")]
	pub field_boosts: Option<FieldBoosts>,
	#[serde(rename = "rankWeights")]
	pub rank_weights: Option<RankWeights>,
	#[serde(rename = "topicFilter")]
	pub topic_filter: Option<Vec<String>>,
	#[serde(rename = "graphBoost")]
	pub graph_boost: Option<GraphBoost>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextSearchOptions {
	pub query: String,
	pub mode: Option<String>,
	pub threshold: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldBoosts {
	pub text: Option<f64>,
	pub metadata: Option<f64>,
	pub topic: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankWeights {
	pub vector: Option<f64>,
	pub text: Option<f64>,
	pub metadata: Option<f64>,
	pub recency: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendOptions {
	#[serde(rename = "queryEmbedding")]
	pub query_embedding: Option<Vec<f32>>,
	pub weights: Option<WeightProfile>,
	#[serde(rename = "maxResults")]
	pub max_results: Option<usize>,
	#[serde(rename = "minScore")]
	pub min_score: Option<f64>,
	pub metadata: Option<Vec<MetadataFilter>>,
	pub topics: Option<Vec<String>>,
	#[serde(rename = "dateRange")]
	pub date_range: Option<DateRange>,
}

/// Strategy for vector index backing the store's search operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum IndexStrategy {
	/// Automatically choose: flat for small stores, HNSW above the threshold.
	#[default]
	Auto,
	/// Always use brute-force flat scan.
	Flat,
	/// Always use HNSW approximate nearest-neighbor index.
	Hnsw,
}

/// Statistics about the current index configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexStats {
	/// The index type currently in effect (`"flat"` or `"hnsw"`).
	pub index_type: String,
	/// Number of vectors in the store.
	pub vector_count: usize,
	/// Dimensionality of stored vectors (0 if store is empty).
	pub dimensions: usize,
	/// The distance metric configured for searches.
	pub metric: DistanceMetric,
}

/// Options for the extended `search_with_options` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchWithOptionsParams {
	/// Query embedding vector.
	#[serde(rename = "queryEmbedding")]
	pub query_embedding: Vec<f32>,
	/// Maximum number of results.
	#[serde(rename = "maxResults")]
	pub max_results: Option<usize>,
	/// Minimum similarity threshold.
	pub threshold: Option<f64>,
	/// Distance metric to use (defaults to store config's `default_metric`).
	pub metric: Option<DistanceMetric>,
	/// If set, apply MMR reranking with this lambda value (0.0..=1.0).
	#[serde(rename = "mmrLambda")]
	pub mmr_lambda: Option<f64>,
}
