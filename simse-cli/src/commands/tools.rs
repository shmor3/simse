//! Tool commands: `/tools`, `/agents`, `/skills`.

use super::{CommandContext, CommandOutput};

/// `/tools [filter]` -- list available tools, optionally filtering by name.
pub fn handle_tools(args: &str, ctx: &CommandContext) -> Vec<CommandOutput> {
	if ctx.tool_defs.is_empty() {
		return vec![CommandOutput::Info("No tools registered. Connect to an ACP server with /setup to get started.".into())];
	}

	let filter = args.trim();
	if filter.is_empty() {
		let rows: Vec<Vec<String>> = ctx
			.tool_defs
			.iter()
			.map(|t| vec![t.name.clone(), t.description.clone()])
			.collect();
		return vec![CommandOutput::Table {
			headers: vec!["Name".into(), "Description".into()],
			rows,
		}];
	}

	let filter_lower = filter.to_lowercase();
	let rows: Vec<Vec<String>> = ctx
		.tool_defs
		.iter()
		.filter(|t| t.name.to_lowercase().contains(&filter_lower))
		.map(|t| vec![t.name.clone(), t.description.clone()])
		.collect();

	if rows.is_empty() {
		return vec![CommandOutput::Info(format!(
			"No tools matching \"{filter}\"."
		))];
	}

	vec![CommandOutput::Table {
		headers: vec!["Name".into(), "Description".into()],
		rows,
	}]
}

/// `/agents` -- list available agents.
pub fn handle_agents(_args: &str, ctx: &CommandContext) -> Vec<CommandOutput> {
	if ctx.agents.is_empty() {
		return vec![CommandOutput::Info("No agents configured. Add agent files to .simse/agents/ to define custom agents.".into())];
	}

	let rows: Vec<Vec<String>> = ctx
		.agents
		.iter()
		.map(|a| {
			vec![
				a.name.clone(),
				a.description.clone().unwrap_or_default(),
			]
		})
		.collect();

	vec![CommandOutput::Table {
		headers: vec!["Name".into(), "Description".into()],
		rows,
	}]
}

