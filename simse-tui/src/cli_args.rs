//! CLI argument parsing — lightweight manual parser (no clap dependency).
//!
//! Provides [`CliArgs`] and [`parse_cli_args`] for parsing command-line arguments
//! into a typed struct. Supports the following flags:
//!
//! - `-p <prompt>` / `--prompt <prompt>`: Non-interactive mode prompt
//! - `--format <text|json>`: Output format (default: "text")
//! - `--continue`: Continue last session
//! - `--resume <id>`: Resume a specific session by ID
//! - `--server <name>`: Override ACP server
//! - `--agent <id>`: Override agent
//! - `-v` / `--verbose`: Verbose output
//! - `-h` / `--help`: Print help and exit

// ---------------------------------------------------------------------------
// CliArgs
// ---------------------------------------------------------------------------

/// Parsed CLI arguments.
#[derive(Debug, Clone, PartialEq)]
pub struct CliArgs {
	/// Non-interactive prompt (if set, run prompt and exit).
	pub prompt: Option<String>,
	/// Output format: "text" or "json". Default: "text".
	pub format: String,
	/// Continue the last session.
	pub continue_session: bool,
	/// Resume a specific session by ID.
	pub resume: Option<String>,
	/// Override ACP server name.
	pub server: Option<String>,
	/// Override agent ID.
	pub agent: Option<String>,
	/// Enable verbose output.
	pub verbose: bool,
	/// If true, the user requested `--help` and the program should print
	/// usage and exit.
	pub help: bool,
}

impl Default for CliArgs {
	fn default() -> Self {
		Self {
			prompt: None,
			format: "text".into(),
			continue_session: false,
			resume: None,
			server: None,
			agent: None,
			verbose: false,
			help: false,
		}
	}
}

// ---------------------------------------------------------------------------
// Help text
// ---------------------------------------------------------------------------

