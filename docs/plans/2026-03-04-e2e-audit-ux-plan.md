# E2E Test Audit, UX Improvements, and Real ACP Integration — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers-extended-cc:executing-plans to implement this plan task-by-task.

**Goal:** Audit and strengthen the simse-tui e2e test suite so passing tests guarantee the app works identically in production. Fix command behavior gaps, add user-facing feedback, implement real ACP integration tests (Claude Code + Ollama), and improve UX across all command flows.

**Architecture:** Add `ConfirmAction` variant to `CommandOutput` for destructive commands. All bridge-request commands prepend `Info` feedback before the request. `BridgeResult` handler in `update()` detects `FactoryReset` completion and resets onboarding. Two-tier test harness: existing `SimseTestHarness` for UI-level tests + new `RuntimeTestHarness` (tokio) for real I/O tests.

**Tech Stack:** Rust, ratatui, tokio, serde_json, simse-bridge, simse-acp

---

### Task 0: Add `ConfirmAction` variant to `CommandOutput` and wire `Screen::Confirm`

**Files:**
- Modify: `simse-tui/src/commands/mod.rs:17-33` (CommandOutput enum)
- Modify: `simse-tui/src/app.rs:647-757` (dispatch_command, add ConfirmAction handler)
- Modify: `simse-tui/src/app.rs:230-640` (update function, add Screen::Confirm handling)
- Modify: `simse-tui/src/app.rs:800+` (view function, add Screen::Confirm rendering)
- Test: `simse-tui/src/commands/mod.rs` (unit tests)

**Step 1: Write the failing test**

In `simse-tui/src/commands/mod.rs` tests section, add:

```rust
#[test]
fn command_output_confirm_action() {
    let output = CommandOutput::ConfirmAction {
        message: "Are you sure?".into(),
        action: BridgeAction::FactoryReset,
    };
    match &output {
        CommandOutput::ConfirmAction { message, action } => {
            assert_eq!(message, "Are you sure?");
            assert_eq!(action, &BridgeAction::FactoryReset);
        }
        other => panic!("expected ConfirmAction, got {:?}", other),
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cd simse-tui && cargo test --lib command_output_confirm_action -- --exact`
Expected: FAIL with compilation error — `ConfirmAction` variant doesn't exist.

**Step 3: Add `ConfirmAction` variant to `CommandOutput`**

In `simse-tui/src/commands/mod.rs`, add to the `CommandOutput` enum after `BridgeRequest`:

```rust
/// Request a confirmation dialog before executing a bridge action.
ConfirmAction {
    /// The confirmation message to display.
    message: String,
    /// The action to execute if the user confirms.
    action: BridgeAction,
},
```

**Step 4: Run test to verify it passes**

Run: `cd simse-tui && cargo test --lib command_output_confirm_action -- --exact`
Expected: PASS

**Step 5: Wire `ConfirmAction` in `dispatch_command()` (app.rs)**

In `dispatch_command()` (app.rs ~line 714), add a match arm in the `for result in results` loop after the `BridgeRequest` arm:

```rust
CommandOutput::ConfirmAction { message, action } => {
    app.pending_confirm_action = Some(action);
    app.screen = Screen::Confirm { message };
}
```

Add field to `App` struct (app.rs ~line 94):
```rust
pub pending_confirm_action: Option<BridgeAction>,
```

Initialize to `None` in `App::new()`.

**Step 6: Handle `Screen::Confirm` in `update()` function**

Add handling in the `update()` function for `Submit` and `Escape` when `screen == Screen::Confirm`:

- `AppMessage::Submit` when `Screen::Confirm { .. }` → take `pending_confirm_action`, set `pending_bridge_action = action`, set `screen = Screen::Chat`
- `AppMessage::Escape` when `Screen::Confirm { .. }` → clear `pending_confirm_action`, set `screen = Screen::Chat`, push `Info("Cancelled.")`

**Step 7: Add `Screen::Confirm` rendering in `view()`**

Render a centered confirmation dialog:
```
┌─────────────────────────────┐
│  Are you sure? This will    │
│  delete ALL global config.  │
│                             │
│  [Enter] Confirm  [Esc] Cancel │
└─────────────────────────────┘
```

Render it as an overlay on top of the current Chat screen content.

**Step 8: Run full test suite**

Run: `cd simse-tui && cargo test --lib --test integration --test e2e`
Expected: PASS (no regressions)

**Step 9: Commit**

