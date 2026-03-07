//! Tests for the librarian, librarian definition, librarian registry,
//! circulation desk, and library services modules.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use simse_core::error::SimseError;
use simse_core::library::librarian::{
	EntryType, Librarian, LibrarianLibraryAccess, LibrarianOptions, OptimizationResult,
	ReorganizationPlan, TextGenerationWithModel, TurnContext, Volume,
};
use simse_core::library::librarian_def::{
	default_definition, load_all_definitions, load_definition, matches_topic, save_definition,
	validate_definition, LibrarianAcp, LibrarianDefinition, LibrarianPermissions,
	LibrarianThresholds,
};
use simse_core::library::librarian_reg::{LibrarianRegistry, LibrarianRegistryOptions};
use simse_core::library::TextGenerationProvider;

// ---------------------------------------------------------------------------
// Mock providers
// ---------------------------------------------------------------------------

/// Mock text generator that returns JSON extraction results.
struct ExtractionGenerator;

#[async_trait]
impl TextGenerationProvider for ExtractionGenerator {
	async fn generate(
		&self,
		_prompt: &str,
		_system_prompt: Option<&str>,
	) -> Result<String, SimseError> {
		Ok(r#"{"memories": [
			{"text": "Rust is a systems language", "topic": "programming/rust", "tags": ["rust", "language"], "entryType": "fact"},
			{"text": "Use cargo for builds", "topic": "programming/rust/tooling", "tags": ["cargo"], "entryType": "observation"}
		]}"#.to_string())
	}
}

/// Mock text generator that returns JSON classification results.
struct ClassificationGenerator;

#[async_trait]
impl TextGenerationProvider for ClassificationGenerator {
	async fn generate(
		&self,
		_prompt: &str,
		_system_prompt: Option<&str>,
	) -> Result<String, SimseError> {
		Ok(r#"{"topic": "programming/rust", "confidence": 0.85}"#.to_string())
	}
}

/// Mock text generator that returns JSON reorganization results.
struct ReorgGenerator;

#[async_trait]
impl TextGenerationProvider for ReorgGenerator {
	async fn generate(
		&self,
		_prompt: &str,
		_system_prompt: Option<&str>,
	) -> Result<String, SimseError> {
		Ok(r#"{
			"moves": [{"volumeId": "vol-1", "newTopic": "programming/go"}],
			"newSubtopics": ["programming/rust/async"],
			"merges": [{"source": "lang/rust", "target": "programming/rust"}]
		}"#
		.to_string())
	}
}

/// Mock text generator that returns JSON optimization results.
struct OptimizeGenerator;

#[async_trait]
impl TextGenerationProvider for OptimizeGenerator {
	async fn generate(
		&self,
		_prompt: &str,
		_system_prompt: Option<&str>,
	) -> Result<String, SimseError> {
		Ok(r#"{
			"pruned": ["vol-old"],
			"summary": "Rust is great for systems programming",
			"reorganization": {
				"moves": [],
				"newSubtopics": [],
				"merges": []
			}
		}"#
		.to_string())
	}
}

/// Mock text generator that returns JSON bid results.
struct BidGenerator {
	confidence: f64,
}

#[async_trait]
impl TextGenerationProvider for BidGenerator {
	async fn generate(
		&self,
		_prompt: &str,
		_system_prompt: Option<&str>,
	) -> Result<String, SimseError> {
		Ok(format!(
			r#"{{"argument": "I am well-suited for this topic", "confidence": {}}}"#,
			self.confidence
		))
	}
}

/// Mock text generator that always fails.
struct FailingGenerator;

#[async_trait]
impl TextGenerationProvider for FailingGenerator {
	async fn generate(
		&self,
		_prompt: &str,
		_system_prompt: Option<&str>,
	) -> Result<String, SimseError> {
		Err(SimseError::other("generation failed"))
	}
}

/// Mock text generator that returns invalid JSON.
struct InvalidJsonGenerator;

#[async_trait]
impl TextGenerationProvider for InvalidJsonGenerator {
	async fn generate(
		&self,
		_prompt: &str,
		_system_prompt: Option<&str>,
	) -> Result<String, SimseError> {
		Ok("not valid json {{{".to_string())
	}
}

/// Mock text generator that returns plain text (for summarize).
struct SummarizeGenerator;

#[async_trait]
impl TextGenerationProvider for SummarizeGenerator {
	async fn generate(
		&self,
		_prompt: &str,
		_system_prompt: Option<&str>,
	) -> Result<String, SimseError> {
		Ok("Summary of the volumes".to_string())
	}
}

/// Mock library access for bidding.
struct MockLibraryAccess;

#[async_trait]
impl LibrarianLibraryAccess for MockLibraryAccess {
	async fn search(
		&self,
		_query: &str,
		_max_results: Option<usize>,
	) -> Result<Vec<Volume>, SimseError> {
		Ok(Vec::new())
	}
	async fn get_topics(&self) -> Result<Vec<String>, SimseError> {
		Ok(vec!["programming/rust".to_string()])
	}
	async fn filter_by_topic(&self, _topics: &[String]) -> Result<Vec<Volume>, SimseError> {
		Ok(Vec::new())
	}
}

/// Mock text generation with model support.
struct ModelGenerator;

#[async_trait]
impl TextGenerationProvider for ModelGenerator {
	async fn generate(
		&self,
		_prompt: &str,
		_system_prompt: Option<&str>,
	) -> Result<String, SimseError> {
		Ok("default model response".to_string())
	}
}

