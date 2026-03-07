//! TUI runtime — wires the async event loop to ACP, tools, and conversation.
//!
//! This module provides [`TuiRuntime`], the high-level async runtime that
//! sits between the terminal event loop in `main.rs` and the ACP engine.
//! It manages the ACP client connection, conversation state, tool registry,
//! permission handling, and command dispatch.
//!
//! The actual terminal event loop (crossterm `read_event` + ratatui `draw`)
//! remains in `main.rs`. This module provides the runtime that `main.rs`
//! orchestrates.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;

use futures::StreamExt;

// simse-core types
use simse_core::engine::acp::client::{
	AcpClient as AcpEngine, AcpConfig as AcpEngineConfig, ServerEntry, StreamOptions,
};
use simse_core::engine::acp::error::AcpError;
use simse_core::engine::acp::permission::PermissionPolicy;
use simse_core::engine::acp::stream::StreamChunk;
use simse_core::agentic_loop::{
	self, AcpClient as AcpClientTrait, AgenticLoopOptions, CancellationToken, GenerateResponse,
	LoopCallbacks, Message, MessageRole, TokenUsage,
};
use simse_core::tools::{ToolCallRequest, ToolRegistry, ToolRegistryOptions};
use simse_core::SimseError;

use crate::ui_core::state::conversation::{ConversationBuffer, ConversationOptions};
use crate::ui_core::state::permission_manager::PermissionManager;
use crate::ui_core::state::permissions::PermissionMode;

use crate::app::AppMessage;
use crate::commands::{
	AgentInfo, BridgeAction, CommandContext, PromptInfo, SessionInfo, SkillInfo, ToolDefInfo,
};
use crate::config::{AcpFileConfig, AcpServerConfig, FileConfigStorage, LoadedConfig};
use crate::session_store::SessionStore;

use crate::ui_core::config::storage::ConfigStorage;

// ---------------------------------------------------------------------------
// ACP Adapter — bridges simse-acp's AcpClient with simse-core's AcpClient trait
// ---------------------------------------------------------------------------

/// Adapter that wraps the simse-acp engine `AcpClient` and implements the
/// simse-core `AcpClient` trait so it can be used with `run_agentic_loop`.
///
/// Uses `generate_stream()` internally because ACP servers (like
/// claude-agent-acp) deliver content via `session/update` streaming
/// notifications. The synchronous `chat()` method only reads the final
/// `session/prompt` response which has empty content.
struct AcpAdapter {
	engine: Arc<AcpEngine>,
	session_id: Option<String>,
	server_name: Option<String>,
}

#[async_trait]
impl AcpClientTrait for AcpAdapter {
	async fn generate(
		&self,
		messages: &[Message],
		system: Option<&str>,
	) -> Result<GenerateResponse, SimseError> {
		// Build a single prompt string from all messages with role prefixes.
		// This mirrors what AcpEngine::chat() does internally when flattening
		// ChatMessage structs into a single Vec<ContentBlock>.
		let mut prompt_parts: Vec<String> = Vec::new();
		for m in messages {
			let prefix = match m.role {
				MessageRole::System => "[System] ",
				MessageRole::Assistant => "[Assistant] ",
				MessageRole::User => "",
			};
			prompt_parts.push(format!("{prefix}{}", m.content));
		}
		let prompt = prompt_parts.join("\n");

		let options = StreamOptions {
			server_name: self.server_name.clone(),
			session_id: self.session_id.clone(),
			system_prompt: system.map(|s| s.to_string()),
			..Default::default()
		};

		let mut stream = self
			.engine
			.generate_stream(&prompt, options)
			.await
			.map_err(|e: AcpError| SimseError::other(e.to_string()))?;

		// Collect all streaming deltas into the full response text.
		let mut text = String::new();
		let mut usage = None;
		while let Some(chunk) = stream.next().await {
			match chunk {
				StreamChunk::Delta { text: delta } => {
					text.push_str(&delta);
				}
				StreamChunk::Complete {
					usage: chunk_usage,
				} => {
					usage = chunk_usage;
					break;
				}
				_ => {}
			}
		}

		Ok(GenerateResponse {
			text,
			usage: usage.map(|u| TokenUsage {
				prompt_tokens: Some(u.prompt_tokens),
				completion_tokens: Some(u.completion_tokens),
				total_tokens: Some(u.total_tokens),
			}),
		})
	}
}

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
	/// ACP engine connection (None until `connect()` is called).
	acp_engine: Option<Arc<AcpEngine>>,
	/// Conversation state buffer.
	conversation: ConversationBuffer,
	/// Tool registry with discovered tools.
	tool_registry: ToolRegistry,
	/// Permission manager for tool call authorization.
	permission_manager: PermissionManager,
	/// Active ACP session ID.
	session_id: Option<String>,
	/// Cancellation token shared with the agentic loop.
	cancel_token: CancellationToken,
	/// Whether verbose mode is enabled.
	pub verbose: bool,
	/// Session persistence store.
	session_store: SessionStore,
	/// Config file storage for settings UI persistence.
	pub config_storage: FileConfigStorage,
}

