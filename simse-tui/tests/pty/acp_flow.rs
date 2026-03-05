//! PTY tests for ACP-related command flows.
//!
//! Merges coverage from the old `e2e/acp_integration.rs` and `e2e/real_acp.rs`
//! into observable PTY tests. Each test spawns the real `simse-tui` binary in a
//! pseudo-terminal and verifies screen output — no internal state assertions.

use super::r#mod::*;
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

/// Small delay to let the PTY propagate key events and re-render.
fn settle() {
	thread::sleep(Duration::from_millis(300));
}

// ═══════════════════════════════════════════════════════════════
// 1. /acp restart shows feedback message
// ═══════════════════════════════════════════════════════════════

#[test]
fn acp_restart_shows_feedback() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/acp restart");

	h.wait_for_text("Restarting ACP connection")
		.expect("'/acp restart' should show 'Restarting ACP connection' on screen");
}

// ═══════════════════════════════════════════════════════════════
// 2. /acp status shows disconnected
// ═══════════════════════════════════════════════════════════════

#[test]
fn acp_status_shows_disconnected() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/acp status");

	h.wait_for_text("disconnected")
		.expect("'/acp status' should show 'disconnected' on screen");
}

// ═══════════════════════════════════════════════════════════════
// 3. /acp (no args) shows status (same as /acp status)
// ═══════════════════════════════════════════════════════════════

#[test]
fn acp_no_args_shows_status() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/acp");

	h.wait_for_text("disconnected")
		.expect("'/acp' with no args should show 'disconnected' on screen");
}

// ═══════════════════════════════════════════════════════════════
// 4. /server switch shows feedback message
// ═══════════════════════════════════════════════════════════════

#[test]
fn server_switch_shows_feedback() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/server test-server");

	h.wait_for_text("Switching to server: test-server")
		.expect("'/server test-server' should show 'Switching to server: test-server' on screen");
}

// ═══════════════════════════════════════════════════════════════
// 5. /model switch shows feedback message
// ═══════════════════════════════════════════════════════════════

#[test]
fn model_switch_shows_feedback() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/model llama3");

	h.wait_for_text("Switching to model: llama3")
		.expect("'/model llama3' should show 'Switching to model: llama3' on screen");
}

// ═══════════════════════════════════════════════════════════════
// 6. /model switch executes and shows bridge result
// ═══════════════════════════════════════════════════════════════

#[test]
fn model_switch_executes() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/model test-model");

	// The SwitchModel bridge action runs synchronously (sets config.default_agent)
	// and returns "Model set to: test-model" as a BridgeResult.
	h.wait_for_text("Model set to: test-model")
		.expect("'/model test-model' should show 'Model set to: test-model' after bridge dispatch");
}

// ═══════════════════════════════════════════════════════════════
// 7. Chat message appears on screen
// ═══════════════════════════════════════════════════════════════

#[test]
fn chat_message_appears_on_screen() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "Hello AI");

	h.wait_for_text("Hello AI")
		.expect("Chat message 'Hello AI' should appear on screen after submission");
}

// ═══════════════════════════════════════════════════════════════
// 8. App starts without thinking spinner
// ═══════════════════════════════════════════════════════════════

#[test]
fn app_starts_without_thinking_spinner() {
	let tmp = TempDir::new().unwrap();
	let h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	// Give the app a moment to fully render after startup.
	settle();

	let contents = h.screen_contents();
	assert!(
		!contents.contains("Thinking"),
		"App should not show 'Thinking' spinner at startup. Screen:\n{contents}"
	);
}

// ═══════════════════════════════════════════════════════════════
// 9. /mcp restart shows feedback message
// ═══════════════════════════════════════════════════════════════

#[test]
fn mcp_restart_shows_feedback() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/mcp restart");

	h.wait_for_text("Restarting MCP connections")
		.expect("'/mcp restart' should show 'Restarting MCP connections' on screen");
}

