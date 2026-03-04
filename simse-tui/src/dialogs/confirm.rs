//! Confirm dialog: centered overlay popup for destructive action confirmation.
//!
//! Displayed when the user is about to perform a destructive action (e.g. deleting
//! all global configs). The dialog requires the user to type "yes" before confirming.
//!
//! # Layout
//!
//! ```text
//! ┌─ Confirm ─────────────────────────────────────┐
//! │                                                │
//! │  ⚠ Delete all global configs?                  │
//! │                                                │
//! │  ❯ No, cancel                                  │
//! │    Yes, proceed                                │
//! │                                                │
//! │  ↑↓ navigate  ↵ select  esc cancel             │
//! └────────────────────────────────────────────────┘
//! ```
//!
//! When "Yes, proceed" is selected, a text input appears:
//!
//! ```text
//! ┌─ Confirm ─────────────────────────────────────┐
//! │                                                │
//! │  ⚠ Delete all global configs?                  │
//! │                                                │
//! │    No, cancel                                  │
//! │  ❯ Yes, proceed                                │
//! │                                                │
//! │  Type "yes" to confirm: yes█                   │
//! │                                                │
//! │  ↑↓ navigate  ↵ select  esc cancel             │
//! └────────────────────────────────────────────────┘
//! ```

use ratatui::{
	layout::Rect,
	style::{Color, Modifier, Style},
	text::{Line, Span},
	widgets::{Block, Borders, Clear, Paragraph, Wrap},
	Frame,
};

// ── Constants ───────────────────────────────────────────

/// Maximum width of the confirm dialog popup.
const MAX_POPUP_WIDTH: u16 = 50;

/// Minimum width of the confirm dialog popup.
const MIN_POPUP_WIDTH: u16 = 30;

// ── State ───────────────────────────────────────────────

/// State for the confirm dialog overlay.
///
/// Tracks which option is selected (0 = No, 1 = Yes) and the typed
/// confirmation input. The user must type "yes" with the Yes option
/// selected in order to confirm.
#[derive(Debug, Clone)]
pub struct ConfirmDialogState {
	/// The message displayed in the dialog header (e.g. "Delete all global configs?").
	pub message: String,
	/// Currently selected option: 0 = "No, cancel", 1 = "Yes, proceed".
	pub selected: usize,
	/// Text typed into the confirmation input (only active when selected == 1).
	pub yes_input: String,
}

impl ConfirmDialogState {
	/// Create a new confirm dialog state with the given message.
	///
	/// Defaults to selected=0 (No, cancel) and an empty yes_input.
	pub fn new(message: impl Into<String>) -> Self {
		Self {
			message: message.into(),
			selected: 0,
			yes_input: String::new(),
		}
	}

	/// Move selection up (toward "No, cancel").
	///
	/// If already at the top, this is a no-op. Clears the yes_input when
	/// moving away from "Yes, proceed".
	pub fn move_up(&mut self) {
		if self.selected > 0 {
			self.selected -= 1;
			self.yes_input.clear();
		}
	}

	/// Move selection down (toward "Yes, proceed").
	///
	/// If already at the bottom, this is a no-op.
	pub fn move_down(&mut self) {
		if self.selected < 1 {
			self.selected += 1;
		}
	}

	/// Append a character to the yes_input buffer.
	///
	/// Only effective when "Yes, proceed" is selected (selected == 1).
	pub fn type_char(&mut self, c: char) {
		if self.selected == 1 {
			self.yes_input.push(c);
		}
	}

	/// Delete the last character from the yes_input buffer.
	///
	/// Only effective when "Yes, proceed" is selected (selected == 1).
	pub fn backspace(&mut self) {
		if self.selected == 1 {
			self.yes_input.pop();
		}
	}

	/// Returns `true` when the user can confirm: "Yes" is selected and
	/// the typed input matches "yes" (case-insensitive, trimmed).
	pub fn can_confirm(&self) -> bool {
		self.selected == 1 && self.yes_input.trim().eq_ignore_ascii_case("yes")
	}

	/// Returns `true` when "No, cancel" is selected.
	pub fn is_cancelled(&self) -> bool {
		self.selected == 0
	}
}

// ── Rendering ───────────────────────────────────────────

