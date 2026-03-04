//! Settings explorer: multi-level overlay for browsing and editing config files.
//!
//! The overlay has three navigation levels:
//! 1. **FileList** — choose which config file to explore
//! 2. **FieldList** — browse fields of the selected file with current values
//! 3. **Editing** — type-specific editors (text, number, boolean, select)
//!
//! # Layout
//!
//! ```text
//! ┌─ Settings ─────────────────────────────────────┐
//! │                                                 │
//! │  ❯ config.json         General                  │
//! │    acp.json            ACP Servers              │
//! │    mcp.json            MCP Servers              │
//! │    embed.json          Embedding                │
//! │    memory.json         Memory                   │
//! │    summarize.json      Summarization            │
//! │    settings.json       Settings                 │
//! │    prompts.json        System Prompts           │
//! │                                                 │
//! │  ↑↓ navigate  ↵ open  ← back  esc dismiss      │
//! └─────────────────────────────────────────────────┘
//! ```
//!
//! At the FieldList level, fields from the selected JSON file are shown with
//! their current values. At the Editing level, a type-specific editor appears.

use std::time::Instant;

use ratatui::{
	layout::Rect,
	style::{Color, Modifier, Style},
	text::{Line, Span},
	widgets::{Block, Borders, Clear, Paragraph, Wrap},
	Frame,
};

// ── Constants ───────────────────────────────────────────

/// Maximum width of the settings explorer popup.
const MAX_POPUP_WIDTH: u16 = 60;

/// Minimum width of the settings explorer popup.
const MIN_POPUP_WIDTH: u16 = 34;

/// Duration (in seconds) for which the "Saved" indicator is visible.
const SAVED_INDICATOR_DURATION_SECS: f64 = 1.5;

/// Config files available in the settings explorer, with display labels.
pub const CONFIG_FILES: &[(&str, &str)] = &[
	("config.json", "General"),
	("acp.json", "ACP Servers"),
	("mcp.json", "MCP Servers"),
	("embed.json", "Embedding"),
	("memory.json", "Memory"),
	("summarize.json", "Summarization"),
	("settings.json", "Settings"),
	("prompts.json", "System Prompts"),
];

/// Maximum character length for a displayed field value before truncating.
const MAX_VALUE_DISPLAY_LEN: usize = 40;

// ── SettingsLevel ───────────────────────────────────────

/// Which navigation level the settings explorer is at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsLevel {
	/// Choosing a config file.
	FileList,
	/// Browsing fields of the selected config file.
	FieldList,
	/// Editing a specific field value.
	Editing,
}

// ── SettingsExplorerState ───────────────────────────────

/// State for the settings explorer overlay.
///
/// Tracks the current navigation level, selection indices, the edit buffer,
/// and the saved indicator timestamp.
#[derive(Debug, Clone)]
pub struct SettingsExplorerState {
	/// Current navigation level.
	pub level: SettingsLevel,
	/// Index of the selected config file in `CONFIG_FILES`.
	pub selected_file: usize,
	/// Index of the selected field within the current config file.
	pub selected_field: usize,
	/// The current edit buffer (for text/number editing).
	pub edit_value: String,
	/// Timestamp of the last save action (for "Saved" indicator).
	pub saved_indicator: Option<Instant>,
}

impl SettingsExplorerState {
	/// Create a new settings explorer state at the FileList level.
	pub fn new() -> Self {
		Self {
			level: SettingsLevel::FileList,
			selected_file: 0,
			selected_field: 0,
			edit_value: String::new(),
			saved_indicator: None,
		}
	}

	/// Move selection up within the current level.
	pub fn move_up(&mut self) {
		match self.level {
			SettingsLevel::FileList => {
				if self.selected_file > 0 {
					self.selected_file -= 1;
				}
			}
			SettingsLevel::FieldList | SettingsLevel::Editing => {
				if self.selected_field > 0 {
					self.selected_field -= 1;
				}
			}
		}
	}

	/// Move selection down within the current level.
	///
	/// The `item_count` is the number of items in the current list. For
	/// `FileList` this is `CONFIG_FILES.len()`, for `FieldList`/`Editing`
	/// it is the number of fields in the selected config.
	pub fn move_down(&mut self, item_count: usize) {
		if item_count == 0 {
			return;
		}
		match self.level {
			SettingsLevel::FileList => {
				if self.selected_file + 1 < item_count {
					self.selected_file += 1;
				}
			}
			SettingsLevel::FieldList | SettingsLevel::Editing => {
				if self.selected_field + 1 < item_count {
					self.selected_field += 1;
				}
			}
		}
	}