#[async_trait]
impl TextGenerationWithModel for ModelGenerator {
	async fn generate_with_model(
		&self,
		_prompt: &str,
		model_id: &str,
	) -> Result<String, SimseError> {
		Ok(format!(
			r#"{{"pruned": ["old-1"], "summary": "optimized with {}", "reorganization": {{"moves": [], "newSubtopics": [], "merges": []}}}}"#,
			model_id
		))
	}
}

// ===========================================================================
// Librarian tests
// ===========================================================================

#[tokio::test]
async fn extract_parses_valid_json() {
	let generator = Arc::new(ExtractionGenerator);
	let librarian = Librarian::new(generator, None);

	let turn = TurnContext {
		user_input: "Tell me about Rust".to_string(),
		response: "Rust is a systems programming language.".to_string(),
	};

	let result = librarian.extract(&turn).await;
	assert_eq!(result.memories.len(), 2);
	assert_eq!(result.memories[0].text, "Rust is a systems language");
	assert_eq!(result.memories[0].topic, "programming/rust");
	assert_eq!(result.memories[0].entry_type, EntryType::Fact);
	assert_eq!(result.memories[0].tags, vec!["rust", "language"]);
	assert_eq!(result.memories[1].text, "Use cargo for builds");
	assert_eq!(result.memories[1].entry_type, EntryType::Observation);
}

#[tokio::test]
async fn extract_returns_empty_on_failure() {
	let generator = Arc::new(FailingGenerator);
	let librarian = Librarian::new(generator, None);

	let turn = TurnContext {
		user_input: "Hello".to_string(),
		response: "Hi there".to_string(),
	};

	let result = librarian.extract(&turn).await;
	assert!(result.memories.is_empty());
}

#[tokio::test]
async fn extract_returns_empty_on_invalid_json() {
	let generator = Arc::new(InvalidJsonGenerator);
	let librarian = Librarian::new(generator, None);

	let turn = TurnContext {
		user_input: "Hello".to_string(),
		response: "Hi there".to_string(),
	};

	let result = librarian.extract(&turn).await;
	assert!(result.memories.is_empty());
}

#[tokio::test]
async fn summarize_returns_text_and_source_ids() {
	let generator = Arc::new(SummarizeGenerator);
	let librarian = Librarian::new(generator, None);

	let volumes = vec![
		Volume {
			id: "v1".to_string(),
			text: "First volume".to_string(),
		},
		Volume {
			id: "v2".to_string(),
			text: "Second volume".to_string(),
		},
	];

	let result = librarian.summarize(&volumes, "test-topic").await.unwrap();
	assert_eq!(result.text, "Summary of the volumes");
	assert_eq!(result.source_ids, vec!["v1", "v2"]);
}

#[tokio::test]
async fn summarize_propagates_errors() {
	let generator = Arc::new(FailingGenerator);
	let librarian = Librarian::new(generator, None);

	let result = librarian.summarize(&[], "topic").await;
	assert!(result.is_err());
}

#[tokio::test]
async fn classify_topic_parses_valid_json() {
	let generator = Arc::new(ClassificationGenerator);
	let librarian = Librarian::new(generator, None);

	let result = librarian
		.classify_topic("Some text about Rust", &["programming/rust".to_string()])
		.await;
	assert_eq!(result.topic, "programming/rust");
	assert!((result.confidence - 0.85).abs() < 0.001);
}

#[tokio::test]
async fn classify_topic_returns_fallback_on_failure() {
	let generator = Arc::new(FailingGenerator);
	let librarian = Librarian::new(generator, None);

	let result = librarian.classify_topic("text", &[]).await;
	assert_eq!(result.topic, "uncategorized");
	assert_eq!(result.confidence, 0.0);
}

#[tokio::test]
async fn classify_topic_returns_fallback_on_invalid_json() {
	let generator = Arc::new(InvalidJsonGenerator);
	let librarian = Librarian::new(generator, None);

	let result = librarian.classify_topic("text", &[]).await;
	assert_eq!(result.topic, "uncategorized");
	assert_eq!(result.confidence, 0.0);
}

#[tokio::test]
async fn reorganize_parses_valid_json() {
	let generator = Arc::new(ReorgGenerator);
	let librarian = Librarian::new(generator, None);

	let volumes = vec![Volume {
		id: "vol-1".to_string(),
		text: "Some content".to_string(),
	}];

	let result = librarian.reorganize("programming/rust", &volumes).await;
	assert_eq!(result.moves.len(), 1);
	assert_eq!(result.moves[0].volume_id, "vol-1");
	assert_eq!(result.moves[0].new_topic, "programming/go");
	assert_eq!(result.new_subtopics, vec!["programming/rust/async"]);
	assert_eq!(result.merges.len(), 1);
	assert_eq!(result.merges[0].source, "lang/rust");
	assert_eq!(result.merges[0].target, "programming/rust");
}

#[tokio::test]
async fn reorganize_returns_empty_on_failure() {
	let generator = Arc::new(FailingGenerator);
	let librarian = Librarian::new(generator, None);

	let result = librarian.reorganize("topic", &[]).await;
	assert!(result.moves.is_empty());
	assert!(result.new_subtopics.is_empty());
	assert!(result.merges.is_empty());
}

