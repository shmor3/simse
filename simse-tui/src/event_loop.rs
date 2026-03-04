//! TUI runtime — wires the async event loop to ACP, tools, and conversation.
//!
//! This module provides [`TuiRuntime`], the high-level async runtime that
//! sits between the terminal event loop in `main.rs` and the ACP bridge.
//! It manages the ACP client connection, conversation state, tool registry,
//! permission handling, and command dispatch.
//!
//! The actual terminal event loop (crossterm `read_event` + ratatui `draw`)
//! remains in `main.rs`. This module provides the runtime that `main.rs`
//! orchestrates.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use simse_bridge::acp_client::AcpClient;
use simse_bridge::acp_types::AcpServerInfo;
use simse_bridge::agentic_loop::{self, AgenticLoopOptions, LoopCallbacks};
use simse_bridge::config::LoadedConfig;
use simse_bridge::session_store::SessionStore;
use simse_bridge::tool_registry::ToolRegistry;
use simse_ui_core::state::conversation::{ConversationBuffer, ConversationOptions};
use simse_ui_core::state::permission_manager::PermissionManager;
use simse_ui_core::state::permissions::PermissionMode;
use simse_ui_core::tools::ToolCallRequest;

use crate::commands::{
	AgentInfo, BridgeAction, CommandContext, PromptInfo, SessionInfo, SkillInfo, ToolDefInfo,
};

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors that can occur in the TUI runtime.
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
	#[error("Not connected to ACP server")]
	NotConnected,
	#[error("ACP error: {0}")]
	Acp(String),
	#[error("No ACP servers configured")]
	NoServersConfigured,
	#[error("ACP server not found: {0}")]
	ServerNotFound(String),
	#[error("No active session")]
	NoSession,
}

// ---------------------------------------------------------------------------
// TuiRuntime
// ---------------------------------------------------------------------------

/// The high-level TUI runtime that wires ACP, tools, and conversation together.
///
/// This struct owns all the state needed to drive an agentic loop from the TUI.
/// The terminal event loop in `main.rs` calls methods on this struct to connect,
/// submit prompts, handle permissions, and abort.
pub struct TuiRuntime {
	/// Loaded configuration (ACP servers, MCP servers, library, etc.).
	config: LoadedConfig,
	/// ACP client connection (None until `connect()` is called).
	acp_client: Option<AcpClient>,
	/// Conversation state buffer.
	conversation: ConversationBuffer,
	/// Tool registry with discovered tools.
	tool_registry: ToolRegistry,
	/// Permission manager for tool call authorization.
	permission_manager: PermissionManager,
	/// Active ACP session ID.
	session_id: Option<String>,
	/// Abort signal shared with the agentic loop.
	abort_signal: Arc<AtomicBool>,
	/// Whether verbose mode is enabled.
	pub verbose: bool,
	/// Session persistence store.
	session_store: SessionStore,
}

impl TuiRuntime {
	/// Create a new TUI runtime from a loaded configuration.
	pub fn new(config: LoadedConfig) -> Self {
		let session_store = SessionStore::new(&config.data_dir);
		Self {
			config,
			acp_client: None,
			conversation: ConversationBuffer::new(ConversationOptions::default()),
			tool_registry: ToolRegistry::new(),
			permission_manager: PermissionManager::new(PermissionMode::Default),
			session_id: None,
			abort_signal: Arc::new(AtomicBool::new(false)),
			verbose: false,
			session_store,
		}
	}

	/// Connect to the configured ACP server, create a session, and discover tools.
	///
	/// Uses `config.default_server` to select which ACP server to connect to.
	/// If no default is set, uses the first configured server. After connecting,
	/// creates a new ACP session and discovers available tools.
	pub async fn connect(&mut self) -> Result<(), RuntimeError> {
		let server_config = self.resolve_server(None)?;

		let server_info = AcpServerInfo {
			command: server_config.command.clone(),
			args: server_config.args.clone(),
			cwd: server_config.cwd.clone(),
			env: server_config.env.clone(),
			timeout_ms: server_config.timeout_ms.unwrap_or(60_000),
			init_timeout_ms: 30_000,
		};

		let client = AcpClient::connect(server_info)
			.await
			.map_err(|e| RuntimeError::Acp(e.to_string()))?;

		// Create a session
		let session_id = client
			.new_session()
			.await
			.map_err(|e| RuntimeError::Acp(e.to_string()))?;

		self.session_id = Some(session_id);
		self.acp_client = Some(client);

		// Discover tools (built-in + MCP tools from connected servers)
		self.tool_registry.discover(&self.config.mcp_servers).await;

		Ok(())
	}

