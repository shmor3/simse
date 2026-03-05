//! Shared PTY test helpers for simse-tui integration tests.
//!
//! All tests spawn the real `simse-tui` binary in a pseudo-terminal via
//! `ratatui-testlib`. This means tests exercise the exact same code path
//! as production: main.rs -> TuiRuntime -> AcpClient -> real ACP servers.

use std::path::Path;
use std::time::Duration;
use ratatui_testlib::{TuiTestHarness, CommandBuilder, KeyCode, Modifiers};

/// Spawn the real simse-tui binary with an isolated data directory.
pub fn spawn_simse(data_dir: &Path) -> TuiTestHarness {
    let mut harness = TuiTestHarness::new(120, 40)
        .expect("Failed to create PTY harness")
        .with_timeout(Duration::from_secs(10));

    let binary = env!("CARGO_BIN_EXE_simse-tui");
    let mut cmd = CommandBuilder::new(binary);
    cmd.arg("--data-dir");
    cmd.arg(data_dir.to_str().expect("data_dir must be valid UTF-8"));
    harness.spawn(cmd).expect("Failed to spawn simse-tui");
    harness
}

/// Spawn simse-tui with a pre-configured data directory.
pub fn spawn_simse_configured(data_dir: &Path) -> TuiTestHarness {
    std::fs::create_dir_all(data_dir).unwrap();
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
    spawn_simse(data_dir)
}

/// Type a command string and press Enter.
pub fn type_command(h: &mut TuiTestHarness, cmd: &str) {
    h.send_keys(cmd).expect("Failed to send keys");
    h.send_key(KeyCode::Enter).expect("Failed to send Enter");
}

/// Send Ctrl+C.
pub fn send_ctrl_c(h: &mut TuiTestHarness) {
    h.send_key_with_modifiers(KeyCode::Char('c'), Modifiers::CTRL)
        .expect("Failed to send Ctrl+C");
}

/// Send Ctrl+L.
pub fn send_ctrl_l(h: &mut TuiTestHarness) {
    h.send_key_with_modifiers(KeyCode::Char('l'), Modifiers::CTRL)
        .expect("Failed to send Ctrl+L");
}

/// Send Escape.
pub fn send_escape(h: &mut TuiTestHarness) {
    h.send_key(KeyCode::Esc).expect("Failed to send Escape");
}

/// Wait for the app to fully start (banner visible).
pub fn wait_for_startup(h: &mut TuiTestHarness) {
    h.wait_for_text("SimSE")
        .expect("App did not start — 'SimSE' not found on screen");
}
