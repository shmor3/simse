//! SimseTestHarness — TestBackend-based end-to-end test harness for simse-tui.
//!
//! Uses ratatui's `TestBackend` to drive the `App` model through its Elm
//! Architecture (`update()` / `view()`) without needing a real terminal or PTY.
//! This is cross-platform and fast, while testing all the same business logic.

use ratatui::backend::TestBackend;
use ratatui::Terminal;
use simse_tui::app::{update, view, App, AppMessage, Screen};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default terminal width for tests.
const TERM_COLS: u16 = 120;
/// Default terminal height for tests.
const TERM_ROWS: u16 = 30;

// ---------------------------------------------------------------------------
// SimseTestHarness
// ---------------------------------------------------------------------------

/// End-to-end test harness that drives simse-tui's Elm Architecture model.
pub struct SimseTestHarness {
	/// The App model.
	pub app: App,
	/// The ratatui terminal with a TestBackend.
	terminal: Terminal<TestBackend>,
}

impl SimseTestHarness {
	/// Create a new harness with a fresh App and render the initial frame.
	pub fn new() -> Self {
		let backend = TestBackend::new(TERM_COLS, TERM_ROWS);
		let mut terminal = Terminal::new(backend).unwrap();
		let app = App::new();
		terminal
			.draw(|frame| view(&app, frame))
			.unwrap();
		Self { app, terminal }
	}

	// -------------------------------------------------------------------
	// Input methods
	// -------------------------------------------------------------------

	/// Type text character by character, re-rendering after each.
	pub fn type_text(&mut self, text: &str) {
		for c in text.chars() {
			self.send(AppMessage::CharInput(c));
		}
	}

	/// Type text and then press Enter.
	pub fn submit(&mut self, text: &str) {
		self.type_text(text);
		self.send(AppMessage::Submit);
	}

	/// Send a message to the app and re-render.
	pub fn send(&mut self, msg: AppMessage) {
		let app = std::mem::replace(&mut self.app, App::new());
		self.app = update(app, msg);
		self.render();
	}

	/// Press Enter.
	pub fn press_enter(&mut self) {
		self.send(AppMessage::Submit);
	}

	/// Press Escape.
	pub fn press_escape(&mut self) {
		self.send(AppMessage::Escape);
	}

	/// Press Tab.
	pub fn press_tab(&mut self) {
		self.send(AppMessage::Tab);
	}

	/// Press Shift+Tab.
	pub fn press_shift_tab(&mut self) {
		self.send(AppMessage::ShiftTab);
	}

	/// Press Backspace.
	pub fn press_backspace(&mut self) {
		self.send(AppMessage::Backspace);
	}

	/// Press Delete.
	pub fn press_delete(&mut self) {
		self.send(AppMessage::Delete);
	}

	/// Press Up arrow.
	pub fn press_up(&mut self) {
		self.send(AppMessage::HistoryUp);
	}

	/// Press Down arrow.
	pub fn press_down(&mut self) {
		self.send(AppMessage::HistoryDown);
	}

	/// Press Left arrow.
	pub fn press_left(&mut self) {
		self.send(AppMessage::CursorLeft);
	}

	/// Press Right arrow.
	pub fn press_right(&mut self) {
		self.send(AppMessage::CursorRight);
	}

	/// Press Ctrl+C.
	pub fn press_ctrl_c(&mut self) {
		self.send(AppMessage::CtrlC);
	}

	/// Press Ctrl+L.
	pub fn press_ctrl_l(&mut self) {
		self.send(AppMessage::CtrlL);
	}

	/// Press PageUp.
	pub fn press_page_up(&mut self) {
		self.send(AppMessage::ScrollUp(10));
	}

	/// Press PageDown.
	pub fn press_page_down(&mut self) {
		self.send(AppMessage::ScrollDown(10));
	}

	/// Press Home.
	pub fn press_home(&mut self) {
		self.send(AppMessage::Home);
	}

	/// Press End.
	pub fn press_end(&mut self) {
		self.send(AppMessage::End);
	}

	/// Delete word backwards (Alt+Backspace).
	pub fn press_delete_word_back(&mut self) {
		self.send(AppMessage::DeleteWordBack);
	}

	/// Paste text.
	pub fn paste(&mut self, text: &str) {
		self.send(AppMessage::Paste(text.to_string()));
	}

	// -------------------------------------------------------------------
	// Screen reading
	// -------------------------------------------------------------------

	/// Re-render the current app state.
	pub fn render(&mut self) {
		let app = &self.app;
		self.terminal
			.draw(|frame| view(app, frame))
			.unwrap();
	}

	/// Get the current visible screen text as a single string.
	pub fn screen_text(&self) -> String {
		let buffer = self.terminal.backend().buffer();
		let mut text = String::new();
		for y in 0..buffer.area.height {
			for x in 0..buffer.area.width {
				let cell = &buffer[(x, y)];
				text.push_str(cell.symbol());
			}
			text.push('\n');
		}
		text
	}

	// -------------------------------------------------------------------
	// Assertions
	// -------------------------------------------------------------------

	/// Assert screen contains text (panics with screen dump on failure).
	pub fn assert_contains(&self, text: &str) {
		let screen = self.screen_text();
		assert!(
			screen.contains(text),
			"Expected screen to contain {:?}, but screen was:\n{}",
			text,
			screen,
		);
	}

	/// Assert screen does NOT contain text.
	pub fn assert_not_contains(&self, text: &str) {
		let screen = self.screen_text();
		assert!(
			!screen.contains(text),
			"Expected screen NOT to contain {:?}, but screen was:\n{}",
			text,
			screen,
		);
	}

	/// Check if screen contains text (non-panicking).
	pub fn contains(&self, text: &str) -> bool {
		self.screen_text().contains(text)
	}

	/// Get the current input value.
	pub fn input_value(&self) -> &str {
		&self.app.input.value
	}

	/// Get the current screen.
	pub fn current_screen(&self) -> &Screen {
		&self.app.screen
	}

	/// Check if the app wants to quit.
	pub fn should_quit(&self) -> bool {
		self.app.should_quit
	}

	// -------------------------------------------------------------------
	// Lifecycle
	// -------------------------------------------------------------------

	/// Gracefully quit by sending exit command.
	pub fn quit(&mut self) {
		self.submit("exit");
	}
}