	/// Go deeper: FileList -> FieldList -> Editing.
	///
	/// When entering FieldList, the field selection resets to 0.
	/// When entering Editing, `edit_value` is populated with `current_value`
	/// (the current value of the selected field as a string).
	pub fn enter(&mut self, current_value: &str) {
		match self.level {
			SettingsLevel::FileList => {
				self.level = SettingsLevel::FieldList;
				self.selected_field = 0;
			}
			SettingsLevel::FieldList => {
				self.level = SettingsLevel::Editing;
				self.edit_value = current_value.to_string();
			}
			SettingsLevel::Editing => {
				// Already at deepest level — enter here means "confirm edit".
				// Handled externally (caller saves).
			}
		}
	}

	/// Go up one level, or signal dismissal at FileList.
	///
	/// Returns `true` if the overlay should be dismissed (back at FileList level).
	pub fn back(&mut self) -> bool {
		match self.level {
			SettingsLevel::Editing => {
				self.level = SettingsLevel::FieldList;
				self.edit_value.clear();
				false
			}
			SettingsLevel::FieldList => {
				self.level = SettingsLevel::FileList;
				false
			}
			SettingsLevel::FileList => {
				// Signal to the caller that the overlay should be dismissed.
				true
			}
		}
	}

	/// Append a character to the edit buffer (only in Editing level).
	pub fn type_char(&mut self, c: char) {
		if self.level == SettingsLevel::Editing {
			self.edit_value.push(c);
		}
	}

	/// Delete the last character from the edit buffer (only in Editing level).
	pub fn backspace(&mut self) {
		if self.level == SettingsLevel::Editing {
			self.edit_value.pop();
		}
	}

	/// Toggle a boolean value in the edit buffer.
	///
	/// If the current edit_value is "true", it becomes "false" and vice versa.
	/// If the edit_value is not a boolean string, this is a no-op.
	pub fn toggle(&mut self) {
		match self.edit_value.as_str() {
			"true" => self.edit_value = "false".to_string(),
			"false" => self.edit_value = "true".to_string(),
			_ => {}
		}
	}

	/// Returns `true` if the saved indicator should be visible.
	///
	/// The indicator is shown for `SAVED_INDICATOR_DURATION_SECS` after the
	/// last `mark_saved()` call.
	pub fn is_saved_visible(&self) -> bool {
		match self.saved_indicator {
			Some(instant) => instant.elapsed().as_secs_f64() < SAVED_INDICATOR_DURATION_SECS,
			None => false,
		}
	}

	/// Mark the current time as when a save occurred.
	pub fn mark_saved(&mut self) {
		self.saved_indicator = Some(Instant::now());
	}

	/// Returns the currently selected config file name.
	pub fn selected_file_name(&self) -> &str {
		CONFIG_FILES
			.get(self.selected_file)
			.map(|(name, _)| *name)
			.unwrap_or("config.json")
	}

	/// Returns the currently selected config file label.
	pub fn selected_file_label(&self) -> &str {
		CONFIG_FILES
			.get(self.selected_file)
			.map(|(_, label)| *label)
			.unwrap_or("General")
	}
}

impl Default for SettingsExplorerState {
	fn default() -> Self {
		Self::new()
	}
}

// ── Rendering ───────────────────────────────────────────

