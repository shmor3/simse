//! Provider-specific prompt resolution.
//!
//! Matches model IDs against glob patterns to resolve provider-specific
//! prompt preambles. First matching pattern wins; falls back to
//! `default_prompt` or empty string.
//!
//! Also provides `provider_prompt()` which returns static prompt strings
//! for known provider names.

use regex::Regex;

// ---------------------------------------------------------------------------
// ProviderPromptConfig
// ---------------------------------------------------------------------------

/// Configuration for provider-specific prompts.
///
/// `prompts` contains `(glob_pattern, prompt_text)` pairs.
/// Glob syntax: `*` matches any sequence of characters, `?` matches any single character.
#[derive(Debug, Clone)]
pub struct ProviderPromptConfig {
	/// Glob pattern -> prompt text pairs. First match wins.
	pub prompts: Vec<(String, String)>,
	/// Fallback prompt when no pattern matches.
	pub default_prompt: Option<String>,
}

// ---------------------------------------------------------------------------
// ProviderPromptResolver
// ---------------------------------------------------------------------------

/// Resolves a model ID to the best-matching provider prompt.
pub struct ProviderPromptResolver {
	entries: Vec<(Regex, String)>,
	default_prompt: String,
}

impl ProviderPromptResolver {
	/// Create a new resolver from a config.
	pub fn new(config: ProviderPromptConfig) -> Self {
		let entries = config
			.prompts
			.into_iter()
			.filter_map(|(pattern, prompt)| {
				let regex = glob_to_regex(&pattern)?;
				Some((regex, prompt))
			})
			.collect();

		let default_prompt = config.default_prompt.unwrap_or_default();

		Self {
			entries,
			default_prompt,
		}
	}

	/// Resolve a model ID to a prompt string.
	///
	/// Returns the prompt for the first matching glob pattern,
	/// or the default prompt, or an empty string.
	pub fn resolve(&self, model_id: &str) -> &str {
		for (regex, prompt) in &self.entries {
			if regex.is_match(model_id) {
				return prompt;
			}
		}
		&self.default_prompt
	}
}

// ---------------------------------------------------------------------------
// Static provider prompts
// ---------------------------------------------------------------------------

/// Returns a static prompt string for a known provider name.
///
/// Known providers: "anthropic", "openai", "google", "cohere".
/// Unknown providers return an empty string.
pub fn provider_prompt(provider: &str) -> &'static str {
	match provider {
		"anthropic" => ANTHROPIC_PROMPT,
		"openai" => OPENAI_PROMPT,
		"google" => GOOGLE_PROMPT,
		"cohere" => COHERE_PROMPT,
		_ => "",
	}
}

const ANTHROPIC_PROMPT: &str = "\
You are Claude, made by Anthropic. You are a helpful, harmless, and honest AI assistant. \
Follow instructions carefully and think step by step.";

const OPENAI_PROMPT: &str = "\
You are a helpful AI assistant powered by OpenAI. \
Follow instructions carefully and provide accurate, well-structured responses.";

const GOOGLE_PROMPT: &str = "\
You are a helpful AI assistant powered by Google. \
Follow instructions carefully and provide accurate, well-structured responses.";

const COHERE_PROMPT: &str = "\
You are a helpful AI assistant powered by Cohere. \
Follow instructions carefully and provide accurate, well-structured responses.";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a simple glob pattern to a regex.
///
/// `*` becomes `.*`, `?` becomes `.`; all other regex-special characters are escaped.
fn glob_to_regex(pattern: &str) -> Option<Regex> {
	let mut escaped = String::new();
	escaped.push('^');

	for ch in pattern.chars() {
		match ch {
			'*' => escaped.push_str(".*"),
			'?' => escaped.push('.'),
			'.' | '+' | '^' | '$' | '{' | '}' | '(' | ')' | '|' | '[' | ']' | '\\' => {
				escaped.push('\\');
				escaped.push(ch);
			}
			_ => escaped.push(ch),
		}
	}

	escaped.push('$');
	match Regex::new(&escaped) {
		Ok(re) => Some(re),
		Err(e) => {
			tracing::warn!(pattern, error = %e, "invalid glob pattern, skipping");
			None
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn glob_to_regex_simple() {
		let re = glob_to_regex("claude-*").unwrap();
		assert!(re.is_match("claude-3-opus"));
		assert!(re.is_match("claude-instant"));
		assert!(!re.is_match("gpt-4"));
	}

	#[test]
	fn glob_to_regex_exact() {
		let re = glob_to_regex("gpt-4").unwrap();
		assert!(re.is_match("gpt-4"));
		assert!(!re.is_match("gpt-4-turbo"));
	}

	#[test]
	fn glob_to_regex_dots_escaped() {
		let re = glob_to_regex("model.v1").unwrap();
		assert!(re.is_match("model.v1"));
		assert!(!re.is_match("modelXv1"));
	}
}
