//! Permission dialog: centered overlay popup for tool permission requests.
//!
//! Displayed when the agent wants to execute a tool that requires user approval.
//! The dialog shows the tool name, primary argument, formatted args, and key hints.
//!
//! # Layout
//!
//! ```text
//! ┌─ Permission Required ──────────────────────┐
//! │                                             │
//! │  ⚠ simse wants to run {tool}({primary_arg}) │
//! │                                             │
//! │  Arguments:                                 │
//! │    command: "ls -la"                        │
//! │    path: "/home/user"                       │
//! │                                             │
//! │  [y] Allow  [n] Deny  [a] Allow always      │
//! └─────────────────────────────────────────────┘
//! ```

use ratatui::{
	layout::Rect,
	style::{Color, Modifier, Style},
	text::{Line, Span},
	widgets::{Block, Borders, Clear, Paragraph, Wrap},
	Frame,
};
use crate::ui_core::app::PermissionRequest;

// ── Constants ───────────────────────────────────────────

/// Keys commonly used as the "primary" argument for display in the header.
const PRIMARY_ARG_KEYS: &[&str] = &["command", "path", "file_path", "query", "name"];

/// Maximum width of the permission dialog popup.
const MAX_POPUP_WIDTH: u16 = 60;

/// Minimum width of the permission dialog popup.
const MIN_POPUP_WIDTH: u16 = 30;

/// Maximum number of arg lines to display before truncating.
const MAX_ARG_LINES: usize = 12;

/// Maximum character length for a displayed arg value before truncating.
const MAX_ARG_VALUE_LEN: usize = 60;

// ── State ───────────────────────────────────────────────

/// State for the permission dialog overlay.
#[derive(Debug, Clone)]
pub struct PermissionDialogState {
	pub request: PermissionRequest,
	pub visible: bool,
}

impl PermissionDialogState {
	/// Create a new visible permission dialog for the given request.
	pub fn new(request: PermissionRequest) -> Self {
		Self {
			request,
			visible: true,
		}
	}

	/// Dismiss the dialog.
	pub fn dismiss(&mut self) {
		self.visible = false;
	}
}

// ── Rendering ───────────────────────────────────────────

/// Render the permission dialog as a centered overlay popup.
///
/// The dialog is rendered on top of whatever is behind it (using `Clear`).
/// It shows the tool name, primary argument, formatted args, and key hints.
pub fn render_permission_dialog(frame: &mut Frame, area: Rect, request: &PermissionRequest) {
	let primary_arg = extract_primary_arg(&request.args);
	let tool_display = match &primary_arg {
		Some(arg) => format!("{}({})", request.tool_name, arg),
		None => request.tool_name.clone(),
	};

	// Build dialog content lines.
	let mut lines: Vec<Line<'static>> = Vec::new();

	// Blank line for padding.
	lines.push(Line::from(""));

	// Header: warning icon + tool display.
	lines.push(Line::from(vec![
		Span::styled(
			"\u{26a0} ",
			Style::default()
				.fg(Color::Yellow)
				.add_modifier(Modifier::BOLD),
		),
		Span::raw("simse wants to run "),
		Span::styled(
			tool_display,
			Style::default().add_modifier(Modifier::BOLD),
		),
	]));

	// Blank separator.
	lines.push(Line::from(""));

	// Arguments section.
	let arg_lines = format_args(&request.args);
	if !arg_lines.is_empty() {
		lines.push(Line::from(Span::styled(
			"  Arguments:",
			Style::default()
				.fg(Color::DarkGray)
				.add_modifier(Modifier::BOLD),
		)));

		let truncated = arg_lines.len() > MAX_ARG_LINES;
		for line in arg_lines.into_iter().take(MAX_ARG_LINES) {
			lines.push(line);
		}
		if truncated {
			lines.push(Line::from(Span::styled(
				"    ...",
				Style::default().fg(Color::DarkGray),
			)));
		}

		lines.push(Line::from(""));
	}

	// Key hints line.
	// Always show y/n.
	let mut hint_spans = vec![Span::raw("  ")];
	hint_spans.push(Span::styled(
		"[y]",
		Style::default()
			.fg(Color::Green)
			.add_modifier(Modifier::BOLD),
	));
	hint_spans.push(Span::raw(" Allow  "));
	hint_spans.push(Span::styled(
		"[n]",
		Style::default()
			.fg(Color::Red)
			.add_modifier(Modifier::BOLD),
	));
	hint_spans.push(Span::raw(" Deny"));

	// Show [a] if the request has an "allow_always" option.
	let has_always = request.options.iter().any(|o| o.id == "allow_always");
	if has_always {
		hint_spans.push(Span::raw("  "));
		hint_spans.push(Span::styled(
			"[a]",
			Style::default()
				.fg(Color::Cyan)
				.add_modifier(Modifier::BOLD),
		));
		hint_spans.push(Span::raw(" Allow always"));
	}

	lines.push(Line::from(hint_spans));

	// Trailing padding.
	lines.push(Line::from(""));

	// Calculate popup dimensions.
	let content_height = lines.len() as u16 + 2; // +2 for border top/bottom
	let popup_width = MAX_POPUP_WIDTH.min(area.width.saturating_sub(4)).max(MIN_POPUP_WIDTH);
	let popup_height = content_height.min(area.height.saturating_sub(4));

	// Center the popup.
	let popup_x = (area.width.saturating_sub(popup_width)) / 2;
	let popup_y = (area.height.saturating_sub(popup_height)) / 2;
	let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

	// Clear the area behind the popup, then render.
	frame.render_widget(Clear, popup_area);

	let popup = Paragraph::new(lines)
		.wrap(Wrap { trim: false })
		.block(
			Block::default()
				.borders(Borders::ALL)
				.border_style(Style::default().fg(Color::Yellow))
				.title(" Permission Required "),
		);

	frame.render_widget(popup, popup_area);
}