/// `/skills` -- list available skills.
pub fn handle_skills(_args: &str, ctx: &CommandContext) -> Vec<CommandOutput> {
	if ctx.skills.is_empty() {
		return vec![CommandOutput::Info("No skills configured. Add skills to .simse/skills/ to extend functionality.".into())];
	}

	let rows: Vec<Vec<String>> = ctx
		.skills
		.iter()
		.map(|s| {
			vec![
				s.name.clone(),
				s.description.clone().unwrap_or_default(),
			]
		})
		.collect();

	vec![CommandOutput::Table {
		headers: vec!["Name".into(), "Description".into()],
		rows,
	}]
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::commands::{AgentInfo, CommandContext, SkillInfo, ToolDefInfo};

	fn empty_ctx() -> CommandContext {
		CommandContext::default()
	}

	fn tools_ctx() -> CommandContext {
		CommandContext {
			tool_defs: vec![
				ToolDefInfo {
					name: "read_file".into(),
					description: "Read a file from disk".into(),
				},
				ToolDefInfo {
					name: "write_file".into(),
					description: "Write a file to disk".into(),
				},
				ToolDefInfo {
					name: "search".into(),
					description: "Search for text in files".into(),
				},
			],
			..Default::default()
		}
	}

	fn agents_ctx() -> CommandContext {
		CommandContext {
			agents: vec![
				AgentInfo {
					name: "coder".into(),
					description: Some("Code generation agent".into()),
				},
				AgentInfo {
					name: "reviewer".into(),
					description: None,
				},
			],
			..Default::default()
		}
	}

	fn skills_ctx() -> CommandContext {
		CommandContext {
			skills: vec![
				SkillInfo {
					name: "commit".into(),
					description: Some("Create a git commit".into()),
				},
				SkillInfo {
					name: "review-pr".into(),
					description: None,
				},
			],
			..Default::default()
		}
	}

	// ── /tools ───────────────────────────────────────────

	#[test]
	fn tools_empty_returns_info() {
		let out = handle_tools("", &empty_ctx());
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg == "No tools registered. Connect to an ACP server with /setup to get started."));
	}

	#[test]
	fn tools_no_args_lists_all() {
		let ctx = tools_ctx();
		let out = handle_tools("", &ctx);
		match &out[0] {
			CommandOutput::Table { headers, rows } => {
				assert_eq!(headers, &["Name", "Description"]);
				assert_eq!(rows.len(), 3);
			}
			other => panic!("expected Table, got {:?}", other),
		}
	}

	#[test]
	fn tools_with_filter() {
		let ctx = tools_ctx();
		let out = handle_tools("read", &ctx);
		match &out[0] {
			CommandOutput::Table { rows, .. } => {
				assert_eq!(rows.len(), 1);
				assert_eq!(rows[0][0], "read_file");
			}
			other => panic!("expected Table, got {:?}", other),
		}
	}

	#[test]
	fn tools_filter_case_insensitive() {
		let ctx = tools_ctx();
		let out = handle_tools("READ", &ctx);
		match &out[0] {
			CommandOutput::Table { rows, .. } => {
				assert_eq!(rows.len(), 1);
				assert_eq!(rows[0][0], "read_file");
			}
			other => panic!("expected Table, got {:?}", other),
		}
	}

	#[test]
	fn tools_filter_no_match() {
		let ctx = tools_ctx();
		let out = handle_tools("nonexistent", &ctx);
		assert!(
			matches!(&out[0], CommandOutput::Info(msg) if msg.contains("No tools matching") && msg.contains("nonexistent"))
		);
	}

	#[test]
	fn tools_trims_whitespace() {
		let ctx = tools_ctx();
		let out = handle_tools("  write  ", &ctx);
		match &out[0] {
			CommandOutput::Table { rows, .. } => {
				assert_eq!(rows.len(), 1);
				assert_eq!(rows[0][0], "write_file");
			}
			other => panic!("expected Table, got {:?}", other),
		}
	}

	#[test]
	fn tools_filter_matches_multiple() {
		let ctx = tools_ctx();
		let out = handle_tools("file", &ctx);
		match &out[0] {
			CommandOutput::Table { rows, .. } => {
				assert_eq!(rows.len(), 2);
			}
			other => panic!("expected Table, got {:?}", other),
		}
	}

	// ── /agents ──────────────────────────────────────────

	#[test]
	fn agents_empty_returns_info() {
		let out = handle_agents("", &empty_ctx());
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg == "No agents configured. Add agent files to .simse/agents/ to define custom agents."));
	}

	#[test]
	fn agents_returns_table() {
		let ctx = agents_ctx();
		let out = handle_agents("", &ctx);
		match &out[0] {
			CommandOutput::Table { headers, rows } => {
				assert_eq!(headers, &["Name", "Description"]);
				assert_eq!(rows.len(), 2);
				assert_eq!(rows[0][0], "coder");
				assert_eq!(rows[0][1], "Code generation agent");
				assert_eq!(rows[1][0], "reviewer");
				assert_eq!(rows[1][1], "");
			}
			other => panic!("expected Table, got {:?}", other),
		}
	}

	// ── /skills ──────────────────────────────────────────

	#[test]
	fn skills_empty_returns_info() {
		let out = handle_skills("", &empty_ctx());
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg == "No skills configured. Add skills to .simse/skills/ to extend functionality."));
	}

	#[test]
	fn skills_returns_table() {
		let ctx = skills_ctx();
		let out = handle_skills("", &ctx);
		match &out[0] {
			CommandOutput::Table { headers, rows } => {
				assert_eq!(headers, &["Name", "Description"]);
				assert_eq!(rows.len(), 2);
				assert_eq!(rows[0][0], "commit");
				assert_eq!(rows[0][1], "Create a git commit");
				assert_eq!(rows[1][0], "review-pr");
				assert_eq!(rows[1][1], "");
			}
			other => panic!("expected Table, got {:?}", other),
		}
	}
}
