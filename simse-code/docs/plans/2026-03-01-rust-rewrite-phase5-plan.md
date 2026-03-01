# Phase 5: Core Infrastructure Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build the foundational I/O and config layer in `simse-bridge` — JSON utilities, config loading, session persistence, and ACP client — so that higher tiers (library, tools, loop, TUI) can be built on a solid async infrastructure.

**Architecture:** All four modules live in `simse-bridge`. Pure I/O code using `tokio` for async, `serde`/`serde_json` for serialization. Types that need to cross crate boundaries go in `simse-ui-core`. Config + session types shared via re-export.

**Tech Stack:** Rust 1.85+, tokio 1, serde/serde_json, flate2, dirs 6, uuid 1 (v4), chrono 0.4, thiserror 2

---

## Pre-Requisites

The following already exist in `simse-bridge`:
- `protocol.rs` — `JsonRpcRequest`, `JsonRpcResponse`, `JsonRpcError`, `JsonRpcNotification`, `RpcMessage`, `parse_message()`
- `client.rs` — `BridgeError`, `BridgeConfig`, `BridgeProcess`, `spawn_bridge()`, `is_healthy()`, `send_line()`, `read_line()`, `request()`, `request_streaming()`
- `config.rs` — stub (empty doc comment only)
- `storage.rs` — stub (empty doc comment only)

Workspace deps already declared: `serde`, `serde_json`, `tokio`, `thiserror`, `flate2`, `dirs`, `uuid`, `chrono`.

---

### Task 25: JSON I/O Utilities

**Files:**
- Create: `simse-bridge/src/json_io.rs`
- Modify: `simse-bridge/src/lib.rs` (add `pub mod json_io;`)

**Context:** Port of `simse-code/json-io.ts`. Four pure I/O functions: `read_json_file`, `write_json_file`, `append_json_line`, `read_json_lines`. Sync implementations (called from async context via `tokio::task::spawn_blocking` where needed). Silent failures on reads (return `None` / empty `Vec`), propagating errors on writes.

**Step 1: Write the failing tests**

Create `simse-bridge/src/json_io.rs` with tests only:

```rust
//! JSON file I/O utilities.
//!
//! Silent failures on reads (None / empty Vec), propagating errors on writes.
//! Tab-indented pretty printing for JSON files, compact for JSONL.

use serde::{de::DeserializeOwned, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::Path;

/// Read and deserialize a JSON file. Returns `None` if the file is missing,
/// empty, or contains invalid JSON.
pub fn read_json_file<T: DeserializeOwned>(path: &Path) -> Option<T> {
	todo!()
}

/// Write a value as pretty-printed JSON (tab-indented) to a file.
/// Creates parent directories if needed.
pub fn write_json_file<T: Serialize>(path: &Path, data: &T) -> io::Result<()> {
	todo!()
}

/// Append a single JSON object as one line to a JSONL file.
/// Creates parent directories if needed.
pub fn append_json_line<T: Serialize>(path: &Path, data: &T) -> io::Result<()> {
	todo!()
}

/// Read all lines from a JSONL file, deserializing each line.
/// Returns empty Vec if file is missing or unreadable.
/// Individual malformed lines are skipped (not fatal).
pub fn read_json_lines<T: DeserializeOwned>(path: &Path) -> Vec<T> {
	todo!()
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde::{Deserialize, Serialize};
	use std::collections::HashMap;
	use tempfile::TempDir;

	#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
	struct TestData {
		name: String,
		value: i32,
	}

	#[test]
	fn read_json_file_missing_returns_none() {
		let dir = TempDir::new().unwrap();
		let path = dir.path().join("nonexistent.json");
		let result: Option<TestData> = read_json_file(&path);
		assert!(result.is_none());
	}

	#[test]
	fn read_json_file_empty_returns_none() {
		let dir = TempDir::new().unwrap();
		let path = dir.path().join("empty.json");
		fs::write(&path, "").unwrap();
		let result: Option<TestData> = read_json_file(&path);
		assert!(result.is_none());
	}

	#[test]
	fn read_json_file_malformed_returns_none() {
		let dir = TempDir::new().unwrap();
		let path = dir.path().join("bad.json");
		fs::write(&path, "not json {{{").unwrap();
		let result: Option<TestData> = read_json_file(&path);
		assert!(result.is_none());
	}

	#[test]
	fn write_and_read_json_file_roundtrip() {
		let dir = TempDir::new().unwrap();
		let path = dir.path().join("sub/dir/test.json");
		let data = TestData {
			name: "hello".into(),
			value: 42,
		};
		write_json_file(&path, &data).unwrap();
		let loaded: Option<TestData> = read_json_file(&path);
		assert_eq!(loaded, Some(data));
	}

	#[test]
	fn write_json_file_uses_tab_indentation() {
		let dir = TempDir::new().unwrap();
		let path = dir.path().join("tabs.json");
		let mut map = HashMap::new();
		map.insert("key", "value");
		write_json_file(&path, &map).unwrap();
		let raw = fs::read_to_string(&path).unwrap();
		assert!(raw.contains("\t\"key\""), "expected tab indentation, got: {raw}");
	}

	#[test]
	fn write_json_file_creates_parent_dirs() {
		let dir = TempDir::new().unwrap();
		let path = dir.path().join("a/b/c/deep.json");
		write_json_file(&path, &42).unwrap();
		assert!(path.exists());
	}

	#[test]
	fn append_and_read_json_lines_roundtrip() {
		let dir = TempDir::new().unwrap();
		let path = dir.path().join("data.jsonl");
		let entries = vec![
			TestData { name: "a".into(), value: 1 },
			TestData { name: "b".into(), value: 2 },
			TestData { name: "c".into(), value: 3 },
		];
		for entry in &entries {
			append_json_line(&path, entry).unwrap();
		}
		let loaded: Vec<TestData> = read_json_lines(&path);
		assert_eq!(loaded, entries);
	}

	#[test]
	fn read_json_lines_missing_returns_empty() {
		let dir = TempDir::new().unwrap();
		let path = dir.path().join("missing.jsonl");
		let result: Vec<TestData> = read_json_lines(&path);
		assert!(result.is_empty());
	}

	#[test]
	fn read_json_lines_skips_malformed_lines() {
		let dir = TempDir::new().unwrap();
		let path = dir.path().join("mixed.jsonl");
		let content = r#"{"name":"good","value":1}
NOT JSON
{"name":"also good","value":2}
"#;
		fs::write(&path, content).unwrap();
		let loaded: Vec<TestData> = read_json_lines(&path);
		assert_eq!(loaded.len(), 2);
		assert_eq!(loaded[0].name, "good");
		assert_eq!(loaded[1].name, "also good");
	}

	#[test]
	fn read_json_lines_handles_trailing_newlines() {
		let dir = TempDir::new().unwrap();
		let path = dir.path().join("trailing.jsonl");
		fs::write(&path, "{\"name\":\"x\",\"value\":0}\n\n\n").unwrap();
		let loaded: Vec<TestData> = read_json_lines(&path);
		assert_eq!(loaded.len(), 1);
	}

	#[test]
	fn append_json_line_creates_parent_dirs() {
		let dir = TempDir::new().unwrap();
		let path = dir.path().join("deep/nested/data.jsonl");
		append_json_line(&path, &42).unwrap();
		assert!(path.exists());
	}
}
```

**Step 2: Add `tempfile` dev-dependency to `simse-bridge/Cargo.toml`**

Add under `[dev-dependencies]`:
```toml
[dev-dependencies]
tempfile = "3"
```

**Step 3: Register module in lib.rs**

In `simse-bridge/src/lib.rs`, add:
```rust
pub mod json_io;
```

**Step 4: Run tests to verify they fail**

Run: `cargo test -p simse-bridge json_io`
Expected: All 11 tests FAIL with `not yet implemented`

**Step 5: Implement the four functions**

Replace the `todo!()` bodies:

```rust
pub fn read_json_file<T: DeserializeOwned>(path: &Path) -> Option<T> {
	let content = fs::read_to_string(path).ok()?;
	serde_json::from_str(&content).ok()
}

pub fn write_json_file<T: Serialize>(path: &Path, data: &T) -> io::Result<()> {
	if let Some(parent) = path.parent() {
		fs::create_dir_all(parent)?;
	}
	let json = serde_json::to_string_pretty(data)
		.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
	// Convert spaces to tabs (serde_json uses 2-space indent)
	let tabbed = json.lines()
		.map(|line| {
			let stripped = line.trim_start_matches(' ');
			let spaces = line.len() - stripped.len();
			let tabs = spaces / 2;
			format!("{}{}", "\t".repeat(tabs), stripped)
		})
		.collect::<Vec<_>>()
		.join("\n");
	fs::write(path, tabbed.as_bytes())
}

pub fn append_json_line<T: Serialize>(path: &Path, data: &T) -> io::Result<()> {
	if let Some(parent) = path.parent() {
		fs::create_dir_all(parent)?;
	}
	let json = serde_json::to_string(data)
		.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
	let mut file = fs::OpenOptions::new()
		.create(true)
		.append(true)
		.open(path)?;
	writeln!(file, "{json}")
}

pub fn read_json_lines<T: DeserializeOwned>(path: &Path) -> Vec<T> {
	let content = match fs::read_to_string(path) {
		Ok(c) => c,
		Err(_) => return Vec::new(),
	};
	content
		.lines()
		.filter(|line| !line.trim().is_empty())
		.filter_map(|line| serde_json::from_str(line).ok())
		.collect()
}
```

