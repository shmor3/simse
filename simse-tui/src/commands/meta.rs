//! Meta commands: `/help`, `/clear`, `/exit`, `/verbose`, `/plan`, `/context`,
//! `/compact`, `/shortcuts`.
//!
//! These commands are largely UI-only (no bridge calls needed).  The handlers
//! return `CommandOutput` items that the TUI dispatch layer can act on directly.

use simse_ui_core::commands::registry::{parse_bool_arg, CommandCategory, CommandDefinition};
use std::collections::BTreeMap;

use super::{CommandOutput, OverlayAction};

/// `/help [command]` -- show help information.
pub fn handle_help(args: &str, commands: &[CommandDefinition]) -> Vec<CommandOutput> {
	let query = args.trim();
	if query.is_empty() {
		let text = format_help_text(commands);
		vec![CommandOutput::Success(text)]
	} else {
		// Look up a specific command.
		let lower = query.to_lowercase();
		if let Some(cmd) = commands.iter().find(|c| {
			c.name.to_lowercase() == lower
				|| c.aliases.iter().any(|a| a.to_lowercase() == lower)
		}) {
			let aliases = if cmd.aliases.is_empty() {
				String::new()
			} else {
				format!("  Aliases: {}\n", cmd.aliases.join(", "))
			};
			let text = format!(
				"/{name} -- {desc}\n  Usage: /{usage}\n{aliases}",
				name = cmd.name,
				desc = cmd.description,
				usage = cmd.usage,
			);
			vec![CommandOutput::Success(text)]
		} else {
			vec![CommandOutput::Error(format!(
				"Unknown command: \"{query}\". Type /help for a list."
			))]
		}
	}
}

/// `/clear` -- clear the screen.  Returns a sentinel that the dispatch layer
/// should intercept to clear `app.output` and re-show the banner.
pub fn handle_clear() -> Vec<CommandOutput> {
	vec![CommandOutput::Info("__clear__".into())]
}

/// `/exit` -- exit the application.  Returns a sentinel that the dispatch
/// layer should intercept to set `app.should_quit = true`.
pub fn handle_exit() -> Vec<CommandOutput> {
	vec![CommandOutput::Info("__exit__".into())]
}

/// `/verbose [on|off]` -- toggle verbose mode.
pub fn handle_verbose(args: &str, current: bool) -> Vec<CommandOutput> {
	match parse_bool_arg(args.trim(), current) {
		Some(val) => {
			let state = if val { "on" } else { "off" };
			vec![CommandOutput::Success(format!("Verbose mode {state}."))]
		}
		None => vec![CommandOutput::Error(format!(
			"Invalid argument: \"{}\". Use on/off/true/false.",
			args.trim()
		))],
	}
}

/// `/plan [on|off]` -- toggle plan mode.
pub fn handle_plan(args: &str, current: bool) -> Vec<CommandOutput> {
	match parse_bool_arg(args.trim(), current) {
		Some(val) => {
			let state = if val { "on" } else { "off" };
			vec![CommandOutput::Success(format!("Plan mode {state}."))]
		}
		None => vec![CommandOutput::Error(format!(
			"Invalid argument: \"{}\". Use on/off/true/false.",
			args.trim()
		))],
	}
}

/// `/context` -- show current context usage.
pub fn handle_context(total_tokens: u64, context_percent: u8) -> Vec<CommandOutput> {
	let tokens = format_tokens(total_tokens);
	vec![CommandOutput::Success(format!(
		"Tokens: {tokens} | Context: {context_percent}%"
	))]
}

/// `/compact` -- request conversation compaction.
pub fn handle_compact() -> Vec<CommandOutput> {
	vec![CommandOutput::Info(
		"Would call bridge to compact conversation history".into(),
	)]
}

/// `/shortcuts` -- open the shortcuts overlay.
pub fn handle_shortcuts() -> Vec<CommandOutput> {
	vec![CommandOutput::OpenOverlay(OverlayAction::Shortcuts)]
}

// ── Helpers ─────────────────────────────────────────────

/// Format token count for display.
pub fn format_tokens(tokens: u64) -> String {
	if tokens >= 1_000_000 {
		format!("{:.1}M", tokens as f64 / 1_000_000.0)
	} else if tokens >= 1_000 {
		format!("{:.1}k", tokens as f64 / 1_000.0)
	} else {
		tokens.to_string()
	}
}

/// Format help text grouped by category.
fn format_help_text(commands: &[CommandDefinition]) -> String {
	let mut groups: BTreeMap<String, Vec<&CommandDefinition>> = BTreeMap::new();
	for cmd in commands {
		if cmd.hidden {
			continue;
		}
		let cat = match cmd.category {
			CommandCategory::Meta => "Meta",
			CommandCategory::Library => "Library",
			CommandCategory::Tools => "Tools",
			CommandCategory::Session => "Session",
			CommandCategory::Config => "Config",
			CommandCategory::Files => "Files",
			CommandCategory::Ai => "AI",
		};
		groups.entry(cat.into()).or_default().push(cmd);
	}

	let mut out = String::from("Available commands:\n");
	for (cat, cmds) in &groups {
		out.push_str(&format!("\n  {cat}:\n"));
		for cmd in cmds {
			let aliases = if cmd.aliases.is_empty() {
				String::new()
			} else {
				format!(" ({})", cmd.aliases.join(", "))
			};
			out.push_str(&format!(
				"    /{}{} -- {}\n",
				cmd.name, aliases, cmd.description
			));
		}
	}
	out
}

