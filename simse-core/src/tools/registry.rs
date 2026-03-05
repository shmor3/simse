//! Tool Registry — registers tools, parses tool calls, executes handlers.
//!
//! Ports `src/ai/tools/tool-registry.ts` (~416 lines of TS) to Rust.
//! Uses interior mutability for metrics so `execute` can take `&self`.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use regex::Regex;

use async_trait::async_trait;

use crate::agentic_loop::ToolExecutor;
use crate::tools::permissions::ToolPermissionResolver;
use crate::tools::types::{
	ParsedResponse, RegisteredTool, ToolCallRequest, ToolCallResult, ToolDefinition, ToolHandler,
	ToolMetrics, ToolRegistryOptions,
};

// ---------------------------------------------------------------------------
// Static regex
// ---------------------------------------------------------------------------

/// Regex for extracting `<tool_use>...</tool_use>` blocks from model output.
static TOOL_USE_RE: LazyLock<Regex> =
	LazyLock::new(|| Regex::new(r"<tool_use>\s*([\s\S]*?)\s*</tool_use>").unwrap());

/// Regex for stripping `<tool_use>...</tool_use>` blocks from text.
static TOOL_USE_STRIP_RE: LazyLock<Regex> =
	LazyLock::new(|| Regex::new(r"<tool_use>\s*[\s\S]*?\s*</tool_use>").unwrap());

// ---------------------------------------------------------------------------
// Internal metrics entry
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct MetricsEntry {
	calls: u64,
	errors: u64,
	total_ms: u64,
	last_at: u64,
}

// ---------------------------------------------------------------------------
// ToolRegistry
// ---------------------------------------------------------------------------

/// Registry that manages tool definitions, handlers, execution, and metrics.
///
/// # Interior mutability
///
/// The `metrics` map uses `Mutex` so that `execute` and `batch_execute` can
/// take `&self` while still recording call statistics.
pub struct ToolRegistry {
	tools: HashMap<String, RegisteredTool>,
	metrics: Mutex<HashMap<String, MetricsEntry>>,
	default_timeout_ms: Option<u64>,
	default_max_output_chars: usize,
	permission_resolver: Option<Arc<dyn ToolPermissionResolver>>,
}

impl std::fmt::Debug for ToolRegistry {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("ToolRegistry")
			.field("tool_count", &self.tools.len())
			.field("default_timeout_ms", &self.default_timeout_ms)
			.field("default_max_output_chars", &self.default_max_output_chars)
			.field(
				"permission_resolver",
				&self.permission_resolver.is_some(),
			)
			.finish()
	}
}

impl ToolRegistry {
	/// Creates a new tool registry with the given options.
	pub fn new(options: ToolRegistryOptions) -> Self {
		Self {
			tools: HashMap::new(),
			metrics: Mutex::new(HashMap::new()),
			default_timeout_ms: options.default_timeout_ms,
			default_max_output_chars: options.max_output_chars.unwrap_or(50_000),
			permission_resolver: options.permission_resolver,
		}
	}

	// -------------------------------------------------------------------
	// Registration
	// -------------------------------------------------------------------

	/// Registers a tool with its handler. Overwrites any existing tool with
	/// the same name.
	pub fn register(&mut self, definition: ToolDefinition, handler: ToolHandler) {
		self.tools
			.insert(definition.name.clone(), RegisteredTool { definition, handler });
	}

	/// Removes a tool by name. Returns `true` if the tool was found and removed.
	pub fn unregister(&mut self, name: &str) -> bool {
		self.tools.remove(name).is_some()
	}

	/// Returns `true` if a tool with the given name is registered.
	pub fn is_registered(&self, name: &str) -> bool {
		self.tools.contains_key(name)
	}

	/// Returns the number of registered tools.
	pub fn tool_count(&self) -> usize {
		self.tools.len()
	}

	/// Returns the names of all registered tools.
	pub fn tool_names(&self) -> Vec<String> {
		self.tools.keys().cloned().collect()
	}

	/// Returns a list of references to all tool definitions.
	pub fn get_tool_definitions(&self) -> Vec<&ToolDefinition> {
		self.tools.values().map(|t| &t.definition).collect()
	}

	/// Returns the definition for a specific tool, if registered.
	pub fn get_tool_definition(&self, name: &str) -> Option<&ToolDefinition> {
		self.tools.get(name).map(|t| &t.definition)
	}

	// -------------------------------------------------------------------
	// Execution
	// -------------------------------------------------------------------

