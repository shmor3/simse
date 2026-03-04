# Autocomplete, Overlay Focus, and E2E Test Suite — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers-extended-cc:executing-plans to implement this plan task-by-task.

**Goal:** Fix autocomplete popup rendering, overlay keyboard focus routing, and build a comprehensive PTY-based e2e test suite.

**Architecture:** Three independent workstreams — inline autocomplete below input using existing `CommandAutocompleteState`, screen-aware focus routing in `update()` for Settings/Librarians/Setup overlays, and a hybrid PTY e2e test suite with `ratatui-testlib` + custom `SimseTestHarness` + mock ACP server.

**Tech Stack:** Rust (edition 2024), ratatui, crossterm, tokio, ratatui-testlib (dev), tempfile (dev)

---

## Task 0: Add `--data-dir` CLI Flag

E2E tests need isolated temp directories. The CLI currently has no `--data-dir` flag, but `ConfigOptions` in simse-bridge already supports `data_dir: Option<PathBuf>`.

**Files:**
- Modify: `simse-tui/src/cli_args.rs`

**Step 1: Write the failing test**

Add to `simse-tui/src/cli_args.rs` at the end of the `tests` module:

```rust
#[test]
fn cli_args_data_dir() {
    let result = parse_cli_args(&args(&["--data-dir", "/tmp/simse-test"]));
    assert_eq!(result.data_dir.as_deref(), Some("/tmp/simse-test"));
}

#[test]
fn cli_args_data_dir_missing_value() {
    let result = parse_cli_args(&args(&["--data-dir"]));
    assert_eq!(result.data_dir, None);
}
```

**Step 2: Run tests to verify failure**

Run: `cd simse-tui && cargo test cli_args_data_dir -- --nocapture`
Expected: compilation error — `data_dir` field doesn't exist on CliArgs

**Step 3: Implement**

In `simse-tui/src/cli_args.rs`:

1. Add field to `CliArgs` struct (after `agent`):
```rust
/// Override data directory (for isolated testing).
pub data_dir: Option<String>,
```

2. Add to `Default` impl: `data_dir: None,`

3. Add to `parse_cli_args` match arm (after `--agent`):
```rust
"--data-dir" => {
    if let Some(value) = args.get(i + 1) {
        result.data_dir = Some(value.clone());
        i += 1;
    }
}
```

4. Add to `help_text()`:
```
      --data-dir <path>     Override data directory
```

**Step 4: Run tests to verify pass**

Run: `cd simse-tui && cargo test cli_args -- --nocapture`
Expected: ALL cli_args tests pass

**Step 5: Commit**

```bash
git add simse-tui/src/cli_args.rs
git commit -m "feat(simse-tui): add --data-dir CLI flag for test isolation"
```

---

## Task 1: Integrate CommandAutocompleteState into App

Replace the disconnected `PromptMode` enum with the feature-complete `CommandAutocompleteState` from `autocomplete.rs`.

**Files:**
- Modify: `simse-tui/src/app.rs`

**Step 1: Write the failing test**

Add to `simse-tui/src/app.rs` test module (at bottom of file). First, check if there's an existing test module — there should be one. Add:

```rust
#[test]
fn app_has_autocomplete_state() {
    let app = App::new();
    assert!(!app.autocomplete.is_active());
}
```

**Step 2: Run test to verify failure**

Run: `cd simse-tui && cargo test app_has_autocomplete_state -- --nocapture`
Expected: compilation error — no field `autocomplete` on `App`

**Step 3: Implement**

In `simse-tui/src/app.rs`:

1. Add import at the top (with other crate imports):
```rust
use crate::autocomplete::CommandAutocompleteState;
```

2. Remove the `PromptMode` enum entirely (lines 44–52):
```rust
// DELETE:
// pub enum PromptMode { Normal, Autocomplete { ... } }
```

3. In `App` struct, replace `pub prompt_mode: PromptMode` (line 74) with:
```rust
/// Command autocomplete state.
pub autocomplete: CommandAutocompleteState,
```

4. In `App::new()`, replace `prompt_mode: PromptMode::Normal` (line 121) with:
```rust
autocomplete: CommandAutocompleteState::new(),
```

5. Fix any references to `prompt_mode` in the file — the Tab handler at lines 439–482 needs rewriting (covered in Task 2). For now, replace the entire Tab match arm with a placeholder that compiles:
```rust
AppMessage::Tab => {
    // Will be wired in Task 2
}
```

**Step 4: Fix compilation**

Run: `cd simse-tui && cargo build 2>&1 | head -30`
Fix any remaining references to `PromptMode` in tests or other files. The `PromptMode` type may be used in tests — search with `grep -rn "PromptMode\|prompt_mode" simse-tui/src/`.

**Step 5: Run tests**

Run: `cd simse-tui && cargo test -- --nocapture 2>&1 | tail -5`
Expected: all tests pass (some Tab-related tests may need updating if they reference `prompt_mode`)

**Step 6: Commit**

```bash
git add simse-tui/src/app.rs
git commit -m "refactor(simse-tui): replace PromptMode with CommandAutocompleteState"
```

