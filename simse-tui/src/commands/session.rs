//! Session commands: `/sessions`, `/resume`, `/rename`, `/server`, `/model`,
//! `/mcp`, `/acp`.

use super::CommandOutput;

/// `/sessions` -- list saved sessions.
pub fn handle_sessions(_args: &str) -> Vec<CommandOutput> {
	vec![CommandOutput::Info(
		"Would call bridge to list saved sessions".into(),
	)]
}

/// `/resume <id>` -- resume a saved session.
pub fn handle_resume(args: &str) -> Vec<CommandOutput> {
	let id = args.trim();
	if id.is_empty() {
		return vec![CommandOutput::Error("Usage: /resume <id>".into())];
	}

	vec![CommandOutput::Info(format!(
		"Would call bridge to resume session \"{id}\""
	))]
}

/// `/rename <title>` -- rename the current session.
pub fn handle_rename(args: &str) -> Vec<CommandOutput> {
	let title = args.trim();
	if title.is_empty() {
		return vec![CommandOutput::Error(
			"Usage: /rename <title>".into(),
		)];
	}

	vec![CommandOutput::Info(format!(
		"Would call bridge to rename current session to \"{title}\""
	))]
}

/// `/server [name]` -- show or change the ACP server.
pub fn handle_server(args: &str) -> Vec<CommandOutput> {
	let name = args.trim();
	if name.is_empty() {
		vec![CommandOutput::Info(
			"Would call bridge to show current ACP server".into(),
		)]
	} else {
		vec![CommandOutput::Info(format!(
			"Would call bridge to switch ACP server to \"{name}\""
		))]
	}
}

/// `/model [name]` -- show or change the model.
pub fn handle_model(args: &str) -> Vec<CommandOutput> {
	let name = args.trim();
	if name.is_empty() {
		vec![CommandOutput::Info(
			"Would call bridge to show current model".into(),
		)]
	} else {
		vec![CommandOutput::Info(format!(
			"Would call bridge to switch model to \"{name}\""
		))]
	}
}

/// `/mcp [status|restart]` -- manage MCP connections.
pub fn handle_mcp(args: &str) -> Vec<CommandOutput> {
	let sub = args.trim().to_lowercase();
	match sub.as_str() {
		"" | "status" => vec![CommandOutput::Info(
			"Would call bridge to show MCP connection status".into(),
		)],
		"restart" => vec![CommandOutput::Info(
			"Would call bridge to restart MCP connections".into(),
		)],
		other => vec![CommandOutput::Error(format!(
			"Unknown MCP subcommand: \"{other}\". Use: status, restart"
		))],
	}
}

/// `/acp [status|restart]` -- manage ACP connection.
pub fn handle_acp(args: &str) -> Vec<CommandOutput> {
	let sub = args.trim().to_lowercase();
	match sub.as_str() {
		"" | "status" => vec![CommandOutput::Info(
			"Would call bridge to show ACP connection status".into(),
		)],
		"restart" => vec![CommandOutput::Info(
			"Would call bridge to restart ACP connection".into(),
		)],
		other => vec![CommandOutput::Error(format!(
			"Unknown ACP subcommand: \"{other}\". Use: status, restart"
		))],
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	// ── /sessions ────────────────────────────────────────

	#[test]
	fn sessions_returns_info() {
		let out = handle_sessions("");
		assert_eq!(out.len(), 1);
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("list")));
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
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("sess-42")));
	}

	#[test]
	fn resume_trims_whitespace() {
		let out = handle_resume("  sess-1  ");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("\"sess-1\"")));
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
		assert!(
			matches!(&out[0], CommandOutput::Info(msg) if msg.contains("My Cool Session"))
		);
	}

	// ── /server ──────────────────────────────────────────

	#[test]
	fn server_no_args_shows_current() {
		let out = handle_server("");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("show")));
	}

	#[test]
	fn server_with_name_switches() {
		let out = handle_server("ollama");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("ollama")));
	}

	// ── /model ───────────────────────────────────────────

	#[test]
	fn model_no_args_shows_current() {
		let out = handle_model("");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("show")));
	}

	#[test]
	fn model_with_name_switches() {
		let out = handle_model("gpt-4o");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("gpt-4o")));
	}

	// ── /mcp ─────────────────────────────────────────────

	#[test]
	fn mcp_no_args_shows_status() {
		let out = handle_mcp("");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("status")));
	}

	#[test]
	fn mcp_status() {
		let out = handle_mcp("status");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("status")));
	}

	#[test]
	fn mcp_restart() {
		let out = handle_mcp("restart");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("restart")));
	}

	#[test]
	fn mcp_unknown_subcommand() {
		let out = handle_mcp("frobnicate");
		assert!(matches!(&out[0], CommandOutput::Error(msg) if msg.contains("frobnicate")));
	}

	#[test]
	fn mcp_case_insensitive() {
		let out = handle_mcp("RESTART");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("restart")));
	}

	// ── /acp ─────────────────────────────────────────────

	#[test]
	fn acp_no_args_shows_status() {
		let out = handle_acp("");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("status")));
	}

	#[test]
	fn acp_status() {
		let out = handle_acp("status");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("status")));
	}

	#[test]
	fn acp_restart() {
		let out = handle_acp("restart");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("restart")));
	}

	#[test]
	fn acp_unknown_subcommand() {
		let out = handle_acp("nope");
		assert!(matches!(&out[0], CommandOutput::Error(msg) if msg.contains("nope")));
	}
}
