//! Librarian Definition — validation, topic matching, and file persistence
//! for configurable librarian JSON definitions.
//!
//! Ports `src/ai/library/librarian-definition.ts` (~209 lines) to Rust.
//!
//! - `LibrarianDefinition` — serde-serializable struct for JSON persistence
//! - `validate_definition()` — strict validation with error list
//! - `matches_topic()` — glob-style matching (`*` = one level, `**` = recursive)
//! - `save_definition` / `load_definition` / `load_all_definitions` — async JSON file I/O

use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// LibrarianDefinition
// ---------------------------------------------------------------------------

/// Permissions a librarian has over its managed content.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LibrarianPermissions {
	pub add: bool,
	pub delete: bool,
	pub reorganize: bool,
}

/// Thresholds that control when escalation/optimization kicks in.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LibrarianThresholds {
	#[serde(rename = "topicComplexity")]
	pub topic_complexity: f64,
	#[serde(rename = "escalateAt")]
	pub escalate_at: f64,
}

/// Optional ACP connection configuration for a librarian.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LibrarianAcp {
	pub command: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub args: Option<Vec<String>>,
	#[serde(rename = "agentId", skip_serializing_if = "Option::is_none")]
	pub agent_id: Option<String>,
}

/// A configurable librarian definition, serializable to/from JSON.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LibrarianDefinition {
	pub name: String,
	pub description: String,
	pub purpose: String,
	pub topics: Vec<String>,
	pub permissions: LibrarianPermissions,
	pub thresholds: LibrarianThresholds,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub acp: Option<LibrarianAcp>,
}

