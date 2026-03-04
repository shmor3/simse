# Autocomplete, Overlay Focus, and E2E Test Suite — Design

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers-extended-cc:executing-plans to implement this plan task-by-task.

**Goal:** Fix autocomplete popup rendering, overlay keyboard focus, and build a comprehensive PTY-based e2e test suite.

**Architecture:** Three independent workstreams — inline autocomplete, screen-aware focus routing, and hybrid PTY e2e tests using ratatui-testlib + custom harness.

---

## Section 1: Inline Autocomplete Below Input

### Problem

The `autocomplete.rs` module has a complete `CommandAutocompleteState` with filtering, navigation, and rendering — but `view()` never calls it. The Tab handler uses a disconnected `PromptMode::Autocomplete` enum. Two parallel systems; neither renders suggestions.

### Design

**Behavior:**
- Typing `/` followed by characters shows matching commands in a list **below the input field** (terminal-style, not a popup box).
- Each keystroke filters the list live via `CommandAutocompleteState.update_matches()`.
- Up/Down arrows navigate the list when visible.
- Tab or Enter accepts the selection and fills the input.
- Escape or clearing the `/` prefix dismisses.
- Single match shows inline ghost text after the cursor (fish shell style).

**Rendering:**
- Plain text lines below input — no border/popup box.
- Selected item highlighted with cyan/bold.
- Format: `  /help        Show help information` (aligned command + description).
- Max 8 visible items; scrollable if more.

**Layout change in `view()`:**
- Current: `[chat_area | input | status_bar]` (3 chunks).
- When autocomplete active: `[chat_area | input | completions_list | status_bar]` (4 chunks). Chat area shrinks.

**State changes:**
- Add `autocomplete: CommandAutocompleteState` field to `App` struct.
- Remove `PromptMode` enum (replaced by `autocomplete.is_active()`).
- On `CharInput`/`Backspace`/`Paste`: call `autocomplete.update_matches()`.
- On `Tab`: if active → `accept()`, if not active + starts with `/` → `update_matches()`.
- On `Up`/`Down`: if `autocomplete.is_active()` → `move_up()`/`move_down()` instead of history.
- On `Escape`: if active → `deactivate()` instead of dismissing overlays.

**Rendering:**
- Replace `render_command_autocomplete()` popup with new inline render function.
- Call from `view()` when `autocomplete.is_active()`.

---

## Section 2: Overlay Focus Routing

### Problem

All key events go to the input field regardless of `app.screen`. Settings, Librarians, and Setup overlays have complete state machines (`move_up()`, `enter()`, `back()`, `type_char()`, etc.) that are never called from `update()`.

### Design

**Screen-aware dispatch in `update()`:**

Each input message handler gets a screen-context guard:

```
CharInput(c) → match screen:
  Settings → settings_state.type_char(c)
  Librarians → librarian_state.type_char(c)
  Setup → setup_state.type_char(c)
  Chat → (current input behavior)

HistoryUp (Up arrow) → match screen:
  Settings → settings_state.move_up()
  Librarians → librarian_state.move_up()
  Setup → setup_state.move_up()
  Chat → (current history behavior)

HistoryDown (Down arrow) → match screen:
  Settings → settings_state.move_down(item_count)
  Librarians → librarian_state.move_down()
  Setup → setup_state.move_down()
  Chat → (current history behavior)

Submit/Enter → match screen:
  Settings → settings_state.enter()
  Librarians → librarian_state.enter()
  Setup → setup_state.enter() → handle SetupAction
  Chat → (current submit behavior)

Backspace → match screen:
  Settings → settings_state.backspace()
  Librarians → librarian_state.backspace()
  Setup → setup_state.backspace()
  Chat → (current backspace behavior)

Escape → match screen:
  Settings → settings_state.back() → if dismiss signal → screen = Chat
  Librarians → librarian_state.back() → if dismiss signal → screen = Chat
  Setup → setup_state.back() → if dismiss signal → screen = Chat
  Chat → (current escape behavior)
```

**Cursor hiding:** When overlay is active, `render_input()` skips `frame.set_cursor_position()`.

**Affected overlays:** Settings, Librarians, Setup. Shortcuts overlay is read-only (any key dismisses — already works).

---

## Section 3: E2E Test Suite

### Problem

