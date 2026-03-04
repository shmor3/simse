//! Onboarding — first-run detection and welcome message.
//!
//! Checks whether ACP servers are configured and provides a formatted
//! welcome banner with setup instructions when they are not.

use ratatui::{
	style::{Color, Modifier, Style},
	text::{Line, Span},
};

// ---------------------------------------------------------------------------
// OnboardingState
// ---------------------------------------------------------------------------

/// Tracks whether the user needs initial setup.
#[derive(Debug, Clone, PartialEq)]
pub struct OnboardingState {
	/// True if no ACP servers are configured and setup is required.
	pub needs_setup: bool,
	/// True if the welcome message has been shown to the user.
	pub welcome_shown: bool,
}

impl OnboardingState {
	/// Create a new onboarding state based on the config.
	///
	/// Calls [`check_config`] to determine if setup is needed.
	pub fn new(config: &serde_json::Value) -> Self {
		Self {
			needs_setup: !check_config(config),
			welcome_shown: false,
		}
	}

	/// Mark the welcome message as having been shown.
	pub fn mark_welcome_shown(&mut self) {
		self.welcome_shown = true;
	}
}

impl Default for OnboardingState {
	fn default() -> Self {
		Self {
			needs_setup: true,
			welcome_shown: false,
		}
	}
}

// ---------------------------------------------------------------------------
// Config checking
// ---------------------------------------------------------------------------

/// Check if ACP servers are configured in the given config JSON.
///
/// Returns `true` if the config contains at least one ACP server entry,
/// `false` otherwise. This is a lightweight check that looks for the
/// `acp.servers` array in the config value.
///
/// Accepts a `serde_json::Value` so the caller can pass in the raw config
/// or a subset of it.
///
/// # Examples
///
/// ```
/// use serde_json::json;
/// use simse_tui::onboarding::check_config;
///
/// let config = json!({
///     "acp": {
///         "servers": [{"name": "ollama", "command": "ollama-acp"}]
///     }
/// });
/// assert!(check_config(&config));
///
/// let empty = json!({});
/// assert!(!check_config(&empty));
/// ```
pub fn check_config(config: &serde_json::Value) -> bool {
	config
		.get("acp")
		.and_then(|acp| acp.get("servers"))
		.and_then(|servers| servers.as_array())
		.map_or(false, |servers| !servers.is_empty())
}

// ---------------------------------------------------------------------------
// Welcome message
// ---------------------------------------------------------------------------

