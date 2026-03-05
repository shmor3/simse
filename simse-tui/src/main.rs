//! SimSE TUI — Terminal interface for SimSE.

mod app;
pub mod at_mention;
pub mod autocomplete;
mod banner;
pub mod cli_args;
pub mod commands;
pub mod config;
pub mod dialogs;
pub mod dispatch;
pub mod error_box;
pub mod event_loop;
pub mod json_io;
pub mod levenshtein;
pub mod markdown;
mod output;
pub mod onboarding;
pub mod overlays;
pub mod session_store;
pub mod spinner;
pub mod status_bar;
pub mod tool_call_box;

use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use crossterm::{
	event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers},
	execute,
	terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::StreamExt;
use ratatui::{backend::CrosstermBackend, Terminal};
use tokio::sync::{mpsc, Mutex};

use app::{update, view, App, AppMessage};
use cli_args::parse_cli_args;

/// Create `LoopCallbacks` that forward agentic loop events to the TUI as
/// `AppMessage`s via an unbounded channel.
fn create_loop_callbacks(
	tx: mpsc::UnboundedSender<AppMessage>,
) -> simse_core::agentic_loop::LoopCallbacks {
	let tx1 = tx.clone();
	let tx2 = tx.clone();
	let tx3 = tx.clone();
	simse_core::agentic_loop::LoopCallbacks {
		on_stream_start: Some(Box::new(move || {
			let _ = tx1.send(AppMessage::StreamStart);
		})),
		on_stream_delta: Some(Box::new(move |delta: &str| {
			let _ = tx2.send(AppMessage::StreamDelta(delta.to_string()));
		})),
		on_error: Some(Box::new(move |error: &simse_core::SimseError| {
			let _ = tx3.send(AppMessage::LoopError(error.to_string()));
		})),
		..Default::default()
	}
}

#[tokio::main]
async fn main() -> io::Result<()> {
	enable_raw_mode()?;
	let mut stdout = io::stdout();
	execute!(stdout, EnterAlternateScreen)?;
	let backend = CrosstermBackend::new(stdout);
	let mut terminal = Terminal::new(backend)?;

	let result = run_app(&mut terminal).await;

	disable_raw_mode()?;
	execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
	terminal.show_cursor()?;

	result
}

async fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
	// Parse CLI arguments and load configuration.
	let args: Vec<String> = std::env::args().collect();
	let cli = parse_cli_args(&args);

	let config_options = crate::config::ConfigOptions {
		data_dir: cli.data_dir.map(PathBuf::from),
		work_dir: None,
		default_agent: cli.agent.clone(),
		log_level: None,
		server_name: cli.server.clone(),
	};
	let config = crate::config::load_config(&config_options);
	let mut rt = event_loop::TuiRuntime::new(config);
	rt.verbose = cli.verbose;
	let runtime = Arc::new(Mutex::new(rt));

	let mut app = App::new();
	let (msg_tx, mut msg_rx) = mpsc::unbounded_channel::<AppMessage>();
	let mut reader = EventStream::new();

	loop {
		terminal.draw(|frame| view(&app, frame))?;

		tokio::select! {
			Some(Ok(event)) = reader.next() => {
				if let Some(msg) = map_event(event) {
					// Schedule Ctrl+C timeout
					if matches!(msg, AppMessage::CtrlC) && !app.ctrl_c_pending {
						let tx = msg_tx.clone();
						tokio::spawn(async move {
							tokio::time::sleep(std::time::Duration::from_secs(2)).await;
							let _ = tx.send(AppMessage::CtrlCTimeout);
						});
					}
					app = update(app, msg);
				}
			}
			Some(msg) = msg_rx.recv() => {
				app = update(app, msg);
			}
		}

		// Dispatch pending bridge action (e.g. after confirming factory-reset).
		// Use try_lock to avoid blocking the UI while the agentic loop runs.
		if let Some(action) = app.pending_bridge_action.take() {
			if let Ok(mut rt) = runtime.try_lock() {
				let result_msg = rt.dispatch_bridge_action(action).await;
				app = update(app, result_msg);
			} else {
				// Runtime busy (agentic loop running), defer until next iteration.
				app.pending_bridge_action = Some(action);
			}
		}

		// Dispatch pending chat message through the agentic loop.
		if let Some(text) = app.pending_chat_message.take() {
			// Lazily connect to ACP on first chat message.
			let mut rt = runtime.lock().await;
			if !rt.is_connected() && !rt.needs_onboarding() {
				match rt.connect().await {
					Ok(()) => {
						let ctx = rt.build_command_context();
						app = update(app, AppMessage::RefreshContext(ctx));
					}
					Err(e) => {
						app = update(
							app,
							AppMessage::LoopError(format!("ACP connection failed: {e}")),
						);
						continue;
					}
				}
			}
			drop(rt);

			app = update(app, AppMessage::StreamStart);
			terminal.draw(|frame| view(&app, frame))?;

			// Spawn the agentic loop as a background task so the UI stays responsive.
			let rt = Arc::clone(&runtime);
			let tx = msg_tx.clone();
			tokio::spawn(async move {
				let callbacks = create_loop_callbacks(tx.clone());
				match rt.lock().await.handle_submit(&text, callbacks).await {
					Ok(final_text) => {
						let _ = tx.send(AppMessage::StreamEnd { text: final_text });
					}
					Err(e) => {
						let _ = tx.send(AppMessage::LoopError(e.to_string()));
					}
				}
			});
		}

		if app.should_quit {
			return Ok(());
		}
	}
}

