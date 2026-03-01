//! Elm Architecture: Model, Update, View.

use ratatui::{
	layout::{Constraint, Direction, Layout, Rect},
	style::{Color, Modifier, Style},
	text::{Line, Span},
	widgets::{Block, Borders, Clear, Paragraph, Wrap},
	Frame,
};
use simse_ui_core::app::{
	OutputItem, PermissionRequest, ToolCallState, ToolCallStatus,
};
use simse_ui_core::commands::registry::{
	all_commands, find_command, parse_bool_arg, CommandCategory, CommandDefinition,
};
use simse_ui_core::input::state as input;
use std::collections::BTreeMap;

use crate::banner;
use crate::output;

// ── Screen ──────────────────────────────────────────────

/// Which screen/overlay is currently active.
#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
	Chat,
	Shortcuts,
	Settings,
	Confirm { message: String },
	Permission(PermissionRequest),
}

// ── PromptMode ──────────────────────────────────────────

/// Input prompt mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptMode {
	Normal,
	Autocomplete {
		selected: usize,
		matches: Vec<String>,
	},
}

// ── LoopStatus ──────────────────────────────────────────

/// Current status of the agentic loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoopStatus {
	Idle,
	Streaming,
	ToolExecuting,
}

// ── App (the Model) ─────────────────────────────────────

/// Application state (the Model).
pub struct App {
	pub input: input::InputState,
	pub output: Vec<OutputItem>,
	pub stream_text: String,
	pub active_tool_calls: Vec<ToolCallState>,
	pub loop_status: LoopStatus,
	pub screen: Screen,
	pub prompt_mode: PromptMode,
	pub scroll_offset: usize,
	pub should_quit: bool,
	pub ctrl_c_pending: bool,
	pub plan_mode: bool,
	pub verbose: bool,
	pub permission_mode: String,
	pub total_tokens: u64,
	pub context_percent: u8,
	pub history: Vec<String>,
	pub history_index: Option<usize>,
	pub history_draft: String,
	pub commands: Vec<CommandDefinition>,
	pub banner_visible: bool,
	pub version: String,
	pub server_name: Option<String>,
	pub model_name: Option<String>,
}

impl App {
	pub fn new() -> Self {
		Self {
			input: input::InputState::default(),
			output: Vec::new(),
			stream_text: String::new(),
			active_tool_calls: Vec::new(),
			loop_status: LoopStatus::Idle,
			screen: Screen::Chat,
			prompt_mode: PromptMode::Normal,
			scroll_offset: 0,
			should_quit: false,
			ctrl_c_pending: false,
			plan_mode: false,
			verbose: false,
			permission_mode: "ask".into(),
			total_tokens: 0,
			context_percent: 0,
			history: Vec::new(),
			history_index: None,
			history_draft: String::new(),
			commands: all_commands(),
			banner_visible: true,
			version: env!("CARGO_PKG_VERSION").into(),
			server_name: None,
			model_name: None,
		}
	}
}

// ── AppMessage ──────────────────────────────────────────

/// Messages the app can receive.
pub enum AppMessage {
	// Input
	CharInput(char),
	Paste(String),
	Submit,
	Backspace,
	Delete,
	DeleteWordBack,
	CursorLeft,
	CursorRight,
	WordLeft,
	WordRight,
	Home,
	End,
	SelectLeft,
	SelectRight,
	SelectHome,
	SelectEnd,
	SelectAll,
	HistoryUp,
	HistoryDown,

	// Navigation
	ScrollUp(usize),
	ScrollDown(usize),
	ScrollToBottom,

	// App control
	CtrlC,
	CtrlCTimeout,
	Escape,
	CtrlL,
	ShiftTab,
	Tab,
	Quit,

	// Screen transitions
	ShowShortcuts,
	DismissOverlay,

	// Loop events (from bridge)
	StreamStart,
	StreamDelta(String),
	StreamEnd {
		text: String,
	},
	ToolCallStart(ToolCallState),
	ToolCallEnd {
		id: String,
		status: ToolCallStatus,
		summary: Option<String>,
		error: Option<String>,
		duration_ms: Option<u64>,
		diff: Option<String>,
	},
	TokenUsage {
		prompt: u64,
		completion: u64,
	},
	LoopComplete,
	LoopError(String),