/// Generate the formatted welcome banner with setup instructions.
///
/// Returns a vector of ratatui `Line`s suitable for rendering in the
/// terminal. The message includes:
///
/// - A welcome header
/// - Brief description of SimSE
/// - Step-by-step setup instructions
/// - Quick-start example
pub fn welcome_message() -> Vec<Line<'static>> {
	let bold = Style::default().add_modifier(Modifier::BOLD);
	let cyan = Style::default().fg(Color::Cyan);
	let dim = Style::default().fg(Color::DarkGray);
	let yellow = Style::default().fg(Color::Yellow);
	let green = Style::default().fg(Color::Green);

	vec![
		Line::from(""),
		Line::from(Span::styled(
			"  Welcome to SimSE!",
			bold.fg(Color::Cyan),
		)),
		Line::from(""),
		Line::from(Span::styled(
			"  SimSE is a modular AI assistant for your terminal.",
			dim,
		)),
		Line::from(Span::styled(
			"  To get started, you need to configure an ACP server.",
			dim,
		)),
		Line::from(""),
		Line::from(Span::styled("  Setup Steps:", bold)),
		Line::from(""),
		Line::from(vec![
			Span::styled("  1. ", yellow),
			Span::styled(
				"Create the config directory:",
				Style::default(),
			),
		]),
		Line::from(Span::styled(
			"     mkdir -p ~/.config/simse",
			cyan,
		)),
		Line::from(""),
		Line::from(vec![
			Span::styled("  2. ", yellow),
			Span::styled(
				"Create an ACP server config:",
				Style::default(),
			),
		]),
		Line::from(Span::styled(
			"     Edit ~/.config/simse/acp.json",
			cyan,
		)),
		Line::from(""),
		Line::from(Span::styled("  Example acp.json:", bold)),
		Line::from(Span::styled("  {", green)),
		Line::from(Span::styled(
			"    \"servers\": [{",
			green,
		)),
		Line::from(Span::styled(
			"      \"name\": \"ollama\",",
			green,
		)),
		Line::from(Span::styled(
			"      \"command\": \"ollama-acp-server\",",
			green,
		)),
		Line::from(Span::styled(
			"      \"args\": [\"--model\", \"llama3.1\"]",
			green,
		)),
		Line::from(Span::styled("    }]", green)),
		Line::from(Span::styled("  }", green)),
		Line::from(""),
		Line::from(vec![
			Span::styled("  3. ", yellow),
			Span::styled(
				"Restart SimSE and start chatting!",
				Style::default(),
			),
		]),
		Line::from(""),
		Line::from(Span::styled(
			"  Run /help for available commands.",
			dim,
		)),
		Line::from(""),
	]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;

	#[test]
	fn onboarding_check_config_with_servers() {
		let config = json!({
			"acp": {
				"servers": [
					{"name": "test", "command": "echo"}
				]
			}
		});
		assert!(check_config(&config));
	}

	#[test]
	fn onboarding_check_config_empty_servers() {
		let config = json!({
			"acp": {
				"servers": []
			}
		});
		assert!(!check_config(&config));
	}

	#[test]
	fn onboarding_check_config_no_acp_key() {
		let config = json!({});
		assert!(!check_config(&config));
	}

	#[test]
	fn onboarding_check_config_no_servers_key() {
		let config = json!({
			"acp": {}
		});
		assert!(!check_config(&config));
	}

	#[test]
	fn onboarding_check_config_servers_not_array() {
		let config = json!({
			"acp": {
				"servers": "not an array"
			}
		});
		assert!(!check_config(&config));
	}

	#[test]
	fn onboarding_check_config_null() {
		let config = json!(null);
		assert!(!check_config(&config));
	}

	#[test]
	fn onboarding_check_config_multiple_servers() {
		let config = json!({
			"acp": {
				"servers": [
					{"name": "server-1", "command": "cmd1"},
					{"name": "server-2", "command": "cmd2"}
				]
			}
		});
		assert!(check_config(&config));
	}

	#[test]
	fn onboarding_state_needs_setup() {
		let config = json!({});
		let state = OnboardingState::new(&config);
		assert!(state.needs_setup);
		assert!(!state.welcome_shown);
	}

	#[test]
	fn onboarding_state_no_setup_needed() {
		let config = json!({
			"acp": {
				"servers": [{"name": "test", "command": "echo"}]
			}
		});
		let state = OnboardingState::new(&config);
		assert!(!state.needs_setup);
		assert!(!state.welcome_shown);
	}

	#[test]
	fn onboarding_state_mark_welcome_shown() {
		let mut state = OnboardingState::default();
		assert!(!state.welcome_shown);
		state.mark_welcome_shown();
		assert!(state.welcome_shown);
	}

	#[test]
	fn onboarding_state_default() {
		let state = OnboardingState::default();
		assert!(state.needs_setup);
		assert!(!state.welcome_shown);
	}

	#[test]
	fn onboarding_state_debug() {
		let state = OnboardingState::default();
		let debug = format!("{:?}", state);
		assert!(debug.contains("OnboardingState"));
		assert!(debug.contains("needs_setup"));
	}

	#[test]
	fn onboarding_state_clone() {
		let state = OnboardingState {
			needs_setup: false,
			welcome_shown: true,
		};
		let cloned = state.clone();
		assert_eq!(state, cloned);
	}

	#[test]
	fn onboarding_welcome_message_not_empty() {
		let lines = welcome_message();
		assert!(!lines.is_empty());
	}

	#[test]
	fn onboarding_welcome_message_contains_setup_steps() {
		let lines = welcome_message();
		let text: String = lines
			.iter()
			.flat_map(|line| {
				line.spans
					.iter()
					.map(|span| span.content.to_string())
			})
			.collect::<Vec<_>>()
			.join(" ");
		assert!(text.contains("Welcome to SimSE"));
		assert!(text.contains("ACP"));
		assert!(text.contains("acp.json"));
		assert!(text.contains("/help"));
	}

	#[test]
	fn onboarding_welcome_message_has_config_example() {
		let lines = welcome_message();
		let text: String = lines
			.iter()
			.flat_map(|line| {
				line.spans
					.iter()
					.map(|span| span.content.to_string())
			})
			.collect::<Vec<_>>()
			.join(" ");
		assert!(text.contains("servers"));
		assert!(text.contains("command"));
	}

	#[test]
	fn onboarding_welcome_message_has_numbered_steps() {
		let lines = welcome_message();
		let text: String = lines
			.iter()
			.flat_map(|line| {
				line.spans
					.iter()
					.map(|span| span.content.to_string())
			})
			.collect::<Vec<_>>()
			.join(" ");
		assert!(text.contains("1."));
		assert!(text.contains("2."));
		assert!(text.contains("3."));
	}
}