---

## Task 2: Wire Autocomplete Handlers in update()

Connect CharInput, Backspace, Paste, Tab, Up/Down, and Escape to `CommandAutocompleteState` methods.

**Files:**
- Modify: `simse-tui/src/app.rs`

**Step 1: Write the failing tests**

Add to the test module in `simse-tui/src/app.rs`:

```rust
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
    // Should have "help" as a match
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
    // Bare "/" shows all commands
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
    // After backspace to "/h", should have more matches
    assert!(app.autocomplete.matches.len() >= count_he);
}
```

**Step 2: Run tests to verify failure**

Run: `cd simse-tui && cargo test typing_slash_activates -- --nocapture`
Expected: FAIL — autocomplete is not activated on CharInput

**Step 3: Implement**

Modify the `update()` function in `simse-tui/src/app.rs`:

**CharInput handler** (around line 257): After inserting the character AND after the `Shortcuts` early return, add autocomplete update:
```rust
AppMessage::CharInput(c) => {
    if app.screen == Screen::Shortcuts {
        app.screen = Screen::Chat;
        return app;
    }
    if c == '?' && app.input.value.is_empty() {
        app.screen = Screen::Shortcuts;
    } else {
        app.input = input::insert(&app.input, &c.to_string());
        // Update autocomplete after input change
        app.autocomplete.update_matches(&app.input.value, &app.commands);
    }
}
```

**Backspace handler** (around line 309): After deleting, update autocomplete:
```rust
AppMessage::Backspace => {
    app.input = input::backspace(&app.input);
    app.autocomplete.update_matches(&app.input.value, &app.commands);
}
```

**Paste handler** (around line 268): After inserting, update autocomplete:
```rust
AppMessage::Paste(text) => {
    app.input = input::insert(&app.input, &text);
    app.autocomplete.update_matches(&app.input.value, &app.commands);
}
```

**Delete handler**: After deleting, update autocomplete:
```rust
AppMessage::Delete => {
    app.input = input::delete(&app.input);
    app.autocomplete.update_matches(&app.input.value, &app.commands);
}
```

**DeleteWordBack handler**: After deleting, update autocomplete:
```rust
AppMessage::DeleteWordBack => {
    app.input = input::delete_word_back(&app.input);
    app.autocomplete.update_matches(&app.input.value, &app.commands);
}
```

**HistoryUp handler** (around line 351): When autocomplete is active, navigate instead of history:
```rust
AppMessage::HistoryUp => {
    if app.autocomplete.is_active() {
        app.autocomplete.move_up();
        return app;
    }
    // ... existing history code
}
```

**HistoryDown handler** (around line 375): When autocomplete is active, navigate instead of history:
```rust
AppMessage::HistoryDown => {
    if app.autocomplete.is_active() {
        app.autocomplete.move_down();
        return app;
    }
    // ... existing history code
}
```

**Tab handler** (around line 439): Replace entirely:
```rust
AppMessage::Tab => {
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
        // Activate autocomplete on first Tab press
        app.autocomplete.update_matches(&app.input.value, &app.commands);
    }
}
```

**Escape handler** (around line 418): When autocomplete is active, dismiss it instead of the screen:
```rust
AppMessage::Escape => {
    if app.autocomplete.is_active() {
        app.autocomplete.deactivate();
    } else if app.screen != Screen::Chat {
        app.screen = Screen::Chat;
    } else if app.loop_status != LoopStatus::Idle {
        app.loop_status = LoopStatus::Idle;
        app.output.push(OutputItem::Info {
            text: "Interrupted.".into(),
        });
    }
}
```

**Submit handler** (around line 271): Deactivate autocomplete when submitting:
```rust
AppMessage::Submit => {
    app.autocomplete.deactivate();
    // ... rest of existing Submit code
}
```

**Step 4: Run tests**

Run: `cd simse-tui && cargo test -- --nocapture 2>&1 | tail -5`
Expected: ALL tests pass including the new autocomplete tests

**Step 5: Commit**

```bash
git add simse-tui/src/app.rs
git commit -m "feat(simse-tui): wire CommandAutocompleteState into update() handlers"
```

---

## Task 3: Create Inline Autocomplete Renderer

Replace the bordered popup in `render_command_autocomplete()` with plain inline lines below the input. Update `view()` to insert a completions area between input and status bar when active.

**Files:**
- Modify: `simse-tui/src/autocomplete.rs`
- Modify: `simse-tui/src/app.rs`

**Step 1: Write the failing test**

In `simse-tui/src/autocomplete.rs`, add to test module:

```rust
#[test]
fn render_inline_completions_produces_lines() {
    let cmds = test_commands();
    let mut state = CommandAutocompleteState::new();
    state.activate("/h", &cmds);

    let lines = render_inline_completions(&state, 60);
    assert!(!lines.is_empty());
    // Should have at least the header line + match lines
    assert!(lines.len() >= 2); // matches for "help", "history", "search"
}
```

**Step 2: Run test to verify failure**