#[tokio::test]
async fn optimize_parses_valid_json() {
	let generator = Arc::new(OptimizeGenerator);
	let librarian = Librarian::new(generator, None);

	let volumes = vec![
		Volume {
			id: "vol-1".to_string(),
			text: "content 1".to_string(),
		},
		Volume {
			id: "vol-old".to_string(),
			text: "outdated content".to_string(),
		},
	];

	let result = librarian.optimize(&volumes, "rust", "gpt-4").await;
	assert_eq!(result.pruned, vec!["vol-old"]);
	assert_eq!(result.summary, "Rust is great for systems programming");
	assert_eq!(result.model_used, "gpt-4");
}

#[tokio::test]
async fn optimize_returns_empty_on_failure() {
	let generator = Arc::new(FailingGenerator);
	let librarian = Librarian::new(generator, None);

	let result = librarian.optimize(&[], "topic", "model-1").await;
	assert!(result.pruned.is_empty());
	assert!(result.summary.is_empty());
	assert_eq!(result.model_used, "model-1");
}

#[tokio::test]
async fn optimize_with_model_generator() {
	let generator = Arc::new(ModelGenerator);
	let model_gen = Arc::new(ModelGenerator);
	let librarian =
		Librarian::new(generator, None).with_model_generator(model_gen as Arc<dyn TextGenerationWithModel>);

	let volumes = vec![Volume {
		id: "v1".to_string(),
		text: "content".to_string(),
	}];

	let result = librarian.optimize(&volumes, "rust", "claude-3").await;
	assert_eq!(result.pruned, vec!["old-1"]);
	assert!(result.summary.contains("claude-3"));
	assert_eq!(result.model_used, "claude-3");
}

#[tokio::test]
async fn bid_parses_valid_json() {
	let generator = Arc::new(BidGenerator { confidence: 0.75 });
	let librarian = Librarian::new(
		generator,
		Some(LibrarianOptions {
			name: Some("rust-expert".to_string()),
			purpose: Some("Manages Rust topics".to_string()),
		}),
	);

	let lib_access = MockLibraryAccess;
	let result = librarian.bid("Rust content", "programming/rust", &lib_access).await;
	assert_eq!(result.librarian_name, "rust-expert");
	assert_eq!(result.argument, "I am well-suited for this topic");
	assert!((result.confidence - 0.75).abs() < 0.001);
}

#[tokio::test]
async fn bid_clamps_confidence() {
	let generator = Arc::new(BidGenerator { confidence: 1.5 });
	let librarian = Librarian::new(generator, None);

	let lib_access = MockLibraryAccess;
	let result = librarian.bid("content", "topic", &lib_access).await;
	assert!(result.confidence <= 1.0);
}

#[tokio::test]
async fn bid_returns_zero_on_failure() {
	let generator = Arc::new(FailingGenerator);
	let librarian = Librarian::new(generator, None);

	let lib_access = MockLibraryAccess;
	let result = librarian.bid("content", "topic", &lib_access).await;
	assert_eq!(result.confidence, 0.0);
	assert!(result.argument.is_empty());
}

#[tokio::test]
async fn librarian_name_and_purpose() {
	let generator = Arc::new(FailingGenerator);
	let librarian = Librarian::new(
		generator,
		Some(LibrarianOptions {
			name: Some("my-lib".to_string()),
			purpose: Some("My purpose".to_string()),
		}),
	);
	assert_eq!(librarian.name(), "my-lib");
	assert_eq!(librarian.purpose(), "My purpose");
}

#[tokio::test]
async fn librarian_defaults() {
	let generator = Arc::new(FailingGenerator);
	let librarian = Librarian::new(generator, None);
	assert_eq!(librarian.name(), "default");
	assert_eq!(librarian.purpose(), "General-purpose librarian");
}

// ===========================================================================
// LibrarianDefinition tests
// ===========================================================================

fn valid_definition() -> LibrarianDefinition {
	LibrarianDefinition {
		name: "rust-expert".to_string(),
		description: "Expert in Rust programming".to_string(),
		purpose: "Manages Rust-related knowledge".to_string(),
		topics: vec!["programming/rust".to_string(), "programming/rust/**".to_string()],
		permissions: LibrarianPermissions {
			add: true,
			delete: true,
			reorganize: true,
		},
		thresholds: LibrarianThresholds {
			topic_complexity: 50.0,
			escalate_at: 100.0,
		},
		acp: None,
	}
}

#[test]
fn validate_valid_definition() {
	let result = validate_definition(&valid_definition());
	assert!(result.valid);
	assert!(result.errors.is_empty());
}

#[test]
fn validate_empty_name() {
	let mut def = valid_definition();
	def.name = String::new();
	let result = validate_definition(&def);
	assert!(!result.valid);
	assert!(result.errors.iter().any(|e| e.contains("name")));
}

#[test]
fn validate_invalid_name_pattern() {
	let mut def = valid_definition();
	def.name = "Invalid Name".to_string();
	let result = validate_definition(&def);
	assert!(!result.valid);
	assert!(result.errors.iter().any(|e| e.contains("kebab-case")));
}

#[test]
fn validate_name_starting_with_hyphen() {
	let mut def = valid_definition();
	def.name = "-invalid".to_string();
	let result = validate_definition(&def);
	assert!(!result.valid);
}

#[test]
fn validate_empty_description() {
	let mut def = valid_definition();
	def.description = String::new();
	let result = validate_definition(&def);
	assert!(!result.valid);
	assert!(result.errors.iter().any(|e| e.contains("description")));
}

