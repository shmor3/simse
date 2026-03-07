//! Command handlers for the SimSE TUI.
//!
//! Each submodule exposes handler functions that parse command arguments and
//! return `Vec<CommandOutput>`.  Bridge-dependent operations return
//! `BridgeRequest(BridgeAction)` items that the event loop executes async.

pub mod ai;
pub mod config;
pub mod files;
pub mod library;
pub mod meta;
pub mod session;
pub mod tools;

/// The result type returned by every command handler.
#[derive(Debug, Clone, PartialEq)]
pub enum CommandOutput {
	/// A successful result message.
	Success(String),
	/// An error message.
	Error(String),
	/// An informational message (dim gray in the UI).
	Info(String),
	/// Tabular data.
	Table {
		headers: Vec<String>,
		rows: Vec<Vec<String>>,
	},
	/// Request the UI to open an overlay.
	OpenOverlay(OverlayAction),
	/// Request an async bridge operation (dispatched by the event loop).
	BridgeRequest(BridgeAction),
	/// Request a confirmation dialog before executing a bridge action.
	ConfirmAction {
		/// The confirmation message to display.
		message: String,
		/// The action to execute if the user confirms.
		action: BridgeAction,
	},
}

/// Overlay actions that a command can request.
#[derive(Debug, Clone, PartialEq)]
pub enum OverlayAction {
	/// Open the settings explorer overlay.
	Settings,
	/// Open the librarian explorer overlay.
	Librarians,
	/// Open the setup wizard, optionally jumping to a preset.
	Setup(Option<String>),
	/// Open the keyboard shortcuts overlay.
	Shortcuts,
}

/// Async operations that the event loop will dispatch via the bridge.
#[derive(Debug, Clone, PartialEq)]
pub enum BridgeAction {
	// ── Library ──────────────────────────────────────────────────────
	/// Add a memory to the library under the given topic.
	LibraryAdd { topic: String, text: String },
	/// Search the library for matching memories.
	LibrarySearch { query: String },
	/// Get recommendations from the library.
	LibraryRecommend { query: String },
	/// List all topics in the library.
	LibraryTopics,
	/// List volumes, optionally filtered by topic.
	LibraryVolumes { topic: Option<String> },
	/// Get a specific memory by ID.
	LibraryGet { id: String },
	/// Delete a memory by ID.
	LibraryDelete { id: String },

	// ── Session ─────────────────────────────────────────────────────
	/// Resume an existing session by ID.
	ResumeSession { id: String },
	/// Switch the active ACP server.
	SwitchServer { name: String },
	/// Switch the active model.
	SwitchModel { name: String },
	/// Restart all MCP connections.
	McpRestart,
	/// Restart all ACP connections.
	AcpRestart,
	/// Rename the current session.
	RenameSession { title: String },

	// ── Files ───────────────────────────────────────────────────────
	/// List tracked files, optionally under a path.
	ListFiles { path: Option<String> },
	/// Save (flush) files, optionally under a path.
	SaveFiles { path: Option<String> },
	/// Validate files, optionally under a path.
	ValidateFiles { path: Option<String> },
	/// Discard a single tracked file.
	DiscardFile { path: String },
	/// Show diffs for tracked files, optionally under a path.
	DiffFiles { path: Option<String> },

	// ── Config ──────────────────────────────────────────────────────
	/// Initialise the project configuration directory.
	InitConfig { force: bool },
	/// Factory-reset the global configuration.
	FactoryReset,
	/// Factory-reset the project-level configuration.
	FactoryResetProject,
	/// Apply a setup preset — writes `acp.json` and updates in-memory config.
	SetupAcp {
		name: String,
		command: String,
		args: Vec<String>,
	},
	/// Load a config file for the settings UI.
	LoadConfigFile {
		filename: String,
		scope: crate::ui_core::config::storage::ConfigScope,
	},
	/// Save a field in a config file from the settings UI.
	SaveConfigField {
		filename: String,
		scope: crate::ui_core::config::storage::ConfigScope,
		key: String,
		value: serde_json::Value,
	},

	// ── AI ───────────────────────────────────────────────────────────
	/// Run a named chain with the given arguments.
	RunChain { name: String, args: String },

	// ── Meta ────────────────────────────────────────────────────────
	/// Compact the conversation history.
	Compact,
}

