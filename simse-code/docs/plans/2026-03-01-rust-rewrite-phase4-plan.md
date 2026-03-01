# SimSE Rust Rewrite — Phase 4: TUI Shell

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build the TUI shell layout with scrollable message area, status bar, banner, proper Ctrl+C double-press exit, async event loop with tokio, and layered app state machine. This phase transforms the barebones scaffold into a functional terminal UI.

**Architecture:** Elm Architecture (TEA) with `App` model, `AppMessage` enum, pure `update()` function, and `view()` rendering. Async event loop using tokio for crossterm events + bridge messages. All rendering via ratatui widgets.

**Tech Stack:** Rust, ratatui, crossterm, tokio, simse-ui-core, simse-bridge

---

## Task 18: Expand App model with full state machine

**Files:**
- Modify: `simse-tui/src/app.rs`

**Step 1: Write the full App model and message types**

Replace the current minimal App/AppMessage with the full state machine:

```rust
use simse_ui_core::app::{OutputItem, ToolCallState, ToolCallStatus, PermissionRequest};
use simse_ui_core::commands::registry::{CommandDefinition, all_commands};
use simse_ui_core::input::state as input;

/// Which screen/modal is active.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    Chat,
    Shortcuts,
    Settings,
    Confirm { message: String },
    Permission(PermissionRequest),
}

/// Prompt input mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptMode {
    Normal,
    Autocomplete { selected: usize, matches: Vec<String> },
}

/// Processing status of the agentic loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoopStatus {
    Idle,
    Streaming,
    ToolExecuting,
}

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
```

**Step 2: Write the full AppMessage enum**

```rust
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
    StreamEnd { text: String },
    ToolCallStart(ToolCallState),
    ToolCallEnd { id: String, status: ToolCallStatus, summary: Option<String>, error: Option<String>, duration_ms: Option<u64>, diff: Option<String> },
    TokenUsage { prompt: u64, completion: u64 },
    LoopComplete,
    LoopError(String),

    // Permission
    PermissionRequest(PermissionRequest),
    PermissionResponse { id: String, option_id: String },

    // Resize
    Resize { width: u16, height: u16 },
}
```

**Step 3: Write the update function**

Implement the pure `update(app, msg) -> App` function handling all message variants. Key behaviors:

- `CtrlC`: If `ctrl_c_pending`, set `should_quit = true`. Otherwise set `ctrl_c_pending = true`.
- `CtrlCTimeout`: Reset `ctrl_c_pending = false`.
- `Submit`: Push input to history, clear input, add `OutputItem::Message` to output, clear `banner_visible`.
- `Escape`: If `loop_status != Idle`, set `loop_status = Idle` and add info "Interrupted.". If screen is overlay, dismiss it.
- `HistoryUp/Down`: Navigate history with draft preservation.
- `StreamDelta`: Append to `stream_text`.
- `StreamEnd`: Push `OutputItem::Message { role: "assistant", text }` to output, clear `stream_text`.
- `ToolCallStart`: Add to `active_tool_calls`.
- `ToolCallEnd`: Update matching tool call, move to output if completed/failed.
- `ShowShortcuts`: Set `screen = Screen::Shortcuts`.
- `ScrollUp/Down`: Adjust `scroll_offset` with bounds checking.
- `ShiftTab`: Cycle permission mode (ask → auto → bypass → ask).
- `CtrlL`: Clear output, reset to banner.

**Step 4: Write tests**

```rust
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
        app = update(app, AppMessage::StreamEnd { text: "full response".into() });
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
        app.output.push(OutputItem::Info { text: "test".into() });
        app.banner_visible = false;
        app = update(app, AppMessage::CtrlL);
        assert!(app.output.is_empty());
        assert!(app.banner_visible);
    }

    #[test]
    fn scroll_bounds() {
        let mut app = App::new();
        // Can't scroll up when at top
        app = update(app, AppMessage::ScrollUp(5));
        assert_eq!(app.scroll_offset, 0);
        // Add enough output to scroll
        for i in 0..50 {
            app.output.push(OutputItem::Info { text: format!("line {i}") });
        }
        app = update(app, AppMessage::ScrollDown(10));
        assert_eq!(app.scroll_offset, 10);
        app = update(app, AppMessage::ScrollToBottom);
        assert_eq!(app.scroll_offset, 0);
    }
}
```

**Step 5: Run tests, commit**

Run: `cargo test -p simse-tui`
Commit: `feat: expand TUI app model with full state machine and messages`

---

## Task 19: Async event loop with tokio

