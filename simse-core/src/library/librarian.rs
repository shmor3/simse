//! Librarian — LLM-driven memory extraction, summarization, classification,
//! reorganization, optimization, and bidding.
//!
//! The `Librarian` struct wraps a `TextGenerationProvider` and provides six
//! async methods that format prompts, call the LLM, parse JSON responses,
//! and gracefully fall back to empty/default results on parse failure.

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;

use crate::error::SimseError;
use crate::library::TextGenerationProvider;

/// Truncate a string to at most `max_bytes` bytes, ensuring the result
/// ends on a valid UTF-8 char boundary.
pub(crate) fn truncate_str(s: &str, max_bytes: usize) -> &str {
	if s.len() <= max_bytes {
		return s;
	}
	let mut end = max_bytes;
	while end > 0 && !s.is_char_boundary(end) {
		end -= 1;
	}
	&s[..end]
}

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

/// Entry type for an extracted memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryType {
	Fact,
	Decision,
	Observation,
}

impl EntryType {
	pub fn as_str(&self) -> &str {
		match self {
			Self::Fact => "fact",
			Self::Decision => "decision",
			Self::Observation => "observation",
		}
	}
}

impl std::fmt::Display for EntryType {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(self.as_str())
	}
}

/// A single extracted memory from a conversation turn.
#[derive(Debug, Clone)]
pub struct ExtractionMemory {
	pub text: String,
	pub topic: String,
	pub tags: Vec<String>,
	pub entry_type: EntryType,
}

/// Result of the `extract` method.
#[derive(Debug, Clone)]
pub struct ExtractionResult {
	pub memories: Vec<ExtractionMemory>,
}

/// Context for a conversation turn.
#[derive(Debug, Clone)]
pub struct TurnContext {
	pub user_input: String,
	pub response: String,
}

/// Result of topic classification.
#[derive(Debug, Clone)]
pub struct ClassificationResult {
	pub topic: String,
	pub confidence: f64,
}

/// A single volume move in a reorganization plan.
#[derive(Debug, Clone)]
pub struct ReorganizationMove {
	pub volume_id: String,
	pub new_topic: String,
}

/// A topic merge in a reorganization plan.
#[derive(Debug, Clone)]
pub struct ReorganizationMerge {
	pub source: String,
	pub target: String,
}

/// Plan for reorganizing volumes within a topic.
#[derive(Debug, Clone)]
pub struct ReorganizationPlan {
	pub moves: Vec<ReorganizationMove>,
	pub new_subtopics: Vec<String>,
	pub merges: Vec<ReorganizationMerge>,
}

/// Result of the `optimize` method.
#[derive(Debug, Clone)]
pub struct OptimizationResult {
	pub pruned: Vec<String>,
	pub summary: String,
	pub reorganization: ReorganizationPlan,
	pub model_used: String,
}

/// A librarian's bid for managing content.
#[derive(Debug, Clone)]
pub struct LibrarianBid {
	pub librarian_name: String,
	pub argument: String,
	pub confidence: f64,
}

/// Subset of Library needed during librarian bidding.
#[async_trait]
pub trait LibrarianLibraryAccess: Send + Sync {
	async fn search(&self, query: &str, max_results: Option<usize>)
		-> Result<Vec<Volume>, SimseError>;
	async fn get_topics(&self) -> Result<Vec<String>, SimseError>;
	async fn filter_by_topic(&self, topics: &[String]) -> Result<Vec<Volume>, SimseError>;
}

/// Minimal volume representation for librarian operations.
/// Uses the vector engine's Volume type directly where possible,
/// but the librarian only needs id and text.
#[derive(Debug, Clone)]
pub struct Volume {
	pub id: String,
	pub text: String,
}

/// Summarization result.
#[derive(Debug, Clone)]
pub struct SummarizeResult {
	pub text: String,
	pub source_ids: Vec<String>,
}

/// Result of arbitration between multiple librarians.
#[derive(Debug, Clone)]
pub struct ArbitrationResult {
	pub winner: String,
	pub reason: String,
	pub bids: Vec<LibrarianBid>,
}

/// Optional extension for text generation with a specific model.
#[async_trait]
pub trait TextGenerationWithModel: TextGenerationProvider {
	/// Generate text using a specific model identifier.
	async fn generate_with_model(
		&self,
		prompt: &str,
		model_id: &str,
	) -> Result<String, SimseError>;
}

// ---------------------------------------------------------------------------
// Internal JSON deserialization helpers
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ExtractedMemoryJson {
	text: Option<String>,
	topic: Option<String>,
	tags: Option<Vec<String>>,
	#[serde(rename = "entryType")]
	entry_type: Option<String>,
}