impl BridgeAction {
	/// Return a kebab-case identifier for this action, used to tag `BridgeResult` messages.
	pub fn action_name(&self) -> &'static str {
		match self {
			BridgeAction::LibraryAdd { .. } => "library-add",
			BridgeAction::LibrarySearch { .. } => "library-search",
			BridgeAction::LibraryRecommend { .. } => "library-recommend",
			BridgeAction::LibraryTopics => "library-topics",
			BridgeAction::LibraryVolumes { .. } => "library-volumes",
			BridgeAction::LibraryGet { .. } => "library-get",
			BridgeAction::LibraryDelete { .. } => "library-delete",
			BridgeAction::ResumeSession { .. } => "resume-session",
			BridgeAction::SwitchServer { .. } => "switch-server",
			BridgeAction::SwitchModel { .. } => "switch-model",
			BridgeAction::McpRestart => "mcp-restart",
			BridgeAction::AcpRestart => "acp-restart",
			BridgeAction::RenameSession { .. } => "rename-session",
			BridgeAction::ListFiles { .. } => "list-files",
			BridgeAction::SaveFiles { .. } => "save-files",
			BridgeAction::ValidateFiles { .. } => "validate-files",
			BridgeAction::DiscardFile { .. } => "discard-file",
			BridgeAction::DiffFiles { .. } => "diff-files",
			BridgeAction::InitConfig { .. } => "init-config",
			BridgeAction::FactoryReset => "factory-reset",
			BridgeAction::FactoryResetProject => "factory-reset-project",
			BridgeAction::SetupAcp { .. } => "setup-acp",
			BridgeAction::LoadConfigFile { .. } => "load-config-file",
			BridgeAction::SaveConfigField { .. } => "save-config-field",
			BridgeAction::RunChain { .. } => "run-chain",
			BridgeAction::Compact => "compact",
		}
	}
}

// ── Supporting info types ────────────────────────────────────────────────

/// Lightweight session descriptor exposed to command handlers.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SessionInfo {
	pub id: String,
	pub title: String,
	pub created_at: String,
	pub updated_at: String,
	pub message_count: usize,
	pub work_dir: String,
}

/// Simplified tool definition (no dependency on crate::ui_core::tools).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ToolDefInfo {
	pub name: String,
	pub description: String,
}

/// Agent descriptor.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AgentInfo {
	pub name: String,
	pub description: Option<String>,
}

/// Skill descriptor.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SkillInfo {
	pub name: String,
	pub description: Option<String>,
}

/// Prompt/chain descriptor.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PromptInfo {
	pub name: String,
	pub description: Option<String>,
	pub step_count: usize,
}

/// Read-only snapshot of runtime state available to sync command handlers.
#[derive(Debug, Clone, Default)]
pub struct CommandContext {
	pub sessions: Vec<SessionInfo>,
	pub tool_defs: Vec<ToolDefInfo>,
	pub agents: Vec<AgentInfo>,
	pub skills: Vec<SkillInfo>,
	pub prompts: Vec<PromptInfo>,
	pub server_name: Option<String>,
	pub model_name: Option<String>,
	pub session_id: Option<String>,
	pub acp_connected: bool,
	pub data_dir: Option<String>,
	pub work_dir: Option<String>,
	pub config_values: Vec<(String, String)>,
}