The old simse-code had 50+ PTY-based e2e tests (node-pty + @xterm/headless) deleted during the Rust migration. Current tests are unit/integration only — no real terminal I/O testing.

### Design

**Architecture:** Hybrid — `ratatui-testlib` as PTY foundation + custom `SimseTestHarness` wrapper.

**Dependencies (dev-only):**
- `ratatui-testlib = { version = "0.1", features = ["mvp"] }` — PTY + vt100 emulation
- `tempfile = "3"` — already present

**Mock ACP Server:** Small Rust binary (`tests/e2e/mock_acp_server.rs`):
- Implements ACP JSON-RPC over stdio
- `initialize` → `{ protocolVersion: 1, agentInfo: { name: "mock-agent" } }`
- `session/new` → returns session ID
- `session/prompt` → echoes back user message
- Configurable via CLI flags

**SimseTestHarness wrapper:**
- `SimseTestHarness::new()` — spawns `simse-tui` binary in PTY with isolated temp config
- `type_text(s)` — character-by-character input
- `submit(s)` — type + Enter
- `press_key(Key)` — Tab, Up, Down, Escape, etc.
- `wait_for_text(text, timeout)` — poll screen
- `wait_for_prompt()` — wait for `>` prompt
- `screen_text()` — full screen as string
- `assert_contains(text)` / `assert_not_contains(text)`

**Test config scaffold:**
- Generates isolated temp dirs with minimal config
- `none` mode: no ACP server (commands return errors we can assert on)
- `mock` mode: uses mock ACP server binary

**Real ACP tests:**
- Gated with `#[ignore]` — run with `cargo test -- --ignored`
- Also gated with env vars: `SIMSE_E2E_CLAUDE=1` / `SIMSE_E2E_OLLAMA=1`
- Check binary/service availability, skip gracefully if not present

### Test Matrix (88 tests)

| File | Count | Coverage |
|------|-------|----------|
| `startup.rs` | 5 | Banner, tips, status bar, permission mode, onboarding |
| `input.rs` | 8 | Typing, backspace, delete, arrows, paste, history, Ctrl+C, word-delete |
| `autocomplete.rs` | 7 | `/` triggers, live filter, Tab accept, Up/Down nav, Escape, single-match, aliases |
| `commands_meta.rs` | 9 | /help, /help cmd, /clear, /exit, /verbose, /plan, /context, /compact, /shortcuts |
| `commands_library.rs` | 7 | /add, /search, /recommend, /topics, /volumes, /get, /delete |
| `commands_session.rs` | 7 | /sessions, /resume, /rename, /server, /model, /mcp, /acp |
| `commands_config.rs` | 5 | /config, /settings, /init, /setup, /factory-reset |
| `commands_files.rs` | 5 | /files, /save, /validate, /discard, /diff |
| `commands_tools.rs` | 5 | /tools, /agents, /skills, /prompts, /chain |
| `overlays.rs` | 8 | Settings nav, Librarians nav, Setup nav, Shortcuts dismiss, Escape, focus transfer |
| `setup_wizard.rs` | 10 | All 4 presets, Ollama 3-step wizard, back nav, Custom edit, Tab toggle, empty blocked |
| `error_states.rs` | 4 | Invalid cmd, empty submit, unknown cmd, error box |
| `acp_integration.rs` | 8 | Mock: connect/prompt/response/switch/restart. Real: claude-code, ollama (ignored by default) |
| **Total** | **88** | |

### Project Structure

```
simse-tui/
  tests/
    e2e/
      mod.rs              # Module declarations
      harness.rs          # SimseTestHarness wrapper
      config.rs           # Test config scaffold
      mock_acp_server.rs  # Mock ACP binary
      startup.rs
      input.rs
      autocomplete.rs
      commands_meta.rs
      commands_library.rs
      commands_session.rs
      commands_config.rs
      commands_files.rs
      commands_tools.rs
      overlays.rs
      setup_wizard.rs
      error_states.rs
      acp_integration.rs
```

---

## Sources

- [ratatui-testlib](https://github.com/raibid-labs/ratatui-testlib) — PTY-based TUI testing library
- [ratatui-testlib docs](https://docs.rs/ratatui-testlib) — API documentation
- [ratatui-testlib on lib.rs](https://lib.rs/crates/ratatui-testlib) — v0.1.0, uses portable-pty + vt100
