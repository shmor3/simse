//! Setup selector: overlay for choosing an ACP provider preset.
//!
//! Presents a list of preset configurations and an optional custom input mode.
//!
//! # Layout
//!
//! ```text
//! +-- Setup ----------------------------------------+
//! |                                                  |
//! |  > Claude Code   Claude Code ACP bridge          |
//! |    Ollama        Local Ollama server              |
//! |    Copilot       GitHub Copilot bridge            |
//! |    Custom        Manual command + args            |
//! |                                                  |
//! |  up/dn navigate  enter select  esc dismiss       |
//! +--------------------------------------------------+
//! ```
//!
//! When "Custom" is selected and Enter is pressed, the overlay switches to
//! an inline editor for command and args fields.
//!
//! ```text
//! +-- Setup: Custom --------------------------------+
//! |                                                  |
//! |  Command: my-acp-bridge|                         |
//! |  Args:    --port 3000                            |
//! |                                                  |
//! |  tab switch field  enter confirm  esc back       |
//! +--------------------------------------------------+
//! ```
//!
//! All config writing happens externally; this module is UI-only.

use ratatui::{
	layout::Rect,
	style::{Color, Modifier, Style},
	text::{Line, Span},
	widgets::{Block, Borders, Clear, Paragraph, Wrap},
	Frame,
};

// ── Constants ───────────────────────────────────────────

/// Maximum width of the setup selector popup.
const MAX_POPUP_WIDTH: u16 = 56;

/// Minimum width of the setup selector popup.
const MIN_POPUP_WIDTH: u16 = 34;

/// Preset entries: (label, description).
const PRESETS: &[(&str, &str)] = &[
	("Claude Code", "Claude Code ACP bridge"),
	("Ollama", "Local Ollama server"),
	("Copilot", "GitHub Copilot bridge"),
	("Custom", "Manual command + args"),
];

// ── SetupPreset ─────────────────────────────────────────

/// Available setup presets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetupPreset {
	/// Claude Code ACP bridge.
	ClaudeCode,
	/// Local Ollama server.
	Ollama,
	/// GitHub Copilot bridge.
	Copilot,
	/// Manual command + args.
	Custom,
}

impl SetupPreset {
	/// Return the preset for a given index in the PRESETS list.
	fn from_index(index: usize) -> Self {
		match index {
			0 => Self::ClaudeCode,
			1 => Self::Ollama,
			2 => Self::Copilot,
			_ => Self::Custom,
		}
	}

	/// Return the display label for the preset.
	pub fn label(&self) -> &'static str {
		match self {
			Self::ClaudeCode => "Claude Code",
			Self::Ollama => "Ollama",
			Self::Copilot => "Copilot",
			Self::Custom => "Custom",
		}
	}

	/// Return the default ACP server name, command, and args for this preset.
	///
	/// Returns `None` for presets that need additional input (Custom, Ollama).
	pub fn acp_defaults(&self) -> Option<(&'static str, &'static str, &'static [&'static str])> {
		match self {
			Self::ClaudeCode => Some((
				"claude-agent-acp",
				"npx",
				&["-y", "@zed-industries/claude-agent-acp"],
			)),
			Self::Copilot => Some((
				"copilot-acp",
				"npx",
				&["-y", "@anthropic-ai/copilot-acp"],
			)),
			_ => None,
		}
	}
}

// ── SetupAction ─────────────────────────────────────────

/// Actions returned by `SetupSelectorState::enter()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetupAction {
	/// A preset was selected (not Ollama, not Custom).
	SelectPreset(SetupPreset),
	/// Ollama was selected — the caller should open the Ollama wizard.
	OpenOllamaWizard,
	/// Custom was selected — the overlay has entered inline edit mode.
	EnterCustomEdit,
	/// Custom edit was confirmed with the given command and args.
	ConfirmCustom {
		/// The command to run.
		command: String,
		/// Space-separated arguments.
		args: String,
	},
	/// No meaningful action (e.g., already in custom edit and not confirmed).
	None,
}

// ── SetupSelectorState ──────────────────────────────────