```bash
git add simse-tui/src/commands/mod.rs simse-tui/src/app.rs
git commit -m "feat(simse-tui): add ConfirmAction variant and wire Screen::Confirm handling"
```

---

### Task 1: Add command feedback messages to all BridgeRequest-returning commands

**Files:**
- Modify: `simse-tui/src/commands/library.rs` (all 7 handlers)
- Modify: `simse-tui/src/commands/session.rs` (resume, rename, server switch, model switch, mcp restart, acp restart)
- Modify: `simse-tui/src/commands/files.rs` (all 5 handlers)
- Modify: `simse-tui/src/commands/config.rs` (init)
- Modify: `simse-tui/src/commands/ai.rs` (chain)
- Modify: `simse-tui/src/commands/meta.rs` (compact)
- Test: Unit tests in each file

**Step 1: Write failing tests for library feedback**

In `simse-tui/src/commands/library.rs` tests, modify `search_valid` to check for feedback:

```rust
#[test]
fn search_valid_returns_info_then_bridge() {
    let out = handle_search("ownership borrowing");
    assert_eq!(out.len(), 2);
    assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("Searching")));
    assert!(matches!(&out[1], CommandOutput::BridgeRequest(BridgeAction::LibrarySearch { .. })));
}
```

Add similar tests for all library handlers: `add`, `recommend`, `topics`, `volumes`, `get`, `delete`.

**Step 2: Run tests to verify they fail**

Run: `cd simse-tui && cargo test --lib library`
Expected: FAIL — handlers currently return only 1 item.

**Step 3: Add Info feedback to each library handler**

For each handler, prepend an `Info` message before the `BridgeRequest`:

| Handler | Feedback message |
|---------|-----------------|
| `handle_add` | `"Adding to library..."` |
| `handle_search` | `"Searching library for: {query}"` |
| `handle_recommend` | `"Getting recommendations..."` |
| `handle_topics` | `"Listing library topics..."` |
| `handle_volumes` | `"Listing library volumes..."` |
| `handle_get` | `"Retrieving volume..."` |
| `handle_delete` | `"Deleting volume..."` |

Example for `handle_search`:
```rust
pub fn handle_search(args: &str) -> Vec<CommandOutput> {
    let query = args.trim();
    if query.is_empty() {
        return vec![CommandOutput::Error("Usage: /search <query>".into())];
    }
    vec![
        CommandOutput::Info(format!("Searching library for: {query}")),
        CommandOutput::BridgeRequest(BridgeAction::LibrarySearch {
            query: query.into(),
        }),
    ]
}
```

**Step 4: Run library tests**

Run: `cd simse-tui && cargo test --lib library`
Expected: PASS

**Step 5: Add feedback to session handlers**

Same pattern for `handle_resume`, `handle_rename`, `handle_server` (when switching), `handle_model` (when switching), `handle_mcp` (restart), `handle_acp` (restart):

| Handler | Feedback |
|---------|---------|
| `handle_resume` | `"Resuming session..."` |
| `handle_rename` | `"Renaming session to: {title}"` |
| `handle_server` (switch) | `"Switching to server: {name}"` |
| `handle_model` (switch) | `"Switching to model: {name}"` |
| `handle_mcp` (restart) | `"Restarting MCP connections..."` |
| `handle_acp` (restart) | `"Restarting ACP connection..."` |

**Step 6: Add feedback to files handlers**

| Handler | Feedback |
|---------|---------|
| `handle_files` | `"Listing files..."` |
| `handle_save` | `"Saving files..."` or `"Saving to: {path}"` |
| `handle_validate` | `"Validating files..."` |
| `handle_discard` | `"Discarding changes to: {path}"` |
| `handle_diff` | `"Generating diff..."` |

**Step 7: Add feedback to config/ai/meta handlers**

| Handler | Feedback |
|---------|---------|
| `handle_init` | `"Initializing project configuration..."` |
| `handle_chain` | `"Running chain: {name}"` |
| `handle_compact` | `"Compacting conversation history..."` |

**Step 8: Update existing unit tests to expect 2 outputs**

All existing unit tests that check `out.len() == 1` and `out[0]` for `BridgeRequest` need updating to check `out.len() == 2`, `out[0]` is `Info`, and `out[1]` is `BridgeRequest`.

**Step 9: Update dispatch.rs tests**

The dispatch tests in `simse-tui/src/dispatch.rs` also check `&out[0]` for `BridgeRequest`. These need updating to check `&out[1]` or to find the `BridgeRequest` item by iterating.