fn map_event(event: Event) -> Option<AppMessage> {
	match event {
		Event::Key(key) if key.kind == KeyEventKind::Press => {
			match (key.code, key.modifiers) {
				(KeyCode::Char('c'), m) if m.contains(KeyModifiers::CONTROL) => {
					Some(AppMessage::CtrlC)
				}
				(KeyCode::Char('l'), m) if m.contains(KeyModifiers::CONTROL) => {
					Some(AppMessage::CtrlL)
				}
				(KeyCode::Char('a'), m) if m.contains(KeyModifiers::CONTROL) => {
					Some(AppMessage::SelectAll)
				}
				(KeyCode::Esc, _) => Some(AppMessage::Escape),
				(KeyCode::BackTab, _) => Some(AppMessage::ShiftTab),
				(KeyCode::Tab, _) => Some(AppMessage::Tab),
				(KeyCode::Enter, _) => Some(AppMessage::Submit),
				(KeyCode::Backspace, m) if m.contains(KeyModifiers::ALT) => {
					Some(AppMessage::DeleteWordBack)
				}
				(KeyCode::Backspace, _) => Some(AppMessage::Backspace),
				(KeyCode::Delete, _) => Some(AppMessage::Delete),
				(KeyCode::Left, m) if m.contains(KeyModifiers::SHIFT) => {
					Some(AppMessage::SelectLeft)
				}
				(KeyCode::Right, m) if m.contains(KeyModifiers::SHIFT) => {
					Some(AppMessage::SelectRight)
				}
				(KeyCode::Home, m) if m.contains(KeyModifiers::SHIFT) => {
					Some(AppMessage::SelectHome)
				}
				(KeyCode::End, m) if m.contains(KeyModifiers::SHIFT) => {
					Some(AppMessage::SelectEnd)
				}
				(KeyCode::Left, m)
					if m.contains(KeyModifiers::ALT) || m.contains(KeyModifiers::CONTROL) =>
				{
					Some(AppMessage::WordLeft)
				}
				(KeyCode::Right, m)
					if m.contains(KeyModifiers::ALT) || m.contains(KeyModifiers::CONTROL) =>
				{
					Some(AppMessage::WordRight)
				}
				(KeyCode::Left, _) => Some(AppMessage::CursorLeft),
				(KeyCode::Right, _) => Some(AppMessage::CursorRight),
				(KeyCode::Home, _) => Some(AppMessage::Home),
				(KeyCode::End, _) => Some(AppMessage::End),
				(KeyCode::Up, _) => Some(AppMessage::HistoryUp),
				(KeyCode::Down, _) => Some(AppMessage::HistoryDown),
				(KeyCode::PageUp, _) => Some(AppMessage::ScrollUp(10)),
				(KeyCode::PageDown, _) => Some(AppMessage::ScrollDown(10)),
				(KeyCode::Char(c), _) => Some(AppMessage::CharInput(c)),
				_ => None,
			}
		}
		Event::Resize(w, h) => Some(AppMessage::Resize {
			width: w,
			height: h,
		}),
		Event::Paste(text) => Some(AppMessage::Paste(text)),
		_ => None,
	}
}
