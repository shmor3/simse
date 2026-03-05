//! Real filesystem integration tests for simse-bridge.
//!
//! These tests exercise config loading, session persistence, and storage
//! against actual files on disk. They fill gaps not covered by unit tests:
//!
//! - Cross-instance persistence (create store, write, create NEW store, read back)
//! - Full lifecycle flows (create → append → load → rename → remove in one test)
//! - Config loading with all config files populated simultaneously
//! - Storage backend cross-instance persistence

use simse_bridge::config::*;
use simse_bridge::json_io::write_json_file;
use simse_bridge::session_store::{SessionMessage, SessionStore};
use simse_bridge::storage::{FileStorageBackend, StorageOptions};
use std::collections::HashMap;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn msg(role: &str, content: &str) -> SessionMessage {
	SessionMessage {
		role: role.into(),
		content: content.into(),
		tool_call_id: None,
		tool_name: None,
	}
}

// ===========================================================================
// Session store: cross-instance persistence
// ===========================================================================

/// Two independent `SessionStore` instances sharing the same directory must
/// see each other's writes. This verifies that persistence is truly
/// file-backed and not cached in memory.
#[test]
fn session_store_cross_instance_persistence() {
	let dir = TempDir::new().unwrap();

	// Instance A: create a session and append messages
	let store_a = SessionStore::new(dir.path());
	let id = store_a.create("/workspace").unwrap();
	store_a.append(&id, &msg("user", "Hello from A")).unwrap();
	store_a
		.append(&id, &msg("assistant", "Reply from A"))
		.unwrap();
	store_a.rename(&id, "Cross-Instance Test").unwrap();

	// Instance B: completely new SessionStore on the same directory
	let store_b = SessionStore::new(dir.path());

	// B should see the session in the index
	let list = store_b.list();
	assert_eq!(list.len(), 1, "Instance B should see 1 session");
	assert_eq!(list[0].id, id);
	assert_eq!(list[0].title, "Cross-Instance Test");
	assert_eq!(list[0].message_count, 2);
	assert_eq!(list[0].work_dir, "/workspace");

	// B should be able to load the messages
	let messages = store_b.load(&id);
	assert_eq!(messages.len(), 2);
	assert_eq!(messages[0].role, "user");
	assert_eq!(messages[0].content, "Hello from A");
	assert_eq!(messages[1].role, "assistant");
	assert_eq!(messages[1].content, "Reply from A");

	// B should be able to append more messages that A can read
	store_b.append(&id, &msg("user", "Hello from B")).unwrap();

	let messages_from_a = store_a.load(&id);
	assert_eq!(messages_from_a.len(), 3);
	assert_eq!(messages_from_a[2].content, "Hello from B");
}

/// Multiple sessions created across different store instances should all
/// coexist in the shared index.
#[test]
fn session_store_multi_session_cross_instance() {
	let dir = TempDir::new().unwrap();

	let store_1 = SessionStore::new(dir.path());
	let id_a = store_1.create("/project-a").unwrap();
	store_1.append(&id_a, &msg("user", "msg-a")).unwrap();

	let store_2 = SessionStore::new(dir.path());
	let id_b = store_2.create("/project-b").unwrap();
	store_2.append(&id_b, &msg("user", "msg-b")).unwrap();

	// A third instance should see both sessions
	let store_3 = SessionStore::new(dir.path());
	let list = store_3.list();
	assert_eq!(list.len(), 2);

	let messages_a = store_3.load(&id_a);
	assert_eq!(messages_a.len(), 1);
	assert_eq!(messages_a[0].content, "msg-a");

	let messages_b = store_3.load(&id_b);
	assert_eq!(messages_b.len(), 1);
	assert_eq!(messages_b[0].content, "msg-b");
}

// ===========================================================================
// Session store: full lifecycle in one flow
// ===========================================================================

/// Single end-to-end test that exercises the complete session lifecycle:
/// create → append → load → rename → get → latest → remove.
#[test]
fn session_store_full_lifecycle() {
	let dir = TempDir::new().unwrap();
	let store = SessionStore::new(dir.path());

	// 1. Create
	let id = store.create("/my/project").unwrap();
	assert!(!id.is_empty());

	// 2. Append a conversation
	store.append(&id, &msg("user", "What is Rust?")).unwrap();
	store
		.append(
			&id,
			&msg("assistant", "Rust is a systems programming language."),
		)
		.unwrap();
	store
		.append(&id, &msg("user", "Tell me more."))
		.unwrap();

	// 3. Load and verify
	let messages = store.load(&id);
	assert_eq!(messages.len(), 3);
	assert_eq!(messages[0].content, "What is Rust?");
	assert_eq!(messages[2].content, "Tell me more.");

	// 4. Rename
	store.rename(&id, "Rust Discussion").unwrap();
	let meta = store.get(&id).unwrap();
	assert_eq!(meta.title, "Rust Discussion");
	assert_eq!(meta.message_count, 3);

	// 5. Latest
	let latest_id = store.latest("/my/project").unwrap();
	assert_eq!(latest_id, id);

	// 6. Remove
	store.remove(&id).unwrap();
	assert!(store.get(&id).is_none());
	assert!(store.list().is_empty());
	assert!(store.load(&id).is_empty());
}