	/// Connect to a specific ACP server by name.
	pub async fn connect_to(&mut self, server_name: &str) -> Result<(), RuntimeError> {
		let server_config = self.resolve_server(Some(server_name))?;

		let server_info = AcpServerInfo {
			command: server_config.command.clone(),
			args: server_config.args.clone(),
			cwd: server_config.cwd.clone(),
			env: server_config.env.clone(),
			timeout_ms: server_config.timeout_ms.unwrap_or(60_000),
			init_timeout_ms: 30_000,
		};

		let client = AcpClient::connect(server_info)
			.await
			.map_err(|e| RuntimeError::Acp(e.to_string()))?;

		let session_id = client
			.new_session()
			.await
			.map_err(|e| RuntimeError::Acp(e.to_string()))?;

		self.session_id = Some(session_id);
		self.acp_client = Some(client);
		self.tool_registry.discover(&self.config.mcp_servers).await;

		Ok(())
	}

	/// Handle a user submission: dispatch a `/command` or run the agentic loop.
	///
	/// If the input starts with `/`, it is treated as a command and dispatched
	/// locally (returning a command result string). Otherwise, the input is
	/// added to the conversation and the agentic loop is run.
	///
	/// Returns the final text response from the loop, or a command result.
	pub async fn handle_submit(
		&mut self,
		input: &str,
		callbacks: &dyn LoopCallbacks,
	) -> Result<String, RuntimeError> {
		// Commands are handled by the TUI app directly; this method is for
		// user messages that should go through the agentic loop.
		let acp_client = self
			.acp_client
			.as_ref()
			.ok_or(RuntimeError::NotConnected)?;

		// Add the user message to the conversation
		self.conversation.add_user(input);

		// Reset abort signal for this run
		self.abort_signal.store(false, Ordering::Relaxed);

		// Build agentic loop options
		let options = AgenticLoopOptions {
			max_turns: 10,
			server_name: self.config.default_server.clone(),
			agent_id: self.config.default_agent.clone(),
			system_prompt: self.config.workspace_prompt.clone(),
			agent_manages_tools: false,
		};

		let result = agentic_loop::run_agentic_loop(
			&mut self.conversation,
			acp_client,
			&self.tool_registry,
			&options,
			callbacks,
			Arc::clone(&self.abort_signal),
		)
		.await;

		// Add the final assistant response to the conversation
		if !result.final_text.is_empty() {
			self.conversation.add_assistant(&result.final_text);
		}

		Ok(result.final_text)
	}

	/// Handle a permission response from the user.
	///
	/// Forwards the permission decision to the ACP client if connected.
	pub async fn handle_permission_response(
		&mut self,
		request_id: &str,
		option_id: &str,
	) -> Result<(), RuntimeError> {
		let acp_client = self
			.acp_client
			.as_ref()
			.ok_or(RuntimeError::NotConnected)?;

		acp_client
			.respond_permission(request_id, option_id)
			.await
			.map_err(|e| RuntimeError::Acp(e.to_string()))?;

		Ok(())
	}

	/// Set the abort signal, causing the agentic loop to exit at the next check.
	pub fn abort(&self) {
		self.abort_signal.store(true, Ordering::Relaxed);
	}

	/// Check if the abort signal is currently set.
	pub fn is_aborted(&self) -> bool {
		self.abort_signal.load(Ordering::Relaxed)
	}

	/// Check if onboarding is needed (no ACP servers are configured).
	pub fn needs_onboarding(&self) -> bool {
		self.config.acp.servers.is_empty()
	}

	/// Check if the runtime is currently connected to an ACP server.
	pub fn is_connected(&self) -> bool {
		self.acp_client.is_some()
	}

	/// Check if the ACP client is healthy (child process still running).
	pub async fn is_healthy(&self) -> bool {
		match &self.acp_client {
			Some(client) => client.is_healthy().await,
			None => false,
		}
	}

