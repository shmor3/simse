//! Configuration system with typed validation and `define_config`.
//!
//! Ports `src/config/schema.ts` + `src/config/settings.ts` (~1,160 lines).
//!
//! - `AppConfig` struct with nested `AcpConfig`, `McpConfig`, `LibraryConfig`,
//!   `ToolsConfig`, `LoopConfig`, `PromptsConfig`, chain types
//! - All derive `Serialize, Deserialize, Clone, Debug, Default`
//! - `define_config(raw, opts)` validates and resolves a JSON value into `AppConfig`
//! - Lenient mode: reset invalid fields to defaults + call `on_warn`

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{ConfigErrorCode, SimseError};

// ---------------------------------------------------------------------------
// Static regexes (compiled once via LazyLock)
// ---------------------------------------------------------------------------

static SEMVER_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d+\.\d+\.\d+$").unwrap());
static STEP_NAME_RE: LazyLock<Regex> =
	LazyLock::new(|| Regex::new(r"^[a-zA-Z_][\w-]*$").unwrap());

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Permission policy for ACP server connections.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PermissionPolicy {
	AutoApprove,
	Prompt,
	Deny,
}

/// MCP transport type.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum McpTransport {
	#[default]
	Stdio,
	Http,
}

/// Provider type for chain steps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StepProvider {
	Acp,
	Mcp,
	Memory,
}

/// Merge strategy for parallel steps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MergeStrategy {
	Concat,
	Keyed,
}

// ---------------------------------------------------------------------------
// ACP types
// ---------------------------------------------------------------------------

/// ACP server entry configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct AcpServerEntry {
	pub name: String,
	pub command: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub args: Option<Vec<String>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub cwd: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub env: Option<HashMap<String, String>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub default_agent: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub timeout_ms: Option<u64>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub permission_policy: Option<PermissionPolicy>,
}

/// ACP configuration section.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct AcpConfig {
	pub servers: Vec<AcpServerEntry>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub default_server: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub default_agent: Option<String>,
}

// ---------------------------------------------------------------------------
// MCP types
// ---------------------------------------------------------------------------

/// MCP server connection configuration (stdio or HTTP).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct McpServerConnection {
	pub name: String,
	#[serde(default)]
	pub transport: McpTransport,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub command: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub args: Option<Vec<String>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub env: Option<HashMap<String, String>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub url: Option<String>,
}

/// MCP client configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct McpClientConfig {
	pub servers: Vec<McpServerConnection>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub client_name: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub client_version: Option<String>,
}

/// MCP server configuration (built-in server mode).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct McpServerConfig {
	pub enabled: bool,
	#[serde(default)]
	pub transport: McpTransport,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub name: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub version: Option<String>,
}

/// Combined MCP configuration (client + server).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct McpConfig {
	pub client: McpClientConfig,
	pub server: McpServerConfig,
}

// ---------------------------------------------------------------------------
// Library config (formerly MemoryConfig)
// ---------------------------------------------------------------------------

/// Library (vector store) configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct LibraryConfig {
	pub enabled: bool,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub embedding_agent: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub similarity_threshold: Option<f64>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub max_results: Option<u32>,
}

// ---------------------------------------------------------------------------
// Tools config
// ---------------------------------------------------------------------------

/// Tool registry configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ToolsConfig {
	pub max_output_chars: Option<usize>,
}

// ---------------------------------------------------------------------------
// Loop config
// ---------------------------------------------------------------------------

/// Agentic loop configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct LoopConfig {
	pub max_turns: Option<u32>,
	pub max_identical_tool_calls: Option<u32>,
	pub auto_compact_chars: Option<usize>,
}

// ---------------------------------------------------------------------------
// Prompts config
// ---------------------------------------------------------------------------

/// Prompts configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct PromptsConfig {
	pub system_prompt: Option<String>,
}

// ---------------------------------------------------------------------------
// Chain types
// ---------------------------------------------------------------------------

/// Parallel sub-step definition within a parallel config.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ParallelSubStepDefinition {
	pub name: String,
	pub template: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub provider: Option<StepProvider>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub agent_id: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub server_name: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub agent_config: Option<HashMap<String, Value>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub system_prompt: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub mcp_server_name: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub mcp_tool_name: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub mcp_arguments: Option<HashMap<String, String>>,
}