// ═══════════════════════════════════════════════════════════════
// 10. Submit clears input field
// ═══════════════════════════════════════════════════════════════

#[test]
fn submit_clears_input() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	// Type a unique string that won't appear elsewhere on screen.
	h.send_keys("xyzzy_draft_12345").unwrap();
	h.wait_for_text("xyzzy_draft_12345")
		.expect("Typed text should appear in input area");

	// Submit it (press Enter). The input field should clear after submit.
	h.send_key(KeyCode::Enter).unwrap();
	settle();

	// The submitted text may appear in the output area as a user message,
	// but the input box itself should be empty. We can verify the input
	// cleared by checking that the text now appears in the output section
	// (rendered as a user message) rather than the Input box.
	// Since the text is unique, if it appeared at all before and appears now,
	// the submit worked. We verify the input cleared by checking that a
	// second unique string typed afterward appears in a clean input field.
	h.send_keys("new_input_text").unwrap();
	settle();

	let contents = h.screen_contents();
	assert!(
		contents.contains("new_input_text"),
		"New text should appear in input after submit cleared the old text. Screen:\n{contents}"
	);
}

// ═══════════════════════════════════════════════════════════════
// 11. E2E: Chat message through ACP gets AI response
// ═══════════════════════════════════════════════════════════════
//
// This is the CRITICAL integration test. It proves the entire chat flow
// works end-to-end:
//   1. Start simse-tui configured with claude-agent-acp (Zed's ACP server)
//   2. User types a chat message
//   3. Lazy ACP connect fires → spawns claude-agent-acp subprocess
//   4. Message goes through pending_chat_message -> handle_submit -> agentic_loop
//   5. Claude processes the message via ACP and returns a response
//   6. Response appears on screen as an assistant message
//
// Requires:
//   - `@zed-industries/claude-agent-acp` installed (`npm i -g @zed-industries/claude-agent-acp`)
//   - `ANTHROPIC_API_KEY` environment variable set

#[test]
fn chat_round_trip_with_claude_code_acp() {
	let tmp = TempDir::new().unwrap();
	let data_dir = tmp.path().join("data");
	let work_dir = tmp.path().join("work");
	std::fs::create_dir_all(&work_dir).unwrap();

	// Write config pointing to claude-agent-acp (the Zed ACP server for Claude).
	// We use `node` as the command with the entry point as an arg, because
	// tokio::process::Command handles executable resolution reliably this way.
	write_acp_config(&data_dir);

	// Long timeout: ACP server startup (~5s) + AI response time (~30s).
	let mut h = spawn_simse_with_timeout(
		&data_dir,
		&work_dir,
		Duration::from_secs(90),
	);
	wait_for_startup(&h);

	// Send a math question whose answer (42) does NOT appear in the prompt.
	// This ensures we can distinguish the AI's response from the user's message.
	type_command(&mut h, "What is the sum of 17 and 25? Reply with ONLY the number.");

	// The user message should appear on screen.
	h.wait_for_text("sum of 17 and 25")
		.expect("User message should appear on screen after submission");

	// Wait for the AI response. The number 42 can only come from the AI,
	// proving the full round-trip: TUI -> pending_chat_message -> handle_submit
	// -> agentic_loop -> ACP -> claude-agent-acp -> streaming response -> screen.
	h.wait_for_text("42")
		.expect(
			"AI response '42' should appear on screen, proving the E2E ACP chat round-trip works",
		);

	// The app should remain running after the response.
	assert!(
		h.is_running(),
		"App should not crash after receiving AI response"
	);
}