// ===========================================================================
// Config loading: all files populated
// ===========================================================================

/// Load config from a directory tree where every config file is populated.
/// This verifies that all config sources are read and merged correctly
/// when present simultaneously.
#[test]
fn config_load_all_files_populated() {
	let dir = TempDir::new().unwrap();
	let data_dir = dir.path().join("data");
	let work_dir = dir.path().join("work");
	let simse_dir = work_dir.join(".simse");
	let agents_dir = simse_dir.join("agents");
	let skills_dir = simse_dir.join("skills").join("search");

	std::fs::create_dir_all(&data_dir).unwrap();
	std::fs::create_dir_all(&agents_dir).unwrap();
	std::fs::create_dir_all(&skills_dir).unwrap();

	// -- Global config files --

	write_json_file(
		&data_dir.join("config.json"),
		&UserConfig {
			default_agent: Some("global-agent".into()),
			log_level: Some("info".into()),
			perplexity_api_key: Some("pplx-xxx".into()),
			github_token: None,
		},
	)
	.unwrap();

	write_json_file(
		&data_dir.join("acp.json"),
		&AcpFileConfig {
			servers: vec![AcpServerConfig {
				name: "claude".into(),
				command: "node".into(),
				args: vec!["server.js".into()],
				cwd: None,
				env: HashMap::new(),
				default_agent: Some("claude-agent".into()),
				timeout_ms: Some(30_000),
			}],
			default_server: Some("claude".into()),
			default_agent: None,
		},
	)
	.unwrap();

	write_json_file(
		&data_dir.join("mcp.json"),
		&McpFileConfig {
			servers: vec![McpServerConfig {
				name: "filesystem".into(),
				transport: "stdio".into(),
				command: "npx".into(),
				args: vec!["-y".into(), "@modelcontextprotocol/server-filesystem".into()],
				env: HashMap::new(),
				required_env: vec![],
			}],
		},
	)
	.unwrap();

	write_json_file(
		&data_dir.join("embed.json"),
		&EmbedFileConfig {
			embedding_model: Some("custom-model/v2".into()),
			dtype: Some("float32".into()),
			tei_url: None,
		},
	)
	.unwrap();

	write_json_file(
		&data_dir.join("memory.json"),
		&LibraryFileConfig {
			enabled: true,
			similarity_threshold: 0.85,
			max_results: 20,
			auto_save: Some(true),
			duplicate_threshold: Some(0.95),
			duplicate_behavior: Some("skip".into()),
			flush_interval_ms: None,
			compression_level: Some(9),
			atomic_write: Some(true),
			auto_summarize_threshold: Some(100),
		},
	)
	.unwrap();

	// -- Workspace config files --

	write_json_file(
		&simse_dir.join("settings.json"),
		&WorkspaceSettings {
			default_agent: Some("workspace-agent".into()),
			log_level: Some("debug".into()),
			system_prompt: Some("Be concise.".into()),
			default_server: Some("claude".into()),
			conversation_topic: Some("coding".into()),
			chain_topic: None,
		},
	)
	.unwrap();

	// Agent file
	std::fs::write(
		agents_dir.join("coder.md"),
		"---\nname: coder\ndescription: A coding assistant\nmodel: gpt-4\n---\nYou are a coding assistant that writes clean Rust code.",
	)
	.unwrap();

	// Skill file
	std::fs::write(
		skills_dir.join("SKILL.md"),
		"---\nname: search\ndescription: Web search\nallowed-tools: web_search, fetch\nargument-hint: query\n---\nSearch the web for information.",
	)
	.unwrap();

	// SIMSE.md workspace prompt
	std::fs::write(
		work_dir.join("SIMSE.md"),
		"You are a helpful AI assistant working on this project.",
	)
	.unwrap();

	// -- Load and verify --

	let config = load_config(&ConfigOptions {
		data_dir: Some(data_dir),
		work_dir: Some(work_dir),
		..Default::default()
	});

	// ACP
	assert_eq!(config.acp.servers.len(), 1);
	assert_eq!(config.acp.servers[0].name, "claude");
	assert_eq!(config.acp.default_server.as_deref(), Some("claude"));

	// MCP (no required_env so should pass through)
	assert_eq!(config.mcp_servers.len(), 1);
	assert_eq!(config.mcp_servers[0].name, "filesystem");
	assert!(config.skipped_servers.is_empty());

	// Embed
	assert_eq!(config.embedding_model, "custom-model/v2");
	assert_eq!(config.embed.dtype.as_deref(), Some("float32"));

	// Library
	assert!(config.library.enabled);
	assert!((config.library.similarity_threshold - 0.85).abs() < f64::EPSILON);
	assert_eq!(config.library.max_results, 20);
	assert_eq!(config.library.auto_save, Some(true));
	assert_eq!(config.library.compression_level, Some(9));

	// Workspace settings — workspace overrides global for log_level and default_agent
	assert_eq!(config.log_level, "debug");
	assert_eq!(config.default_agent.as_deref(), Some("workspace-agent"));
	assert_eq!(config.default_server.as_deref(), Some("claude"));

	// Agents
	assert_eq!(config.agents.len(), 1);
	assert_eq!(config.agents[0].name, "coder");
	assert_eq!(config.agents[0].model.as_deref(), Some("gpt-4"));
	assert!(config.agents[0]
		.system_prompt
		.contains("clean Rust code"));

	// Skills
	assert_eq!(config.skills.len(), 1);
	assert_eq!(config.skills[0].name, "search");
	assert_eq!(config.skills[0].allowed_tools, vec!["web_search", "fetch"]);

	// Workspace prompt
	assert_eq!(
		config.workspace_prompt.as_deref(),
		Some("You are a helpful AI assistant working on this project.")
	);
}

