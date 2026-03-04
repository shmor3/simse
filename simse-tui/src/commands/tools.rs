//! Tool commands: `/tools`, `/agents`, `/skills`.

use super::CommandOutput;

/// `/tools [filter]` -- list available tools, optionally filtering by name.
pub fn handle_tools(args: &str) -> Vec<CommandOutput> {
	let filter = args.trim();
	if filter.is_empty() {
		vec![CommandOutput::Info(
			"Would call bridge to list all available tools".into(),
		)]
	} else {
		vec![CommandOutput::Info(format!(
			"Would call bridge to list tools matching \"{filter}\""
		))]
	}
}

/// `/agents` -- list available agents.
pub fn handle_agents(_args: &str) -> Vec<CommandOutput> {
	vec![CommandOutput::Info(
		"Would call bridge to list available agents".into(),
	)]
}

/// `/skills` -- list available skills.
pub fn handle_skills(_args: &str) -> Vec<CommandOutput> {
	vec![CommandOutput::Info(
		"Would call bridge to list available skills".into(),
	)]
}

#[cfg(test)]
mod tests {
	use super::*;

	// ── /tools ───────────────────────────────────────────

	#[test]
	fn tools_no_args_lists_all() {
		let out = handle_tools("");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("all")));
	}

	#[test]
	fn tools_with_filter() {
		let out = handle_tools("read");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("\"read\"")));
	}

	#[test]
	fn tools_trims_whitespace() {
		let out = handle_tools("  write  ");
		assert!(
			matches!(&out[0], CommandOutput::Info(msg) if msg.contains("\"write\""))
		);
	}

	// ── /agents ──────────────────────────────────────────

	#[test]
	fn agents_returns_info() {
		let out = handle_agents("");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("agents")));
	}

	// ── /skills ──────────────────────────────────────────

	#[test]
	fn skills_returns_info() {
		let out = handle_skills("");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("skills")));
	}
}
