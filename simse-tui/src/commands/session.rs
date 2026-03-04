//! Session commands: `/sessions`, `/resume`, `/rename`, `/server`, `/model`,
//! `/mcp`, `/acp`.

use super::{BridgeAction, CommandContext, CommandOutput};

/// `/sessions` -- list saved sessions.
pub fn handle_sessions(_args: &str, ctx: &CommandContext) -> Vec<CommandOutput> {
	if ctx.sessions.is_empty() {
		return vec![CommandOutput::Info("No saved sessions.".into())];
	}

	let headers = vec![
		"ID".into(),
		"Title".into(),
		"Messages".into(),
		"Updated".into(),
		"Work Dir".into(),
	];

	let rows: Vec<Vec<String>> = ctx
		.sessions
		.iter()
		.map(|s| {
			vec![
				s.id.clone(),
				s.title.clone(),
				s.message_count.to_string(),
				s.updated_at.clone(),
				s.work_dir.clone(),
			]
		})
		.collect();

	vec![CommandOutput::Table { headers, rows }]
}

/// `/resume <id>` -- resume a saved session.
pub fn handle_resume(args: &str) -> Vec<CommandOutput> {
	let id = args.trim();
	if id.is_empty() {
		return vec![CommandOutput::Error("Usage: /resume <id>".into())];
	}

	vec![CommandOutput::BridgeRequest(BridgeAction::ResumeSession {
		id: id.to_string(),
	})]
}

/// `/rename <title>` -- rename the current session.
pub fn handle_rename(args: &str) -> Vec<CommandOutput> {
	let title = args.trim();
	if title.is_empty() {
		return vec![CommandOutput::Error("Usage: /rename <title>".into())];
	}

	vec![CommandOutput::BridgeRequest(BridgeAction::RenameSession {
		title: title.to_string(),
	})]
}

/// `/server [name]` -- show or change the ACP server.
pub fn handle_server(args: &str, ctx: &CommandContext) -> Vec<CommandOutput> {
	let name = args.trim();
	if name.is_empty() {
		return match &ctx.server_name {
			Some(server) => vec![CommandOutput::Success(format!(
				"Current server: {server}"
			))],
			None => vec![CommandOutput::Info(
				"No ACP server configured.".into(),
			)],
		};
	}

	vec![CommandOutput::BridgeRequest(BridgeAction::SwitchServer {
		name: name.to_string(),
	})]
}

/// `/model [name]` -- show or change the model.
pub fn handle_model(args: &str, ctx: &CommandContext) -> Vec<CommandOutput> {
	let name = args.trim();
	if name.is_empty() {
		return match &ctx.model_name {
			Some(model) => vec![CommandOutput::Success(format!(
				"Current model: {model}"
			))],
			None => vec![CommandOutput::Info("No model configured.".into())],
		};
	}

	vec![CommandOutput::BridgeRequest(BridgeAction::SwitchModel {
		name: name.to_string(),
	})]
}

/// `/mcp [status|restart]` -- manage MCP connections.
pub fn handle_mcp(args: &str, ctx: &CommandContext) -> Vec<CommandOutput> {
	let sub = args.trim().to_lowercase();
	match sub.as_str() {
		"" | "status" => {
			let server = ctx
				.server_name
				.as_deref()
				.unwrap_or("none");
			let status = if ctx.acp_connected {
				"connected"
			} else {
				"disconnected"
			};
			vec![CommandOutput::Success(format!(
				"MCP status: server={server}, status={status}"
			))]
		}
		"restart" => vec![CommandOutput::BridgeRequest(BridgeAction::McpRestart)],
		other => vec![CommandOutput::Error(format!(
			"Unknown MCP subcommand: \"{other}\". Use: status, restart"
		))],
	}
}

