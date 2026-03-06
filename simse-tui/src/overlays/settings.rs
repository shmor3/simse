//! Settings form overlay: multi-level popup for browsing and editing config files.
//!
//! The overlay has three navigation levels (driven by `SettingsFormState` from `simse-ui-core`):
//! 1. **FileList** — choose which config file to explore
//! 2. **FieldList** — browse fields of the selected file with current values
//! 3. **Editing** — type-specific editors (text, number, boolean, select)
//!
//! # Layout
//!
//! ```text
//! ┌─ Settings ─────────────────────────────────────────┐
//! │                                                     │
//! │  ❯ config.json         General                      │
//! │    acp.json            ACP Servers                  │
//! │    mcp.json            MCP Servers                  │
//! │    embed.json          Embedding                    │
//! │    memory.json         Memory                       │
//! │    summarize.json      Summarization                │
//! │    settings.json       Settings                     │
//! │    prompts.json        System Prompts               │
//! │                                                     │
//! │  ↑↓ navigate  ↵ open  ← back  esc dismiss          │
//! └─────────────────────────────────────────────────────┘
//! ```
//!
//! At the FieldList level, fields from the selected JSON file are shown with
//! their current values. At the Editing level, a type-specific editor appears.

use ratatui::{
	layout::Rect,
	style::{Color, Modifier, Style},
	text::{Line, Span},
	widgets::{Block, Borders, Clear, Paragraph, Wrap},
	Frame,
};

use simse_ui_core::config::settings_state::{SettingsFormState, SettingsLevel, CONFIG_FILES};

// ── Constants ───────────────────────────────────────────

/// Maximum width of the settings form popup.
const MAX_POPUP_WIDTH: u16 = 60;

/// Minimum width of the settings form popup.
const MIN_POPUP_WIDTH: u16 = 34;

/// Maximum character length for a displayed field value before truncating.
const MAX_VALUE_DISPLAY_LEN: usize = 40;

// ── Rendering ───────────────────────────────────────────

/// Render the settings form as a centered overlay popup.
///
/// All state — including loaded config data — lives in `SettingsFormState`.
pub fn render_settings_form(
	frame: &mut Frame,
	area: Rect,
	state: &SettingsFormState,
) {
	let mut lines: Vec<Line<'static>> = Vec::new();

	// Blank line for padding.
	lines.push(Line::from(""));

	match state.level {
		SettingsLevel::FileList => {
			render_file_list(&mut lines, state);
		}
		SettingsLevel::FieldList => {
			render_field_list(&mut lines, state);
		}
		SettingsLevel::Editing | SettingsLevel::ArrayEntry => {
			render_editing(&mut lines, state);
		}
	}

	// Error display.
	if let Some(ref err) = state.error {
		lines.push(Line::from(""));
		lines.push(Line::from(Span::styled(
			format!("  \u{2718} {err}"),
			Style::default()
				.fg(Color::Red)
				.add_modifier(Modifier::BOLD),
		)));
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
		SettingsLevel::Editing | SettingsLevel::ArrayEntry => {
			format!(" {} \u{2022} Edit ", state.selected_file_label())
		}
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
		SettingsLevel::Editing | SettingsLevel::ArrayEntry => Color::Yellow,
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
fn render_file_list(lines: &mut Vec<Line<'static>>, state: &SettingsFormState) {
	for (i, (filename, label, _scope)) in CONFIG_FILES.iter().enumerate() {
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
	state: &SettingsFormState,
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

	let fields = extract_fields(&state.config_data);
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
	state: &SettingsFormState,
) {
	let fields = extract_fields(&state.config_data);
	let field_info = fields.get(state.selected_field);

	let (field_name, _value_display, value_type) = match field_info {
		Some(info) => info.clone(),
		None => ("unknown".to_string(), "null".to_string(), "null".to_string()),
	};

	// Check if this is a Select field via schema.
	let select_options: Option<Vec<String>> = state
		.current_file_schema()
		.and_then(|schema| {
			let field = schema.fields.get(state.selected_field)?.clone();
			if let simse_ui_core::config::settings_schema::FieldType::Select { options } = field.field_type {
				Some(options)
			} else {
				None
			}
		});
	let is_select = select_options.is_some();

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

	if value_type == "boolean" {
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
			"  Press space or \u{21b5} to toggle",
			Style::default().fg(Color::DarkGray),
		)));
	} else if is_select {
		// Select field: show current option with cycling indicator.
		if let Some(ref options) = select_options {
			let current_idx = state.select_index;
			for (i, option) in options.iter().enumerate() {
				let selected = i == current_idx;
				let prefix = if selected { "  \u{25cf} " } else { "  \u{25cb} " };
				let style = if selected {
					Style::default()
						.fg(Color::Cyan)
						.add_modifier(Modifier::BOLD)
				} else {
					Style::default().fg(Color::DarkGray)
				};
				lines.push(Line::from(Span::styled(
					format!("{prefix}{option}"),
					style,
				)));
			}
			lines.push(Line::from(""));
			lines.push(Line::from(Span::styled(
				"  Press space or \u{21b5} to cycle",
				Style::default().fg(Color::DarkGray),
			)));
		}
	} else {
		// Text/number input with cursor position.
		let edit_val = &state.edit_value;
		let cursor_pos = state.cursor;

		if edit_val.is_empty() {
			// Show placeholder with blinking cursor.
			lines.push(Line::from(vec![
				Span::styled("  Value: ", Style::default().fg(Color::DarkGray)),
				Span::styled(
					"\u{2588}",
					Style::default()
						.fg(Color::White)
						.add_modifier(Modifier::SLOW_BLINK),
				),
			]));
		} else {
			// Split the value at the cursor position to show the cursor inline.
			let before: String = edit_val.chars().take(char_count_at_byte(edit_val, cursor_pos)).collect();
			let after: String = edit_val[cursor_pos..].to_string();

			let mut spans = vec![
				Span::styled("  Value: ", Style::default().fg(Color::DarkGray)),
				Span::styled(before, Style::default().fg(Color::White)),
			];

			if cursor_pos < edit_val.len() {
				// Cursor is in the middle: highlight the character under cursor.
				let cursor_char: String = after.chars().take(1).collect();
				let rest: String = after.chars().skip(1).collect();
				spans.push(Span::styled(
					cursor_char,
					Style::default()
						.fg(Color::Black)
						.bg(Color::White),
				));
				if !rest.is_empty() {
					spans.push(Span::styled(rest, Style::default().fg(Color::White)));
				}
			} else {
				// Cursor is at the end: show blinking block.
				spans.push(Span::styled(
					"\u{2588}",
					Style::default()
						.fg(Color::White)
						.add_modifier(Modifier::SLOW_BLINK),
				));
			}

			lines.push(Line::from(spans));
		}
	}
}