Run: `cd simse-tui && cargo test render_inline_completions_produces -- --nocapture`
Expected: compilation error — function doesn't exist

**Step 3: Implement the inline renderer**

In `simse-tui/src/autocomplete.rs`, add a new public function (keep the old `render_command_autocomplete` for now — it can be deleted later):

```rust
/// Render inline completion lines below the input (no border/popup).
///
/// Returns a list of `Line` items to be rendered in a dedicated layout chunk.
/// Format: `  /name          description`
/// Selected item is highlighted in cyan+bold.
/// Max `MAX_VISIBLE_MATCHES` items shown.
pub fn render_inline_completions<'a>(
    state: &CommandAutocompleteState,
    width: u16,
) -> Vec<Line<'a>> {
    if !state.is_active() {
        return Vec::new();
    }

    let visible = state.visible_matches();
    if visible.is_empty() {
        return Vec::new();
    }

    let selected_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let normal_name_style = Style::default().fg(Color::White);
    let desc_style = Style::default().fg(Color::DarkGray);
    let selected_desc_style = Style::default().fg(Color::Gray);

    // Find the longest command name for alignment
    let max_name_len = visible
        .iter()
        .map(|m| m.name.len() + 1) // +1 for the '/'
        .max()
        .unwrap_or(0);

    visible
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let is_selected = i == state.selected_index();
            let indicator = if is_selected { " > " } else { "   " };
            let name_style = if is_selected {
                selected_style
            } else {
                normal_name_style
            };
            let d_style = if is_selected {
                selected_desc_style
            } else {
                desc_style
            };

            let cmd_name = format!("/{}", m.name);
            let padding = " ".repeat(max_name_len.saturating_sub(cmd_name.len()) + 2);

            Line::from(vec![
                Span::styled(indicator.to_string(), name_style),
                Span::styled(cmd_name, name_style),
                Span::styled(padding, desc_style),
                Span::styled(m.description.clone(), d_style),
            ])
        })
        .collect()
}
```

**Step 4: Update view() to show completions below input**

In `simse-tui/src/app.rs`, import the new render function:
```rust
use crate::autocomplete::{render_inline_completions, CommandAutocompleteState};
```

Replace the `view()` function's layout to conditionally insert a completions chunk:

```rust
pub fn view(app: &App, frame: &mut Frame) {
    let area = frame.area();

    let completions_height = if app.autocomplete.is_active() {
        (app.autocomplete.visible_matches().len() as u16).min(8)
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
        _ => {}
    }
}
```

**Step 5: Run tests**

Run: `cd simse-tui && cargo test -- --nocapture 2>&1 | tail -5`
Expected: ALL tests pass

**Step 6: Commit**

```bash
git add simse-tui/src/autocomplete.rs simse-tui/src/app.rs
git commit -m "feat(simse-tui): add inline autocomplete renderer below input"
```

---

## Task 4: Ghost Text Rendering

When there's a single autocomplete match, show the remaining characters as dimmed ghost text after the cursor in the input field.

**Files:**
- Modify: `simse-tui/src/app.rs` (render_input function)

**Step 1: Implement ghost text in render_input**

Modify `render_input()` in `simse-tui/src/app.rs` to append ghost text when available:

```rust
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

    // Cursor: hide when overlay is active (Task 5), otherwise show
    if app.screen == Screen::Chat {
        let cursor_x = area.x.saturating_add(1).saturating_add(
            (app.input.cursor as u16).min(area.width.saturating_sub(2)),
        );
        let cursor_y = area.y + 1;
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}
```

**Step 2: Run tests**

Run: `cd simse-tui && cargo test -- --nocapture 2>&1 | tail -5`
Expected: ALL tests pass

**Step 3: Commit**

```bash
git add simse-tui/src/app.rs
git commit -m "feat(simse-tui): add ghost text and hide cursor on overlays"
```

---

## Task 5: Overlay Focus Routing — Settings

Route keyboard events to `SettingsExplorerState` when `screen == Screen::Settings`.

**Files:**
- Modify: `simse-tui/src/app.rs`

**Step 1: Write the failing tests**

Add to app.rs test module:

```rust
#[test]
fn settings_overlay_captures_arrow_keys() {
    let mut app = App::new();
    app.screen = Screen::Settings;
    let initial_file = app.settings_state.selected_file;
    app = update(app, AppMessage::HistoryDown);
    // Settings.move_down needs item_count; in FileList mode there are 8 files
    assert_ne!(app.settings_state.selected_file, initial_file);
}

#[test]
fn settings_overlay_captures_escape() {
    let mut app = App::new();
    app.screen = Screen::Settings;
    // At FileList level, back() returns true -> dismiss
    app = update(app, AppMessage::Escape);
    assert_eq!(app.screen, Screen::Chat);
}

#[test]
fn settings_overlay_captures_enter() {
    let mut app = App::new();
    app.screen = Screen::Settings;
    // Enter at FileList goes to FieldList
    app = update(app, AppMessage::Submit);
    assert_eq!(app.settings_state.level, crate::overlays::settings::SettingsLevel::FieldList);
}

#[test]
fn settings_overlay_captures_char_input() {
    let mut app = App::new();
    app.screen = Screen::Settings;
    // In FileList mode, type_char is a no-op but should NOT go to input
    let original_input = app.input.value.clone();
    app = update(app, AppMessage::CharInput('a'));
    assert_eq!(app.input.value, original_input);
}
```

