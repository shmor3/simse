//! Config commands: `/setup`, `/init`, `/config`, `/settings`,
//! `/factory-reset`, `/factory-reset-project`.

use super::{CommandOutput, OverlayAction};

/// `/setup [preset]` -- open the setup wizard (optionally jumping to a preset).
pub fn handle_setup(args: &str) -> Vec<CommandOutput> {
	let preset = args.trim();
	if preset.is_empty() {
		vec![CommandOutput::OpenOverlay(OverlayAction::Setup(None))]
	} else {
		let valid_presets = ["ollama", "openai", "anthropic", "azure", "custom"];
		let lower = preset.to_lowercase();
		if valid_presets.contains(&lower.as_str()) {
			vec![CommandOutput::OpenOverlay(OverlayAction::Setup(Some(
				lower,
			)))]
		} else {
			vec![CommandOutput::Error(format!(
				"Unknown preset: \"{preset}\". Valid presets: {}",
				valid_presets.join(", ")
			))]
		}
	}
}

/// `/init [--force]` -- initialize project configuration.
pub fn handle_init(args: &str) -> Vec<CommandOutput> {
	let args = args.trim();
	let force = args == "--force" || args == "-f";

	if !args.is_empty() && !force {
		return vec![CommandOutput::Error(
			"Usage: /init [--force]".into(),
		)];
	}

	if force {
		vec![CommandOutput::Info(
			"Would call bridge to initialize project configuration (force overwrite)".into(),
		)]
	} else {
		vec![CommandOutput::Info(
			"Would call bridge to initialize project configuration".into(),
		)]
	}
}

/// `/config [key]` -- show configuration values.
pub fn handle_config(args: &str) -> Vec<CommandOutput> {
	let key = args.trim();
	if key.is_empty() {
		vec![CommandOutput::Info(
			"Would call bridge to show all configuration values".into(),
		)]
	} else {
		vec![CommandOutput::Info(format!(
			"Would call bridge to show configuration value for \"{key}\""
		))]
	}
}

/// `/settings` -- open the settings explorer overlay.
pub fn handle_settings(_args: &str) -> Vec<CommandOutput> {
	vec![CommandOutput::OpenOverlay(OverlayAction::Settings)]
}

/// `/factory-reset` -- reset all user settings to defaults.
pub fn handle_factory_reset(_args: &str) -> Vec<CommandOutput> {
	vec![CommandOutput::Info(
		"Would call bridge to perform factory reset of all user settings".into(),
	)]
}

/// `/factory-reset-project` -- reset project-level settings to defaults.
pub fn handle_factory_reset_project(_args: &str) -> Vec<CommandOutput> {
	vec![CommandOutput::Info(
		"Would call bridge to perform factory reset of project settings".into(),
	)]
}

#[cfg(test)]
mod tests {
	use super::*;

	// ── /setup ───────────────────────────────────────────

	#[test]
	fn setup_no_args_opens_overlay() {
		let out = handle_setup("");
		assert_eq!(out.len(), 1);
		assert!(matches!(
			&out[0],
			CommandOutput::OpenOverlay(OverlayAction::Setup(None))
		));
	}

	#[test]
	fn setup_valid_preset() {
		let out = handle_setup("ollama");
		assert!(matches!(
			&out[0],
			CommandOutput::OpenOverlay(OverlayAction::Setup(Some(p))) if p == "ollama"
		));
	}

	#[test]
	fn setup_case_insensitive() {
		let out = handle_setup("OpenAI");
		assert!(matches!(
			&out[0],
			CommandOutput::OpenOverlay(OverlayAction::Setup(Some(p))) if p == "openai"
		));
	}

	#[test]
	fn setup_invalid_preset() {
		let out = handle_setup("invalid-preset");
		assert!(matches!(&out[0], CommandOutput::Error(msg) if msg.contains("invalid-preset")));
	}

	#[test]
	fn setup_all_valid_presets() {
		for preset in &["ollama", "openai", "anthropic", "azure", "custom"] {
			let out = handle_setup(preset);
			assert!(
				matches!(&out[0], CommandOutput::OpenOverlay(OverlayAction::Setup(Some(_)))),
				"Preset {preset} should be valid"
			);
		}
	}

	// ── /init ────────────────────────────────────────────

	#[test]
	fn init_no_args() {
		let out = handle_init("");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("initialize")));
	}

	#[test]
	fn init_force() {
		let out = handle_init("--force");
		assert!(
			matches!(&out[0], CommandOutput::Info(msg) if msg.contains("force"))
		);
	}

	#[test]
	fn init_short_force() {
		let out = handle_init("-f");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("force")));
	}

	#[test]
	fn init_invalid_flag() {
		let out = handle_init("--banana");
		assert!(matches!(&out[0], CommandOutput::Error(msg) if msg.contains("Usage")));
	}

	// ── /config ──────────────────────────────────────────

	#[test]
	fn config_no_args_shows_all() {
		let out = handle_config("");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("all")));
	}

	#[test]
	fn config_with_key() {
		let out = handle_config("acp.timeout");
		assert!(
			matches!(&out[0], CommandOutput::Info(msg) if msg.contains("acp.timeout"))
		);
	}

	// ── /settings ────────────────────────────────────────

	#[test]
	fn settings_opens_overlay() {
		let out = handle_settings("");
		assert_eq!(out.len(), 1);
		assert!(matches!(
			&out[0],
			CommandOutput::OpenOverlay(OverlayAction::Settings)
		));
	}

	// ── /factory-reset ───────────────────────────────────

	#[test]
	fn factory_reset_returns_info() {
		let out = handle_factory_reset("");
		assert!(
			matches!(&out[0], CommandOutput::Info(msg) if msg.contains("factory reset"))
		);
	}

	// ── /factory-reset-project ───────────────────────────

	#[test]
	fn factory_reset_project_returns_info() {
		let out = handle_factory_reset_project("");
		assert!(
			matches!(&out[0], CommandOutput::Info(msg) if msg.contains("project"))
		);
	}
}