/// Format a `CommandOutput::Table` as a fixed-width plain-text table.
pub fn format_table(headers: &[String], rows: &[Vec<String>]) -> String {
	if headers.is_empty() {
		return String::new();
	}

	// Determine column widths.
	let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
	for row in rows {
		for (i, cell) in row.iter().enumerate() {
			if i < widths.len() {
				widths[i] = widths[i].max(cell.len());
			}
		}
	}

	let mut out = String::new();

	// Header row.
	for (i, h) in headers.iter().enumerate() {
		if i > 0 {
			out.push_str("  ");
		}
		out.push_str(&format!("{:<width$}", h, width = widths[i]));
	}
	out.push('\n');

	// Separator.
	for (i, w) in widths.iter().enumerate() {
		if i > 0 {
			out.push_str("  ");
		}
		out.push_str(&"-".repeat(*w));
	}
	out.push('\n');

	// Data rows.
	for row in rows {
		for (i, cell) in row.iter().enumerate() {
			if i > 0 {
				out.push_str("  ");
			}
			let w = widths.get(i).copied().unwrap_or(0);
			out.push_str(&format!("{:<width$}", cell, width = w));
		}
		out.push('\n');
	}

	out
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn format_table_empty_headers() {
		assert_eq!(format_table(&[], &[]), "");
	}

	#[test]
	fn format_table_basic() {
		let headers = vec!["Name".into(), "Value".into()];
		let rows = vec![
			vec!["foo".into(), "1".into()],
			vec!["barbaz".into(), "2".into()],
		];
		let table = format_table(&headers, &rows);
		assert!(table.contains("Name"));
		assert!(table.contains("barbaz"));
		assert!(table.contains("---"));
	}

	#[test]
	fn overlay_action_variants() {
		let a = OverlayAction::Settings;
		let b = OverlayAction::Librarians;
		let c = OverlayAction::Setup(Some("ollama".into()));
		let d = OverlayAction::Shortcuts;
		// Ensure Debug works and equality checks pass.
		assert_ne!(a, b);
		assert_ne!(c, d);
		assert_eq!(a, OverlayAction::Settings);
	}

	#[test]
	fn command_output_variants() {
		let s = CommandOutput::Success("ok".into());
		let e = CommandOutput::Error("fail".into());
		let i = CommandOutput::Info("note".into());
		assert_ne!(s, e);
		assert_ne!(e, i);
	}

	#[test]
	fn bridge_action_debug() {
		let action = BridgeAction::LibrarySearch {
			query: "test".into(),
		};
		let dbg = format!("{:?}", action);
		assert!(dbg.contains("LibrarySearch"));
		assert!(dbg.contains("test"));
	}

	#[test]
	fn bridge_action_clone_eq() {
		let a = BridgeAction::LibraryAdd {
			topic: "rust".into(),
			text: "hello".into(),
		};
		let b = a.clone();
		assert_eq!(a, b);

		let c = BridgeAction::McpRestart;
		let d = BridgeAction::AcpRestart;
		assert_ne!(c, d);
	}

	#[test]
	fn command_context_default() {
		let ctx = CommandContext::default();
		assert!(ctx.sessions.is_empty());
		assert!(ctx.tool_defs.is_empty());
		assert!(ctx.agents.is_empty());
		assert!(ctx.skills.is_empty());
		assert!(ctx.prompts.is_empty());
		assert!(ctx.server_name.is_none());
		assert!(ctx.model_name.is_none());
		assert!(ctx.session_id.is_none());
		assert!(!ctx.acp_connected);
		assert!(ctx.data_dir.is_none());
		assert!(ctx.work_dir.is_none());
		assert!(ctx.config_values.is_empty());
	}

	#[test]
	fn command_output_bridge_request() {
		let output = CommandOutput::BridgeRequest(BridgeAction::LibraryTopics);
		match &output {
			CommandOutput::BridgeRequest(BridgeAction::LibraryTopics) => {}
			other => panic!("expected BridgeRequest(LibraryTopics), got {:?}", other),
		}
	}

	#[test]
	fn session_info_default() {
		let info = SessionInfo::default();
		assert_eq!(info.id, "");
		assert_eq!(info.title, "");
		assert_eq!(info.created_at, "");
		assert_eq!(info.updated_at, "");
		assert_eq!(info.message_count, 0);
		assert_eq!(info.work_dir, "");
	}

	#[test]
	fn tool_def_info_default() {
		let info = ToolDefInfo::default();
		assert_eq!(info.name, "");
		assert_eq!(info.description, "");
	}

	#[test]
	fn command_output_confirm_action() {
		let output = CommandOutput::ConfirmAction {
			message: "Are you sure?".into(),
			action: BridgeAction::FactoryReset,
		};
		match &output {
			CommandOutput::ConfirmAction { message, action } => {
				assert_eq!(message, "Are you sure?");
				assert_eq!(action, &BridgeAction::FactoryReset);
			}
			other => panic!("expected ConfirmAction, got {:?}", other),
		}
	}

	#[test]
	fn bridge_action_name_factory_reset() {
		assert_eq!(BridgeAction::FactoryReset.action_name(), "factory-reset");
	}

	#[test]
	fn bridge_action_name_factory_reset_project() {
		assert_eq!(BridgeAction::FactoryResetProject.action_name(), "factory-reset-project");
	}

	#[test]
	fn bridge_action_name_all_variants() {
		// Ensure every variant returns a non-empty string.
		let actions: Vec<BridgeAction> = vec![
			BridgeAction::LibraryAdd { topic: "t".into(), text: "x".into() },
			BridgeAction::LibrarySearch { query: "q".into() },
			BridgeAction::LibraryRecommend { query: "q".into() },
			BridgeAction::LibraryTopics,
			BridgeAction::LibraryVolumes { topic: None },
			BridgeAction::LibraryGet { id: "1".into() },
			BridgeAction::LibraryDelete { id: "1".into() },
			BridgeAction::ResumeSession { id: "s".into() },
			BridgeAction::SwitchServer { name: "n".into() },
			BridgeAction::SwitchModel { name: "m".into() },
			BridgeAction::McpRestart,
			BridgeAction::AcpRestart,
			BridgeAction::RenameSession { title: "t".into() },
			BridgeAction::ListFiles { path: None },
			BridgeAction::SaveFiles { path: None },
			BridgeAction::ValidateFiles { path: None },
			BridgeAction::DiscardFile { path: "p".into() },
			BridgeAction::DiffFiles { path: None },
			BridgeAction::InitConfig { force: false },
			BridgeAction::FactoryReset,
			BridgeAction::FactoryResetProject,
			BridgeAction::SetupAcp { name: "n".into(), command: "c".into(), args: vec![] },
			BridgeAction::LoadConfigFile { filename: "config.json".into(), scope: crate::ui_core::config::storage::ConfigScope::Global },
			BridgeAction::SaveConfigField { filename: "config.json".into(), scope: crate::ui_core::config::storage::ConfigScope::Global, key: "k".into(), value: serde_json::json!("v") },
			BridgeAction::RunChain { name: "c".into(), args: "a".into() },
			BridgeAction::Compact,
		];
		for action in actions {
			let name = action.action_name();
			assert!(!name.is_empty(), "action_name() should not be empty for {:?}", action);
			assert!(name.contains('-') || name.len() > 3, "action_name() should be kebab-case: {}", name);
		}
	}
}