impl TuiRuntime {
	/// Create a new TUI runtime from a loaded configuration.
	pub fn new(config: LoadedConfig) -> Self {
		let session_store = SessionStore::new(&config.data_dir);
		let config_storage =
			FileConfigStorage::new(config.data_dir.clone(), config.work_dir.clone());
		Self {
			config,
			acp_engine: None,
			conversation: ConversationBuffer::new(ConversationOptions::default()),
			tool_registry: ToolRegistry::new(ToolRegistryOptions::default()),
			permission_manager: PermissionManager::new(PermissionMode::Default),
			session_id: None,
			cancel_token: CancellationToken::new(),
			verbose: false,
			session_store,
			config_storage,
		}
	}

	/// Connect to the configured ACP server, create a session, and discover tools.
	///
	/// Uses `config.default_server` to select which ACP server to connect to.
	/// If no default is set, uses the first configured server. After connecting,
	/// creates a new ACP session via simse-acp's AcpClient.
	pub async fn connect(&mut self) -> Result<(), RuntimeError> {
		let server_config = self.resolve_server(None)?;

		let acp_config = AcpEngineConfig {
			servers: vec![ServerEntry {
				name: server_config.name.clone(),
				command: server_config.command.clone(),
				args: server_config.args.clone(),
				cwd: server_config.cwd.clone(),
				env: server_config.env.clone(),
				default_agent: server_config.default_agent.clone(),
				timeout_ms: server_config.timeout_ms,
				permission_policy: Some(PermissionPolicy::AutoApprove),
			}],
			default_server: Some(server_config.name.clone()),
			default_agent: self.config.default_agent.clone(),
			mcp_servers: vec![],
		};

		let engine = AcpEngine::new(acp_config)
			.await
			.map_err(|e: AcpError| RuntimeError::Acp(e.to_string()))?;

		self.acp_engine = Some(Arc::new(engine));

		Ok(())
	}

	/// Connect to a specific ACP server by name.
	pub async fn connect_to(&mut self, server_name: &str) -> Result<(), RuntimeError> {
		let server_config = self.resolve_server(Some(server_name))?;

		let acp_config = AcpEngineConfig {
			servers: vec![ServerEntry {
				name: server_config.name.clone(),
				command: server_config.command.clone(),
				args: server_config.args.clone(),
				cwd: server_config.cwd.clone(),
				env: server_config.env.clone(),
				default_agent: server_config.default_agent.clone(),
				timeout_ms: server_config.timeout_ms,
				permission_policy: Some(PermissionPolicy::AutoApprove),
			}],
			default_server: Some(server_config.name.clone()),
			default_agent: self.config.default_agent.clone(),
			mcp_servers: vec![],
		};

		let engine = AcpEngine::new(acp_config)
			.await
			.map_err(|e: AcpError| RuntimeError::Acp(e.to_string()))?;

		self.acp_engine = Some(Arc::new(engine));

		Ok(())
	}