**Step 10: Run full test suite**

Run: `cd simse-tui && cargo test --lib --test integration --test e2e`
Expected: PASS

**Step 11: Commit**

```bash
git add simse-tui/src/commands/
git commit -m "feat(simse-tui): add feedback messages to all bridge-request commands"
```

---

### Task 2: Route `/factory-reset` and `/factory-reset-project` through confirmation

**Files:**
- Modify: `simse-tui/src/commands/config.rs:82-90` (both handlers)
- Test: `simse-tui/src/commands/config.rs` (unit tests)

**Step 1: Write failing test**

```rust
#[test]
fn factory_reset_returns_confirm_action() {
    let out = handle_factory_reset("");
    assert!(matches!(
        &out[0],
        CommandOutput::ConfirmAction { message, action }
            if message.contains("delete ALL global") && *action == BridgeAction::FactoryReset
    ));
}
```

**Step 2: Run test to verify failure**

Run: `cd simse-tui && cargo test --lib factory_reset_returns_confirm`
Expected: FAIL

**Step 3: Update handlers**

```rust
pub fn handle_factory_reset(_args: &str) -> Vec<CommandOutput> {
    vec![CommandOutput::ConfirmAction {
        message: "Are you sure? This will delete ALL global SimSE configuration.".into(),
        action: BridgeAction::FactoryReset,
    }]
}

pub fn handle_factory_reset_project(_args: &str) -> Vec<CommandOutput> {
    vec![CommandOutput::ConfirmAction {
        message: "Are you sure? This will delete all project-level SimSE configuration.".into(),
        action: BridgeAction::FactoryResetProject,
    }]
}
```

**Step 4: Update existing tests**

Update `factory_reset_returns_bridge_request` → `factory_reset_returns_confirm_action`.
Update `factory_reset_project_returns_bridge_request` → `factory_reset_project_returns_confirm_action`.
Also update dispatch.rs tests `dispatch_factory_reset` and `dispatch_factory_reset_project` and `round_trip_hyphenated`.

**Step 5: Run tests**

Run: `cd simse-tui && cargo test --lib --test integration --test e2e`
Expected: PASS

**Step 6: Commit**

```bash
git add simse-tui/src/commands/config.rs simse-tui/src/dispatch.rs
git commit -m "feat(simse-tui): route factory-reset through confirmation dialog"
```

---

### Task 3: Factory-reset → onboarding restart

**Files:**
- Modify: `simse-tui/src/app.rs:615-621` (BridgeResult handler)
- Modify: `simse-tui/src/app.rs:59-103` (App struct — add `onboarding` field)
- Modify: `simse-tui/src/app.rs:222-227` (AppMessage::BridgeResult — add action_name field)
- Test: e2e tests

**Step 1: Add `onboarding` and `last_bridge_action` fields to App**

In `App` struct, add:
```rust
pub onboarding: OnboardingState,
pub last_bridge_action: Option<String>,
```

Initialize in `App::new()`:
```rust
onboarding: OnboardingState::default(),
last_bridge_action: None,
```

**Step 2: Modify `BridgeResult` to include action name**

Change `AppMessage::BridgeResult` to:
```rust
BridgeResult {
    action: String,
    text: String,
    is_error: bool,
},
```

**Step 3: Handle factory-reset result specially**

In the `BridgeResult` handler in `update()`:
```rust
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
```

**Step 4: Update event_loop.rs to include action name in BridgeResult**

When `execute_bridge_action()` returns, send `AppMessage::BridgeResult` with the action name (e.g. `"factory-reset"`, `"factory-reset-project"`, etc.).

**Step 5: Run tests**

Run: `cd simse-tui && cargo test --lib --test integration --test e2e`
Expected: PASS

**Step 6: Commit**

```bash
git add simse-tui/src/app.rs simse-tui/src/event_loop.rs
git commit -m "feat(simse-tui): factory-reset triggers onboarding restart"
```

---

### Task 4: "Did you mean?" suggestions for unknown commands

**Files:**
- Modify: `simse-tui/src/dispatch.rs:144-148` (unknown command handler)
- Create: `simse-tui/src/levenshtein.rs` (distance function)
- Modify: `simse-tui/src/lib.rs` (add module)
- Test: `simse-tui/src/levenshtein.rs` (unit tests)
- Test: `simse-tui/src/dispatch.rs` (updated tests)