/// Count the number of characters up to a given byte position.
fn char_count_at_byte(s: &str, byte_pos: usize) -> usize {
	s[..byte_pos.min(s.len())].chars().count()
}

/// Render key hints at the bottom of the overlay.
fn render_key_hints(lines: &mut Vec<Line<'static>>, state: &SettingsFormState) {
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
		SettingsLevel::Editing | SettingsLevel::ArrayEntry => {
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

	// ── char_count_at_byte ──────────────────────────

	#[test]
	fn char_count_at_byte_ascii() {
		assert_eq!(char_count_at_byte("hello", 0), 0);
		assert_eq!(char_count_at_byte("hello", 3), 3);
		assert_eq!(char_count_at_byte("hello", 5), 5);
	}

	#[test]
	fn char_count_at_byte_beyond_len() {
		assert_eq!(char_count_at_byte("hi", 10), 2);
	}

	// ── Render smoke tests ──────────────────────────

	#[test]
	fn render_settings_file_list_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let state = SettingsFormState::new();

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_settings_form(frame, area, &state);
			})
			.unwrap();
	}

	#[test]
	fn render_settings_field_list_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut state = SettingsFormState::new();
		state.level = SettingsLevel::FieldList;
		state.config_data = serde_json::json!({
			"host": "localhost",
			"port": 8080,
			"debug": true
		});

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_settings_form(frame, area, &state);
			})
			.unwrap();
	}

	#[test]
	fn render_settings_editing_text_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut state = SettingsFormState::new();
		state.level = SettingsLevel::Editing;
		state.edit_value = "localhost".to_string();
		state.cursor = 9;
		state.config_data = serde_json::json!({"host": "localhost"});

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_settings_form(frame, area, &state);
			})
			.unwrap();
	}

	#[test]
	fn render_settings_editing_boolean_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut state = SettingsFormState::new();
		state.level = SettingsLevel::Editing;
		state.edit_value = "true".to_string();
		state.cursor = 4;
		state.config_data = serde_json::json!({"debug": true});

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_settings_form(frame, area, &state);
			})
			.unwrap();
	}

	#[test]
	fn render_settings_empty_config_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut state = SettingsFormState::new();
		state.level = SettingsLevel::FieldList;
		state.config_data = serde_json::json!({});

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_settings_form(frame, area, &state);
			})
			.unwrap();
	}

	#[test]
	fn render_settings_small_terminal_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(30, 10);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let state = SettingsFormState::new();

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_settings_form(frame, area, &state);
			})
			.unwrap();
	}

	#[test]
	fn render_settings_with_saved_indicator_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut state = SettingsFormState::new();
		state.saved_indicator = Some(std::time::Instant::now());

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_settings_form(frame, area, &state);
			})
			.unwrap();
	}

	#[test]
	fn render_settings_with_many_fields_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 30);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut state = SettingsFormState::new();
		state.level = SettingsLevel::FieldList;

		let mut data = serde_json::Map::new();
		for i in 0..20 {
			data.insert(
				format!("field_{i}"),
				serde_json::Value::String(format!("value_{i}")),
			);
		}
		state.config_data = serde_json::Value::Object(data);

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_settings_form(frame, area, &state);
			})
			.unwrap();
	}

	#[test]
	fn render_settings_editing_with_no_fields_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut state = SettingsFormState::new();
		state.level = SettingsLevel::Editing;
		state.edit_value = "test".to_string();
		state.cursor = 4;
		state.config_data = serde_json::json!({});

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_settings_form(frame, area, &state);
			})
			.unwrap();
	}

	#[test]
	fn render_settings_editing_with_long_value_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(60, 20);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut state = SettingsFormState::new();
		state.level = SettingsLevel::Editing;
		let long_val = "a".repeat(200);
		state.edit_value = long_val.clone();
		state.cursor = long_val.len();
		state.config_data = serde_json::json!({"key": long_val});

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_settings_form(frame, area, &state);
			})
			.unwrap();
	}

	#[test]
	fn render_settings_with_error_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut state = SettingsFormState::new();
		state.error = Some("Something went wrong".to_string());

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_settings_form(frame, area, &state);
			})
			.unwrap();
	}

	#[test]
	fn render_settings_editing_with_cursor_in_middle_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut state = SettingsFormState::new();
		state.level = SettingsLevel::Editing;
		state.edit_value = "hello world".to_string();
		state.cursor = 5; // cursor between "hello" and " world"
		state.config_data = serde_json::json!({"key": "hello world"});

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_settings_form(frame, area, &state);
			})
			.unwrap();
	}
}