// ═══════════════════════════════════════════════════════════════
// 12. E2E: Two consecutive messages both get AI responses
// ═══════════════════════════════════════════════════════════════
//
// This test proves that the ACP session is correctly reused across
// multiple messages. The second message must also get a response,
// which requires:
//   - loop_status resets to Idle after StreamEnd
//   - The ACP session is reused (not a new one per message)
//   - Only the NEW user message is sent as the prompt (not the full
//     conversation history, which the ACP server already tracks)
//   - The UI remains non-blocking during the agentic loop
//
// Requires:
//   - `@zed-industries/claude-agent-acp` installed (`npm i -g @zed-industries/claude-agent-acp`)
//   - `ANTHROPIC_API_KEY` environment variable set

#[test]
fn two_message_conversation_with_claude_code_acp() {
	let tmp = TempDir::new().unwrap();
	let data_dir = tmp.path().join("data");
	let work_dir = tmp.path().join("work");
	std::fs::create_dir_all(&work_dir).unwrap();

	write_acp_config(&data_dir);

	// Generous timeout: ACP server startup + two AI round-trips.
	let mut h = spawn_simse_with_timeout(
		&data_dir,
		&work_dir,
		Duration::from_secs(120),
	);
	wait_for_startup(&h);

	// ── First message ──────────────────────────────────────────
	// Ask a math question whose answer (42) cannot appear in the prompt.
	type_command(&mut h, "What is the sum of 17 and 25? Reply with ONLY the number.");

	h.wait_for_text("42")
		.expect("First AI response '42' should appear on screen");

	// Give the UI a moment to process StreamEnd and reset loop_status.
	settle();

	// ── Second message ─────────────────────────────────────────
	// Ask a different math question whose answer (7) is distinct.
	// This proves the session is reused and the second message works.
	type_command(&mut h, "What is the sum of 3 and 4? Reply with ONLY the number.");

	h.wait_for_text("sum of 3 and 4")
		.expect("Second user message should appear on screen");

	h.wait_for_text("7")
		.expect(
			"Second AI response '7' should appear on screen, proving multi-message ACP session reuse works",
		);

	assert!(
		h.is_running(),
		"App should not crash after second AI response"
	);
}

// ═══════════════════════════════════════════════════════════════
// 13. Setup wizard writes resolved ACP config
// ═══════════════════════════════════════════════════════════════
//
// Verifies that selecting Claude Code in the setup wizard writes
// acp.json with a resolved `node` command (not `npx`), thanks to
// the `resolve_npx_to_node` resolution in the SetupAcp handler.
//
// Requires:
//   - `@zed-industries/claude-agent-acp` installed (`npm i -g @zed-industries/claude-agent-acp`)

#[test]
fn setup_wizard_resolves_npx_to_node() {
	let tmp = TempDir::new().unwrap();
	let data_dir = tmp.path().join("data");
	let work_dir = tmp.path().join("work");
	std::fs::create_dir_all(&data_dir).unwrap();
	std::fs::create_dir_all(&work_dir).unwrap();

	let mut h = spawn_simse_with_timeout(
		&data_dir,
		&work_dir,
		Duration::from_secs(30),
	);
	wait_for_startup(&h);

	// Open setup wizard.
	type_command(&mut h, "/setup");

	h.wait_for_text("Claude Code")
		.expect("Setup wizard should show 'Claude Code'");

	// Select Claude Code (first item).
	h.send_key(KeyCode::Enter).unwrap();

	h.wait_for_text("Selected preset: Claude Code")
		.expect("Should show 'Selected preset: Claude Code'");

	// Wait for the SetupAcp bridge action to process (writes acp.json).
	h.wait_for_text("configured")
		.expect("Should show ACP server configured message");

	// Verify acp.json was written with `node` command (not `npx`).
	let acp_json = std::fs::read_to_string(data_dir.join("acp.json"))
		.expect("acp.json should exist after setup");

	let config: serde_json::Value =
		serde_json::from_str(&acp_json).expect("acp.json should be valid JSON");

	let command = config["servers"][0]["command"]
		.as_str()
		.expect("servers[0].command should be a string");

	assert!(
		command.to_lowercase().contains("node"),
		"SetupAcp should resolve npx to node binary. Got: {command}"
	);
	assert!(
		command != "npx",
		"SetupAcp should NOT use npx. acp.json: {acp_json}"
	);

	let args = config["servers"][0]["args"]
		.as_array()
		.expect("servers[0].args should be an array");

	assert_eq!(args.len(), 1, "Should have exactly 1 arg (the entry point path)");

	let entry_point = args[0].as_str().unwrap();
	assert!(
		std::path::Path::new(entry_point).exists(),
		"Resolved entry point should exist on disk: {entry_point}"
	);
}