**Files:**
- Modify: `simse-tui/src/main.rs`

**Step 1: Convert to tokio async runtime**

Replace the synchronous event loop with a tokio-based async loop that:

1. Uses `crossterm::event::EventStream` for non-blocking terminal events
2. Spawns a Ctrl+C timeout task (2-second timer)
3. Maps crossterm `Event::Key` to `AppMessage` with full modifier support
4. Maps `Event::Resize` to `AppMessage::Resize`
5. Uses `tokio::select!` to handle terminal events + future bridge messages

```rust
use tokio::sync::mpsc;
use crossterm::event::{EventStream, KeyEvent, KeyCode, KeyModifiers, KeyEventKind};
use futures::StreamExt;

#[tokio::main]
async fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

async fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let mut app = App::new();
    let (msg_tx, mut msg_rx) = mpsc::unbounded_channel::<AppMessage>();
    let mut reader = EventStream::new();

    loop {
        terminal.draw(|frame| view(&app, frame))?;

        tokio::select! {
            Some(Ok(event)) = reader.next() => {
                if let Some(msg) = map_event(event) {
                    // Handle Ctrl+C timeout scheduling
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

        if app.should_quit {
            return Ok(());
        }
    }
}
```

**Step 2: Write the `map_event` function**

Map all crossterm key events to `AppMessage`:

```rust
fn map_event(event: Event) -> Option<AppMessage> {
    match event {
        Event::Key(KeyEvent { code, modifiers, kind: KeyEventKind::Press, .. }) => {
            match (code, modifiers) {
                (KeyCode::Char('c'), m) if m.contains(KeyModifiers::CONTROL) => Some(AppMessage::CtrlC),
                (KeyCode::Char('l'), m) if m.contains(KeyModifiers::CONTROL) => Some(AppMessage::CtrlL),
                (KeyCode::Esc, _) => Some(AppMessage::Escape),
                (KeyCode::BackTab, _) => Some(AppMessage::ShiftTab),
                (KeyCode::Tab, _) => Some(AppMessage::Tab),
                (KeyCode::Enter, _) => Some(AppMessage::Submit),
                (KeyCode::Backspace, m) if m.contains(KeyModifiers::ALT) => Some(AppMessage::DeleteWordBack),
                (KeyCode::Backspace, _) => Some(AppMessage::Backspace),
                (KeyCode::Delete, _) => Some(AppMessage::Delete),
                (KeyCode::Left, m) if m.contains(KeyModifiers::SHIFT) => Some(AppMessage::SelectLeft),
                (KeyCode::Right, m) if m.contains(KeyModifiers::SHIFT) => Some(AppMessage::SelectRight),
                (KeyCode::Left, m) if m.contains(KeyModifiers::ALT) || m.contains(KeyModifiers::CONTROL) => Some(AppMessage::WordLeft),
                (KeyCode::Right, m) if m.contains(KeyModifiers::ALT) || m.contains(KeyModifiers::CONTROL) => Some(AppMessage::WordRight),
                (KeyCode::Left, _) => Some(AppMessage::CursorLeft),
                (KeyCode::Right, _) => Some(AppMessage::CursorRight),
                (KeyCode::Home, m) if m.contains(KeyModifiers::SHIFT) => Some(AppMessage::SelectHome),
                (KeyCode::End, m) if m.contains(KeyModifiers::SHIFT) => Some(AppMessage::SelectEnd),
                (KeyCode::Home, _) => Some(AppMessage::Home),
                (KeyCode::End, _) => Some(AppMessage::End),
                (KeyCode::Up, _) => Some(AppMessage::HistoryUp),
                (KeyCode::Down, _) => Some(AppMessage::HistoryDown),
                (KeyCode::PageUp, _) => Some(AppMessage::ScrollUp(10)),
                (KeyCode::PageDown, _) => Some(AppMessage::ScrollDown(10)),
                (KeyCode::Char('a'), m) if m.contains(KeyModifiers::CONTROL) => Some(AppMessage::SelectAll),
                (KeyCode::Char(c), _) => Some(AppMessage::CharInput(c)),
                _ => None,
            }
        }
        Event::Resize(w, h) => Some(AppMessage::Resize { width: w, height: h }),
        Event::Paste(text) => Some(AppMessage::Paste(text)),
        _ => None,
    }
}
```

**Step 3: Add `futures` dependency to Cargo.toml**

Add `futures = "0.3"` to `simse-tui/Cargo.toml` and to workspace `[workspace.dependencies]`.

**Step 4: Run cargo check, commit**