/// State for the setup selector overlay.
#[derive(Debug, Clone)]
pub struct SetupSelectorState {
	/// Index of the currently highlighted preset (0..PRESETS.len()).
	pub selected: usize,
	/// The custom command string (edited when `editing_custom` is true).
	pub custom_command: String,
	/// The custom args string (edited when `editing_custom` is true).
	pub custom_args: String,
	/// Whether the overlay is in custom-edit mode.
	pub editing_custom: bool,
	/// Which field is being edited: 0 = command, 1 = args.
	pub editing_field: usize,
}

impl SetupSelectorState {
	/// Create a new setup selector state pointing at the first preset.
	pub fn new() -> Self {
		Self {
			selected: 0,
			custom_command: String::new(),
			custom_args: String::new(),
			editing_custom: false,
			editing_field: 0,
		}
	}

	/// Move the selection up by one.
	pub fn move_up(&mut self) {
		if !self.editing_custom && self.selected > 0 {
			self.selected -= 1;
		}
	}

	/// Move the selection down by one.
	pub fn move_down(&mut self) {
		if !self.editing_custom && self.selected + 1 < PRESETS.len() {
			self.selected += 1;
		}
	}

	/// Handle Enter.
	///
	/// In preset list mode, selects the highlighted preset.
	/// In custom edit mode, confirms the custom command + args.
	pub fn enter(&mut self) -> SetupAction {
		if self.editing_custom {
			if self.custom_command.is_empty() {
				return SetupAction::None;
			}
			return SetupAction::ConfirmCustom {
				command: self.custom_command.clone(),
				args: self.custom_args.clone(),
			};
		}

		let preset = SetupPreset::from_index(self.selected);
		match preset {
			SetupPreset::Ollama => SetupAction::OpenOllamaWizard,
			SetupPreset::Custom => {
				self.editing_custom = true;
				self.editing_field = 0;
				SetupAction::EnterCustomEdit
			}
			other => SetupAction::SelectPreset(other),
		}
	}

	/// Type a character into the active custom-edit field.
	pub fn type_char(&mut self, c: char) {
		if !self.editing_custom {
			return;
		}
		match self.editing_field {
			0 => self.custom_command.push(c),
			_ => self.custom_args.push(c),
		}
	}

	/// Backspace in the active custom-edit field.
	pub fn backspace(&mut self) {
		if !self.editing_custom {
			return;
		}
		match self.editing_field {
			0 => {
				self.custom_command.pop();
			}
			_ => {
				self.custom_args.pop();
			}
		}
	}

	/// Handle back / Esc.
	///
	/// If in custom-edit mode, returns to the preset list.
	/// If in preset list mode, returns `true` to signal dismissal.
	pub fn back(&mut self) -> bool {
		if self.editing_custom {
			self.editing_custom = false;
			false
		} else {
			// Signal dismissal.
			true
		}
	}

	/// Switch the editing field in custom-edit mode (Tab key).
	///
	/// Toggles between command (0) and args (1).
	pub fn toggle_field(&mut self) {
		if self.editing_custom {
			self.editing_field = if self.editing_field == 0 { 1 } else { 0 };
		}
	}

	/// Return a reference to the currently selected preset.
	pub fn selected_preset(&self) -> SetupPreset {
		SetupPreset::from_index(self.selected)
	}
}

impl Default for SetupSelectorState {
	fn default() -> Self {
		Self::new()
	}
}

// ── Rendering ───────────────────────────────────────────

/// Render the setup selector as a centered overlay popup.
pub fn render_setup_selector(frame: &mut Frame, area: Rect, state: &SetupSelectorState) {
	let mut lines: Vec<Line<'static>> = Vec::new();

	// Blank line for padding.
	lines.push(Line::from(""));

	if state.editing_custom {
		render_custom_edit(&mut lines, state);
	} else {
		render_preset_list(&mut lines, state);
	}

	// Blank separator.
	lines.push(Line::from(""));

	// Key hints.
	render_key_hints(&mut lines, state);

	// Trailing padding.
	lines.push(Line::from(""));

	// Build title.
	let title = if state.editing_custom {
		" Setup: Custom ".to_string()
	} else {
		" Setup ".to_string()
	};

	// Calculate popup dimensions.
	let content_height = lines.len() as u16 + 2; // +2 for border top/bottom
	let available_width = area.width.saturating_sub(4);
	let popup_width = MAX_POPUP_WIDTH
		.min(available_width)
		.max(MIN_POPUP_WIDTH)
		.min(area.width);
	let popup_height = content_height
		.min(area.height.saturating_sub(2))
		.min(area.height);

	// Center the popup.
	let popup_x = (area.width.saturating_sub(popup_width)) / 2;
	let popup_y = (area.height.saturating_sub(popup_height)) / 2;
	let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

	// Clear the area behind the popup, then render.
	frame.render_widget(Clear, popup_area);

	let border_color = if state.editing_custom {
		Color::Yellow
	} else {
		Color::Cyan
	};

	let popup = Paragraph::new(lines)
		.wrap(Wrap { trim: false })
		.block(
			Block::default()
				.borders(Borders::ALL)
				.border_style(Style::default().fg(border_color))
				.title(title),
		);

	frame.render_widget(popup, popup_area);
}