#[test]
fn validate_empty_purpose() {
	let mut def = valid_definition();
	def.purpose = String::new();
	let result = validate_definition(&def);
	assert!(!result.valid);
	assert!(result.errors.iter().any(|e| e.contains("purpose")));
}

#[test]
fn validate_empty_topics() {
	let mut def = valid_definition();
	def.topics = Vec::new();
	let result = validate_definition(&def);
	assert!(!result.valid);
	assert!(result.errors.iter().any(|e| e.contains("topics")));
}

#[test]
fn validate_negative_thresholds() {
	let mut def = valid_definition();
	def.thresholds.topic_complexity = -1.0;
	def.thresholds.escalate_at = 0.0;
	let result = validate_definition(&def);
	assert!(!result.valid);
	assert!(result.errors.len() >= 2);
}

#[test]
fn validate_empty_acp_command() {
	let mut def = valid_definition();
	def.acp = Some(LibrarianAcp {
		command: String::new(),
		args: None,
		agent_id: None,
	});
	let result = validate_definition(&def);
	assert!(!result.valid);
	assert!(result.errors.iter().any(|e| e.contains("acp.command")));
}

#[test]
fn validate_valid_acp() {
	let mut def = valid_definition();
	def.acp = Some(LibrarianAcp {
		command: "run-agent".to_string(),
		args: Some(vec!["--port".to_string(), "8080".to_string()]),
		agent_id: Some("agent-1".to_string()),
	});
	let result = validate_definition(&def);
	assert!(result.valid);
}

#[test]
fn validate_multiple_errors() {
	let def = LibrarianDefinition {
		name: String::new(),
		description: String::new(),
		purpose: String::new(),
		topics: Vec::new(),
		permissions: LibrarianPermissions {
			add: true,
			delete: true,
			reorganize: true,
		},
		thresholds: LibrarianThresholds {
			topic_complexity: 0.0,
			escalate_at: 0.0,
		},
		acp: None,
	};
	let result = validate_definition(&def);
	assert!(!result.valid);
	// Should have at least 5 errors: name, description, purpose, topics, 2x thresholds
	assert!(result.errors.len() >= 5);
}

// ===========================================================================
// Topic matching tests
// ===========================================================================

#[test]
fn matches_exact_topic() {
	assert!(matches_topic(
		&["programming/rust".to_string()],
		"programming/rust"
	));
}

#[test]
fn no_match_different_topic() {
	assert!(!matches_topic(
		&["programming/rust".to_string()],
		"programming/go"
	));
}

#[test]
fn wildcard_star_matches_one_level() {
	assert!(matches_topic(
		&["programming/*".to_string()],
		"programming/rust"
	));
}

#[test]
fn wildcard_star_does_not_match_deeper() {
	assert!(!matches_topic(
		&["programming/*".to_string()],
		"programming/rust/async"
	));
}

#[test]
fn double_star_matches_recursive() {
	assert!(matches_topic(
		&["programming/**".to_string()],
		"programming/rust/async/tokio"
	));
}

#[test]
fn double_star_matches_one_level() {
	assert!(matches_topic(
		&["programming/**".to_string()],
		"programming/rust"
	));
}

#[test]
fn double_star_matches_zero_levels() {
	// `**` should match zero additional segments after the prefix
	assert!(matches_topic(&["**".to_string()], "anything"));
}

#[test]
fn double_star_matches_everything() {
	assert!(matches_topic(&["**".to_string()], "a/b/c/d"));
}

#[test]
fn multiple_patterns_any_match() {
	assert!(matches_topic(
		&["code/rust".to_string(), "code/go".to_string()],
		"code/go"
	));
}

#[test]
fn no_patterns_no_match() {
	assert!(!matches_topic(&[], "anything"));
}

// ===========================================================================
// Persistence tests
// ===========================================================================

#[tokio::test]
async fn save_and_load_definition() {
	let dir = tempfile::tempdir().unwrap();
	let def = valid_definition();

	save_definition(dir.path(), &def).await.unwrap();
	let loaded = load_definition(dir.path(), "rust-expert").await;
	assert!(loaded.is_some());
	let loaded = loaded.unwrap();
	assert_eq!(loaded.name, "rust-expert");
	assert_eq!(loaded.description, def.description);
	assert_eq!(loaded.topics, def.topics);
}

#[tokio::test]
async fn load_nonexistent_definition() {
	let dir = tempfile::tempdir().unwrap();
	let loaded = load_definition(dir.path(), "nonexistent").await;
	assert!(loaded.is_none());
}

#[tokio::test]
async fn load_all_definitions_from_dir() {
	let dir = tempfile::tempdir().unwrap();

	let def1 = valid_definition();
	let mut def2 = valid_definition();
	def2.name = "go-expert".to_string();
	def2.description = "Expert in Go".to_string();
	def2.topics = vec!["programming/go".to_string()];

	save_definition(dir.path(), &def1).await.unwrap();
	save_definition(dir.path(), &def2).await.unwrap();

	let all = load_all_definitions(dir.path()).await;
	assert_eq!(all.len(), 2);
}

#[tokio::test]
async fn load_all_skips_invalid() {
	let dir = tempfile::tempdir().unwrap();

	// Write a valid definition
	let def = valid_definition();
	save_definition(dir.path(), &def).await.unwrap();

	// Write invalid JSON
	tokio::fs::write(dir.path().join("invalid.json"), "not json {")
		.await
		.unwrap();

	let all = load_all_definitions(dir.path()).await;
	assert_eq!(all.len(), 1);
}