	/// Get the current session ID, if any.
	pub fn session_id(&self) -> Option<&str> {
		self.session_id.as_deref()
	}

	/// Get a reference to the conversation buffer.
	pub fn conversation(&self) -> &ConversationBuffer {
		&self.conversation
	}

	/// Get a mutable reference to the conversation buffer.
	pub fn conversation_mut(&mut self) -> &mut ConversationBuffer {
		&mut self.conversation
	}

	/// Get a reference to the tool registry.
	pub fn tool_registry(&self) -> &ToolRegistry {
		&self.tool_registry
	}

	/// Get a reference to the permission manager.
	pub fn permission_manager(&self) -> &PermissionManager {
		&self.permission_manager
	}

	/// Get a mutable reference to the permission manager.
	pub fn permission_manager_mut(&mut self) -> &mut PermissionManager {
		&mut self.permission_manager
	}

	/// Get a reference to the loaded configuration.
	pub fn config(&self) -> &LoadedConfig {
		&self.config
	}

	/// Get the abort signal Arc for sharing with async tasks.
	pub fn abort_signal(&self) -> Arc<AtomicBool> {
		Arc::clone(&self.abort_signal)
	}

	/// Get the last agentic loop result (for diagnostics).
	///
	/// Returns the agent info from the ACP client, if available.
	pub fn agent_name(&self) -> Option<String> {
		self.acp_client
			.as_ref()
			.and_then(|c| c.agent_info())
			.map(|info| info.name.clone())
	}

	/// Clear the conversation and start fresh.
	pub fn reset_conversation(&mut self) {
		self.conversation = ConversationBuffer::new(ConversationOptions::default());
	}

	/// Build a `CommandContext` snapshot from the current runtime state.
	///
	/// This creates a read-only snapshot that command handlers can use for
	/// sync operations (listing sessions, tools, config, etc.).
	pub fn build_command_context(&self) -> CommandContext {
		let sessions = self
			.session_store
			.list()
			.into_iter()
			.map(|m| SessionInfo {
				id: m.id,
				title: m.title,
				created_at: m.created_at,
				updated_at: m.updated_at,
				message_count: m.message_count,
				work_dir: m.work_dir,
			})
			.collect();

		let tool_defs = self
			.tool_registry
			.get_tool_definitions()
			.into_iter()
			.map(|d| ToolDefInfo {
				name: d.name,
				description: d.description,
			})
			.collect();

		let agents = self
			.config
			.agents
			.iter()
			.map(|a| AgentInfo {
				name: a.name.clone(),
				description: a.description.clone(),
			})
			.collect();

		let skills = self
			.config
			.skills
			.iter()
			.map(|s| SkillInfo {
				name: s.name.clone(),
				description: s.description.clone(),
			})
			.collect();

		let prompts = self
			.config
			.prompts
			.iter()
			.map(|(name, p)| PromptInfo {
				name: name.clone(),
				description: p.description.clone(),
				step_count: p.steps.len(),
			})
			.collect();

		let config_values = self.build_config_display();

		CommandContext {
			sessions,
			tool_defs,
			agents,
			skills,
			prompts,
			server_name: self.config.default_server.clone(),
			model_name: self.config.default_agent.clone(),
			session_id: self.session_id.clone(),
			acp_connected: self.is_connected(),
			data_dir: Some(self.config.data_dir.display().to_string()),
			work_dir: Some(self.config.work_dir.display().to_string()),
			config_values,
		}
	}

