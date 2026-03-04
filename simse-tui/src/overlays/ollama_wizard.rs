//! Ollama wizard: multi-step overlay for configuring a local Ollama server.
//!
//! Three steps:
//! 1. **URL input** — enter the Ollama API URL (default: `http://localhost:11434`)
//! 2. **Model selection** — pick a model from a list fetched externally
//! 3. **Confirmation** — review and confirm the selection
//!
//! # Layout (URL input)
//!
//! ```text
//! +-- Ollama: URL ──────────────────────────────────+
//! |                                                  |
//! |  Enter the Ollama server URL:                    |
//! |                                                  |
//! |  URL: http://localhost:11434|                     |
//! |                                                  |
//! |  enter fetch models  esc cancel                  |
//! +--------------------------------------------------+
//! ```
//!
//! # Layout (Model selection)
//!
//! ```text
//! +-- Ollama: Model ────────────────────────────────+
//! |                                                  |
//! |  Select a model:                                 |
//! |                                                  |
//! |  > llama3.2                                      |
//! |    codellama                                     |
//! |    mistral                                       |
//! |                                                  |
//! |  up/dn navigate  enter select  esc back          |
//! +--------------------------------------------------+
//! ```
//!
//! # Layout (Confirm)
//!
//! ```text
//! +-- Ollama: Confirm ──────────────────────────────+
//! |                                                  |
//! |  URL:   http://localhost:11434                    |
//! |  Model: llama3.2                                 |
//! |                                                  |
//! |  enter confirm  esc back                         |
//! +--------------------------------------------------+
//! ```
//!
//! The actual HTTP fetch and config writing happen in the caller.

use ratatui::{
	layout::Rect,
	style::{Color, Modifier, Style},
	text::{Line, Span},
	widgets::{Block, Borders, Clear, Paragraph, Wrap},
	Frame,
};

// ── Constants ───────────────────────────────────────────

/// Maximum width of the Ollama wizard popup.
const MAX_POPUP_WIDTH: u16 = 56;

/// Minimum width of the Ollama wizard popup.
const MIN_POPUP_WIDTH: u16 = 34;

/// Default Ollama server URL.
const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";

// ── OllamaStep ──────────────────────────────────────────

/// Current step of the Ollama wizard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OllamaStep {
	/// Entering the Ollama server URL.
	UrlInput,
	/// Selecting a model from the fetched list.
	ModelSelect,
	/// Reviewing and confirming the selection.
	Confirm,
}

// ── OllamaAction ────────────────────────────────────────

/// Actions returned by `OllamaWizardState::enter()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OllamaAction {
	/// URL was confirmed — the caller should fetch the model list.
	FetchModels(String),
	/// A model was selected — advance to confirmation.
	SelectModel(String),
	/// The user confirmed the final selection.
	Confirm {
		/// The Ollama server URL.
		url: String,
		/// The selected model name.
		model: String,
	},
	/// No meaningful action.
	None,
}

// ── OllamaWizardState ──────────────────────────────────

/// State for the Ollama wizard overlay.
#[derive(Debug, Clone)]
pub struct OllamaWizardState {
	/// Current wizard step.
	pub step: OllamaStep,
	/// The Ollama server URL being edited.
	pub url: String,
	/// Model names available after fetching.
	pub models: Vec<String>,
	/// Index of the currently highlighted model.
	pub selected_model: usize,
	/// Optional error message to display.
	pub error: Option<String>,
}

impl OllamaWizardState {
	/// Create a new wizard state at the URL input step with the default URL.
	pub fn new() -> Self {
		Self {
			step: OllamaStep::UrlInput,
			url: DEFAULT_OLLAMA_URL.to_string(),
			models: Vec::new(),
			selected_model: 0,
			error: None,
		}
	}

	/// Populate the model list after an external fetch.
	///
	/// Clears any previous error and resets the selection to the first model.
	pub fn set_models(&mut self, models: Vec<String>) {
		self.models = models;
		self.selected_model = 0;
		self.error = None;
		self.step = OllamaStep::ModelSelect;
	}

	/// Set an error message to display.
	pub fn set_error(&mut self, msg: String) {
		self.error = Some(msg);
	}

	/// Move selection up in the ModelSelect step.
	pub fn move_up(&mut self) {
		if self.step == OllamaStep::ModelSelect && self.selected_model > 0 {
			self.selected_model -= 1;
		}
	}