/// Render the preset list.
fn render_preset_list(lines: &mut Vec<Line<'static>>, state: &SetupSelectorState) {
	for (i, (label, desc)) in PRESETS.iter().enumerate() {
		let selected = i == state.selected;
		let prefix = if selected { "  \u{276f} " } else { "    " };
		let color = if selected { Color::Cyan } else { Color::Reset };
		let mut style = Style::default().fg(color);
		if selected {
			style = style.add_modifier(Modifier::BOLD);
		}

		lines.push(Line::from(vec![
			Span::styled(format!("{prefix}{label}"), style),
			Span::styled(
				format!("  {desc}"),
				Style::default().fg(Color::DarkGray),
			),
		]));
	}
}

/// Render the custom command + args editor.
fn render_custom_edit(lines: &mut Vec<Line<'static>>, state: &SetupSelectorState) {
	let cmd_active = state.editing_field == 0;
	let args_active = state.editing_field == 1;

	let cursor = Span::styled(
		"\u{2588}",
		Style::default()
			.fg(Color::White)
			.add_modifier(Modifier::SLOW_BLINK),
	);

	// Command field.
	let cmd_label_style = if cmd_active {
		Style::default()
			.fg(Color::Cyan)
			.add_modifier(Modifier::BOLD)
	} else {
		Style::default().fg(Color::DarkGray)
	};
	let cmd_value = if state.custom_command.is_empty() && cmd_active {
		vec![
			Span::styled("  Command: ", cmd_label_style),
			cursor.clone(),
		]
	} else {
		let mut spans = vec![
			Span::styled("  Command: ", cmd_label_style),
			Span::styled(
				state.custom_command.clone(),
				Style::default().fg(Color::White),
			),
		];
		if cmd_active {
			spans.push(cursor.clone());
		}
		spans
	};
	lines.push(Line::from(cmd_value));

	// Args field.
	let args_label_style = if args_active {
		Style::default()
			.fg(Color::Cyan)
			.add_modifier(Modifier::BOLD)
	} else {
		Style::default().fg(Color::DarkGray)
	};
	let args_value = if state.custom_args.is_empty() && args_active {
		vec![
			Span::styled("  Args:    ", args_label_style),
			cursor,
		]
	} else {
		let mut spans = vec![
			Span::styled("  Args:    ", args_label_style),
			Span::styled(
				state.custom_args.clone(),
				Style::default().fg(Color::White),
			),
		];
		if args_active {
			spans.push(cursor);
		}
		spans
	};
	lines.push(Line::from(args_value));
}

/// Render key hints at the bottom of the overlay.
fn render_key_hints(lines: &mut Vec<Line<'static>>, state: &SetupSelectorState) {
	let dim = Style::default().fg(Color::DarkGray);
	let bold_dim = Style::default()
		.fg(Color::DarkGray)
		.add_modifier(Modifier::BOLD);

	let mut spans = Vec::new();
	spans.push(Span::raw("  "));

	if state.editing_custom {
		spans.push(Span::styled("tab", bold_dim));
		spans.push(Span::styled(" switch field  ", dim));
		spans.push(Span::styled("\u{21b5}", bold_dim));
		spans.push(Span::styled(" confirm  ", dim));
		spans.push(Span::styled("esc", bold_dim));
		spans.push(Span::styled(" back", dim));
	} else {
		spans.push(Span::styled("\u{2191}\u{2193}", bold_dim));
		spans.push(Span::styled(" navigate  ", dim));
		spans.push(Span::styled("\u{21b5}", bold_dim));
		spans.push(Span::styled(" select  ", dim));
		spans.push(Span::styled("esc", bold_dim));
		spans.push(Span::styled(" dismiss", dim));
	}

	lines.push(Line::from(spans));
}