Run: `cargo check -p simse-tui`
Commit: `feat: async event loop with tokio and full key mapping`

---

## Task 20: Three-pane layout with ratatui

**Files:**
- Modify: `simse-tui/src/app.rs` (the `view` function)

**Step 1: Implement the three-pane layout**

Replace the current simple `view()` with a proper layout:

```
┌─── SimSE ──────────────────────────────────┐
│ [Banner or Message History]                 │  ← scrollable
│                                             │
│ [Active area: streaming text + tool calls]  │  ← only when processing
├─────────────────────────────────────────────┤
│ > input text here_                          │  ← 3 lines high
├─────────────────────────────────────────────┤
│ ask (shift+tab) · ? for shortcuts  42 tokens│  ← 1 line status bar
└─────────────────────────────────────────────┘
```

Layout constraints:
- Status bar: `Length(1)` at bottom
- Input area: `Length(3)` above status bar
- Chat area: `Min(1)` fills remaining space

```rust
pub fn view(app: &App, frame: &mut Frame) {
    let area = frame.area();

    // Main vertical layout: [chat | input(3) | status(1)]
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),     // chat area
            Constraint::Length(3),   // input
            Constraint::Length(1),   // status bar
        ])
        .split(area);

    render_chat_area(app, frame, chunks[0]);
    render_input(app, frame, chunks[1]);
    render_status_bar(app, frame, chunks[2]);
}
```

**Step 2: Implement `render_chat_area`**

When `banner_visible` and output is empty, render a simple welcome banner. Otherwise render the output items list with scrolling.

For output items:
- `OutputItem::Message { role: "user", .. }` → cyan `> ` prefix + text
- `OutputItem::Message { role: "assistant", .. }` → plain text
- `OutputItem::ToolCall(..)` → `⏺ ToolName` with status color
- `OutputItem::Error { .. }` → red text
- `OutputItem::Info { .. }` → dim text
- `OutputItem::CommandResult { .. }` → plain text

If `loop_status != Idle`:
- Show `stream_text` if not empty
- Show active tool calls with spinner

**Step 3: Implement `render_input`**

Render the input state with cursor position. Show placeholder "Type a message..." when empty. Show `Press Ctrl-C again to exit` warning when `ctrl_c_pending`.

**Step 4: Implement `render_status_bar`**

Port the status bar from the TS version:

Left side (dim, `·` separated):
- Permission mode + "(shift+tab to cycle)"
- "esc to interrupt" when processing
- "plan mode" when enabled
- "verbose on" when enabled
- "? for shortcuts" always

Right side (dim, `·` separated):
- Token count (formatted: 1.2k tokens)
- Context percent (42% context)

**Step 5: Implement `render_shortcuts_overlay`**

When `screen == Screen::Shortcuts`, render a centered overlay showing keyboard shortcuts in a bordered box.

**Step 6: Write tests**

Test the format_tokens helper function:

```rust
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
```

**Step 7: Run cargo check, commit**

Run: `cargo check -p simse-tui`
Commit: `feat: three-pane layout with chat area, input, and status bar`

---

## Task 21: Banner widget

**Files:**
- Create: `simse-tui/src/banner.rs`
- Modify: `simse-tui/src/main.rs` (add `mod banner;`)
- Modify: `simse-tui/src/app.rs` (use banner in `render_chat_area`)

**Step 1: Create the banner widget**

Port the TS `Banner` component to a ratatui widget. Use box-drawing characters for borders:

```
 ╭── simse v0.1.0 ──────────────────────────╮
 │     ╭──╮    │ Tips for getting started    │
 │     ╰─╮│    │ Run /help for all commands  │
 │       ╰╯    │ Use /add to save a volume   │
 │             │ Use /search to find volumes │
 │  server·mod │─────────────────────────────│
 │  ~/work/dir │ Recent activity             │
 │             │ No recent activity           │
 ╰───────────────────────────────────────────╯
```

Implement as a function `render_banner(frame, area, app)` that:
1. Calculates column widths (27% left, 73% right with divider)
2. Renders the title bar with version
3. Left column: mascot (centered), server·model, work dir (dim)
4. Right column: Tips section, separator, Recent activity
5. Bottom border

Use ratatui `Span` with `Style::default().fg(Color::Cyan)` for the primary color and `Style::default().fg(Color::Green)` for secondary.

**Step 2: Wire into render_chat_area**

In `render_chat_area`, when `app.banner_visible && app.output.is_empty()`, call `render_banner` instead of the message list.

**Step 3: Run cargo check, commit**