/// Render the settings explorer as a centered overlay popup.
///
/// The `config_data` parameter contains the loaded JSON for the currently
/// selected config file. It is provided by the caller from the bridge's
/// config loading. If `config_data` is `null` or not an object, an empty
/// field list is shown.
pub fn render_settings_explorer(
	frame: &mut Frame,
	area: Rect,
	state: &SettingsExplorerState,
	config_data: &serde_json::Value,
) {
	let mut lines: Vec<Line<'static>> = Vec::new();

	// Blank line for padding.
	lines.push(Line::from(""));

	match state.level {
		SettingsLevel::FileList => {
			render_file_list(&mut lines, state);
		}
		SettingsLevel::FieldList => {
			render_field_list(&mut lines, state, config_data);
		}
		SettingsLevel::Editing => {
			render_editing(&mut lines, state, config_data);
		}
	}

	// Blank separator.
	lines.push(Line::from(""));

	// Key hints.
	render_key_hints(&mut lines, state);

	// Saved indicator.
	if state.is_saved_visible() {
		lines.push(Line::from(""));
		lines.push(Line::from(Span::styled(
			"  \u{2714} Saved",
			Style::default()
				.fg(Color::Green)
				.add_modifier(Modifier::BOLD),
		)));
	}

	// Trailing padding.
	lines.push(Line::from(""));

	// Build the title based on current level.
	let title = match state.level {
		SettingsLevel::FileList => " Settings ".to_string(),
		SettingsLevel::FieldList => format!(" {} ", state.selected_file_label()),
		SettingsLevel::Editing => format!(" {} \u{2022} Edit ", state.selected_file_label()),
	};

	// Calculate popup dimensions.
	let content_height = lines.len() as u16 + 2; // +2 for border top/bottom
	let available_width = area.width.saturating_sub(4);
	let popup_width = MAX_POPUP_WIDTH
		.min(available_width)
		.max(MIN_POPUP_WIDTH)
		.min(area.width); // never exceed total area width
	let popup_height = content_height.min(area.height.saturating_sub(2)).min(area.height);

	// Center the popup.
	let popup_x = (area.width.saturating_sub(popup_width)) / 2;
	let popup_y = (area.height.saturating_sub(popup_height)) / 2;
	let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

	// Clear the area behind the popup, then render.
	frame.render_widget(Clear, popup_area);

	let border_color = match state.level {
		SettingsLevel::FileList => Color::Cyan,
		SettingsLevel::FieldList => Color::Blue,
		SettingsLevel::Editing => Color::Yellow,
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

/// Render the file list level.
fn render_file_list(lines: &mut Vec<Line<'static>>, state: &SettingsExplorerState) {
	for (i, (filename, label)) in CONFIG_FILES.iter().enumerate() {
		let selected = i == state.selected_file;
		let prefix = if selected { "  \u{276f} " } else { "    " };
		let color = if selected { Color::Cyan } else { Color::Reset };
		let mut style = Style::default().fg(color);
		if selected {
			style = style.add_modifier(Modifier::BOLD);
		}

		lines.push(Line::from(vec![
			Span::styled(format!("{prefix}{filename}"), style),
			Span::styled(
				format!("  {label}"),
				Style::default().fg(Color::DarkGray),
			),
		]));
	}
}

/// Render the field list level.
fn render_field_list(
	lines: &mut Vec<Line<'static>>,
	state: &SettingsExplorerState,
	config_data: &serde_json::Value,
) {
	// Header: show which file we're browsing.
	lines.push(Line::from(vec![
		Span::styled("  ", Style::default()),
		Span::styled(
			state.selected_file_name().to_string(),
			Style::default()
				.fg(Color::Cyan)
				.add_modifier(Modifier::BOLD),
		),
	]));
	lines.push(Line::from(""));

	let fields = extract_fields(config_data);
	if fields.is_empty() {
		lines.push(Line::from(Span::styled(
			"    (empty or not loaded)",
			Style::default().fg(Color::DarkGray),
		)));
		return;
	}

	for (i, (key, value_display, value_type)) in fields.iter().enumerate() {
		let selected = i == state.selected_field;
		let prefix = if selected { "  \u{276f} " } else { "    " };
		let key_color = if selected { Color::Cyan } else { Color::White };
		let mut key_style = Style::default().fg(key_color);
		if selected {
			key_style = key_style.add_modifier(Modifier::BOLD);
		}

		let type_color = match value_type.as_str() {
			"boolean" => Color::Magenta,
			"number" => Color::Yellow,
			"string" => Color::Green,
			_ => Color::DarkGray,
		};

		lines.push(Line::from(vec![
			Span::styled(format!("{prefix}{key}"), key_style),
			Span::styled(": ", Style::default().fg(Color::DarkGray)),
			Span::styled(
				truncate_value(value_display, MAX_VALUE_DISPLAY_LEN),
				Style::default().fg(type_color),
			),
		]));
	}
}

/// Render the editing level.
fn render_editing(
	lines: &mut Vec<Line<'static>>,
	state: &SettingsExplorerState,
	config_data: &serde_json::Value,
) {
	let fields = extract_fields(config_data);
	let field_info = fields.get(state.selected_field);

	let (field_name, _value_display, value_type) = match field_info {
		Some(info) => info.clone(),
		None => ("unknown".to_string(), "null".to_string(), "null".to_string()),
	};

	// Header: show which field we're editing.
	lines.push(Line::from(vec![
		Span::styled("  Editing ", Style::default().fg(Color::DarkGray)),
		Span::styled(
			field_name,
			Style::default()
				.fg(Color::Cyan)
				.add_modifier(Modifier::BOLD),
		),
		Span::styled(
			format!(" ({value_type})"),
			Style::default().fg(Color::DarkGray),
		),
	]));
	lines.push(Line::from(""));

	match value_type.as_str() {
		"boolean" => {
			// Toggle display.
			let is_true = state.edit_value == "true";
			let true_style = if is_true {
				Style::default()
					.fg(Color::Green)
					.add_modifier(Modifier::BOLD)
			} else {
				Style::default().fg(Color::DarkGray)
			};
			let false_style = if !is_true {
				Style::default()
					.fg(Color::Red)
					.add_modifier(Modifier::BOLD)
			} else {
				Style::default().fg(Color::DarkGray)
			};

			lines.push(Line::from(vec![
				Span::styled("    ", Style::default()),
				Span::styled("true", true_style),
				Span::styled(" / ", Style::default().fg(Color::DarkGray)),
				Span::styled("false", false_style),
			]));
			lines.push(Line::from(""));
			lines.push(Line::from(Span::styled(
				"  Press space or ↵ to toggle",
				Style::default().fg(Color::DarkGray),
			)));
		}
		_ => {
			// Text/number input.
			let display = if state.edit_value.is_empty() {
				Span::styled("_", Style::default().fg(Color::DarkGray))
			} else {
				Span::styled(
					state.edit_value.clone(),
					Style::default().fg(Color::White),
				)
			};

			lines.push(Line::from(vec![
				Span::styled("  Value: ", Style::default().fg(Color::DarkGray)),
				display,
				Span::styled(
					"\u{2588}",
					Style::default()
						.fg(Color::White)
						.add_modifier(Modifier::SLOW_BLINK),
				),
			]));
		}
	}
}

/// Render key hints at the bottom of the overlay.
fn render_key_hints(lines: &mut Vec<Line<'static>>, state: &SettingsExplorerState) {
	let dim = Style::default().fg(Color::DarkGray);
	let bold_dim = Style::default()
		.fg(Color::DarkGray)
		.add_modifier(Modifier::BOLD);

	let mut spans = Vec::new();
	spans.push(Span::raw("  "));

	match state.level {
		SettingsLevel::FileList => {
			spans.push(Span::styled("\u{2191}\u{2193}", bold_dim));
			spans.push(Span::styled(" navigate  ", dim));
			spans.push(Span::styled("\u{21b5}", bold_dim));
			spans.push(Span::styled(" open  ", dim));
			spans.push(Span::styled("esc", bold_dim));
			spans.push(Span::styled(" dismiss", dim));
		}
		SettingsLevel::FieldList => {
			spans.push(Span::styled("\u{2191}\u{2193}", bold_dim));
			spans.push(Span::styled(" navigate  ", dim));
			spans.push(Span::styled("\u{21b5}", bold_dim));
			spans.push(Span::styled(" edit  ", dim));
			spans.push(Span::styled("\u{2190}", bold_dim));
			spans.push(Span::styled(" back  ", dim));
			spans.push(Span::styled("esc", bold_dim));
			spans.push(Span::styled(" dismiss", dim));
		}
		SettingsLevel::Editing => {
			spans.push(Span::styled("\u{21b5}", bold_dim));
			spans.push(Span::styled(" save  ", dim));
			spans.push(Span::styled("\u{2190}", bold_dim));
			spans.push(Span::styled(" back  ", dim));
			spans.push(Span::styled("esc", bold_dim));
			spans.push(Span::styled(" dismiss", dim));
		}
	}

	lines.push(Line::from(spans));
}

// ── Helpers ─────────────────────────────────────────────

/// Extract fields from a JSON value as `(key, display_value, type_name)` triples.
///
/// Only top-level keys of a JSON object are extracted. Nested objects and arrays
/// are shown as compact JSON strings.
pub fn extract_fields(data: &serde_json::Value) -> Vec<(String, String, String)> {
	let obj = match data.as_object() {
		Some(obj) => obj,
		None => return Vec::new(),
	};

	obj.iter()
		.map(|(key, value)| {
			let display = format_display_value(value);
			let type_name = json_type_name(value);
			(key.clone(), display, type_name)
		})
		.collect()
}

/// Format a JSON value for display in the field list.
fn format_display_value(value: &serde_json::Value) -> String {
	match value {
		serde_json::Value::String(s) => format!("\"{s}\""),
		serde_json::Value::Number(n) => n.to_string(),
		serde_json::Value::Bool(b) => b.to_string(),
		serde_json::Value::Null => "null".to_string(),
		serde_json::Value::Array(arr) => {
			if arr.is_empty() {
				"[]".to_string()
			} else {
				serde_json::to_string(value).unwrap_or_else(|_| "[...]".to_string())
			}
		}
		serde_json::Value::Object(map) => {
			if map.is_empty() {
				"{}".to_string()
			} else {
				serde_json::to_string(value).unwrap_or_else(|_| "{...}".to_string())
			}
		}
	}
}

/// Return a human-readable type name for a JSON value.
fn json_type_name(value: &serde_json::Value) -> String {
	match value {
		serde_json::Value::String(_) => "string".to_string(),
		serde_json::Value::Number(_) => "number".to_string(),
		serde_json::Value::Bool(_) => "boolean".to_string(),
		serde_json::Value::Null => "null".to_string(),
		serde_json::Value::Array(_) => "array".to_string(),
		serde_json::Value::Object(_) => "object".to_string(),
	}
}

/// Truncate a display value to `max_len` characters, appending "..." if truncated.
fn truncate_value(s: &str, max_len: usize) -> String {
	if s.chars().count() <= max_len {
		s.to_string()
	} else {
		let truncated: String = s.chars().take(max_len.saturating_sub(3)).collect();
		format!("{truncated}...")
	}
}

// ── Tests ───────────────────────────────────────────────

#[cfg(test)]
mod tests {
	use super::*;

	// ── SettingsExplorerState::new ──────────────────

	#[test]
	fn new_defaults_to_file_list() {
		let state = SettingsExplorerState::new();
		assert_eq!(state.level, SettingsLevel::FileList);
		assert_eq!(state.selected_file, 0);
		assert_eq!(state.selected_field, 0);
		assert!(state.edit_value.is_empty());
		assert!(state.saved_indicator.is_none());
	}

	#[test]
	fn default_equals_new() {
		let a = SettingsExplorerState::new();
		let b = SettingsExplorerState::default();
		assert_eq!(a.level, b.level);
		assert_eq!(a.selected_file, b.selected_file);
		assert_eq!(a.selected_field, b.selected_field);
		assert_eq!(a.edit_value, b.edit_value);
	}

	// ── CONFIG_FILES ────────────────────────────────

	#[test]
	fn config_files_has_expected_entries() {
		assert_eq!(CONFIG_FILES.len(), 8);
		assert_eq!(CONFIG_FILES[0], ("config.json", "General"));
		assert_eq!(CONFIG_FILES[7], ("prompts.json", "System Prompts"));
	}

	// ── move_up / move_down ─────────────────────────

	#[test]
	fn move_up_at_file_list_clamps() {
		let mut state = SettingsExplorerState::new();
		state.move_up();
		assert_eq!(state.selected_file, 0);
	}

	#[test]
	fn move_down_at_file_list() {
		let mut state = SettingsExplorerState::new();
		state.move_down(CONFIG_FILES.len());
		assert_eq!(state.selected_file, 1);
		state.move_down(CONFIG_FILES.len());
		assert_eq!(state.selected_file, 2);
	}

	#[test]
	fn move_down_at_file_list_clamps() {
		let mut state = SettingsExplorerState::new();
		for _ in 0..20 {
			state.move_down(CONFIG_FILES.len());
		}
		assert_eq!(state.selected_file, CONFIG_FILES.len() - 1);
	}

	#[test]
	fn move_up_at_field_list() {
		let mut state = SettingsExplorerState::new();
		state.enter(""); // -> FieldList
		state.selected_field = 3;
		state.move_up();
		assert_eq!(state.selected_field, 2);
	}

	#[test]
	fn move_down_at_field_list() {
		let mut state = SettingsExplorerState::new();
		state.enter(""); // -> FieldList
		state.move_down(5);
		assert_eq!(state.selected_field, 1);
	}

	#[test]
	fn move_down_zero_items_is_noop() {
		let mut state = SettingsExplorerState::new();
		state.move_down(0);
		assert_eq!(state.selected_file, 0);
	}

	#[test]
	fn move_up_at_field_list_clamps() {
		let mut state = SettingsExplorerState::new();
		state.enter(""); // -> FieldList
		state.move_up();
		assert_eq!(state.selected_field, 0);
	}

	#[test]
	fn move_down_at_field_list_clamps() {
		let mut state = SettingsExplorerState::new();
		state.enter(""); // -> FieldList
		for _ in 0..20 {
			state.move_down(3);
		}
		assert_eq!(state.selected_field, 2);
	}

	// ── enter ───────────────────────────────────────

	#[test]
	fn enter_goes_from_file_list_to_field_list() {
		let mut state = SettingsExplorerState::new();
		state.selected_file = 2;
		state.enter("");
		assert_eq!(state.level, SettingsLevel::FieldList);
		assert_eq!(state.selected_field, 0);
	}

	#[test]
	fn enter_goes_from_field_list_to_editing() {
		let mut state = SettingsExplorerState::new();
		state.enter(""); // -> FieldList
		state.enter("current_value");
		assert_eq!(state.level, SettingsLevel::Editing);
		assert_eq!(state.edit_value, "current_value");
	}

	#[test]
	fn enter_at_editing_is_noop() {
		let mut state = SettingsExplorerState::new();
		state.enter(""); // -> FieldList
		state.enter("val"); // -> Editing
		state.enter("ignored");
		assert_eq!(state.level, SettingsLevel::Editing);
		// edit_value should remain "val", not change to "ignored"
		assert_eq!(state.edit_value, "val");
	}

	#[test]
	fn enter_resets_selected_field() {
		let mut state = SettingsExplorerState::new();
		state.selected_field = 5; // This shouldn't matter at FileList level
		state.enter("");
		assert_eq!(state.selected_field, 0);
	}

	// ── back ────────────────────────────────────────

	#[test]
	fn back_from_editing_goes_to_field_list() {
		let mut state = SettingsExplorerState::new();
		state.enter(""); // -> FieldList
		state.enter("val"); // -> Editing
		let dismiss = state.back();
		assert!(!dismiss);
		assert_eq!(state.level, SettingsLevel::FieldList);
		assert!(state.edit_value.is_empty());
	}

	#[test]
	fn back_from_field_list_goes_to_file_list() {
		let mut state = SettingsExplorerState::new();
		state.enter(""); // -> FieldList
		let dismiss = state.back();
		assert!(!dismiss);
		assert_eq!(state.level, SettingsLevel::FileList);
	}

	#[test]
	fn back_from_file_list_signals_dismiss() {
		let mut state = SettingsExplorerState::new();
		let dismiss = state.back();
		assert!(dismiss);
		assert_eq!(state.level, SettingsLevel::FileList);
	}

	// ── type_char / backspace ───────────────────────

	#[test]
	fn type_char_appends_in_editing() {
		let mut state = SettingsExplorerState::new();
		state.enter(""); // -> FieldList
		state.enter(""); // -> Editing
		state.type_char('h');
		state.type_char('i');
		assert_eq!(state.edit_value, "hi");
	}

	#[test]
	fn type_char_ignored_in_file_list() {
		let mut state = SettingsExplorerState::new();
		state.type_char('x');
		assert!(state.edit_value.is_empty());
	}

	#[test]
	fn type_char_ignored_in_field_list() {
		let mut state = SettingsExplorerState::new();
		state.enter(""); // -> FieldList
		state.type_char('x');
		assert!(state.edit_value.is_empty());
	}

	#[test]
	fn backspace_removes_last_char_in_editing() {
		let mut state = SettingsExplorerState::new();
		state.enter(""); // -> FieldList
		state.enter("abc"); // -> Editing
		state.backspace();
		assert_eq!(state.edit_value, "ab");
	}

	#[test]
	fn backspace_on_empty_is_noop() {
		let mut state = SettingsExplorerState::new();
		state.enter(""); // -> FieldList
		state.enter(""); // -> Editing
		state.backspace();
		assert!(state.edit_value.is_empty());
	}

	#[test]
	fn backspace_ignored_in_file_list() {
		let mut state = SettingsExplorerState::new();
		state.backspace();
		assert!(state.edit_value.is_empty());
	}

	// ── toggle ──────────────────────────────────────

	#[test]
	fn toggle_true_to_false() {
		let mut state = SettingsExplorerState::new();
		state.enter(""); // -> FieldList
		state.enter("true"); // -> Editing
		state.toggle();
		assert_eq!(state.edit_value, "false");
	}

	#[test]
	fn toggle_false_to_true() {
		let mut state = SettingsExplorerState::new();
		state.enter(""); // -> FieldList
		state.enter("false"); // -> Editing
		state.toggle();
		assert_eq!(state.edit_value, "true");
	}

	#[test]
	fn toggle_non_boolean_is_noop() {
		let mut state = SettingsExplorerState::new();
		state.enter(""); // -> FieldList
		state.enter("hello"); // -> Editing
		state.toggle();
		assert_eq!(state.edit_value, "hello");
	}

	#[test]
	fn toggle_empty_is_noop() {
		let mut state = SettingsExplorerState::new();
		state.enter(""); // -> FieldList
		state.enter(""); // -> Editing
		state.toggle();
		assert!(state.edit_value.is_empty());
	}

	// ── is_saved_visible / mark_saved ───────────────

	#[test]
	fn saved_not_visible_initially() {
		let state = SettingsExplorerState::new();
		assert!(!state.is_saved_visible());
	}

	#[test]
	fn saved_visible_after_mark() {
		let mut state = SettingsExplorerState::new();
		state.mark_saved();
		assert!(state.is_saved_visible());
	}

	// ── selected_file_name / selected_file_label ────

	#[test]
	fn selected_file_name_returns_correct_name() {
		let mut state = SettingsExplorerState::new();
		assert_eq!(state.selected_file_name(), "config.json");
		state.selected_file = 3;
		assert_eq!(state.selected_file_name(), "embed.json");
	}

	#[test]
	fn selected_file_label_returns_correct_label() {
		let mut state = SettingsExplorerState::new();
		assert_eq!(state.selected_file_label(), "General");
		state.selected_file = 5;
		assert_eq!(state.selected_file_label(), "Summarization");
	}

	#[test]
	fn selected_file_name_out_of_bounds_defaults() {
		let mut state = SettingsExplorerState::new();
		state.selected_file = 100;
		assert_eq!(state.selected_file_name(), "config.json");
		assert_eq!(state.selected_file_label(), "General");
	}

	// ── extract_fields ──────────────────────────────

	#[test]
	fn extract_fields_from_object() {
		let data = serde_json::json!({
			"host": "localhost",
			"port": 8080,
			"debug": true,
			"name": null
		});
		let fields = extract_fields(&data);
		assert_eq!(fields.len(), 4);

		// Find the "host" field.
		let host = fields.iter().find(|(k, _, _)| k == "host").unwrap();
		assert_eq!(host.1, "\"localhost\"");
		assert_eq!(host.2, "string");

		// Find the "port" field.
		let port = fields.iter().find(|(k, _, _)| k == "port").unwrap();
		assert_eq!(port.1, "8080");
		assert_eq!(port.2, "number");

		// Find the "debug" field.
		let debug = fields.iter().find(|(k, _, _)| k == "debug").unwrap();
		assert_eq!(debug.1, "true");
		assert_eq!(debug.2, "boolean");

		// Find the "name" field.
		let name = fields.iter().find(|(k, _, _)| k == "name").unwrap();
		assert_eq!(name.1, "null");
		assert_eq!(name.2, "null");
	}

	#[test]
	fn extract_fields_from_null() {
		let data = serde_json::Value::Null;
		let fields = extract_fields(&data);
		assert!(fields.is_empty());
	}

	#[test]
	fn extract_fields_from_array() {
		let data = serde_json::json!([1, 2, 3]);
		let fields = extract_fields(&data);
		assert!(fields.is_empty());
	}

	#[test]
	fn extract_fields_from_empty_object() {
		let data = serde_json::json!({});
		let fields = extract_fields(&data);
		assert!(fields.is_empty());
	}

	#[test]
	fn extract_fields_nested_object() {
		let data = serde_json::json!({"servers": {"s1": {"url": "http://localhost"}}});
		let fields = extract_fields(&data);
		assert_eq!(fields.len(), 1);
		assert_eq!(fields[0].2, "object");
		assert!(fields[0].1.contains("s1"));
	}

	#[test]
	fn extract_fields_array_value() {
		let data = serde_json::json!({"tags": ["a", "b", "c"]});
		let fields = extract_fields(&data);
		assert_eq!(fields.len(), 1);
		assert_eq!(fields[0].2, "array");
	}

	// ── format_display_value ────────────────────────

	#[test]
	fn format_display_value_string() {
		let val = serde_json::json!("hello");
		assert_eq!(format_display_value(&val), "\"hello\"");
	}

	#[test]
	fn format_display_value_number() {
		let val = serde_json::json!(42);
		assert_eq!(format_display_value(&val), "42");
	}

	#[test]
	fn format_display_value_bool() {
		assert_eq!(format_display_value(&serde_json::json!(true)), "true");
		assert_eq!(format_display_value(&serde_json::json!(false)), "false");
	}

	#[test]
	fn format_display_value_null() {
		assert_eq!(
			format_display_value(&serde_json::Value::Null),
			"null"
		);
	}

	#[test]
	fn format_display_value_empty_array() {
		assert_eq!(format_display_value(&serde_json::json!([])), "[]");
	}

	#[test]
	fn format_display_value_empty_object() {
		assert_eq!(format_display_value(&serde_json::json!({})), "{}");
	}

	// ── json_type_name ──────────────────────────────

	#[test]
	fn json_type_name_all_types() {
		assert_eq!(json_type_name(&serde_json::json!("s")), "string");
		assert_eq!(json_type_name(&serde_json::json!(1)), "number");
		assert_eq!(json_type_name(&serde_json::json!(true)), "boolean");
		assert_eq!(json_type_name(&serde_json::Value::Null), "null");
		assert_eq!(json_type_name(&serde_json::json!([])), "array");
		assert_eq!(json_type_name(&serde_json::json!({})), "object");
	}

	// ── truncate_value ──────────────────────────────

	#[test]
	fn truncate_value_short() {
		assert_eq!(truncate_value("hello", 10), "hello");
	}

	#[test]
	fn truncate_value_exact() {
		assert_eq!(truncate_value("hello", 5), "hello");
	}

	#[test]
	fn truncate_value_long() {
		let result = truncate_value("hello world!", 8);
		assert_eq!(result, "hello...");
		assert!(result.len() <= 8);
	}

	#[test]
	fn truncate_value_very_short_max() {
		let result = truncate_value("hello", 3);
		assert_eq!(result, "...");
	}

	// ── Full navigation workflow ────────────────────

	#[test]
	fn full_navigation_workflow() {
		let mut state = SettingsExplorerState::new();

		// Start at file list.
		assert_eq!(state.level, SettingsLevel::FileList);

		// Navigate to "mcp.json" (index 2).
		state.move_down(CONFIG_FILES.len());
		state.move_down(CONFIG_FILES.len());
		assert_eq!(state.selected_file, 2);
		assert_eq!(state.selected_file_name(), "mcp.json");

		// Enter field list.
		state.enter("");
		assert_eq!(state.level, SettingsLevel::FieldList);
		assert_eq!(state.selected_field, 0);

		// Navigate down two fields.
		state.move_down(5);
		state.move_down(5);
		assert_eq!(state.selected_field, 2);

		// Enter editing with current value.
		state.enter("http://localhost:3000");
		assert_eq!(state.level, SettingsLevel::Editing);
		assert_eq!(state.edit_value, "http://localhost:3000");

		// Edit the value.
		state.backspace();
		state.type_char('1');
		assert_eq!(state.edit_value, "http://localhost:3001");

		// Back to field list.
		let dismiss = state.back();
		assert!(!dismiss);
		assert_eq!(state.level, SettingsLevel::FieldList);
		assert!(state.edit_value.is_empty());

		// Back to file list.
		let dismiss = state.back();
		assert!(!dismiss);
		assert_eq!(state.level, SettingsLevel::FileList);

		// Back again = dismiss.
		let dismiss = state.back();
		assert!(dismiss);
	}

	#[test]
	fn toggle_workflow() {
		let mut state = SettingsExplorerState::new();
		state.enter(""); // -> FieldList
		state.enter("true"); // -> Editing

		assert_eq!(state.edit_value, "true");
		state.toggle();
		assert_eq!(state.edit_value, "false");
		state.toggle();
		assert_eq!(state.edit_value, "true");
	}

	#[test]
	fn saved_indicator_workflow() {
		let mut state = SettingsExplorerState::new();
		assert!(!state.is_saved_visible());

		state.mark_saved();
		assert!(state.is_saved_visible());

		// The indicator should remain visible for at least a brief time.
		// We can't easily test the 1.5s timeout in a unit test without sleeping,
		// but we verify it's set.
		assert!(state.saved_indicator.is_some());
	}

	// ── Render smoke tests ──────────────────────────

	#[test]
	fn render_settings_file_list_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let state = SettingsExplorerState::new();
		let config_data = serde_json::Value::Null;

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_settings_explorer(frame, area, &state, &config_data);
			})
			.unwrap();
	}

	#[test]
	fn render_settings_field_list_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut state = SettingsExplorerState::new();
		state.enter(""); // -> FieldList
		let config_data = serde_json::json!({
			"host": "localhost",
			"port": 8080,
			"debug": true
		});

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_settings_explorer(frame, area, &state, &config_data);
			})
			.unwrap();
	}

	#[test]
	fn render_settings_editing_text_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut state = SettingsExplorerState::new();
		state.enter(""); // -> FieldList
		state.enter("localhost"); // -> Editing
		let config_data = serde_json::json!({"host": "localhost"});

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_settings_explorer(frame, area, &state, &config_data);
			})
			.unwrap();
	}

	#[test]
	fn render_settings_editing_boolean_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut state = SettingsExplorerState::new();
		state.enter(""); // -> FieldList
		state.enter("true"); // -> Editing
		let config_data = serde_json::json!({"debug": true});

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_settings_explorer(frame, area, &state, &config_data);
			})
			.unwrap();
	}

	#[test]
	fn render_settings_empty_config_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut state = SettingsExplorerState::new();
		state.enter(""); // -> FieldList
		let config_data = serde_json::json!({});

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_settings_explorer(frame, area, &state, &config_data);
			})
			.unwrap();
	}

	#[test]
	fn render_settings_small_terminal_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(30, 10);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let state = SettingsExplorerState::new();
		let config_data = serde_json::Value::Null;

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_settings_explorer(frame, area, &state, &config_data);
			})
			.unwrap();
	}

	#[test]
	fn render_settings_with_saved_indicator_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut state = SettingsExplorerState::new();
		state.mark_saved();
		let config_data = serde_json::Value::Null;

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_settings_explorer(frame, area, &state, &config_data);
			})
			.unwrap();
	}

	#[test]
	fn render_settings_with_many_fields_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 30);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut state = SettingsExplorerState::new();
		state.enter(""); // -> FieldList

		let mut data = serde_json::Map::new();
		for i in 0..20 {
			data.insert(
				format!("field_{i}"),
				serde_json::Value::String(format!("value_{i}")),
			);
		}
		let config_data = serde_json::Value::Object(data);

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_settings_explorer(frame, area, &state, &config_data);
			})
			.unwrap();
	}

	#[test]
	fn render_settings_editing_with_no_fields_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut state = SettingsExplorerState::new();
		state.level = SettingsLevel::Editing;
		state.edit_value = "test".to_string();
		let config_data = serde_json::json!({});

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_settings_explorer(frame, area, &state, &config_data);
			})
			.unwrap();
	}

	#[test]
	fn render_settings_editing_with_long_value_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(60, 20);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut state = SettingsExplorerState::new();
		state.enter(""); // -> FieldList
		let long_val = "a".repeat(200);
		state.enter(&long_val); // -> Editing
		let config_data = serde_json::json!({"key": long_val});

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_settings_explorer(frame, area, &state, &config_data);
			})
			.unwrap();
	}
}