#[tokio::test]
async fn load_all_empty_dir() {
	let dir = tempfile::tempdir().unwrap();
	let all = load_all_definitions(dir.path()).await;
	assert!(all.is_empty());
}

#[tokio::test]
async fn load_all_nonexistent_dir() {
	let dir = std::path::PathBuf::from("/tmp/simse_test_nonexistent_dir_xyz");
	let all = load_all_definitions(&dir).await;
	assert!(all.is_empty());
}

// ===========================================================================
// Default definition tests
// ===========================================================================

#[test]
fn default_definition_is_valid() {
	let def = default_definition();
	let result = validate_definition(&def);
	assert!(result.valid);
	assert_eq!(def.name, "default");
	assert_eq!(def.topics, vec!["**"]);
}

// ===========================================================================
// Serialization roundtrip tests
// ===========================================================================

#[test]
fn definition_serde_roundtrip() {
	let def = valid_definition();
	let json = serde_json::to_string_pretty(&def).unwrap();
	let parsed: LibrarianDefinition = serde_json::from_str(&json).unwrap();
	assert_eq!(def, parsed);
}

#[test]
fn definition_with_acp_serde_roundtrip() {
	let mut def = valid_definition();
	def.acp = Some(LibrarianAcp {
		command: "run-agent".to_string(),
		args: Some(vec!["--port".to_string(), "8080".to_string()]),
		agent_id: Some("agent-1".to_string()),
	});
	let json = serde_json::to_string_pretty(&def).unwrap();
	let parsed: LibrarianDefinition = serde_json::from_str(&json).unwrap();
	assert_eq!(def, parsed);
}

// ===========================================================================
// LibrarianRegistry tests
// ===========================================================================

#[tokio::test]
async fn registry_initialize_creates_default() {
	let dir = tempfile::tempdir().unwrap();
	let generator = Arc::new(FailingGenerator);
	let lib_access = Arc::new(MockLibraryAccess);

	let registry = LibrarianRegistry::new(LibrarianRegistryOptions::new(
		dir.path().to_path_buf(),
		lib_access,
		generator,
	));

	registry.initialize().await.unwrap();

	let default = registry.get("default").await;
	assert!(default.is_some());
	assert_eq!(default.unwrap().definition.name, "default");
}

#[tokio::test]
async fn registry_initialize_is_idempotent() {
	let dir = tempfile::tempdir().unwrap();
	let generator = Arc::new(FailingGenerator);
	let lib_access = Arc::new(MockLibraryAccess);

	let registry = LibrarianRegistry::new(LibrarianRegistryOptions::new(
		dir.path().to_path_buf(),
		lib_access,
		generator,
	));

	registry.initialize().await.unwrap();
	registry.initialize().await.unwrap(); // Should not fail
}

#[tokio::test]
async fn registry_register_and_list() {
	let dir = tempfile::tempdir().unwrap();
	let generator = Arc::new(FailingGenerator);
	let lib_access = Arc::new(MockLibraryAccess);

	let registry = LibrarianRegistry::new(LibrarianRegistryOptions::new(
		dir.path().to_path_buf(),
		lib_access,
		generator,
	));
	registry.initialize().await.unwrap();

	let def = valid_definition();
	let managed = registry.register(def).await.unwrap();
	assert_eq!(managed.definition.name, "rust-expert");

	let list = registry.list().await;
	assert_eq!(list.len(), 2); // default + rust-expert
}

#[tokio::test]
async fn registry_register_persists_to_disk() {
	let dir = tempfile::tempdir().unwrap();
	let generator = Arc::new(FailingGenerator);
	let lib_access = Arc::new(MockLibraryAccess);

	let registry = LibrarianRegistry::new(LibrarianRegistryOptions::new(
		dir.path().to_path_buf(),
		lib_access,
		generator,
	));
	registry.initialize().await.unwrap();

	let def = valid_definition();
	registry.register(def).await.unwrap();

	// Check file exists on disk
	let file = dir.path().join("rust-expert.json");
	assert!(file.exists());
}

#[tokio::test]
async fn registry_register_rejects_invalid() {
	let dir = tempfile::tempdir().unwrap();
	let generator = Arc::new(FailingGenerator);
	let lib_access = Arc::new(MockLibraryAccess);

	let registry = LibrarianRegistry::new(LibrarianRegistryOptions::new(
		dir.path().to_path_buf(),
		lib_access,
		generator,
	));
	registry.initialize().await.unwrap();

	let mut def = valid_definition();
	def.name = String::new(); // Invalid
	let result = registry.register(def).await;
	assert!(result.is_err());
}

#[tokio::test]
async fn registry_register_rejects_duplicate() {
	let dir = tempfile::tempdir().unwrap();
	let generator = Arc::new(FailingGenerator);
	let lib_access = Arc::new(MockLibraryAccess);

	let registry = LibrarianRegistry::new(LibrarianRegistryOptions::new(
		dir.path().to_path_buf(),
		lib_access,
		generator,
	));
	registry.initialize().await.unwrap();

	let def = valid_definition();
	registry.register(def.clone()).await.unwrap();
	let result = registry.register(def).await;
	assert!(result.is_err());
}