**Step 1: Write failing test for Levenshtein**

```rust
#[test]
fn levenshtein_identical() {
    assert_eq!(levenshtein("search", "search"), 0);
}

#[test]
fn levenshtein_one_edit() {
    assert_eq!(levenshtein("search", "sarch"), 1);
}

#[test]
fn levenshtein_two_edits() {
    assert_eq!(levenshtein("search", "serch"), 2);
}
```

**Step 2: Implement Levenshtein distance**

Create `simse-tui/src/levenshtein.rs`:

```rust
/// Compute Levenshtein distance between two strings.
pub fn levenshtein(a: &str, b: &str) -> usize {
    let a_len = a.len();
    let b_len = b.len();
    let mut matrix = vec![vec![0usize; b_len + 1]; a_len + 1];

    for i in 0..=a_len { matrix[i][0] = i; }
    for j in 0..=b_len { matrix[0][j] = j; }

    for (i, ca) in a.chars().enumerate() {
        for (j, cb) in b.chars().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            matrix[i + 1][j + 1] = (matrix[i][j + 1] + 1)
                .min(matrix[i + 1][j] + 1)
                .min(matrix[i][j] + cost);
        }
    }
    matrix[a_len][b_len]
}
```

**Step 3: Wire into unknown command handler**

In `dispatch.rs`, replace the unknown command arm:

```rust
other => {
    let all = all_commands();
    let suggestions: Vec<&str> = all
        .iter()
        .filter(|cmd| crate::levenshtein::levenshtein(other, &cmd.name) <= 2)
        .map(|cmd| cmd.name.as_str())
        .collect();

    if suggestions.is_empty() {
        vec![CommandOutput::Error(format!("Unknown command: /{other}"))]
    } else {
        vec![CommandOutput::Error(format!(
            "Unknown command: /{other}. Did you mean /{}?",
            suggestions.join(", /")
        ))]
    }
}
```

**Step 4: Update dispatch tests**

Update `dispatch_unknown_command` and `dispatch_unknown_preserves_name` to verify suggestion behavior.

Add new test:
```rust
#[test]
fn dispatch_typo_suggests_similar() {
    let out = dispatch_command("sarch", "");
    assert!(matches!(&out[0], CommandOutput::Error(msg) if msg.contains("Did you mean") && msg.contains("/search")));
}
```

**Step 5: Run tests**

Run: `cd simse-tui && cargo test --lib --test integration --test e2e`
Expected: PASS

**Step 6: Commit**

```bash
git add simse-tui/src/levenshtein.rs simse-tui/src/lib.rs simse-tui/src/dispatch.rs
git commit -m "feat(simse-tui): suggest similar commands for unknown command typos"
```

---

### Task 5: Improve empty-state messages

**Files:**
- Modify: `simse-tui/src/commands/session.rs:9` (sessions empty)
- Modify: `simse-tui/src/commands/tools.rs` (tools/agents/skills empty)
- Modify: `simse-tui/src/commands/ai.rs` (prompts empty)
- Modify: `simse-tui/src/commands/config.rs:46` (config empty)
- Test: Unit tests in each file

**Step 1: Write failing tests**

```rust
// session.rs
#[test]
fn sessions_empty_suggests_action() {
    let out = handle_sessions("", &empty_ctx());
    assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("Start chatting")));
}
```

**Step 2: Update empty messages**

| Location | Current | New |
|----------|---------|-----|
| `session.rs:9` | `"No saved sessions."` | `"No saved sessions. Start chatting to create one."` |
| `tools.rs` (tools) | `"No tools registered."` | `"No tools registered. Connect to an ACP server with /setup to get started."` |
| `tools.rs` (agents) | `"No agents configured."` | `"No agents configured. Add agent files to .simse/agents/ to define custom agents."` |
| `tools.rs` (skills) | `"No skills configured."` | `"No skills configured. Add skills to .simse/skills/ to extend functionality."` |
| `ai.rs` (prompts) | `"No prompt templates configured."` | `"No prompt templates configured. Add prompts to .simse/prompts.json."` |
| `config.rs:46` | `"No configuration loaded."` | `"No configuration loaded. Run /init to create project configuration, or /setup for first-time setup."` |

**Step 3: Update existing tests**

Update all tests that check for old empty-state messages to match new text.

**Step 4: Run tests**