	/// Executes a single tool call.
	///
	/// 1. Looks up the tool (returns error result if not found).
	/// 2. Checks the permission resolver (returns error result if denied).
	/// 3. Runs the handler with an optional timeout.
	/// 4. Truncates output if it exceeds the character limit.
	/// 5. Records metrics.
	pub async fn execute(&self, call: &ToolCallRequest) -> ToolCallResult {
		let registered = match self.tools.get(&call.name) {
			Some(r) => r,
			None => {
				return ToolCallResult {
					id: call.id.clone(),
					name: call.name.clone(),
					output: format!("Tool not found: \"{}\"", call.name),
					is_error: true,
					duration_ms: None,
					diff: None,
				};
			}
		};

		// Permission check
		if let Some(resolver) = &self.permission_resolver {
			let allowed = resolver
				.check(call, Some(&registered.definition))
				.await;
			if !allowed {
				return ToolCallResult {
					id: call.id.clone(),
					name: call.name.clone(),
					output: format!("Permission denied for tool: \"{}\"", call.name),
					is_error: true,
					duration_ms: None,
					diff: None,
				};
			}
		}

		let start = Instant::now();
		let mut is_error = false;

		let result = {
			let timeout_ms = registered
				.definition
				.timeout_ms
				.or(self.default_timeout_ms);

			let handler = Arc::clone(&registered.handler);
			let args = call.arguments.clone();

			let handler_result = if let Some(ms) = timeout_ms {
				let duration = Duration::from_millis(ms);
				let label = format!("tool:{}", call.name);
				crate::utils::timeout::with_timeout(
					async move { handler(args).await },
					duration,
					Some(&label),
				)
				.await
			} else {
				handler(args).await
			};

			match handler_result {
				Ok(mut output) => {
					// Truncate oversized output
					let char_limit = registered
						.definition
						.max_output_chars
						.unwrap_or(self.default_max_output_chars);
					if char_limit > 0 && output.len() > char_limit {
						let total = output.len();
						// Find nearest char boundary at or before limit to avoid
						// panicking on multi-byte UTF-8 sequences
						let mut truncate_at = char_limit.min(output.len());
						while truncate_at > 0 && !output.is_char_boundary(truncate_at) {
							truncate_at -= 1;
						}
						output.truncate(truncate_at);
						output.push_str(&format!(
							"\n\n[OUTPUT TRUNCATED — {} chars total, showing first {}]",
							total, char_limit
						));
					}

					ToolCallResult {
						id: call.id.clone(),
						name: call.name.clone(),
						output,
						is_error: false,
						duration_ms: Some(start.elapsed().as_millis() as u64),
						diff: None,
					}
				}
				Err(err) => {
					is_error = true;
					ToolCallResult {
						id: call.id.clone(),
						name: call.name.clone(),
						output: format!(
							"Tool execution failed for \"{}\": {}",
							call.name, err
						),
						is_error: true,
						duration_ms: Some(start.elapsed().as_millis() as u64),
						diff: None,
					}
				}
			}
		};

		// Record metrics
		let elapsed_ms = start.elapsed().as_millis() as u64;
		let now_ms = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.unwrap_or_default()
			.as_millis() as u64;
		{
			let mut metrics = self.metrics.lock().unwrap_or_else(|e| e.into_inner());
			let entry = metrics
				.entry(call.name.clone())
				.or_insert(MetricsEntry {
					calls: 0,
					errors: 0,
					total_ms: 0,
					last_at: 0,
				});
			entry.calls += 1;
			if is_error {
				entry.errors += 1;
			}
			entry.total_ms += elapsed_ms;
			entry.last_at = now_ms;
		}

		result
	}

