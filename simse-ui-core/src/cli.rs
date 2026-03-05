//! Non-interactive CLI mode: argument parsing and result formatting.
//!
//! Supports `-p <prompt>` / `--prompt <prompt>` for single-shot generation
//! without REPL. The actual execution happens in simse-tui;
//! this module is purely data + logic (no I/O, no async).

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Output format for non-interactive results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
	Text,
	Json,
}

impl Default for OutputFormat {
	fn default() -> Self {
		Self::Text
	}
}

/// Parsed arguments for a non-interactive invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NonInteractiveArgs {
	pub prompt: String,
	pub format: OutputFormat,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub server_name: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub agent_id: Option<String>,
}

/// Result of a non-interactive execution, ready for formatting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NonInteractiveResult {
	pub output: String,
	pub model: String,
	pub duration_ms: u64,
	pub exit_code: i32,
}

// ---------------------------------------------------------------------------
// Argument parsing
// ---------------------------------------------------------------------------

/// Parse non-interactive arguments from a raw arg slice.
///
/// Returns `None` if no `-p` / `--prompt` flag is found (i.e. interactive
/// mode). Unknown flags are silently skipped, matching the TS behaviour.
pub fn parse_non_interactive_args(args: &[String]) -> Option<NonInteractiveArgs> {
	let mut prompt: Option<String> = None;
	let mut format = OutputFormat::Text;
	let mut server_name: Option<String> = None;
	let mut agent_id: Option<String> = None;

	let mut i = 0;
	while i < args.len() {
		let arg = &args[i];
		let next = args.get(i + 1);

		match arg.as_str() {
			"-p" | "--prompt" => {
				if let Some(val) = next {
					prompt = Some(val.clone());
					i += 2;
					continue;
				}
			}
			"--format" => {
				if let Some(val) = next {
					match val.as_str() {
						"json" => format = OutputFormat::Json,
						"text" => format = OutputFormat::Text,
						_ => {} // ignore invalid format values
					}
					i += 2;
					continue;
				}
			}
			"--server" => {
				if let Some(val) = next {
					server_name = Some(val.clone());
					i += 2;
					continue;
				}
			}
			"--agent" => {
				if let Some(val) = next {
					agent_id = Some(val.clone());
					i += 2;
					continue;
				}
			}
			_ => {}
		}

		i += 1;
	}

	let prompt = prompt?;

	Some(NonInteractiveArgs {
		prompt,
		format,
		server_name,
		agent_id,
	})
}

/// Format the result for output based on the requested format.
///
/// - `Text` returns `result.output` verbatim.
/// - `Json` returns a JSON object with `output`, `model`, and `durationMs`,
///   indented with tabs (matching the TS reference).
pub fn format_non_interactive_result(
	result: &NonInteractiveResult,
	format: OutputFormat,
) -> String {
	match format {
		OutputFormat::Text => result.output.clone(),
		OutputFormat::Json => {
			// Build a minimal JSON object matching the TS shape:
			// { "output": "...", "model": "...", "durationMs": N }
			let obj = serde_json::json!({
				"output": result.output,
				"model": result.model,
				"durationMs": result.duration_ms,
			});
			// TS uses JSON.stringify(…, null, '\t') — tab-indented.
			// serde_json's pretty_printer uses spaces; we replicate tab indent manually.
			format_json_with_tabs(&obj)
		}
	}
}

