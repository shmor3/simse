//! PTY tests for session commands (`/sessions`, `/resume`, `/rename`, `/mcp status`).
//!
//! Deduplication notes:
//!   - `/sessions` (no sessions) -- already in command_feedback.rs as `empty_sessions_shows_guidance`.
//!   - `/server`, `/model`, `/acp restart` -- already in acp_flow.rs.
//!   - `/mcp restart` -- already in acp_flow.rs.
//!
//! Remaining tests here:
//!   1. `/resume sess-1`  -- shows "Resuming session..."
//!   2. `/rename Cool Name` -- shows "Renaming session to: Cool Name"
//!   3. `/mcp status` -- shows "MCP status"

use super::r#mod::*;
use std::time::Duration;
use tempfile::TempDir;

/// Small delay to let the PTY propagate key events and re-render.
fn settle() {
	std::thread::sleep(Duration::from_millis(300));
}

// ═══════════════════════════════════════════════════════════════
// 1. /resume shows "Resuming session..."
// ═══════════════════════════════════════════════════════════════

#[test]
fn resume_shows_feedback() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/resume sess-1");
	settle();

	h.wait_for_text("Resuming session")
		.expect("'/resume sess-1' should show 'Resuming session' feedback");
}

// ═══════════════════════════════════════════════════════════════
// 2. /rename shows "Renaming session to: Cool Name"
// ═══════════════════════════════════════════════════════════════

#[test]
fn rename_shows_feedback() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/rename Cool Name");
	settle();

	h.wait_for_text("Renaming session to")
		.expect("'/rename Cool Name' should show 'Renaming session to' feedback");
}

// ═══════════════════════════════════════════════════════════════
// 3. /mcp status shows MCP status info
// ═══════════════════════════════════════════════════════════════

#[test]
fn mcp_status_shows_info() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/mcp status");
	settle();

	h.wait_for_text("MCP status")
		.expect("'/mcp status' should show 'MCP status' feedback");
}
