//! Settings field definitions, types, defaults, validation.

use serde::{Deserialize, Serialize};

/// A field type in the settings schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FieldType {
	Text,
	Number,
	Boolean,
	Select { options: Vec<String> },
	FilePath,
}

/// A single settings field definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldSchema {
	pub key: String,
	pub label: String,
	pub description: String,
	pub field_type: FieldType,
	pub default_value: serde_json::Value,
	pub section: String,
}

/// Represents a config file's schema: its filename, description, and fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigFileSchema {
	pub filename: String,
	pub description: String,
	pub fields: Vec<FieldSchema>,
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

/// Create a `FieldSchema` with the given parameters.
fn field(
	key: &str,
	label: &str,
	description: &str,
	field_type: FieldType,
	default_value: serde_json::Value,
	section: &str,
) -> FieldSchema {
	FieldSchema {
		key: key.to_string(),
		label: label.to_string(),
		description: description.to_string(),
		field_type,
		default_value,
		section: section.to_string(),
	}
}

/// Create a `Select` field type from a list of string options.
fn select(options: &[&str]) -> FieldType {
	FieldType::Select {
		options: options.iter().map(|s| s.to_string()).collect(),
	}
}

// ---------------------------------------------------------------------------
// Schema definitions
// ---------------------------------------------------------------------------

fn config_json_schema() -> ConfigFileSchema {
	ConfigFileSchema {
		filename: "config.json".to_string(),
		description: "General user preferences".to_string(),
		fields: vec![
			field(
				"logLevel",
				"Log Level",
				"Log level for the application",
				select(&["debug", "info", "warn", "error", "none"]),
				serde_json::json!("warn"),
				"config",
			),
			field(
				"defaultAgent",
				"Default Agent",
				"Default agent ID for generation",
				FieldType::Text,
				serde_json::Value::Null,
				"config",
			),
			field(
				"perplexityApiKey",
				"Perplexity API Key",
				"Perplexity API key for web search",
				FieldType::Text,
				serde_json::Value::Null,
				"config",
			),
			field(
				"githubToken",
				"GitHub Token",
				"GitHub personal access token",
				FieldType::Text,
				serde_json::Value::Null,
				"config",
			),
		],
	}
}

fn acp_json_schema() -> ConfigFileSchema {
	ConfigFileSchema {
		filename: "acp.json".to_string(),
		description: "ACP server configuration".to_string(),
		fields: vec![field(
			"defaultServer",
			"Default Server",
			"Default ACP server name",
			FieldType::Text,
			serde_json::Value::Null,
			"acp",
		)],
	}
}

fn embed_json_schema() -> ConfigFileSchema {
	ConfigFileSchema {
		filename: "embed.json".to_string(),
		description: "Embedding provider configuration".to_string(),
		fields: vec![
			field(
				"embeddingModel",
				"Embedding Model",
				"Hugging Face model ID for in-process embeddings",
				FieldType::Text,
				serde_json::json!("nomic-ai/nomic-embed-text-v1.5"),
				"embed",
			),
			field(
				"dtype",
				"Quantization Dtype",
				"ONNX quantization dtype",
				select(&["fp32", "fp16", "q8", "q4"]),
				serde_json::Value::Null,
				"embed",
			),
			field(
				"teiUrl",
				"TEI URL",
				"TEI server URL (when set, uses TEI HTTP bridge instead of local embedder)",
				FieldType::Text,
				serde_json::Value::Null,
				"embed",
			),
		],
	}
}

fn memory_json_schema() -> ConfigFileSchema {
	ConfigFileSchema {
		filename: "memory.json".to_string(),
		description: "Library, stacks, and storage configuration".to_string(),
		fields: vec![
			field(
				"enabled",
				"Enabled",
				"Whether the library is enabled",
				FieldType::Boolean,
				serde_json::json!(true),
				"memory",
			),
			field(
				"similarityThreshold",
				"Similarity Threshold",
				"Similarity threshold for library search (0-1)",
				FieldType::Number,
				serde_json::json!(0.7),
				"memory",
			),
			field(
				"maxResults",
				"Max Results",
				"Maximum library search results",
				FieldType::Number,
				serde_json::json!(10),
				"memory",
			),
			field(
				"autoSummarizeThreshold",
				"Auto-Summarize Threshold",
				"Max volumes per topic before auto-summarizing oldest entries (0 = disabled)",
				FieldType::Number,
				serde_json::json!(20),
				"memory",
			),
			field(
				"duplicateThreshold",
				"Duplicate Threshold",
				"Cosine similarity threshold for duplicate detection (0-1, 0 = disabled)",
				FieldType::Number,
				serde_json::json!(0),
				"memory",
			),
			field(
				"duplicateBehavior",
				"Duplicate Behavior",
				"Duplicate detection behavior",
				select(&["skip", "warn", "error"]),
				serde_json::json!("skip"),
				"memory",
			),
		],
	}
}

