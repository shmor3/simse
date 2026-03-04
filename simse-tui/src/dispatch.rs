//! Command dispatch — routes slash-command names to handler modules.
//!
//! The main entry points are:
//! - [`parse_command_line`]: splits `/command args` into `("command", "args")`.
//! - [`dispatch_command`]: simple dispatch with default state (no app context).
//! - [`DispatchContext`]: carries UI state so meta commands (`/help`, `/verbose`,
//!   `/plan`, `/context`) can receive the current values they need.

use simse_ui_core::commands::registry::{all_commands, CommandDefinition};

use crate::commands::{self, CommandOutput};

/// Parse a `/command args` line into `(command_name, args)`.
///
/// Returns `None` if the input does not start with `/` or has no command name.
pub fn parse_command_line(input: &str) -> Option<(String, String)> {
	let trimmed = input.trim();
	if !trimmed.starts_with('/') {
		return None;
	}

	let without_slash = &trimmed[1..];
	if without_slash.is_empty() {
		return None;
	}

	let mut parts = without_slash.splitn(2, ' ');
	let command = parts.next()?.to_lowercase();
	let args = parts.next().unwrap_or("").to_string();

	if command.is_empty() {
		return None;
	}

	Some((command, args))
}

/// Holds UI state needed by meta commands that require context beyond the
/// command arguments (e.g. the current verbose/plan toggles, token counts,
/// and the command registry for `/help`).
pub struct DispatchContext {
	/// Whether verbose mode is currently on.
	pub verbose: bool,
	/// Whether plan mode is currently on.
	pub plan: bool,
	/// Current total token count (for `/context`).
	pub total_tokens: u64,
	/// Current context window usage percentage (for `/context`).
	pub context_percent: u8,
	/// The registered command definitions (for `/help`).
	pub commands: Vec<CommandDefinition>,
}

impl Default for DispatchContext {
	fn default() -> Self {
		Self {
			verbose: false,
			plan: false,
			total_tokens: 0,
			context_percent: 0,
			commands: all_commands(),
		}
	}
}

impl DispatchContext {
	/// Dispatch a command with full UI context.
	pub fn dispatch(&self, command: &str, args: &str) -> Vec<CommandOutput> {
		dispatch_inner(command, args, self)
	}
}

/// Dispatch a slash command to the appropriate handler.
///
/// Meta commands that require UI state (`/help`, `/verbose`, `/plan`,
/// `/context`) will use sensible defaults (empty command list for help,
/// `false` for toggles, zero for token counts).  Use [`DispatchContext`]
/// when you need to pass real state.
pub fn dispatch_command(command: &str, args: &str) -> Vec<CommandOutput> {
	let ctx = DispatchContext::default();
	dispatch_inner(command, args, &ctx)
}