/// Check if the given args indicate non-interactive mode.
pub fn is_non_interactive(args: &[String]) -> bool {
	args.iter()
		.any(|a| a == "-p" || a == "--prompt")
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Serialize a JSON value with tab indentation to match the TS
/// `JSON.stringify(value, null, '\t')` output.
fn format_json_with_tabs(value: &serde_json::Value) -> String {
	format_json_value(value, 0)
}

fn format_json_value(value: &serde_json::Value, depth: usize) -> String {
	match value {
		serde_json::Value::Object(map) => {
			if map.is_empty() {
				return "{}".to_string();
			}
			let indent = "\t".repeat(depth + 1);
			let closing_indent = "\t".repeat(depth);
			let entries: Vec<String> = map
				.iter()
				.map(|(k, v)| {
					format!(
						"{}\"{}\": {}",
						indent,
						escape_json_string(k),
						format_json_value(v, depth + 1)
					)
				})
				.collect();
			format!("{{\n{}\n{}}}", entries.join(",\n"), closing_indent)
		}
		serde_json::Value::Array(arr) => {
			if arr.is_empty() {
				return "[]".to_string();
			}
			let indent = "\t".repeat(depth + 1);
			let closing_indent = "\t".repeat(depth);
			let entries: Vec<String> = arr
				.iter()
				.map(|v| format!("{}{}", indent, format_json_value(v, depth + 1)))
				.collect();
			format!("[\n{}\n{}]", entries.join(",\n"), closing_indent)
		}
		serde_json::Value::String(s) => format!("\"{}\"", escape_json_string(s)),
		serde_json::Value::Number(n) => n.to_string(),
		serde_json::Value::Bool(b) => b.to_string(),
		serde_json::Value::Null => "null".to_string(),
	}
}

fn escape_json_string(s: &str) -> String {
	let mut out = String::with_capacity(s.len());
	for ch in s.chars() {
		match ch {
			'"' => out.push_str("\\\""),
			'\\' => out.push_str("\\\\"),
			'\n' => out.push_str("\\n"),
			'\r' => out.push_str("\\r"),
			'\t' => out.push_str("\\t"),
			c if c < '\u{20}' => {
				out.push_str(&format!("\\u{:04x}", c as u32));
			}
			c => out.push(c),
		}
	}
	out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;

	// -- is_non_interactive ---------------------------------------------------

	#[test]
	fn is_non_interactive_short_flag() {
		let args: Vec<String> = vec!["-p".into(), "hello".into()];
		assert!(is_non_interactive(&args));
	}

	#[test]
	fn is_non_interactive_long_flag() {
		let args: Vec<String> = vec!["--prompt".into(), "hello".into()];
		assert!(is_non_interactive(&args));
	}

	#[test]
	fn is_non_interactive_false_without_flag() {
		let args: Vec<String> = vec!["--format".into(), "json".into()];
		assert!(!is_non_interactive(&args));
	}

	#[test]
	fn is_non_interactive_empty_args() {
		let args: Vec<String> = vec![];
		assert!(!is_non_interactive(&args));
	}

	// -- parse_non_interactive_args -------------------------------------------

	#[test]
	fn parse_prompt_short() {
		let args: Vec<String> = vec!["-p".into(), "say hi".into()];
		let parsed = parse_non_interactive_args(&args).unwrap();
		assert_eq!(parsed.prompt, "say hi");
		assert_eq!(parsed.format, OutputFormat::Text);
		assert!(parsed.server_name.is_none());
		assert!(parsed.agent_id.is_none());
	}

	#[test]
	fn parse_prompt_long() {
		let args: Vec<String> = vec!["--prompt".into(), "say hi".into()];
		let parsed = parse_non_interactive_args(&args).unwrap();
		assert_eq!(parsed.prompt, "say hi");
	}

	#[test]
	fn parse_all_flags() {
		let args: Vec<String> = vec![
			"-p".into(),
			"do stuff".into(),
			"--format".into(),
			"json".into(),
			"--server".into(),
			"my-server".into(),
			"--agent".into(),
			"agent-1".into(),
		];
		let parsed = parse_non_interactive_args(&args).unwrap();
		assert_eq!(parsed.prompt, "do stuff");
		assert_eq!(parsed.format, OutputFormat::Json);
		assert_eq!(parsed.server_name.as_deref(), Some("my-server"));
		assert_eq!(parsed.agent_id.as_deref(), Some("agent-1"));
	}

	#[test]
	fn parse_no_prompt_returns_none() {
		let args: Vec<String> = vec!["--format".into(), "json".into()];
		assert!(parse_non_interactive_args(&args).is_none());
	}

	#[test]
	fn parse_empty_args_returns_none() {
		let args: Vec<String> = vec![];
		assert!(parse_non_interactive_args(&args).is_none());
	}

	#[test]
	fn parse_prompt_without_value_returns_none() {
		let args: Vec<String> = vec!["-p".into()];
		assert!(parse_non_interactive_args(&args).is_none());
	}

	#[test]
	fn parse_invalid_format_defaults_to_text() {
		let args: Vec<String> = vec![
			"-p".into(),
			"hello".into(),
			"--format".into(),
			"yaml".into(),
		];
		let parsed = parse_non_interactive_args(&args).unwrap();
		assert_eq!(parsed.format, OutputFormat::Text);
	}

	#[test]
	fn parse_flags_any_order() {
		let args: Vec<String> = vec![
			"--agent".into(),
			"a1".into(),
			"--format".into(),
			"json".into(),
			"-p".into(),
			"query".into(),
			"--server".into(),
			"srv".into(),
		];
		let parsed = parse_non_interactive_args(&args).unwrap();
		assert_eq!(parsed.prompt, "query");
		assert_eq!(parsed.format, OutputFormat::Json);
		assert_eq!(parsed.server_name.as_deref(), Some("srv"));
		assert_eq!(parsed.agent_id.as_deref(), Some("a1"));
	}

	#[test]
	fn parse_unknown_flags_ignored() {
		let args: Vec<String> = vec![
			"--unknown".into(),
			"val".into(),
			"-p".into(),
			"hi".into(),
		];
		let parsed = parse_non_interactive_args(&args).unwrap();
		assert_eq!(parsed.prompt, "hi");
	}

	// -- format_non_interactive_result ----------------------------------------

	#[test]
	fn format_text_returns_output_verbatim() {
		let result = NonInteractiveResult {
			output: "Hello, world!".into(),
			model: "gpt-4".into(),
			duration_ms: 123,
			exit_code: 0,
		};
		let formatted = format_non_interactive_result(&result, OutputFormat::Text);
		assert_eq!(formatted, "Hello, world!");
	}

	#[test]
	fn format_json_has_tab_indent() {
		let result = NonInteractiveResult {
			output: "response".into(),
			model: "claude-3".into(),
			duration_ms: 456,
			exit_code: 0,
		};
		let formatted = format_non_interactive_result(&result, OutputFormat::Json);

		// Must parse as valid JSON
		let parsed: serde_json::Value = serde_json::from_str(&formatted).unwrap();
		assert_eq!(parsed["output"], "response");
		assert_eq!(parsed["model"], "claude-3");
		assert_eq!(parsed["durationMs"], 456);

		// Must be tab-indented (like JSON.stringify(…, null, '\t'))
		assert!(formatted.contains('\t'));
		assert!(formatted.contains("\n\t\"output\""));
	}

	#[test]
	fn format_json_excludes_exit_code() {
		let result = NonInteractiveResult {
			output: "out".into(),
			model: "m".into(),
			duration_ms: 0,
			exit_code: 1,
		};
		let formatted = format_non_interactive_result(&result, OutputFormat::Json);
		let parsed: serde_json::Value = serde_json::from_str(&formatted).unwrap();
		assert!(parsed.get("exitCode").is_none());
		assert!(parsed.get("exit_code").is_none());
	}

	#[test]
	fn format_json_special_characters_escaped() {
		let result = NonInteractiveResult {
			output: "line1\nline2\ttab\"quote".into(),
			model: "m".into(),
			duration_ms: 0,
			exit_code: 0,
		};
		let formatted = format_non_interactive_result(&result, OutputFormat::Json);
		// Must be valid JSON
		let parsed: serde_json::Value = serde_json::from_str(&formatted).unwrap();
		assert_eq!(parsed["output"], "line1\nline2\ttab\"quote");
	}

	// -- serde roundtrip ------------------------------------------------------

	#[test]
	fn output_format_serde_roundtrip() {
		let json = serde_json::to_string(&OutputFormat::Json).unwrap();
		assert_eq!(json, "\"json\"");
		let text = serde_json::to_string(&OutputFormat::Text).unwrap();
		assert_eq!(text, "\"text\"");

		let rt: OutputFormat = serde_json::from_str("\"json\"").unwrap();
		assert_eq!(rt, OutputFormat::Json);
		let rt: OutputFormat = serde_json::from_str("\"text\"").unwrap();
		assert_eq!(rt, OutputFormat::Text);
	}

	#[test]
	fn non_interactive_args_serde_roundtrip() {
		let args = NonInteractiveArgs {
			prompt: "hello".into(),
			format: OutputFormat::Json,
			server_name: Some("srv".into()),
			agent_id: None,
		};
		let json = serde_json::to_string(&args).unwrap();
		let rt: NonInteractiveArgs = serde_json::from_str(&json).unwrap();
		assert_eq!(rt, args);

		// agent_id=None should be absent from JSON
		assert!(!json.contains("agent_id"));
	}
}