	/// Execute a bridge action asynchronously.
	///
	/// Returns a human-readable result string on success, or an error message.
	pub async fn execute_bridge_action(
		&mut self,
		action: BridgeAction,
	) -> Result<String, RuntimeError> {
		match action {
			// ── Session ─────────────────────────────────
			BridgeAction::ResumeSession { id } => {
				let meta = self
					.session_store
					.get(&id)
					.ok_or_else(|| RuntimeError::Acp(format!("Session not found: {id}")))?;
				let messages = self.session_store.load(&id);
				self.conversation =
					ConversationBuffer::new(ConversationOptions::default());
				for msg in &messages {
					match msg.role.as_str() {
						"user" => self.conversation.add_user(&msg.content),
						"assistant" => self.conversation.add_assistant(&msg.content),
						_ => {}
					}
				}
				self.session_id = Some(id.clone());
				Ok(format!("Resumed session: {}", meta.title))
			}
			BridgeAction::RenameSession { title } => {
				let sid = self.session_id.as_ref().ok_or(RuntimeError::NoSession)?;
				self.session_store
					.rename(sid, &title)
					.map_err(|e| RuntimeError::Acp(e.to_string()))?;
				Ok(format!("Session renamed to: {title}"))
			}
			BridgeAction::SwitchServer { name } => {
				self.connect_to(&name).await?;
				Ok(format!("Switched to server: {name}"))
			}
			BridgeAction::SwitchModel { name } => {
				self.config.default_agent = Some(name.clone());
				Ok(format!("Model set to: {name}"))
			}
			BridgeAction::McpRestart => {
				self.tool_registry
					.discover(&self.config.mcp_servers)
					.await;
				Ok("MCP tools rediscovered.".into())
			}
			BridgeAction::AcpRestart => {
				if let Some(ref server) = self.config.default_server.clone() {
					self.connect_to(server).await?;
				} else {
					self.connect().await?;
				}
				Ok("ACP connection restarted.".into())
			}

			// ── Config ──────────────────────────────────
			BridgeAction::InitConfig { force } => {
				let dir = self.config.work_dir.join(".simse");
				if dir.exists() && !force {
					return Ok("Project already initialized. Use --force to overwrite.".into());
				}
				std::fs::create_dir_all(&dir)
					.map_err(|e| RuntimeError::Acp(e.to_string()))?;
				Ok(format!("Initialized project config at {}", dir.display()))
			}
			BridgeAction::FactoryReset => {
				let data_dir = &self.config.data_dir;
				if data_dir.exists() {
					std::fs::remove_dir_all(data_dir)
						.map_err(|e| RuntimeError::Acp(e.to_string()))?;
				}
				Ok("Factory reset complete. Global configuration removed.".into())
			}
			BridgeAction::FactoryResetProject => {
				let dir = self.config.work_dir.join(".simse");
				if dir.exists() {
					std::fs::remove_dir_all(&dir)
						.map_err(|e| RuntimeError::Acp(e.to_string()))?;
				}
				Ok("Project configuration reset.".into())
			}

			// ── AI ──────────────────────────────────────
			BridgeAction::RunChain { name, args } => {
				// Chain execution requires the agentic loop
				let acp_client = self
					.acp_client
					.as_ref()
					.ok_or(RuntimeError::NotConnected)?;

				self.conversation.add_user(&format!(
					"[chain:{name}] {args}"
				));

				self.abort_signal.store(false, Ordering::Relaxed);

				let options = AgenticLoopOptions {
					max_turns: 10,
					server_name: self.config.default_server.clone(),
					agent_id: self.config.default_agent.clone(),
					system_prompt: self.config.workspace_prompt.clone(),
					agent_manages_tools: false,
				};

				let callbacks = simse_bridge::agentic_loop::NoopCallbacks;
				let result = agentic_loop::run_agentic_loop(
					&mut self.conversation,
					acp_client,
					&self.tool_registry,
					&options,
					&callbacks,
					Arc::clone(&self.abort_signal),
				)
				.await;

				if !result.final_text.is_empty() {
					self.conversation.add_assistant(&result.final_text);
				}

				Ok(result.final_text)
			}

			// ── Library ─────────────────────────────────
			BridgeAction::LibraryAdd { topic, text } => {
				self.call_tool("library_shelve", serde_json::json!({
					"topic": topic,
					"text": text,
				}))
				.await
			}
			BridgeAction::LibrarySearch { query } => {
				self.call_tool("library_search", serde_json::json!({
					"query": query,
				}))
				.await
			}
			BridgeAction::LibraryRecommend { query } => {
				self.call_tool("library_search", serde_json::json!({
					"query": query,
					"recommend": true,
				}))
				.await
			}
			BridgeAction::LibraryTopics => {
				self.call_tool("library_search", serde_json::json!({
					"query": "*",
					"listTopics": true,
				}))
				.await
			}
			BridgeAction::LibraryVolumes { topic } => {
				let mut params = serde_json::json!({"query": "*", "listVolumes": true});
				if let Some(t) = topic {
					params["topic"] = serde_json::Value::String(t);
				}
				self.call_tool("library_search", params).await
			}
			BridgeAction::LibraryGet { id } => {
				self.call_tool("library_search", serde_json::json!({
					"id": id,
				}))
				.await
			}
			BridgeAction::LibraryDelete { id } => {
				self.call_tool("library_search", serde_json::json!({
					"id": id,
					"delete": true,
				}))
				.await
			}

			// ── Files ───────────────────────────────────
			BridgeAction::ListFiles { path } => {
				let params = match path {
					Some(p) => serde_json::json!({"path": p}),
					None => serde_json::json!({}),
				};
				self.call_tool("vfs_list", params).await
			}
			BridgeAction::SaveFiles { path } => {
				let params = match path {
					Some(p) => serde_json::json!({"path": p}),
					None => serde_json::json!({}),
				};
				self.call_tool("vfs_write", params).await
			}
			BridgeAction::ValidateFiles { path } => {
				let params = match path {
					Some(p) => serde_json::json!({"path": p, "validate": true}),
					None => serde_json::json!({"validate": true}),
				};
				self.call_tool("vfs_read", params).await
			}
			BridgeAction::DiscardFile { path } => {
				self.call_tool("vfs_write", serde_json::json!({
					"path": path,
					"discard": true,
				}))
				.await
			}
			BridgeAction::DiffFiles { path } => {
				let params = match path {
					Some(p) => serde_json::json!({"path": p, "diff": true}),
					None => serde_json::json!({"diff": true}),
				};
				self.call_tool("vfs_read", params).await
			}

			// ── Meta ────────────────────────────────────
			BridgeAction::Compact => {
				let msg_count = self.conversation.to_messages().len();
				self.conversation
					.compact("[User-requested compaction: conversation history summarized]");
				Ok(format!(
					"Conversation compacted ({msg_count} messages → 1 summary)."
				))
			}
		}
	}