	// Permission
	PermissionRequested(PermissionRequest),
	PermissionResponse {
		id: String,
		option_id: String,
	},

	// Resize
	Resize {
		width: u16,
		height: u16,
	},
}

// ── Update ──────────────────────────────────────────────

/// Update: pure function from (Model, Message) -> Model.
pub fn update(mut app: App, msg: AppMessage) -> App {
	// Any user action resets the ctrl-c pending state (except CtrlC itself and CtrlCTimeout).
	match &msg {
		AppMessage::CtrlC | AppMessage::CtrlCTimeout => {}
		_ => {
			app.ctrl_c_pending = false;
		}
	}

	match msg {
		// ── Input ────────────────────────────────────
		AppMessage::CharInput(c) => {
			if c == '?' && app.input.value.is_empty() {
				app.screen = Screen::Shortcuts;
			} else {
				app.input = input::insert(&app.input, &c.to_string());
			}
		}
		AppMessage::Paste(text) => {
			app.input = input::insert(&app.input, &text);
		}
		AppMessage::Submit => {
			let text = app.input.value.trim().to_string();
			if text.is_empty() {
				return app;
			}

			// Add to history (dedup consecutive, cap at 100).
			if app.history.last().map_or(true, |last| *last != text) {
				app.history.push(text.clone());
				if app.history.len() > 100 {
					app.history.remove(0);
				}
			}
			app.history_index = None;
			app.history_draft.clear();

			// Clear input.
			app.input = input::InputState::default();
			app.banner_visible = false;

			// Handle "exit" / "quit" bare words.
			let lower = text.to_lowercase();
			if lower == "exit" || lower == "quit" {
				app.should_quit = true;
				return app;
			}

			// Command dispatch.
			if text.starts_with('/') {
				app = dispatch_command(app, &text);
			} else {
				// Regular user message.
				app.output.push(OutputItem::Message {
					role: "user".into(),
					text,
				});
			}
		}
		AppMessage::Backspace => {
			app.input = input::backspace(&app.input);
		}
		AppMessage::Delete => {
			app.input = input::delete(&app.input);
		}
		AppMessage::DeleteWordBack => {
			app.input = input::delete_word_back(&app.input);
		}
		AppMessage::CursorLeft => {
			app.input = input::move_left(&app.input, false);
		}
		AppMessage::CursorRight => {
			app.input = input::move_right(&app.input, false);
		}
		AppMessage::WordLeft => {
			app.input = input::move_word_left(&app.input, false);
		}
		AppMessage::WordRight => {
			app.input = input::move_word_right(&app.input, false);
		}
		AppMessage::Home => {
			app.input = input::move_home(&app.input, false);
		}
		AppMessage::End => {
			app.input = input::move_end(&app.input, false);
		}
		AppMessage::SelectLeft => {
			app.input = input::move_left(&app.input, true);
		}
		AppMessage::SelectRight => {
			app.input = input::move_right(&app.input, true);
		}
		AppMessage::SelectHome => {
			app.input = input::move_home(&app.input, true);
		}
		AppMessage::SelectEnd => {
			app.input = input::move_end(&app.input, true);
		}
		AppMessage::SelectAll => {
			app.input = input::select_all(&app.input);
		}
		AppMessage::HistoryUp => {
			if app.history.is_empty() {
				return app;
			}
			match app.history_index {
				None => {
					// Save current input as draft.
					app.history_draft = app.input.value.clone();
					let idx = app.history.len() - 1;
					app.history_index = Some(idx);
					let text = app.history[idx].clone();
					app.input = input::InputState::default();
					app.input = input::insert(&app.input, &text);
				}
				Some(idx) if idx > 0 => {
					let new_idx = idx - 1;
					app.history_index = Some(new_idx);
					let text = app.history[new_idx].clone();
					app.input = input::InputState::default();
					app.input = input::insert(&app.input, &text);
				}
				_ => {}
			}
		}
		AppMessage::HistoryDown => {
			match app.history_index {
				Some(idx) => {
					if idx + 1 < app.history.len() {
						let new_idx = idx + 1;
						app.history_index = Some(new_idx);
						let text = app.history[new_idx].clone();
						app.input = input::InputState::default();
						app.input = input::insert(&app.input, &text);
					} else {
						// Past end: restore draft.
						app.history_index = None;
						let draft = app.history_draft.clone();
						app.input = input::InputState::default();
						app.input = input::insert(&app.input, &draft);
					}
				}
				None => {}
			}
		}

		// ── Navigation ──────────────────────────────
		AppMessage::ScrollUp(n) => {
			app.scroll_offset = app.scroll_offset.saturating_add(n);
		}
		AppMessage::ScrollDown(n) => {
			app.scroll_offset = app.scroll_offset.saturating_sub(n);
		}
		AppMessage::ScrollToBottom => {
			app.scroll_offset = 0;
		}

		// ── App control ─────────────────────────────
		AppMessage::CtrlC => {
			if app.ctrl_c_pending {
				app.should_quit = true;
			} else {
				app.ctrl_c_pending = true;
			}
		}
		AppMessage::CtrlCTimeout => {
			app.ctrl_c_pending = false;
		}
		AppMessage::Escape => {
			if app.screen != Screen::Chat {
				app.screen = Screen::Chat;
			} else if app.loop_status != LoopStatus::Idle {
				app.loop_status = LoopStatus::Idle;
				app.output.push(OutputItem::Info {
					text: "Interrupted.".into(),
				});
			}
		}
		AppMessage::CtrlL => {
			app.output.clear();
			app.banner_visible = true;
		}
		AppMessage::ShiftTab => {
			app.permission_mode = match app.permission_mode.as_str() {
				"ask" => "auto".into(),
				"auto" => "bypass".into(),
				_ => "ask".into(),
			};
		}
		AppMessage::Tab => {
			// Tab completion will be handled in a future task.
		}
		AppMessage::Quit => {
			app.should_quit = true;
		}

		// ── Screen transitions ──────────────────────
		AppMessage::ShowShortcuts => {
			app.screen = Screen::Shortcuts;
		}
		AppMessage::DismissOverlay => {
			app.screen = Screen::Chat;
		}

		// ── Loop events ─────────────────────────────
		AppMessage::StreamStart => {
			app.loop_status = LoopStatus::Streaming;
			app.stream_text.clear();
		}
		AppMessage::StreamDelta(delta) => {
			app.stream_text.push_str(&delta);
		}
		AppMessage::StreamEnd { text } => {
			app.output.push(OutputItem::Message {
				role: "assistant".into(),
				text,
			});
			app.stream_text.clear();
		}
		AppMessage::ToolCallStart(tc) => {
			app.active_tool_calls.push(tc);
			app.loop_status = LoopStatus::ToolExecuting;
		}
		AppMessage::ToolCallEnd {
			id,
			status,
			summary,
			error,
			duration_ms,
			diff,
		} => {
			if let Some(pos) = app.active_tool_calls.iter().position(|tc| tc.id == id) {
				let mut tc = app.active_tool_calls.remove(pos);
				tc.status = status;
				tc.summary = summary;
				tc.error = error;
				tc.duration_ms = duration_ms;
				tc.diff = diff;
				app.output.push(OutputItem::ToolCall(tc));
			}
		}
		AppMessage::TokenUsage { prompt, completion } => {
			app.total_tokens += prompt + completion;
		}
		AppMessage::LoopComplete => {
			app.loop_status = LoopStatus::Idle;
			// Move any remaining active tool calls to output.
			for tc in app.active_tool_calls.drain(..) {
				app.output.push(OutputItem::ToolCall(tc));
			}
		}
		AppMessage::LoopError(message) => {
			app.output.push(OutputItem::Error { message });
			app.loop_status = LoopStatus::Idle;
		}

		// ── Permission ──────────────────────────────
		AppMessage::PermissionRequested(req) => {
			app.screen = Screen::Permission(req);
		}
		AppMessage::PermissionResponse { .. } => {
			// Bridge will handle the actual response; dismiss the overlay.
			app.screen = Screen::Chat;
		}

		// ── Resize ──────────────────────────────────
		AppMessage::Resize { .. } => {
			// Resize is handled by the terminal framework.
		}
	}
	app
}