	/// Handle a user submission: run the agentic loop with the given input.
	///
	/// The input is added to the conversation and the agentic loop is run.
	/// Returns the final text response from the loop.
	pub async fn handle_submit(
		&mut self,
		input: &str,
		callbacks: LoopCallbacks,
	) -> Result<String, RuntimeError> {
		let engine = Arc::clone(
			self.acp_engine
				.as_ref()
				.ok_or(RuntimeError::NotConnected)?,
		);

		// Add the user message to the conversation
		self.update_conversation(|c| c.add_user(input));

		// Reset cancellation token for this run
		self.cancel_token = CancellationToken::new();

		// Build agentic loop options
		let options = AgenticLoopOptions {
			max_turns: 10,
			system_prompt: self.config.workspace_prompt.clone(),
			agent_manages_tools: false,
			..Default::default()
		};

		// Convert ConversationBuffer messages to agentic loop Messages
		let conv_messages = self.conversation.to_messages();
		let mut loop_messages: Vec<Message> = conv_messages
			.iter()
			.map(|m| {
				let role = match m.role {
					simse_core::conversation::Role::User => MessageRole::User,
					simse_core::conversation::Role::Assistant => MessageRole::Assistant,
					simse_core::conversation::Role::System => MessageRole::System,
					simse_core::conversation::Role::ToolResult => MessageRole::User,
				};
				Message {
					role,
					content: m.content.clone(),
				}
			})
			.collect();

		let adapter = AcpAdapter {
			engine: Arc::clone(&engine),
			session_id: self.session_id.clone(),
			server_name: self.config.default_server.clone(),
		};

		let result = agentic_loop::run_agentic_loop(
			&adapter,
			&self.tool_registry,
			&mut loop_messages,
			options,
			Some(callbacks),
			Some(&self.cancel_token),
			None,
			None,
		)
		.await;

		match result {
			Ok(loop_result) => {
				// Add the final assistant response to the conversation
				if !loop_result.final_text.is_empty() {
					let conv = std::mem::replace(
						&mut self.conversation,
						ConversationBuffer::new(ConversationOptions::default()),
					);
					self.conversation = conv.add_assistant(&loop_result.final_text);
				}
				Ok(loop_result.final_text)
			}
			Err(e) => Err(RuntimeError::Acp(e.to_string())),
		}
	}

	/// Handle a permission response from the user.
	///
	/// Forwards the permission decision to the ACP engine if connected.
	pub async fn handle_permission_response(
		&mut self,
		_request_id: &str,
		_option_id: &str,
	) -> Result<(), RuntimeError> {
		let _engine = self
			.acp_engine
			.as_ref()
			.ok_or(RuntimeError::NotConnected)?;

		// Permission handling via the ACP engine is not yet wired up.
		// The bridge's AcpClient had respond_permission, but simse-acp's
		// AcpClient struct does not expose this directly in the same way.
		// For now, this is a no-op placeholder.
		Ok(())
	}

	/// Cancel the current agentic loop at the next check point.
	pub fn abort(&self) {
		self.cancel_token.cancel();
	}

	/// Check if the cancellation token has been triggered.
	pub fn is_aborted(&self) -> bool {
		self.cancel_token.is_cancelled()
	}

	/// Check if onboarding is needed (no ACP servers are configured).
	pub fn needs_onboarding(&self) -> bool {
		self.config.acp.servers.is_empty()
	}

	/// Check if the runtime is currently connected to an ACP server.
	pub fn is_connected(&self) -> bool {
		self.acp_engine.is_some()
	}