/// Parallel execution configuration within a chain step.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ParallelConfigDefinition {
	pub sub_steps: Vec<ParallelSubStepDefinition>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub merge_strategy: Option<MergeStrategy>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub fail_tolerant: Option<bool>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub concat_separator: Option<String>,
}

/// Chain step definition.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ChainStepDefinition {
	pub name: String,
	pub template: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub provider: Option<StepProvider>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub agent_id: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub server_name: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub agent_config: Option<HashMap<String, Value>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub system_prompt: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub input_mapping: Option<HashMap<String, String>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub mcp_server_name: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub mcp_tool_name: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub mcp_arguments: Option<HashMap<String, String>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub store_to_memory: Option<bool>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub memory_metadata: Option<HashMap<String, String>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub parallel: Option<ParallelConfigDefinition>,
}

/// Chain definition.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ChainDefinition {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub description: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub agent_id: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub server_name: Option<String>,
	#[serde(default)]
	pub initial_values: HashMap<String, String>,
	pub steps: Vec<ChainStepDefinition>,
}

// ---------------------------------------------------------------------------
// AppConfig — top-level
// ---------------------------------------------------------------------------

/// Fully resolved application configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
	pub acp: AcpConfig,
	pub mcp: McpConfig,
	pub library: LibraryConfig,
	pub tools: ToolsConfig,
	#[serde(rename = "loop")]
	pub loop_config: LoopConfig,
	pub prompts: PromptsConfig,
	#[serde(default)]
	pub chains: HashMap<String, ChainDefinition>,
}

// ---------------------------------------------------------------------------
// Validation primitives
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ValidationIssue {
	pub path: String,
	pub message: String,
}

fn issue(path: impl Into<String>, message: impl Into<String>) -> Vec<ValidationIssue> {
	vec![ValidationIssue {
		path: path.into(),
		message: message.into(),
	}]
}

fn validate_non_empty(value: &str, path: &str, label: &str) -> Vec<ValidationIssue> {
	if value.is_empty() {
		issue(path, format!("{label} cannot be empty"))
	} else {
		Vec::new()
	}
}

fn validate_url(value: &str, path: &str, label: &str) -> Vec<ValidationIssue> {
	// Basic URL validation: must have a scheme (protocol) followed by ://
	// Matches the behaviour of JS `new URL(value)` which requires a valid scheme.
	let has_scheme = value.contains("://");
	let scheme_part = value.split("://").next().unwrap_or("");
	let valid_scheme = !scheme_part.is_empty()
		&& scheme_part
			.chars()
			.all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.');
	let has_host = value
		.split("://")
		.nth(1)
		.is_some_and(|rest| !rest.is_empty());

	if !has_scheme || !valid_scheme || !has_host {
		issue(path, format!("{label} must be a valid URL"))
	} else {
		Vec::new()
	}
}

struct RangeConstraints {
	min: Option<f64>,
	max: Option<f64>,
	integer: bool,
}

fn validate_range(
	value: f64,
	path: &str,
	label: &str,
	constraints: &RangeConstraints,
) -> Vec<ValidationIssue> {
	if value.is_nan() {
		return issue(path, format!("{label} must be a number"));
	}
	if constraints.integer && value.fract() != 0.0 {
		return issue(path, format!("{label} must be an integer"));
	}
	if let Some(min) = constraints.min {
		if value < min {
			return issue(path, format!("{label} must be at least {min}"));
		}
	}
	if let Some(max) = constraints.max {
		if value > max {
			return issue(path, format!("{label} must be at most {max}"));
		}
	}
	Vec::new()
}

// ---------------------------------------------------------------------------
// ACP validation
// ---------------------------------------------------------------------------

fn validate_acp_server_entry(value: &AcpServerEntry, path: &str) -> Vec<ValidationIssue> {
	let mut issues = Vec::new();
	issues.extend(validate_non_empty(&value.name, &format!("{path}.name"), "ACP server name"));
	issues.extend(validate_non_empty(
		&value.command,
		&format!("{path}.command"),
		"ACP server command",
	));
	if let Some(ref agent) = value.default_agent {
		issues.extend(validate_non_empty(
			agent,
			&format!("{path}.defaultAgent"),
			"Default agent ID",
		));
	}
	if let Some(timeout) = value.timeout_ms {
		issues.extend(validate_range(
			timeout as f64,
			&format!("{path}.timeoutMs"),
			"timeoutMs",
			&RangeConstraints {
				min: Some(1000.0),
				max: Some(600_000.0),
				integer: true,
			},
		));
	}
	issues
}