**Step 6: Run tests to verify they pass**

Run: `cargo test -p simse-bridge json_io`
Expected: 11 tests PASS

**Step 7: Run full workspace tests**

Run: `cargo test --workspace`
Expected: All ~150+ tests PASS

**Step 8: Commit**

```bash
git add simse-bridge/src/json_io.rs simse-bridge/src/lib.rs simse-bridge/Cargo.toml
git commit -m "feat(bridge): add JSON I/O utilities (read/write/append/lines)"
```

---

### Task 26: Config File Types and Loading

**Files:**
- Rewrite: `simse-bridge/src/config.rs`
- Modify: `simse-bridge/Cargo.toml` (add `glob` dependency if needed)

**Context:** Port of `simse-code/config.ts`. Loads 8+ JSON config files from global (`~/.simse/`) and workspace (`.simse/`) directories. Parses agent personas from `.md` files with YAML frontmatter. Loads skills from `SKILL.md` files. Merges with precedence: CLI > workspace > global > defaults.

**Step 1: Write the failing tests for config types and loading**

Rewrite `simse-bridge/src/config.rs`:

```rust
//! Configuration file loading.
//!
//! Loads hierarchical config from global (~/.simse/) and workspace (.simse/) directories.
//! Merges with precedence: CLI flags > workspace settings > global config > defaults.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::json_io::read_json_file;

// ── ACP Config ──────────────────────────────────────────

/// A single ACP server definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpServerConfig {
	pub name: String,
	pub command: String,
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub args: Vec<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub cwd: Option<String>,
	#[serde(default, skip_serializing_if = "HashMap::is_empty")]
	pub env: HashMap<String, String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub default_agent: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub timeout_ms: Option<u64>,
}

/// Contents of acp.json.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpFileConfig {
	#[serde(default)]
	pub servers: Vec<AcpServerConfig>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub default_server: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub default_agent: Option<String>,
}

// ── MCP Config ──────────────────────────────────────────

/// A single MCP server definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerConfig {
	pub name: String,
	#[serde(default)]
	pub transport: String,
	pub command: String,
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub args: Vec<String>,
	#[serde(default, skip_serializing_if = "HashMap::is_empty")]
	pub env: HashMap<String, String>,
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub required_env: Vec<String>,
}

/// Contents of mcp.json.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct McpFileConfig {
	#[serde(default)]
	pub servers: Vec<McpServerConfig>,
}

// ── Embed Config ────────────────────────────────────────

/// Contents of embed.json.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbedFileConfig {
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub embedding_model: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub dtype: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub tei_url: Option<String>,
}

// ── Library Config ──────────────────────────────────────

/// Contents of memory.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryFileConfig {
	#[serde(default = "default_true")]
	pub enabled: bool,
	#[serde(default = "default_similarity_threshold")]
	pub similarity_threshold: f64,
	#[serde(default = "default_max_results")]
	pub max_results: usize,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub auto_save: Option<bool>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub duplicate_threshold: Option<f64>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub duplicate_behavior: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub flush_interval_ms: Option<u64>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub compression_level: Option<u32>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub atomic_write: Option<bool>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub auto_summarize_threshold: Option<usize>,
}

fn default_true() -> bool { true }
fn default_similarity_threshold() -> f64 { 0.7 }
fn default_max_results() -> usize { 10 }

impl Default for LibraryFileConfig {
	fn default() -> Self {
		Self {
			enabled: true,
			similarity_threshold: 0.7,
			max_results: 10,
			auto_save: None,
			duplicate_threshold: None,
			duplicate_behavior: None,
			flush_interval_ms: None,
			compression_level: None,
			atomic_write: None,
			auto_summarize_threshold: None,
		}
	}
}

// ── Summarize Config ────────────────────────────────────

/// Contents of summarize.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummarizeFileConfig {
	pub server: String,
	pub command: String,
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub args: Vec<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub agent: Option<String>,
	#[serde(default, skip_serializing_if = "HashMap::is_empty")]
	pub env: HashMap<String, String>,
}

// ── User Config ─────────────────────────────────────────

/// Contents of config.json (global user preferences).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserConfig {
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub default_agent: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub log_level: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub perplexity_api_key: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub github_token: Option<String>,
}

// ── Workspace Settings ──────────────────────────────────

/// Contents of .simse/settings.json.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSettings {
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub default_agent: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub log_level: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub system_prompt: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub default_server: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub conversation_topic: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub chain_topic: Option<String>,
}

// ── Prompt Config ───────────────────────────────────────

/// A single step in a prompt chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptStepConfig {
	pub name: String,
	pub template: String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub system_prompt: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub agent_id: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub server_name: Option<String>,
	#[serde(default, skip_serializing_if = "HashMap::is_empty")]
	pub input_mapping: HashMap<String, String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub store_to_memory: Option<bool>,
	#[serde(default, skip_serializing_if = "HashMap::is_empty")]
	pub memory_metadata: HashMap<String, String>,
}

/// A named prompt chain definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptConfig {
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub description: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub agent_id: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub server_name: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub system_prompt: Option<String>,
	pub steps: Vec<PromptStepConfig>,
}

/// Contents of .simse/prompts.json — named prompt chains.
pub type PromptsFileConfig = HashMap<String, PromptConfig>;

// ── Agent Config ────────────────────────────────────────

/// An agent persona loaded from .simse/agents/*.md.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentConfig {
	pub name: String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub description: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub model: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub server_name: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub agent_id: Option<String>,
	pub system_prompt: String,
}

// ── Skill Config ────────────────────────────────────────

/// A skill loaded from .simse/skills/{name}/SKILL.md.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillConfig {
	pub name: String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub description: Option<String>,
	#[serde(default)]
	pub allowed_tools: Vec<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub argument_hint: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub model: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub server_name: Option<String>,
	pub file_path: PathBuf,
	pub body: String,
}

// ── Skipped Server ──────────────────────────────────────

/// An MCP server that was skipped due to missing required env vars.
#[derive(Debug, Clone)]
pub struct SkippedServer {
	pub name: String,
	pub missing_env: Vec<String>,
}

// ── Loaded Config ───────────────────────────────────────

/// Options for loading config (CLI overrides).
#[derive(Debug, Clone, Default)]
pub struct ConfigOptions {
	pub data_dir: Option<PathBuf>,
	pub work_dir: Option<PathBuf>,
	pub default_agent: Option<String>,
	pub log_level: Option<String>,
	pub server_name: Option<String>,
}

/// The fully resolved configuration.
#[derive(Debug, Clone)]
pub struct LoadedConfig {
	pub data_dir: PathBuf,
	pub work_dir: PathBuf,
	pub acp: AcpFileConfig,
	pub mcp: McpFileConfig,
	pub embed: EmbedFileConfig,
	pub library: LibraryFileConfig,
	pub summarize: Option<SummarizeFileConfig>,
	pub user: UserConfig,
	pub workspace_settings: WorkspaceSettings,
	pub prompts: PromptsFileConfig,
	pub agents: Vec<AgentConfig>,
	pub skills: Vec<SkillConfig>,
	pub workspace_prompt: Option<String>,
	pub skipped_servers: Vec<SkippedServer>,
	pub log_level: String,
	pub default_agent: Option<String>,
	pub default_server: Option<String>,
	pub embedding_model: String,
}

// ── Frontmatter Parsing ─────────────────────────────────

/// Parsed frontmatter result.
#[derive(Debug, Clone)]
pub struct Frontmatter {
	pub meta: HashMap<String, String>,
	pub body: String,
}

/// Parse YAML-like frontmatter from a markdown file.
/// Format: `---\nkey: value\n---\nbody`
pub fn parse_frontmatter(content: &str) -> Frontmatter {
	todo!()
}

// ── Agent + Skill Loading ───────────────────────────────

/// Load agent personas from .simse/agents/*.md files.
pub fn load_agents(agents_dir: &Path) -> Vec<AgentConfig> {
	todo!()
}

/// Load skills from .simse/skills/{name}/SKILL.md files.
pub fn load_skills(skills_dir: &Path) -> Vec<SkillConfig> {
	todo!()
}

// ── Config Loading ──────────────────────────────────────

/// Load and merge all config files into a single LoadedConfig.
pub fn load_config(options: &ConfigOptions) -> LoadedConfig {
	todo!()
}

/// Check which MCP servers have missing required env vars.
fn check_mcp_servers(
	servers: &[McpServerConfig],
) -> (Vec<McpServerConfig>, Vec<SkippedServer>) {
	todo!()
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::fs;
	use tempfile::TempDir;

	// ── Frontmatter ──

	#[test]
	fn parse_frontmatter_with_meta() {
		let content = "---\nname: test-agent\ndescription: A test\n---\nBody text here";
		let fm = parse_frontmatter(content);
		assert_eq!(fm.meta.get("name").unwrap(), "test-agent");
		assert_eq!(fm.meta.get("description").unwrap(), "A test");
		assert_eq!(fm.body, "Body text here");
	}

	#[test]
	fn parse_frontmatter_no_meta() {
		let content = "Just a body with no frontmatter";
		let fm = parse_frontmatter(content);
		assert!(fm.meta.is_empty());
		assert_eq!(fm.body, "Just a body with no frontmatter");
	}

	#[test]
	fn parse_frontmatter_empty_body() {
		let content = "---\nname: empty\n---\n";
		let fm = parse_frontmatter(content);
		assert_eq!(fm.meta.get("name").unwrap(), "empty");
		assert!(fm.body.is_empty());
	}

	#[test]
	fn parse_frontmatter_no_closing_delimiter() {
		let content = "---\nname: broken\nno closing delimiter here";
		let fm = parse_frontmatter(content);
		// Entire content becomes body when no closing ---
		assert!(fm.meta.is_empty());
		assert_eq!(fm.body, content);
	}

	// ── Config Type Serialization ──

	#[test]
	fn acp_file_config_deserializes() {
		let json = r#"{
			"servers": [{
				"name": "test",
				"command": "node",
				"args": ["server.js"]
			}],
			"defaultServer": "test"
		}"#;
		let config: AcpFileConfig = serde_json::from_str(json).unwrap();
		assert_eq!(config.servers.len(), 1);
		assert_eq!(config.servers[0].name, "test");
		assert_eq!(config.default_server, Some("test".into()));
	}

	#[test]
	fn mcp_server_config_with_required_env() {
		let json = r#"{
			"name": "perplexity",
			"transport": "stdio",
			"command": "npx",
			"args": ["-y", "perplexity-mcp"],
			"env": { "PERPLEXITY_API_KEY": "${PERPLEXITY_API_KEY}" },
			"requiredEnv": ["PERPLEXITY_API_KEY"]
		}"#;
		let config: McpServerConfig = serde_json::from_str(json).unwrap();
		assert_eq!(config.required_env, vec!["PERPLEXITY_API_KEY"]);
	}

	#[test]
	fn library_file_config_defaults() {
		let config: LibraryFileConfig = serde_json::from_str("{}").unwrap();
		assert!(config.enabled);
		assert!((config.similarity_threshold - 0.7).abs() < f64::EPSILON);
		assert_eq!(config.max_results, 10);
	}

	#[test]
	fn embed_file_config_optional_fields() {
		let config: EmbedFileConfig = serde_json::from_str("{}").unwrap();
		assert!(config.embedding_model.is_none());
		assert!(config.tei_url.is_none());
	}

	#[test]
	fn workspace_settings_partial() {
		let json = r#"{ "logLevel": "debug" }"#;
		let settings: WorkspaceSettings = serde_json::from_str(json).unwrap();
		assert_eq!(settings.log_level, Some("debug".into()));
		assert!(settings.default_agent.is_none());
	}

	// ── Agent Loading ──

	#[test]
	fn load_agents_from_dir() {
		let dir = TempDir::new().unwrap();
		let agents_dir = dir.path().join("agents");
		fs::create_dir_all(&agents_dir).unwrap();

		let agent_md = "---\nname: coder\ndescription: A coding agent\nmodel: gpt-4\n---\nYou are a coding assistant.";
		fs::write(agents_dir.join("coder.md"), agent_md).unwrap();

		let agents = load_agents(&agents_dir);
		assert_eq!(agents.len(), 1);
		assert_eq!(agents[0].name, "coder");
		assert_eq!(agents[0].description, Some("A coding agent".into()));
		assert_eq!(agents[0].system_prompt, "You are a coding assistant.");
	}

	#[test]
	fn load_agents_fallback_name_from_filename() {
		let dir = TempDir::new().unwrap();
		let agents_dir = dir.path().join("agents");
		fs::create_dir_all(&agents_dir).unwrap();

		// No name in frontmatter — should use filename
		let agent_md = "---\ndescription: No name\n---\nBody here.";
		fs::write(agents_dir.join("my-agent.md"), agent_md).unwrap();

		let agents = load_agents(&agents_dir);
		assert_eq!(agents.len(), 1);
		assert_eq!(agents[0].name, "my-agent");
	}

	#[test]
	fn load_agents_skips_empty_body() {
		let dir = TempDir::new().unwrap();
		let agents_dir = dir.path().join("agents");
		fs::create_dir_all(&agents_dir).unwrap();

		let agent_md = "---\nname: empty\n---\n";
		fs::write(agents_dir.join("empty.md"), agent_md).unwrap();

		let agents = load_agents(&agents_dir);
		assert!(agents.is_empty());
	}

	#[test]
	fn load_agents_missing_dir_returns_empty() {
		let dir = TempDir::new().unwrap();
		let agents = load_agents(&dir.path().join("nonexistent"));
		assert!(agents.is_empty());
	}

	// ── Skill Loading ──

	#[test]
	fn load_skills_from_dir() {
		let dir = TempDir::new().unwrap();
		let skills_dir = dir.path().join("skills");
		let skill_dir = skills_dir.join("my-skill");
		fs::create_dir_all(&skill_dir).unwrap();

		let skill_md = "---\nname: my-skill\ndescription: Does things\nallowed-tools: read,write,bash\nargument-hint: <file>\n---\nSkill instructions here.";
		fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();

		let skills = load_skills(&skills_dir);
		assert_eq!(skills.len(), 1);
		assert_eq!(skills[0].name, "my-skill");
		assert_eq!(skills[0].allowed_tools, vec!["read", "write", "bash"]);
		assert_eq!(skills[0].argument_hint, Some("<file>".into()));
		assert_eq!(skills[0].body, "Skill instructions here.");
	}

	#[test]
	fn load_skills_missing_dir_returns_empty() {
		let dir = TempDir::new().unwrap();
		let skills = load_skills(&dir.path().join("nonexistent"));
		assert!(skills.is_empty());
	}

	// ── MCP Server Filtering ──

	#[test]
	fn check_mcp_servers_filters_missing_env() {
		let servers = vec![
			McpServerConfig {
				name: "available".into(),
				transport: "stdio".into(),
				command: "test".into(),
				args: vec![],
				env: HashMap::new(),
				required_env: vec![],
			},
			McpServerConfig {
				name: "needs-key".into(),
				transport: "stdio".into(),
				command: "test".into(),
				args: HashMap::new().into_iter().collect(),
				env: HashMap::new(),
				required_env: vec!["MISSING_API_KEY".into()],
			},
		];
		let (kept, skipped) = check_mcp_servers(&servers);
		assert_eq!(kept.len(), 1);
		assert_eq!(kept[0].name, "available");
		assert_eq!(skipped.len(), 1);
		assert_eq!(skipped[0].name, "needs-key");
		assert_eq!(skipped[0].missing_env, vec!["MISSING_API_KEY"]);
	}

	// ── Full Config Loading ──

	#[test]
	fn load_config_with_minimal_files() {
		let dir = TempDir::new().unwrap();
		let data_dir = dir.path().join("data");
		fs::create_dir_all(&data_dir).unwrap();

		// Write minimal acp.json
		let acp_json = r#"{"servers":[{"name":"test","command":"echo","args":["hi"]}]}"#;
		fs::write(data_dir.join("acp.json"), acp_json).unwrap();

		let config = load_config(&ConfigOptions {
			data_dir: Some(data_dir),
			work_dir: Some(dir.path().to_path_buf()),
			..Default::default()
		});

		assert_eq!(config.acp.servers.len(), 1);
		assert_eq!(config.acp.servers[0].name, "test");
		assert_eq!(config.log_level, "warn"); // default
		assert_eq!(config.embedding_model, "nomic-ai/nomic-embed-text-v1.5");
	}

	#[test]
	fn load_config_precedence_cli_over_workspace_over_global() {
		let dir = TempDir::new().unwrap();
		let data_dir = dir.path().join("data");
		let work_dir = dir.path().join("work");
		let simse_dir = work_dir.join(".simse");
		fs::create_dir_all(&data_dir).unwrap();
		fs::create_dir_all(&simse_dir).unwrap();

		// Global config says log_level = "info"
		fs::write(data_dir.join("config.json"), r#"{"logLevel":"info"}"#).unwrap();
		// Workspace settings say log_level = "debug"
		fs::write(simse_dir.join("settings.json"), r#"{"logLevel":"debug"}"#).unwrap();

		// CLI flag says "trace" — should win
		let config = load_config(&ConfigOptions {
			data_dir: Some(data_dir),
			work_dir: Some(work_dir),
			log_level: Some("trace".into()),
			..Default::default()
		});
		assert_eq!(config.log_level, "trace");
	}

	#[test]
	fn load_config_reads_workspace_prompt() {
		let dir = TempDir::new().unwrap();
		let data_dir = dir.path().join("data");
		let work_dir = dir.path().join("work");
		fs::create_dir_all(&data_dir).unwrap();
		fs::create_dir_all(&work_dir).unwrap();
		fs::write(work_dir.join("SIMSE.md"), "  Project instructions  ").unwrap();

		let config = load_config(&ConfigOptions {
			data_dir: Some(data_dir),
			work_dir: Some(work_dir),
			..Default::default()
		});
		assert_eq!(config.workspace_prompt, Some("Project instructions".into()));
	}

	#[test]
	fn load_config_empty_workspace_prompt_is_none() {
		let dir = TempDir::new().unwrap();
		let data_dir = dir.path().join("data");
		let work_dir = dir.path().join("work");
		fs::create_dir_all(&data_dir).unwrap();
		fs::create_dir_all(&work_dir).unwrap();
		fs::write(work_dir.join("SIMSE.md"), "   \n\t  ").unwrap();

		let config = load_config(&ConfigOptions {
			data_dir: Some(data_dir),
			work_dir: Some(work_dir),
			..Default::default()
		});
		assert!(config.workspace_prompt.is_none());
	}

	#[test]
	fn load_config_loads_prompts() {
		let dir = TempDir::new().unwrap();
		let data_dir = dir.path().join("data");
		let work_dir = dir.path().join("work");
		let simse_dir = work_dir.join(".simse");
		fs::create_dir_all(&data_dir).unwrap();
		fs::create_dir_all(&simse_dir).unwrap();

		let prompts_json = r#"{
			"summarize": {
				"description": "Summarize text",
				"steps": [{ "name": "step1", "template": "Summarize: {{input}}" }]
			}
		}"#;
		fs::write(simse_dir.join("prompts.json"), prompts_json).unwrap();

		let config = load_config(&ConfigOptions {
			data_dir: Some(data_dir),
			work_dir: Some(work_dir),
			..Default::default()
		});
		assert!(config.prompts.contains_key("summarize"));
		assert_eq!(config.prompts["summarize"].steps.len(), 1);
	}
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p simse-bridge config`
Expected: All tests FAIL with `not yet implemented`