	/// Move selection down in the ModelSelect step.
	pub fn move_down(&mut self) {
		if self.step == OllamaStep::ModelSelect && self.selected_model + 1 < self.models.len() {
			self.selected_model += 1;
		}
	}

	/// Handle Enter for the current step.
	pub fn enter(&mut self) -> OllamaAction {
		match self.step {
			OllamaStep::UrlInput => {
				if self.url.is_empty() {
					return OllamaAction::None;
				}
				OllamaAction::FetchModels(self.url.clone())
			}
			OllamaStep::ModelSelect => {
				if self.models.is_empty() {
					return OllamaAction::None;
				}
				let model = self.models[self.selected_model].clone();
				self.step = OllamaStep::Confirm;
				OllamaAction::SelectModel(model)
			}
			OllamaStep::Confirm => {
				if self.models.is_empty() {
					return OllamaAction::None;
				}
				OllamaAction::Confirm {
					url: self.url.clone(),
					model: self.models[self.selected_model].clone(),
				}
			}
		}
	}

	/// Type a character into the URL field (only in UrlInput step).
	pub fn type_char(&mut self, c: char) {
		if self.step == OllamaStep::UrlInput {
			self.url.push(c);
		}
	}

	/// Backspace in the URL field (only in UrlInput step).
	pub fn backspace(&mut self) {
		if self.step == OllamaStep::UrlInput {
			self.url.pop();
		}
	}

	/// Handle back / Esc.
	///
	/// Goes to the previous step, or returns `true` to signal dismissal
	/// from the UrlInput step.
	pub fn back(&mut self) -> bool {
		match self.step {
			OllamaStep::Confirm => {
				self.step = OllamaStep::ModelSelect;
				false
			}
			OllamaStep::ModelSelect => {
				self.step = OllamaStep::UrlInput;
				self.error = None;
				false
			}
			OllamaStep::UrlInput => {
				// Signal dismissal.
				true
			}
		}
	}

	/// Return the currently selected model name, if any.
	pub fn selected_model(&self) -> Option<&str> {
		self.models.get(self.selected_model).map(|s| s.as_str())
	}
}

impl Default for OllamaWizardState {
	fn default() -> Self {
		Self::new()
	}
}

// ── Rendering ───────────────────────────────────────────

