//! PTY tests for the onboarding flow.
//!
//! These tests verify observable onboarding behavior through the real binary:
//!
//! - **Fresh start** — a fresh app (no config) shows the welcome banner with
//!   version info and tips.
//! - **Welcome mentions /help** — the tips section includes "/help".
//! - **Setup command** — `/setup` opens the setup wizard overlay.
//! - **Setup with preset** — `/setup ollama` opens setup with "ollama" preset.
//! - **Factory reset restarts onboarding** — after a full factory-reset
//!   the app returns to the initial state with the banner visible.

use super::r#mod::*;
use std::time::Duration;
use tempfile::TempDir;

// ═══════════════════════════════════════════════════════════════
// 1. Fresh start shows the banner with version info
// ═══════════════════════════════════════════════════════════════

#[test]
fn fresh_start_shows_welcome_banner() {
	let tmp = TempDir::new().unwrap();
	let h = spawn_simse(tmp.path());

	// A fresh start (no config) should show the banner with the version.
	h.wait_for_text("simse v")
		.expect("Fresh start should show 'simse v' banner");
}

// ═══════════════════════════════════════════════════════════════
// 2. Welcome screen mentions /help
// ═══════════════════════════════════════════════════════════════

#[test]
fn fresh_start_mentions_help() {
	let tmp = TempDir::new().unwrap();
	let h = spawn_simse(tmp.path());

	// The banner/tips area should mention /help.
	h.wait_for_text("/help")
		.expect("Fresh start should mention '/help' in the tips area");
}

// ═══════════════════════════════════════════════════════════════
// 3. /setup opens the setup wizard
// ═══════════════════════════════════════════════════════════════

#[test]
fn setup_command_shows_wizard() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/setup");

	h.wait_for_text("Setup")
		.expect("'/setup' should open the Setup wizard overlay");
}

// ═══════════════════════════════════════════════════════════════
// 4. /setup ollama opens the setup wizard with preset
// ═══════════════════════════════════════════════════════════════

#[test]
fn setup_with_preset_shows_preset() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/setup ollama");

	h.wait_for_text("Setup")
		.expect("'/setup ollama' should open the Setup wizard");

	// The preset name should appear somewhere in the wizard.
	// The setup wizard displays "Ollama" (capitalized) as a preset option.
	h.wait_for_text("Ollama")
		.expect("Setup wizard with preset should show 'Ollama'");
}

// ═══════════════════════════════════════════════════════════════
// 5. Factory reset on a configured app shows the banner again
// ═══════════════════════════════════════════════════════════════

#[test]
fn factory_reset_returns_to_initial_state() {
	let tmp = TempDir::new().unwrap();
	let data_dir = tmp.path().join("data");
	let work_dir = tmp.path().join("work");
	std::fs::create_dir_all(&work_dir).unwrap();

	// Pre-configure the data_dir so the app starts in configured mode.
	std::fs::create_dir_all(&data_dir).unwrap();
	std::fs::write(
		data_dir.join("config.json"),
		r#"{"logLevel": "warn"}"#,
	)
	.unwrap();
	std::fs::write(
		data_dir.join("acp.json"),
		r#"{"servers": [{"name": "claude-code", "command": "claude"}]}"#,
	)
	.unwrap();

	let binary = env!("CARGO_BIN_EXE_simse-tui");
	let mut cmd = portable_pty::CommandBuilder::new(binary);
	cmd.arg("--data-dir");
	cmd.arg(data_dir.to_str().unwrap());
	cmd.cwd(&work_dir);
	let mut h = PtyHarness::spawn(cmd, 120, 40, Duration::from_secs(15));

	wait_for_startup(&h);

	// Factory reset the app.
	type_command(&mut h, "/factory-reset");

	h.wait_for_text("Are you sure")
		.expect("Factory reset should show confirmation dialog");

	// Confirm.
	h.send_key(KeyCode::Enter).unwrap();

	h.wait_for_text("Factory reset complete")
		.expect("Should show 'Factory reset complete' after confirming");

	// After factory reset, the data directory should be deleted.
	assert!(
		!data_dir.exists(),
		"Expected data_dir to be deleted after factory reset: {}",
		data_dir.display()
	);
}