**Step 2: Run tests to verify failure**

Run: `cd simse-tui && cargo test settings_overlay_captures -- --nocapture`
Expected: FAIL — HistoryDown goes to input history, not settings

**Step 3: Implement screen-aware dispatch**

In `simse-tui/src/app.rs` `update()` function, add screen guards to each input handler. Import the settings CONFIG_FILES constant:

```rust
use crate::overlays::settings::CONFIG_FILES;
```

**CharInput** — add Settings/Librarians/Setup guards at the top:
```rust
AppMessage::CharInput(c) => {
    match &app.screen {
        Screen::Settings => {
            app.settings_state.type_char(c);
            return app;
        }
        Screen::Librarians => {
            app.librarian_state.type_char(c);
            return app;
        }
        Screen::Setup { .. } => {
            app.setup_state.type_char(c);
            return app;
        }
        Screen::Shortcuts => {
            app.screen = Screen::Chat;
            return app;
        }
        _ => {}
    }
    // ... existing Chat behavior
}
```

**Backspace** — add overlay guards:
```rust
AppMessage::Backspace => {
    match &app.screen {
        Screen::Settings => {
            app.settings_state.backspace();
            return app;
        }
        Screen::Librarians => {
            app.librarian_state.backspace();
            return app;
        }
        Screen::Setup { .. } => {
            app.setup_state.backspace();
            return app;
        }
        _ => {}
    }
    app.input = input::backspace(&app.input);
    app.autocomplete.update_matches(&app.input.value, &app.commands);
}
```

**HistoryUp (Up arrow)** — add overlay guards:
```rust
AppMessage::HistoryUp => {
    match &app.screen {
        Screen::Settings => {
            app.settings_state.move_up();
            return app;
        }
        Screen::Librarians => {
            app.librarian_state.move_up();
            return app;
        }
        Screen::Setup { .. } => {
            app.setup_state.move_up();
            return app;
        }
        _ => {}
    }
    if app.autocomplete.is_active() {
        app.autocomplete.move_up();
        return app;
    }
    // ... existing history code
}
```

**HistoryDown (Down arrow)** — add overlay guards:
```rust
AppMessage::HistoryDown => {
    match &app.screen {
        Screen::Settings => {
            // Settings.move_down() needs item_count
            let count = match app.settings_state.level {
                crate::overlays::settings::SettingsLevel::FileList => CONFIG_FILES.len(),
                _ => {
                    // For FieldList/Editing, we need the field count from config data.
                    // Use the config_data length or default to a safe value.
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
        Screen::Librarians => {
            app.librarian_state.move_down();
            return app;
        }
        Screen::Setup { .. } => {
            app.setup_state.move_down();
            return app;
        }
        _ => {}
    }
    if app.autocomplete.is_active() {
        app.autocomplete.move_down();
        return app;
    }
    // ... existing history code
}
```

**Submit (Enter)** — add overlay guards:
```rust
AppMessage::Submit => {
    match &app.screen {
        Screen::Settings => {
            // enter() needs current_value for the selected field
            let current_value = get_settings_current_value(&app);
            app.settings_state.enter(&current_value);
            return app;
        }
        Screen::Librarians => {
            app.librarian_state.enter();
            return app;
        }
        Screen::Setup { .. } => {
            let action = app.setup_state.enter();
            handle_setup_action(&mut app, action);
            return app;
        }
        _ => {}
    }
    app.autocomplete.deactivate();
    // ... existing Submit code
}
```

**Escape** — add overlay-specific back() handling:
```rust
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
```

**Tab** — add Setup toggle_field guard:
```rust
AppMessage::Tab => {
    match &app.screen {
        Screen::Setup { .. } => {
            app.setup_state.toggle_field();
            return app;
        }
        _ => {}
    }
    // ... existing autocomplete Tab behavior
}
```

**Add helper functions** at the bottom of `app.rs` (before tests):

```rust
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
            // Store the preset selection as a bridge action
            app.output.push(OutputItem::CommandResult {
                text: format!("Selected preset: {}", preset.label()),
            });
            app.screen = Screen::Chat;
        }
        SetupAction::OpenOllamaWizard => {
            app.output.push(OutputItem::Info {
                text: "Opening Ollama wizard...".into(),
            });
            // The event loop will handle the actual wizard flow
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
```

**Step 4: Run tests**

Run: `cd simse-tui && cargo test -- --nocapture 2>&1 | tail -5`
Expected: ALL tests pass

**Step 5: Commit**

```bash
git add simse-tui/src/app.rs
git commit -m "feat(simse-tui): add screen-aware focus routing for all overlays"
```

---

## Task 6: E2E Test Infrastructure — Dependencies and Harness

Set up the e2e test infrastructure: add dev dependencies, create the test harness wrapper.

