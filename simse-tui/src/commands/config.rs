//! Config commands: `/setup`, `/init`, `/config`, `/settings`,
//! `/factory-reset`, `/factory-reset-project`.

use super::{BridgeAction, CommandContext, CommandOutput, OverlayAction};

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

	vec![
		CommandOutput::Info("Initializing project configuration...".into()),
		CommandOutput::BridgeRequest(BridgeAction::InitConfig { force }),
	]
}

/// `/config [key]` -- show configuration values.
pub fn handle_config(args: &str, ctx: &CommandContext) -> Vec<CommandOutput> {
	let key = args.trim();
	if key.is_empty() {
		if ctx.config_values.is_empty() {
			vec![CommandOutput::Info("No configuration loaded. Run /init to create project configuration, or /setup for first-time setup.".into())]
		} else {
			let headers = vec!["Key".into(), "Value".into()];
			let rows: Vec<Vec<String>> = ctx
				.config_values
				.iter()
				.map(|(k, v)| vec![k.clone(), v.clone()])
				.collect();
			vec![CommandOutput::Table { headers, rows }]
		}
	} else {
		let matching: Vec<Vec<String>> = ctx
			.config_values
			.iter()
			.filter(|(k, _)| k == key)
			.map(|(k, v)| vec![k.clone(), v.clone()])
			.collect();
		if matching.is_empty() {
			vec![CommandOutput::Error(format!(
				"Configuration key not found: {key}"
			))]
		} else {
			let headers = vec!["Key".into(), "Value".into()];
			vec![CommandOutput::Table {
				headers,
				rows: matching,
			}]
		}
	}
}

/// `/settings` -- open the settings explorer overlay.
pub fn handle_settings(_args: &str) -> Vec<CommandOutput> {
	vec![CommandOutput::OpenOverlay(OverlayAction::Settings)]
}

/// `/factory-reset` -- reset all user settings to defaults.
pub fn handle_factory_reset(_args: &str) -> Vec<CommandOutput> {
	vec![CommandOutput::ConfirmAction {
		message: "Are you sure? This will delete ALL global SimSE configuration.".into(),
		action: BridgeAction::FactoryReset,
	}]
}

/// `/factory-reset-project` -- reset project-level settings to defaults.
pub fn handle_factory_reset_project(_args: &str) -> Vec<CommandOutput> {
	vec![CommandOutput::ConfirmAction {
		message: "Are you sure? This will delete all project-level SimSE configuration.".into(),
		action: BridgeAction::FactoryResetProject,
	}]
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
		assert_eq!(out.len(), 2);
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg == "Initializing project configuration..."));
		assert!(matches!(
			&out[1],
			CommandOutput::BridgeRequest(BridgeAction::InitConfig { force: false })
		));
	}

	#[test]
	fn init_force() {
		let out = handle_init("--force");
		assert_eq!(out.len(), 2);
		assert!(matches!(&out[0], CommandOutput::Info(_)));
		assert!(matches!(
			&out[1],
			CommandOutput::BridgeRequest(BridgeAction::InitConfig { force: true })
		));
	}

	#[test]
	fn init_short_force() {
		let out = handle_init("-f");
		assert_eq!(out.len(), 2);
		assert!(matches!(&out[0], CommandOutput::Info(_)));
		assert!(matches!(
			&out[1],
			CommandOutput::BridgeRequest(BridgeAction::InitConfig { force: true })
		));
	}

	#[test]
	fn init_invalid_flag() {
		let out = handle_init("--banana");
		assert!(matches!(&out[0], CommandOutput::Error(msg) if msg.contains("Usage")));
	}

	// ── /config ──────────────────────────────────────────

	#[test]
	fn config_no_args_empty() {
		let ctx = CommandContext::default();
		let out = handle_config("", &ctx);
		assert_eq!(out.len(), 1);
		assert!(matches!(
			&out[0],
			CommandOutput::Info(msg) if msg == "No configuration loaded. Run /init to create project configuration, or /setup for first-time setup."
		));
	}

	#[test]
	fn config_no_args_shows_all() {
		let ctx = CommandContext {
			config_values: vec![
				("acp.timeout".into(), "60".into()),
				("log.level".into(), "info".into()),
			],
			..Default::default()
		};
		let out = handle_config("", &ctx);
		assert_eq!(out.len(), 1);
		match &out[0] {
			CommandOutput::Table { headers, rows } => {
				assert_eq!(headers, &["Key", "Value"]);
				assert_eq!(rows.len(), 2);
				assert_eq!(rows[0], vec!["acp.timeout", "60"]);
				assert_eq!(rows[1], vec!["log.level", "info"]);
			}
			other => panic!("expected Table, got {:?}", other),
		}
	}

	#[test]
	fn config_with_key_found() {
		let ctx = CommandContext {
			config_values: vec![
				("acp.timeout".into(), "60".into()),
				("log.level".into(), "info".into()),
			],
			..Default::default()
		};
		let out = handle_config("acp.timeout", &ctx);
		assert_eq!(out.len(), 1);
		match &out[0] {
			CommandOutput::Table { headers, rows } => {
				assert_eq!(headers, &["Key", "Value"]);
				assert_eq!(rows.len(), 1);
				assert_eq!(rows[0], vec!["acp.timeout", "60"]);
			}
			other => panic!("expected Table, got {:?}", other),
		}
	}

	#[test]
	fn config_with_key_not_found() {
		let ctx = CommandContext {
			config_values: vec![("acp.timeout".into(), "60".into())],
			..Default::default()
		};
		let out = handle_config("nonexistent.key", &ctx);
		assert_eq!(out.len(), 1);
		assert!(matches!(
			&out[0],
			CommandOutput::Error(msg) if msg.contains("nonexistent.key")
		));
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
	fn factory_reset_returns_confirm_action() {
		let out = handle_factory_reset("");
		assert_eq!(out.len(), 1);
		assert!(matches!(
			&out[0],
			CommandOutput::ConfirmAction {
				action: BridgeAction::FactoryReset,
				..
			}
		));
	}

	#[test]
	fn factory_reset_confirm_message_mentions_global() {
		let out = handle_factory_reset("");
		match &out[0] {
			CommandOutput::ConfirmAction { message, .. } => {
				assert!(
					message.contains("global"),
					"Confirm message should mention 'global', got: {message}"
				);
			}
			other => panic!("expected ConfirmAction, got {:?}", other),
		}
	}

	// ── /factory-reset-project ───────────────────────────

	#[test]
	fn factory_reset_project_returns_confirm_action() {
		let out = handle_factory_reset_project("");
		assert_eq!(out.len(), 1);
		assert!(matches!(
			&out[0],
			CommandOutput::ConfirmAction {
				action: BridgeAction::FactoryResetProject,
				..
			}
		));
	}

	#[test]
	fn factory_reset_project_confirm_message_mentions_project() {
		let out = handle_factory_reset_project("");
		match &out[0] {
			CommandOutput::ConfirmAction { message, .. } => {
				assert!(
					message.contains("project"),
					"Confirm message should mention 'project', got: {message}"
				);
			}
			other => panic!("expected ConfirmAction, got {:?}", other),
		}
	}

}
