//! Command registry: lookup by name/alias, categorization.

use serde::{Deserialize, Serialize};

/// Command category.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandCategory {
	Meta,
	Library,
	Tools,
	Session,
	Config,
	Files,
	Ai,
}

/// A command definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandDefinition {
	pub name: String,
	pub description: String,
	pub usage: String,
	pub aliases: Vec<String>,
	pub category: CommandCategory,
	pub hidden: bool,
}

/// Look up a command by name or alias from a list of definitions.
pub fn find_command<'a>(
	commands: &'a [CommandDefinition],
	input: &str,
) -> Option<&'a CommandDefinition> {
	let query = input.to_lowercase();
	commands.iter().find(|cmd| {
		cmd.name.to_lowercase() == query
			|| cmd.aliases.iter().any(|a| a.to_lowercase() == query)
	})
}

/// Filter commands matching a prefix (for autocomplete).
pub fn filter_commands<'a>(
	commands: &'a [CommandDefinition],
	prefix: &str,
) -> Vec<&'a CommandDefinition> {
	let prefix = prefix.to_lowercase();
	commands
		.iter()
		.filter(|cmd| {
			cmd.name.to_lowercase().contains(&prefix)
				|| cmd
					.aliases
					.iter()
					.any(|a| a.to_lowercase().contains(&prefix))
		})
		.collect()
}

/// Parse "on"/"off"/"true"/"false"/"1"/"0" or empty string (toggle).
pub fn parse_bool_arg(arg: &str, current: bool) -> Option<bool> {
	match arg.trim().to_lowercase().as_str() {
		"" => Some(!current),
		"on" | "true" | "1" => Some(true),
		"off" | "false" | "0" => Some(false),
		_ => None,
	}
}

/// Helper to build a `CommandDefinition` with less boilerplate.
fn cmd(
	name: &str,
	desc: &str,
	usage: &str,
	aliases: &[&str],
	category: CommandCategory,
) -> CommandDefinition {
	CommandDefinition {
		name: name.into(),
		description: desc.into(),
		usage: usage.into(),
		aliases: aliases.iter().map(|a| (*a).into()).collect(),
		category,
		hidden: false,
	}
}

/// Return all built-in command definitions.
pub fn all_commands() -> Vec<CommandDefinition> {
	use CommandCategory::*;

	vec![
		// ── Meta (7) ──────────────────────────────────────────
		cmd("help", "Show help information", "help [command]", &["?"], Meta),
		cmd("clear", "Clear the screen", "clear", &[], Meta),
		cmd("verbose", "Toggle verbose output", "verbose [on|off]", &["v"], Meta),
		cmd("plan", "Toggle plan mode", "plan [on|off]", &[], Meta),
		cmd("context", "Show current context usage", "context", &[], Meta),
		cmd("compact", "Compact conversation history", "compact", &[], Meta),
		cmd("exit", "Exit the application", "exit", &["quit", "q"], Meta),
		// ── Library (7) ───────────────────────────────────────
		cmd("add", "Add a volume to the library", "add <text>", &[], Library),
		cmd("search", "Search the library", "search <query>", &["s"], Library),
		cmd("recommend", "Get library recommendations", "recommend [topic]", &["rec"], Library),
		cmd("topics", "List library topics", "topics", &[], Library),
		cmd("volumes", "List library volumes", "volumes [topic]", &["ls"], Library),
		cmd("get", "Get a library volume by ID", "get <id>", &[], Library),
		cmd("delete", "Delete a library volume", "delete <id>", &["rm"], Library),
		// ── Tools (3) ─────────────────────────────────────────
		cmd("tools", "List available tools", "tools [filter]", &[], Tools),
		cmd("agents", "List available agents", "agents", &[], Tools),
		cmd("skills", "List available skills", "skills", &[], Tools),
		// ── Session (7) ───────────────────────────────────────
		cmd("sessions", "List saved sessions", "sessions", &[], Session),
		cmd("resume", "Resume a saved session", "resume <id>", &["r"], Session),
		cmd("rename", "Rename current session", "rename <name>", &[], Session),
		cmd("server", "Show or change ACP server", "server [name]", &[], Session),
		cmd("model", "Show or change model", "model [name]", &[], Session),
		cmd("mcp", "Manage MCP connections", "mcp [status|restart]", &[], Session),
		cmd("acp", "Manage ACP connection", "acp [status|restart]", &[], Session),
		// ── Config (2) ────────────────────────────────────────
		cmd("config", "Show configuration", "config [key]", &[], Config),
		cmd("settings", "View or change settings", "settings [key] [value]", &["set"], Config),
		// ── Files (5) ─────────────────────────────────────────
		cmd("files", "List files in virtual filesystem", "files [path]", &[], Files),
		cmd("save", "Save a virtual file to disk", "save <path>", &[], Files),
		cmd("validate", "Validate virtual file contents", "validate [path]", &[], Files),
		cmd("discard", "Discard virtual file changes", "discard <path>", &[], Files),
		cmd("diff", "Show diff of virtual file changes", "diff [path]", &[], Files),
		// ── AI (2) ────────────────────────────────────────────
		cmd("chain", "Run a prompt chain", "chain <name> [args...]", &["prompt"], Ai),
		cmd("prompts", "List available prompt templates", "prompts", &[], Ai),
	]
}