/// Render the confirm dialog as a centered overlay popup.
///
/// The dialog is rendered on top of whatever is behind it (using `Clear`).
/// It shows the message as a header, two selectable options, a text input
/// for typing "yes" when the Yes option is selected, and key hints.
pub fn render_confirm_dialog(frame: &mut Frame, area: Rect, state: &ConfirmDialogState) {
	let mut lines: Vec<Line<'static>> = Vec::new();

	// Blank line for padding.
	lines.push(Line::from(""));

	// Header: warning icon + message.
	lines.push(Line::from(vec![
		Span::styled(
			"  \u{26a0} ",
			Style::default()
				.fg(Color::Red)
				.add_modifier(Modifier::BOLD),
		),
		Span::styled(
			state.message.clone(),
			Style::default().add_modifier(Modifier::BOLD),
		),
	]));

	// Blank separator.
	lines.push(Line::from(""));

	// Option 0: "No, cancel" (default).
	let no_selected = state.selected == 0;
	let no_color = if no_selected { Color::Cyan } else { Color::Reset };
	let no_prefix = if no_selected { "  \u{276f} " } else { "    " };
	let mut no_style = Style::default().fg(no_color);
	if no_selected {
		no_style = no_style.add_modifier(Modifier::BOLD);
	}
	lines.push(Line::from(Span::styled(
		format!("{no_prefix}No, cancel"),
		no_style,
	)));

	// Option 1: "Yes, proceed".
	let yes_selected = state.selected == 1;
	let yes_color = if yes_selected { Color::Red } else { Color::Reset };
	let yes_prefix = if yes_selected { "  \u{276f} " } else { "    " };
	let mut yes_style = Style::default().fg(yes_color);
	if yes_selected {
		yes_style = yes_style.add_modifier(Modifier::BOLD);
	}
	lines.push(Line::from(Span::styled(
		format!("{yes_prefix}Yes, proceed"),
		yes_style,
	)));

	// Confirmation text input (only when "Yes" is selected).
	if yes_selected {
		lines.push(Line::from(""));

		let input_display = if state.yes_input.is_empty() {
			Span::styled("_", Style::default().fg(Color::DarkGray))
		} else {
			let color = if state.can_confirm() {
				Color::Green
			} else {
				Color::Yellow
			};
			Span::styled(state.yes_input.clone(), Style::default().fg(color))
		};

		lines.push(Line::from(vec![
			Span::styled(
				"  Type \"yes\" to confirm: ",
				Style::default().fg(Color::DarkGray),
			),
			input_display,
			Span::styled(
				"\u{2588}",
				Style::default()
					.fg(Color::White)
					.add_modifier(Modifier::SLOW_BLINK),
			),
		]));
	}

	// Blank separator.
	lines.push(Line::from(""));

	// Key hints.
	lines.push(Line::from(vec![
		Span::styled(
			"  \u{2191}\u{2193}",
			Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD),
		),
		Span::styled(" navigate  ", Style::default().fg(Color::DarkGray)),
		Span::styled(
			"\u{21b5}",
			Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD),
		),
		Span::styled(" select  ", Style::default().fg(Color::DarkGray)),
		Span::styled(
			"esc",
			Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD),
		),
		Span::styled(" cancel", Style::default().fg(Color::DarkGray)),
	]));

	// Trailing padding.
	lines.push(Line::from(""));

	// Calculate popup dimensions.
	let content_height = lines.len() as u16 + 2; // +2 for border top/bottom
	let popup_width = MAX_POPUP_WIDTH
		.min(area.width.saturating_sub(4))
		.max(MIN_POPUP_WIDTH);
	let popup_height = content_height.min(area.height.saturating_sub(4));

	// Center the popup.
	let popup_x = (area.width.saturating_sub(popup_width)) / 2;
	let popup_y = (area.height.saturating_sub(popup_height)) / 2;
	let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

	// Clear the area behind the popup, then render.
	frame.render_widget(Clear, popup_area);

	let border_color = if state.can_confirm() {
		Color::Green
	} else {
		Color::Red
	};

	let popup = Paragraph::new(lines)
		.wrap(Wrap { trim: false })
		.block(
			Block::default()
				.borders(Borders::ALL)
				.border_style(Style::default().fg(border_color))
				.title(" Confirm "),
		);

	frame.render_widget(popup, popup_area);
}

// ── Tests ───────────────────────────────────────────────

#[cfg(test)]
mod tests {
	use super::*;