/// Default librarian definition (created programmatically, never on disk).
pub fn default_definition() -> LibrarianDefinition {
	LibrarianDefinition {
		name: "default".to_string(),
		description: "General-purpose head librarian that manages all topics.".to_string(),
		purpose: "General-purpose head librarian for routing, arbitration, and fallback."
			.to_string(),
		topics: vec!["**".to_string()],
		permissions: LibrarianPermissions {
			add: true,
			delete: true,
			reorganize: true,
		},
		thresholds: LibrarianThresholds {
			topic_complexity: 100.0,
			escalate_at: 500.0,
		},
		acp: None,
	}
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// Result of validating a `LibrarianDefinition`.
#[derive(Debug, Clone)]
pub struct ValidationResult {
	pub valid: bool,
	pub errors: Vec<String>,
}

/// Kebab-case name pattern: `^[a-z0-9][a-z0-9-]*$`
static NAME_PATTERN: LazyLock<Regex> =
	LazyLock::new(|| Regex::new(r"^[a-z0-9][a-z0-9-]*$").unwrap());

/// Validates a `LibrarianDefinition` against schema rules.
///
/// Returns a `ValidationResult` with `valid = true` if all checks pass,
/// or a list of human-readable error strings.
pub fn validate_definition(input: &LibrarianDefinition) -> ValidationResult {
	let mut errors = Vec::new();

	// name
	if input.name.is_empty() {
		errors.push("name must be a non-empty string".to_string());
	} else if !NAME_PATTERN.is_match(&input.name) {
		errors.push(
			"name must be kebab-case (lowercase alphanumeric and hyphens, starting with alphanumeric)"
				.to_string(),
		);
	}

	// description
	if input.description.is_empty() {
		errors.push("description must be a non-empty string".to_string());
	}

	// purpose
	if input.purpose.is_empty() {
		errors.push("purpose must be a non-empty string".to_string());
	}

	// topics
	if input.topics.is_empty() {
		errors.push("topics must be a non-empty array of strings".to_string());
	}

	// thresholds
	if input.thresholds.topic_complexity <= 0.0 {
		errors.push("thresholds.topicComplexity must be a positive number".to_string());
	}
	if input.thresholds.escalate_at <= 0.0 {
		errors.push("thresholds.escalateAt must be a positive number".to_string());
	}

	// acp (optional)
	if let Some(ref acp) = input.acp {
		if acp.command.is_empty() {
			errors.push("acp.command must be a non-empty string".to_string());
		}
	}

	ValidationResult {
		valid: errors.is_empty(),
		errors,
	}
}

/// Validate a raw JSON value against the LibrarianDefinition schema.
///
/// This is useful when loading from disk where the JSON may be malformed.
pub fn validate_definition_json(value: &serde_json::Value) -> ValidationResult {
	match serde_json::from_value::<LibrarianDefinition>(value.clone()) {
		Ok(def) => validate_definition(&def),
		Err(e) => ValidationResult {
			valid: false,
			errors: vec![format!("Failed to parse as LibrarianDefinition: {}", e)],
		},
	}
}

// ---------------------------------------------------------------------------
// Topic Matching
// ---------------------------------------------------------------------------

/// Check if a topic matches any of the given glob patterns.
///
/// - `*` matches any single segment (between `/` separators).
/// - `**` matches zero or more segments (recursive).
///
/// Examples:
/// - `matches_topic(&["code/*"], "code/react")` => true
/// - `matches_topic(&["code/**"], "code/react/hooks")` => true
/// - `matches_topic(&["**"], "anything/at/all")` => true
pub fn matches_topic(patterns: &[String], topic: &str) -> bool {
	let topic_segments: Vec<&str> = topic.split('/').collect();
	patterns.iter().any(|pattern| {
		let pattern_segments: Vec<&str> = pattern.split('/').collect();
		match_segments(&pattern_segments, &topic_segments)
	})
}

/// Recursive segment matcher.
fn match_segments(pattern: &[&str], topic: &[&str]) -> bool {
	match (pattern.first(), topic.first()) {
		// Both empty -> match
		(None, None) => true,
		// Pattern has `**` -> try matching rest with zero or more topic segments
		(Some(&"**"), _) => {
			// `**` at the end matches everything
			if pattern.len() == 1 {
				return true;
			}
			// Try consuming 0..=N topic segments
			for i in 0..=topic.len() {
				if match_segments(&pattern[1..], &topic[i..]) {
					return true;
				}
			}
			false
		}
		// Pattern has `*` -> matches any single segment
		(Some(&"*"), Some(_)) => match_segments(&pattern[1..], &topic[1..]),
		// Literal match
		(Some(p), Some(t)) => {
			if p == t {
				match_segments(&pattern[1..], &topic[1..])
			} else {
				false
			}
		}
		// One side exhausted
		_ => false,
	}
}

// ---------------------------------------------------------------------------
// Persistence
// ---------------------------------------------------------------------------

/// Saves a librarian definition to `<librarians_dir>/<name>.json`.
///
/// Creates the directory if it does not exist.
pub async fn save_definition(
	librarians_dir: &Path,
	definition: &LibrarianDefinition,
) -> Result<(), std::io::Error> {
	tokio::fs::create_dir_all(librarians_dir).await?;
	let file_path = librarians_dir.join(format!("{}.json", definition.name));
	let json = serde_json::to_string_pretty(definition)
		.map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
	tokio::fs::write(file_path, format!("{}\n", json)).await
}

/// Loads a single librarian definition from `<librarians_dir>/<name>.json`.
///
/// Returns `None` if the file does not exist or fails validation.
pub async fn load_definition(
	librarians_dir: &Path,
	name: &str,
) -> Option<LibrarianDefinition> {
	let file_path = librarians_dir.join(format!("{}.json", name));
	let raw = tokio::fs::read_to_string(&file_path).await.ok()?;
	let parsed: LibrarianDefinition = serde_json::from_str(&raw).ok()?;
	let result = validate_definition(&parsed);
	if result.valid {
		Some(parsed)
	} else {
		None
	}
}

/// Loads all valid librarian definitions from `<librarians_dir>`.
///
/// Reads every `.json` file, validates each, and returns only the valid ones.
/// Returns an empty vec if the directory does not exist.
pub async fn load_all_definitions(
	librarians_dir: &Path,
) -> Vec<LibrarianDefinition> {
	let mut entries = match tokio::fs::read_dir(librarians_dir).await {
		Ok(e) => e,
		Err(_) => return Vec::new(),
	};

	let mut definitions = Vec::new();

	while let Ok(Some(entry)) = entries.next_entry().await {
		let path = entry.path();
		if path.extension().is_some_and(|ext| ext == "json") {
			if let Ok(raw) = tokio::fs::read_to_string(&path).await {
				if let Ok(parsed) = serde_json::from_str::<LibrarianDefinition>(&raw) {
					if validate_definition(&parsed).valid {
						definitions.push(parsed);
					}
				}
			}
		}
	}

	definitions
}