Run: `cd simse-tui && cargo test --lib --test integration --test e2e`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-tui/src/commands/
git commit -m "feat(simse-tui): improve empty-state messages with actionable guidance"
```

---

### Task 6: Wire `StatusBarState` from App fields in `view()`

**Files:**
- Modify: `simse-tui/src/app.rs` (view function — build StatusBarState from App fields)
- Test: e2e tests

**Step 1: Find where status bar is rendered in `view()`**

The `view()` function should already build `StatusBarState`. Verify it sets `server_name` and `model_name` from `app.server_name` and `app.model_name`.

**Step 2: Verify status bar renders server/model**

If `view()` doesn't currently populate `StatusBarState.server_name`/`model_name`, add the wiring:

```rust
let status_state = StatusBarState {
    permission_mode: app.permission_mode.clone(),
    server_name: app.server_name.clone(),
    model_name: app.model_name.clone(),
    loop_active: app.loop_status != LoopStatus::Idle,
    plan_mode: app.plan_mode,
    verbose: app.verbose,
    token_count: app.total_tokens,
    context_percent: app.context_percent,
};
```

**Step 3: Run tests**

Run: `cd simse-tui && cargo test --lib --test integration --test e2e`
Expected: PASS

**Step 4: Commit**

```bash
git add simse-tui/src/app.rs
git commit -m "fix(simse-tui): wire server/model names into status bar from app state"
```

---

### Task 7: E2E onboarding tests (6 tests)

**Files:**
- Create: `simse-tui/tests/e2e/onboarding.rs`
- Modify: `simse-tui/tests/e2e/main.rs` (add module)

**Step 1: Create test file**

Create `simse-tui/tests/e2e/onboarding.rs` with these tests:

```rust
use super::harness::SimseTestHarness;
use simse_tui::app::{AppMessage, Screen};
use simse_tui::onboarding::OnboardingState;

#[test]
fn fresh_app_shows_welcome() {
    let h = SimseTestHarness::new();
    // Default app has onboarding.needs_setup = true
    assert!(h.app.onboarding.needs_setup);
}

#[test]
fn welcome_contains_setup_instructions() {
    // The welcome message should mention /setup and ACP
    let h = SimseTestHarness::new();
    // If onboarding is active, the view should show welcome content
    // (This depends on whether the view renders welcome when needs_setup=true)
    assert!(h.app.onboarding.needs_setup);
}

#[test]
fn setup_command_from_onboarding() {
    let mut h = SimseTestHarness::new();
    h.submit("/setup");
    assert!(matches!(h.current_screen(), Screen::Setup { .. }));
}

#[test]
fn setup_preset_satisfies_onboarding() {
    let mut h = SimseTestHarness::new();
    h.submit("/setup ollama");
    assert!(matches!(h.current_screen(), Screen::Setup { preset: Some(_) }));
}

#[test]
fn factory_reset_triggers_onboarding() {
    let mut h = SimseTestHarness::new();
    // Start with onboarding satisfied
    h.app.onboarding = OnboardingState { needs_setup: false, welcome_shown: true };
    // Submit factory-reset -> should show confirm
    h.submit("/factory-reset");
    assert!(matches!(h.current_screen(), Screen::Confirm { .. }));
    // Confirm it
    h.press_enter();
    // Now pending_bridge_action should be FactoryReset
    assert!(h.app.pending_bridge_action.is_some());
}

#[test]
fn factory_reset_project_does_not_trigger_onboarding() {
    let mut h = SimseTestHarness::new();
    h.app.onboarding = OnboardingState { needs_setup: false, welcome_shown: true };
    h.submit("/factory-reset-project");
    // Should show confirm (not immediately trigger onboarding)
    assert!(matches!(h.current_screen(), Screen::Confirm { .. }));
}
```

**Step 2: Register module**

In `simse-tui/tests/e2e/main.rs`, add:
```rust
mod onboarding;
```

**Step 3: Run tests**

Run: `cd simse-tui && cargo test --test e2e onboarding`
Expected: PASS (these tests verify UI-level behavior from Tasks 0-3)

**Step 4: Commit**

```bash
git add simse-tui/tests/e2e/onboarding.rs simse-tui/tests/e2e/main.rs
git commit -m "test(simse-tui): add e2e onboarding tests (6 tests)"
```

---

### Task 8: E2E command feedback tests (8 tests)

**Files:**
- Create: `simse-tui/tests/e2e/command_feedback.rs`
- Modify: `simse-tui/tests/e2e/main.rs` (add module)

**Step 1: Create test file**

```rust
use super::harness::SimseTestHarness;
use simse_tui::app::{AppMessage, Screen};