// ── Tests ───────────────────────────────────────────────

#[cfg(test)]
mod tests {
	use super::*;

	// ── SetupSelectorState::new ────────────────────

	#[test]
	fn setup_new_defaults() {
		let state = SetupSelectorState::new();
		assert_eq!(state.selected, 0);
		assert!(state.custom_command.is_empty());
		assert!(state.custom_args.is_empty());
		assert!(!state.editing_custom);
		assert_eq!(state.editing_field, 0);
	}

	#[test]
	fn setup_default_equals_new() {
		let a = SetupSelectorState::new();
		let b = SetupSelectorState::default();
		assert_eq!(a.selected, b.selected);
		assert_eq!(a.editing_custom, b.editing_custom);
		assert_eq!(a.editing_field, b.editing_field);
	}

	// ── SetupPreset ────────────────────────────────

	#[test]
	fn setup_preset_from_index() {
		assert_eq!(SetupPreset::from_index(0), SetupPreset::ClaudeCode);
		assert_eq!(SetupPreset::from_index(1), SetupPreset::Ollama);
		assert_eq!(SetupPreset::from_index(2), SetupPreset::Copilot);
		assert_eq!(SetupPreset::from_index(3), SetupPreset::Custom);
		assert_eq!(SetupPreset::from_index(99), SetupPreset::Custom);
	}

	#[test]
	fn setup_preset_label() {
		assert_eq!(SetupPreset::ClaudeCode.label(), "Claude Code");
		assert_eq!(SetupPreset::Ollama.label(), "Ollama");
		assert_eq!(SetupPreset::Copilot.label(), "Copilot");
		assert_eq!(SetupPreset::Custom.label(), "Custom");
	}

	// ── move_up / move_down ────────────────────────

	#[test]
	fn setup_move_up_clamps_at_zero() {
		let mut state = SetupSelectorState::new();
		state.move_up();
		assert_eq!(state.selected, 0);
	}

	#[test]
	fn setup_move_down_increments() {
		let mut state = SetupSelectorState::new();
		state.move_down();
		assert_eq!(state.selected, 1);
		state.move_down();
		assert_eq!(state.selected, 2);
	}

	#[test]
	fn setup_move_down_clamps_at_last() {
		let mut state = SetupSelectorState::new();
		for _ in 0..20 {
			state.move_down();
		}
		assert_eq!(state.selected, PRESETS.len() - 1);
	}

	#[test]
	fn setup_move_up_decrements() {
		let mut state = SetupSelectorState::new();
		state.selected = 3;
		state.move_up();
		assert_eq!(state.selected, 2);
	}

	#[test]
	fn setup_move_up_ignored_in_custom_edit() {
		let mut state = SetupSelectorState::new();
		state.selected = 2;
		state.editing_custom = true;
		state.move_up();
		assert_eq!(state.selected, 2);
	}

	#[test]
	fn setup_move_down_ignored_in_custom_edit() {
		let mut state = SetupSelectorState::new();
		state.editing_custom = true;
		state.move_down();
		assert_eq!(state.selected, 0);
	}

	// ── enter ──────────────────────────────────────

	#[test]
	fn setup_enter_claude_code() {
		let mut state = SetupSelectorState::new();
		state.selected = 0;
		let action = state.enter();
		assert_eq!(action, SetupAction::SelectPreset(SetupPreset::ClaudeCode));
		assert!(!state.editing_custom);
	}

	#[test]
	fn setup_enter_ollama() {
		let mut state = SetupSelectorState::new();
		state.selected = 1;
		let action = state.enter();
		assert_eq!(action, SetupAction::OpenOllamaWizard);
		assert!(!state.editing_custom);
	}

