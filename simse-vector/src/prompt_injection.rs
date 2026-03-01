// ---------------------------------------------------------------------------
// Structured Prompt Injection — Memory Context Formatter
// ---------------------------------------------------------------------------
//
// Formats memory search results as structured XML tags or natural text
// for injection into the system prompt. This gives the LLM relevant
// context from past conversations without polluting the user's message.
//
// Ported from the TypeScript implementation in src/prompt-injection.ts.
// ---------------------------------------------------------------------------

use crate::types::Lookup;

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

pub struct PromptInjectionOptions {
	/// Maximum number of results to include.
	pub max_results: Option<usize>,
	/// Minimum relevance score to include (0-1).
	pub min_score: Option<f64>,
	/// Output format: "structured" (XML tags) or "natural".
	pub format: Option<String>,
	/// XML tag name used for the outer wrapper. Defaults to "memory-context".
	pub tag: Option<String>,
	/// Maximum total characters in the output. Defaults to 4000.
	pub max_chars: Option<usize>,
}

impl Default for PromptInjectionOptions {
	fn default() -> Self {
		Self {
			max_results: None,
			min_score: None,
			format: None,
			tag: None,
			max_chars: None,
		}
	}
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Format a duration in milliseconds as a human-readable age string.
///
/// Returns seconds, minutes, hours, or days depending on magnitude.
pub fn format_age(ms: u64) -> String {
	let seconds = ms / 1000;
	if seconds < 60 {
		return format!("{}s", seconds);
	}
	let minutes = seconds / 60;
	if minutes < 60 {
		return format!("{}m", minutes);
	}
	let hours = minutes / 60;
	if hours < 24 {
		return format!("{}h", hours);
	}
	let days = hours / 24;
	format!("{}d", days)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Format memory context for prompt injection.
///
/// Returns an empty string if no results pass the filters.
///
/// `now` is the current timestamp in milliseconds.
pub fn format_memory_context(
	results: &[Lookup],
	options: &PromptInjectionOptions,
	now: u64,
) -> String {
	if results.is_empty() {
		return String::new();
	}

	let max_results = options.max_results.unwrap_or(results.len());
	let min_score = options.min_score.unwrap_or(0.0);
	let format = options
		.format
		.as_deref()
		.unwrap_or("structured");
	let tag = options.tag.as_deref().unwrap_or("memory-context");
	let max_chars = options.max_chars.unwrap_or(4000);

	let filtered: Vec<&Lookup> = results
		.iter()
		.filter(|r| r.score >= min_score)
		.take(max_results)
		.collect();

	if filtered.is_empty() {
		return String::new();
	}

	if format == "natural" {
		let header = "Relevant context from library:";
		let mut lines = vec![header.to_string()];
		let mut chars = header.len();

		for r in &filtered {
			let topic = r
				.volume
				.metadata
				.get("topic")
				.map(|s| s.as_str())
				.unwrap_or("uncategorized");
			let line = format!(
				"- [{}] (relevance: {:.2}) {}",
				topic, r.score, r.volume.text
			);
			if chars + line.len() > max_chars {
				break;
			}
			chars += line.len();
			lines.push(line);
		}

		return lines.join("\n");
	}

	// Structured format (XML)
	let mut entries: Vec<String> = Vec::new();
	let mut chars: usize = 0;

	for r in &filtered {
		let topic = r
			.volume
			.metadata
			.get("topic")
			.map(|s| s.as_str())
			.unwrap_or("uncategorized");
		let age = if now >= r.volume.timestamp {
			format_age(now - r.volume.timestamp)
		} else {
			"0s".to_string()
		};
		let entry = format!(
			"<entry topic=\"{}\" relevance=\"{:.2}\" age=\"{}\">\n{}\n</entry>",
			topic, r.score, age, r.volume.text
		);
		if chars + entry.len() > max_chars {
			break;
		}
		chars += entry.len();
		entries.push(entry);
	}

	format!("<{}>\n{}\n</{}>", tag, entries.join("\n"), tag)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;
	use crate::types::Volume;
	use std::collections::HashMap;

	fn make_lookup(text: &str, score: f64, topic: &str, timestamp: u64) -> Lookup {
		let mut metadata = HashMap::new();
		metadata.insert("topic".to_string(), topic.to_string());
		Lookup {
			volume: Volume {
				id: format!("vol-{}", text.len()),
				text: text.to_string(),
				embedding: vec![],
				metadata,
				timestamp,
			},
			score,
		}
	}

	#[test]
	fn empty_results_returns_empty_string() {
		let result = format_memory_context(&[], &PromptInjectionOptions::default(), 1000);
		assert_eq!(result, "");
	}

	#[test]
	fn structured_format_basic() {
		let results = vec![make_lookup("Hello world", 0.92, "rust", 1000)];
		let opts = PromptInjectionOptions::default();
		let output = format_memory_context(&results, &opts, 2000);
		assert!(output.contains("<memory-context>"));
		assert!(output.contains("</memory-context>"));
		assert!(output.contains("topic=\"rust\""));
		assert!(output.contains("relevance=\"0.92\""));
		assert!(output.contains("Hello world"));
	}

	#[test]
	fn natural_format_basic() {
		let results = vec![make_lookup("Hello world", 0.85, "go", 1000)];
		let opts = PromptInjectionOptions {
			format: Some("natural".to_string()),
			..Default::default()
		};
		let output = format_memory_context(&results, &opts, 2000);
		assert!(output.starts_with("Relevant context from library:"));
		assert!(output.contains("[go]"));
		assert!(output.contains("relevance: 0.85"));
		assert!(output.contains("Hello world"));
	}

	#[test]
	fn min_score_filters_results() {
		let results = vec![
			make_lookup("high score", 0.9, "rust", 1000),
			make_lookup("low score", 0.3, "rust", 1000),
		];
		let opts = PromptInjectionOptions {
			min_score: Some(0.5),
			..Default::default()
		};
		let output = format_memory_context(&results, &opts, 2000);
		assert!(output.contains("high score"));
		assert!(!output.contains("low score"));
	}

	#[test]
	fn min_score_all_filtered_returns_empty() {
		let results = vec![make_lookup("low", 0.2, "rust", 1000)];
		let opts = PromptInjectionOptions {
			min_score: Some(0.5),
			..Default::default()
		};
		let output = format_memory_context(&results, &opts, 2000);
		assert_eq!(output, "");
	}

	#[test]
	fn max_results_limits_output() {
		let results = vec![
			make_lookup("first", 0.9, "rust", 1000),
			make_lookup("second", 0.8, "rust", 1000),
			make_lookup("third", 0.7, "rust", 1000),
		];
		let opts = PromptInjectionOptions {
			max_results: Some(2),
			..Default::default()
		};
		let output = format_memory_context(&results, &opts, 2000);
		assert!(output.contains("first"));
		assert!(output.contains("second"));
		assert!(!output.contains("third"));
	}

	#[test]
	fn custom_tag() {
		let results = vec![make_lookup("text", 0.9, "rust", 1000)];
		let opts = PromptInjectionOptions {
			tag: Some("context".to_string()),
			..Default::default()
		};
		let output = format_memory_context(&results, &opts, 2000);
		assert!(output.contains("<context>"));
		assert!(output.contains("</context>"));
	}

	#[test]
	fn max_chars_truncates() {
		let long_text = "a".repeat(200);
		let results = vec![
			make_lookup(&long_text, 0.9, "rust", 1000),
			make_lookup(&long_text, 0.8, "rust", 1000),
		];
		let opts = PromptInjectionOptions {
			max_chars: Some(300),
			..Default::default()
		};
		let output = format_memory_context(&results, &opts, 2000);
		// Should only include one entry because two would exceed max_chars
		let entry_count = output.matches("<entry").count();
		assert_eq!(entry_count, 1);
	}

	#[test]
	fn format_age_seconds() {
		assert_eq!(format_age(5000), "5s");
		assert_eq!(format_age(59000), "59s");
	}

	#[test]
	fn format_age_minutes() {
		assert_eq!(format_age(60_000), "1m");
		assert_eq!(format_age(3_540_000), "59m");
	}

	#[test]
	fn format_age_hours() {
		assert_eq!(format_age(3_600_000), "1h");
		assert_eq!(format_age(82_800_000), "23h");
	}

	#[test]
	fn format_age_days() {
		assert_eq!(format_age(86_400_000), "1d");
		assert_eq!(format_age(172_800_000), "2d");
	}

	#[test]
	fn structured_age_calculated() {
		// now=10000, timestamp=3000 => age=7000ms => 7s
		let results = vec![make_lookup("text", 0.9, "rust", 3000)];
		let opts = PromptInjectionOptions::default();
		let output = format_memory_context(&results, &opts, 10000);
		assert!(output.contains("age=\"7s\""));
	}

	#[test]
	fn uncategorized_when_no_topic() {
		let results = vec![Lookup {
			volume: Volume {
				id: "vol-1".to_string(),
				text: "no topic text".to_string(),
				embedding: vec![],
				metadata: HashMap::new(),
				timestamp: 1000,
			},
			score: 0.9,
		}];
		let opts = PromptInjectionOptions::default();
		let output = format_memory_context(&results, &opts, 2000);
		assert!(output.contains("topic=\"uncategorized\""));
	}
}
