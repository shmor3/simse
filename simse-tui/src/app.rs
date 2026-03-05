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
use simse_ui_core::commands::registry::{all_commands, CommandCategory, CommandDefinition};
use simse_ui_core::input::state as input;
use std::collections::BTreeMap;

use crate::autocomplete::{render_inline_completions, CommandAutocompleteState};
use crate::onboarding::OnboardingState;

/// Maximum height (in rows) for the inline completions area.
const MAX_VISIBLE_COMPLETIONS: u16 = 8;
use crate::banner;
use crate::commands::{
	format_table, AgentInfo, BridgeAction, CommandContext, CommandOutput, OverlayAction,
	PromptInfo, SessionInfo, SkillInfo, ToolDefInfo,
};
use crate::dispatch::{parse_command_line, DispatchContext};
use crate::output;
use crate::overlays::librarian::{render_librarian_explorer, LibrarianExplorerState};
use crate::overlays::settings::{render_settings_explorer, SettingsExplorerState};
use crate::overlays::setup::{render_setup_selector, SetupSelectorState};

// ── Screen ──────────────────────────────────────────────

/// Which screen/overlay is currently active.
#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
	Chat,
	Shortcuts,
	Settings,
	Librarians,
	Setup { preset: Option<String> },
	Confirm { message: String },
	Permission(PermissionRequest),
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
	/// Command autocomplete state.
	pub autocomplete: CommandAutocompleteState,
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
	pub session_id: Option<String>,
	pub acp_connected: bool,
	pub data_dir: Option<String>,
	pub work_dir: Option<String>,
	pub sessions: Vec<SessionInfo>,
	pub tool_defs: Vec<ToolDefInfo>,
	pub agents: Vec<AgentInfo>,
	pub skills: Vec<SkillInfo>,
	pub prompts: Vec<PromptInfo>,
	pub config_values: Vec<(String, String)>,
	pub pending_bridge_action: Option<BridgeAction>,
	/// Pending action waiting for confirmation via Screen::Confirm.
	pub pending_confirm_action: Option<BridgeAction>,
	/// Overlay state for the settings explorer.
	pub settings_state: SettingsExplorerState,
	/// Overlay state for the settings explorer config data.
	pub settings_config_data: serde_json::Value,
	/// Overlay state for the librarian explorer.
	pub librarian_state: LibrarianExplorerState,
	/// Overlay state for the setup selector.
	pub setup_state: SetupSelectorState,
	/// Onboarding state — tracks whether first-run setup is needed.
	pub onboarding: OnboardingState,
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
			autocomplete: CommandAutocompleteState::new(),
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
			session_id: None,
			acp_connected: false,
			data_dir: None,
			work_dir: None,
			sessions: Vec::new(),
			tool_defs: Vec::new(),
			agents: Vec::new(),
			skills: Vec::new(),
			prompts: Vec::new(),
			config_values: Vec::new(),
			pending_bridge_action: None,
			pending_confirm_action: None,
			settings_state: SettingsExplorerState::new(),
			settings_config_data: serde_json::Value::Null,
			librarian_state: LibrarianExplorerState::new(Vec::new()),
			setup_state: SetupSelectorState::new(),
			onboarding: OnboardingState::default(),
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

	// Bridge
	BridgeResult {
		action: String,
		text: String,
		is_error: bool,
	},
	RefreshContext(CommandContext),

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
			match &app.screen {
				Screen::Settings => { app.settings_state.type_char(c); return app; }
				Screen::Librarians => { app.librarian_state.type_char(c); return app; }
				Screen::Setup { .. } => { app.setup_state.type_char(c); return app; }
				_ => {}
			}
			if app.screen == Screen::Shortcuts {
				app.screen = Screen::Chat;
				return app;
			}
			if c == '?' && app.input.value.is_empty() {
				app.screen = Screen::Shortcuts;
			} else {
				app.input = input::insert(&app.input, &c.to_string());
			}
			app.autocomplete.update_matches(&app.input.value, &app.commands);
		}
		AppMessage::Paste(text) => {
			app.input = input::insert(&app.input, &text);
			app.autocomplete.update_matches(&app.input.value, &app.commands);
		}
		AppMessage::Submit => {
			match &app.screen {
				Screen::Settings => {
					let current_value = get_settings_current_value(&app);
					app.settings_state.enter(&current_value);
					return app;
				}
				Screen::Librarians => { app.librarian_state.enter(); return app; }
				Screen::Setup { .. } => {
					let action = app.setup_state.enter();
					handle_setup_action(&mut app, action);
					return app;
				}
				Screen::Confirm { .. } => {
					if let Some(action) = app.pending_confirm_action.take() {
						app.pending_bridge_action = Some(action);
					}
					app.screen = Screen::Chat;
					return app;
				}
				_ => {}
			}
			app.autocomplete.deactivate();
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
			match &app.screen {
				Screen::Settings => { app.settings_state.backspace(); return app; }
				Screen::Librarians => { app.librarian_state.backspace(); return app; }
				Screen::Setup { .. } => { app.setup_state.backspace(); return app; }
				_ => {}
			}
			app.input = input::backspace(&app.input);
			app.autocomplete.update_matches(&app.input.value, &app.commands);
		}
		AppMessage::Delete => {
			app.input = input::delete(&app.input);
			app.autocomplete.update_matches(&app.input.value, &app.commands);
		}
		AppMessage::DeleteWordBack => {
			app.input = input::delete_word_back(&app.input);
			app.autocomplete.update_matches(&app.input.value, &app.commands);
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
			match &app.screen {
				Screen::Settings => { app.settings_state.move_up(); return app; }
				Screen::Librarians => { app.librarian_state.move_up(); return app; }
				Screen::Setup { .. } => { app.setup_state.move_up(); return app; }
				_ => {}
			}
			if app.autocomplete.is_active() {
				app.autocomplete.move_up();
				return app;
			}
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
			match &app.screen {
				Screen::Settings => {
					let count = match app.settings_state.level {
						crate::overlays::settings::SettingsLevel::FileList => {
							crate::overlays::settings::CONFIG_FILES.len()
						}
						_ => {
							if let Some(obj) = app.settings_config_data.as_object() {
								obj.len()
							} else {
								0
							}
						}
					};
					app.settings_state.move_down(count);
					return app;
				}
				Screen::Librarians => { app.librarian_state.move_down(); return app; }
				Screen::Setup { .. } => { app.setup_state.move_down(); return app; }
				_ => {}
			}
			if app.autocomplete.is_active() {
				app.autocomplete.move_down();
				return app;
			}
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
			if app.autocomplete.is_active() {
				app.autocomplete.deactivate();
			} else {
				match &app.screen {
					Screen::Settings => {
						if app.settings_state.back() {
							app.screen = Screen::Chat;
						}
					}
					Screen::Librarians => {
						if app.librarian_state.back() {
							app.screen = Screen::Chat;
						}
					}
					Screen::Setup { .. } => {
						if app.setup_state.back() {
							app.screen = Screen::Chat;
						}
					}
					Screen::Confirm { .. } => {
						app.pending_confirm_action = None;
						app.screen = Screen::Chat;
						app.output.push(OutputItem::Info {
							text: "Cancelled.".into(),
						});
					}
					Screen::Chat => {
						if app.loop_status != LoopStatus::Idle {
							app.loop_status = LoopStatus::Idle;
							app.output.push(OutputItem::Info {
								text: "Interrupted.".into(),
							});
						}
					}
					_ => {
						app.screen = Screen::Chat;
					}
				}
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
			match &app.screen {
				Screen::Setup { .. } => { app.setup_state.toggle_field(); return app; }
				_ => {}
			}
			if app.autocomplete.is_active() {
				if let Some(completed) = app.autocomplete.accept() {
					let with_space = format!("{completed} ");
					app.input = input::InputState {
						value: with_space.clone(),
						cursor: with_space.len(),
						..Default::default()
					};
				}
			} else if app.input.value.starts_with('/') {
				app.autocomplete.update_matches(&app.input.value, &app.commands);
			}
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

		// ── Bridge ──────────────────────────────────
		AppMessage::BridgeResult { action, text, is_error } => {
			if is_error {
				app.output.push(OutputItem::Error { message: text });
			} else {
				app.output.push(OutputItem::CommandResult { text });
			}
			// After factory-reset, restart onboarding
			if action == "factory-reset" && !is_error {
				app.onboarding = OnboardingState { needs_setup: true, welcome_shown: false };
				app.server_name = None;
				app.model_name = None;
				app.acp_connected = false;
				app.config_values.clear();
			}
		}
		AppMessage::RefreshContext(ctx) => {
			app.sessions = ctx.sessions;
			app.tool_defs = ctx.tool_defs;
			app.agents = ctx.agents;
			app.skills = ctx.skills;
			app.prompts = ctx.prompts;
			app.server_name = ctx.server_name;
			app.model_name = ctx.model_name;
			app.session_id = ctx.session_id;
			app.acp_connected = ctx.acp_connected;
			app.data_dir = ctx.data_dir;
			app.work_dir = ctx.work_dir;
			app.config_values = ctx.config_values;
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
	let Some((command, args)) = parse_command_line(text) else {
		app.output.push(OutputItem::Error {
			message: "Invalid command.".into(),
		});
		return app;
	};

	// Build CommandContext from current app state
	let cmd_ctx = CommandContext {
		sessions: app.sessions.clone(),
		tool_defs: app.tool_defs.clone(),
		agents: app.agents.clone(),
		skills: app.skills.clone(),
		prompts: app.prompts.clone(),
		server_name: app.server_name.clone(),
		model_name: app.model_name.clone(),
		session_id: app.session_id.clone(),
		acp_connected: app.acp_connected,
		data_dir: app.data_dir.clone(),
		work_dir: app.work_dir.clone(),
		config_values: app.config_values.clone(),
	};

	let ctx = DispatchContext {
		verbose: app.verbose,
		plan: app.plan_mode,
		total_tokens: app.total_tokens,
		context_percent: app.context_percent,
		commands: app.commands.clone(),
		cmd_ctx,
	};

	let results = ctx.dispatch(&command, &args);

	// Apply side effects for commands that mutate app state
	match command.as_str() {
		"clear" => {
			app.output.clear();
			app.banner_visible = true;
			return app;
		}
		"exit" | "quit" | "q" => {
			app.should_quit = true;
			return app;
		}
		"verbose" | "v" => {
			// Toggle verbose: the handler was passed current state,
			// so the success message tells us the new state.
			for r in &results {
				if let CommandOutput::Success(msg) = r {
					app.verbose = msg.contains(" on");
				}
			}
		}
		"plan" => {
			for r in &results {
				if let CommandOutput::Success(msg) = r {
					app.plan_mode = msg.contains(" on");
				}
			}
		}
		_ => {}
	}

	// Convert CommandOutput items into App output
	for result in results {
		match result {
			CommandOutput::Success(text) => {
				app.output.push(OutputItem::CommandResult { text });
			}
			CommandOutput::Error(message) => {
				app.output.push(OutputItem::Error { message });
			}
			CommandOutput::Info(text) => {
				// Filter out sentinel values (clear/exit already handled above)
				if text == "__clear__" || text == "__exit__" {
					continue;
				}
				app.output.push(OutputItem::Info { text });
			}
			CommandOutput::Table { headers, rows } => {
				let text = format_table(&headers, &rows);
				app.output.push(OutputItem::CommandResult { text });
			}
			CommandOutput::BridgeRequest(action) => {
				// Store pending action for the event loop to pick up and execute.
				app.pending_bridge_action = Some(action);
			}
			CommandOutput::ConfirmAction { message, action } => {
				app.pending_confirm_action = Some(action);
				app.screen = Screen::Confirm { message };
			}
			CommandOutput::OpenOverlay(action) => match action {
				OverlayAction::Settings => {
					app.settings_state = SettingsExplorerState::new();
					app.screen = Screen::Settings;
				}
				OverlayAction::Shortcuts => {
					app.screen = Screen::Shortcuts;
				}
				OverlayAction::Librarians => {
					app.librarian_state =
						LibrarianExplorerState::new(app.librarian_state.librarians.clone());
					app.screen = Screen::Librarians;
				}
				OverlayAction::Setup(preset) => {
					app.setup_state = SetupSelectorState::new();
					app.screen = Screen::Setup { preset };
				}
			},
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

	let completions_height = if app.autocomplete.is_active() {
		let total = app.autocomplete.matches.len() as u16;
		total.min(MAX_VISIBLE_COMPLETIONS)
	} else {
		0
	};

	let chunks = Layout::default()
		.direction(Direction::Vertical)
		.constraints(if completions_height > 0 {
			vec![
				Constraint::Min(1),
				Constraint::Length(3),
				Constraint::Length(completions_height),
				Constraint::Length(1),
			]
		} else {
			vec![
				Constraint::Min(1),
				Constraint::Length(3),
				Constraint::Length(0),
				Constraint::Length(1),
			]
		})
		.split(area);

	// 1. Chat area
	render_chat_area(app, frame, chunks[0]);

	// 2. Input
	render_input(app, frame, chunks[1]);

	// 3. Completions (inline, below input)
	if completions_height > 0 {
		let lines = render_inline_completions(&app.autocomplete, chunks[2].width);
		let completions = Paragraph::new(lines);
		frame.render_widget(completions, chunks[2]);
	}

	// 4. Status bar
	let status = render_status_line(app, chunks[3].width);
	frame.render_widget(Paragraph::new(status), chunks[3]);

	// 5. Overlay screens (rendered on top of everything)
	match &app.screen {
		Screen::Shortcuts => render_shortcuts_overlay(frame, area),
		Screen::Settings => {
			render_settings_explorer(frame, area, &app.settings_state, &app.settings_config_data);
		}
		Screen::Librarians => {
			render_librarian_explorer(frame, area, &app.librarian_state);
		}
		Screen::Setup { .. } => {
			render_setup_selector(frame, area, &app.setup_state);
		}
		Screen::Confirm { message } => {
			render_confirm_overlay(frame, area, message);
		}
		_ => {}
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

	// Calculate scroll: offset 0 = pinned to bottom (latest content visible).
	// scroll_offset increases as user scrolls up (away from bottom).
	let visible_height = area.height as usize;
	let total_lines = lines.len();
	let max_scroll = total_lines.saturating_sub(visible_height);
	let clamped_offset = app.scroll_offset.min(max_scroll);
	let scroll = max_scroll.saturating_sub(clamped_offset) as u16;

	let chat = Paragraph::new(lines)
		.wrap(Wrap { trim: false })
		.scroll((scroll, 0));
	frame.render_widget(chat, area);
}

/// Render the input area.
fn render_input(app: &App, frame: &mut Frame, area: Rect) {
	let ghost = app.autocomplete.ghost_text();

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
	} else if let Some(ref ghost_str) = ghost {
		Line::from(vec![
			Span::raw(app.input.value.clone()),
			Span::styled(ghost_str.clone(), Style::default().fg(Color::DarkGray)),
		])
	} else {
		Line::from(app.input.value.as_str())
	};

	let input_widget = Paragraph::new(input_display)
		.block(Block::default().borders(Borders::ALL).title("Input"));
	frame.render_widget(input_widget, area);

	// Hide cursor when overlay is active
	if app.screen == Screen::Chat {
		let cursor_x = area.x.saturating_add(1).saturating_add(
			(app.input.cursor as u16).min(area.width.saturating_sub(2)),
		);
		let cursor_y = area.y + 1;
		frame.set_cursor_position((cursor_x, cursor_y));
	}
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

/// Render a centered confirmation dialog overlay.
fn render_confirm_overlay(frame: &mut Frame, area: Rect, message: &str) {
	let popup_width = 50u16.min(area.width.saturating_sub(4));
	let popup_height = 8u16.min(area.height.saturating_sub(4));
	let popup_x = (area.width.saturating_sub(popup_width)) / 2;
	let popup_y = (area.height.saturating_sub(popup_height)) / 2;

	let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

	let dim = Style::default().fg(Color::DarkGray);
	let cyan = Style::default().fg(Color::Cyan);

	let lines = vec![
		Line::from(""),
		Line::from(Span::styled(
			format!("  {message}"),
			Style::default().fg(Color::White),
		)),
		Line::from(""),
		Line::from(""),
		Line::from(vec![
			Span::styled("  [Enter] ", cyan),
			Span::styled("Confirm  ", dim),
			Span::styled("[Esc] ", cyan),
			Span::styled("Cancel", dim),
		]),
	];

	frame.render_widget(Clear, popup_area);
	let popup = Paragraph::new(lines).block(
		Block::default()
			.borders(Borders::ALL)
			.border_style(Style::default().fg(Color::Yellow))
			.title(" Confirm "),
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

	let gap = (width as usize).saturating_sub(left.chars().count() + right.chars().count() + 2);
	let full = format!(" {left}{}{right} ", " ".repeat(gap));

	Line::from(Span::styled(
		full,
		Style::default().fg(Color::DarkGray),
	))
}

/// Get the current value string for the selected settings field.
fn get_settings_current_value(app: &App) -> String {
	if let Some(obj) = app.settings_config_data.as_object() {
		let keys: Vec<&String> = obj.keys().collect();
		if let Some(key) = keys.get(app.settings_state.selected_field) {
			if let Some(val) = obj.get(*key) {
				return match val {
					serde_json::Value::String(s) => s.clone(),
					other => other.to_string(),
				};
			}
		}
	}
	String::new()
}

/// Handle a SetupAction returned by the setup selector.
fn handle_setup_action(app: &mut App, action: crate::overlays::setup::SetupAction) {
	use crate::overlays::setup::SetupAction;
	match action {
		SetupAction::SelectPreset(preset) => {
			app.output.push(OutputItem::CommandResult {
				text: format!("Selected preset: {}", preset.label()),
			});
			app.screen = Screen::Chat;
		}
		SetupAction::OpenOllamaWizard => {
			app.output.push(OutputItem::Info {
				text: "Opening Ollama wizard...".into(),
			});
		}
		SetupAction::EnterCustomEdit => {
			// Stay in Setup screen, now in custom edit mode
		}
		SetupAction::ConfirmCustom { command, args } => {
			app.output.push(OutputItem::CommandResult {
				text: format!("Custom ACP: {command} {args}"),
			});
			app.screen = Screen::Chat;
		}
		SetupAction::None => {}
	}
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
	fn any_key_dismisses_shortcuts() {
		let mut app = App::new();
		app.screen = Screen::Shortcuts;
		app = update(app, AppMessage::CharInput('a'));
		assert_eq!(app.screen, Screen::Chat);
		assert!(app.input.value.is_empty()); // key should NOT be inserted
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

	// ── Overlay screen transition tests ─────────────

	#[test]
	fn librarians_overlay_opens_via_command() {
		let mut app = App::new();
		app.input = input::insert(&app.input, "/librarians");
		app = update(app, AppMessage::Submit);
		assert_eq!(app.screen, Screen::Librarians);
	}

	#[test]
	fn setup_overlay_opens_via_command() {
		let mut app = App::new();
		app.input = input::insert(&app.input, "/setup");
		app = update(app, AppMessage::Submit);
		assert!(matches!(app.screen, Screen::Setup { preset: None }));
	}

	#[test]
	fn escape_dismisses_librarians_overlay() {
		let mut app = App::new();
		app.screen = Screen::Librarians;
		app = update(app, AppMessage::Escape);
		assert_eq!(app.screen, Screen::Chat);
	}

	#[test]
	fn escape_dismisses_setup_overlay() {
		let mut app = App::new();
		app.screen = Screen::Setup { preset: None };
		app = update(app, AppMessage::Escape);
		assert_eq!(app.screen, Screen::Chat);
	}

	#[test]
	fn escape_dismisses_settings_overlay() {
		let mut app = App::new();
		app.screen = Screen::Settings;
		app = update(app, AppMessage::Escape);
		assert_eq!(app.screen, Screen::Chat);
	}

	#[test]
	fn setup_overlay_with_preset() {
		let mut app = App::new();
		// Directly set through the overlay action path
		app.setup_state = SetupSelectorState::new();
		app.screen = Screen::Setup {
			preset: Some("ollama".into()),
		};
		assert!(matches!(
			app.screen,
			Screen::Setup {
				preset: Some(ref p)
			} if p == "ollama"
		));
		// Escape dismisses it
		app = update(app, AppMessage::Escape);
		assert_eq!(app.screen, Screen::Chat);
	}

	#[test]
	fn dismiss_overlay_message_returns_to_chat_from_librarians() {
		let mut app = App::new();
		app.screen = Screen::Librarians;
		app = update(app, AppMessage::DismissOverlay);
		assert_eq!(app.screen, Screen::Chat);
	}

	#[test]
	fn dismiss_overlay_message_returns_to_chat_from_setup() {
		let mut app = App::new();
		app.screen = Screen::Setup { preset: None };
		app = update(app, AppMessage::DismissOverlay);
		assert_eq!(app.screen, Screen::Chat);
	}

	#[test]
	fn new_app_has_default_overlay_states() {
		let app = App::new();
		assert!(app.librarian_state.librarians.is_empty());
		assert_eq!(app.setup_state.selected, 0);
		assert_eq!(
			app.settings_state.level,
			crate::overlays::settings::SettingsLevel::FileList
		);
		assert_eq!(app.settings_config_data, serde_json::Value::Null);
	}

	#[test]
	fn librarians_overlay_resets_state_on_open() {
		let mut app = App::new();
		// Simulate having some librarian data
		use crate::overlays::librarian::LibrarianEntry;
		app.librarian_state = LibrarianExplorerState::new(vec![LibrarianEntry::default_new()]);
		app.librarian_state.selected = 1; // non-zero selection
		// Open the overlay via command dispatch
		app.input = input::insert(&app.input, "/librarians");
		app = update(app, AppMessage::Submit);
		assert_eq!(app.screen, Screen::Librarians);
		// State should be reset: selection back to 0, but librarians preserved
		assert_eq!(app.librarian_state.selected, 0);
		assert_eq!(app.librarian_state.librarians.len(), 1);
	}

	#[test]
	fn setup_overlay_resets_state_on_open() {
		let mut app = App::new();
		app.setup_state.selected = 3;
		app.setup_state.editing_custom = true;
		// Open the overlay via command dispatch
		app.input = input::insert(&app.input, "/setup");
		app = update(app, AppMessage::Submit);
		assert!(matches!(app.screen, Screen::Setup { preset: None }));
		// State should be freshly initialized
		assert_eq!(app.setup_state.selected, 0);
		assert!(!app.setup_state.editing_custom);
	}

	// ── Render smoke tests for new overlay screens ──

	#[test]
	fn render_librarians_overlay_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut app = App::new();
		app.screen = Screen::Librarians;

		terminal
			.draw(|frame| {
				view(&app, frame);
			})
			.unwrap();
	}

	#[test]
	fn render_setup_overlay_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut app = App::new();
		app.screen = Screen::Setup { preset: None };

		terminal
			.draw(|frame| {
				view(&app, frame);
			})
			.unwrap();
	}

	#[test]
	fn render_settings_overlay_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut app = App::new();
		app.screen = Screen::Settings;

		terminal
			.draw(|frame| {
				view(&app, frame);
			})
			.unwrap();
	}

	#[test]
	fn render_librarians_with_entries_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut app = App::new();
		use crate::overlays::librarian::LibrarianEntry;
		app.librarian_state = LibrarianExplorerState::new(vec![
			LibrarianEntry {
				name: "my-lib".into(),
				description: "General purpose".into(),
				permissions: vec!["add".into(), "delete".into()],
				topics: vec!["**".into()],
			},
			LibrarianEntry {
				name: "code-reviewer".into(),
				description: "Reviews code changes".into(),
				permissions: vec!["add".into()],
				topics: vec!["rust".into(), "web/**".into()],
			},
		]);
		app.screen = Screen::Librarians;

		terminal
			.draw(|frame| {
				view(&app, frame);
			})
			.unwrap();
	}

	#[test]
	fn render_settings_with_config_data_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut app = App::new();
		app.settings_config_data = serde_json::json!({
			"host": "localhost",
			"port": 8080,
			"debug": true
		});
		app.screen = Screen::Settings;

		terminal
			.draw(|frame| {
				view(&app, frame);
			})
			.unwrap();
	}

	#[test]
	fn app_has_autocomplete_state() {
		let app = App::new();
		assert!(!app.autocomplete.is_active());
	}

	// ── Autocomplete integration tests ─────────────

	#[test]
	fn typing_slash_activates_autocomplete() {
		let mut app = App::new();
		app = update(app, AppMessage::CharInput('/'));
		app = update(app, AppMessage::CharInput('h'));
		assert!(app.autocomplete.is_active());
		assert!(app.autocomplete.matches.iter().any(|m| m.name == "help"));
	}

	#[test]
	fn tab_accepts_autocomplete_selection() {
		let mut app = App::new();
		app = update(app, AppMessage::CharInput('/'));
		app = update(app, AppMessage::CharInput('h'));
		app = update(app, AppMessage::CharInput('e'));
		app = update(app, AppMessage::CharInput('l'));
		assert!(app.autocomplete.is_active());
		app = update(app, AppMessage::Tab);
		assert!(!app.autocomplete.is_active());
		assert!(app.input.value.starts_with("/help"));
	}

	#[test]
	fn escape_deactivates_autocomplete() {
		let mut app = App::new();
		app = update(app, AppMessage::CharInput('/'));
		app = update(app, AppMessage::CharInput('h'));
		assert!(app.autocomplete.is_active());
		app = update(app, AppMessage::Escape);
		assert!(!app.autocomplete.is_active());
	}

	#[test]
	fn up_down_navigate_autocomplete() {
		let mut app = App::new();
		app = update(app, AppMessage::CharInput('/'));
		assert!(app.autocomplete.is_active());
		let initial_selected = app.autocomplete.selected;
		app = update(app, AppMessage::HistoryDown);
		assert_eq!(app.autocomplete.selected, initial_selected + 1);
		app = update(app, AppMessage::HistoryUp);
		assert_eq!(app.autocomplete.selected, initial_selected);
	}

	#[test]
	fn backspace_updates_autocomplete() {
		let mut app = App::new();
		app = update(app, AppMessage::CharInput('/'));
		app = update(app, AppMessage::CharInput('h'));
		app = update(app, AppMessage::CharInput('e'));
		let count_he = app.autocomplete.matches.len();
		app = update(app, AppMessage::Backspace);
		assert!(app.autocomplete.matches.len() >= count_he);
	}

	#[test]
	fn submit_deactivates_autocomplete() {
		let mut app = App::new();
		app = update(app, AppMessage::CharInput('/'));
		app = update(app, AppMessage::CharInput('h'));
		assert!(app.autocomplete.is_active());
		app = update(app, AppMessage::Submit);
		assert!(!app.autocomplete.is_active());
	}

	// ── Overlay focus routing tests ─────────────────

	#[test]
	fn settings_overlay_captures_arrow_keys() {
		let mut app = App::new();
		app.screen = Screen::Settings;
		let initial = app.settings_state.selected_file;
		app = update(app, AppMessage::HistoryDown);
		assert_ne!(app.settings_state.selected_file, initial);
	}

	#[test]
	fn settings_overlay_escape_dismisses() {
		let mut app = App::new();
		app.screen = Screen::Settings;
		app = update(app, AppMessage::Escape);
		assert_eq!(app.screen, Screen::Chat);
	}

	#[test]
	fn settings_overlay_enter_goes_to_field_list() {
		use crate::overlays::settings::SettingsLevel;
		let mut app = App::new();
		app.screen = Screen::Settings;
		app = update(app, AppMessage::Submit);
		assert_eq!(app.settings_state.level, SettingsLevel::FieldList);
	}

	#[test]
	fn settings_overlay_char_does_not_reach_input() {
		let mut app = App::new();
		app.screen = Screen::Settings;
		let original = app.input.value.clone();
		app = update(app, AppMessage::CharInput('a'));
		assert_eq!(app.input.value, original);
	}

	#[test]
	fn librarian_overlay_captures_navigation() {
		let mut app = App::new();
		app.screen = Screen::Librarians;
		// No entries by default, but move_down/move_up should not crash
		app = update(app, AppMessage::HistoryDown);
		app = update(app, AppMessage::HistoryUp);
		// Should still be on Librarians screen
		assert_eq!(app.screen, Screen::Librarians);
	}

	#[test]
	fn librarian_overlay_escape_dismisses() {
		let mut app = App::new();
		app.screen = Screen::Librarians;
		app = update(app, AppMessage::Escape);
		assert_eq!(app.screen, Screen::Chat);
	}

	#[test]
	fn setup_overlay_captures_navigation() {
		let mut app = App::new();
		app.screen = Screen::Setup { preset: None };
		app = update(app, AppMessage::HistoryDown);
		assert_eq!(app.setup_state.selected, 1);
	}

	#[test]
	fn setup_overlay_escape_dismisses() {
		let mut app = App::new();
		app.screen = Screen::Setup { preset: None };
		app = update(app, AppMessage::Escape);
		assert_eq!(app.screen, Screen::Chat);
	}

	#[test]
	fn setup_overlay_enter_selects_preset() {
		let mut app = App::new();
		app.screen = Screen::Setup { preset: None };
		// First entry is "Claude Code"
		app = update(app, AppMessage::Submit);
		// Should transition back to Chat with a result message
		assert_eq!(app.screen, Screen::Chat);
	}

	// ── Confirm screen tests ──────────────────────

	#[test]
	fn confirm_submit_executes_pending_action() {
		let mut app = App::new();
		app.pending_confirm_action = Some(BridgeAction::FactoryReset);
		app.screen = Screen::Confirm {
			message: "Are you sure?".into(),
		};
		app = update(app, AppMessage::Submit);
		assert_eq!(app.screen, Screen::Chat);
		assert_eq!(app.pending_bridge_action, Some(BridgeAction::FactoryReset));
		assert!(app.pending_confirm_action.is_none());
	}

	#[test]
	fn confirm_escape_cancels() {
		let mut app = App::new();
		app.pending_confirm_action = Some(BridgeAction::FactoryReset);
		app.screen = Screen::Confirm {
			message: "Are you sure?".into(),
		};
		app = update(app, AppMessage::Escape);
		assert_eq!(app.screen, Screen::Chat);
		assert!(app.pending_bridge_action.is_none());
		assert!(app.pending_confirm_action.is_none());
		assert!(app
			.output
			.iter()
			.any(|o| matches!(o, OutputItem::Info { text } if text == "Cancelled.")));
	}

	#[test]
	fn render_confirm_overlay_does_not_panic() {
		let backend = ratatui::backend::TestBackend::new(80, 24);
		let mut terminal = ratatui::Terminal::new(backend).unwrap();
		let mut app = App::new();
		app.screen = Screen::Confirm {
			message: "Delete everything?".into(),
		};

		terminal
			.draw(|frame| {
				view(&app, frame);
			})
			.unwrap();
	}

	#[test]
	fn setup_overlay_tab_toggles_field() {
		let mut app = App::new();
		app.screen = Screen::Setup { preset: None };
		// Navigate to Custom (index 3)
		app = update(app, AppMessage::HistoryDown);
		app = update(app, AppMessage::HistoryDown);
		app = update(app, AppMessage::HistoryDown);
		// Enter Custom -> enters custom edit mode
		app = update(app, AppMessage::Submit);
		assert!(app.setup_state.editing_custom);
		// Tab should toggle field
		let field_before = app.setup_state.editing_field;
		app = update(app, AppMessage::Tab);
		assert_ne!(app.setup_state.editing_field, field_before);
	}

	// ── Factory-reset onboarding restart tests ────

	#[test]
	fn factory_reset_success_restarts_onboarding() {
		let mut app = App::new();
		app.onboarding = OnboardingState { needs_setup: false, welcome_shown: true };
		app.server_name = Some("test-server".into());
		app.model_name = Some("llama3".into());
		app.acp_connected = true;
		app.config_values = vec![("key".into(), "val".into())];

		app = update(app, AppMessage::BridgeResult {
			action: "factory-reset".into(),
			text: "Factory reset complete.".into(),
			is_error: false,
		});

		// Onboarding should be restarted
		assert!(app.onboarding.needs_setup);
		assert!(!app.onboarding.welcome_shown);
		// Connection state should be cleared
		assert!(app.server_name.is_none());
		assert!(app.model_name.is_none());
		assert!(!app.acp_connected);
		assert!(app.config_values.is_empty());
		// Success message should be in output
		assert!(app.output.iter().any(|o| matches!(o, OutputItem::CommandResult { text } if text.contains("Factory reset"))));
	}

	#[test]
	fn factory_reset_error_does_not_restart_onboarding() {
		let mut app = App::new();
		app.onboarding = OnboardingState { needs_setup: false, welcome_shown: true };
		app.server_name = Some("test-server".into());
		app.model_name = Some("llama3".into());
		app.acp_connected = true;

		app = update(app, AppMessage::BridgeResult {
			action: "factory-reset".into(),
			text: "Permission denied".into(),
			is_error: true,
		});

		// Onboarding should NOT be restarted on error
		assert!(!app.onboarding.needs_setup);
		assert!(app.onboarding.welcome_shown);
		// Connection state should be preserved
		assert!(app.server_name.is_some());
		assert!(app.model_name.is_some());
		assert!(app.acp_connected);
		// Error should be in output
		assert!(app.output.iter().any(|o| matches!(o, OutputItem::Error { .. })));
	}

	#[test]
	fn non_factory_reset_action_does_not_restart_onboarding() {
		let mut app = App::new();
		app.onboarding = OnboardingState { needs_setup: false, welcome_shown: true };
		app.server_name = Some("test-server".into());

		app = update(app, AppMessage::BridgeResult {
			action: "init-config".into(),
			text: "Initialized project config.".into(),
			is_error: false,
		});

		// Onboarding should NOT be restarted for non-factory-reset actions
		assert!(!app.onboarding.needs_setup);
		assert!(app.onboarding.welcome_shown);
		assert!(app.server_name.is_some());
	}
}
