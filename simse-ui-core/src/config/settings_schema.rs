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