**Files:**
- Modify: `simse-tui/Cargo.toml`
- Create: `simse-tui/tests/e2e/mod.rs`
- Create: `simse-tui/tests/e2e/harness.rs`
- Create: `simse-tui/tests/e2e/config.rs`

**Step 1: Research ratatui-testlib availability**

Run: `cd simse-tui && cargo search ratatui-testlib 2>&1 | head -5`

**IMPORTANT**: `ratatui-testlib` may not be published on crates.io yet. If not found, we'll use `portable-pty` + `vt100` directly as our PTY stack:
- `portable-pty = "0.8"` — cross-platform PTY
- `vt100 = "0.15"` — terminal emulator for screen parsing

Either way, add dev dependencies to `simse-tui/Cargo.toml`:

```toml
[dev-dependencies]
tempfile = "3"
portable-pty = "0.8"
vt100 = "0.15"
```

**Step 2: Create test harness**

Create `simse-tui/tests/e2e/harness.rs`:

```rust
//! SimseTestHarness: PTY-based test harness for simse-tui e2e tests.
//!
//! Spawns the simse-tui binary in a pseudo-terminal, provides methods to
//! send keystrokes and assert screen content.

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Default terminal size for tests.
const DEFAULT_COLS: u16 = 80;
const DEFAULT_ROWS: u16 = 24;

/// Default timeout for wait operations.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// Test harness wrapping a PTY-spawned simse-tui instance.
pub struct SimseTestHarness {
    /// The PTY master (for writing input).
    master: Box<dyn portable_pty::MasterPty + Send>,
    /// Writer end of the PTY.
    writer: Box<dyn Write + Send>,
    /// Background reader thread collects output into a shared buffer.
    screen_buf: Arc<Mutex<Vec<u8>>>,
    /// VT100 parser for screen state.
    parser: Arc<Mutex<vt100::Parser>>,
    /// Child process handle.
    child: Box<dyn portable_pty::Child + Send + Sync>,
    /// Temp directory for isolated config (kept alive for lifetime).
    _temp_dir: tempfile::TempDir,
}

impl SimseTestHarness {
    /// Spawn simse-tui in a PTY with an isolated temp data directory.
    ///
    /// The binary is found via `cargo_bin()` (the built debug binary).
    pub fn new() -> Self {
        Self::with_args(&[])
    }

    /// Spawn with additional CLI arguments.
    pub fn with_args(extra_args: &[&str]) -> Self {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let data_dir = temp_dir.path().join("data");
        std::fs::create_dir_all(&data_dir).expect("create data dir");

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: DEFAULT_ROWS,
                cols: DEFAULT_COLS,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("open PTY");

        let binary = cargo_bin("simse-tui");
        let mut cmd = CommandBuilder::new(&binary);
        cmd.arg("--data-dir");
        cmd.arg(data_dir.to_str().unwrap());
        for arg in extra_args {
            cmd.arg(arg);
        }
        // Set TERM for proper terminal behavior
        cmd.env("TERM", "xterm-256color");

        let child = pair.slave.spawn_command(cmd).expect("spawn simse-tui");
        let writer = pair.master.take_writer().expect("take writer");

        // Set up background reader
        let parser = Arc::new(Mutex::new(vt100::Parser::new(
            DEFAULT_ROWS,
            DEFAULT_COLS,
            0,
        )));
        let screen_buf = Arc::new(Mutex::new(Vec::new()));

        let parser_clone = Arc::clone(&parser);
        let buf_clone = Arc::clone(&screen_buf);
        let mut reader = pair.master.try_clone_reader().expect("clone reader");

        std::thread::spawn(move || {
            let mut tmp = [0u8; 4096];
            loop {
                match reader.read(&mut tmp) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let data = &tmp[..n];
                        buf_clone.lock().unwrap().extend_from_slice(data);
                        parser_clone.lock().unwrap().process(data);
                    }
                }
            }
        });

        SimseTestHarness {
            master: pair.master,
            writer,
            screen_buf,
            parser,
            child,
            _temp_dir: temp_dir,
        }
    }

    /// Type text character by character.
    pub fn type_text(&mut self, text: &str) {
        for c in text.chars() {
            let mut buf = [0u8; 4];
            let s = c.encode_utf8(&mut buf);
            self.writer.write_all(s.as_bytes()).expect("write char");
            self.writer.flush().expect("flush");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Type text and press Enter.
    pub fn submit(&mut self, text: &str) {
        self.type_text(text);
        self.press_enter();
    }

    /// Send Enter key.
    pub fn press_enter(&mut self) {
        self.writer.write_all(b"\r").expect("write enter");
        self.writer.flush().expect("flush");
    }

    /// Send Escape key.
    pub fn press_escape(&mut self) {
        self.writer.write_all(b"\x1b").expect("write escape");
        self.writer.flush().expect("flush");
    }

    /// Send Tab key.
    pub fn press_tab(&mut self) {
        self.writer.write_all(b"\t").expect("write tab");
        self.writer.flush().expect("flush");
    }

    /// Send Backspace key.
    pub fn press_backspace(&mut self) {
        self.writer.write_all(b"\x7f").expect("write backspace");
        self.writer.flush().expect("flush");
    }

    /// Send Up arrow key.
    pub fn press_up(&mut self) {
        self.writer.write_all(b"\x1b[A").expect("write up");
        self.writer.flush().expect("flush");
    }

    /// Send Down arrow key.
    pub fn press_down(&mut self) {
        self.writer.write_all(b"\x1b[B").expect("write down");
        self.writer.flush().expect("flush");
    }

    /// Send Left arrow key.
    pub fn press_left(&mut self) {
        self.writer.write_all(b"\x1b[D").expect("write left");
        self.writer.flush().expect("flush");
    }

    /// Send Right arrow key.
    pub fn press_right(&mut self) {
        self.writer.write_all(b"\x1b[C").expect("write right");
        self.writer.flush().expect("flush");
    }

    /// Send Ctrl+C.
    pub fn press_ctrl_c(&mut self) {
        self.writer.write_all(b"\x03").expect("write ctrl-c");
        self.writer.flush().expect("flush");
    }

    /// Send Ctrl+L.
    pub fn press_ctrl_l(&mut self) {
        self.writer.write_all(b"\x0c").expect("write ctrl-l");
        self.writer.flush().expect("flush");
    }

    /// Send PageUp.
    pub fn press_page_up(&mut self) {
        self.writer.write_all(b"\x1b[5~").expect("write pgup");
        self.writer.flush().expect("flush");
    }

    /// Send PageDown.
    pub fn press_page_down(&mut self) {
        self.writer.write_all(b"\x1b[6~").expect("write pgdn");
        self.writer.flush().expect("flush");
    }

    /// Send Shift+Tab (BackTab).
    pub fn press_shift_tab(&mut self) {
        self.writer.write_all(b"\x1b[Z").expect("write shift-tab");
        self.writer.flush().expect("flush");
    }

    /// Send Home key.
    pub fn press_home(&mut self) {
        self.writer.write_all(b"\x1b[H").expect("write home");
        self.writer.flush().expect("flush");
    }

    /// Send End key.
    pub fn press_end(&mut self) {
        self.writer.write_all(b"\x1b[F").expect("write end");
        self.writer.flush().expect("flush");
    }

    /// Send Delete key.
    pub fn press_delete(&mut self) {
        self.writer.write_all(b"\x1b[3~").expect("write delete");
        self.writer.flush().expect("flush");
    }

    /// Get the current screen content as a string.
    pub fn screen_text(&self) -> String {
        let parser = self.parser.lock().unwrap();
        let screen = parser.screen();
        let mut text = String::new();
        for row in 0..screen.size().0 {
            let line = screen.rows_formatted(row, row + 1);
            // Use the plain text content
            text.push_str(&screen.contents_between(
                row, 0,
                row, screen.size().1,
            ));
            text.push('\n');
        }
        text
    }

    /// Wait for specific text to appear on screen.
    pub fn wait_for_text(&self, text: &str, timeout: Duration) -> bool {
        let start = Instant::now();
        while start.elapsed() < timeout {
            if self.screen_text().contains(text) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        false
    }

    /// Wait for text with default timeout (5s).
    pub fn wait_for(&self, text: &str) -> bool {
        self.wait_for_text(text, DEFAULT_TIMEOUT)
    }

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

    /// Wait for the input prompt to be ready.
    pub fn wait_for_prompt(&self) -> bool {
        self.wait_for("Input")
    }

    /// Gracefully quit the application.
    pub fn quit(&mut self) {
        self.press_ctrl_c();
        std::thread::sleep(Duration::from_millis(100));
        self.press_ctrl_c();
        std::thread::sleep(Duration::from_millis(500));
    }
}

impl Drop for SimseTestHarness {
    fn drop(&mut self) {
        // Try to kill the child process
        let _ = self.child.kill();
    }
}

/// Find the path to a cargo-built binary.
fn cargo_bin(name: &str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop(); // up from simse-tui
    path.push("target");
    path.push("debug");
    if cfg!(windows) {
        path.push(format!("{name}.exe"));
    } else {
        path.push(name);
    }
    if !path.exists() {
        // Try the workspace target dir directly
        path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("target");
        path.push("debug");
        if cfg!(windows) {
            path.push(format!("{name}.exe"));
        } else {
            path.push(name);
        }
    }
    path
}
```