// ── Command dispatch ────────────────────────────────────

fn dispatch_command(mut app: App, text: &str) -> App {
	let without_slash = &text[1..];
	let mut parts = without_slash.splitn(2, ' ');
	let cmd_name = parts.next().unwrap_or("");
	let arg = parts.next().unwrap_or("").trim();

	match cmd_name.to_lowercase().as_str() {
		"help" | "?" => {
			let help = format_help_text(&app.commands);
			app.output.push(OutputItem::CommandResult { text: help });
		}
		"clear" => {
			app.output.clear();
			app.banner_visible = true;
		}
		"exit" | "quit" | "q" => {
			app.should_quit = true;
		}
		"verbose" | "v" => {
			match parse_bool_arg(arg, app.verbose) {
				Some(val) => {
					app.verbose = val;
					let state = if val { "on" } else { "off" };
					app.output.push(OutputItem::Info {
						text: format!("Verbose mode {state}."),
					});
				}
				None => {
					app.output.push(OutputItem::Error {
						message: format!("Invalid argument: {arg}. Use on/off/true/false."),
					});
				}
			}
		}
		"plan" => {
			match parse_bool_arg(arg, app.plan_mode) {
				Some(val) => {
					app.plan_mode = val;
					let state = if val { "on" } else { "off" };
					app.output.push(OutputItem::Info {
						text: format!("Plan mode {state}."),
					});
				}
				None => {
					app.output.push(OutputItem::Error {
						message: format!("Invalid argument: {arg}. Use on/off/true/false."),
					});
				}
			}
		}
		"context" => {
			let tokens = format_tokens(app.total_tokens);
			let ctx = app.context_percent;
			app.output.push(OutputItem::CommandResult {
				text: format!("Tokens: {tokens} | Context: {ctx}%"),
			});
		}
		"compact" => {
			app.output.push(OutputItem::Info {
				text: "Compaction requested.".into(),
			});
		}
		_ => {
			// Check if it's a registered command.
			if find_command(&app.commands, cmd_name).is_some() {
				app.output.push(OutputItem::Info {
					text: format!("/{cmd_name} is not yet implemented in TUI."),
				});
			} else {
				app.output.push(OutputItem::Error {
					message: format!("Unknown command: /{cmd_name}"),
				});
			}
		}
	}
	app
}