fn validate_acp_config(value: &AcpConfig, path: &str) -> Vec<ValidationIssue> {
	if value.servers.is_empty() {
		return issue(
			format!("{path}.servers"),
			"At least one ACP server must be configured",
		);
	}

	let mut issues = Vec::new();

	for (i, server) in value.servers.iter().enumerate() {
		issues.extend(validate_acp_server_entry(server, &format!("{path}.servers[{i}]")));
	}

	// Detect duplicate server names (only if no prior issues)
	if issues.is_empty() {
		let mut seen = HashSet::new();
		for server in &value.servers {
			if !seen.insert(&server.name) {
				issues.extend(issue(
					format!("{path}.servers"),
					format!("Duplicate ACP server name: \"{}\"", server.name),
				));
			}
		}
	}

	if let Some(ref default_server) = value.default_server {
		issues.extend(validate_non_empty(
			default_server,
			&format!("{path}.defaultServer"),
			"Default server name",
		));
	}

	if let Some(ref default_agent) = value.default_agent {
		issues.extend(validate_non_empty(
			default_agent,
			&format!("{path}.defaultAgent"),
			"Default agent ID",
		));
	}

	// Cross-validate: defaultServer must reference a configured server name
	if let Some(ref default_server) = value.default_server {
		if !default_server.is_empty() {
			let server_names: HashSet<&str> = value.servers.iter().map(|s| s.name.as_str()).collect();
			if !server_names.contains(default_server.as_str()) {
				let names: Vec<&str> = value.servers.iter().map(|s| s.name.as_str()).collect();
				issues.extend(issue(
					format!("{path}.defaultServer"),
					format!(
						"Default server \"{}\" is not defined in servers (available: {})",
						default_server,
						names.join(", ")
					),
				));
			}
		}
	}

	issues
}

// ---------------------------------------------------------------------------
// MCP validation
// ---------------------------------------------------------------------------

fn validate_mcp_server_connection(value: &McpServerConnection, path: &str) -> Vec<ValidationIssue> {
	let mut issues = Vec::new();

	issues.extend(validate_non_empty(&value.name, &format!("{path}.name"), "MCP server name"));

	match value.transport {
		McpTransport::Stdio => {
			if let Some(ref command) = value.command {
				issues.extend(validate_non_empty(command, &format!("{path}.command"), "stdio command"));
			} else {
				issues.extend(issue(format!("{path}.command"), "stdio command cannot be empty"));
			}
		}
		McpTransport::Http => {
			if let Some(ref url_str) = value.url {
				issues.extend(validate_url(url_str, &format!("{path}.url"), "http URL"));
			} else {
				issues.extend(issue(format!("{path}.url"), "http URL must be a valid URL"));
			}
		}
	}

	issues
}

fn validate_mcp_client_config(value: &McpClientConfig, path: &str) -> Vec<ValidationIssue> {
	if value.servers.is_empty() {
		return Vec::new();
	}

	let mut issues = Vec::new();

	for (i, server) in value.servers.iter().enumerate() {
		issues.extend(validate_mcp_server_connection(
			server,
			&format!("{path}.servers[{i}]"),
		));
	}

	// Detect duplicate server names
	if issues.is_empty() {
		let mut seen = HashSet::new();
		for server in &value.servers {
			if !seen.insert(&server.name) {
				issues.extend(issue(
					format!("{path}.servers"),
					format!("Duplicate MCP server name: \"{}\"", server.name),
				));
			}
		}
	}

	issues
}

fn validate_mcp_server_config(value: &McpServerConfig, path: &str) -> Vec<ValidationIssue> {
	let mut issues = Vec::new();
	let enabled = value.enabled;

	if enabled && value.name.is_none() {
		issues.extend(issue(
			format!("{path}.name"),
			"MCP server name is required when enabled",
		));
	}

	if let Some(ref name) = value.name {
		issues.extend(validate_non_empty(name, &format!("{path}.name"), "MCP server name"));
	}

	if enabled && value.version.is_none() {
		issues.extend(issue(
			format!("{path}.version"),
			"MCP server version is required when enabled",
		));
	}

	if let Some(ref version) = value.version {
		if !SEMVER_RE.is_match(version) {
			issues.extend(issue(
				format!("{path}.version"),
				"MCP server version must be semver (e.g. 1.0.0)",
			));
		}
	}

	issues
}