/// Render the Ollama wizard as a centered overlay popup.
pub fn render_ollama_wizard(frame: &mut Frame, area: Rect, state: &OllamaWizardState) {
	let mut lines: Vec<Line<'static>> = Vec::new();

	// Blank line for padding.
	lines.push(Line::from(""));

	match state.step {
		OllamaStep::UrlInput => render_url_input(&mut lines, state),
		OllamaStep::ModelSelect => render_model_select(&mut lines, state),
		OllamaStep::Confirm => render_confirm(&mut lines, state),
	}

	// Error display.
	if let Some(ref err) = state.error {
		lines.push(Line::from(""));
		lines.push(Line::from(Span::styled(
			format!("  Error: {err}"),
			Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
		)));
	}

	// Blank separator.
	lines.push(Line::from(""));

	// Key hints.
	render_key_hints(&mut lines, state);

	// Trailing padding.
	lines.push(Line::from(""));

	// Build title.
	let title = match state.step {
		OllamaStep::UrlInput => " Ollama: URL ".to_string(),
		OllamaStep::ModelSelect => " Ollama: Model ".to_string(),
		OllamaStep::Confirm => " Ollama: Confirm ".to_string(),
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

	let border_color = match state.step {
		OllamaStep::UrlInput => Color::Cyan,
		OllamaStep::ModelSelect => Color::Blue,
		OllamaStep::Confirm => Color::Green,
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

/// Render the URL input step.
fn render_url_input(lines: &mut Vec<Line<'static>>, state: &OllamaWizardState) {
	lines.push(Line::from(Span::styled(
		"  Enter the Ollama server URL:",
		Style::default().fg(Color::White),
	)));
	lines.push(Line::from(""));

	let cursor = Span::styled(
		"\u{2588}",
		Style::default()
			.fg(Color::White)
			.add_modifier(Modifier::SLOW_BLINK),
	);

	let url_spans = if state.url.is_empty() {
		vec![
			Span::styled(
				"  URL: ",
				Style::default()
					.fg(Color::Cyan)
					.add_modifier(Modifier::BOLD),
			),
			cursor,
		]
	} else {
		vec![
			Span::styled(
				"  URL: ",
				Style::default()
					.fg(Color::Cyan)
					.add_modifier(Modifier::BOLD),
			),
			Span::styled(state.url.clone(), Style::default().fg(Color::White)),
			cursor,
		]
	};

	lines.push(Line::from(url_spans));
}

/// Render the model selection step.
fn render_model_select(lines: &mut Vec<Line<'static>>, state: &OllamaWizardState) {
	lines.push(Line::from(Span::styled(
		"  Select a model:",
		Style::default().fg(Color::White),
	)));
	lines.push(Line::from(""));

	if state.models.is_empty() {
		lines.push(Line::from(Span::styled(
			"    (no models found)",
			Style::default().fg(Color::DarkGray),
		)));
		return;
	}

	for (i, model) in state.models.iter().enumerate() {
		let selected = i == state.selected_model;
		let prefix = if selected { "  \u{276f} " } else { "    " };
		let color = if selected { Color::Cyan } else { Color::Reset };
		let mut style = Style::default().fg(color);
		if selected {
			style = style.add_modifier(Modifier::BOLD);
		}

		lines.push(Line::from(Span::styled(
			format!("{prefix}{model}"),
			style,
		)));
	}
}

/// Render the confirmation step.
fn render_confirm(lines: &mut Vec<Line<'static>>, state: &OllamaWizardState) {
	let model_name = state
		.selected_model()
		.unwrap_or("(none)")
		.to_string();

	lines.push(Line::from(vec![
		Span::styled(
			"  URL:   ",
			Style::default()
				.fg(Color::DarkGray)
				.add_modifier(Modifier::BOLD),
		),
		Span::styled(state.url.clone(), Style::default().fg(Color::White)),
	]));

	lines.push(Line::from(vec![
		Span::styled(
			"  Model: ",
			Style::default()
				.fg(Color::DarkGray)
				.add_modifier(Modifier::BOLD),
		),
		Span::styled(model_name, Style::default().fg(Color::Cyan)),
	]));
}

/// Render key hints at the bottom of the overlay.
fn render_key_hints(lines: &mut Vec<Line<'static>>, state: &OllamaWizardState) {
	let dim = Style::default().fg(Color::DarkGray);
	let bold_dim = Style::default()
		.fg(Color::DarkGray)
		.add_modifier(Modifier::BOLD);

	let mut spans = Vec::new();
	spans.push(Span::raw("  "));

	match state.step {
		OllamaStep::UrlInput => {
			spans.push(Span::styled("\u{21b5}", bold_dim));
			spans.push(Span::styled(" fetch models  ", dim));
			spans.push(Span::styled("esc", bold_dim));
			spans.push(Span::styled(" cancel", dim));
		}
		OllamaStep::ModelSelect => {
			spans.push(Span::styled("\u{2191}\u{2193}", bold_dim));
			spans.push(Span::styled(" navigate  ", dim));
			spans.push(Span::styled("\u{21b5}", bold_dim));
			spans.push(Span::styled(" select  ", dim));
			spans.push(Span::styled("esc", bold_dim));
			spans.push(Span::styled(" back", dim));
		}
		OllamaStep::Confirm => {
			spans.push(Span::styled("\u{21b5}", bold_dim));
			spans.push(Span::styled(" confirm  ", dim));
			spans.push(Span::styled("esc", bold_dim));
			spans.push(Span::styled(" back", dim));
		}
	}

	lines.push(Line::from(spans));
}

// ── Tests ───────────────────────────────────────────────

#[cfg(test)]
mod tests {
	use super::*;

	// ── OllamaWizardState::new ─────────────────────

	#[test]
	fn ollama_new_defaults() {
		let state = OllamaWizardState::new();
		assert_eq!(state.step, OllamaStep::UrlInput);
		assert_eq!(state.url, DEFAULT_OLLAMA_URL);
		assert!(state.models.is_empty());
		assert_eq!(state.selected_model, 0);
		assert!(state.error.is_none());
	}

	#[test]
	fn ollama_default_equals_new() {
		let a = OllamaWizardState::new();
		let b = OllamaWizardState::default();
		assert_eq!(a.step, b.step);
		assert_eq!(a.url, b.url);
		assert_eq!(a.models, b.models);
		assert_eq!(a.selected_model, b.selected_model);
	}

	// ── set_models ─────────────────────────────────

	#[test]
	fn ollama_set_models_populates_and_transitions() {
		let mut state = OllamaWizardState::new();
		state.set_models(vec!["llama3.2".to_string(), "codellama".to_string()]);
		assert_eq!(state.step, OllamaStep::ModelSelect);
		assert_eq!(state.models.len(), 2);
		assert_eq!(state.selected_model, 0);
		assert!(state.error.is_none());
	}

	#[test]
	fn ollama_set_models_clears_error() {
		let mut state = OllamaWizardState::new();
		state.set_error("connection refused".to_string());
		assert!(state.error.is_some());

		state.set_models(vec!["model1".to_string()]);
		assert!(state.error.is_none());
	}

	#[test]
	fn ollama_set_models_resets_selection() {
		let mut state = OllamaWizardState::new();
		state.set_models(vec!["a".to_string(), "b".to_string(), "c".to_string()]);
		state.selected_model = 2;
		state.set_models(vec!["x".to_string(), "y".to_string()]);
		assert_eq!(state.selected_model, 0);
	}

	// ── set_error ──────────────────────────────────

	#[test]
	fn ollama_set_error_stores_message() {
		let mut state = OllamaWizardState::new();
		state.set_error("timeout".to_string());
		assert_eq!(state.error, Some("timeout".to_string()));
	}

	// ── move_up / move_down ────────────────────────

	#[test]
	fn ollama_move_up_clamps_at_zero() {
		let mut state = OllamaWizardState::new();
		state.set_models(vec!["a".to_string(), "b".to_string()]);
		state.move_up();
		assert_eq!(state.selected_model, 0);
	}

	#[test]
	fn ollama_move_down_increments() {
		let mut state = OllamaWizardState::new();
		state.set_models(vec!["a".to_string(), "b".to_string(), "c".to_string()]);
		state.move_down();
		assert_eq!(state.selected_model, 1);
		state.move_down();
		assert_eq!(state.selected_model, 2);
	}

	#[test]
	fn ollama_move_down_clamps_at_last() {
		let mut state = OllamaWizardState::new();
		state.set_models(vec!["a".to_string(), "b".to_string()]);
		for _ in 0..10 {
			state.move_down();
		}
		assert_eq!(state.selected_model, 1);
	}

	#[test]
	fn ollama_move_ignored_in_url_input() {
		let mut state = OllamaWizardState::new();
		state.move_up();
		state.move_down();
		assert_eq!(state.selected_model, 0);
	}

	#[test]
	fn ollama_move_ignored_in_confirm() {
		let mut state = OllamaWizardState::new();
		state.set_models(vec!["a".to_string(), "b".to_string()]);
		state.selected_model = 1;
		state.step = OllamaStep::Confirm;
		state.move_up();
		assert_eq!(state.selected_model, 1);
		state.move_down();
		assert_eq!(state.selected_model, 1);
	}

	// ── enter ──────────────────────────────────────

	#[test]
	fn ollama_enter_url_input_fetches_models() {
		let mut state = OllamaWizardState::new();
		let action = state.enter();
		assert_eq!(
			action,
			OllamaAction::FetchModels(DEFAULT_OLLAMA_URL.to_string())
		);
	}

	#[test]
	fn ollama_enter_url_input_empty_url_returns_none() {
		let mut state = OllamaWizardState::new();
		state.url.clear();
		let action = state.enter();
		assert_eq!(action, OllamaAction::None);
	}

	#[test]
	fn ollama_enter_model_select_selects_model() {
		let mut state = OllamaWizardState::new();
		state.set_models(vec!["llama3.2".to_string(), "codellama".to_string()]);
		state.selected_model = 1;
		let action = state.enter();
		assert_eq!(
			action,
			OllamaAction::SelectModel("codellama".to_string())
		);
		assert_eq!(state.step, OllamaStep::Confirm);
	}

	#[test]
	fn ollama_enter_model_select_no_models_returns_none() {
		let mut state = OllamaWizardState::new();
		state.step = OllamaStep::ModelSelect;
		let action = state.enter();
		assert_eq!(action, OllamaAction::None);
	}

	#[test]
	fn ollama_enter_confirm_returns_confirm() {
		let mut state = OllamaWizardState::new();
		state.set_models(vec!["llama3.2".to_string()]);
		state.step = OllamaStep::Confirm;
		let action = state.enter();
		assert_eq!(
			action,
			OllamaAction::Confirm {
				url: DEFAULT_OLLAMA_URL.to_string(),
				model: "llama3.2".to_string(),
			}
		);
	}

	#[test]
	fn ollama_enter_confirm_no_models_returns_none() {
		let mut state = OllamaWizardState::new();
		state.step = OllamaStep::Confirm;
		let action = state.enter();
		assert_eq!(action, OllamaAction::None);
	}

	// ── type_char / backspace ──────────────────────

	#[test]
	fn ollama_type_char_appends_to_url() {
		let mut state = OllamaWizardState::new();
		state.url.clear();
		state.type_char('h');
		state.type_char('i');
		assert_eq!(state.url, "hi");
	}

	#[test]
	fn ollama_type_char_ignored_in_model_select() {
		let mut state = OllamaWizardState::new();
		state.set_models(vec!["a".to_string()]);
		let url_before = state.url.clone();
		state.type_char('z');
		assert_eq!(state.url, url_before);
	}

	#[test]
	fn ollama_type_char_ignored_in_confirm() {
		let mut state = OllamaWizardState::new();
		state.step = OllamaStep::Confirm;
		let url_before = state.url.clone();
		state.type_char('z');
		assert_eq!(state.url, url_before);
	}

	#[test]
	fn ollama_backspace_removes_last_from_url() {
		let mut state = OllamaWizardState::new();
		state.url = "http://localhost:1143".to_string();
		state.backspace();
		assert_eq!(state.url, "http://localhost:114");
	}

	#[test]
	fn ollama_backspace_on_empty_url_is_noop() {
		let mut state = OllamaWizardState::new();
		state.url.clear();
		state.backspace();
		assert!(state.url.is_empty());
	}

	#[test]
	fn ollama_backspace_ignored_in_model_select() {
		let mut state = OllamaWizardState::new();
		state.set_models(vec!["a".to_string()]);
		let url_before = state.url.clone();
		state.backspace();
		assert_eq!(state.url, url_before);
	}

	// ── back ───────────────────────────────────────

	#[test]
	fn ollama_back_from_confirm_to_model_select() {
		let mut state = OllamaWizardState::new();
		state.step = OllamaStep::Confirm;
		let dismiss = state.back();
		assert!(!dismiss);
		assert_eq!(state.step, OllamaStep::ModelSelect);
	}

	#[test]
	fn ollama_back_from_model_select_to_url_input() {
		let mut state = OllamaWizardState::new();
		state.set_models(vec!["a".to_string()]);
		state.set_error("some error".to_string());
		let dismiss = state.back();
		assert!(!dismiss);
		assert_eq!(state.step, OllamaStep::UrlInput);
		assert!(state.error.is_none());
	}

	#[test]
	fn ollama_back_from_url_input_signals_dismiss() {
		let mut state = OllamaWizardState::new();
		let dismiss = state.back();
		assert!(dismiss);
	}

	// ── selected_model ─────────────────────────────

	#[test]
	fn ollama_selected_model_returns_none_when_empty() {
		let state = OllamaWizardState::new();
		assert!(state.selected_model().is_none());
	}

	#[test]
	fn ollama_selected_model_returns_correct_model() {
		let mut state = OllamaWizardState::new();
		state.set_models(vec![
			"llama3.2".to_string(),
			"codellama".to_string(),
			"mistral".to_string(),
		]);
		assert_eq!(state.selected_model(), Some("llama3.2"));
		state.selected_model = 2;
		assert_eq!(state.selected_model(), Some("mistral"));
	}

	// ── Full workflow ──────────────────────────────

	#[test]
	fn ollama_full_workflow() {
		let mut state = OllamaWizardState::new();

		// Step 1: URL input — use default URL.
		assert_eq!(state.step, OllamaStep::UrlInput);
		assert_eq!(state.url, DEFAULT_OLLAMA_URL);

		let action = state.enter();
		assert_eq!(
			action,
			OllamaAction::FetchModels(DEFAULT_OLLAMA_URL.to_string())
		);

		// Externally, models are fetched and set.
		state.set_models(vec![
			"llama3.2".to_string(),
			"codellama".to_string(),
			"mistral".to_string(),
		]);
		assert_eq!(state.step, OllamaStep::ModelSelect);

		// Step 2: Navigate to "codellama".
		state.move_down();
		assert_eq!(state.selected_model, 1);
		assert_eq!(state.selected_model(), Some("codellama"));

		let action = state.enter();
		assert_eq!(
			action,
			OllamaAction::SelectModel("codellama".to_string())
		);
		assert_eq!(state.step, OllamaStep::Confirm);

		// Step 3: Confirm.
		let action = state.enter();
		assert_eq!(
			action,
			OllamaAction::Confirm {
				url: DEFAULT_OLLAMA_URL.to_string(),
				model: "codellama".to_string(),
			}
		);
	}

	#[test]
	fn ollama_full_workflow_back_all_the_way() {
		let mut state = OllamaWizardState::new();
		state.set_models(vec!["a".to_string()]);
		state.step = OllamaStep::Confirm;

		// Back from Confirm -> ModelSelect.
		assert!(!state.back());
		assert_eq!(state.step, OllamaStep::ModelSelect);

		// Back from ModelSelect -> UrlInput.
		assert!(!state.back());
		assert_eq!(state.step, OllamaStep::UrlInput);

		// Back from UrlInput -> dismiss.
		assert!(state.back());
	}

	#[test]
	fn ollama_url_edit_workflow() {
		let mut state = OllamaWizardState::new();

		// Clear the default URL and type a new one.
		state.url.clear();
		state.type_char('h');
		state.type_char('t');
		state.type_char('t');
		state.type_char('p');
		assert_eq!(state.url, "http");

		// Backspace.
		state.backspace();
		assert_eq!(state.url, "htt");
	}

	#[test]
	fn ollama_error_display_workflow() {
		let mut state = OllamaWizardState::new();

		// Fetch models fails.
		state.set_error("connection refused".to_string());
		assert_eq!(state.error, Some("connection refused".to_string()));

		// User retries, succeeds.
		state.set_models(vec!["llama3.2".to_string()]);
		assert!(state.error.is_none());
		assert_eq!(state.step, OllamaStep::ModelSelect);
	}

	// ── Render smoke tests ─────────────────────────

	#[test]
	fn render_ollama_url_input_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let state = OllamaWizardState::new();

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_ollama_wizard(frame, area, &state);
			})
			.unwrap();
	}

	#[test]
	fn render_ollama_model_select_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut state = OllamaWizardState::new();
		state.set_models(vec![
			"llama3.2".to_string(),
			"codellama".to_string(),
			"mistral".to_string(),
		]);

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_ollama_wizard(frame, area, &state);
			})
			.unwrap();
	}

	#[test]
	fn render_ollama_model_select_empty_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut state = OllamaWizardState::new();
		state.step = OllamaStep::ModelSelect;

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_ollama_wizard(frame, area, &state);
			})
			.unwrap();
	}

	#[test]
	fn render_ollama_confirm_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut state = OllamaWizardState::new();
		state.set_models(vec!["llama3.2".to_string()]);
		state.step = OllamaStep::Confirm;

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_ollama_wizard(frame, area, &state);
			})
			.unwrap();
	}

	#[test]
	fn render_ollama_with_error_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut state = OllamaWizardState::new();
		state.set_error("connection refused".to_string());

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_ollama_wizard(frame, area, &state);
			})
			.unwrap();
	}

	#[test]
	fn render_ollama_small_terminal_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(30, 10);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let state = OllamaWizardState::new();

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_ollama_wizard(frame, area, &state);
			})
			.unwrap();
	}

	#[test]
	fn render_ollama_confirm_no_models_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut state = OllamaWizardState::new();
		state.step = OllamaStep::Confirm;

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_ollama_wizard(frame, area, &state);
			})
			.unwrap();
	}

	#[test]
	fn render_ollama_url_empty_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut state = OllamaWizardState::new();
		state.url.clear();

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_ollama_wizard(frame, area, &state);
			})
			.unwrap();
	}

	#[test]
	fn render_ollama_many_models_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 30);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut state = OllamaWizardState::new();
		let models: Vec<String> = (0..20).map(|i| format!("model-{i}")).collect();
		state.set_models(models);

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_ollama_wizard(frame, area, &state);
			})
			.unwrap();
	}
}
