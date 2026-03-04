//! E2E tests: setup wizard flow.
//!
//! Exercises the setup selector overlay through the full App model's
//! `update()` → `view()` cycle — preset listing, selection, navigation,
//! custom edit mode, and dismissal.

use simse_tui::app::Screen;

use crate::harness::SimseTestHarness;

// ═══════════════════════════════════════════════════════════════
// 1. Setup shows preset list
// ═══════════════════════════════════════════════════════════════

#[test]
fn setup_shows_preset_list() {
	let mut h = SimseTestHarness::new();
	h.submit("/setup");

	assert_eq!(h.current_screen(), &Screen::Setup { preset: None });

	// All four presets should be visible on screen.
	h.assert_contains("Claude Code");
	h.assert_contains("Ollama");
	h.assert_contains("Copilot");
	h.assert_contains("Custom");
}

// ═══════════════════════════════════════════════════════════════
// 2. Select Claude Code preset
// ═══════════════════════════════════════════════════════════════

#[test]
fn setup_select_claude_preset() {
	let mut h = SimseTestHarness::new();
	h.submit("/setup");

	assert_eq!(h.current_screen(), &Screen::Setup { preset: None });

	// Claude Code is index 0 (default selected). Press Enter to select it.
	assert_eq!(h.app.setup_state.selected, 0);
	h.press_enter();

	// handle_setup_action for SelectPreset transitions to Chat and pushes output.
	assert_eq!(h.current_screen(), &Screen::Chat);
	h.assert_contains("Selected preset: Claude Code");
}

// ═══════════════════════════════════════════════════════════════
// 3. Select Ollama preset
// ═══════════════════════════════════════════════════════════════

#[test]
fn setup_select_ollama_preset() {
	let mut h = SimseTestHarness::new();
	h.submit("/setup");

	assert_eq!(h.current_screen(), &Screen::Setup { preset: None });

	// Navigate down to Ollama (index 1).
	h.press_down();
	assert_eq!(h.app.setup_state.selected, 1);

	// Press Enter: OpenOllamaWizard action pushes info text.
	h.press_enter();

	// The screen stays on Setup (OpenOllamaWizard does not transition to Chat).
	assert_eq!(h.current_screen(), &Screen::Setup { preset: None });
	h.assert_contains("Opening Ollama wizard");
}

// ═══════════════════════════════════════════════════════════════
// 4. Select Copilot preset
// ═══════════════════════════════════════════════════════════════

#[test]
fn setup_select_copilot_preset() {
	let mut h = SimseTestHarness::new();
	h.submit("/setup");

	assert_eq!(h.current_screen(), &Screen::Setup { preset: None });

	// Navigate to Copilot (index 2).
	h.press_down();
	h.press_down();
	assert_eq!(h.app.setup_state.selected, 2);

	h.press_enter();

	// handle_setup_action for SelectPreset transitions to Chat.
	assert_eq!(h.current_screen(), &Screen::Chat);
	h.assert_contains("Selected preset: Copilot");
}

// ═══════════════════════════════════════════════════════════════
// 5. Select Custom preset — enters custom edit mode
// ═══════════════════════════════════════════════════════════════

#[test]
fn setup_select_custom_preset() {
	let mut h = SimseTestHarness::new();
	h.submit("/setup");

	assert_eq!(h.current_screen(), &Screen::Setup { preset: None });

	// Navigate to Custom (index 3).
	h.press_down();
	h.press_down();
	h.press_down();
	assert_eq!(h.app.setup_state.selected, 3);

	h.press_enter();

	// EnterCustomEdit keeps the Setup screen but activates custom editing.
	assert_eq!(h.current_screen(), &Screen::Setup { preset: None });
	assert!(h.app.setup_state.editing_custom);
	assert_eq!(h.app.setup_state.editing_field, 0);

	// The rendered overlay should show "Setup: Custom" title and field labels.
	h.assert_contains("Setup: Custom");
	h.assert_contains("Command:");
}

// ═══════════════════════════════════════════════════════════════
// 6. Ollama wizard flow — select Ollama and verify wizard info
// ═══════════════════════════════════════════════════════════════