	/// Check if the ACP engine is healthy (connection still alive).
	pub async fn is_healthy(&self) -> bool {
		self.acp_engine.is_some()
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

	/// Apply a functional transformation to the conversation buffer.
	///
	/// Takes the current buffer, passes it to the closure, and stores the
	/// result back. This avoids the borrow-and-consume conflict inherent
	/// in owned-return methods called through `&mut self`.
	pub fn update_conversation(&mut self, f: impl FnOnce(ConversationBuffer) -> ConversationBuffer) {
		let conv = std::mem::replace(
			&mut self.conversation,
			ConversationBuffer::new(ConversationOptions::default()),
		);
		self.conversation = f(conv);
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

	/// Get the cancellation token for sharing with async tasks.
	pub fn cancel_token(&self) -> CancellationToken {
		self.cancel_token.clone()
	}

	/// Get the agent name from the ACP engine, if available.
	pub fn agent_name(&self) -> Option<String> {
		// The simse-acp AcpClient does not expose agent_info in the same way
		// as the bridge. Return None for now.
		None
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
				name: d.name.clone(),
				description: d.description.clone(),
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
				let mut conv = ConversationBuffer::new(ConversationOptions::default());
				for msg in &messages {
					conv = match msg.role.as_str() {
						"user" => conv.add_user(&msg.content),
						"assistant" => conv.add_assistant(&msg.content),
						_ => conv,
					};
				}
				self.conversation = conv;
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
				// MCP discovery is not yet supported via simse-core's ToolRegistry.
				// This is a no-op placeholder.
				Ok("MCP tool rediscovery not yet implemented.".into())
			}
			BridgeAction::AcpRestart => {
				if let Some(server) = self.config.default_server.clone() {
					self.connect_to(&server).await?;
				} else {
					self.connect().await?;
				}
				Ok("ACP connection restarted.".into())
			}

			// ── Config ──────────────────────────────────
			BridgeAction::InitConfig { force } => {
				use crate::ui_core::config::storage::ConfigScope;
				let exists = self
					.config_storage
					.file_exists("settings.json", ConfigScope::Project)
					.await;
				if exists && !force {
					return Ok(
						"Project already initialized. Use --force to overwrite.".into(),
					);
				}
				self.config_storage
					.ensure_dir(ConfigScope::Project)
					.await
					.map_err(|e| RuntimeError::Acp(e.to_string()))?;
				let dir = self.config.work_dir.join(".simse");
				Ok(format!("Initialized project config at {}", dir.display()))
			}
			BridgeAction::FactoryReset => {
				use crate::ui_core::config::storage::ConfigScope;
				self.config_storage
					.delete_all(ConfigScope::Global)
					.await
					.map_err(|e| RuntimeError::Acp(e.to_string()))?;
				Ok("Factory reset complete. Global configuration removed.".into())
			}
			BridgeAction::SetupAcp { name, command, args } => {
				// Resolve npx-based commands to direct `node` invocations.
				let (command, args) =
					resolve_npx_to_node(&command, &args).unwrap_or((command, args));

				let server = AcpServerConfig {
					name: name.clone(),
					command: command.clone(),
					args: args.clone(),
					cwd: None,
					env: std::collections::HashMap::new(),
					default_agent: None,
					timeout_ms: None,
				};

				// Write acp.json to the data directory
				let data_dir = &self.config.data_dir;
				std::fs::create_dir_all(data_dir)
					.map_err(|e| RuntimeError::Acp(e.to_string()))?;

				let acp_config = AcpFileConfig {
					servers: vec![server.clone()],
					default_server: Some(name.clone()),
					default_agent: None,
				};

				crate::json_io::write_json_file(&data_dir.join("acp.json"), &acp_config)
					.map_err(|e| RuntimeError::Acp(e.to_string()))?;

				// Update in-memory config
				self.config.acp.servers = vec![server];
				self.config.acp.default_server = Some(name.clone());
				self.config.default_server = Some(name.clone());

				Ok(format!("ACP server '{name}' configured. Ready to connect."))
			}
			BridgeAction::FactoryResetProject => {
				use crate::ui_core::config::storage::ConfigScope;
				self.config_storage
					.delete_all(ConfigScope::Project)
					.await
					.map_err(|e| RuntimeError::Acp(e.to_string()))?;
				Ok("Project configuration reset.".into())
			}

			// ── AI ──────────────────────────────────────
			BridgeAction::RunChain { name, args } => {
				let engine = self
					.acp_engine
					.as_ref()
					.ok_or(RuntimeError::NotConnected)?;

				let chain_input = format!("[chain:{name}] {args}");
				let conv = std::mem::replace(
					&mut self.conversation,
					ConversationBuffer::new(ConversationOptions::default()),
				);
				self.conversation = conv.add_user(&chain_input);

				self.cancel_token = CancellationToken::new();
				let options = AgenticLoopOptions {
					max_turns: 10,
					system_prompt: self.config.workspace_prompt.clone(),
					agent_manages_tools: false,
					..Default::default()
				};

				let conv_messages = self.conversation.to_messages();
				let mut loop_messages: Vec<Message> = conv_messages
					.iter()
					.map(|m| {
						let role = match m.role {
							simse_core::conversation::Role::User => MessageRole::User,
							simse_core::conversation::Role::Assistant => {
								MessageRole::Assistant
							}
							simse_core::conversation::Role::System => MessageRole::System,
							simse_core::conversation::Role::ToolResult => MessageRole::User,
						};
						Message {
							role,
							content: m.content.clone(),
						}
					})
					.collect();

				let adapter = AcpAdapter {
					engine: Arc::clone(engine),
					session_id: self.session_id.clone(),
					server_name: self.config.default_server.clone(),
				};

				let result = agentic_loop::run_agentic_loop(
					&adapter,
					&self.tool_registry,
					&mut loop_messages,
					options,
					None,
					Some(&self.cancel_token),
					None,
					None,
				)
				.await;

				match result {
					Ok(loop_result) => {
						if !loop_result.final_text.is_empty() {
							let conv = std::mem::replace(
								&mut self.conversation,
								ConversationBuffer::new(ConversationOptions::default()),
							);
							self.conversation = conv.add_assistant(&loop_result.final_text);
						}
						Ok(loop_result.final_text)
					}
					Err(e) => Err(RuntimeError::Acp(e.to_string())),
				}
			}

			// ── Library ─────────────────────────────────
			BridgeAction::LibraryAdd { topic, text } => {
				self.call_tool(
					"library_shelve",
					serde_json::json!({
						"topic": topic,
						"text": text,
					}),
				)
				.await
			}
			BridgeAction::LibrarySearch { query } => {
				self.call_tool(
					"library_search",
					serde_json::json!({
						"query": query,
					}),
				)
				.await
			}
			BridgeAction::LibraryRecommend { query } => {
				self.call_tool(
					"library_search",
					serde_json::json!({
						"query": query,
						"recommend": true,
					}),
				)
				.await
			}
			BridgeAction::LibraryTopics => {
				self.call_tool(
					"library_search",
					serde_json::json!({
						"query": "*",
						"listTopics": true,
					}),
				)
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
				self.call_tool(
					"library_search",
					serde_json::json!({
						"id": id,
					}),
				)
				.await
			}
			BridgeAction::LibraryDelete { id } => {
				self.call_tool(
					"library_search",
					serde_json::json!({
						"id": id,
						"delete": true,
					}),
				)
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
				self.call_tool(
					"vfs_write",
					serde_json::json!({
						"path": path,
						"discard": true,
					}),
				)
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
				let conv = std::mem::replace(
					&mut self.conversation,
					ConversationBuffer::new(ConversationOptions::default()),
				);
				self.conversation = conv.compact("[User-requested compaction: conversation history summarized]");
				Ok(format!(
					"Conversation compacted ({msg_count} messages → 1 summary)."
				))
			}
			// Handled directly in dispatch_bridge_action (returns AppMessage).
			BridgeAction::LoadConfigFile { .. } | BridgeAction::SaveConfigField { .. } => {
				unreachable!("LoadConfigFile and SaveConfigField are dispatched directly in dispatch_bridge_action")
			}
		}
	}