// ── Helpers ─────────────────────────────────────────────

/// Format help text grouped by category.
fn format_help_text(commands: &[CommandDefinition]) -> String {
	let mut groups: BTreeMap<String, Vec<&CommandDefinition>> = BTreeMap::new();
	for cmd in commands {
		if cmd.hidden {
			continue;
		}
		let cat = match cmd.category {
			CommandCategory::Meta => "Meta",
			CommandCategory::Library => "Library",
			CommandCategory::Tools => "Tools",
			CommandCategory::Session => "Session",
			CommandCategory::Config => "Config",
			CommandCategory::Files => "Files",
			CommandCategory::Ai => "AI",
		};
		groups.entry(cat.into()).or_default().push(cmd);
	}

	let mut out = String::from("Available commands:\n");
	for (cat, cmds) in &groups {
		out.push_str(&format!("\n  {cat}:\n"));
		for cmd in cmds {
			let aliases = if cmd.aliases.is_empty() {
				String::new()
			} else {
				format!(" ({})", cmd.aliases.join(", "))
			};
			out.push_str(&format!(
				"    /{}{} — {}\n",
				cmd.name, aliases, cmd.description
			));
		}
	}
	out
}

/// Format token count: 1000+ as "1.0k", etc.
pub fn format_tokens(tokens: u64) -> String {
	if tokens >= 1_000_000 {
		format!("{:.1}M", tokens as f64 / 1_000_000.0)
	} else if tokens >= 1_000 {
		format!("{:.1}k", tokens as f64 / 1_000.0)
	} else {
		tokens.to_string()
	}
}

