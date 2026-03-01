//! Core application state machine.

use serde::{Deserialize, Serialize};

/// A volume (document) in the library.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeView {
	pub id: String,
	pub text: String,
	pub topic: String,
	pub metadata: std::collections::HashMap<String, String>,
	pub timestamp: i64,
}

/// A search result with relevance score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultView {
	pub volume: VolumeView,
	pub score: f64,
}

/// Aggregated topic metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicView {
	pub topic: String,
	pub volume_count: usize,
}

/// Options for text generation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GenerateOptions {
	pub skip_library: bool,
	pub library_max_results: Option<usize>,
	pub library_threshold: Option<f64>,
}

/// Result of text generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateResult {
	pub content: String,
	pub agent_id: String,
	pub server_name: String,
	pub library_context: Vec<SearchResultView>,
	pub stored_volume_id: Option<String>,
}
