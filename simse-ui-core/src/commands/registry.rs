//! Command registry: lookup by name/alias, categorization.

use serde::{Deserialize, Serialize};

/// A command definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandDefinition {
	pub name: String,
	pub description: String,
	pub aliases: Vec<String>,
	pub category: String,
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

#[cfg(test)]
mod tests {
	use super::*;

	fn sample_commands() -> Vec<CommandDefinition> {
		vec![
			CommandDefinition {
				name: "help".into(),
				description: "Show help".into(),
				aliases: vec!["h".into(), "?".into()],
				category: "meta".into(),
				hidden: false,
			},
			CommandDefinition {
				name: "exit".into(),
				description: "Exit the app".into(),
				aliases: vec!["quit".into(), "q".into()],
				category: "meta".into(),
				hidden: false,
			},
			CommandDefinition {
				name: "search".into(),
				description: "Search library".into(),
				aliases: vec!["find".into()],
				category: "library".into(),
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
}
