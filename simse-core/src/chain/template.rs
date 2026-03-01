//! Prompt template with variable extraction and formatting.
//!
//! Ports `src/ai/chain/prompt-template.ts` (~85 lines).
//!
//! Extracts `{varname}` placeholders from a template string, deduplicates
//! variable names, and provides `format()` to substitute values. Uses
//! `LazyLock<Regex>` to avoid recompiling the regex on every call.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::error::{SimseError, TemplateErrorCode};

/// Regex for extracting `{varname}` placeholders.
/// Matches word characters and hyphens inside braces: `{[\w-]+}`.
static VAR_REGEX: LazyLock<Regex> = LazyLock::new(|| {
	Regex::new(r"\{([\w-]+)\}").expect("VAR_REGEX is a valid regex")
});

/// A prompt template that extracts `{variable}` placeholders and supports
/// formatting with provided values.
#[derive(Debug, Clone)]
pub struct PromptTemplate {
	raw: String,
	variables: Vec<String>,
}

impl PromptTemplate {
	/// Create a new prompt template from a raw template string.
	///
	/// Returns an error if the template is empty.
	/// Extracts unique variable names from `{varname}` placeholders,
	/// preserving first-occurrence order and deduplicating.
	pub fn new(template: impl Into<String>) -> Result<Self, SimseError> {
		let raw = template.into();

		if raw.is_empty() {
			return Err(SimseError::template(
				TemplateErrorCode::Empty,
				"Template string cannot be empty",
			));
		}

		// Extract unique variable names, preserving insertion order.
		let mut variables = Vec::new();
		for cap in VAR_REGEX.captures_iter(&raw) {
			let var_name = cap[1].to_string();
			if !variables.contains(&var_name) {
				variables.push(var_name);
			}
		}

		Ok(Self { raw, variables })
	}

	/// Format the template by substituting all variable placeholders.
	///
	/// Returns an error if any variable is missing from the provided values.
	pub fn format(&self, values: &HashMap<String, String>) -> Result<String, SimseError> {
		let missing: Vec<&str> = self
			.variables
			.iter()
			.filter(|v| !values.contains_key(v.as_str()))
			.map(String::as_str)
			.collect();

		if !missing.is_empty() {
			return Err(SimseError::template(
				TemplateErrorCode::MissingVariables,
				format!(
					"Missing template variables: {}",
					missing.join(", ")
				),
			));
		}

		let mut result = self.raw.clone();
		for var_name in &self.variables {
			let placeholder = format!("{{{var_name}}}");
			if let Some(value) = values.get(var_name) {
				result = result.replace(&placeholder, value);
			}
		}

		Ok(result)
	}

	/// Returns `true` if the template contains any variable placeholders.
	pub fn has_variables(&self) -> bool {
		!self.variables.is_empty()
	}

	/// Returns the deduplicated variable names in first-occurrence order.
	pub fn variables(&self) -> Vec<&str> {
		self.variables.iter().map(String::as_str).collect()
	}

	/// Returns the raw template string.
	pub fn raw(&self) -> &str {
		&self.raw
	}
}