#[tokio::test]
async fn registry_unregister() {
	let dir = tempfile::tempdir().unwrap();
	let generator = Arc::new(FailingGenerator);
	let lib_access = Arc::new(MockLibraryAccess);

	let registry = LibrarianRegistry::new(LibrarianRegistryOptions::new(
		dir.path().to_path_buf(),
		lib_access,
		generator,
	));
	registry.initialize().await.unwrap();

	let def = valid_definition();
	registry.register(def).await.unwrap();

	registry.unregister("rust-expert").await.unwrap();
	assert!(registry.get("rust-expert").await.is_none());
}

#[tokio::test]
async fn registry_cannot_unregister_default() {
	let dir = tempfile::tempdir().unwrap();
	let generator = Arc::new(FailingGenerator);
	let lib_access = Arc::new(MockLibraryAccess);

	let registry = LibrarianRegistry::new(LibrarianRegistryOptions::new(
		dir.path().to_path_buf(),
		lib_access,
		generator,
	));
	registry.initialize().await.unwrap();

	let result = registry.unregister("default").await;
	assert!(result.is_err());
}

#[tokio::test]
async fn registry_loads_from_disk_on_init() {
	let dir = tempfile::tempdir().unwrap();

	// Pre-save a definition to disk
	let def = valid_definition();
	save_definition(dir.path(), &def).await.unwrap();

	let generator = Arc::new(FailingGenerator);
	let lib_access = Arc::new(MockLibraryAccess);

	let registry = LibrarianRegistry::new(LibrarianRegistryOptions::new(
		dir.path().to_path_buf(),
		lib_access,
		generator,
	));
	registry.initialize().await.unwrap();

	let list = registry.list().await;
	assert_eq!(list.len(), 2); // default + rust-expert loaded from disk
}

#[tokio::test]
async fn registry_resolve_no_match_returns_default() {
	let dir = tempfile::tempdir().unwrap();
	let generator = Arc::new(BidGenerator { confidence: 0.5 });
	let lib_access = Arc::new(MockLibraryAccess);

	let registry = LibrarianRegistry::new(LibrarianRegistryOptions::new(
		dir.path().to_path_buf(),
		lib_access,
		generator,
	));
	registry.initialize().await.unwrap();

	// Register a specialist that only matches "data/**"
	let mut def = valid_definition();
	def.topics = vec!["data/**".to_string()];
	registry.register(def).await.unwrap();

	// Resolve for a topic that doesn't match the specialist
	let result = registry.resolve_librarian("music/jazz", "some content").await;
	// Only default matches (it has "**" pattern), so should return default directly
	assert_eq!(result.winner, "default");
}

#[tokio::test]
async fn registry_resolve_single_match() {
	let dir = tempfile::tempdir().unwrap();
	let generator = Arc::new(BidGenerator { confidence: 0.8 });
	let lib_access = Arc::new(MockLibraryAccess);

	let mut opts = LibrarianRegistryOptions::new(
		dir.path().to_path_buf(),
		lib_access,
		generator,
	);
	// Use a very high gap so self-resolution triggers easily
	opts.self_resolution_gap = 0.1;

	let registry = LibrarianRegistry::new(opts);
	registry.initialize().await.unwrap();

	// Note: default matches "**" which includes everything,
	// so when there are no other registered librarians,
	// the default is the only match and should be returned directly.
	let result = registry.resolve_librarian("any/topic", "content").await;
	assert_eq!(result.winner, "default");
	assert!(result.bids.is_empty()); // single match, no bidding
}

#[tokio::test]
async fn registry_dispose() {
	let dir = tempfile::tempdir().unwrap();
	let generator = Arc::new(FailingGenerator);
	let lib_access = Arc::new(MockLibraryAccess);

	let registry = LibrarianRegistry::new(LibrarianRegistryOptions::new(
		dir.path().to_path_buf(),
		lib_access,
		generator,
	));
	registry.initialize().await.unwrap();

	registry.dispose().await;

	let list = registry.list().await;
	assert!(list.is_empty());
}

// ===========================================================================
// CirculationDesk tests
// ===========================================================================

use simse_core::library::circulation::{
	CirculationDesk, CirculationDeskOptions, CirculationLibraryOps, CirculationThresholds,
	DuplicateCheckResult,
};

struct MockLibraryOps {
	added: tokio::sync::Mutex<Vec<(String, HashMap<String, String>)>>,
}

impl MockLibraryOps {
	fn new() -> Self {
		Self {
			added: tokio::sync::Mutex::new(Vec::new()),
		}
	}
}

#[async_trait]
impl CirculationLibraryOps for MockLibraryOps {
	async fn add_volume(
		&self,
		text: &str,
		metadata: HashMap<String, String>,
	) -> Result<String, SimseError> {
		let mut added = self.added.lock().await;
		added.push((text.to_string(), metadata));
		Ok(format!("vol-{}", added.len()))
	}

	async fn check_duplicate(&self, _text: &str) -> Result<DuplicateCheckResult, SimseError> {
		Ok(DuplicateCheckResult { is_duplicate: false })
	}

	async fn get_volumes_for_topic(&self, _topic: &str) -> Result<Vec<Volume>, SimseError> {
		Ok(Vec::new())
	}

	async fn delete_volume(&self, _id: &str) -> Result<(), SimseError> {
		Ok(())
	}

	async fn get_total_volume_count(&self) -> Result<usize, SimseError> {
		Ok(0)
	}

	async fn get_all_topics(&self) -> Result<Vec<String>, SimseError> {
		Ok(Vec::new())
	}
}