/// Return the help/usage text for the CLI.
pub fn help_text() -> String {
	"\
SimSE — AI-powered terminal assistant

Usage: simse [options]

Options:
  -p, --prompt <text>     Run a single prompt non-interactively
      --format <fmt>      Output format: text (default) or json
      --continue          Continue the last session
      --resume <id>       Resume a specific session by ID
      --server <name>     Override ACP server name
      --agent <id>        Override agent ID
  -v, --verbose           Enable verbose output
  -h, --help              Show this help message"
		.into()
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/// Parse CLI arguments into a [`CliArgs`] struct.
///
/// The `args` slice should be the full `std::env::args()` output, including
/// the program name at index 0 (which is skipped).
///
/// Unknown flags are silently ignored. Flags that expect a value but are
/// missing one at the end of the args list are ignored.
pub fn parse_cli_args(args: &[String]) -> CliArgs {
	let mut result = CliArgs::default();

	// Skip args[0] (program name)
	let args = if args.is_empty() { args } else { &args[1..] };

	let mut i = 0;
	while i < args.len() {
		let arg = &args[i];

		match arg.as_str() {
			"-h" | "--help" => {
				result.help = true;
			}
			"-v" | "--verbose" => {
				result.verbose = true;
			}
			"--continue" => {
				result.continue_session = true;
			}
			"-p" | "--prompt" => {
				if let Some(value) = args.get(i + 1) {
					result.prompt = Some(value.clone());
					i += 1;
				}
			}
			"--format" => {
				if let Some(value) = args.get(i + 1) {
					result.format = value.clone();
					i += 1;
				}
			}
			"--resume" => {
				if let Some(value) = args.get(i + 1) {
					result.resume = Some(value.clone());
					i += 1;
				}
			}
			"--server" => {
				if let Some(value) = args.get(i + 1) {
					result.server = Some(value.clone());
					i += 1;
				}
			}
			"--agent" => {
				if let Some(value) = args.get(i + 1) {
					result.agent = Some(value.clone());
					i += 1;
				}
			}
			_ => {
				// Unknown flag — silently ignore
			}
		}

		i += 1;
	}

	result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;

	/// Helper to build an args vec from a string slice (prepends program name).
	fn args(parts: &[&str]) -> Vec<String> {
		let mut v = vec!["simse".to_string()];
		v.extend(parts.iter().map(|s| s.to_string()));
		v
	}

	#[test]
	fn cli_args_default_values() {
		let result = parse_cli_args(&args(&[]));
		assert_eq!(result.prompt, None);
		assert_eq!(result.format, "text");
		assert!(!result.continue_session);
		assert_eq!(result.resume, None);
		assert_eq!(result.server, None);
		assert_eq!(result.agent, None);
		assert!(!result.verbose);
		assert!(!result.help);
	}

	#[test]
	fn cli_args_prompt_short() {
		let result = parse_cli_args(&args(&["-p", "hello world"]));
		assert_eq!(result.prompt.as_deref(), Some("hello world"));
	}

	#[test]
	fn cli_args_prompt_long() {
		let result = parse_cli_args(&args(&["--prompt", "explain rust"]));
		assert_eq!(result.prompt.as_deref(), Some("explain rust"));
	}

	#[test]
	fn cli_args_format() {
		let result = parse_cli_args(&args(&["--format", "json"]));
		assert_eq!(result.format, "json");
	}

	#[test]
	fn cli_args_format_default() {
		let result = parse_cli_args(&args(&[]));
		assert_eq!(result.format, "text");
	}

	#[test]
	fn cli_args_continue() {
		let result = parse_cli_args(&args(&["--continue"]));
		assert!(result.continue_session);
	}

	#[test]
	fn cli_args_resume() {
		let result = parse_cli_args(&args(&["--resume", "sess-abc-123"]));
		assert_eq!(result.resume.as_deref(), Some("sess-abc-123"));
	}

	#[test]
	fn cli_args_server() {
		let result = parse_cli_args(&args(&["--server", "ollama"]));
		assert_eq!(result.server.as_deref(), Some("ollama"));
	}

	#[test]
	fn cli_args_agent() {
		let result = parse_cli_args(&args(&["--agent", "coder"]));
		assert_eq!(result.agent.as_deref(), Some("coder"));
	}

	#[test]
	fn cli_args_verbose_short() {
		let result = parse_cli_args(&args(&["-v"]));
		assert!(result.verbose);
	}

	#[test]
	fn cli_args_verbose_long() {
		let result = parse_cli_args(&args(&["--verbose"]));
		assert!(result.verbose);
	}

	#[test]
	fn cli_args_help_short() {
		let result = parse_cli_args(&args(&["-h"]));
		assert!(result.help);
	}

	#[test]
	fn cli_args_help_long() {
		let result = parse_cli_args(&args(&["--help"]));
		assert!(result.help);
	}

	#[test]
	fn cli_args_multiple_flags() {
		let result = parse_cli_args(&args(&[
			"-p",
			"test prompt",
			"--format",
			"json",
			"--server",
			"ollama",
			"--agent",
			"coder",
			"-v",
		]));
		assert_eq!(result.prompt.as_deref(), Some("test prompt"));
		assert_eq!(result.format, "json");
		assert_eq!(result.server.as_deref(), Some("ollama"));
		assert_eq!(result.agent.as_deref(), Some("coder"));
		assert!(result.verbose);
		assert!(!result.help);
	}

	#[test]
	fn cli_args_unknown_flags_ignored() {
		let result = parse_cli_args(&args(&["--unknown", "--also-unknown", "value"]));
		assert_eq!(result, CliArgs::default());
	}

	#[test]
	fn cli_args_missing_value_at_end() {
		// --prompt with no following value: prompt stays None
		let result = parse_cli_args(&args(&["-p"]));
		assert_eq!(result.prompt, None);
	}

	#[test]
	fn cli_args_missing_format_value() {
		let result = parse_cli_args(&args(&["--format"]));
		assert_eq!(result.format, "text"); // stays default
	}

	#[test]
	fn cli_args_missing_resume_value() {
		let result = parse_cli_args(&args(&["--resume"]));
		assert_eq!(result.resume, None);
	}

	#[test]
	fn cli_args_missing_server_value() {
		let result = parse_cli_args(&args(&["--server"]));
		assert_eq!(result.server, None);
	}

	#[test]
	fn cli_args_missing_agent_value() {
		let result = parse_cli_args(&args(&["--agent"]));
		assert_eq!(result.agent, None);
	}

	#[test]
	fn cli_args_empty_args_list() {
		let result = parse_cli_args(&[]);
		assert_eq!(result, CliArgs::default());
	}

	#[test]
	fn cli_args_program_name_only() {
		let result = parse_cli_args(&["simse".to_string()]);
		assert_eq!(result, CliArgs::default());
	}

	#[test]
	fn cli_args_continue_and_resume_both() {
		// Both can be set; the runtime decides which takes precedence
		let result = parse_cli_args(&args(&["--continue", "--resume", "sess-1"]));
		assert!(result.continue_session);
		assert_eq!(result.resume.as_deref(), Some("sess-1"));
	}

	#[test]
	fn cli_args_help_text_not_empty() {
		let text = help_text();
		assert!(!text.is_empty());
		assert!(text.contains("--prompt"));
		assert!(text.contains("--format"));
		assert!(text.contains("--continue"));
		assert!(text.contains("--resume"));
		assert!(text.contains("--server"));
		assert!(text.contains("--agent"));
		assert!(text.contains("--verbose"));
		assert!(text.contains("--help"));
	}

	#[test]
	fn cli_args_default_trait() {
		let args = CliArgs::default();
		assert_eq!(args.format, "text");
		assert!(!args.verbose);
		assert!(!args.help);
		assert!(!args.continue_session);
	}

	#[test]
	fn cli_args_debug_trait() {
		let args = CliArgs::default();
		let debug = format!("{:?}", args);
		assert!(debug.contains("CliArgs"));
		assert!(debug.contains("format"));
	}

	#[test]
	fn cli_args_clone_trait() {
		let args = parse_cli_args(&args(&["-p", "test", "-v"]));
		let cloned = args.clone();
		assert_eq!(args, cloned);
	}

	#[test]
	fn cli_args_prompt_with_spaces() {
		let result = parse_cli_args(&args(&["-p", "explain how closures work in Rust"]));
		assert_eq!(
			result.prompt.as_deref(),
			Some("explain how closures work in Rust")
		);
	}

	#[test]
	fn cli_args_format_text_explicit() {
		let result = parse_cli_args(&args(&["--format", "text"]));
		assert_eq!(result.format, "text");
	}

	#[test]
	fn cli_args_last_value_wins() {
		// When the same flag is given twice, the last value wins
		let result = parse_cli_args(&args(&["-p", "first", "-p", "second"]));
		assert_eq!(result.prompt.as_deref(), Some("second"));
	}
}