	#[test]
	fn setup_enter_copilot() {
		let mut state = SetupSelectorState::new();
		state.selected = 2;
		let action = state.enter();
		assert_eq!(action, SetupAction::SelectPreset(SetupPreset::Copilot));
		assert!(!state.editing_custom);
	}

	#[test]
	fn setup_enter_custom_enters_edit_mode() {
		let mut state = SetupSelectorState::new();
		state.selected = 3;
		let action = state.enter();
		assert_eq!(action, SetupAction::EnterCustomEdit);
		assert!(state.editing_custom);
		assert_eq!(state.editing_field, 0);
	}

	#[test]
	fn setup_enter_custom_confirm_with_command() {
		let mut state = SetupSelectorState::new();
		state.editing_custom = true;
		state.custom_command = "my-bridge".to_string();
		state.custom_args = "--port 3000".to_string();
		let action = state.enter();
		assert_eq!(
			action,
			SetupAction::ConfirmCustom {
				command: "my-bridge".to_string(),
				args: "--port 3000".to_string(),
			}
		);
	}

	#[test]
	fn setup_enter_custom_confirm_empty_command_returns_none() {
		let mut state = SetupSelectorState::new();
		state.editing_custom = true;
		let action = state.enter();
		assert_eq!(action, SetupAction::None);
	}

	// ── type_char / backspace ──────────────────────

	#[test]
	fn setup_type_char_command_field() {
		let mut state = SetupSelectorState::new();
		state.editing_custom = true;
		state.editing_field = 0;
		state.type_char('a');
		state.type_char('b');
		assert_eq!(state.custom_command, "ab");
		assert!(state.custom_args.is_empty());
	}

	#[test]
	fn setup_type_char_args_field() {
		let mut state = SetupSelectorState::new();
		state.editing_custom = true;
		state.editing_field = 1;
		state.type_char('x');
		state.type_char('y');
		assert!(state.custom_command.is_empty());
		assert_eq!(state.custom_args, "xy");
	}

	#[test]
	fn setup_type_char_ignored_when_not_editing() {
		let mut state = SetupSelectorState::new();
		state.type_char('z');
		assert!(state.custom_command.is_empty());
		assert!(state.custom_args.is_empty());
	}

	#[test]
	fn setup_backspace_command_field() {
		let mut state = SetupSelectorState::new();
		state.editing_custom = true;
		state.editing_field = 0;
		state.custom_command = "abc".to_string();
		state.backspace();
		assert_eq!(state.custom_command, "ab");
	}

	#[test]
	fn setup_backspace_args_field() {
		let mut state = SetupSelectorState::new();
		state.editing_custom = true;
		state.editing_field = 1;
		state.custom_args = "xyz".to_string();
		state.backspace();
		assert_eq!(state.custom_args, "xy");
	}

	#[test]
	fn setup_backspace_on_empty_is_noop() {
		let mut state = SetupSelectorState::new();
		state.editing_custom = true;
		state.editing_field = 0;
		state.backspace();
		assert!(state.custom_command.is_empty());
	}

	#[test]
	fn setup_backspace_ignored_when_not_editing() {
		let mut state = SetupSelectorState::new();
		state.custom_command = "abc".to_string();
		state.backspace();
		assert_eq!(state.custom_command, "abc");
	}

	// ── back ───────────────────────────────────────

	#[test]
	fn setup_back_from_custom_edit_returns_to_list() {
		let mut state = SetupSelectorState::new();
		state.editing_custom = true;
		let dismiss = state.back();
		assert!(!dismiss);
		assert!(!state.editing_custom);
	}

	#[test]
	fn setup_back_from_list_signals_dismiss() {
		let mut state = SetupSelectorState::new();
		let dismiss = state.back();
		assert!(dismiss);
	}

	// ── toggle_field ───────────────────────────────

	#[test]
	fn setup_toggle_field_switches_0_to_1() {
		let mut state = SetupSelectorState::new();
		state.editing_custom = true;
		state.editing_field = 0;
		state.toggle_field();
		assert_eq!(state.editing_field, 1);
	}

	#[test]
	fn setup_toggle_field_switches_1_to_0() {
		let mut state = SetupSelectorState::new();
		state.editing_custom = true;
		state.editing_field = 1;
		state.toggle_field();
		assert_eq!(state.editing_field, 0);
	}