// ── View ────────────────────────────────────────────────

/// View: render the model to the terminal.
pub fn view(app: &App, frame: &mut Frame) {
	let area = frame.area();
	let chunks = Layout::default()
		.direction(Direction::Vertical)
		.constraints([
			Constraint::Min(1),
			Constraint::Length(3),
			Constraint::Length(1),
		])
		.split(area);

	// 1. Chat area
	render_chat_area(app, frame, chunks[0]);

	// 2. Input
	render_input(app, frame, chunks[1]);

	// 3. Status bar
	let status = render_status_line(app, chunks[2].width);
	frame.render_widget(Paragraph::new(status), chunks[2]);

	// 4. Shortcuts overlay (on top of everything)
	if app.screen == Screen::Shortcuts {
		render_shortcuts_overlay(frame, area);
	}
}

/// Render the chat area: either the banner or scrollable output.
fn render_chat_area(app: &App, frame: &mut Frame, area: Rect) {
	if app.banner_visible && app.output.is_empty() {
		banner::render_banner(frame, area, app);
		return;
	}

	// Build all output lines.
	let mut lines = output::render_output_items(&app.output, area.width);

	// If streaming, append the in-progress stream text.
	if app.loop_status != LoopStatus::Idle && !app.stream_text.is_empty() {
		for line in app.stream_text.lines() {
			lines.push(Line::from(Span::raw(line.to_string())));
		}
	}

	// Show active tool calls.
	for tc in &app.active_tool_calls {
		let tc_lines = output::render_output_item(&simse_ui_core::app::OutputItem::ToolCall(
			tc.clone(),
		));
		lines.extend(tc_lines);
	}

	// Calculate scroll: we scroll from the bottom.
	let visible_height = area.height as usize;
	let total_lines = lines.len();
	let max_scroll = total_lines.saturating_sub(visible_height);
	let scroll = app.scroll_offset.min(max_scroll) as u16;

	let chat = Paragraph::new(lines)
		.wrap(Wrap { trim: false })
		.scroll((scroll, 0));
	frame.render_widget(chat, area);
}

/// Render the input area.
fn render_input(app: &App, frame: &mut Frame, area: Rect) {
	let input_display = if app.input.value.is_empty() {
		if app.ctrl_c_pending {
			Line::from(Span::styled(
				"Press Ctrl-C again to exit",
				Style::default().fg(Color::Yellow),
			))
		} else {
			Line::from(Span::styled(
				"Type a message...",
				Style::default().fg(Color::DarkGray),
			))
		}
	} else {
		Line::from(app.input.value.as_str())
	};
	let input_widget = Paragraph::new(input_display)
		.block(Block::default().borders(Borders::ALL).title("Input"));
	frame.render_widget(input_widget, area);

	// Cursor position
	let cursor_x = area.x + 1 + app.input.cursor as u16;
	let cursor_y = area.y + 1;
	frame.set_cursor_position((cursor_x, cursor_y));
}