/// Config precedence: CLI options override workspace settings which
/// override global config, even when all files exist.
#[test]
fn config_cli_overrides_all_layers() {
	let dir = TempDir::new().unwrap();
	let data_dir = dir.path().join("data");
	let work_dir = dir.path().join("work");
	let simse_dir = work_dir.join(".simse");
	std::fs::create_dir_all(&data_dir).unwrap();
	std::fs::create_dir_all(&simse_dir).unwrap();

	// Global config
	write_json_file(
		&data_dir.join("config.json"),
		&UserConfig {
			default_agent: Some("global-agent".into()),
			log_level: Some("info".into()),
			..Default::default()
		},
	)
	.unwrap();

	// Workspace settings
	write_json_file(
		&simse_dir.join("settings.json"),
		&WorkspaceSettings {
			default_agent: Some("ws-agent".into()),
			log_level: Some("debug".into()),
			default_server: Some("ws-server".into()),
			..Default::default()
		},
	)
	.unwrap();

	// ACP config with default_server
	write_json_file(
		&data_dir.join("acp.json"),
		&AcpFileConfig {
			servers: vec![],
			default_server: Some("acp-server".into()),
			default_agent: None,
		},
	)
	.unwrap();

	// CLI overrides everything
	let config = load_config(&ConfigOptions {
		data_dir: Some(data_dir),
		work_dir: Some(work_dir),
		default_agent: Some("cli-agent".into()),
		log_level: Some("error".into()),
		server_name: Some("cli-server".into()),
	});

	assert_eq!(config.log_level, "error");
	assert_eq!(config.default_agent.as_deref(), Some("cli-agent"));
	assert_eq!(config.default_server.as_deref(), Some("cli-server"));
}

// ===========================================================================
// Storage backend: cross-instance persistence
// ===========================================================================

/// Save data with one `FileStorageBackend`, then load it with a completely
/// new instance pointing to the same path.
#[tokio::test]
async fn storage_backend_cross_instance_persistence() {
	let dir = TempDir::new().unwrap();
	let path = dir.path().join("store.simk");

	// Instance A: write data
	{
		let backend_a = FileStorageBackend::new(path.clone(), StorageOptions::default());
		let mut data = HashMap::new();
		data.insert("key-1".to_string(), b"value-one".to_vec());
		data.insert("key-2".to_string(), vec![0xDE, 0xAD, 0xBE, 0xEF]);
		data.insert("key-3".to_string(), Vec::new());
		backend_a.save(&data).await.unwrap();
	}

	// Instance B: completely new backend, same path
	{
		let backend_b = FileStorageBackend::new(path.clone(), StorageOptions::default());
		let loaded = backend_b.load().await.unwrap();

		assert_eq!(loaded.len(), 3);
		assert_eq!(loaded.get("key-1").unwrap(), b"value-one");
		assert_eq!(loaded.get("key-2").unwrap(), &vec![0xDE, 0xAD, 0xBE, 0xEF]);
		assert_eq!(loaded.get("key-3").unwrap(), &Vec::<u8>::new());
	}
}

/// Save, then overwrite with different data, then load from a new instance.
/// Verifies that save replaces (not appends to) the file.
#[tokio::test]
async fn storage_backend_overwrite_replaces_data() {
	let dir = TempDir::new().unwrap();
	let path = dir.path().join("overwrite.simk");

	let backend = FileStorageBackend::new(path.clone(), StorageOptions::default());

	// First write
	let mut data1 = HashMap::new();
	data1.insert("old-key".to_string(), b"old-value".to_vec());
	backend.save(&data1).await.unwrap();

	// Overwrite with different data
	let mut data2 = HashMap::new();
	data2.insert("new-key".to_string(), b"new-value".to_vec());
	backend.save(&data2).await.unwrap();

	// New instance should see only the second write
	let backend2 = FileStorageBackend::new(path, StorageOptions::default());
	let loaded = backend2.load().await.unwrap();

	assert_eq!(loaded.len(), 1);
	assert!(loaded.get("old-key").is_none());
	assert_eq!(loaded.get("new-key").unwrap(), b"new-value");
}
