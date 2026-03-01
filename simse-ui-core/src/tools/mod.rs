//! Tool definitions, registry, execution types.

use serde::{Deserialize, Serialize};

/// A tool parameter definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParameter {
	#[serde(rename = "type")]
	pub param_type: String,
	pub description: String,
	#[serde(default)]
	pub required: bool,
}

/// A tool definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
	pub name: String,
	pub description: String,
	pub parameters: std::collections::HashMap<String, ToolParameter>,
}

/// A request to call a tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRequest {
	pub id: String,
	pub name: String,
	pub arguments: serde_json::Value,
}

/// Result of a tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallResult {
	pub id: String,
	pub name: String,
	pub output: String,
	pub is_error: bool,
	pub diff: Option<String>,
}