	// ── ConfirmDialogState::new ─────────────────────

	#[test]
	fn new_defaults_to_no_selected() {
		let state = ConfirmDialogState::new("Delete everything?");
		assert_eq!(state.selected, 0);
		assert_eq!(state.message, "Delete everything?");
		assert!(state.yes_input.is_empty());
	}

	#[test]
	fn new_is_cancelled_by_default() {
		let state = ConfirmDialogState::new("Reset config?");
		assert!(state.is_cancelled());
		assert!(!state.can_confirm());
	}

	// ── move_up / move_down ─────────────────────────

	#[test]
	fn move_down_selects_yes() {
		let mut state = ConfirmDialogState::new("msg");
		assert_eq!(state.selected, 0);
		state.move_down();
		assert_eq!(state.selected, 1);
		assert!(!state.is_cancelled());
	}

	#[test]
	fn move_down_clamps_at_bottom() {
		let mut state = ConfirmDialogState::new("msg");
		state.move_down();
		state.move_down();
		state.move_down();
		assert_eq!(state.selected, 1);
	}

	#[test]
	fn move_up_selects_no() {
		let mut state = ConfirmDialogState::new("msg");
		state.move_down();
		assert_eq!(state.selected, 1);
		state.move_up();
		assert_eq!(state.selected, 0);
		assert!(state.is_cancelled());
	}

	#[test]
	fn move_up_clamps_at_top() {
		let mut state = ConfirmDialogState::new("msg");
		state.move_up();
		state.move_up();
		assert_eq!(state.selected, 0);
	}

	#[test]
	fn move_up_clears_yes_input() {
		let mut state = ConfirmDialogState::new("msg");
		state.move_down();
		state.type_char('y');
		state.type_char('e');
		assert_eq!(state.yes_input, "ye");
		state.move_up();
		assert!(state.yes_input.is_empty());
	}

	// ── type_char ───────────────────────────────────

	#[test]
	fn type_char_appends_when_yes_selected() {
		let mut state = ConfirmDialogState::new("msg");
		state.move_down();
		state.type_char('y');
		state.type_char('e');
		state.type_char('s');
		assert_eq!(state.yes_input, "yes");
	}

	#[test]
	fn type_char_ignored_when_no_selected() {
		let mut state = ConfirmDialogState::new("msg");
		state.type_char('y');
		state.type_char('e');
		state.type_char('s');
		assert!(state.yes_input.is_empty());
	}

	// ── backspace ───────────────────────────────────

	#[test]
	fn backspace_removes_last_char() {
		let mut state = ConfirmDialogState::new("msg");
		state.move_down();
		state.type_char('y');
		state.type_char('e');
		state.type_char('s');
		state.backspace();
		assert_eq!(state.yes_input, "ye");
	}

	#[test]
	fn backspace_on_empty_is_noop() {
		let mut state = ConfirmDialogState::new("msg");
		state.move_down();
		state.backspace();
		assert!(state.yes_input.is_empty());
	}

	#[test]
	fn backspace_ignored_when_no_selected() {
		let mut state = ConfirmDialogState::new("msg");
		// Put some text first, then move up, then try backspace on "No".
		state.move_down();
		state.type_char('y');
		state.move_up(); // clears yes_input
		state.backspace(); // should be noop
		assert!(state.yes_input.is_empty());
	}

	// ── can_confirm ─────────────────────────────────

	#[test]
	fn can_confirm_when_yes_typed() {
		let mut state = ConfirmDialogState::new("msg");
		state.move_down();
		state.type_char('y');
		state.type_char('e');
		state.type_char('s');
		assert!(state.can_confirm());
	}

	#[test]
	fn can_confirm_case_insensitive() {
		let mut state = ConfirmDialogState::new("msg");
		state.move_down();
		state.type_char('Y');
		state.type_char('E');
		state.type_char('S');
		assert!(state.can_confirm());
	}

	#[test]
	fn can_confirm_with_whitespace() {
		let mut state = ConfirmDialogState::new("msg");
		state.move_down();
		state.type_char(' ');
		state.type_char('y');
		state.type_char('e');
		state.type_char('s');
		state.type_char(' ');
		assert!(state.can_confirm());
	}