#[tokio::test]
async fn circulation_desk_requires_librarian_or_registry() {
	let ops: Arc<dyn CirculationLibraryOps> = Arc::new(MockLibraryOps::new());
	let result = CirculationDesk::new(CirculationDeskOptions {
		librarian: None,
		registry: None,
		library_ops: ops,
		thresholds: CirculationThresholds::default(),
		catalog: None,
	});
	assert!(result.is_err());
}

#[tokio::test]
async fn circulation_desk_enqueue_and_drain() {
	let generator = Arc::new(ExtractionGenerator);
	let librarian = Arc::new(Librarian::new(generator, None));
	let ops = Arc::new(MockLibraryOps::new());
	let ops_trait: Arc<dyn CirculationLibraryOps> = Arc::clone(&ops) as Arc<dyn CirculationLibraryOps>;

	let desk = CirculationDesk::new(CirculationDeskOptions {
		librarian: Some(librarian),
		registry: None,
		library_ops: ops_trait,
		thresholds: CirculationThresholds::default(),
		catalog: None,
	})
	.unwrap();

	desk.enqueue_extraction(TurnContext {
		user_input: "Tell me about Rust".to_string(),
		response: "Rust is great".to_string(),
	});

	assert_eq!(desk.pending(), 1);
	assert!(!desk.is_processing());

	desk.drain().await;

	assert_eq!(desk.pending(), 0);

	// Check that volumes were added
	let added = ops.added.lock().await;
	assert_eq!(added.len(), 2); // ExtractionGenerator returns 2 memories
}

#[tokio::test]
async fn circulation_desk_dispose_prevents_enqueue() {
	let generator = Arc::new(ExtractionGenerator);
	let librarian = Arc::new(Librarian::new(generator, None));
	let ops: Arc<dyn CirculationLibraryOps> = Arc::new(MockLibraryOps::new());

	let desk = CirculationDesk::new(CirculationDeskOptions {
		librarian: Some(librarian),
		registry: None,
		library_ops: ops,
		thresholds: CirculationThresholds::default(),
		catalog: None,
	})
	.unwrap();

	desk.dispose().await;

	desk.enqueue_extraction(TurnContext {
		user_input: "test".to_string(),
		response: "test".to_string(),
	});

	// Should not enqueue after dispose
	assert_eq!(desk.pending(), 0);
}

#[tokio::test]
async fn circulation_desk_flush_clears_queue() {
	let generator = Arc::new(ExtractionGenerator);
	let librarian = Arc::new(Librarian::new(generator, None));
	let ops: Arc<dyn CirculationLibraryOps> = Arc::new(MockLibraryOps::new());

	let desk = CirculationDesk::new(CirculationDeskOptions {
		librarian: Some(librarian),
		registry: None,
		library_ops: ops,
		thresholds: CirculationThresholds::default(),
		catalog: None,
	})
	.unwrap();

	desk.enqueue_compendium("topic1".to_string());
	desk.enqueue_reorganization("topic2".to_string());

	desk.flush().await;
	assert_eq!(desk.pending(), 0);
}

#[tokio::test]
async fn circulation_desk_multiple_job_types() {
	let generator = Arc::new(SummarizeGenerator);
	let librarian = Arc::new(Librarian::new(generator, None));
	let ops: Arc<dyn CirculationLibraryOps> = Arc::new(MockLibraryOps::new());

	let desk = CirculationDesk::new(CirculationDeskOptions {
		librarian: Some(librarian),
		registry: None,
		library_ops: ops,
		thresholds: CirculationThresholds::default(),
		catalog: None,
	})
	.unwrap();

	desk.enqueue_compendium("topic1".to_string());
	desk.enqueue_reorganization("topic2".to_string());
	desk.enqueue_optimization("topic3".to_string());

	assert_eq!(desk.pending(), 3);

	desk.drain().await;
	assert_eq!(desk.pending(), 0);
}

// ===========================================================================
// LibraryServices tests
// ===========================================================================

use simse_core::library::{EmbeddingProvider, Library, LibraryConfig};
use simse_core::library::services::{LibraryContext, LibraryServices, LibraryServicesOptions};
use simse_adaptive_engine::store::StoreConfig;

/// Simple mock embedder for services tests.
struct SimpleEmbedder;

impl SimpleEmbedder {
	fn make_embedding(text: &str) -> Vec<f32> {
		let len = text.len() as f32;
		vec![len / 100.0, 0.5, 0.5, 0.5]
	}
}

#[async_trait]
impl EmbeddingProvider for SimpleEmbedder {
	async fn embed(
		&self,
		input: &[String],
		_model: &str,
	) -> Result<Vec<Vec<f32>>, SimseError> {
		Ok(input.iter().map(|t| Self::make_embedding(t)).collect())
	}
}

fn create_test_library() -> Arc<Library> {
	let embedder = Arc::new(SimpleEmbedder);
	let config = LibraryConfig::default();
	let store_config = StoreConfig::default();
	Arc::new(Library::new(embedder, config, store_config, None))
}

#[tokio::test]
async fn services_enrich_returns_original_when_not_initialized() {
	let library = create_test_library();
	let services = LibraryServices::new(Arc::clone(&library), None);

	let context = LibraryContext {
		user_input: "test query".to_string(),
		current_system_prompt: "You are helpful.".to_string(),
		conversation_history: String::new(),
		turn: 1,
	};

	let result = services.enrich_system_prompt(&context).await;
	assert_eq!(result, "You are helpful.");
}