	// -----------------------------------------------------------------------
	// Private helpers
	// -----------------------------------------------------------------------

	/// Execute a tool by name with the given arguments.
	///
	/// Wraps the tool registry's `execute()` API with a generated call ID.
	async fn call_tool(
		&self,
		name: &str,
		arguments: serde_json::Value,
	) -> Result<String, RuntimeError> {
		static CALL_COUNTER: AtomicU64 = AtomicU64::new(1);
		let call = ToolCallRequest {
			id: format!("call_{}", CALL_COUNTER.fetch_add(1, Ordering::Relaxed)),
			name: name.into(),
			arguments,
		};
		let result = self.tool_registry.execute(&call).await;
		if result.is_error {
			Err(RuntimeError::Acp(result.output))
		} else {
			Ok(result.output)
		}
	}

	/// Build a flat list of config key-value pairs for display.
	fn build_config_display(&self) -> Vec<(String, String)> {
		let mut values = Vec::new();
		values.push((
			"log.level".into(),
			self.config.log_level.clone(),
		));
		if let Some(ref server) = self.config.default_server {
			values.push(("acp.default_server".into(), server.clone()));
		}
		if let Some(ref agent) = self.config.default_agent {
			values.push(("acp.default_agent".into(), agent.clone()));
		}
		values.push((
			"embedding.model".into(),
			self.config.embedding_model.clone(),
		));
		values.push((
			"data_dir".into(),
			self.config.data_dir.display().to_string(),
		));
		values.push((
			"work_dir".into(),
			self.config.work_dir.display().to_string(),
		));
		for server in &self.config.acp.servers {
			values.push((
				format!("acp.servers.{}.command", server.name),
				server.command.clone(),
			));
		}
		for mcp in &self.config.mcp_servers {
			values.push((
				format!("mcp.servers.{}.command", mcp.name),
				mcp.command.clone(),
			));
		}
		values
	}

	/// Resolve an ACP server config by name, or use the default/first.
	fn resolve_server(
		&self,
		server_name: Option<&str>,
	) -> Result<simse_bridge::config::AcpServerConfig, RuntimeError> {
		if self.config.acp.servers.is_empty() {
			return Err(RuntimeError::NoServersConfigured);
		}

		let name = server_name
			.map(String::from)
			.or_else(|| self.config.default_server.clone());

		match name {
			Some(ref n) => self
				.config
				.acp
				.servers
				.iter()
				.find(|s| s.name == *n)
				.cloned()
				.ok_or_else(|| RuntimeError::ServerNotFound(n.clone())),
			None => Ok(self.config.acp.servers[0].clone()),
		}
	}
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;
	use simse_bridge::config::{
		AcpFileConfig, AcpServerConfig, EmbedFileConfig, LibraryFileConfig, UserConfig,
		WorkspaceSettings,
	};
	use std::collections::HashMap;
	use std::path::PathBuf;