	#[test]
	fn cannot_confirm_with_partial_input() {
		let mut state = ConfirmDialogState::new("msg");
		state.move_down();
		state.type_char('y');
		state.type_char('e');
		assert!(!state.can_confirm());
	}

	#[test]
	fn cannot_confirm_with_wrong_input() {
		let mut state = ConfirmDialogState::new("msg");
		state.move_down();
		state.type_char('n');
		state.type_char('o');
		assert!(!state.can_confirm());
	}

	#[test]
	fn cannot_confirm_when_no_selected_even_with_yes_text() {
		// This tests an edge case: if somehow yes_input had "yes" but
		// selected is 0, can_confirm should return false.
		let mut state = ConfirmDialogState::new("msg");
		state.move_down();
		state.type_char('y');
		state.type_char('e');
		state.type_char('s');
		assert!(state.can_confirm());
		// Move up clears input, so we test with a manually set state.
		let manual = ConfirmDialogState {
			message: "msg".into(),
			selected: 0,
			yes_input: "yes".into(),
		};
		assert!(!manual.can_confirm());
	}

	#[test]
	fn cannot_confirm_with_empty_input() {
		let mut state = ConfirmDialogState::new("msg");
		state.move_down();
		assert!(!state.can_confirm());
	}

	// ── is_cancelled ────────────────────────────────

	#[test]
	fn is_cancelled_when_no_selected() {
		let state = ConfirmDialogState::new("msg");
		assert!(state.is_cancelled());
	}

	#[test]
	fn is_not_cancelled_when_yes_selected() {
		let mut state = ConfirmDialogState::new("msg");
		state.move_down();
		assert!(!state.is_cancelled());
	}

	// ── render_confirm_dialog (smoke tests) ─────────

	#[test]
	fn render_no_selected_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let state = ConfirmDialogState::new("Delete all global configs?");

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_confirm_dialog(frame, area, &state);
			})
			.unwrap();
	}

	#[test]
	fn render_yes_selected_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut state = ConfirmDialogState::new("Delete all global configs?");
		state.move_down();

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_confirm_dialog(frame, area, &state);
			})
			.unwrap();
	}

	#[test]
	fn render_with_typed_yes_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut state = ConfirmDialogState::new("Reset everything?");
		state.move_down();
		state.type_char('y');
		state.type_char('e');
		state.type_char('s');

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_confirm_dialog(frame, area, &state);
			})
			.unwrap();
	}

	#[test]
	fn render_small_terminal_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(30, 10);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let state = ConfirmDialogState::new("Delete?");

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_confirm_dialog(frame, area, &state);
			})
			.unwrap();
	}

	#[test]
	fn render_long_message_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let long_msg = "A".repeat(200);
		let state = ConfirmDialogState::new(long_msg);

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_confirm_dialog(frame, area, &state);
			})
			.unwrap();
	}

	#[test]
	fn render_yes_with_partial_input_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(60, 20);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut state = ConfirmDialogState::new("Clear history?");
		state.move_down();
		state.type_char('y');

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_confirm_dialog(frame, area, &state);
			})
			.unwrap();
	}

	// ── Full workflow integration ───────────────────

	#[test]
	fn full_confirm_workflow() {
		let mut state = ConfirmDialogState::new("Delete all data?");

		// Initial: No selected, cancelled.
		assert!(state.is_cancelled());
		assert!(!state.can_confirm());

		// Navigate to Yes.
		state.move_down();
		assert!(!state.is_cancelled());
		assert!(!state.can_confirm());

		// Type "yes".
		state.type_char('y');
		assert!(!state.can_confirm());
		state.type_char('e');
		assert!(!state.can_confirm());
		state.type_char('s');
		assert!(state.can_confirm());

		// Backspace revokes confirmation.
		state.backspace();
		assert!(!state.can_confirm());
		assert_eq!(state.yes_input, "ye");

		// Retype.
		state.type_char('s');
		assert!(state.can_confirm());
	}

	#[test]
	fn cancel_workflow() {
		let mut state = ConfirmDialogState::new("Delete?");

		// Navigate down to Yes and type something.
		state.move_down();
		state.type_char('y');

		// Navigate back up — should clear input.
		state.move_up();
		assert!(state.is_cancelled());
		assert!(state.yes_input.is_empty());

		// Cannot type when No is selected.
		state.type_char('y');
		assert!(state.yes_input.is_empty());
	}
}
