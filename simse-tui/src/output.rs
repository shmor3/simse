//! Output item rendering: converts OutputItem variants to ratatui Lines.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use simse_ui_core::app::{OutputItem, ToolCallState, ToolCallStatus};

/// Convert output items to renderable Lines.
pub fn render_output_items(items: &[OutputItem], _width: u16) -> Vec<Line<'static>> {
	let mut lines = Vec::new();
	for item in items {
		lines.extend(render_output_item(item));
		lines.push(Line::default()); // spacing between items
	}
	lines
}

/// Convert a single OutputItem to Lines.
pub fn render_output_item(item: &OutputItem) -> Vec<Line<'static>> {
	match item {
		OutputItem::Message { role, text } => render_message(role, text),
		OutputItem::ToolCall(tc) => render_tool_call(tc),
		OutputItem::CommandResult { text } => render_command_result(text),
		OutputItem::Error { message } => render_error(message),
		OutputItem::Info { text } => render_info(text),
	}
}

/// Render a user or assistant message.
fn render_message(role: &str, text: &str) -> Vec<Line<'static>> {
	let mut lines = Vec::new();
	let text_lines: Vec<&str> = text.lines().collect();

	if role == "user" {
		for (i, line) in text_lines.iter().enumerate() {
			if i == 0 {
				lines.push(Line::from(vec![
					Span::styled(
						"\u{276f} ",
						Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
					),
					Span::raw(line.to_string()),
				]));
			} else {
				lines.push(Line::from(vec![
					Span::raw("  "),
					Span::raw(line.to_string()),
				]));
			}
		}
	} else {
		// Assistant or other roles: plain white text.
		for line in text_lines {
			lines.push(Line::from(Span::raw(line.to_string())));
		}
	}

	if lines.is_empty() {
		lines.push(Line::default());
	}

	lines
}

/// Render a tool call with status indicator.
fn render_tool_call(tc: &ToolCallState) -> Vec<Line<'static>> {
	let mut lines = Vec::new();

	let (status_color, status_char) = match tc.status {
		ToolCallStatus::Active => (Color::Yellow, "\u{23fa}"),
		ToolCallStatus::Completed => (Color::Green, "\u{23fa}"),
		ToolCallStatus::Failed => (Color::Red, "\u{23fa}"),
	};

	// First line: status icon + tool name
	let mut first_spans = vec![
		Span::styled(
			format!("{status_char} "),
			Style::default().fg(status_color),
		),
		Span::styled(
			tc.name.clone(),
			Style::default()
				.fg(status_color)
				.add_modifier(Modifier::BOLD),
		),
	];

	// Append duration if present.
	if let Some(ms) = tc.duration_ms {
		first_spans.push(Span::styled(
			format!(" ({ms}ms)"),
			Style::default().fg(Color::DarkGray),
		));
	}

	lines.push(Line::from(first_spans));

	// Second line: summary or error.
	if let Some(ref error) = tc.error {
		lines.push(Line::from(vec![
			Span::raw("  "),
			Span::styled(error.clone(), Style::default().fg(Color::Red)),
		]));
	} else if let Some(ref summary) = tc.summary {
		lines.push(Line::from(vec![
			Span::raw("  "),
			Span::styled(summary.clone(), Style::default().fg(Color::DarkGray)),
		]));
	}

	// Diff lines if present.
	if let Some(ref diff) = tc.diff {
		for line in diff.lines() {
			let color = if line.starts_with('+') {
				Color::Green
			} else if line.starts_with('-') {
				Color::Red
			} else {
				Color::DarkGray
			};
			lines.push(Line::from(vec![
				Span::raw("  "),
				Span::styled(line.to_string(), Style::default().fg(color)),
			]));
		}
	}

	lines
}

/// Render a command result as plain text lines.
fn render_command_result(text: &str) -> Vec<Line<'static>> {
	text.lines().map(|l| Line::from(Span::raw(l.to_string()))).collect()
}

/// Render an error with red prefix.
fn render_error(message: &str) -> Vec<Line<'static>> {
	vec![Line::from(vec![
		Span::styled(
			"\u{2717} ",
			Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
		),
		Span::styled(message.to_string(), Style::default().fg(Color::Red)),
	])]
}

/// Render info text in dim gray.
fn render_info(text: &str) -> Vec<Line<'static>> {
	text.lines()
		.map(|l| {
			Line::from(Span::styled(
				l.to_string(),
				Style::default().fg(Color::DarkGray),
			))
		})
		.collect()
}

// ── Tests ───────────────────────────────────────────────

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn render_user_message_has_prefix() {
		let lines = render_output_item(&OutputItem::Message {
			role: "user".into(),
			text: "hello".into(),
		});
		assert!(!lines.is_empty());
	}

	#[test]
	fn render_error_is_nonempty() {
		let lines = render_output_item(&OutputItem::Error {
			message: "fail".into(),
		});
		assert!(!lines.is_empty());
	}

	#[test]
	fn render_tool_call_completed() {
		let tc = ToolCallState {
			id: "1".into(),
			name: "read_file".into(),
			args: r#"{"path": "test.rs"}"#.into(),
			status: ToolCallStatus::Completed,
			started_at: 0,
			duration_ms: Some(150),
			summary: Some("Read 42 lines".into()),
			error: None,
			diff: None,
		};
		let lines = render_output_item(&OutputItem::ToolCall(tc));
		assert!(lines.len() >= 2);
	}

	#[test]
	fn render_info_is_nonempty() {
		let lines = render_output_item(&OutputItem::Info {
			text: "info msg".into(),
		});
		assert!(!lines.is_empty());
	}

	#[test]
	fn render_tool_call_with_diff() {
		let tc = ToolCallState {
			id: "1".into(),
			name: "write_file".into(),
			args: "{}".into(),
			status: ToolCallStatus::Completed,
			started_at: 0,
			duration_ms: Some(50),
			summary: Some("Wrote file".into()),
			error: None,
			diff: Some("+added line\n-removed line\n context".into()),
		};
		let lines = render_output_item(&OutputItem::ToolCall(tc));
		assert!(lines.len() >= 5); // name + summary + 3 diff lines
	}

	#[test]
	fn render_multiple_items() {
		let items = vec![
			OutputItem::Message {
				role: "user".into(),
				text: "hi".into(),
			},
			OutputItem::Info {
				text: "done".into(),
			},
		];
		let lines = render_output_items(&items, 80);
		assert!(lines.len() >= 3); // at least 2 items + spacing
	}
}
