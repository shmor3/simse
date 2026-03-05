//! PTY tests for tool commands (`/tools`, `/agents`, `/skills`, `/prompts`, `/chain`).
//!
//! These tests verify observable screen output through the real binary.
//! With no ACP server connected, these commands show "empty" feedback.

use super::r#mod::*;
use std::time::Duration;
use tempfile::TempDir;

/// Small delay to let the PTY propagate key events and re-render.
fn settle() {
	std::thread::sleep(Duration::from_millis(300));
}

// ═══════════════════════════════════════════════════════════════
// 1. /tools shows "No tools registered"
// ═══════════════════════════════════════════════════════════════

#[test]
fn tools_shows_no_tools() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/tools");
	settle();

	h.wait_for_text("No tools registered")
		.expect("'/tools' with no ACP server should show 'No tools registered'");
}

// ═══════════════════════════════════════════════════════════════
// 2. /agents shows "No agents configured"
// ═══════════════════════════════════════════════════════════════

#[test]
fn agents_shows_no_agents() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/agents");
	settle();

	h.wait_for_text("No agents configured")
		.expect("'/agents' with no agents should show 'No agents configured'");
}

// ═══════════════════════════════════════════════════════════════
// 3. /skills shows "No skills configured"
// ═══════════════════════════════════════════════════════════════

#[test]
fn skills_shows_no_skills() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/skills");
	settle();

	h.wait_for_text("No skills configured")
		.expect("'/skills' with no skills should show 'No skills configured'");
}

// ═══════════════════════════════════════════════════════════════
// 4. /prompts shows "No prompt templates"
// ═══════════════════════════════════════════════════════════════

#[test]
fn prompts_shows_no_prompts() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/prompts");
	settle();

	h.wait_for_text("No prompt templates")
		.expect("'/prompts' with no prompts should show 'No prompt templates'");
}

// ═══════════════════════════════════════════════════════════════
// 5. /chain shows "Running chain: summarize"
// ═══════════════════════════════════════════════════════════════

#[test]
fn chain_shows_feedback() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/chain summarize");
	settle();

	h.wait_for_text("Running chain")
		.expect("'/chain summarize' should show 'Running chain' feedback");
}