/// `/acp [status|restart]` -- manage ACP connection.
pub fn handle_acp(args: &str, ctx: &CommandContext) -> Vec<CommandOutput> {
	let sub = args.trim().to_lowercase();
	match sub.as_str() {
		"" | "status" => {
			let status = if ctx.acp_connected {
				"connected"
			} else {
				"disconnected"
			};
			vec![CommandOutput::Success(format!(
				"ACP status: {status}"
			))]
		}
		"restart" => vec![CommandOutput::BridgeRequest(BridgeAction::AcpRestart)],
		other => vec![CommandOutput::Error(format!(
			"Unknown ACP subcommand: \"{other}\". Use: status, restart"
		))],
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::commands::SessionInfo;

	fn empty_ctx() -> CommandContext {
		CommandContext::default()
	}

	fn ctx_with_sessions() -> CommandContext {
		CommandContext {
			sessions: vec![
				SessionInfo {
					id: "s-1".into(),
					title: "First".into(),
					created_at: "2026-01-01".into(),
					updated_at: "2026-01-02".into(),
					message_count: 10,
					work_dir: "/home/user/proj".into(),
				},
				SessionInfo {
					id: "s-2".into(),
					title: "Second".into(),
					created_at: "2026-02-01".into(),
					updated_at: "2026-02-05".into(),
					message_count: 3,
					work_dir: "/tmp".into(),
				},
			],
			..Default::default()
		}
	}

	// ── /sessions ────────────────────────────────────────

	#[test]
	fn sessions_empty_returns_info() {
		let out = handle_sessions("", &empty_ctx());
		assert_eq!(out.len(), 1);
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg == "No saved sessions."));
	}

	#[test]
	fn sessions_with_data_returns_table() {
		let out = handle_sessions("", &ctx_with_sessions());
		assert_eq!(out.len(), 1);
		match &out[0] {
			CommandOutput::Table { headers, rows } => {
				assert_eq!(headers, &["ID", "Title", "Messages", "Updated", "Work Dir"]);
				assert_eq!(rows.len(), 2);
				assert_eq!(rows[0][0], "s-1");
				assert_eq!(rows[0][1], "First");
				assert_eq!(rows[0][2], "10");
				assert_eq!(rows[0][3], "2026-01-02");
				assert_eq!(rows[0][4], "/home/user/proj");
				assert_eq!(rows[1][0], "s-2");
				assert_eq!(rows[1][2], "3");
			}
			other => panic!("expected Table, got {:?}", other),
		}
	}

	#[test]
	fn sessions_table_has_five_columns() {
		let out = handle_sessions("", &ctx_with_sessions());
		match &out[0] {
			CommandOutput::Table { headers, rows } => {
				assert_eq!(headers.len(), 5);
				for row in rows {
					assert_eq!(row.len(), 5);
				}
			}
			other => panic!("expected Table, got {:?}", other),
		}
	}

	// ── /resume ──────────────────────────────────────────

	#[test]
	fn resume_empty_is_error() {
		let out = handle_resume("");
		assert!(matches!(&out[0], CommandOutput::Error(_)));
	}

	#[test]
	fn resume_valid() {
		let out = handle_resume("sess-42");
		assert!(matches!(
			&out[0],
			CommandOutput::BridgeRequest(BridgeAction::ResumeSession { id }) if id == "sess-42"
		));
	}

	#[test]
	fn resume_trims_whitespace() {
		let out = handle_resume("  sess-1  ");
		assert!(matches!(
			&out[0],
			CommandOutput::BridgeRequest(BridgeAction::ResumeSession { id }) if id == "sess-1"
		));
	}

	// ── /rename ──────────────────────────────────────────

	#[test]
	fn rename_empty_is_error() {
		let out = handle_rename("");
		assert!(matches!(&out[0], CommandOutput::Error(_)));
	}

	#[test]
	fn rename_valid() {
		let out = handle_rename("My Cool Session");
		assert!(matches!(
			&out[0],
			CommandOutput::BridgeRequest(BridgeAction::RenameSession { title }) if title == "My Cool Session"
		));
	}

	// ── /server ──────────────────────────────────────────

	#[test]
	fn server_no_args_no_server_configured() {
		let out = handle_server("", &empty_ctx());
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg == "No ACP server configured."));
	}

	#[test]
	fn server_no_args_shows_current() {
		let ctx = CommandContext {
			server_name: Some("ollama".into()),
			..Default::default()
		};
		let out = handle_server("", &ctx);
		assert!(matches!(&out[0], CommandOutput::Success(msg) if msg.contains("ollama")));
	}

	#[test]
	fn server_with_name_switches() {
		let out = handle_server("ollama", &empty_ctx());
		assert!(matches!(
			&out[0],
			CommandOutput::BridgeRequest(BridgeAction::SwitchServer { name }) if name == "ollama"
		));
	}

	// ── /model ───────────────────────────────────────────

	#[test]
	fn model_no_args_no_model_configured() {
		let out = handle_model("", &empty_ctx());
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg == "No model configured."));
	}

	#[test]
	fn model_no_args_shows_current() {
		let ctx = CommandContext {
			model_name: Some("gpt-4o".into()),
			..Default::default()
		};
		let out = handle_model("", &ctx);
		assert!(matches!(&out[0], CommandOutput::Success(msg) if msg.contains("gpt-4o")));
	}

	#[test]
	fn model_with_name_switches() {
		let out = handle_model("gpt-4o", &empty_ctx());
		assert!(matches!(
			&out[0],
			CommandOutput::BridgeRequest(BridgeAction::SwitchModel { name }) if name == "gpt-4o"
		));
	}

	// ── /mcp ─────────────────────────────────────────────

	#[test]
	fn mcp_no_args_shows_status() {
		let out = handle_mcp("", &empty_ctx());
		assert!(matches!(&out[0], CommandOutput::Success(msg) if msg.contains("status")));
	}

	#[test]
	fn mcp_status() {
		let ctx = CommandContext {
			server_name: Some("anthropic".into()),
			acp_connected: true,
			..Default::default()
		};
		let out = handle_mcp("status", &ctx);
		assert!(matches!(
			&out[0],
			CommandOutput::Success(msg) if msg.contains("anthropic") && msg.contains("connected")
		));
	}

	#[test]
	fn mcp_restart() {
		let out = handle_mcp("restart", &empty_ctx());
		assert!(matches!(
			&out[0],
			CommandOutput::BridgeRequest(BridgeAction::McpRestart)
		));
	}

	#[test]
	fn mcp_unknown_subcommand() {
		let out = handle_mcp("frobnicate", &empty_ctx());
		assert!(matches!(&out[0], CommandOutput::Error(msg) if msg.contains("frobnicate")));
	}

	#[test]
	fn mcp_case_insensitive() {
		let out = handle_mcp("RESTART", &empty_ctx());
		assert!(matches!(
			&out[0],
			CommandOutput::BridgeRequest(BridgeAction::McpRestart)
		));
	}

	// ── /acp ─────────────────────────────────────────────

	#[test]
	fn acp_no_args_shows_status() {
		let out = handle_acp("", &empty_ctx());
		assert!(matches!(&out[0], CommandOutput::Success(msg) if msg.contains("disconnected")));
	}

	#[test]
	fn acp_status_connected() {
		let ctx = CommandContext {
			acp_connected: true,
			..Default::default()
		};
		let out = handle_acp("status", &ctx);
		assert!(matches!(&out[0], CommandOutput::Success(msg) if msg.contains("connected")));
	}

	#[test]
	fn acp_restart() {
		let out = handle_acp("restart", &empty_ctx());
		assert!(matches!(
			&out[0],
			CommandOutput::BridgeRequest(BridgeAction::AcpRestart)
		));
	}

	#[test]
	fn acp_unknown_subcommand() {
		let out = handle_acp("nope", &empty_ctx());
		assert!(matches!(&out[0], CommandOutput::Error(msg) if msg.contains("nope")));
	}
}