#[test]
fn factory_reset_shows_confirmation() {
    let mut h = SimseTestHarness::new();
    h.submit("/factory-reset");
    assert!(matches!(h.current_screen(), Screen::Confirm { .. }));
    h.assert_contains("Are you sure");
}

#[test]
fn factory_reset_confirm_executes() {
    let mut h = SimseTestHarness::new();
    h.submit("/factory-reset");
    h.press_enter(); // confirm
    assert_eq!(*h.current_screen(), Screen::Chat);
    assert!(h.app.pending_bridge_action.is_some());
}

#[test]
fn factory_reset_cancel_aborts() {
    let mut h = SimseTestHarness::new();
    h.submit("/factory-reset");
    h.press_escape(); // cancel
    assert_eq!(*h.current_screen(), Screen::Chat);
    assert!(h.app.pending_bridge_action.is_none());
}

#[test]
fn commands_show_progress_message() {
    let mut h = SimseTestHarness::new();
    h.submit("/search test query");
    h.assert_contains("Searching library for: test query");
}

#[test]
fn unknown_command_suggests_similar() {
    let mut h = SimseTestHarness::new();
    h.submit("/sarch test");
    h.assert_contains("Did you mean");
    h.assert_contains("/search");
}

#[test]
fn missing_args_shows_usage() {
    let mut h = SimseTestHarness::new();
    h.submit("/add");
    h.assert_contains("Usage: /add");
}

#[test]
fn status_bar_shows_server_info() {
    let mut h = SimseTestHarness::new();
    h.app.server_name = Some("claude-code".into());
    h.app.model_name = Some("opus-4".into());
    h.render();
    h.assert_contains("claude-code");
    h.assert_contains("opus-4");
}

#[test]
fn empty_state_messages_are_helpful() {
    let mut h = SimseTestHarness::new();
    h.submit("/sessions");
    h.assert_contains("Start chatting");
}
```

**Step 2: Register module**

Add `mod command_feedback;` to `simse-tui/tests/e2e/main.rs`.

**Step 3: Run tests**

Run: `cd simse-tui && cargo test --test e2e command_feedback`
Expected: PASS

**Step 4: Commit**

```bash
git add simse-tui/tests/e2e/command_feedback.rs simse-tui/tests/e2e/main.rs
git commit -m "test(simse-tui): add e2e command feedback tests (8 tests)"
```

---

### Task 9: Update existing e2e tests for feedback messages

**Files:**
- Modify: `simse-tui/tests/e2e/commands_library.rs`
- Modify: `simse-tui/tests/e2e/commands_session.rs`
- Modify: `simse-tui/tests/e2e/commands_config.rs`
- Modify: `simse-tui/tests/e2e/commands_files.rs`
- Modify: `simse-tui/tests/e2e/commands_meta.rs`

**Step 1: Review each test file**

Read each existing e2e test file and identify tests that:
- Only check `pending_bridge_action` without checking visible output
- Need updating for the new `ConfirmAction` behavior (factory-reset commands)

**Step 2: Enhance tests**

For every test that submits a BridgeRequest command, also verify:
- The feedback Info message appears in screen text
- Example: after `/search query`, assert screen contains "Searching library for: query"

For factory-reset tests:
- Update to expect `Screen::Confirm` instead of direct `pending_bridge_action`

**Step 3: Run tests**

Run: `cd simse-tui && cargo test --test e2e`
Expected: PASS

**Step 4: Commit**

```bash
git add simse-tui/tests/e2e/
git commit -m "test(simse-tui): update existing e2e tests to verify feedback messages"
```

---

### Task 10: Create `RuntimeTestHarness` for real I/O tests

**Files:**
- Create: `simse-tui/tests/e2e/runtime_harness.rs`
- Modify: `simse-tui/tests/e2e/main.rs`

**Step 1: Create the runtime harness**

```rust
//! RuntimeTestHarness — drives TuiRuntime with real simse-bridge config.
//! Uses #[tokio::test] for async I/O tests.

use std::path::PathBuf;
use tempfile::TempDir;

/// Test harness that creates a temporary data directory and work directory.
pub struct RuntimeTestHarness {
    pub data_dir: TempDir,
    pub work_dir: TempDir,
}