Run: `cargo check -p simse-tui`
Commit: `feat: add welcome banner widget with two-column layout`

---

## Task 22: Output item rendering

**Files:**
- Create: `simse-tui/src/output.rs`
- Modify: `simse-tui/src/main.rs` (add `mod output;`)
- Modify: `simse-tui/src/app.rs` (use output rendering)

**Step 1: Create output item rendering functions**

```rust
use ratatui::text::{Line, Span};
use ratatui::style::{Color, Modifier, Style};
use simse_ui_core::app::{OutputItem, ToolCallState, ToolCallStatus};

/// Convert an OutputItem to ratatui Lines for rendering.
pub fn render_output_item(item: &OutputItem) -> Vec<Line<'static>> {
    match item {
        OutputItem::Message { role, text } => render_message(role, text),
        OutputItem::ToolCall(tc) => render_tool_call(tc),
        OutputItem::CommandResult { text } => vec![Line::from(text.clone())],
        OutputItem::Error { message } => render_error(message),
        OutputItem::Info { text } => render_info(text),
    }
}
```

Implement:
- `render_message(role, text)`: User messages get cyan `❯ ` prefix. Assistant messages get plain text. Both word-wrap to available width.
- `render_tool_call(tc)`: Show `⏺ name(args_summary)` with colored status indicator (yellow active, green completed, red failed). Show summary/duration on second line. Show diff lines if present.
- `render_error(msg)`: Red text with `✗ ` prefix.
- `render_info(text)`: Dim gray text.

**Step 2: Use in chat area rendering**

In `render_chat_area`, iterate `app.output`, call `render_output_item` for each, and render as a scrollable paragraph/list.

**Step 3: Write tests**

```rust
#[test]
fn render_user_message_has_prefix() {
    let lines = render_output_item(&OutputItem::Message {
        role: "user".into(),
        text: "hello".into(),
    });
    assert!(!lines.is_empty());
}

#[test]
fn render_error_has_red() {
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
    assert!(lines.len() >= 2); // name line + summary line
}
```

**Step 4: Run cargo check, commit**

Run: `cargo check -p simse-tui`
Commit: `feat: add output item rendering for chat messages and tool calls`

---

## Task 23: Shortcuts overlay

**Files:**
- Create: `simse-tui/src/shortcuts.rs`
- Modify: `simse-tui/src/main.rs` (add `mod shortcuts;`)
- Modify: `simse-tui/src/app.rs` (render when Screen::Shortcuts)

**Step 1: Create shortcuts data and rendering**

Define all keyboard shortcuts and render them in a centered bordered overlay:

```rust
pub struct Shortcut {
    pub keys: &'static str,
    pub description: &'static str,
}

pub fn all_shortcuts() -> Vec<Shortcut> {
    vec![
        Shortcut { keys: "Enter", description: "Submit message" },
        Shortcut { keys: "Ctrl+C ×2", description: "Exit" },
        Shortcut { keys: "Escape", description: "Interrupt / dismiss" },
        Shortcut { keys: "Ctrl+L", description: "Clear conversation" },
        Shortcut { keys: "Shift+Tab", description: "Cycle permission mode" },
        Shortcut { keys: "Up/Down", description: "Command history" },
        Shortcut { keys: "PageUp/Down", description: "Scroll output" },
        Shortcut { keys: "/command", description: "Run a command" },
        Shortcut { keys: "@file", description: "Mention a file" },
        Shortcut { keys: "?", description: "Show this help" },
    ]
}
```

Render as a centered popup box with Clear widget behind it to dim background.

**Step 2: Wire into view**

In the main `view()`, after rendering the normal layout, if `app.screen == Screen::Shortcuts`, render the overlay on top.

**Step 3: Handle `?` key on empty input**

In `update`, when `CharInput('?')` and `app.input.value.is_empty()`, show shortcuts instead of inserting the character.

**Step 4: Write tests**

```rust
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
fn escape_dismisses_shortcuts() {
    let mut app = App::new();
    app.screen = Screen::Shortcuts;
    app = update(app, AppMessage::Escape);
    assert_eq!(app.screen, Screen::Chat);
}

#[test]
fn any_key_dismisses_shortcuts() {
    let mut app = App::new();
    app.screen = Screen::Shortcuts;
    app = update(app, AppMessage::CharInput('a'));
    assert_eq!(app.screen, Screen::Chat);
}
```

**Step 5: Run cargo check, commit**

Run: `cargo check -p simse-tui`
Commit: `feat: add keyboard shortcuts overlay`

---

## Task 24: Command dispatch and slash commands