**Step 3: Create test config scaffold**

Create `simse-tui/tests/e2e/config.rs`:

```rust
//! Test configuration scaffold — creates isolated temp environments.

use std::path::Path;

/// Write minimal config files to a data directory.
pub fn write_minimal_config(data_dir: &Path) {
    let config = data_dir.join("config.json");
    std::fs::write(
        &config,
        r#"{"defaultServer": "mock", "permissionMode": "bypass"}"#,
    )
    .expect("write config.json");

    let acp = data_dir.join("acp.json");
    std::fs::write(
        &acp,
        r#"{"servers": {}}"#,
    )
    .expect("write acp.json");
}
```

**Step 4: Create module file**

Create `simse-tui/tests/e2e/mod.rs`:

```rust
pub mod harness;
pub mod config;
```

**Step 5: Verify build**

Run: `cd simse-tui && cargo test --no-run 2>&1 | tail -5`
Expected: compiles successfully

**Step 6: Commit**

```bash
git add simse-tui/Cargo.toml simse-tui/tests/e2e/
git commit -m "feat(simse-tui): add e2e test infrastructure with PTY harness"
```

---

## Task 7: E2E Tests — Startup

**Files:**
- Create: `simse-tui/tests/e2e/startup.rs`

Create `simse-tui/tests/e2e/startup.rs` with 5 tests:

```rust
//! E2E tests: startup behavior.

use super::harness::SimseTestHarness;
use std::time::Duration;

#[test]
fn startup_shows_banner() {
    let harness = SimseTestHarness::new();
    assert!(harness.wait_for("SimSE", Duration::from_secs(10)));
    harness.assert_contains("SimSE");
}

#[test]
fn startup_shows_input_prompt() {
    let harness = SimseTestHarness::new();
    assert!(harness.wait_for_prompt());
    harness.assert_contains("Input");
}

#[test]
fn startup_shows_status_bar() {
    let harness = SimseTestHarness::new();
    assert!(harness.wait_for_prompt());
    // Status bar shows permission mode
    harness.assert_contains("ask");
}

#[test]
fn startup_shows_version() {
    let harness = SimseTestHarness::new();
    assert!(harness.wait_for_prompt());
    // Version should appear in banner or status bar
    let screen = harness.screen_text();
    // The version comes from CARGO_PKG_VERSION
    assert!(screen.contains("v") || screen.contains("0."));
}

#[test]
fn startup_shows_tips() {
    let harness = SimseTestHarness::new();
    assert!(harness.wait_for_prompt());
    // Banner should show tip text or keyboard hint
    let screen = harness.screen_text();
    assert!(
        screen.contains("?") || screen.contains("help") || screen.contains("Ctrl"),
        "Expected startup tips on screen: {}",
        screen
    );
}
```

Add to `mod.rs`: `mod startup;`

**Commit:**
```bash
git add simse-tui/tests/e2e/
git commit -m "test(simse-tui): add e2e startup tests"
```

---

## Task 8: E2E Tests — Input

**Files:**
- Create: `simse-tui/tests/e2e/input.rs`

Create `simse-tui/tests/e2e/input.rs` with 8 tests covering typing, backspace, delete, arrows, paste, history, Ctrl+C, and word-delete.

Add to `mod.rs`: `mod input;`

**Commit:**
```bash
git add simse-tui/tests/e2e/
git commit -m "test(simse-tui): add e2e input tests"
```

---

## Task 9: E2E Tests — Autocomplete

**Files:**
- Create: `simse-tui/tests/e2e/autocomplete.rs`

7 tests: `/` trigger, live filter, Tab accept, Up/Down nav, Escape dismiss, single-match ghost, alias match.

Add to `mod.rs`: `mod autocomplete;`

**Commit:**
```bash
git add simse-tui/tests/e2e/
git commit -m "test(simse-tui): add e2e autocomplete tests"
```

---

## Task 10: E2E Tests — Meta Commands

**Files:**
- Create: `simse-tui/tests/e2e/commands_meta.rs`

9 tests: /help, /help <cmd>, /clear, /exit, /verbose, /plan, /context, /compact, /shortcuts.

Add to `mod.rs`: `mod commands_meta;`

**Commit:**
```bash
git add simse-tui/tests/e2e/
git commit -m "test(simse-tui): add e2e meta command tests"
```

---

## Task 11: E2E Tests — Library Commands

**Files:**
- Create: `simse-tui/tests/e2e/commands_library.rs`

7 tests: /add, /search, /recommend, /topics, /volumes, /get, /delete. These will produce error messages (no ACP connected) which we can assert on.

Add to `mod.rs`: `mod commands_library;`

**Commit:**
```bash
git add simse-tui/tests/e2e/
git commit -m "test(simse-tui): add e2e library command tests"
```

---

## Task 12: E2E Tests — Session Commands

**Files:**
- Create: `simse-tui/tests/e2e/commands_session.rs`

7 tests: /sessions, /resume, /rename, /server, /model, /mcp, /acp.

Add to `mod.rs`: `mod commands_session;`

**Commit:**
```bash
git add simse-tui/tests/e2e/
git commit -m "test(simse-tui): add e2e session command tests"
```

---

## Task 13: E2E Tests — Config Commands

**Files:**
- Create: `simse-tui/tests/e2e/commands_config.rs`

5 tests: /config, /settings, /init, /setup, /factory-reset.

Add to `mod.rs`: `mod commands_config;`

**Commit:**
```bash
git add simse-tui/tests/e2e/
git commit -m "test(simse-tui): add e2e config command tests"
```

---

## Task 14: E2E Tests — File Commands

**Files:**
- Create: `simse-tui/tests/e2e/commands_files.rs`

5 tests: /files, /save, /validate, /discard, /diff.