// ── Helpers ─────────────────────────────────────────────

/// Extract the primary argument value from a JSON args object.
///
/// Looks for well-known keys (`command`, `path`, `file_path`, `query`, `name`)
/// and returns the first string value found.
fn extract_primary_arg(args: &serde_json::Value) -> Option<String> {
	let obj = args.as_object()?;
	for key in PRIMARY_ARG_KEYS {
		if let Some(serde_json::Value::String(val)) = obj.get(*key)
			&& !val.is_empty() {
				return Some(truncate_str(val, MAX_ARG_VALUE_LEN));
			}
	}
	None
}

/// Format a JSON args value as indented key-value lines for display.
fn format_args(args: &serde_json::Value) -> Vec<Line<'static>> {
	let mut lines = Vec::new();

	match args {
		serde_json::Value::Object(map) => {
			for (key, value) in map {
				let val_str = format_json_value(value);
				let truncated = truncate_str(&val_str, MAX_ARG_VALUE_LEN);
				lines.push(Line::from(vec![
					Span::styled(
						format!("    {key}"),
						Style::default().fg(Color::Cyan),
					),
					Span::styled(": ", Style::default().fg(Color::DarkGray)),
					Span::styled(truncated, Style::default().fg(Color::White)),
				]));
			}
		}
		serde_json::Value::Null => {}
		other => {
			let val_str = format_json_value(other);
			let truncated = truncate_str(&val_str, MAX_ARG_VALUE_LEN);
			lines.push(Line::from(vec![
				Span::raw("    "),
				Span::styled(truncated, Style::default().fg(Color::White)),
			]));
		}
	}

	lines
}

/// Format a JSON value for display: strings without quotes wrapping, others as compact JSON.
fn format_json_value(value: &serde_json::Value) -> String {
	match value {
		serde_json::Value::String(s) => format!("\"{s}\""),
		serde_json::Value::Null => "null".into(),
		serde_json::Value::Bool(b) => b.to_string(),
		serde_json::Value::Number(n) => n.to_string(),
		serde_json::Value::Array(arr) => {
			if arr.is_empty() {
				"[]".into()
			} else {
				serde_json::to_string(value).unwrap_or_else(|_| "[...]".into())
			}
		}
		serde_json::Value::Object(map) => {
			if map.is_empty() {
				"{}".into()
			} else {
				serde_json::to_string(value).unwrap_or_else(|_| "{...}".into())
			}
		}
	}
}