fn summarize_json_schema() -> ConfigFileSchema {
	ConfigFileSchema {
		filename: "summarize.json".to_string(),
		description: "Summarization ACP server configuration".to_string(),
		fields: vec![
			field(
				"server",
				"Server",
				"ACP server name to use for summarization",
				FieldType::Text,
				serde_json::Value::Null,
				"summarize",
			),
			field(
				"command",
				"Command",
				"Command to start the summarization ACP server",
				FieldType::Text,
				serde_json::Value::Null,
				"summarize",
			),
			field(
				"agent",
				"Agent",
				"Agent ID for the summarization ACP server",
				FieldType::Text,
				serde_json::Value::Null,
				"summarize",
			),
		],
	}
}

fn settings_json_schema() -> ConfigFileSchema {
	ConfigFileSchema {
		filename: "settings.json".to_string(),
		description: "Workspace-level overrides (.simse/settings.json)".to_string(),
		fields: vec![
			field(
				"defaultAgent",
				"Default Agent",
				"Default agent ID",
				FieldType::Text,
				serde_json::Value::Null,
				"settings",
			),
			field(
				"logLevel",
				"Log Level",
				"Log level",
				select(&["debug", "info", "warn", "error", "none"]),
				serde_json::Value::Null,
				"settings",
			),
			field(
				"systemPrompt",
				"System Prompt",
				"System prompt applied to all generate() calls",
				FieldType::Text,
				serde_json::Value::Null,
				"settings",
			),
			field(
				"defaultServer",
				"Default Server",
				"ACP server name override",
				FieldType::Text,
				serde_json::Value::Null,
				"settings",
			),
			field(
				"conversationTopic",
				"Conversation Topic",
				"Topic name used when storing generate() results in the library",
				FieldType::Text,
				serde_json::Value::Null,
				"settings",
			),
			field(
				"chainTopic",
				"Chain Topic",
				"Topic name used when storing chain results in the library",
				FieldType::Text,
				serde_json::Value::Null,
				"settings",
			),
		],
	}
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Returns schemas for all 6 config files.
pub fn all_config_schemas() -> Vec<ConfigFileSchema> {
	vec![
		config_json_schema(),
		acp_json_schema(),
		embed_json_schema(),
		memory_json_schema(),
		summarize_json_schema(),
		settings_json_schema(),
	]
}

/// Returns the schema for a specific config filename, or `None` if unknown.
pub fn get_config_schema(filename: &str) -> Option<ConfigFileSchema> {
	match filename {
		"config.json" => Some(config_json_schema()),
		"acp.json" => Some(acp_json_schema()),
		"embed.json" => Some(embed_json_schema()),
		"memory.json" => Some(memory_json_schema()),
		"summarize.json" => Some(summarize_json_schema()),
		"settings.json" => Some(settings_json_schema()),
		_ => None,
	}
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn all_config_schemas_present() {
		let schemas = all_config_schemas();
		assert_eq!(schemas.len(), 6);
		let filenames: Vec<_> = schemas.iter().map(|s| s.filename.as_str()).collect();
		assert!(filenames.contains(&"config.json"));
		assert!(filenames.contains(&"acp.json"));
		assert!(filenames.contains(&"embed.json"));
		assert!(filenames.contains(&"memory.json"));
		assert!(filenames.contains(&"summarize.json"));
		assert!(filenames.contains(&"settings.json"));
	}

	#[test]
	fn config_json_has_log_level() {
		let schema = get_config_schema("config.json").unwrap();
		let field = schema.fields.iter().find(|f| f.key == "logLevel").unwrap();
		assert!(matches!(field.field_type, FieldType::Select { .. }));
	}

	#[test]
	fn config_json_log_level_options() {
		let schema = get_config_schema("config.json").unwrap();
		let field = schema.fields.iter().find(|f| f.key == "logLevel").unwrap();
		if let FieldType::Select { options } = &field.field_type {
			assert!(options.contains(&"debug".to_string()));
			assert!(options.contains(&"warn".to_string()));
			assert_eq!(options.len(), 5);
		} else {
			panic!("Expected Select type");
		}
	}

	#[test]
	fn memory_json_has_enabled_bool() {
		let schema = get_config_schema("memory.json").unwrap();
		let field = schema.fields.iter().find(|f| f.key == "enabled").unwrap();
		assert!(matches!(field.field_type, FieldType::Boolean));
	}

	#[test]
	fn memory_json_has_six_fields() {
		let schema = get_config_schema("memory.json").unwrap();
		assert_eq!(schema.fields.len(), 6);
	}

	#[test]
	fn embed_json_has_embedding_model_default() {
		let schema = get_config_schema("embed.json").unwrap();
		let field = schema
			.fields
			.iter()
			.find(|f| f.key == "embeddingModel")
			.unwrap();
		assert_eq!(
			field.default_value,
			serde_json::json!("nomic-ai/nomic-embed-text-v1.5")
		);
	}

	#[test]
	fn settings_json_has_six_fields() {
		let schema = get_config_schema("settings.json").unwrap();
		assert_eq!(schema.fields.len(), 6);
	}

	#[test]
	fn get_unknown_schema_returns_none() {
		assert!(get_config_schema("nonexistent.json").is_none());
	}
}