#[derive(Deserialize)]
struct ExtractionJson {
	memories: Option<Vec<ExtractedMemoryJson>>,
}

#[derive(Deserialize)]
struct ClassificationJson {
	topic: Option<String>,
	confidence: Option<f64>,
}

#[derive(Deserialize)]
struct ReorgMoveJson {
	#[serde(rename = "volumeId")]
	volume_id: Option<String>,
	#[serde(rename = "newTopic")]
	new_topic: Option<String>,
}

#[derive(Deserialize)]
struct ReorgMergeJson {
	source: Option<String>,
	target: Option<String>,
}

#[derive(Deserialize)]
struct ReorgJson {
	moves: Option<Vec<ReorgMoveJson>>,
	#[serde(rename = "newSubtopics")]
	new_subtopics: Option<Vec<String>>,
	merges: Option<Vec<ReorgMergeJson>>,
}

#[derive(Deserialize)]
struct OptimizationJson {
	pruned: Option<Vec<String>>,
	summary: Option<String>,
	reorganization: Option<ReorgJson>,
}

#[derive(Deserialize)]
struct BidJson {
	argument: Option<String>,
	confidence: Option<f64>,
}

// ---------------------------------------------------------------------------
// Librarian
// ---------------------------------------------------------------------------

/// LLM-driven librarian for memory extraction, summarization, classification,
/// reorganization, optimization, and bidding.
pub struct Librarian {
	text_generator: Arc<dyn TextGenerationProvider>,
	/// Optional model-specific generator (for optimize with a powerful model).
	model_generator: Option<Arc<dyn TextGenerationWithModel>>,
	name: String,
	purpose: String,
}

/// Options for creating a librarian.
#[derive(Debug, Clone, Default)]
pub struct LibrarianOptions {
	pub name: Option<String>,
	pub purpose: Option<String>,
}

impl Librarian {
	/// Create a new librarian with the given text generation provider.
	pub fn new(
		text_generator: Arc<dyn TextGenerationProvider>,
		options: Option<LibrarianOptions>,
	) -> Self {
		let opts = options.unwrap_or_default();
		Self {
			text_generator,
			model_generator: None,
			name: opts.name.unwrap_or_else(|| "default".to_string()),
			purpose: opts.purpose.unwrap_or_else(|| "General-purpose librarian".to_string()),
		}
	}

	/// Create a librarian with a model-specific generator for optimize operations.
	pub fn with_model_generator(
		mut self,
		generator: Arc<dyn TextGenerationWithModel>,
	) -> Self {
		self.model_generator = Some(generator);
		self
	}

	/// The librarian's name.
	pub fn name(&self) -> &str {
		&self.name
	}

	/// The librarian's purpose.
	pub fn purpose(&self) -> &str {
		&self.purpose
	}

	// -----------------------------------------------------------------------
	// extract
	// -----------------------------------------------------------------------

	/// Analyze a conversation turn and extract important information as memories.
	///
	/// Returns an empty result on LLM failure or JSON parse failure.
	pub async fn extract(&self, turn: &TurnContext) -> ExtractionResult {
		let prompt = format!(
			r#"Analyze this conversation turn and extract important information worth remembering.

User: {}
Assistant: {}

Return a JSON object with this structure:
{{
  "memories": [
    {{
      "text": "concise fact or decision",
      "topic": "hierarchical/topic/path",
      "tags": ["tag1", "tag2"],
      "entryType": "fact" | "decision" | "observation"
    }}
  ]
}}

Rules:
- Only extract genuinely important facts, decisions, or observations
- Skip trivial conversational content
- Use hierarchical topic paths separated by /
- Return {{"memories": []}} if nothing worth remembering

Respond with ONLY valid JSON, no other text."#,
			turn.user_input, turn.response
		);

		let response = match self.text_generator.generate(&prompt, None).await {
			Ok(r) => r,
			Err(_) => return ExtractionResult { memories: Vec::new() },
		};

		let parsed: ExtractionJson = match serde_json::from_str(&response) {
			Ok(p) => p,
			Err(_) => return ExtractionResult { memories: Vec::new() },
		};

		let memories = parsed
			.memories
			.unwrap_or_default()
			.into_iter()
			.filter_map(|m| {
				let text = m.text?;
				let topic = m.topic?;
				let tags = m.tags.unwrap_or_default();
				let entry_type_str = m.entry_type?;
				let entry_type = match entry_type_str.as_str() {
					"fact" => EntryType::Fact,
					"decision" => EntryType::Decision,
					"observation" => EntryType::Observation,
					_ => return None,
				};
				Some(ExtractionMemory {
					text,
					topic,
					tags,
					entry_type,
				})
			})
			.collect();

		ExtractionResult { memories }
	}