	/// Executes multiple tool calls concurrently with bounded parallelism.
	///
	/// Uses a tokio semaphore to limit concurrency (default: 8).
	pub async fn batch_execute(
		&self,
		calls: &[ToolCallRequest],
		max_concurrency: Option<usize>,
	) -> Vec<ToolCallResult> {
		if calls.is_empty() {
			return Vec::new();
		}

		let concurrency = max_concurrency.unwrap_or(8).max(1);
		let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency));

		let futures: Vec<_> = calls
			.iter()
			.map(|call| {
				let sem = Arc::clone(&semaphore);
				async move {
					let _permit = sem.acquire().await.unwrap();
					self.execute(call).await
				}
			})
			.collect();

		// Use futures::future::join_all to run them concurrently
		futures::future::join_all(futures).await
	}

	// -------------------------------------------------------------------
	// Parsing
	// -------------------------------------------------------------------

	/// Parses `<tool_use>` blocks from a model response string.
	///
	/// Extracts JSON objects like `{"id": "...", "name": "...", "arguments": {...}}`.
	/// Malformed JSON is silently skipped. The returned `text` field contains
	/// the response with all `<tool_use>` blocks stripped.
	pub fn parse_tool_calls(response: &str) -> ParsedResponse {
		let mut tool_calls = Vec::new();

		for cap in TOOL_USE_RE.captures_iter(response) {
			let json_str = cap[1].trim();
			if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str) {
				let name = match parsed.get("name").and_then(|v| v.as_str()) {
					Some(n) => n.to_string(),
					None => continue,
				};
				let id = parsed
					.get("id")
					.and_then(|v| v.as_str())
					.map(|s| s.to_string())
					.unwrap_or_else(|| format!("call_{}", tool_calls.len() + 1));
				let arguments = parsed
					.get("arguments")
					.cloned()
					.unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

				tool_calls.push(ToolCallRequest {
					id,
					name,
					arguments,
				});
			}
			// Malformed JSON — skip
		}

		let text = TOOL_USE_STRIP_RE
			.replace_all(response, "")
			.trim()
			.to_string();

		ParsedResponse { text, tool_calls }
	}

	// -------------------------------------------------------------------
	// System prompt formatting
	// -------------------------------------------------------------------

	/// Formats all registered tools as a system prompt section.
	///
	/// Includes usage instructions and a list of each tool with its name,
	/// description, and parameters.
	pub fn format_for_system_prompt(&self) -> String {
		if self.tools.is_empty() {
			return String::new();
		}

		let mut lines = vec![
			"You have access to tools. To use a tool, include a JSON block wrapped in <tool_use> tags:".to_string(),
			String::new(),
			"<tool_use>".to_string(),
			r#"{"id": "call_1", "name": "tool_name", "arguments": {"key": "value"}}"#.to_string(),
			"</tool_use>".to_string(),
			String::new(),
			"You can call multiple tools in one response. After tool results are provided, continue your response.".to_string(),
			"Only use tools when necessary — if you can answer directly, do so.".to_string(),
			String::new(),
			"Available tools:".to_string(),
			String::new(),
		];

		for tool in self.tools.values() {
			lines.push(format!(
				"- {}: {}",
				tool.definition.name, tool.definition.description
			));
			if !tool.definition.parameters.is_empty() {
				let param_desc: Vec<String> = tool
					.definition
					.parameters
					.iter()
					.map(|(k, v)| {
						if v.required {
							format!("{} ({}, required)", k, v.param_type)
						} else {
							format!("{} ({})", k, v.param_type)
						}
					})
					.collect();
				lines.push(format!("  Parameters: {}", param_desc.join(", ")));
			}
			lines.push(String::new());
		}

		lines.join("\n")
	}

	// -------------------------------------------------------------------
	// Metrics
	// -------------------------------------------------------------------

	/// Returns metrics for a specific tool, if any calls have been recorded.
	pub fn get_tool_metrics(&self, name: &str) -> Option<ToolMetrics> {
		let metrics = self.metrics.lock().unwrap_or_else(|e| e.into_inner());
		metrics.get(name).map(|m| ToolMetrics {
			name: name.to_string(),
			call_count: m.calls,
			error_count: m.errors,
			total_duration_ms: m.total_ms,
			avg_duration_ms: if m.calls > 0 {
				m.total_ms as f64 / m.calls as f64
			} else {
				0.0
			},
			last_called_at: m.last_at,
		})
	}

	/// Returns metrics for all tools that have been called.
	pub fn get_all_tool_metrics(&self) -> Vec<ToolMetrics> {
		let metrics = self.metrics.lock().unwrap_or_else(|e| e.into_inner());
		metrics
			.iter()
			.map(|(name, m)| ToolMetrics {
				name: name.clone(),
				call_count: m.calls,
				error_count: m.errors,
				total_duration_ms: m.total_ms,
				avg_duration_ms: if m.calls > 0 {
					m.total_ms as f64 / m.calls as f64
				} else {
					0.0
				},
				last_called_at: m.last_at,
			})
			.collect()
	}

	/// Clears all recorded metrics.
	pub fn clear_metrics(&self) {
		let mut metrics = self.metrics.lock().unwrap_or_else(|e| e.into_inner());
		metrics.clear();
	}
}

// ---------------------------------------------------------------------------
// ToolExecutor trait implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl ToolExecutor for ToolRegistry {
	fn parse_tool_calls(&self, response: &str) -> ParsedResponse {
		ToolRegistry::parse_tool_calls(response)
	}

	async fn execute(&self, call: &ToolCallRequest) -> ToolCallResult {
		Self::execute(self, call).await
	}
}