/// Truncate a string to `max_len` characters, appending "..." if truncated.
fn truncate_str(s: &str, max_len: usize) -> String {
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
	use crate::ui_core::app::PermissionOption;

	fn make_request(tool: &str, args: serde_json::Value) -> PermissionRequest {
		PermissionRequest {
			id: "req-1".into(),
			tool_name: tool.into(),
			args,
			options: vec![
				PermissionOption {
					id: "allow_once".into(),
					label: "Allow once".into(),
				},
				PermissionOption {
					id: "deny".into(),
					label: "Deny".into(),
				},
				PermissionOption {
					id: "allow_always".into(),
					label: "Allow always".into(),
				},
			],
		}
	}

	// ── extract_primary_arg ─────────────────────────

	#[test]
	fn extract_primary_arg_finds_command() {
		let args = serde_json::json!({"command": "ls -la", "cwd": "/tmp"});
		assert_eq!(extract_primary_arg(&args), Some("ls -la".into()));
	}

	#[test]
	fn extract_primary_arg_finds_path() {
		let args = serde_json::json!({"path": "/home/user/file.rs"});
		assert_eq!(
			extract_primary_arg(&args),
			Some("/home/user/file.rs".into())
		);
	}

	#[test]
	fn extract_primary_arg_finds_file_path() {
		let args = serde_json::json!({"file_path": "src/main.rs"});
		assert_eq!(extract_primary_arg(&args), Some("src/main.rs".into()));
	}

	#[test]
	fn extract_primary_arg_finds_query() {
		let args = serde_json::json!({"query": "search term"});
		assert_eq!(extract_primary_arg(&args), Some("search term".into()));
	}

	#[test]
	fn extract_primary_arg_finds_name() {
		let args = serde_json::json!({"name": "test-project"});
		assert_eq!(extract_primary_arg(&args), Some("test-project".into()));
	}

	#[test]
	fn extract_primary_arg_priority_order() {
		// "command" should take priority over "path".
		let args = serde_json::json!({"path": "/tmp", "command": "echo hello"});
		assert_eq!(extract_primary_arg(&args), Some("echo hello".into()));
	}

	#[test]
	fn extract_primary_arg_none_when_no_match() {
		let args = serde_json::json!({"other_key": "value"});
		assert_eq!(extract_primary_arg(&args), None);
	}

	#[test]
	fn extract_primary_arg_none_when_null() {
		let args = serde_json::Value::Null;
		assert_eq!(extract_primary_arg(&args), None);
	}

	#[test]
	fn extract_primary_arg_skips_empty_string() {
		let args = serde_json::json!({"command": "", "path": "/tmp"});
		assert_eq!(extract_primary_arg(&args), Some("/tmp".into()));
	}

	#[test]
	fn extract_primary_arg_skips_non_string() {
		let args = serde_json::json!({"command": 42, "path": "/tmp"});
		assert_eq!(extract_primary_arg(&args), Some("/tmp".into()));
	}

	#[test]
	fn extract_primary_arg_truncates_long_value() {
		let long_cmd = "a".repeat(100);
		let args = serde_json::json!({"command": long_cmd});
		let result = extract_primary_arg(&args).unwrap();
		assert!(result.len() <= MAX_ARG_VALUE_LEN);
		assert!(result.ends_with("..."));
	}

	// ── format_args ─────────────────────────────────

	#[test]
	fn format_args_object() {
		let args = serde_json::json!({"path": "/tmp", "recursive": true});
		let lines = format_args(&args);
		assert_eq!(lines.len(), 2);

		let text0: String = lines[0].spans.iter().map(|s| s.content.to_string()).collect();
		let text1: String = lines[1].spans.iter().map(|s| s.content.to_string()).collect();
		let combined = format!("{text0} {text1}");
		assert!(combined.contains("path"));
		assert!(combined.contains("recursive"));
	}

	#[test]
	fn format_args_empty_object() {
		let args = serde_json::json!({});
		let lines = format_args(&args);
		assert!(lines.is_empty());
	}

	#[test]
	fn format_args_null() {
		let args = serde_json::Value::Null;
		let lines = format_args(&args);
		assert!(lines.is_empty());
	}

	#[test]
	fn format_args_non_object() {
		let args = serde_json::json!("raw string");
		let lines = format_args(&args);
		assert_eq!(lines.len(), 1);
	}

	// ── format_json_value ───────────────────────────

	#[test]
	fn format_json_value_string() {
		let val = serde_json::json!("hello");
		assert_eq!(format_json_value(&val), "\"hello\"");
	}

	#[test]
	fn format_json_value_number() {
		let val = serde_json::json!(42);
		assert_eq!(format_json_value(&val), "42");
	}

	#[test]
	fn format_json_value_bool() {
		assert_eq!(format_json_value(&serde_json::json!(true)), "true");
		assert_eq!(format_json_value(&serde_json::json!(false)), "false");
	}

	#[test]
	fn format_json_value_null() {
		assert_eq!(format_json_value(&serde_json::Value::Null), "null");
	}

	#[test]
	fn format_json_value_empty_array() {
		assert_eq!(format_json_value(&serde_json::json!([])), "[]");
	}

	#[test]
	fn format_json_value_array() {
		let val = serde_json::json!([1, 2, 3]);
		let result = format_json_value(&val);
		assert!(result.contains("1"));
		assert!(result.contains("2"));
		assert!(result.contains("3"));
	}

	#[test]
	fn format_json_value_empty_object() {
		assert_eq!(format_json_value(&serde_json::json!({})), "{}");
	}

	#[test]
	fn format_json_value_object() {
		let val = serde_json::json!({"key": "val"});
		let result = format_json_value(&val);
		assert!(result.contains("key"));
		assert!(result.contains("val"));
	}

	// ── truncate_str ────────────────────────────────

	#[test]
	fn truncate_str_short() {
		assert_eq!(truncate_str("hello", 10), "hello");
	}

	#[test]
	fn truncate_str_exact() {
		assert_eq!(truncate_str("hello", 5), "hello");
	}

	#[test]
	fn truncate_str_long() {
		let result = truncate_str("hello world", 8);
		assert_eq!(result, "hello...");
		assert!(result.len() <= 8);
	}

	#[test]
	fn truncate_str_very_short_max() {
		let result = truncate_str("hello", 3);
		assert_eq!(result, "...");
	}

	// ── PermissionDialogState ───────────────────────

	#[test]
	fn state_new_is_visible() {
		let req = make_request("bash", serde_json::json!({"command": "ls"}));
		let state = PermissionDialogState::new(req.clone());
		assert!(state.visible);
		assert_eq!(state.request.tool_name, "bash");
	}

	#[test]
	fn state_dismiss_hides() {
		let req = make_request("bash", serde_json::json!({}));
		let mut state = PermissionDialogState::new(req);
		assert!(state.visible);
		state.dismiss();
		assert!(!state.visible);
	}

	// ── render_permission_dialog (smoke test) ───────

	#[test]
	fn render_permission_dialog_does_not_panic() {
		// We cannot fully test Frame rendering without a real terminal backend,
		// but we can ensure the function does not panic with valid inputs.
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();

		let req = make_request(
			"bash",
			serde_json::json!({"command": "rm -rf /", "cwd": "/home"}),
		);

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_permission_dialog(frame, area, &req);
			})
			.unwrap();
	}

	#[test]
	fn render_with_no_args_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();

		let req = make_request("custom_tool", serde_json::json!(null));

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_permission_dialog(frame, area, &req);
			})
			.unwrap();
	}

	#[test]
	fn render_with_many_args_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(60, 20);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();

		let mut args = serde_json::Map::new();
		for i in 0..20 {
			args.insert(
				format!("arg_{i}"),
				serde_json::Value::String(format!("value_{i}")),
			);
		}
		let req = make_request("many_args_tool", serde_json::Value::Object(args));

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_permission_dialog(frame, area, &req);
			})
			.unwrap();
	}

	#[test]
	fn render_without_allow_always_option() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();

		let req = PermissionRequest {
			id: "req-2".into(),
			tool_name: "read_file".into(),
			args: serde_json::json!({"path": "/etc/passwd"}),
			options: vec![
				PermissionOption {
					id: "allow_once".into(),
					label: "Allow once".into(),
				},
				PermissionOption {
					id: "deny".into(),
					label: "Deny".into(),
				},
			],
		};

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_permission_dialog(frame, area, &req);
			})
			.unwrap();
	}

	#[test]
	fn render_small_terminal_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(30, 10);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();

		let req = make_request(
			"bash",
			serde_json::json!({"command": "echo hello"}),
		);

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_permission_dialog(frame, area, &req);
			})
			.unwrap();
	}

	#[test]
	fn render_with_long_tool_name_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();

		let long_name = "a".repeat(100);
		let req = make_request(&long_name, serde_json::json!({"path": "/tmp"}));

		terminal
			.draw(|frame| {
				let area = frame.area();
				render_permission_dialog(frame, area, &req);
			})
			.unwrap();
	}
}