	// -----------------------------------------------------------------------
	// summarize
	// -----------------------------------------------------------------------

	/// Summarize multiple volumes into a single concise summary.
	pub async fn summarize(
		&self,
		volumes: &[Volume],
		topic: &str,
	) -> Result<SummarizeResult, SimseError> {
		let combined_text: String = volumes
			.iter()
			.enumerate()
			.map(|(i, v)| format!("--- Volume {} ---\n{}", i + 1, v.text))
			.collect::<Vec<_>>()
			.join("\n\n");

		let prompt = format!(
			"Summarize the following volumes from topic \"{}\" into a single concise summary that preserves all key information:\n\n{}",
			topic, combined_text
		);

		let text = self.text_generator.generate(&prompt, None).await?;
		let source_ids = volumes.iter().map(|v| v.id.clone()).collect();

		Ok(SummarizeResult { text, source_ids })
	}

	// -----------------------------------------------------------------------
	// classify_topic
	// -----------------------------------------------------------------------

	/// Classify text into the most appropriate topic.
	///
	/// Returns a fallback `("uncategorized", 0.0)` on LLM or parse failure.
	pub async fn classify_topic(
		&self,
		text: &str,
		existing_topics: &[String],
	) -> ClassificationResult {
		let topics_list: String = existing_topics
			.iter()
			.map(|t| format!("- {}", t))
			.collect::<Vec<_>>()
			.join("\n");

		let prompt = format!(
			r#"Classify the following text into the most appropriate topic.

Text: {}

Existing topics:
{}

Return a JSON object:
{{"topic": "best/topic/path", "confidence": 0.0-1.0}}

You may suggest a new subtopic if none of the existing ones fit well.
Respond with ONLY valid JSON."#,
			text, topics_list
		);

		let response = match self.text_generator.generate(&prompt, None).await {
			Ok(r) => r,
			Err(_) => {
				return ClassificationResult {
					topic: "uncategorized".to_string(),
					confidence: 0.0,
				}
			}
		};

		match serde_json::from_str::<ClassificationJson>(&response) {
			Ok(parsed) => ClassificationResult {
				topic: parsed.topic.unwrap_or_else(|| "uncategorized".to_string()),
				confidence: parsed.confidence.unwrap_or(0.0),
			},
			Err(_) => ClassificationResult {
				topic: "uncategorized".to_string(),
				confidence: 0.0,
			},
		}
	}

	// -----------------------------------------------------------------------
	// reorganize
	// -----------------------------------------------------------------------

	/// Suggest reorganization of volumes within a topic.
	///
	/// Returns an empty plan on LLM or parse failure.
	pub async fn reorganize(
		&self,
		topic: &str,
		volumes: &[Volume],
	) -> ReorganizationPlan {
		let volume_list: String = volumes
			.iter()
			.map(|v| format!("- [{}] {}", v.id, v.text))
			.collect::<Vec<_>>()
			.join("\n");

		let prompt = format!(
			r#"Review the following volumes in topic "{}" and suggest reorganization.

Volumes:
{}

Return a JSON object:
{{
  "moves": [{{"volumeId": "id", "newTopic": "new/topic/path"}}],
  "newSubtopics": ["new/subtopic"],
  "merges": [{{"source": "topic/a", "target": "topic/b"}}]
}}

Respond with ONLY valid JSON."#,
			topic, volume_list
		);

		let response = match self.text_generator.generate(&prompt, None).await {
			Ok(r) => r,
			Err(_) => return ReorganizationPlan::empty(),
		};

		match serde_json::from_str::<ReorgJson>(&response) {
			Ok(parsed) => Self::parse_reorg(parsed),
			Err(_) => ReorganizationPlan::empty(),
		}
	}

	// -----------------------------------------------------------------------
	// optimize
	// -----------------------------------------------------------------------