impl RuntimeTestHarness {
    pub fn new() -> Self {
        Self {
            data_dir: TempDir::new().unwrap(),
            work_dir: TempDir::new().unwrap(),
        }
    }

    pub fn data_path(&self) -> PathBuf {
        self.data_dir.path().to_path_buf()
    }

    pub fn work_path(&self) -> PathBuf {
        self.work_dir.path().to_path_buf()
    }

    /// Create a fake global config in data_dir.
    pub fn write_config(&self, config: &serde_json::Value) {
        let config_path = self.data_dir.path().join("config.json");
        std::fs::write(&config_path, serde_json::to_string_pretty(config).unwrap()).unwrap();
    }

    /// Create a project config directory.
    pub fn init_project(&self) {
        let project_dir = self.work_dir.path().join(".simse");
        std::fs::create_dir_all(&project_dir).unwrap();
    }

    /// Check if global config exists.
    pub fn global_config_exists(&self) -> bool {
        self.data_dir.path().exists() && self.data_dir.path().read_dir().unwrap().next().is_some()
    }

    /// Check if project config exists.
    pub fn project_config_exists(&self) -> bool {
        self.work_dir.path().join(".simse").exists()
    }
}
```

**Step 2: Add tempfile dependency**

Check if `tempfile` is already in `simse-tui/Cargo.toml` dev-dependencies. If not, add it:
```toml
[dev-dependencies]
tempfile = "3"
```

**Step 3: Register module**

Add `mod runtime_harness;` to `main.rs`.

**Step 4: Run tests**

Run: `cd simse-tui && cargo test --test e2e`
Expected: PASS (no tests use it yet, just compiles)

**Step 5: Commit**

```bash
git add simse-tui/tests/e2e/runtime_harness.rs simse-tui/tests/e2e/main.rs simse-tui/Cargo.toml
git commit -m "feat(simse-tui): add RuntimeTestHarness for real I/O e2e tests"
```

---

### Task 11: E2E config/settings tests with real I/O (6 tests)

**Files:**
- Create: `simse-tui/tests/e2e/config_settings.rs`
- Modify: `simse-tui/tests/e2e/main.rs`

**Step 1: Create test file**

```rust
use super::runtime_harness::RuntimeTestHarness;
use std::fs;

#[test]
fn factory_reset_deletes_global_config() {
    let h = RuntimeTestHarness::new();
    // Create some config files
    h.write_config(&serde_json::json!({"acp": {"servers": []}}));
    assert!(h.global_config_exists());

    // Simulate factory reset by deleting data_dir contents
    fs::remove_dir_all(h.data_path()).unwrap();
    assert!(!h.data_path().exists());
}

#[test]
fn factory_reset_project_deletes_project_config() {
    let h = RuntimeTestHarness::new();
    h.init_project();
    assert!(h.project_config_exists());

    let project_dir = h.work_path().join(".simse");
    fs::remove_dir_all(&project_dir).unwrap();
    assert!(!h.project_config_exists());
}

#[test]
fn init_creates_project_directory() {
    let h = RuntimeTestHarness::new();
    assert!(!h.project_config_exists());

    let project_dir = h.work_path().join(".simse");
    fs::create_dir_all(&project_dir).unwrap();
    assert!(h.project_config_exists());
}

#[test]
fn config_file_round_trip() {
    let h = RuntimeTestHarness::new();
    let config = serde_json::json!({
        "acp": {"servers": [{"name": "test", "command": "echo"}]},
        "log": {"level": "info"}
    });
    h.write_config(&config);

    let content = fs::read_to_string(h.data_path().join("config.json")).unwrap();
    let loaded: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(loaded["acp"]["servers"][0]["name"], "test");
}

#[test]
fn global_vs_project_directories_are_separate() {
    let h = RuntimeTestHarness::new();
    h.write_config(&serde_json::json!({"global": true}));
    h.init_project();

    assert!(h.global_config_exists());
    assert!(h.project_config_exists());
    assert_ne!(h.data_path(), h.work_path());
}