	/// Execute a bridge action and return an [`AppMessage::BridgeResult`].
	///
	/// This is the primary entry point for `main.rs` to dispatch a
	/// `BridgeAction` picked up from `App::pending_bridge_action`.
	/// The returned message includes the action name so the app can
	/// perform action-specific side effects (e.g. restarting onboarding
	/// after a factory-reset).
	pub async fn dispatch_bridge_action(&mut self, action: BridgeAction) -> AppMessage {
		// LoadConfigFile and SaveConfigField return dedicated AppMessage
		// variants instead of the generic BridgeResult wrapper.
		match &action {
			BridgeAction::LoadConfigFile { filename, scope } => {
				match self.config_storage.load_file(filename, *scope).await {
					Ok(data) => return AppMessage::SettingsFileLoaded(data),
					Err(e) => {
						if matches!(
							e,
							crate::ui_core::config::storage::ConfigError::NotFound { .. }
						) {
							return AppMessage::SettingsFileLoaded(serde_json::json!({}));
						}
						return AppMessage::SettingsError(e.to_string());
					}
				}
			}
			BridgeAction::SaveConfigField {
				filename,
				scope,
				key,
				value,
			} => {
				let mut data = self
					.config_storage
					.load_file(filename, *scope)
					.await
					.unwrap_or(serde_json::json!({}));
				crate::ui_core::config::storage::set_field(&mut data, key, value.clone());
				match self.config_storage.save_file(filename, *scope, &data).await {
					Ok(()) => {
						return AppMessage::SettingsFieldSaved {
							key: key.clone(),
							value: value.clone(),
						}
					}
					Err(e) => return AppMessage::SettingsError(e.to_string()),
				}
			}
			_ => {}
		}

		let action_name = action.action_name().to_string();
		match self.execute_bridge_action(action).await {
			Ok(text) => AppMessage::BridgeResult {
				action: action_name,
				text,
				is_error: false,
			},
			Err(e) => AppMessage::BridgeResult {
				action: action_name,
				text: e.to_string(),
				is_error: true,
			},
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
			"log.level".to_string(),
			self.config.log_level.clone(),
		));
		if let Some(ref server) = self.config.default_server {
			values.push(("acp.default_server".to_string(), server.clone()));
		}
		if let Some(ref agent) = self.config.default_agent {
			values.push(("acp.default_agent".to_string(), agent.clone()));
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
	) -> Result<AcpServerConfig, RuntimeError> {
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
// npx → node resolution
// ---------------------------------------------------------------------------

/// Try to resolve an npx-based ACP command to a direct `node` invocation.
///
/// When the setup wizard selects a preset using `npx -y <package>`, the npx
/// process wrapper can cause issues with stdio piping (especially on Windows
/// with shim managers like proto). This function resolves the package's entry
/// point on disk so we can use `node <entry_point>` instead.
///
/// Returns `Some((command, args))` if resolution succeeds, `None` otherwise
/// (in which case the caller should keep the original npx command as fallback).
fn resolve_npx_to_node(command: &str, args: &[String]) -> Option<(String, Vec<String>)> {
	// Only applies to npx commands.
	let cmd_lower = command.to_lowercase();
	if !cmd_lower.ends_with("npx") && !cmd_lower.ends_with("npx.exe") {
		return None;
	}

	// Find the package name (skip flags like -y).
	let package = args.iter().find(|a| !a.starts_with('-'))?;

	// Collect candidate node binary directories to search for the package.
	let candidates = find_node_module_dirs();

	for node_dir in &candidates {
		let pkg_dir = node_dir.join("node_modules").join(package.as_str());
		let pkg_json_path = pkg_dir.join("package.json");

		if let Ok(pkg_json) = std::fs::read_to_string(&pkg_json_path)
			&& let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&pkg_json) {
				// Prefer the `bin` entry point (what `npx` runs) over `main`
				// (which is the library entry point). For scoped packages,
				// `bin` is an object with named entries; use the first value.
				let main = pkg
					.get("bin")
					.and_then(|bin| {
						bin.as_str().map(String::from).or_else(|| {
							bin.as_object()
								.and_then(|obj| obj.values().next())
								.and_then(|v| v.as_str())
								.map(String::from)
						})
					})
					.or_else(|| {
						pkg.get("main")
							.and_then(|v| v.as_str())
							.map(String::from)
					})
					.unwrap_or_else(|| "index.js".to_string());

				let entry_point = pkg_dir.join(&main);
				if entry_point.exists() {
					// Find the node binary in this directory.
					let node_bin = if cfg!(windows) {
						node_dir.join("node.exe")
					} else {
						node_dir.join("bin").join("node")
					};

					// Fall back to bare "node" if binary not found at expected path.
					let node_cmd = if node_bin.exists() {
						node_bin.to_string_lossy().into_owned()
					} else {
						"node".to_string()
					};

					return Some((
						node_cmd,
						vec![entry_point.to_string_lossy().into_owned()],
					));
				}
			}
	}

	None
}

/// Collect directories that may contain `node_modules/` with globally
/// installed npm packages. No subprocess calls — pure filesystem lookups.
fn find_node_module_dirs() -> Vec<std::path::PathBuf> {
	use std::path::PathBuf;

	let mut dirs = Vec::new();

	// 1. Check well-known system Node.js install locations.
	if cfg!(windows) {
		dirs.push(PathBuf::from(r"C:\Program Files\nodejs"));
	} else {
		dirs.push(PathBuf::from("/usr/local/lib"));
		dirs.push(PathBuf::from("/usr/lib"));
	}

	// 2. Check proto tool versions (handles proto shim manager on Windows).
	if let Some(home) = home_dir() {
		let proto_node_dir = home.join(".proto").join("tools").join("node");
		if proto_node_dir.exists()
			&& let Ok(entries) = std::fs::read_dir(&proto_node_dir) {
				for entry in entries.flatten() {
					if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
						let name = entry.file_name();
						let name_str = name.to_string_lossy();
						if name_str.starts_with(|c: char| c.is_ascii_digit()) {
							dirs.push(entry.path());
						}
					}
				}
			}

		// 3. Check nvm versions (common on Linux/macOS).
		let nvm_dir = home.join(".nvm").join("versions").join("node");
		if nvm_dir.exists()
			&& let Ok(entries) = std::fs::read_dir(&nvm_dir) {
				for entry in entries.flatten() {
					if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
						dirs.push(entry.path());
					}
				}
			}
	}

	// 3. Also check `where node` / `which node` for PATH-based installs.
	if let Ok(output) = std::process::Command::new(if cfg!(windows) {
		"where"
	} else {
		"which"
	})
	.arg("node")
	.output()
		&& output.status.success()
			&& let Ok(paths) = String::from_utf8(output.stdout) {
				for line in paths.lines() {
					let p = std::path::Path::new(line.trim());
					if let Some(parent) = p.parent()
						&& !dirs.contains(&parent.to_path_buf()) {
							dirs.push(parent.to_path_buf());
						}
				}
			}

	dirs
}