#[cfg(test)]
mod tests {
	use super::*;

	fn sample_commands() -> Vec<CommandDefinition> {
		vec![
			CommandDefinition {
				name: "help".into(),
				description: "Show help".into(),
				usage: "help [command]".into(),
				aliases: vec!["h".into(), "?".into()],
				category: CommandCategory::Meta,
				hidden: false,
			},
			CommandDefinition {
				name: "exit".into(),
				description: "Exit the app".into(),
				usage: "exit".into(),
				aliases: vec!["quit".into(), "q".into()],
				category: CommandCategory::Meta,
				hidden: false,
			},
			CommandDefinition {
				name: "search".into(),
				description: "Search library".into(),
				usage: "search <query>".into(),
				aliases: vec!["find".into()],
				category: CommandCategory::Library,
				hidden: false,
			},
		]
	}

	#[test]
	fn find_command_by_name() {
		let cmds = sample_commands();
		let found = find_command(&cmds, "help");
		assert!(found.is_some());
		assert_eq!(found.unwrap().name, "help");
	}

	#[test]
	fn find_command_by_alias() {
		let cmds = sample_commands();
		let found = find_command(&cmds, "q");
		assert!(found.is_some());
		assert_eq!(found.unwrap().name, "exit");
	}

	#[test]
	fn find_command_case_insensitive() {
		let cmds = sample_commands();
		assert!(find_command(&cmds, "HELP").is_some());
		assert!(find_command(&cmds, "Q").is_some());
	}

	#[test]
	fn find_command_returns_none_for_unknown() {
		let cmds = sample_commands();
		assert!(find_command(&cmds, "nonexistent").is_none());
	}

	#[test]
	fn filter_commands_by_prefix() {
		let cmds = sample_commands();
		let filtered = filter_commands(&cmds, "hel");
		assert_eq!(filtered.len(), 1);
		assert_eq!(filtered[0].name, "help");
	}

	#[test]
	fn filter_commands_matches_aliases() {
		let cmds = sample_commands();
		let filtered = filter_commands(&cmds, "fin");
		assert_eq!(filtered.len(), 1);
		assert_eq!(filtered[0].name, "search");
	}

	#[test]
	fn all_commands_has_at_least_30() {
		let cmds = all_commands();
		assert!(cmds.len() >= 30);
	}

	#[test]
	fn all_categories_represented() {
		let cmds = all_commands();
		let categories: std::collections::HashSet<_> = cmds.iter().map(|c| &c.category).collect();
		assert!(categories.contains(&CommandCategory::Meta));
		assert!(categories.contains(&CommandCategory::Library));
		assert!(categories.contains(&CommandCategory::Tools));
		assert!(categories.contains(&CommandCategory::Session));
		assert!(categories.contains(&CommandCategory::Config));
		assert!(categories.contains(&CommandCategory::Files));
		assert!(categories.contains(&CommandCategory::Ai));
	}

	#[test]
	fn parse_bool_arg_on_off() {
		assert_eq!(parse_bool_arg("on", false), Some(true));
		assert_eq!(parse_bool_arg("off", true), Some(false));
		assert_eq!(parse_bool_arg("true", false), Some(true));
		assert_eq!(parse_bool_arg("false", true), Some(false));
		assert_eq!(parse_bool_arg("1", false), Some(true));
		assert_eq!(parse_bool_arg("0", true), Some(false));
	}

	#[test]
	fn parse_bool_arg_toggle() {
		assert_eq!(parse_bool_arg("", true), Some(false));
		assert_eq!(parse_bool_arg("", false), Some(true));
	}

	#[test]
	fn parse_bool_arg_invalid() {
		assert_eq!(parse_bool_arg("maybe", false), None);
	}

	#[test]
	fn find_exit_by_q_alias() {
		let cmds = all_commands();
		let exit = find_command(&cmds, "q");
		assert!(exit.is_some());
		assert_eq!(exit.unwrap().name, "exit");
	}
}