/// Render a centered shortcuts overlay popup.
fn render_shortcuts_overlay(frame: &mut Frame, area: Rect) {
	let popup_width = 50u16.min(area.width.saturating_sub(4));
	let popup_height = 16u16.min(area.height.saturating_sub(4));
	let popup_x = (area.width.saturating_sub(popup_width)) / 2;
	let popup_y = (area.height.saturating_sub(popup_height)) / 2;

	let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

	let bold = Style::default().add_modifier(Modifier::BOLD);
	let dim = Style::default().fg(Color::DarkGray);
	let cyan = Style::default().fg(Color::Cyan);

	let lines = vec![
		Line::from(""),
		Line::from(Span::styled(" Keyboard Shortcuts", bold)),
		Line::from(""),
		Line::from(vec![
			Span::styled("  Enter       ", cyan),
			Span::styled("Send message", dim),
		]),
		Line::from(vec![
			Span::styled("  Escape      ", cyan),
			Span::styled("Interrupt / dismiss", dim),
		]),
		Line::from(vec![
			Span::styled("  Ctrl+C      ", cyan),
			Span::styled("Quit (press twice)", dim),
		]),
		Line::from(vec![
			Span::styled("  Ctrl+L      ", cyan),
			Span::styled("Clear screen", dim),
		]),
		Line::from(vec![
			Span::styled("  Shift+Tab   ", cyan),
			Span::styled("Cycle permission mode", dim),
		]),
		Line::from(vec![
			Span::styled("  PgUp/PgDn   ", cyan),
			Span::styled("Scroll output", dim),
		]),
		Line::from(vec![
			Span::styled("  Up/Down     ", cyan),
			Span::styled("History navigation", dim),
		]),
		Line::from(vec![
			Span::styled("  ?           ", cyan),
			Span::styled("Toggle this overlay", dim),
		]),
		Line::from(""),
		Line::from(Span::styled(
			"  Press Escape to close",
			Style::default().fg(Color::DarkGray),
		)),
	];

	// Clear the area behind the popup, then render the bordered block.
	frame.render_widget(Clear, popup_area);
	let popup = Paragraph::new(lines).block(
		Block::default()
			.borders(Borders::ALL)
			.border_style(Style::default().fg(Color::Cyan))
			.title(" Shortcuts "),
	);
	frame.render_widget(popup, popup_area);
}

fn render_status_line(app: &App, width: u16) -> Line<'static> {
	let sep = " \u{00b7} ";
	let mut hints = Vec::new();

	hints.push(format!("{} (shift+tab)", app.permission_mode));
	if app.loop_status != LoopStatus::Idle {
		hints.push("esc to interrupt".into());
	}
	if app.plan_mode {
		hints.push("plan mode".into());
	}
	if app.verbose {
		hints.push("verbose on".into());
	}
	hints.push("? for shortcuts".into());

	let left = hints.join(sep);

	let mut stats = Vec::new();
	if app.total_tokens > 0 {
		stats.push(format!("{} tokens", format_tokens(app.total_tokens)));
	}
	if app.context_percent > 0 {
		stats.push(format!("{}% context", app.context_percent));
	}
	let right = stats.join(sep);

	let gap = (width as usize).saturating_sub(left.len() + right.len() + 2);
	let full = format!(" {left}{}{right} ", " ".repeat(gap));

	Line::from(Span::styled(
		full,
		Style::default().fg(Color::DarkGray),
	))
}