Add to `mod.rs`: `mod commands_files;`

**Commit:**
```bash
git add simse-tui/tests/e2e/
git commit -m "test(simse-tui): add e2e file command tests"
```

---

## Task 15: E2E Tests — Tool Commands

**Files:**
- Create: `simse-tui/tests/e2e/commands_tools.rs`

5 tests: /tools, /agents, /skills, /prompts, /chain.

Add to `mod.rs`: `mod commands_tools;`

**Commit:**
```bash
git add simse-tui/tests/e2e/
git commit -m "test(simse-tui): add e2e tool command tests"
```

---

## Task 16: E2E Tests — Overlays

**Files:**
- Create: `simse-tui/tests/e2e/overlays.rs`

8 tests: Settings open & nav, Librarians open & nav, Setup open & nav, Shortcuts open & dismiss, Escape from each, focus transfer between overlays.

Add to `mod.rs`: `mod overlays;`

**Commit:**
```bash
git add simse-tui/tests/e2e/
git commit -m "test(simse-tui): add e2e overlay tests"
```

---

## Task 17: E2E Tests — Setup Wizard

**Files:**
- Create: `simse-tui/tests/e2e/setup_wizard.rs`

10 tests: All 4 presets (Claude Code, Ollama, Copilot, Custom), Ollama 3-step wizard flow, back navigation, Custom command edit, Tab field toggle, empty command blocked.

Add to `mod.rs`: `mod setup_wizard;`

**Commit:**
```bash
git add simse-tui/tests/e2e/
git commit -m "test(simse-tui): add e2e setup wizard tests"
```

---

## Task 18: E2E Tests — Error States

**Files:**
- Create: `simse-tui/tests/e2e/error_states.rs`

4 tests: Invalid command format, empty submit, unknown command name, error display.

Add to `mod.rs`: `mod error_states;`

**Commit:**
```bash
git add simse-tui/tests/e2e/
git commit -m "test(simse-tui): add e2e error state tests"
```

---

## Task 19: E2E Tests — ACP Integration

**Files:**
- Create: `simse-tui/tests/e2e/acp_integration.rs`

8 tests total:
- 3 mock ACP tests (connect, prompt/response, server switch) — run by default
- 2 real Claude Code tests (connect, prompt roundtrip) — gated with `#[ignore]` + `SIMSE_E2E_CLAUDE=1`
- 2 real Ollama tests (connect, prompt roundtrip) — gated with `#[ignore]` + `SIMSE_E2E_OLLAMA=1`
- 1 ACP restart test

Real tests check for binary availability (`which claude` / `which ollama`) and skip gracefully.

Add to `mod.rs`: `mod acp_integration;`

**Commit:**
```bash
git add simse-tui/tests/e2e/
git commit -m "test(simse-tui): add e2e ACP integration tests"
```

---

## Task 20: Run Full Test Suite and Fix

Run the complete test suite across all three TUI crates and fix any failures.

**Step 1: Build the binary**

Run: `cd simse-tui && cargo build`
Expected: successful build

**Step 2: Run unit + integration tests**

Run: `cd simse-tui && cargo test 2>&1 | tail -20`
Expected: all pass

**Step 3: Run e2e tests (non-ignored)**

Run: `cd simse-tui && cargo test --test e2e 2>&1 | tail -30`
Expected: all non-ignored e2e tests pass

**Step 4: Fix any failures**

Debug and fix any test failures found.

**Step 5: Final commit**

```bash
git add -A
git commit -m "fix(simse-tui): fix test suite issues from full run"
```

---

## Test Matrix Summary

| File | Count | Coverage |
|------|-------|---------|
| `startup.rs` | 5 | Banner, tips, status bar, permission mode, version |
| `input.rs` | 8 | Typing, backspace, delete, arrows, paste, history, Ctrl+C, word-delete |
| `autocomplete.rs` | 7 | `/` triggers, live filter, Tab accept, Up/Down nav, Escape, single-match, aliases |
| `commands_meta.rs` | 9 | /help, /help cmd, /clear, /exit, /verbose, /plan, /context, /compact, /shortcuts |
| `commands_library.rs` | 7 | /add, /search, /recommend, /topics, /volumes, /get, /delete |
| `commands_session.rs` | 7 | /sessions, /resume, /rename, /server, /model, /mcp, /acp |
| `commands_config.rs` | 5 | /config, /settings, /init, /setup, /factory-reset |
| `commands_files.rs` | 5 | /files, /save, /validate, /discard, /diff |
| `commands_tools.rs` | 5 | /tools, /agents, /skills, /prompts, /chain |
| `overlays.rs` | 8 | Settings nav, Librarians nav, Setup nav, Shortcuts dismiss, Escape, focus |
| `setup_wizard.rs` | 10 | All 4 presets, Ollama wizard, back nav, Custom edit, Tab toggle, empty blocked |
| `error_states.rs` | 4 | Invalid cmd, empty submit, unknown cmd, error box |
| `acp_integration.rs` | 8 | Mock: connect/prompt/switch. Real: claude-code, ollama (ignored) |
| **Total** | **88** | |