**Files:**
- Modify: `simse-tui/src/app.rs`

**Step 1: Add command dispatch logic to update**

When `Submit` is received and input starts with `/`, dispatch as a command:

```rust
fn handle_submit(app: &mut App) {
    let input = app.input.value.trim().to_string();
    if input.is_empty() {
        return;
    }

    // Add to history
    if app.history.last().map_or(true, |last| last != &input) {
        app.history.push(input.clone());
        if app.history.len() > 100 {
            app.history.remove(0);
        }
    }
    app.history_index = None;
    app.input = input::InputState::default();
    app.banner_visible = false;

    // Dispatch
    if input == "exit" || input == "quit" {
        app.should_quit = true;
    } else if let Some(cmd_input) = input.strip_prefix('/') {
        handle_command(app, cmd_input);
    } else {
        // Normal message → will be sent to bridge
        app.output.push(OutputItem::Message {
            role: "user".into(),
            text: input,
        });
    }
}

fn handle_command(app: &mut App, cmd_input: &str) {
    let (name, args) = cmd_input.split_once(' ').unwrap_or((cmd_input, ""));

    match name {
        "help" | "?" => {
            let text = format_help_text(&app.commands);
            app.output.push(OutputItem::CommandResult { text });
        }
        "clear" => {
            app.output.clear();
            app.banner_visible = true;
        }
        "exit" | "quit" | "q" => {
            app.should_quit = true;
        }
        "verbose" | "v" => {
            if let Some(val) = simse_ui_core::commands::registry::parse_bool_arg(args, app.verbose) {
                app.verbose = val;
                app.output.push(OutputItem::Info {
                    text: format!("Verbose: {}", if val { "on" } else { "off" }),
                });
            }
        }
        "plan" => {
            if let Some(val) = simse_ui_core::commands::registry::parse_bool_arg(args, app.plan_mode) {
                app.plan_mode = val;
                app.output.push(OutputItem::Info {
                    text: format!("Plan mode: {}", if val { "on" } else { "off" }),
                });
            }
        }
        "context" => {
            app.output.push(OutputItem::Info {
                text: format!("Tokens: {} · Context: {}%", app.total_tokens, app.context_percent),
            });
        }
        "compact" => {
            app.output.push(OutputItem::Info { text: "Compaction requested.".into() });
        }
        _ => {
            // Check command registry
            if simse_ui_core::commands::registry::find_command(&app.commands, name).is_some() {
                app.output.push(OutputItem::Info {
                    text: format!("/{name}: not yet implemented in TUI"),
                });
            } else {
                app.output.push(OutputItem::Error {
                    message: format!("Unknown command: /{name}"),
                });
            }
        }
    }
}
```

**Step 2: Implement `format_help_text`**

Group commands by category and format as a readable list.

**Step 3: Write tests**

```rust
#[test]
fn slash_help_adds_output() {
    let mut app = App::new();
    app.input = input::insert(&app.input, "/help");
    app = update(app, AppMessage::Submit);
    assert!(app.output.iter().any(|o| matches!(o, OutputItem::CommandResult { .. })));
}

#[test]
fn slash_clear_resets() {
    let mut app = App::new();
    app.output.push(OutputItem::Info { text: "test".into() });
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
    assert!(app.output.iter().any(|o| matches!(o, OutputItem::Error { .. })));
}

#[test]
fn normal_message_adds_to_output() {
    let mut app = App::new();
    app.input = input::insert(&app.input, "hello world");
    app = update(app, AppMessage::Submit);
    assert!(app.output.iter().any(|o| matches!(o, OutputItem::Message { role, .. } if role == "user")));
}
```

**Step 4: Run tests, commit**

Run: `cargo test -p simse-tui`
Commit: `feat: add command dispatch with /help, /clear, /exit, /verbose, /plan`

---

## Implementation Notes

- **No bridge connection yet**: Phase 4 builds the UI shell without actually connecting to the TS bridge. Normal messages are added to output but not sent anywhere. Bridge integration is Phase 8.
- **Rendering tests**: Use `ratatui::backend::TestBackend` for rendering assertions where needed. Most logic tests only need the `update()` function.
- **Futures dependency**: Required for `EventStream::next()` in the async event loop.
- **Crossterm features**: Ensure `crossterm` has `event-stream` feature enabled for async event reading.
- **History cap**: Max 100 entries, deduplicate consecutive identical inputs.
- **Scroll behavior**: `scroll_offset` is measured from the bottom (0 = latest content visible). PageUp/Down move by 10 lines.