#[test]
fn setup_ollama_wizard_flow() {
	let mut h = SimseTestHarness::new();
	h.submit("/setup");

	// Navigate to Ollama (index 1).
	h.press_down();
	assert_eq!(h.app.setup_state.selected, 1);

	// First Enter opens Ollama wizard.
	h.press_enter();
	h.assert_contains("Opening Ollama wizard");

	// Still on Setup screen after wizard trigger.
	assert_eq!(h.current_screen(), &Screen::Setup { preset: None });

	// Pressing Enter again while still on Ollama triggers the action again.
	h.press_enter();

	// Escape dismisses the setup overlay entirely (back from preset list).
	h.press_escape();
	assert_eq!(h.current_screen(), &Screen::Chat);
}

// ═══════════════════════════════════════════════════════════════
// 7. Back navigation — Down then Up changes selection
// ═══════════════════════════════════════════════════════════════

#[test]
fn setup_back_navigation() {
	let mut h = SimseTestHarness::new();
	h.submit("/setup");

	assert_eq!(h.current_screen(), &Screen::Setup { preset: None });

	// Start at position 0.
	assert_eq!(h.app.setup_state.selected, 0);

	// Navigate down twice to position 2.
	h.press_down();
	h.press_down();
	assert_eq!(h.app.setup_state.selected, 2);

	// Navigate up once to position 1.
	h.press_up();
	assert_eq!(h.app.setup_state.selected, 1);

	// Navigate up again to position 0.
	h.press_up();
	assert_eq!(h.app.setup_state.selected, 0);

	// Navigate up at 0 stays at 0 (clamped).
	h.press_up();
	assert_eq!(h.app.setup_state.selected, 0);
}

// ═══════════════════════════════════════════════════════════════
// 8. Custom edit — type a command and verify state
// ═══════════════════════════════════════════════════════════════

#[test]
fn setup_custom_edit_command() {
	let mut h = SimseTestHarness::new();
	h.submit("/setup");

	// Navigate to Custom and enter edit mode.
	h.press_down();
	h.press_down();
	h.press_down();
	h.press_enter();

	assert!(h.app.setup_state.editing_custom);
	assert_eq!(h.app.setup_state.editing_field, 0);

	// Type a command. In Setup screen, CharInput routes to setup_state.type_char.
	h.type_text("my-bridge");

	assert_eq!(h.app.setup_state.custom_command, "my-bridge");
	// Input field should remain empty (chars routed to overlay, not input).
	assert_eq!(h.input_value(), "");

	// The typed command should be rendered on screen.
	h.assert_contains("my-bridge");
}

// ═══════════════════════════════════════════════════════════════
// 9. Tab toggles field between command and args in custom edit
// ═══════════════════════════════════════════════════════════════

#[test]
fn setup_tab_toggles_field() {
	let mut h = SimseTestHarness::new();
	h.submit("/setup");

	// Navigate to Custom and enter edit mode.
	h.press_down();
	h.press_down();
	h.press_down();
	h.press_enter();

	assert!(h.app.setup_state.editing_custom);
	assert_eq!(h.app.setup_state.editing_field, 0); // starts on command

	// Tab should toggle to args field.
	h.press_tab();
	assert_eq!(h.app.setup_state.editing_field, 1);

	// Type into args field.
	h.type_text("--port 3000");
	assert_eq!(h.app.setup_state.custom_args, "--port 3000");
	// Command should still be empty since we typed into args.
	assert_eq!(h.app.setup_state.custom_command, "");

	// Tab again toggles back to command field.
	h.press_tab();
	assert_eq!(h.app.setup_state.editing_field, 0);

	// Type into command field now.
	h.type_text("acp-server");
	assert_eq!(h.app.setup_state.custom_command, "acp-server");
}

// ═══════════════════════════════════════════════════════════════
// 10. Escape from setup returns to Chat
// ═══════════════════════════════════════════════════════════════

#[test]
fn setup_escape_returns_to_chat() {
	let mut h = SimseTestHarness::new();
	h.submit("/setup");

	assert_eq!(h.current_screen(), &Screen::Setup { preset: None });

	// Escape from preset list mode dismisses the overlay.
	h.press_escape();
	assert_eq!(h.current_screen(), &Screen::Chat);

	// Verify we are back to normal Chat — the input field works again.
	h.type_text("hello");
	assert_eq!(h.input_value(), "hello");
}