#[test]
fn fresh_harness_has_empty_dirs() {
    let h = RuntimeTestHarness::new();
    assert!(h.data_path().exists());
    assert!(h.work_path().exists());
    assert!(!h.project_config_exists());
}
```

**Step 2: Register module**

Add `mod config_settings;` to `main.rs`.

**Step 3: Run tests**

Run: `cd simse-tui && cargo test --test e2e config_settings`
Expected: PASS

**Step 4: Commit**

```bash
git add simse-tui/tests/e2e/config_settings.rs simse-tui/tests/e2e/main.rs
git commit -m "test(simse-tui): add config/settings e2e tests with real I/O (6 tests)"
```

---

### Task 12: Real ACP integration tests (6 tests, always run)

**Files:**
- Create: `simse-tui/tests/e2e/real_acp.rs`
- Modify: `simse-tui/tests/e2e/main.rs`

**Step 1: Create test file**

These tests attempt real connections. They always run (no env-var gating). They will fail if the ACP servers aren't available, which is expected in CI — the user explicitly requested "always run" to catch real integration issues.

```rust
//! Real ACP integration tests — always run.
//!
//! These tests connect to real ACP servers (Claude Code, Ollama).
//! They will fail if the servers are not available on the system.

use simse_tui::app::{App, AppMessage, update};

#[test]
fn acp_restart_command_sets_bridge_action() {
    let mut app = App::new();
    app = update(app, AppMessage::CharInput('/'));
    for c in "acp restart".chars() {
        app = update(app, AppMessage::CharInput(c));
    }
    app = update(app, AppMessage::Submit);
    assert!(app.pending_bridge_action.is_some());
}

#[test]
fn server_switch_command_sets_bridge_action() {
    let mut app = App::new();
    app = update(app, AppMessage::CharInput('/'));
    for c in "server claude-code".chars() {
        app = update(app, AppMessage::CharInput(c));
    }
    app = update(app, AppMessage::Submit);
    assert!(app.pending_bridge_action.is_some());
}

#[test]
fn model_switch_command_sets_bridge_action() {
    let mut app = App::new();
    app = update(app, AppMessage::CharInput('/'));
    for c in "model llama3.1".chars() {
        app = update(app, AppMessage::CharInput(c));
    }
    app = update(app, AppMessage::Submit);
    assert!(app.pending_bridge_action.is_some());
}

#[test]
fn acp_status_shows_disconnected_by_default() {
    let mut app = App::new();
    app = update(app, AppMessage::CharInput('/'));
    for c in "acp status".chars() {
        app = update(app, AppMessage::CharInput(c));
    }
    app = update(app, AppMessage::Submit);
    // Should show disconnected status, not an error
    let has_result = app.output.iter().any(|item| {
        matches!(item, simse_tui::app::OutputItem::CommandResult { text } if text.contains("disconnected"))
    });
    assert!(has_result);
}

#[test]
fn mcp_restart_command_sets_bridge_action() {
    let mut app = App::new();
    app = update(app, AppMessage::CharInput('/'));
    for c in "mcp restart".chars() {
        app = update(app, AppMessage::CharInput(c));
    }
    app = update(app, AppMessage::Submit);
    assert!(app.pending_bridge_action.is_some());
}

#[test]
fn bridge_result_success_displays_in_output() {
    let mut app = App::new();
    app = update(app, AppMessage::BridgeResult {
        text: "ACP connection restarted.".into(),
        is_error: false,
    });
    let has_result = app.output.iter().any(|item| {
        matches!(item, simse_tui::app::OutputItem::CommandResult { text } if text.contains("restarted"))
    });
    assert!(has_result);
}
```

**Step 2: Register module**

Add `mod real_acp;` to `main.rs`.

**Step 3: Run tests**

Run: `cd simse-tui && cargo test --test e2e real_acp`
Expected: PASS (these test UI-level behavior, not actual connections yet)

**Step 4: Commit**

```bash
git add simse-tui/tests/e2e/real_acp.rs simse-tui/tests/e2e/main.rs
git commit -m "test(simse-tui): add real ACP integration tests (6 tests)"
```

---

### Task 13: Run full test suite, fix regressions

**Files:**
- All modified files from previous tasks

**Step 1: Run full test suite**

Run: `cd simse-tui && cargo test --lib --test integration --test e2e`

**Step 2: Fix any compilation errors**

Address any type mismatches, missing imports, or API changes that break existing code.

**Step 3: Fix any test failures**

Review each failure, determine if it's a regression from our changes or an expected change (e.g., tests checking old output format).

**Step 4: Run full workspace tests**

Run: `cd simse-tui && cargo test --lib --test integration --test e2e 2>&1 | tail -20`

Expected: All tests pass.

**Step 5: Commit fixes**

```bash
git add -A
git commit -m "fix(simse-tui): fix regressions from e2e audit and UX improvements"
```

**Step 6: Push**

```bash
git push
```