fn validate_mcp_config(value: &McpConfig, path: &str) -> Vec<ValidationIssue> {
	let mut issues = Vec::new();
	issues.extend(validate_mcp_client_config(&value.client, &format!("{path}.client")));
	issues.extend(validate_mcp_server_config(&value.server, &format!("{path}.server")));
	issues
}

// ---------------------------------------------------------------------------
// Library validation
// ---------------------------------------------------------------------------

fn validate_library_config(value: &LibraryConfig, path: &str) -> Vec<ValidationIssue> {
	let enabled = value.enabled;
	let mut issues = Vec::new();

	if enabled && value.embedding_agent.is_none() {
		issues.extend(issue(
			format!("{path}.embeddingAgent"),
			"Embedding agent ID is required when library is enabled",
		));
	}

	if let Some(ref agent) = value.embedding_agent {
		issues.extend(validate_non_empty(
			agent,
			&format!("{path}.embeddingAgent"),
			"Embedding agent ID",
		));
	}

	if enabled && value.similarity_threshold.is_none() {
		issues.extend(issue(
			format!("{path}.similarityThreshold"),
			"Similarity threshold is required when library is enabled",
		));
	}

	if let Some(threshold) = value.similarity_threshold {
		issues.extend(validate_range(
			threshold,
			&format!("{path}.similarityThreshold"),
			"Similarity threshold",
			&RangeConstraints {
				min: Some(0.0),
				max: Some(1.0),
				integer: false,
			},
		));
	}

	if enabled && value.max_results.is_none() {
		issues.extend(issue(
			format!("{path}.maxResults"),
			"Max results is required when library is enabled",
		));
	}

	if let Some(max_results) = value.max_results {
		issues.extend(validate_range(
			max_results as f64,
			&format!("{path}.maxResults"),
			"maxResults",
			&RangeConstraints {
				min: Some(1.0),
				max: Some(100.0),
				integer: true,
			},
		));
	}

	issues
}

// ---------------------------------------------------------------------------
// Chain validation
// ---------------------------------------------------------------------------

fn validate_parallel_sub_step(
	value: &ParallelSubStepDefinition,
	path: &str,
) -> Vec<ValidationIssue> {
	let mut issues = Vec::new();

	let name_issues = validate_non_empty(&value.name, &format!("{path}.name"), "Sub-step name");
	if name_issues.is_empty() && !STEP_NAME_RE.is_match(&value.name) {
		issues.extend(issue(
			format!("{path}.name"),
			"Sub-step name must start with a letter or underscore and contain only word characters or hyphens",
		));
	}
	issues.extend(name_issues);

	issues.extend(validate_non_empty(
		&value.template,
		&format!("{path}.template"),
		"Sub-step template",
	));

	if value.provider.as_ref() == Some(&StepProvider::Mcp) {
		let missing_server = value
			.mcp_server_name
			.as_ref()
			.is_none_or(|s| s.is_empty());
		if missing_server {
			issues.extend(issue(
				format!("{path}.mcpServerName"),
				"MCP sub-step requires \"mcpServerName\" to be set",
			));
		}
		let missing_tool = value.mcp_tool_name.as_ref().is_none_or(|s| s.is_empty());
		if missing_tool {
			issues.extend(issue(
				format!("{path}.mcpToolName"),
				"MCP sub-step requires \"mcpToolName\" to be set",
			));
		}
	}

	issues
}

fn validate_parallel_config(value: &ParallelConfigDefinition, path: &str) -> Vec<ValidationIssue> {
	let mut issues = Vec::new();

	if value.sub_steps.len() < 2 {
		issues.extend(issue(
			format!("{path}.subSteps"),
			"Parallel config must have at least 2 sub-steps",
		));
	}

	for (i, sub) in value.sub_steps.iter().enumerate() {
		issues.extend(validate_parallel_sub_step(sub, &format!("{path}.subSteps[{i}]")));
	}

	// Detect duplicate sub-step names
	if issues.is_empty() {
		let mut seen = HashSet::new();
		for sub in &value.sub_steps {
			if !sub.name.is_empty() && !seen.insert(&sub.name) {
				issues.extend(issue(
					format!("{path}.subSteps"),
					format!("Duplicate sub-step name: \"{}\"", sub.name),
				));
			}
		}
	}

	issues
}