	#[test]
	fn setup_toggle_field_ignored_when_not_editing() {
		let mut state = SetupSelectorState::new();
		state.editing_field = 0;
		state.toggle_field();
		assert_eq!(state.editing_field, 0);
	}

	// ── selected_preset ────────────────────────────

	#[test]
	fn setup_selected_preset_returns_correct_preset() {
		let mut state = SetupSelectorState::new();
		assert_eq!(state.selected_preset(), SetupPreset::ClaudeCode);
		state.selected = 1;
		assert_eq!(state.selected_preset(), SetupPreset::Ollama);
		state.selected = 2;
		assert_eq!(state.selected_preset(), SetupPreset::Copilot);
		state.selected = 3;
		assert_eq!(state.selected_preset(), SetupPreset::Custom);
	}

	// ── Full workflow ──────────────────────────────

	#[test]
	fn setup_full_workflow_select_preset() {
		let mut state = SetupSelectorState::new();

		// Navigate to Copilot.
		state.move_down();
		state.move_down();
		assert_eq!(state.selected, 2);
		assert_eq!(state.selected_preset(), SetupPreset::Copilot);

		// Select it.
		let action = state.enter();
		assert_eq!(action, SetupAction::SelectPreset(SetupPreset::Copilot));
	}

	#[test]
	fn setup_full_workflow_custom_edit() {
		let mut state = SetupSelectorState::new();

		// Navigate to Custom.
		state.move_down();
		state.move_down();
		state.move_down();
		assert_eq!(state.selected, 3);

		// Enter custom edit.
		let action = state.enter();
		assert_eq!(action, SetupAction::EnterCustomEdit);
		assert!(state.editing_custom);

		// Type command.
		state.type_char('m');
		state.type_char('y');
		state.type_char('-');
		state.type_char('c');
		state.type_char('m');
		state.type_char('d');
		assert_eq!(state.custom_command, "my-cmd");

		// Switch to args.
		state.toggle_field();
		assert_eq!(state.editing_field, 1);

		// Type args.
		state.type_char('-');
		state.type_char('-');
		state.type_char('v');
		assert_eq!(state.custom_args, "--v");

		// Confirm.
		let action = state.enter();
		assert_eq!(
			action,
			SetupAction::ConfirmCustom {
				command: "my-cmd".to_string(),
				args: "--v".to_string(),
			}
		);
	}

	#[test]
	fn setup_full_workflow_custom_back() {
		let mut state = SetupSelectorState::new();

		// Navigate to Custom and enter.
		state.selected = 3;
		state.enter();
		assert!(state.editing_custom);

		// Back to list.
		let dismiss = state.back();
		assert!(!dismiss);
		assert!(!state.editing_custom);

		// Dismiss.
		let dismiss = state.back();
		assert!(dismiss);
	}

	// ── Render smoke tests ─────────────────────────

	#[test]
	fn render_setup_selector_preset_list_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let state = SetupSelectorState::new();

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_setup_selector(frame, area, &state);
			})
			.unwrap();
	}

	#[test]
	fn render_setup_selector_custom_edit_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut state = SetupSelectorState::new();
		state.editing_custom = true;
		state.custom_command = "my-bridge".to_string();
		state.custom_args = "--port 3000".to_string();

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_setup_selector(frame, area, &state);
			})
			.unwrap();
	}

	#[test]
	fn render_setup_selector_custom_edit_empty_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut state = SetupSelectorState::new();
		state.editing_custom = true;

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_setup_selector(frame, area, &state);
			})
			.unwrap();
	}

	#[test]
	fn render_setup_selector_small_terminal_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(30, 10);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let state = SetupSelectorState::new();

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_setup_selector(frame, area, &state);
			})
			.unwrap();
	}

	#[test]
	fn render_setup_selector_selected_last_item_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut state = SetupSelectorState::new();
		state.selected = PRESETS.len() - 1;

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_setup_selector(frame, area, &state);
			})
			.unwrap();
	}

	#[test]
	fn render_setup_selector_args_field_active_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut state = SetupSelectorState::new();
		state.editing_custom = true;
		state.editing_field = 1;
		state.custom_args = "some args".to_string();

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_setup_selector(frame, area, &state);
			})
			.unwrap();
	}
}
