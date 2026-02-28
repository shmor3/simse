use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Volume {
	pub id: String,
	pub text: String,
	pub embedding: Vec<f32>,
	pub metadata: HashMap<String, String>,
	pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lookup {
	pub volume: Volume,
	pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextLookup {
	pub volume: Volume,
	pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedLookup {
	pub volume: Volume,
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
	#[serde(rename = "existingVolume")]
	pub existing_volume: Option<Volume>,
	pub similarity: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateVolumes {
	pub representative: Volume,
	pub duplicates: Vec<Volume>,
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
	#[serde(rename = "volumeCount")]
	pub volume_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
	pub volume: Volume,
	pub score: f64,
	pub scores: RecommendationScores,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendationScores {
	pub vector: Option<f64>,
	pub recency: Option<f64>,
	pub frequency: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightProfile {
	pub vector: Option<f64>,
	pub recency: Option<f64>,
	pub frequency: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
pub struct PatronProfile {
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