fn validate_chain_step_definition(
	value: &ChainStepDefinition,
	path: &str,
) -> Vec<ValidationIssue> {
	let mut issues = Vec::new();

	// name
	let name_issues = validate_non_empty(&value.name, &format!("{path}.name"), "Chain step name");
	if name_issues.is_empty() && !STEP_NAME_RE.is_match(&value.name) {
		issues.extend(issue(
			format!("{path}.name"),
			"Step name must start with a letter or underscore and contain only word characters or hyphens",
		));
	}
	issues.extend(name_issues);

	// template
	issues.extend(validate_non_empty(
		&value.template,
		&format!("{path}.template"),
		"Chain step template",
	));

	// MCP provider requires mcpServerName and mcpToolName
	if value.provider.as_ref() == Some(&StepProvider::Mcp) {
		let missing_server = value
			.mcp_server_name
			.as_ref()
			.is_none_or(|s| s.is_empty());
		if missing_server {
			issues.extend(issue(
				format!("{path}.mcpServerName"),
				"MCP step requires \"mcpServerName\" to be set",
			));
		}
		let missing_tool = value.mcp_tool_name.as_ref().is_none_or(|s| s.is_empty());
		if missing_tool {
			issues.extend(issue(
				format!("{path}.mcpToolName"),
				"MCP step requires \"mcpToolName\" to be set",
			));
		}
	}

	// Parallel config validation
	if let Some(ref parallel) = value.parallel {
		issues.extend(validate_parallel_config(parallel, &format!("{path}.parallel")));
	}

	issues
}

fn validate_chain_definition(value: &ChainDefinition, path: &str) -> Vec<ValidationIssue> {
	if value.steps.is_empty() {
		return issue(format!("{path}.steps"), "A chain must have at least one step");
	}

	let mut issues = Vec::new();

	for (i, step) in value.steps.iter().enumerate() {
		issues.extend(validate_chain_step_definition(
			step,
			&format!("{path}.steps[{i}]"),
		));
	}

	// Detect duplicate step names
	if issues.is_empty() {
		let mut seen = HashSet::new();
		for step in &value.steps {
			if !step.name.is_empty() && !seen.insert(&step.name) {
				issues.extend(issue(
					format!("{path}.steps"),
					format!("Duplicate step name: \"{}\"", step.name),
				));
			}
		}
	}

	issues
}

// ---------------------------------------------------------------------------
// Top-level validation
// ---------------------------------------------------------------------------

fn validate_app_config(value: &AppConfig) -> Vec<ValidationIssue> {
	let mut issues = Vec::new();

	issues.extend(validate_acp_config(&value.acp, "acp"));
	issues.extend(validate_mcp_config(&value.mcp, "mcp"));

	// Only validate library if explicitly enabled (or has fields set)
	if value.library.enabled
		|| value.library.embedding_agent.is_some()
		|| value.library.similarity_threshold.is_some()
		|| value.library.max_results.is_some()
	{
		issues.extend(validate_library_config(&value.library, "library"));
	}

	for (chain_name, chain_def) in &value.chains {
		issues.extend(validate_chain_definition(
			chain_def,
			&format!("chains.{chain_name}"),
		));
	}

	issues
}

// ---------------------------------------------------------------------------
// Resolution helpers
// ---------------------------------------------------------------------------

fn resolve_acp_server_entry(input: &mut AcpServerEntry) {
	// Apply default timeout if not specified
	if input.timeout_ms.is_none() {
		input.timeout_ms = Some(30_000);
	}
}

fn resolve_chain_step(step: &mut ChainStepDefinition, chain_agent_id: Option<&str>, chain_server_name: Option<&str>) {
	// Inherit chain-level agentId/serverName if not overridden
	if step.agent_id.is_none() {
		step.agent_id = chain_agent_id.map(String::from);
	}
	if step.server_name.is_none() {
		step.server_name = chain_server_name.map(String::from);
	}

	// Resolve parallel sub-steps
	if let Some(ref mut parallel) = step.parallel {
		let step_agent = step.agent_id.clone();
		let step_server = step.server_name.clone();
		for sub in &mut parallel.sub_steps {
			if sub.agent_id.is_none() {
				sub.agent_id = step_agent.clone();
			}
			if sub.server_name.is_none() {
				sub.server_name = step_server.clone();
			}
		}
	}
}