**Step 3: Implement `parse_frontmatter`**

```rust
pub fn parse_frontmatter(content: &str) -> Frontmatter {
	if !content.starts_with("---\n") && !content.starts_with("---\r\n") {
		return Frontmatter {
			meta: HashMap::new(),
			body: content.to_string(),
		};
	}

	// Find closing --- (after the opening one at position 0)
	let after_open = if content.starts_with("---\r\n") { 5 } else { 4 };
	let rest = &content[after_open..];

	let close_pos = rest.find("\n---\n")
		.or_else(|| rest.find("\n---\r\n"))
		.or_else(|| {
			// Handle case where --- is at the very end
			if rest.ends_with("\n---") {
				Some(rest.len() - 3)
			} else {
				None
			}
		});

	let Some(close_pos) = close_pos else {
		return Frontmatter {
			meta: HashMap::new(),
			body: content.to_string(),
		};
	};

	let yaml_block = &rest[..close_pos];
	let body_start = close_pos + 4; // skip \n---
	let body = if body_start < rest.len() {
		rest[body_start..].trim_start_matches('\n').trim_start_matches('\r')
	} else {
		""
	};

	let mut meta = HashMap::new();
	for line in yaml_block.lines() {
		let line = line.trim();
		if line.is_empty() {
			continue;
		}
		if let Some((key, value)) = line.split_once(':') {
			meta.insert(key.trim().to_string(), value.trim().to_string());
		}
	}

	Frontmatter {
		meta,
		body: body.trim_end().to_string(),
	}
}
```