// ── Tests ───────────────────────────────────────────────

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn new_app_defaults() {
		let app = App::new();
		assert_eq!(app.loop_status, LoopStatus::Idle);
		assert_eq!(app.screen, Screen::Chat);
		assert!(!app.should_quit);
		assert!(app.banner_visible);
		assert!(!app.commands.is_empty());
	}

	#[test]
	fn ctrl_c_double_press_quits() {
		let mut app = App::new();
		app = update(app, AppMessage::CtrlC);
		assert!(app.ctrl_c_pending);
		assert!(!app.should_quit);
		app = update(app, AppMessage::CtrlC);
		assert!(app.should_quit);
	}

	#[test]
	fn ctrl_c_timeout_resets() {
		let mut app = App::new();
		app = update(app, AppMessage::CtrlC);
		assert!(app.ctrl_c_pending);
		app = update(app, AppMessage::CtrlCTimeout);
		assert!(!app.ctrl_c_pending);
	}

	#[test]
	fn submit_adds_to_output_and_history() {
		let mut app = App::new();
		app.input = input::insert(&app.input, "hello world");
		app = update(app, AppMessage::Submit);
		assert!(app.input.value.is_empty());
		assert_eq!(app.history.len(), 1);
		assert_eq!(app.history[0], "hello world");
		assert!(!app.output.is_empty());
	}

	#[test]
	fn submit_empty_does_nothing() {
		let mut app = App::new();
		app = update(app, AppMessage::Submit);
		assert!(app.history.is_empty());
		assert!(app.output.is_empty());
	}

	#[test]
	fn escape_dismisses_overlay() {
		let mut app = App::new();
		app.screen = Screen::Shortcuts;
		app = update(app, AppMessage::Escape);
		assert_eq!(app.screen, Screen::Chat);
	}

	#[test]
	fn escape_interrupts_loop() {
		let mut app = App::new();
		app.loop_status = LoopStatus::Streaming;
		app = update(app, AppMessage::Escape);
		assert_eq!(app.loop_status, LoopStatus::Idle);
	}

	#[test]
	fn stream_delta_appends() {
		let mut app = App::new();
		app.loop_status = LoopStatus::Streaming;
		app = update(app, AppMessage::StreamDelta("hello ".into()));
		app = update(app, AppMessage::StreamDelta("world".into()));
		assert_eq!(app.stream_text, "hello world");
	}

	#[test]
	fn stream_end_moves_to_output() {
		let mut app = App::new();
		app.loop_status = LoopStatus::Streaming;
		app.stream_text = "partial".into();
		app = update(
			app,
			AppMessage::StreamEnd {
				text: "full response".into(),
			},
		);
		assert!(app.stream_text.is_empty());
		assert!(!app.output.is_empty());
	}

	#[test]
	fn history_navigation() {
		let mut app = App::new();
		app.history = vec!["first".into(), "second".into()];
		app.input = input::insert(&app.input, "draft");
		app = update(app, AppMessage::HistoryUp);
		assert_eq!(app.input.value, "second");
		app = update(app, AppMessage::HistoryUp);
		assert_eq!(app.input.value, "first");
		app = update(app, AppMessage::HistoryDown);
		assert_eq!(app.input.value, "second");
		app = update(app, AppMessage::HistoryDown);
		assert_eq!(app.input.value, "draft");
	}

	#[test]
	fn shift_tab_cycles_permission_mode() {
		let mut app = App::new();
		assert_eq!(app.permission_mode, "ask");
		app = update(app, AppMessage::ShiftTab);
		assert_eq!(app.permission_mode, "auto");
		app = update(app, AppMessage::ShiftTab);
		assert_eq!(app.permission_mode, "bypass");
		app = update(app, AppMessage::ShiftTab);
		assert_eq!(app.permission_mode, "ask");
	}

	#[test]
	fn ctrl_l_clears_output() {
		let mut app = App::new();
		app.output.push(OutputItem::Info {
			text: "test".into(),
		});
		app.banner_visible = false;
		app = update(app, AppMessage::CtrlL);
		assert!(app.output.is_empty());
		assert!(app.banner_visible);
	}

	#[test]
	fn scroll_bounds() {
		let mut app = App::new();
		app = update(app, AppMessage::ScrollUp(5));
		assert_eq!(app.scroll_offset, 5);
		app = update(app, AppMessage::ScrollToBottom);
		assert_eq!(app.scroll_offset, 0);
	}

	#[test]
	fn slash_help_adds_output() {
		let mut app = App::new();
		app.input = input::insert(&app.input, "/help");
		app = update(app, AppMessage::Submit);
		assert!(app
			.output
			.iter()
			.any(|o| matches!(o, OutputItem::CommandResult { .. })));
	}

	#[test]
	fn slash_clear_resets() {
		let mut app = App::new();
		app.output.push(OutputItem::Info {
			text: "test".into(),
		});
		app.banner_visible = false;
		app.input = input::insert(&app.input, "/clear");
		app = update(app, AppMessage::Submit);
		assert!(app.output.is_empty());
		assert!(app.banner_visible);
	}

	#[test]
	fn slash_exit_quits() {
		let mut app = App::new();
		app.input = input::insert(&app.input, "/exit");
		app = update(app, AppMessage::Submit);
		assert!(app.should_quit);
	}

	#[test]
	fn bare_exit_quits() {
		let mut app = App::new();
		app.input = input::insert(&app.input, "exit");
		app = update(app, AppMessage::Submit);
		assert!(app.should_quit);
	}

	#[test]
	fn slash_verbose_toggles() {
		let mut app = App::new();
		assert!(!app.verbose);
		app.input = input::insert(&app.input, "/verbose");
		app = update(app, AppMessage::Submit);
		assert!(app.verbose);
	}

	#[test]
	fn unknown_command_shows_error() {
		let mut app = App::new();
		app.input = input::insert(&app.input, "/nonexistent");
		app = update(app, AppMessage::Submit);
		assert!(app
			.output
			.iter()
			.any(|o| matches!(o, OutputItem::Error { .. })));
	}

	#[test]
	fn normal_message_adds_to_output() {
		let mut app = App::new();
		app.input = input::insert(&app.input, "hello world");
		app = update(app, AppMessage::Submit);
		assert!(app
			.output
			.iter()
			.any(|o| matches!(o, OutputItem::Message { .. })));
	}

	#[test]
	fn question_mark_empty_shows_shortcuts() {
		let mut app = App::new();
		app = update(app, AppMessage::CharInput('?'));
		assert_eq!(app.screen, Screen::Shortcuts);
		assert!(app.input.value.is_empty());
	}

	#[test]
	fn question_mark_nonempty_inserts() {
		let mut app = App::new();
		app.input = input::insert(&app.input, "hello");
		app = update(app, AppMessage::CharInput('?'));
		assert_eq!(app.screen, Screen::Chat);
		assert!(app.input.value.contains('?'));
	}

	#[test]
	fn format_tokens_small() {
		assert_eq!(format_tokens(42), "42");
		assert_eq!(format_tokens(999), "999");
	}

	#[test]
	fn format_tokens_thousands() {
		assert_eq!(format_tokens(1000), "1.0k");
		assert_eq!(format_tokens(1500), "1.5k");
		assert_eq!(format_tokens(42000), "42.0k");
	}

	#[test]
	fn tool_call_lifecycle() {
		let mut app = App::new();
		let tc = ToolCallState {
			id: "tc1".into(),
			name: "read_file".into(),
			args: "{}".into(),
			status: ToolCallStatus::Active,
			started_at: 1000,
			duration_ms: None,
			summary: None,
			error: None,
			diff: None,
		};
		app = update(app, AppMessage::ToolCallStart(tc));
		assert_eq!(app.active_tool_calls.len(), 1);
		assert_eq!(app.loop_status, LoopStatus::ToolExecuting);

		app = update(
			app,
			AppMessage::ToolCallEnd {
				id: "tc1".into(),
				status: ToolCallStatus::Completed,
				summary: Some("Read 42 lines".into()),
				error: None,
				duration_ms: Some(150),
				diff: None,
			},
		);
		assert!(app.active_tool_calls.is_empty());
		assert!(app
			.output
			.iter()
			.any(|o| matches!(o, OutputItem::ToolCall(..))));
	}

	#[test]
	fn loop_error_adds_error_output() {
		let mut app = App::new();
		app.loop_status = LoopStatus::Streaming;
		app = update(app, AppMessage::LoopError("Connection lost".into()));
		assert_eq!(app.loop_status, LoopStatus::Idle);
		assert!(app
			.output
			.iter()
			.any(|o| matches!(o, OutputItem::Error { .. })));
	}

	#[test]
	fn token_usage_accumulates() {
		let mut app = App::new();
		app = update(
			app,
			AppMessage::TokenUsage {
				prompt: 100,
				completion: 50,
			},
		);
		assert_eq!(app.total_tokens, 150);
		app = update(
			app,
			AppMessage::TokenUsage {
				prompt: 200,
				completion: 100,
			},
		);
		assert_eq!(app.total_tokens, 450);
	}

	#[test]
	fn history_deduplicates_consecutive() {
		let mut app = App::new();
		app.input = input::insert(&app.input, "hello");
		app = update(app, AppMessage::Submit);
		app.input = input::insert(&app.input, "hello");
		app = update(app, AppMessage::Submit);
		assert_eq!(app.history.len(), 1);
	}
}