fn resolve_chain_definition(chain: &mut ChainDefinition) {
	let agent_id = chain.agent_id.clone();
	let server_name = chain.server_name.clone();
	for step in &mut chain.steps {
		resolve_chain_step(step, agent_id.as_deref(), server_name.as_deref());
	}
}

// ---------------------------------------------------------------------------
// DefineConfigOptions
// ---------------------------------------------------------------------------

/// Options controlling `define_config` behaviour.
pub struct DefineConfigOptions {
	/// If `true`, validation errors are logged as warnings and defaults are
	/// used for the invalid fields instead of throwing.
	pub lenient: bool,
	/// Optional warning handler called in lenient mode.
	pub on_warn: Option<Box<dyn FnOnce(Vec<ValidationIssue>)>>,
}

// ---------------------------------------------------------------------------
// define_config
// ---------------------------------------------------------------------------

/// Create a validated `AppConfig` from a JSON `Value`.
///
/// Applies sensible defaults for all optional fields and validates the
/// configuration against semantic constraints.
///
/// # Errors
///
/// Returns `SimseError::Config` with `ConfigErrorCode::ValidationFailed`
/// when the input fails validation (unless `lenient` is `true`).
pub fn define_config(
	raw: Value,
	options: Option<DefineConfigOptions>,
) -> Result<AppConfig, SimseError> {
	let lenient = options.as_ref().is_some_and(|o| o.lenient);

	// Deserialize JSON into AppConfig (serde defaults handle missing fields)
	let mut config: AppConfig = serde_json::from_value(raw).map_err(|e| {
		SimseError::config(
			ConfigErrorCode::ValidationFailed,
			format!("Failed to parse config: {e}"),
		)
	})?;

	// Validate
	let issues = validate_app_config(&config);

	if !issues.is_empty() {
		if lenient {
			if let Some(opts) = options {
				if let Some(on_warn) = opts.on_warn {
					on_warn(issues.clone());
				}
			}

			// In lenient mode, reset invalid fields to defaults
			let invalid_paths: HashSet<String> = issues.iter().map(|i| i.path.clone()).collect();

			// Reset library fields
			if invalid_paths.iter().any(|p| p.starts_with("library.")) {
				if invalid_paths.contains("library.similarityThreshold") {
					config.library.similarity_threshold = None;
				}
				if invalid_paths.contains("library.maxResults") {
					config.library.max_results = None;
				}
				if invalid_paths.contains("library.embeddingAgent") {
					config.library.embedding_agent = None;
				}
				if invalid_paths.contains("library.enabled") {
					config.library.enabled = false;
				}
			}

			// Reset MCP server fields
			if invalid_paths.iter().any(|p| p.starts_with("mcp.server.")) {
				if invalid_paths.contains("mcp.server.enabled") {
					config.mcp.server.enabled = false;
				}
				if invalid_paths.contains("mcp.server.name") {
					config.mcp.server.name = None;
				}
				if invalid_paths.contains("mcp.server.version") {
					config.mcp.server.version = None;
				}
			}
		} else {
			let messages: Vec<String> = issues
				.iter()
				.map(|i| format!("{}: {}", i.path, i.message))
				.collect();
			return Err(SimseError::config(
				ConfigErrorCode::ValidationFailed,
				format!("Config validation failed: {}", messages.join("; ")),
			));
		}
	}

	// Guard: even in lenient mode, need at least one server
	if config.acp.servers.is_empty() {
		return Err(SimseError::config(
			ConfigErrorCode::ValidationFailed,
			"ACP servers are required and must be a non-empty array",
		));
	}

	// Apply resolution defaults
	for server in &mut config.acp.servers {
		resolve_acp_server_entry(server);
	}

	// Resolve chain definitions (agent inheritance)
	let chain_keys: Vec<String> = config.chains.keys().cloned().collect();
	for key in chain_keys {
		if let Some(mut chain) = config.chains.remove(&key) {
			resolve_chain_definition(&mut chain);
			config.chains.insert(key, chain);
		}
	}

	Ok(config)
}