**Step 4: Implement `load_agents`**

```rust
pub fn load_agents(agents_dir: &Path) -> Vec<AgentConfig> {
	let entries = match std::fs::read_dir(agents_dir) {
		Ok(e) => e,
		Err(_) => return Vec::new(),
	};

	let mut agents = Vec::new();
	for entry in entries.flatten() {
		let path = entry.path();
		if path.extension().and_then(|e| e.to_str()) != Some("md") {
			continue;
		}
		let content = match std::fs::read_to_string(&path) {
			Ok(c) => c,
			Err(_) => continue,
		};
		let fm = parse_frontmatter(&content);
		if fm.body.trim().is_empty() {
			continue;
		}
		let filename_name = path.file_stem()
			.and_then(|s| s.to_str())
			.unwrap_or("unknown")
			.to_string();
		agents.push(AgentConfig {
			name: fm.meta.get("name").cloned().unwrap_or(filename_name),
			description: fm.meta.get("description").cloned(),
			model: fm.meta.get("model").cloned(),
			server_name: fm.meta.get("serverName").or_else(|| fm.meta.get("server-name")).cloned(),
			agent_id: fm.meta.get("agentId").or_else(|| fm.meta.get("agent-id")).cloned(),
			system_prompt: fm.body,
		});
	}
	agents
}
```

**Step 5: Implement `load_skills`**

```rust
pub fn load_skills(skills_dir: &Path) -> Vec<SkillConfig> {
	let entries = match std::fs::read_dir(skills_dir) {
		Ok(e) => e,
		Err(_) => return Vec::new(),
	};

	let mut skills = Vec::new();
	for entry in entries.flatten() {
		let skill_dir = entry.path();
		if !skill_dir.is_dir() {
			continue;
		}
		let skill_path = skill_dir.join("SKILL.md");
		let content = match std::fs::read_to_string(&skill_path) {
			Ok(c) => c,
			Err(_) => continue,
		};
		let fm = parse_frontmatter(&content);
		let dir_name = skill_dir.file_name()
			.and_then(|s| s.to_str())
			.unwrap_or("unknown")
			.to_string();

		let allowed_tools = fm.meta.get("allowed-tools")
			.map(|s| s.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect())
			.unwrap_or_default();

		skills.push(SkillConfig {
			name: fm.meta.get("name").cloned().unwrap_or(dir_name),
			description: fm.meta.get("description").cloned(),
			allowed_tools,
			argument_hint: fm.meta.get("argument-hint").cloned(),
			model: fm.meta.get("model").cloned(),
			server_name: fm.meta.get("server-name").cloned(),
			file_path: skill_path.clone(),
			body: fm.body,
		});
	}
	skills
}
```

**Step 6: Implement `check_mcp_servers`**

```rust
fn check_mcp_servers(
	servers: &[McpServerConfig],
) -> (Vec<McpServerConfig>, Vec<SkippedServer>) {
	let mut kept = Vec::new();
	let mut skipped = Vec::new();

	for server in servers {
		let missing: Vec<String> = server.required_env.iter()
			.filter(|key| {
				// Check server.env first, then process env
				let in_server_env = server.env.get(key.as_str())
					.map(|v| !v.is_empty() && !v.starts_with("${"))
					.unwrap_or(false);
				let in_process_env = std::env::var(key).is_ok();
				!in_server_env && !in_process_env
			})
			.cloned()
			.collect();

		if missing.is_empty() {
			kept.push(server.clone());
		} else {
			skipped.push(SkippedServer {
				name: server.name.clone(),
				missing_env,
			});
		}
	}

	(kept, skipped)
}
```

**Step 7: Implement `load_config`**

```rust
pub fn load_config(options: &ConfigOptions) -> LoadedConfig {
	let work_dir = options.work_dir.clone()
		.unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
	let data_dir = options.data_dir.clone()
		.unwrap_or_else(|| {
			dirs::config_dir()
				.unwrap_or_else(|| PathBuf::from("."))
				.join("simse")
		});
	let simse_dir = work_dir.join(".simse");

	// Load global config files
	let user: UserConfig = read_json_file(&data_dir.join("config.json")).unwrap_or_default();
	let acp: AcpFileConfig = read_json_file(&data_dir.join("acp.json")).unwrap_or_default();
	let mcp_raw: McpFileConfig = read_json_file(&data_dir.join("mcp.json")).unwrap_or_default();
	let embed: EmbedFileConfig = read_json_file(&data_dir.join("embed.json")).unwrap_or_default();
	let library: LibraryFileConfig = read_json_file(&data_dir.join("memory.json")).unwrap_or_default();
	let summarize: Option<SummarizeFileConfig> = read_json_file(&data_dir.join("summarize.json"));

	// Load workspace config files
	let workspace_settings: WorkspaceSettings =
		read_json_file(&simse_dir.join("settings.json")).unwrap_or_default();
	let prompts: PromptsFileConfig =
		read_json_file(&simse_dir.join("prompts.json")).unwrap_or_default();

	// Load agents and skills
	let agents = load_agents(&simse_dir.join("agents"));
	let skills = load_skills(&simse_dir.join("skills"));

	// Load SIMSE.md workspace prompt
	let workspace_prompt = std::fs::read_to_string(work_dir.join("SIMSE.md"))
		.ok()
		.map(|s| s.trim().to_string())
		.filter(|s| !s.is_empty());

	// Filter MCP servers with missing env
	let (mcp_servers, skipped_servers) = check_mcp_servers(&mcp_raw.servers);
	let mcp = McpFileConfig { servers: mcp_servers };

	// Resolve precedence: CLI > workspace > global > defaults
	let log_level = options.log_level.clone()
		.or(workspace_settings.log_level.clone())
		.or(user.log_level.clone())
		.unwrap_or_else(|| "warn".into());

	let default_agent = options.default_agent.clone()
		.or(workspace_settings.default_agent.clone())
		.or(user.default_agent.clone());

	let default_server = options.server_name.clone()
		.or(workspace_settings.default_server.clone())
		.or(acp.default_server.clone());

	let embedding_model = embed.embedding_model.clone()
		.unwrap_or_else(|| "nomic-ai/nomic-embed-text-v1.5".into());

	LoadedConfig {
		data_dir,
		work_dir,
		acp,
		mcp,
		embed,
		library,
		summarize,
		user,
		workspace_settings,
		prompts,
		agents,
		skills,
		workspace_prompt,
		skipped_servers,
		log_level,
		default_agent,
		default_server,
		embedding_model,
	}
}
```