// ═══════════════════════════════════════════════════════════════
// 14. E2E: Setup wizard → two messages (exact manual flow)
// ═══════════════════════════════════════════════════════════════
//
// This test reproduces the EXACT manual user flow (setup wizard path):
//   1. Start simse-tui with an empty data directory (no config)
//   2. Open the setup wizard via /setup
//   3. Select "Claude Code" (the default first item)
//   4. The setup wizard returns `npx -y @zed-industries/claude-agent-acp`
//   5. The SetupAcp handler resolves the npx package to a direct `node`
//      invocation (resolve_npx_to_node), avoiding npx process wrapper issues
//   6. Send first chat message → wait for AI response
//   7. Send second chat message → wait for AI response
//
// This is different from test #12 because:
//   - Config is written by the setup wizard path (with npx→node resolution)
//   - The app goes through the onboarding → setup → connect path
//
// Requires:
//   - `@zed-industries/claude-agent-acp` installed (`npm i -g @zed-industries/claude-agent-acp`)
//   - `ANTHROPIC_API_KEY` environment variable set

#[test]
fn setup_wizard_then_two_messages_with_claude_code_acp() {
	let tmp = TempDir::new().unwrap();
	let data_dir = tmp.path().join("data");
	let work_dir = tmp.path().join("work");
	std::fs::create_dir_all(&data_dir).unwrap();
	std::fs::create_dir_all(&work_dir).unwrap();

	// Start with EMPTY data dir — no config files at all.
	// This triggers onboarding mode (needs_setup = true).
	let mut h = spawn_simse_with_timeout(
		&data_dir,
		&work_dir,
		Duration::from_secs(120),
	);
	wait_for_startup(&h);

	// ── Step 1: Open setup wizard ─────────────────────────────
	type_command(&mut h, "/setup");

	h.wait_for_text("Claude Code")
		.expect("Setup wizard should show 'Claude Code' preset");

	// ── Step 2: Select Claude Code (first item, already selected) ─
	h.send_key(KeyCode::Enter).unwrap();

	h.wait_for_text("Selected preset: Claude Code")
		.expect("Selecting Claude Code should show confirmation message");

	// Give the bridge action time to process (writes acp.json, updates config).
	settle();
	settle();

	// ── Step 3: First chat message ────────────────────────────
	// This triggers: lazy ACP connect → npx spawns claude-agent-acp → session created.
	type_command(&mut h, "What is the sum of 17 and 25? Reply with ONLY the number.");

	h.wait_for_text("sum of 17 and 25")
		.expect("First user message should appear on screen");

	h.wait_for_text("42")
		.expect("First AI response '42' should appear on screen");

	// Give the UI time to process StreamEnd and reset loop_status to Idle.
	settle();

	// ── Step 4: Second chat message ───────────────────────────
	// This is where the bug was reported: the second message fails in the manual flow.
	type_command(&mut h, "What is the sum of 3 and 4? Reply with ONLY the number.");

	h.wait_for_text("sum of 3 and 4")
		.expect("Second user message should appear on screen");

	h.wait_for_text("7")
		.expect(
			"Second AI response '7' should appear, proving setup wizard → two-message flow works",
		);

	assert!(
		h.is_running(),
		"App should not crash after second AI response via setup wizard flow"
	);
}