/// Inner dispatch — shared by both the context-free and context-aware entry
/// points.
fn dispatch_inner(command: &str, args: &str, ctx: &DispatchContext) -> Vec<CommandOutput> {
	match command {
		// ── Library ──────────────────────────────────────────
		"add" => commands::library::handle_add(args),
		"search" => commands::library::handle_search(args),
		"recommend" => commands::library::handle_recommend(args),
		"topics" => commands::library::handle_topics(args),
		"volumes" => commands::library::handle_volumes(args),
		"get" => commands::library::handle_get(args),
		"delete" => commands::library::handle_delete(args),
		"librarians" => commands::library::handle_librarians(args),

		// ── Session ──────────────────────────────────────────
		"sessions" => commands::session::handle_sessions(args),
		"resume" => commands::session::handle_resume(args),
		"rename" => commands::session::handle_rename(args),
		"server" => commands::session::handle_server(args),
		"model" => commands::session::handle_model(args),
		"mcp" => commands::session::handle_mcp(args),
		"acp" => commands::session::handle_acp(args),

		// ── Config ───────────────────────────────────────────
		"setup" => commands::config::handle_setup(args),
		"init" => commands::config::handle_init(args),
		"config" => commands::config::handle_config(args),
		"settings" => commands::config::handle_settings(args),
		"factory-reset" => commands::config::handle_factory_reset(args),
		"factory-reset-project" => commands::config::handle_factory_reset_project(args),

		// ── Files ────────────────────────────────────────────
		"files" => commands::files::handle_files(args),
		"save" => commands::files::handle_save(args),
		"validate" => commands::files::handle_validate(args),
		"discard" => commands::files::handle_discard(args),
		"diff" => commands::files::handle_diff(args),

		// ── AI ───────────────────────────────────────────────
		"chain" => commands::ai::handle_chain(args),
		"prompts" => commands::ai::handle_prompts(args),

		// ── Tools ────────────────────────────────────────────
		"tools" => commands::tools::handle_tools(args),
		"agents" => commands::tools::handle_agents(args),
		"skills" => commands::tools::handle_skills(args),

		// ── Meta ─────────────────────────────────────────────
		"help" => commands::meta::handle_help(args, &ctx.commands),
		"clear" => commands::meta::handle_clear(),
		"exit" | "quit" | "q" => commands::meta::handle_exit(),
		"verbose" => commands::meta::handle_verbose(args, ctx.verbose),
		"plan" => commands::meta::handle_plan(args, ctx.plan),
		"context" => commands::meta::handle_context(ctx.total_tokens, ctx.context_percent),
		"compact" => commands::meta::handle_compact(),
		"shortcuts" => commands::meta::handle_shortcuts(),

		// ── Unknown ──────────────────────────────────────────
		other => vec![CommandOutput::Error(format!(
			"Unknown command: /{other}"
		))],
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::commands::OverlayAction;

	// ── parse_command_line ───────────────────────────────────

	#[test]
	fn parse_valid_command_no_args() {
		let result = parse_command_line("/help");
		assert_eq!(result, Some(("help".into(), String::new())));
	}

	#[test]
	fn parse_valid_command_with_args() {
		let result = parse_command_line("/search foo bar");
		assert_eq!(result, Some(("search".into(), "foo bar".into())));
	}

	#[test]
	fn parse_trims_surrounding_whitespace() {
		let result = parse_command_line("  /add topic text  ");
		// trim() removes leading/trailing whitespace from the whole input first,
		// so trailing spaces are stripped before splitting.
		assert_eq!(result, Some(("add".into(), "topic text".into())));
	}

	#[test]
	fn parse_lowercases_command() {
		let result = parse_command_line("/HELP");
		assert_eq!(result, Some(("help".into(), String::new())));
	}

	#[test]
	fn parse_no_slash_returns_none() {
		assert_eq!(parse_command_line("help"), None);
	}

	#[test]
	fn parse_empty_returns_none() {
		assert_eq!(parse_command_line(""), None);
	}

	#[test]
	fn parse_just_slash_returns_none() {
		assert_eq!(parse_command_line("/"), None);
	}

	#[test]
	fn parse_whitespace_only_returns_none() {
		assert_eq!(parse_command_line("   "), None);
	}

	#[test]
	fn parse_slash_with_spaces_returns_none() {
		assert_eq!(parse_command_line("  /  "), None);
	}

	#[test]
	fn parse_hyphenated_command() {
		let result = parse_command_line("/factory-reset");
		assert_eq!(result, Some(("factory-reset".into(), String::new())));
	}

	#[test]
	fn parse_hyphenated_command_with_args() {
		let result = parse_command_line("/factory-reset-project --force");
		assert_eq!(
			result,
			Some(("factory-reset-project".into(), "--force".into()))
		);
	}

	// ── dispatch_command: library ───────────────────────────

	#[test]
	fn dispatch_add() {
		let out = dispatch_command("add", "topic some text");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("topic=\"topic\"")));
	}

	#[test]
	fn dispatch_search() {
		let out = dispatch_command("search", "query");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("query")));
	}

	#[test]
	fn dispatch_recommend() {
		let out = dispatch_command("recommend", "patterns");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("patterns")));
	}

	#[test]
	fn dispatch_topics() {
		let out = dispatch_command("topics", "");
		assert!(matches!(&out[0], CommandOutput::Info(_)));
	}

	#[test]
	fn dispatch_volumes() {
		let out = dispatch_command("volumes", "rust");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("rust")));
	}

	#[test]
	fn dispatch_get() {
		let out = dispatch_command("get", "id-42");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("id-42")));
	}

	#[test]
	fn dispatch_delete() {
		let out = dispatch_command("delete", "id-99");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("id-99")));
	}

	#[test]
	fn dispatch_librarians() {
		let out = dispatch_command("librarians", "");
		assert!(matches!(
			&out[0],
			CommandOutput::OpenOverlay(OverlayAction::Librarians)
		));
	}

	// ── dispatch_command: session ────────────────────────────

	#[test]
	fn dispatch_sessions() {
		let out = dispatch_command("sessions", "");
		assert!(matches!(&out[0], CommandOutput::Info(_)));
	}

	#[test]
	fn dispatch_resume() {
		let out = dispatch_command("resume", "sess-1");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("sess-1")));
	}

	#[test]
	fn dispatch_rename() {
		let out = dispatch_command("rename", "Cool Name");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("Cool Name")));
	}

	#[test]
	fn dispatch_server() {
		let out = dispatch_command("server", "ollama");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("ollama")));
	}

	#[test]
	fn dispatch_model() {
		let out = dispatch_command("model", "gpt-4o");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("gpt-4o")));
	}

	#[test]
	fn dispatch_mcp() {
		let out = dispatch_command("mcp", "status");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("status")));
	}

	#[test]
	fn dispatch_acp() {
		let out = dispatch_command("acp", "restart");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("restart")));
	}

	// ── dispatch_command: config ─────────────────────────────

	#[test]
	fn dispatch_setup() {
		let out = dispatch_command("setup", "");
		assert!(matches!(
			&out[0],
			CommandOutput::OpenOverlay(OverlayAction::Setup(None))
		));
	}

	#[test]
	fn dispatch_init() {
		let out = dispatch_command("init", "");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("initialize")));
	}

	#[test]
	fn dispatch_config() {
		let out = dispatch_command("config", "acp.timeout");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("acp.timeout")));
	}

	#[test]
	fn dispatch_settings() {
		let out = dispatch_command("settings", "");
		assert!(matches!(
			&out[0],
			CommandOutput::OpenOverlay(OverlayAction::Settings)
		));
	}

	#[test]
	fn dispatch_factory_reset() {
		let out = dispatch_command("factory-reset", "");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("factory reset")));
	}

	#[test]
	fn dispatch_factory_reset_project() {
		let out = dispatch_command("factory-reset-project", "");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("project")));
	}

	// ── dispatch_command: files ──────────────────────────────

	#[test]
	fn dispatch_files() {
		let out = dispatch_command("files", "src");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("src")));
	}

	#[test]
	fn dispatch_save() {
		let out = dispatch_command("save", "output.txt");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("output.txt")));
	}

	#[test]
	fn dispatch_validate() {
		let out = dispatch_command("validate", "");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("all")));
	}

	#[test]
	fn dispatch_discard() {
		let out = dispatch_command("discard", "temp.rs");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("temp.rs")));
	}

	#[test]
	fn dispatch_diff() {
		let out = dispatch_command("diff", "lib.rs");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("lib.rs")));
	}

	// ── dispatch_command: ai ─────────────────────────────────

	#[test]
	fn dispatch_chain() {
		let out = dispatch_command("chain", "summarize");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("summarize")));
	}

	#[test]
	fn dispatch_prompts() {
		let out = dispatch_command("prompts", "");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("prompt templates")));
	}

	// ── dispatch_command: tools ──────────────────────────────

	#[test]
	fn dispatch_tools() {
		let out = dispatch_command("tools", "read");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("read")));
	}

	#[test]
	fn dispatch_agents() {
		let out = dispatch_command("agents", "");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("agents")));
	}

	#[test]
	fn dispatch_skills() {
		let out = dispatch_command("skills", "");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("skills")));
	}

	// ── dispatch_command: meta ───────────────────────────────

	#[test]
	fn dispatch_help() {
		let out = dispatch_command("help", "");
		assert!(matches!(&out[0], CommandOutput::Success(msg) if msg.contains("Available commands")));
	}

	#[test]
	fn dispatch_clear() {
		let out = dispatch_command("clear", "");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg == "__clear__"));
	}

	#[test]
	fn dispatch_exit() {
		let out = dispatch_command("exit", "");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg == "__exit__"));
	}

	#[test]
	fn dispatch_quit_alias() {
		let out = dispatch_command("quit", "");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg == "__exit__"));
	}

	#[test]
	fn dispatch_q_alias() {
		let out = dispatch_command("q", "");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg == "__exit__"));
	}

	#[test]
	fn dispatch_verbose() {
		let out = dispatch_command("verbose", "on");
		assert!(matches!(&out[0], CommandOutput::Success(msg) if msg.contains("on")));
	}

	#[test]
	fn dispatch_plan() {
		let out = dispatch_command("plan", "off");
		// Default plan state is false; explicit "off" should work.
		assert!(matches!(&out[0], CommandOutput::Success(msg) if msg.contains("off")));
	}

	#[test]
	fn dispatch_context() {
		let out = dispatch_command("context", "");
		// Default tokens = 0, percent = 0.
		assert!(matches!(&out[0], CommandOutput::Success(msg) if msg.contains("0%")));
	}

	#[test]
	fn dispatch_compact() {
		let out = dispatch_command("compact", "");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("compact")));
	}

	#[test]
	fn dispatch_shortcuts() {
		let out = dispatch_command("shortcuts", "");
		assert!(matches!(
			&out[0],
			CommandOutput::OpenOverlay(OverlayAction::Shortcuts)
		));
	}

	// ── dispatch_command: unknown ────────────────────────────

	#[test]
	fn dispatch_unknown_command() {
		let out = dispatch_command("foobar", "");
		assert!(matches!(&out[0], CommandOutput::Error(msg) if msg.contains("/foobar")));
	}

	#[test]
	fn dispatch_unknown_preserves_name() {
		let out = dispatch_command("xyzzy", "blah");
		assert!(matches!(&out[0], CommandOutput::Error(msg) if msg.contains("/xyzzy")));
	}

	// ── DispatchContext ──────────────────────────────────────

	#[test]
	fn dispatch_context_with_state() {
		let ctx = DispatchContext {
			verbose: true,
			plan: true,
			total_tokens: 42_000,
			context_percent: 65,
			commands: all_commands(),
		};

		// /verbose with no args toggles from current (true -> off).
		let out = ctx.dispatch("verbose", "");
		assert!(matches!(&out[0], CommandOutput::Success(msg) if msg.contains("off")));

		// /plan with no args toggles from current (true -> off).
		let out = ctx.dispatch("plan", "");
		assert!(matches!(&out[0], CommandOutput::Success(msg) if msg.contains("off")));

		// /context shows the provided token counts.
		let out = ctx.dispatch("context", "");
		assert!(
			matches!(&out[0], CommandOutput::Success(msg) if msg.contains("42.0k") && msg.contains("65%"))
		);
	}

	#[test]
	fn dispatch_context_default() {
		let ctx = DispatchContext::default();
		assert!(!ctx.verbose);
		assert!(!ctx.plan);
		assert_eq!(ctx.total_tokens, 0);
		assert_eq!(ctx.context_percent, 0);
		assert!(!ctx.commands.is_empty());
	}

	// ── Integration: parse_command_line + dispatch ────────────

	#[test]
	fn round_trip_parse_and_dispatch() {
		let input = "/search hello world";
		let (cmd, args) = parse_command_line(input).unwrap();
		let out = dispatch_command(&cmd, &args);
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("hello world")));
	}

	#[test]
	fn round_trip_no_args() {
		let input = "/help";
		let (cmd, args) = parse_command_line(input).unwrap();
		let out = dispatch_command(&cmd, &args);
		assert!(matches!(&out[0], CommandOutput::Success(msg) if msg.contains("Available commands")));
	}

	#[test]
	fn round_trip_hyphenated() {
		let input = "/factory-reset";
		let (cmd, args) = parse_command_line(input).unwrap();
		let out = dispatch_command(&cmd, &args);
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("factory reset")));
	}

	#[test]
	fn round_trip_case_insensitive() {
		let input = "/HELP";
		let (cmd, args) = parse_command_line(input).unwrap();
		assert_eq!(cmd, "help");
		let out = dispatch_command(&cmd, &args);
		assert!(matches!(&out[0], CommandOutput::Success(_)));
	}
}