	/// Prune, summarize, and reorganize volumes using a (potentially more
	/// powerful) model.
	///
	/// Falls back to the default generator if no model-specific generator
	/// is configured.
	pub async fn optimize(
		&self,
		volumes: &[Volume],
		topic: &str,
		model_id: &str,
	) -> OptimizationResult {
		let volume_list: String = volumes
			.iter()
			.map(|v| format!("- [{}] {}", v.id, v.text))
			.collect::<Vec<_>>()
			.join("\n");

		let prompt = format!(
			r#"You are a memory optimization agent. Analyze the following volumes in topic "{}" and perform maintenance.

Volumes:
{}

Tasks:
1. PRUNE: Identify volume IDs that are redundant, outdated, or low-value. List their IDs.
2. SUMMARIZE: Write a single concise summary that preserves all important information from the remaining (non-pruned) volumes.
3. REORGANIZE: Suggest any topic restructuring (moves, new subtopics, merges).

Return a JSON object:
{{
  "pruned": ["id1", "id2"],
  "summary": "concise summary text",
  "reorganization": {{
    "moves": [{{"volumeId": "id", "newTopic": "new/topic"}}],
    "newSubtopics": ["new/subtopic"],
    "merges": [{{"source": "topic/a", "target": "topic/b"}}]
  }}
}}

Respond with ONLY valid JSON."#,
			topic, volume_list
		);

		let response = if let Some(ref model_gen) = self.model_generator {
			match model_gen.generate_with_model(&prompt, model_id).await {
				Ok(r) => r,
				Err(_) => return OptimizationResult::empty(model_id),
			}
		} else {
			match self.text_generator.generate(&prompt, None).await {
				Ok(r) => r,
				Err(_) => return OptimizationResult::empty(model_id),
			}
		};

		match serde_json::from_str::<OptimizationJson>(&response) {
			Ok(parsed) => {
				let reorganization = parsed
					.reorganization
					.map(Self::parse_reorg)
					.unwrap_or_else(ReorganizationPlan::empty);

				OptimizationResult {
					pruned: parsed.pruned.unwrap_or_default(),
					summary: parsed.summary.unwrap_or_default(),
					reorganization,
					model_used: model_id.to_string(),
				}
			}
			Err(_) => OptimizationResult::empty(model_id),
		}
	}

	// -----------------------------------------------------------------------
	// bid
	// -----------------------------------------------------------------------

	/// Score confidence for managing a given topic/content pair.
	///
	/// The `_library` parameter is reserved for future use (enriching the
	/// bid prompt with existing volume context). Currently unused.
	///
	/// Returns a zero-confidence bid on LLM or parse failure.
	pub async fn bid(
		&self,
		content: &str,
		topic: &str,
		_library: &dyn LibrarianLibraryAccess,
	) -> LibrarianBid {
		let preview = truncate_str(content, 500);

		let prompt = format!(
			r#"You are a specialist librarian named "{}".
Purpose: {}

Given the following content and topic, assess how well-suited you are to manage it.

Topic: {}
Content preview: {}

Return ONLY valid JSON:
{{"argument": "brief reason why you should manage this", "confidence": 0.0-1.0}}"#,
			self.name, self.purpose, topic, preview
		);

		let response = match self.text_generator.generate(&prompt, None).await {
			Ok(r) => r,
			Err(_) => {
				return LibrarianBid {
					librarian_name: self.name.clone(),
					argument: String::new(),
					confidence: 0.0,
				}
			}
		};

		match serde_json::from_str::<BidJson>(&response) {
			Ok(parsed) => {
				let raw_confidence = parsed.confidence.unwrap_or(0.0);
				LibrarianBid {
					librarian_name: self.name.clone(),
					argument: parsed.argument.unwrap_or_default(),
					confidence: raw_confidence.clamp(0.0, 1.0),
				}
			}
			Err(_) => LibrarianBid {
				librarian_name: self.name.clone(),
				argument: String::new(),
				confidence: 0.0,
			},
		}
	}

	// -----------------------------------------------------------------------
	// Internal helpers
	// -----------------------------------------------------------------------

	fn parse_reorg(json: ReorgJson) -> ReorganizationPlan {
		let moves = json
			.moves
			.unwrap_or_default()
			.into_iter()
			.filter_map(|m| {
				Some(ReorganizationMove {
					volume_id: m.volume_id?,
					new_topic: m.new_topic?,
				})
			})
			.collect();

		let new_subtopics = json.new_subtopics.unwrap_or_default();

		let merges = json
			.merges
			.unwrap_or_default()
			.into_iter()
			.filter_map(|m| {
				Some(ReorganizationMerge {
					source: m.source?,
					target: m.target?,
				})
			})
			.collect();

		ReorganizationPlan {
			moves,
			new_subtopics,
			merges,
		}
	}
}

// ---------------------------------------------------------------------------
// Convenience constructors
// ---------------------------------------------------------------------------

impl ReorganizationPlan {
	/// Create an empty reorganization plan.
	pub fn empty() -> Self {
		Self {
			moves: Vec::new(),
			new_subtopics: Vec::new(),
			merges: Vec::new(),
		}
	}
}

impl OptimizationResult {
	/// Create an empty optimization result.
	pub fn empty(model_id: &str) -> Self {
		Self {
			pruned: Vec::new(),
			summary: String::new(),
			reorganization: ReorganizationPlan::empty(),
			model_used: model_id.to_string(),
		}
	}
}

/// Convenience factory that creates a default Librarian from a TextGenerationProvider.
pub fn create_default_librarian(provider: Arc<dyn TextGenerationProvider>) -> Librarian {
	Librarian::new(provider, None)
}