	/// Build a minimal LoadedConfig for testing.
	fn test_config() -> LoadedConfig {
		LoadedConfig {
			acp: AcpFileConfig {
				servers: vec![AcpServerConfig {
					name: "test-server".into(),
					command: "echo".into(),
					args: vec!["hello".into()],
					cwd: None,
					env: HashMap::new(),
					default_agent: None,
					timeout_ms: Some(5000),
				}],
				default_server: Some("test-server".into()),
				default_agent: None,
			},
			mcp_servers: Vec::new(),
			skipped_servers: Vec::new(),
			embed: EmbedFileConfig::default(),
			library: LibraryFileConfig::default(),
			summarize: None,
			user: UserConfig::default(),
			workspace_settings: WorkspaceSettings::default(),
			prompts: HashMap::new(),
			agents: Vec::new(),
			skills: Vec::new(),
			workspace_prompt: None,
			log_level: "warn".into(),
			default_agent: None,
			default_server: Some("test-server".into()),
			embedding_model: "nomic-ai/nomic-embed-text-v1.5".into(),
			data_dir: PathBuf::from("/tmp/simse-test"),
			work_dir: PathBuf::from("/tmp/simse-test-work"),
		}
	}

	/// Build a config with no ACP servers (needs onboarding).
	fn empty_config() -> LoadedConfig {
		LoadedConfig {
			acp: AcpFileConfig::default(),
			mcp_servers: Vec::new(),
			skipped_servers: Vec::new(),
			embed: EmbedFileConfig::default(),
			library: LibraryFileConfig::default(),
			summarize: None,
			user: UserConfig::default(),
			workspace_settings: WorkspaceSettings::default(),
			prompts: HashMap::new(),
			agents: Vec::new(),
			skills: Vec::new(),
			workspace_prompt: None,
			log_level: "warn".into(),
			default_agent: None,
			default_server: None,
			embedding_model: "nomic-ai/nomic-embed-text-v1.5".into(),
			data_dir: PathBuf::from("/tmp/simse-test"),
			work_dir: PathBuf::from("/tmp/simse-test-work"),
		}
	}

	#[test]
	fn event_loop_new_runtime() {
		let config = test_config();
		let runtime = TuiRuntime::new(config);
		assert!(!runtime.is_connected());
		assert!(runtime.session_id().is_none());
		assert!(!runtime.verbose);
	}

	#[test]
	fn event_loop_needs_onboarding_no_servers() {
		let config = empty_config();
		let runtime = TuiRuntime::new(config);
		assert!(runtime.needs_onboarding());
	}

	#[test]
	fn event_loop_needs_onboarding_with_servers() {
		let config = test_config();
		let runtime = TuiRuntime::new(config);
		assert!(!runtime.needs_onboarding());
	}

	#[test]
	fn event_loop_abort_signal() {
		let runtime = TuiRuntime::new(test_config());
		assert!(!runtime.is_aborted());
		runtime.abort();
		assert!(runtime.is_aborted());
	}

	#[test]
	fn event_loop_abort_signal_shared() {
		let runtime = TuiRuntime::new(test_config());
		let signal = runtime.abort_signal();
		assert!(!signal.load(Ordering::Relaxed));
		runtime.abort();
		assert!(signal.load(Ordering::Relaxed));
	}

	#[test]
	fn event_loop_not_connected_initially() {
		let runtime = TuiRuntime::new(test_config());
		assert!(!runtime.is_connected());
	}

	#[tokio::test]
	async fn event_loop_not_healthy_when_disconnected() {
		let runtime = TuiRuntime::new(test_config());
		assert!(!runtime.is_healthy().await);
	}

	#[test]
	fn event_loop_conversation_access() {
		let mut runtime = TuiRuntime::new(test_config());
		runtime.conversation_mut().add_user("Hello");
		let messages = runtime.conversation().to_messages();
		assert_eq!(messages.len(), 1);
	}