**Step 8: Run tests to verify they pass**

Run: `cargo test -p simse-bridge config`
Expected: All ~17 tests PASS

**Step 9: Run full workspace tests**

Run: `cargo test --workspace`
Expected: All tests PASS

**Step 10: Commit**

```bash
git add simse-bridge/src/config.rs
git commit -m "feat(bridge): add config loading with frontmatter parsing and MCP env filtering"
```

---

### Task 27: Session Store

**Files:**
- Rewrite: `simse-bridge/src/storage.rs` → rename to `simse-bridge/src/session_store.rs`
- Modify: `simse-bridge/src/lib.rs` (rename module)
- May also create: `simse-ui-core/src/state/session.rs` for shared types

**Context:** Port of `simse-code/session-store.ts`. JSONL-based crash-safe session persistence. Uses `json_io` functions. Session metadata stored in `sessions/index.json`, individual session messages in `sessions/{id}.jsonl`. Append-only writes for crash safety.

**Step 1: Add session types to simse-ui-core**

Create `simse-ui-core/src/state/session.rs`:

```rust
//! Session metadata types shared between crates.

use serde::{Deserialize, Serialize};

/// Metadata about a saved session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMeta {
	pub id: String,
	pub title: String,
	pub created_at: String,
	pub updated_at: String,
	pub message_count: usize,
	pub work_dir: String,
}
```

Register in `simse-ui-core/src/state/mod.rs`:
```rust
pub mod session;
```

**Step 2: Write the failing tests for session store**

Rename `simse-bridge/src/storage.rs` to `simse-bridge/src/session_store.rs` and update `lib.rs`. Write full implementation file with tests:

```rust
//! Crash-safe JSONL session persistence.
//!
//! Two stores on disk:
//! - `sessions/index.json` — array of SessionMeta (newest first)
//! - `sessions/{id}.jsonl` — append-only message log

use serde::{Deserialize, Serialize};
use simse_ui_core::state::session::SessionMeta;
use std::path::{Path, PathBuf};

use crate::json_io::{append_json_line, read_json_file, read_json_lines, write_json_file};

/// A single entry in a session's JSONL file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionEntry {
	ts: String,
	role: String,
	content: String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	tool_call_id: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	tool_name: Option<String>,
}

/// A conversation message (simplified for session store).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMessage {
	pub role: String,
	pub content: String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub tool_call_id: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub tool_name: Option<String>,
}

/// Session store for JSONL-based session persistence.
pub struct SessionStore {
	sessions_dir: PathBuf,
	index_path: PathBuf,
}

impl SessionStore {
	/// Create a new session store rooted at `data_dir/sessions/`.
	pub fn new(data_dir: &Path) -> Self {
		let sessions_dir = data_dir.join("sessions");
		let index_path = sessions_dir.join("index.json");
		Self { sessions_dir, index_path }
	}

	/// Create a new session. Returns the session ID.
	pub fn create(&self, work_dir: &str) -> std::io::Result<String> {
		todo!()
	}

	/// Append a message to a session.
	pub fn append(&self, session_id: &str, message: &SessionMessage) -> std::io::Result<()> {
		todo!()
	}

	/// Load all messages from a session. Per-line error isolation.
	pub fn load(&self, session_id: &str) -> Vec<SessionMessage> {
		todo!()
	}

	/// List all sessions (newest first).
	pub fn list(&self) -> Vec<SessionMeta> {
		todo!()
	}

	/// Get metadata for a specific session.
	pub fn get(&self, session_id: &str) -> Option<SessionMeta> {
		todo!()
	}

	/// Rename a session.
	pub fn rename(&self, session_id: &str, title: &str) -> std::io::Result<()> {
		todo!()
	}

	/// Delete a session (removes from index and deletes JSONL file).
	pub fn remove(&self, session_id: &str) -> std::io::Result<()> {
		todo!()
	}

	/// Get the latest session ID for a given work directory.
	pub fn latest(&self, work_dir: &str) -> Option<String> {
		todo!()
	}

	// ── Internal helpers ──

	fn load_index(&self) -> Vec<SessionMeta> {
		read_json_file(&self.index_path).unwrap_or_default()
	}

	fn save_index(&self, index: &[SessionMeta]) -> std::io::Result<()> {
		write_json_file(&self.index_path, &index.to_vec())
	}

	fn session_file_path(&self, id: &str) -> PathBuf {
		self.sessions_dir.join(format!("{id}.jsonl"))
	}

	fn generate_id() -> String {
		let random_hex: String = uuid::Uuid::new_v4().to_string()[..8].to_string();
		let timestamp = chrono::Utc::now().timestamp().to_string();
		// Base36-encode the timestamp for compactness
		let ts_base36 = radix_fmt(timestamp.parse::<u64>().unwrap_or(0));
		format!("{random_hex}-{ts_base36}")
	}
}

/// Simple base-36 encoding for a u64.
fn radix_fmt(mut n: u64) -> String {
	if n == 0 {
		return "0".to_string();
	}
	const CHARS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
	let mut result = Vec::new();
	while n > 0 {
		result.push(CHARS[(n % 36) as usize] as char);
		n /= 36;
	}
	result.into_iter().rev().collect()
}

#[cfg(test)]
mod tests {
	use super::*;
	use tempfile::TempDir;

	fn make_store() -> (TempDir, SessionStore) {
		let dir = TempDir::new().unwrap();
		let store = SessionStore::new(dir.path());
		(dir, store)
	}

	fn sample_message(role: &str, content: &str) -> SessionMessage {
		SessionMessage {
			role: role.into(),
			content: content.into(),
			tool_call_id: None,
			tool_name: None,
		}
	}

	#[test]
	fn create_returns_id_and_appears_in_list() {
		let (_dir, store) = make_store();
		let id = store.create("/tmp/project").unwrap();
		assert!(!id.is_empty());
		let sessions = store.list();
		assert_eq!(sessions.len(), 1);
		assert_eq!(sessions[0].id, id);
		assert_eq!(sessions[0].work_dir, "/tmp/project");
		assert_eq!(sessions[0].message_count, 0);
	}

	#[test]
	fn create_newest_first() {
		let (_dir, store) = make_store();
		let id1 = store.create("/a").unwrap();
		let id2 = store.create("/b").unwrap();
		let sessions = store.list();
		assert_eq!(sessions.len(), 2);
		assert_eq!(sessions[0].id, id2); // newest first
		assert_eq!(sessions[1].id, id1);
	}

	#[test]
	fn append_and_load_messages() {
		let (_dir, store) = make_store();
		let id = store.create("/work").unwrap();
		store.append(&id, &sample_message("user", "hello")).unwrap();
		store.append(&id, &sample_message("assistant", "hi back")).unwrap();

		let messages = store.load(&id);
		assert_eq!(messages.len(), 2);
		assert_eq!(messages[0].role, "user");
		assert_eq!(messages[0].content, "hello");
		assert_eq!(messages[1].role, "assistant");
		assert_eq!(messages[1].content, "hi back");
	}

	#[test]
	fn append_updates_message_count() {
		let (_dir, store) = make_store();
		let id = store.create("/work").unwrap();
		store.append(&id, &sample_message("user", "hello")).unwrap();
		store.append(&id, &sample_message("assistant", "hi")).unwrap();

		let meta = store.get(&id).unwrap();
		assert_eq!(meta.message_count, 2);
	}

	#[test]
	fn load_nonexistent_returns_empty() {
		let (_dir, store) = make_store();
		let messages = store.load("nonexistent-id");
		assert!(messages.is_empty());
	}

	#[test]
	fn get_returns_none_for_missing() {
		let (_dir, store) = make_store();
		assert!(store.get("not-here").is_none());
	}

	#[test]
	fn rename_updates_title() {
		let (_dir, store) = make_store();
		let id = store.create("/work").unwrap();
		store.rename(&id, "My Session").unwrap();
		let meta = store.get(&id).unwrap();
		assert_eq!(meta.title, "My Session");
	}

	#[test]
	fn remove_deletes_from_index_and_file() {
		let (_dir, store) = make_store();
		let id = store.create("/work").unwrap();
		store.append(&id, &sample_message("user", "test")).unwrap();
		store.remove(&id).unwrap();

		assert!(store.get(&id).is_none());
		assert!(!store.session_file_path(&id).exists());
	}

	#[test]
	fn latest_returns_most_recent_for_workdir() {
		let (_dir, store) = make_store();
		let _id1 = store.create("/project-a").unwrap();
		let id2 = store.create("/project-b").unwrap();
		let id3 = store.create("/project-a").unwrap();

		assert_eq!(store.latest("/project-a"), Some(id3));
		assert_eq!(store.latest("/project-b"), Some(id2));
		assert_eq!(store.latest("/nonexistent"), None);
	}

	#[test]
	fn load_tolerates_corrupt_lines() {
		let (_dir, store) = make_store();
		let id = store.create("/work").unwrap();
		let path = store.session_file_path(&id);

		// Write some valid and some corrupt lines
		let content = format!(
			"{}\nNOT VALID JSON\n{}\n",
			serde_json::to_string(&SessionEntry {
				ts: "2026-01-01T00:00:00Z".into(),
				role: "user".into(),
				content: "good line".into(),
				tool_call_id: None,
				tool_name: None,
			}).unwrap(),
			serde_json::to_string(&SessionEntry {
				ts: "2026-01-01T00:01:00Z".into(),
				role: "assistant".into(),
				content: "also good".into(),
				tool_call_id: None,
				tool_name: None,
			}).unwrap(),
		);
		std::fs::write(&path, content).unwrap();

		let messages = store.load(&id);
		assert_eq!(messages.len(), 2);
		assert_eq!(messages[0].content, "good line");
		assert_eq!(messages[1].content, "also good");
	}

	#[test]
	fn append_with_tool_fields() {
		let (_dir, store) = make_store();
		let id = store.create("/work").unwrap();
		let msg = SessionMessage {
			role: "tool_result".into(),
			content: "42".into(),
			tool_call_id: Some("call_1".into()),
			tool_name: Some("calculator".into()),
		};
		store.append(&id, &msg).unwrap();
		let loaded = store.load(&id);
		assert_eq!(loaded.len(), 1);
		assert_eq!(loaded[0].tool_call_id, Some("call_1".into()));
		assert_eq!(loaded[0].tool_name, Some("calculator".into()));
	}

	#[test]
	fn radix_fmt_base36() {
		assert_eq!(radix_fmt(0), "0");
		assert_eq!(radix_fmt(35), "z");
		assert_eq!(radix_fmt(36), "10");
		assert_eq!(radix_fmt(1709337600), "qf6oao"); // a realistic timestamp
	}
}
```