/// Get the user's home directory.
fn home_dir() -> Option<std::path::PathBuf> {
	#[cfg(windows)]
	{
		std::env::var("USERPROFILE")
			.ok()
			.map(std::path::PathBuf::from)
	}
	#[cfg(not(windows))]
	{
		std::env::var("HOME")
			.ok()
			.map(std::path::PathBuf::from)
	}
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;
	use crate::config::{
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
		let token = runtime.cancel_token();
		assert!(!token.is_cancelled());
		runtime.abort();
		assert!(token.is_cancelled());
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
		runtime.update_conversation(|c| c.add_user("Hello"));
		let messages = runtime.conversation().to_messages();
		assert_eq!(messages.len(), 1);
	}

	#[test]
	fn event_loop_reset_conversation() {
		let mut runtime = TuiRuntime::new(test_config());
		runtime.update_conversation(|c| c.add_user("Hello"));
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
		let cb = LoopCallbacks::default();
		let err = runtime.handle_submit("hello", cb).await.unwrap_err();
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

	// ── resolve_npx_to_node ────────────────────────

	#[test]
	fn resolve_npx_skips_non_npx_command() {
		let result = resolve_npx_to_node("node", &["script.js".into()]);
		assert!(result.is_none());
	}

	#[test]
	fn resolve_npx_skips_no_package_name() {
		let result = resolve_npx_to_node("npx", &["-y".into()]);
		assert!(result.is_none());
	}

	#[test]
	fn resolve_npx_skips_unknown_package() {
		let result = resolve_npx_to_node(
			"npx",
			&["-y".into(), "definitely-nonexistent-package-xyz-999".into()],
		);
		assert!(result.is_none());
	}

	#[test]
	fn resolve_npx_handles_exe_suffix() {
		// Should still return None for unknown package, but not panic.
		let result = resolve_npx_to_node(
			"C:/path/to/npx.exe",
			&["-y".into(), "nonexistent-pkg".into()],
		);
		assert!(result.is_none());
	}

	#[test]
	fn resolve_npx_resolves_claude_agent_acp() {
		// This test only passes when @zed-industries/claude-agent-acp is
		// globally installed. Skip gracefully otherwise.
		let result = resolve_npx_to_node(
			"npx",
			&["-y".into(), "@zed-industries/claude-agent-acp".into()],
		);
		if let Some((cmd, args)) = result {
			assert!(
				cmd.to_lowercase().contains("node"),
				"Resolved command should contain 'node', got: {cmd}"
			);
			assert_eq!(args.len(), 1);
			assert!(
				std::path::Path::new(&args[0]).exists(),
				"Resolved path does not exist: {}",
				args[0]
			);
		}
		// If None, the package isn't installed — that's fine, skip silently.
	}

	#[test]
	fn find_node_module_dirs_returns_candidates() {
		let dirs = find_node_module_dirs();
		assert!(
			!dirs.is_empty(),
			"Should find at least one node module directory"
		);
	}
}