	#[test]
	fn event_loop_reset_conversation() {
		let mut runtime = TuiRuntime::new(test_config());
		runtime.conversation_mut().add_user("Hello");
		assert!(!runtime.conversation().to_messages().is_empty());
		runtime.reset_conversation();
		assert!(runtime.conversation().to_messages().is_empty());
	}

	#[test]
	fn event_loop_tool_registry_access() {
		let runtime = TuiRuntime::new(test_config());
		assert_eq!(runtime.tool_registry().tool_count(), 0);
	}

	#[test]
	fn event_loop_permission_manager_access() {
		let runtime = TuiRuntime::new(test_config());
		let _pm = runtime.permission_manager();
	}

	#[test]
	fn event_loop_config_access() {
		let runtime = TuiRuntime::new(test_config());
		assert_eq!(runtime.config().log_level, "warn");
		assert_eq!(
			runtime.config().default_server.as_deref(),
			Some("test-server")
		);
	}

	#[test]
	fn event_loop_resolve_server_default() {
		let runtime = TuiRuntime::new(test_config());
		let server = runtime.resolve_server(None).unwrap();
		assert_eq!(server.name, "test-server");
	}

	#[test]
	fn event_loop_resolve_server_by_name() {
		let runtime = TuiRuntime::new(test_config());
		let server = runtime.resolve_server(Some("test-server")).unwrap();
		assert_eq!(server.name, "test-server");
	}

	#[test]
	fn event_loop_resolve_server_not_found() {
		let runtime = TuiRuntime::new(test_config());
		let err = runtime.resolve_server(Some("nonexistent")).unwrap_err();
		match err {
			RuntimeError::ServerNotFound(name) => assert_eq!(name, "nonexistent"),
			_ => panic!("Expected ServerNotFound"),
		}
	}

	#[test]
	fn event_loop_resolve_server_no_servers() {
		let runtime = TuiRuntime::new(empty_config());
		let err = runtime.resolve_server(None).unwrap_err();
		assert!(matches!(err, RuntimeError::NoServersConfigured));
	}

	#[test]
	fn event_loop_resolve_server_first_when_no_default() {
		let mut config = test_config();
		config.default_server = None;
		let runtime = TuiRuntime::new(config);
		let server = runtime.resolve_server(None).unwrap();
		assert_eq!(server.name, "test-server");
	}

	#[test]
	fn event_loop_agent_name_none_when_disconnected() {
		let runtime = TuiRuntime::new(test_config());
		assert!(runtime.agent_name().is_none());
	}

	#[test]
	fn event_loop_verbose_default_false() {
		let runtime = TuiRuntime::new(test_config());
		assert!(!runtime.verbose);
	}

	#[test]
	fn event_loop_verbose_can_be_set() {
		let mut runtime = TuiRuntime::new(test_config());
		runtime.verbose = true;
		assert!(runtime.verbose);
	}

	#[tokio::test]
	async fn event_loop_handle_submit_not_connected() {
		let mut runtime = TuiRuntime::new(test_config());
		let cb = simse_bridge::agentic_loop::NoopCallbacks;
		let err = runtime.handle_submit("hello", &cb).await.unwrap_err();
		assert!(matches!(err, RuntimeError::NotConnected));
	}

	#[tokio::test]
	async fn event_loop_handle_permission_not_connected() {
		let mut runtime = TuiRuntime::new(test_config());
		let err = runtime
			.handle_permission_response("req-1", "allow")
			.await
			.unwrap_err();
		assert!(matches!(err, RuntimeError::NotConnected));
	}

	#[test]
	fn event_loop_error_display() {
		assert_eq!(
			format!("{}", RuntimeError::NotConnected),
			"Not connected to ACP server"
		);
		assert_eq!(
			format!("{}", RuntimeError::NoServersConfigured),
			"No ACP servers configured"
		);
		assert_eq!(
			format!("{}", RuntimeError::ServerNotFound("x".into())),
			"ACP server not found: x"
		);
		assert_eq!(
			format!("{}", RuntimeError::NoSession),
			"No active session"
		);
		assert_eq!(
			format!("{}", RuntimeError::Acp("timeout".into())),
			"ACP error: timeout"
		);
	}
}