**Step 3: Run tests to verify they fail**

Run: `cargo test -p simse-bridge session_store`
Expected: All tests FAIL with `not yet implemented`

**Step 4: Implement the session store methods**

Fill in the `todo!()` bodies:

```rust
pub fn create(&self, work_dir: &str) -> std::io::Result<String> {
	let id = Self::generate_id();
	let now = chrono::Utc::now().to_rfc3339();
	let meta = SessionMeta {
		id: id.clone(),
		title: format!("Session {}", chrono::Local::now().format("%Y-%m-%d %H:%M")),
		created_at: now.clone(),
		updated_at: now,
		message_count: 0,
		work_dir: work_dir.to_string(),
	};
	let mut index = self.load_index();
	index.insert(0, meta); // newest first
	self.save_index(&index)?;
	Ok(id)
}

pub fn append(&self, session_id: &str, message: &SessionMessage) -> std::io::Result<()> {
	let entry = SessionEntry {
		ts: chrono::Utc::now().to_rfc3339(),
		role: message.role.clone(),
		content: message.content.clone(),
		tool_call_id: message.tool_call_id.clone(),
		tool_name: message.tool_name.clone(),
	};
	append_json_line(&self.session_file_path(session_id), &entry)?;

	// Update metadata
	let mut index = self.load_index();
	if let Some(pos) = index.iter().position(|m| m.id == session_id) {
		let old = &index[pos];
		index[pos] = SessionMeta {
			id: old.id.clone(),
			title: old.title.clone(),
			created_at: old.created_at.clone(),
			updated_at: chrono::Utc::now().to_rfc3339(),
			message_count: old.message_count + 1,
			work_dir: old.work_dir.clone(),
		};
		self.save_index(&index)?;
	}
	Ok(())
}

pub fn load(&self, session_id: &str) -> Vec<SessionMessage> {
	let path = self.session_file_path(session_id);
	let content = match std::fs::read_to_string(&path) {
		Ok(c) => c,
		Err(_) => return Vec::new(),
	};
	content
		.lines()
		.filter(|line| !line.trim().is_empty())
		.filter_map(|line| {
			let entry: SessionEntry = serde_json::from_str(line).ok()?;
			Some(SessionMessage {
				role: entry.role,
				content: entry.content,
				tool_call_id: entry.tool_call_id,
				tool_name: entry.tool_name,
			})
		})
		.collect()
}

pub fn list(&self) -> Vec<SessionMeta> {
	self.load_index()
}

pub fn get(&self, session_id: &str) -> Option<SessionMeta> {
	self.load_index().into_iter().find(|m| m.id == session_id)
}

pub fn rename(&self, session_id: &str, title: &str) -> std::io::Result<()> {
	let mut index = self.load_index();
	if let Some(pos) = index.iter().position(|m| m.id == session_id) {
		let old = &index[pos];
		index[pos] = SessionMeta {
			id: old.id.clone(),
			title: title.to_string(),
			created_at: old.created_at.clone(),
			updated_at: old.updated_at.clone(),
			message_count: old.message_count,
			work_dir: old.work_dir.clone(),
		};
		self.save_index(&index)?;
	}
	Ok(())
}

pub fn remove(&self, session_id: &str) -> std::io::Result<()> {
	let mut index = self.load_index();
	index.retain(|m| m.id != session_id);
	self.save_index(&index)?;
	let path = self.session_file_path(session_id);
	if path.exists() {
		std::fs::remove_file(&path)?;
	}
	Ok(())
}

pub fn latest(&self, work_dir: &str) -> Option<String> {
	self.load_index()
		.into_iter()
		.find(|m| m.work_dir == work_dir)
		.map(|m| m.id)
}
```

**Step 5: Run tests to verify they pass**

Run: `cargo test -p simse-bridge session_store`
Expected: All 13 tests PASS

**Step 6: Run full workspace tests**

Run: `cargo test --workspace`
Expected: All tests PASS

**Step 7: Commit**

```bash
git add simse-bridge/src/session_store.rs simse-bridge/src/lib.rs simse-ui-core/src/state/session.rs simse-ui-core/src/state/mod.rs
git commit -m "feat(bridge): add crash-safe JSONL session store"
```

---

### Task 28: ACP Client

**Files:**
- Create: `simse-bridge/src/acp_client.rs`
- Create: `simse-bridge/src/acp_types.rs`
- Modify: `simse-bridge/src/lib.rs` (add modules)

**Context:** Full ACP client implementation — the real bridge to AI backends. Builds on existing `client.rs` primitives (spawn, send, read, request). Adds: initialize handshake, session management, generate/stream/embed methods, permission handling. This replaces the stub bridge-server approach with direct ACP protocol.

**TS reference:** `acp-client.ts`, `acp-connection.ts`, `acp-results.ts`, `acp-ollama-bridge.ts` (for protocol understanding).

**Step 1: Create ACP types**

Create `simse-bridge/src/acp_types.rs`:

```rust
//! ACP protocol types.
//!
//! Agent Client Protocol v1 types for session management,
//! content blocks, streaming, permissions, and tool calls.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// ACP content block — the fundamental unit of ACP messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
	Text { text: String },
	Data { data: serde_json::Value },
}

/// Parameters for session/prompt requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPromptParams {
	pub session_id: String,
	pub content: Vec<ContentBlock>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub metadata: Option<PromptMetadata>,
}

/// Metadata attached to a prompt.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptMetadata {
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub agent_id: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub system_prompt: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub temperature: Option<f64>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub max_tokens: Option<u32>,
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub images: Vec<String>,
}

/// Result from session/prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPromptResult {
	pub content: Vec<ContentBlock>,
	pub stop_reason: String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub metadata: Option<ResultMetadata>,
}

/// Metadata in a prompt result.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultMetadata {
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub usage: Option<TokenUsage>,
}

/// Token usage stats.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
	#[serde(default)]
	pub prompt_tokens: u64,
	#[serde(default)]
	pub completion_tokens: u64,
}

/// Agent info returned by initialize.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentInfo {
	pub name: String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub version: Option<String>,
}

/// Initialize response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
	pub protocol_version: u32,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub agent_info: Option<AgentInfo>,
}

/// Session/new response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionNewResult {
	pub session_id: String,
}

/// A session update notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionUpdate {
	pub session_id: String,
	pub update: UpdatePayload,
}

/// Payload within a session update.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePayload {
	pub session_update: String,
	#[serde(default)]
	pub content: Vec<ContentBlock>,
}

/// A permission request from the agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRequestParams {
	pub session_id: String,
	pub tool_name: String,
	pub args: serde_json::Value,
	pub options: Vec<PermissionOption>,
}

/// An option in a permission request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionOption {
	pub id: String,
	pub label: String,
}

/// Permission response outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionOutcome {
	pub outcome: OutcomeSelection,
}

/// The selection within a permission outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutcomeSelection {
	pub outcome: String, // "selected"
	pub option_id: String,
}

/// Options for generate requests.
#[derive(Debug, Clone, Default)]
pub struct GenerateOptions {
	pub agent_id: Option<String>,
	pub server_name: Option<String>,
	pub system_prompt: Option<String>,
	pub temperature: Option<f64>,
	pub max_tokens: Option<u32>,
	pub images: Vec<String>,
}

/// A streaming delta from generateStream.
#[derive(Debug, Clone)]
pub enum StreamEvent {
	/// Text delta from the assistant.
	Delta(String),
	/// Tool call started or updated.
	ToolCall {
		id: String,
		name: String,
		args: String,
	},
	/// Tool call update (progress).
	ToolCallUpdate {
		id: String,
		status: String,
		summary: Option<String>,
	},
	/// Stream complete with final result.
	Complete(SessionPromptResult),
	/// Token usage update.
	Usage(TokenUsage),
}

/// Result from a generate (non-streaming) call.
#[derive(Debug, Clone)]
pub struct GenerateResult {
	pub content: String,
	pub stop_reason: String,
	pub usage: Option<TokenUsage>,
}

/// Result from an embed call.
#[derive(Debug, Clone)]
pub struct EmbedResult {
	pub embeddings: Vec<Vec<f32>>,
	pub prompt_tokens: u64,
}

/// ACP server config needed by the client.
#[derive(Debug, Clone)]
pub struct AcpServerInfo {
	pub command: String,
	pub args: Vec<String>,
	pub cwd: Option<String>,
	pub env: HashMap<String, String>,
	pub timeout_ms: u64,
	pub init_timeout_ms: u64,
}

impl Default for AcpServerInfo {
	fn default() -> Self {
		Self {
			command: String::new(),
			args: Vec::new(),
			cwd: None,
			env: HashMap::new(),
			timeout_ms: 60_000,
			init_timeout_ms: 30_000,
		}
	}
}
```

**Step 2: Create ACP client with tests**

Create `simse-bridge/src/acp_client.rs`:

```rust
//! ACP client — full Agent Client Protocol implementation.
//!
//! Wraps the low-level bridge process with high-level methods:
//! initialize, new_session, generate, generate_stream, embed.

use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

use crate::client::{
	spawn_bridge, is_healthy, request, request_streaming, send_line,
	BridgeConfig, BridgeError, BridgeProcess,
};
use crate::protocol::JsonRpcNotification;
use crate::acp_types::*;

/// The ACP client manages a connection to an ACP server process.
pub struct AcpClient {
	bridge: Arc<Mutex<BridgeProcess>>,
	server_info: AcpServerInfo,
	agent_info: Option<AgentInfo>,
}

/// Errors specific to ACP operations.
#[derive(Debug, thiserror::Error)]
pub enum AcpError {
	#[error("Bridge error: {0}")]
	Bridge(#[from] BridgeError),
	#[error("Protocol error: {0}")]
	Protocol(String),
	#[error("Not initialized")]
	NotInitialized,
	#[error("Session not found: {0}")]
	SessionNotFound(String),
	#[error("Serialization error: {0}")]
	Serialization(#[from] serde_json::Error),
}

impl AcpClient {
	/// Spawn and initialize an ACP server.
	pub async fn connect(server: AcpServerInfo) -> Result<Self, AcpError> {
		let config = BridgeConfig {
			command: server.command.clone(),
			args: server.args.clone(),
			timeout_ms: server.timeout_ms,
			..Default::default()
		};
		let bridge = spawn_bridge(&config).await?;
		let mut client = Self {
			bridge: Arc::new(Mutex::new(bridge)),
			server_info: server,
			agent_info: None,
		};
		client.initialize().await?;
		Ok(client)
	}

	/// Check if the underlying process is still alive.
	pub async fn is_healthy(&self) -> bool {
		let bridge = self.bridge.lock().await;
		is_healthy(&bridge)
	}

	/// Get the agent info from the initialize handshake.
	pub fn agent_info(&self) -> Option<&AgentInfo> {
		self.agent_info.as_ref()
	}

	/// Perform the initialize handshake.
	async fn initialize(&mut self) -> Result<InitializeResult, AcpError> {
		let params = serde_json::json!({
			"protocolVersion": 1,
			"clientInfo": {
				"name": "simse-tui",
				"version": env!("CARGO_PKG_VERSION"),
			}
		});
		let mut bridge = self.bridge.lock().await;
		let resp = request(&mut bridge, "initialize", Some(params)).await?;

		if let Some(error) = resp.error {
			return Err(AcpError::Protocol(error.message));
		}
		let result: InitializeResult = serde_json::from_value(
			resp.result.ok_or_else(|| AcpError::Protocol("No result in initialize response".into()))?
		)?;
		self.agent_info = result.agent_info.clone();
		Ok(result)
	}

	/// Create a new ACP session.
	pub async fn new_session(&self) -> Result<String, AcpError> {
		let mut bridge = self.bridge.lock().await;
		let resp = request(&mut bridge, "session/new", None).await?;
		if let Some(error) = resp.error {
			return Err(AcpError::Protocol(error.message));
		}
		let result: SessionNewResult = serde_json::from_value(
			resp.result.ok_or_else(|| AcpError::Protocol("No result in session/new response".into()))?
		)?;
		Ok(result.session_id)
	}

	/// Generate (non-streaming).
	pub async fn generate(
		&self,
		session_id: &str,
		prompt: &str,
		options: &GenerateOptions,
	) -> Result<GenerateResult, AcpError> {
		let params = self.build_prompt_params(session_id, prompt, options);
		let mut bridge = self.bridge.lock().await;
		let resp = request(&mut bridge, "session/prompt", Some(serde_json::to_value(&params)?)).await?;
		if let Some(error) = resp.error {
			return Err(AcpError::Protocol(error.message));
		}
		let result: SessionPromptResult = serde_json::from_value(
			resp.result.ok_or_else(|| AcpError::Protocol("No result".into()))?
		)?;
		let content = extract_text_content(&result.content);
		Ok(GenerateResult {
			content,
			stop_reason: result.stop_reason,
			usage: result.metadata.and_then(|m| m.usage),
		})
	}

	/// Generate with streaming — returns a receiver of stream events.
	pub async fn generate_stream(
		&self,
		session_id: &str,
		prompt: &str,
		options: &GenerateOptions,
	) -> Result<mpsc::UnboundedReceiver<StreamEvent>, AcpError> {
		let params = self.build_prompt_params(session_id, prompt, options);
		let (notif_tx, mut notif_rx) = mpsc::unbounded_channel::<JsonRpcNotification>();
		let (event_tx, event_rx) = mpsc::unbounded_channel::<StreamEvent>();

		let bridge = self.bridge.clone();

		tokio::spawn(async move {
			let mut bridge = bridge.lock().await;
			let resp = request_streaming(
				&mut bridge,
				"session/prompt",
				Some(serde_json::to_value(&params).unwrap_or_default()),
				notif_tx,
			);

			// Forward notifications as stream events
			let event_tx_clone = event_tx.clone();
			let forward_handle = tokio::spawn(async move {
				while let Some(notif) = notif_rx.recv().await {
					if let Some(event) = parse_notification(&notif) {
						if event_tx_clone.send(event).is_err() {
							break;
						}
					}
				}
			});

			match resp.await {
				Ok(resp) => {
					if let Some(result_val) = resp.result {
						if let Ok(result) = serde_json::from_value::<SessionPromptResult>(result_val) {
							let _ = event_tx.send(StreamEvent::Complete(result));
						}
					}
				}
				Err(e) => {
					// Stream error — channel will just close
					let _ = e;
				}
			}
			forward_handle.abort();
		});

		Ok(event_rx)
	}

	/// Embed texts via ACP.
	pub async fn embed(
		&self,
		session_id: &str,
		texts: &[&str],
		model: Option<&str>,
	) -> Result<EmbedResult, AcpError> {
		let content = vec![ContentBlock::Data {
			data: serde_json::json!({
				"action": "embed",
				"texts": texts,
				"model": model,
			}),
		}];
		let params = SessionPromptParams {
			session_id: session_id.to_string(),
			content,
			metadata: None,
		};
		let mut bridge = self.bridge.lock().await;
		let resp = request(&mut bridge, "session/prompt", Some(serde_json::to_value(&params)?)).await?;
		if let Some(error) = resp.error {
			return Err(AcpError::Protocol(error.message));
		}
		let result: SessionPromptResult = serde_json::from_value(
			resp.result.ok_or_else(|| AcpError::Protocol("No result".into()))?
		)?;
		// Extract embeddings from data block
		for block in &result.content {
			if let ContentBlock::Data { data } = block {
				if let Some(embeddings) = data.get("embeddings") {
					let embs: Vec<Vec<f32>> = serde_json::from_value(embeddings.clone())?;
					let prompt_tokens = result.metadata
						.as_ref()
						.and_then(|m| m.usage.as_ref())
						.map(|u| u.prompt_tokens)
						.unwrap_or(0);
					return Ok(EmbedResult { embeddings: embs, prompt_tokens });
				}
			}
		}
		Err(AcpError::Protocol("No embeddings in response".into()))
	}

	/// Set a session config option (e.g., mode or model).
	pub async fn set_session_config(
		&self,
		session_id: &str,
		config_option_id: &str,
		group_id: &str,
	) -> Result<(), AcpError> {
		let params = serde_json::json!({
			"sessionId": session_id,
			"configOptionId": config_option_id,
			"groupId": group_id,
		});
		let mut bridge = self.bridge.lock().await;
		let resp = request(&mut bridge, "session/set_config_option", Some(params)).await?;
		if let Some(error) = resp.error {
			return Err(AcpError::Protocol(error.message));
		}
		Ok(())
	}

	/// Respond to a permission request.
	pub async fn respond_permission(
		&self,
		request_id: &str,
		option_id: &str,
	) -> Result<(), AcpError> {
		let params = serde_json::json!({
			"id": request_id,
			"outcome": {
				"outcome": "selected",
				"optionId": option_id,
			}
		});
		let mut bridge = self.bridge.lock().await;
		// Permission responses are sent as notifications (no id expected back)
		let line = serde_json::to_string(&serde_json::json!({
			"jsonrpc": "2.0",
			"method": "session/permission_response",
			"params": params,
		}))?;
		send_line(&mut bridge, &line).await?;
		Ok(())
	}

	// ── Internal helpers ──

	fn build_prompt_params(
		&self,
		session_id: &str,
		prompt: &str,
		options: &GenerateOptions,
	) -> SessionPromptParams {
		let content = vec![ContentBlock::Text { text: prompt.to_string() }];
		let metadata = PromptMetadata {
			agent_id: options.agent_id.clone(),
			system_prompt: options.system_prompt.clone(),
			temperature: options.temperature,
			max_tokens: options.max_tokens,
			images: options.images.clone(),
		};
		SessionPromptParams {
			session_id: session_id.to_string(),
			content,
			metadata: Some(metadata),
		}
	}
}

/// Extract text content from content blocks.
pub fn extract_text_content(blocks: &[ContentBlock]) -> String {
	blocks
		.iter()
		.filter_map(|b| match b {
			ContentBlock::Text { text } => Some(text.as_str()),
			_ => None,
		})
		.collect::<Vec<_>>()
		.join("")
}

/// Parse a notification into a StreamEvent.
fn parse_notification(notif: &JsonRpcNotification) -> Option<StreamEvent> {
	if notif.method != "session/update" {
		return None;
	}
	let params = notif.params.as_ref()?;
	let update: SessionUpdate = serde_json::from_value(params.clone()).ok()?;

	match update.update.session_update.as_str() {
		"agent_message_chunk" => {
			let text = extract_text_content(&update.update.content);
			if !text.is_empty() {
				Some(StreamEvent::Delta(text))
			} else {
				None
			}
		}
		"tool_call" => {
			// Extract tool call info from content
			let data = update.update.content.first()?;
			if let ContentBlock::Data { data } = data {
				Some(StreamEvent::ToolCall {
					id: data.get("id")?.as_str()?.to_string(),
					name: data.get("name")?.as_str()?.to_string(),
					args: data.get("arguments").map(|v| v.to_string()).unwrap_or_default(),
				})
			} else {
				None
			}
		}
		"tool_call_update" => {
			let data = update.update.content.first()?;
			if let ContentBlock::Data { data } = data {
				Some(StreamEvent::ToolCallUpdate {
					id: data.get("id")?.as_str()?.to_string(),
					status: data.get("status")?.as_str()?.to_string(),
					summary: data.get("summary").and_then(|v| v.as_str()).map(String::from),
				})
			} else {
				None
			}
		}
		_ => None,
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn extract_text_from_content_blocks() {
		let blocks = vec![
			ContentBlock::Text { text: "Hello ".into() },
			ContentBlock::Data { data: serde_json::json!({"key": "value"}) },
			ContentBlock::Text { text: "world".into() },
		];
		assert_eq!(extract_text_content(&blocks), "Hello world");
	}

	#[test]
	fn extract_text_from_empty_blocks() {
		let blocks: Vec<ContentBlock> = vec![];
		assert_eq!(extract_text_content(&blocks), "");
	}

	#[test]
	fn content_block_text_serializes() {
		let block = ContentBlock::Text { text: "hello".into() };
		let json = serde_json::to_string(&block).unwrap();
		assert!(json.contains("\"type\":\"text\""));
		assert!(json.contains("\"text\":\"hello\""));
	}

	#[test]
	fn content_block_data_serializes() {
		let block = ContentBlock::Data {
			data: serde_json::json!({"action": "embed"}),
		};
		let json = serde_json::to_string(&block).unwrap();
		assert!(json.contains("\"type\":\"data\""));
	}

	#[test]
	fn session_prompt_params_serializes() {
		let params = SessionPromptParams {
			session_id: "test-123".into(),
			content: vec![ContentBlock::Text { text: "prompt".into() }],
			metadata: Some(PromptMetadata {
				agent_id: Some("agent-1".into()),
				..Default::default()
			}),
		};
		let json = serde_json::to_value(&params).unwrap();
		assert_eq!(json["sessionId"], "test-123");
		assert_eq!(json["content"][0]["type"], "text");
		assert_eq!(json["metadata"]["agentId"], "agent-1");
	}

	#[test]
	fn parse_notification_agent_message_chunk() {
		let notif = JsonRpcNotification {
			jsonrpc: "2.0".into(),
			method: "session/update".into(),
			params: Some(serde_json::json!({
				"sessionId": "s1",
				"update": {
					"sessionUpdate": "agent_message_chunk",
					"content": [{ "type": "text", "text": "Hello" }]
				}
			})),
		};
		let event = parse_notification(&notif).unwrap();
		match event {
			StreamEvent::Delta(text) => assert_eq!(text, "Hello"),
			other => panic!("Expected Delta, got {other:?}"),
		}
	}

	#[test]
	fn parse_notification_ignores_non_session_update() {
		let notif = JsonRpcNotification {
			jsonrpc: "2.0".into(),
			method: "other/method".into(),
			params: None,
		};
		assert!(parse_notification(&notif).is_none());
	}

	#[test]
	fn initialize_result_deserializes() {
		let json = r#"{"protocolVersion":1,"agentInfo":{"name":"test-server","version":"0.1.0"}}"#;
		let result: InitializeResult = serde_json::from_str(json).unwrap();
		assert_eq!(result.protocol_version, 1);
		assert_eq!(result.agent_info.unwrap().name, "test-server");
	}

	#[test]
	fn token_usage_deserializes() {
		let json = r#"{"promptTokens":100,"completionTokens":50}"#;
		let usage: TokenUsage = serde_json::from_str(json).unwrap();
		assert_eq!(usage.prompt_tokens, 100);
		assert_eq!(usage.completion_tokens, 50);
	}

	#[test]
	fn generate_options_defaults() {
		let opts = GenerateOptions::default();
		assert!(opts.agent_id.is_none());
		assert!(opts.system_prompt.is_none());
		assert!(opts.images.is_empty());
	}

	#[test]
	fn acp_server_info_defaults() {
		let info = AcpServerInfo::default();
		assert_eq!(info.timeout_ms, 60_000);
		assert_eq!(info.init_timeout_ms, 30_000);
	}
}
```

**Step 3: Register modules in lib.rs**

In `simse-bridge/src/lib.rs`, add:
```rust
pub mod acp_types;
pub mod acp_client;
```

**Step 4: Run tests to verify unit tests pass**

Run: `cargo test -p simse-bridge acp`
Expected: All 10 unit tests PASS (these are purely serialization/parsing tests — no subprocess needed)

**Step 5: Run full workspace tests**

Run: `cargo test --workspace`
Expected: All tests PASS

**Step 6: Commit**

```bash
git add simse-bridge/src/acp_types.rs simse-bridge/src/acp_client.rs simse-bridge/src/lib.rs
git commit -m "feat(bridge): add ACP client with session management and streaming"
```

---

## Summary

| Task | Module | Tests | Depends On |
|------|--------|-------|------------|
| 25: JSON I/O | `simse-bridge/json_io.rs` | 11 | — |
| 26: Config Loading | `simse-bridge/config.rs` | ~17 | Task 25 |
| 27: Session Store | `simse-bridge/session_store.rs` | 13 | Task 25 |
| 28: ACP Client | `simse-bridge/acp_client.rs` + `acp_types.rs` | 10 | existing `client.rs` |

**Total new tests: ~51**
**Estimated new test count after Phase 5: ~200+**

After all four tasks, the bridge crate has: JSON I/O primitives, hierarchical config loading with agent/skill/frontmatter support, crash-safe JSONL session persistence, and a full ACP client with streaming and permission handling. This unlocks Phase 6 (library system) and Phase 7 (tool system + agentic loop).