#[cfg(test)]
mod tests {
	use super::*;
	use simse_ui_core::commands::registry::all_commands;

	fn test_commands() -> Vec<CommandDefinition> {
		all_commands()
	}

	// ── /help ────────────────────────────────────────────

	#[test]
	fn help_no_args_lists_all() {
		let cmds = test_commands();
		let out = handle_help("", &cmds);
		assert_eq!(out.len(), 1);
		assert!(matches!(&out[0], CommandOutput::Success(text) if text.contains("Available commands")));
	}

	#[test]
	fn help_shows_categories() {
		let cmds = test_commands();
		let out = handle_help("", &cmds);
		if let CommandOutput::Success(text) = &out[0] {
			assert!(text.contains("Meta"));
			assert!(text.contains("Library"));
			assert!(text.contains("Tools"));
			assert!(text.contains("Session"));
		} else {
			panic!("Expected Success");
		}
	}

	#[test]
	fn help_specific_command() {
		let cmds = test_commands();
		let out = handle_help("search", &cmds);
		assert!(matches!(&out[0], CommandOutput::Success(text) if text.contains("search")));
	}

	#[test]
	fn help_specific_command_by_alias() {
		let cmds = test_commands();
		let out = handle_help("s", &cmds);
		assert!(matches!(&out[0], CommandOutput::Success(text) if text.contains("search")));
	}

	#[test]
	fn help_unknown_command() {
		let cmds = test_commands();
		let out = handle_help("nonexistent", &cmds);
		assert!(matches!(&out[0], CommandOutput::Error(msg) if msg.contains("Unknown")));
	}

	#[test]
	fn help_case_insensitive() {
		let cmds = test_commands();
		let out = handle_help("HELP", &cmds);
		assert!(matches!(&out[0], CommandOutput::Success(_)));
	}

	// ── /clear ───────────────────────────────────────────

	#[test]
	fn clear_returns_sentinel() {
		let out = handle_clear();
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg == "__clear__"));
	}

	// ── /exit ────────────────────────────────────────────

	#[test]
	fn exit_returns_sentinel() {
		let out = handle_exit();
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg == "__exit__"));
	}

	// ── /verbose ─────────────────────────────────────────

	#[test]
	fn verbose_toggle_off_to_on() {
		let out = handle_verbose("", false);
		assert!(matches!(&out[0], CommandOutput::Success(msg) if msg.contains("on")));
	}

	#[test]
	fn verbose_toggle_on_to_off() {
		let out = handle_verbose("", true);
		assert!(matches!(&out[0], CommandOutput::Success(msg) if msg.contains("off")));
	}

	#[test]
	fn verbose_explicit_on() {
		let out = handle_verbose("on", false);
		assert!(matches!(&out[0], CommandOutput::Success(msg) if msg.contains("on")));
	}

	#[test]
	fn verbose_explicit_off() {
		let out = handle_verbose("off", true);
		assert!(matches!(&out[0], CommandOutput::Success(msg) if msg.contains("off")));
	}

	#[test]
	fn verbose_invalid() {
		let out = handle_verbose("maybe", false);
		assert!(matches!(&out[0], CommandOutput::Error(msg) if msg.contains("maybe")));
	}

	// ── /plan ────────────────────────────────────────────

	#[test]
	fn plan_toggle() {
		let out = handle_plan("", false);
		assert!(matches!(&out[0], CommandOutput::Success(msg) if msg.contains("on")));
	}

	#[test]
	fn plan_explicit() {
		let out = handle_plan("off", true);
		assert!(matches!(&out[0], CommandOutput::Success(msg) if msg.contains("off")));
	}

	#[test]
	fn plan_invalid() {
		let out = handle_plan("nah", false);
		assert!(matches!(&out[0], CommandOutput::Error(_)));
	}

	// ── /context ─────────────────────────────────────────

	#[test]
	fn context_shows_tokens_and_percent() {
		let out = handle_context(1500, 42);
		assert!(
			matches!(&out[0], CommandOutput::Success(msg) if msg.contains("1.5k") && msg.contains("42%"))
		);
	}

	#[test]
	fn context_zero_tokens() {
		let out = handle_context(0, 0);
		assert!(
			matches!(&out[0], CommandOutput::Success(msg) if msg.contains("0") && msg.contains("0%"))
		);
	}

	// ── /compact ─────────────────────────────────────────

	#[test]
	fn compact_returns_info() {
		let out = handle_compact();
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("compact")));
	}

	// ── /shortcuts ───────────────────────────────────────

	#[test]
	fn shortcuts_opens_overlay() {
		let out = handle_shortcuts();
		assert!(matches!(
			&out[0],
			CommandOutput::OpenOverlay(OverlayAction::Shortcuts)
		));
	}

	// ── format_tokens ────────────────────────────────────

	#[test]
	fn format_tokens_small() {
		assert_eq!(format_tokens(42), "42");
		assert_eq!(format_tokens(999), "999");
	}

	#[test]
	fn format_tokens_thousands() {
		assert_eq!(format_tokens(1000), "1.0k");
		assert_eq!(format_tokens(1500), "1.5k");
		assert_eq!(format_tokens(42000), "42.0k");
	}

	#[test]
	fn format_tokens_millions() {
		assert_eq!(format_tokens(1_000_000), "1.0M");
		assert_eq!(format_tokens(2_500_000), "2.5M");
	}
}