#[tokio::test]
async fn services_enrich_returns_original_when_empty() {
	let library = create_test_library();
	library.initialize(None).unwrap();

	let services = LibraryServices::new(Arc::clone(&library), None);

	let context = LibraryContext {
		user_input: "test query".to_string(),
		current_system_prompt: "You are helpful.".to_string(),
		conversation_history: String::new(),
		turn: 1,
	};

	let result = services.enrich_system_prompt(&context).await;
	assert_eq!(result, "You are helpful.");
}

#[tokio::test]
async fn services_after_response_stores_qa() {
	let library = create_test_library();
	library.initialize(None).unwrap();

	let services = LibraryServices::new(Arc::clone(&library), None);

	services
		.after_response("What is Rust?", "Rust is a language.")
		.await
		.unwrap();

	assert_eq!(library.size(), 1);
}

#[tokio::test]
async fn services_after_response_skips_empty() {
	let library = create_test_library();
	library.initialize(None).unwrap();

	let services = LibraryServices::new(Arc::clone(&library), None);

	services.after_response("test", "").await.unwrap();
	services.after_response("test", "   ").await.unwrap();

	assert_eq!(library.size(), 0);
}

#[tokio::test]
async fn services_after_response_skips_errors() {
	let library = create_test_library();
	library.initialize(None).unwrap();

	let services = LibraryServices::new(Arc::clone(&library), None);

	services
		.after_response("test", "Error: something went wrong")
		.await
		.unwrap();

	assert_eq!(library.size(), 0);
}

#[tokio::test]
async fn services_after_response_skips_when_disabled() {
	let library = create_test_library();
	library.initialize(None).unwrap();

	let services = LibraryServices::new(
		Arc::clone(&library),
		Some(LibraryServicesOptions {
			store_responses: false,
			..Default::default()
		}),
	);

	services
		.after_response("test", "valid response")
		.await
		.unwrap();

	assert_eq!(library.size(), 0);
}

#[tokio::test]
async fn services_after_response_skips_when_not_initialized() {
	let library = create_test_library();
	// NOT initialized
	let services = LibraryServices::new(Arc::clone(&library), None);

	services
		.after_response("test", "valid response")
		.await
		.unwrap();

	assert_eq!(library.size(), 0);
}

#[tokio::test]
async fn services_enrich_with_context() {
	let library = create_test_library();
	library.initialize(None).unwrap();

	// Add some content to library
	let mut meta = HashMap::new();
	meta.insert("topic".to_string(), "rust".to_string());
	library.add("Rust is a systems programming language", meta).await.unwrap();

	let services = LibraryServices::new(Arc::clone(&library), None);

	let context = LibraryContext {
		user_input: "Rust is a systems programming language".to_string(), // Same text to get high similarity
		current_system_prompt: "You are helpful.".to_string(),
		conversation_history: String::new(),
		turn: 1,
	};

	let result = services.enrich_system_prompt(&context).await;
	// Should have appended memory context
	assert!(result.starts_with("You are helpful."));
	assert!(result.len() > "You are helpful.".len());
}

#[tokio::test]
async fn services_uses_circulation_desk() {
	let library = create_test_library();
	library.initialize(None).unwrap();

	let generator = Arc::new(ExtractionGenerator);
	let librarian = Arc::new(Librarian::new(generator, None));
	let ops: Arc<dyn CirculationLibraryOps> = Arc::new(MockLibraryOps::new());

	let desk = Arc::new(
		CirculationDesk::new(CirculationDeskOptions {
			librarian: Some(librarian),
			registry: None,
			library_ops: Arc::clone(&ops),
			thresholds: CirculationThresholds::default(),
			catalog: None,
		})
		.unwrap(),
	);

	let services = LibraryServices::new(
		Arc::clone(&library),
		Some(LibraryServicesOptions {
			circulation_desk: Some(Arc::clone(&desk)),
			..Default::default()
		}),
	);

	// Should enqueue to circulation desk instead of direct add
	services
		.after_response("What is Rust?", "Rust is great")
		.await
		.unwrap();

	// Library should be empty (went to circulation desk instead)
	assert_eq!(library.size(), 0);

	// But the desk should have a pending job
	assert_eq!(desk.pending(), 1);
}

// ===========================================================================
// ReorganizationPlan / OptimizationResult convenience
// ===========================================================================

#[test]
fn reorganization_plan_empty() {
	let plan = ReorganizationPlan::empty();
	assert!(plan.moves.is_empty());
	assert!(plan.new_subtopics.is_empty());
	assert!(plan.merges.is_empty());
}

#[test]
fn optimization_result_empty() {
	let result = OptimizationResult::empty("test-model");
	assert!(result.pruned.is_empty());
	assert!(result.summary.is_empty());
	assert_eq!(result.model_used, "test-model");
}

// ===========================================================================
// EntryType tests
// ===========================================================================

#[test]
fn entry_type_as_str() {
	assert_eq!(EntryType::Fact.as_str(), "fact");
	assert_eq!(EntryType::Decision.as_str(), "decision");
	assert_eq!(EntryType::Observation.as_str(), "observation");
}

#[test]
fn entry_type_display() {
	assert_eq!(format!("{}", EntryType::Fact), "fact");
	assert_eq!(format!("{}", EntryType::Decision), "decision");
	assert_eq!(format!("{}", EntryType::Observation), "observation");
}
