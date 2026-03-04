# E2E Test Audit, UX Improvements, and Real ACP Integration — Design

## Goal

Audit and strengthen the simse-tui e2e test suite so that passing tests guarantee the app works identically in production. Fix command behavior gaps, add user-facing feedback, implement real ACP integration tests (Claude Code + Ollama), and improve UX across all command flows.

## Audit Findings

### Critical Gaps

1. **Tests verify enum variants, not behavior.** Commands like `/factory-reset` check `app.pending_bridge_action == BridgeAction::FactoryReset` but never verify files are deleted.
2. **Factory-reset doesn't restart onboarding.** After deleting `data_dir`, the app continues as if configured.
3. **No onboarding e2e tests.** The first-run flow is completely untested end-to-end.
4. **Commands give no user feedback.** `/factory-reset`, `/add`, `/search`, etc. silently create a BridgeAction with zero visible output.
5. **No real ACP tests.** Existing `acp_integration.rs` only tests UI state, never connects to a real server.
6. **Settings edit→save is untested.** Navigation works but persistence is never verified.
7. **Config file I/O is untested in e2e.** Full load→display→edit→save→verify flow is missing.
8. **No confirmation for destructive commands.** `/factory-reset` executes immediately without warning.

## Architecture

### Two Harness Tiers

1. **`SimseTestHarness`** (existing) — Drives `App` model via `TestBackend`. Synchronous. Tests UI state, rendering, navigation, command dispatch. Fast (~0.1s per test).

2. **`RuntimeTestHarness`** (new) — Drives `TuiRuntime` with real `simse-bridge` config. Uses `#[tokio::test]`. Tests real file I/O, ACP connections, config persistence. Slower (~1-5s per test).

### Event Flow

```
User types /factory-reset
  → App.update(Submit)
  → dispatch returns [Info("Resetting..."), BridgeRequest(FactoryReset)]
  → App shows Confirm screen: "Are you sure?"
  → User presses Enter
  → App sets pending_bridge_action = FactoryReset
  → TuiRuntime.execute_bridge_action()
  → Deletes data_dir
  → Sends AppMessage::BridgeResult { action: "factory-reset", result: Ok("...") }
  → App.update(BridgeResult)
  → Pushes success to output
  → Resets onboarding state (needs_setup = true)
  → User sees welcome message
```

## Component Design

### 1. Command Feedback Messages

Every command returning a `BridgeRequest` also returns `CommandOutput::Info("...")`:

| Command | Feedback |
|---------|----------|
| `/factory-reset` | "Resetting all global configuration..." |
| `/factory-reset-project` | "Resetting project configuration..." |
| `/init` | "Initializing project configuration..." |
| `/compact` | "Compacting conversation history..." |
| `/add <topic>` | "Adding to library..." |
| `/search <query>` | "Searching library for: {query}" |
| `/recommend` | "Getting recommendations..." |
| `/topics` | "Listing library topics..." |
| `/volumes` | "Listing library volumes..." |
| `/get <id>` | "Retrieving volume..." |
| `/delete <id>` | "Deleting volume..." |
| `/resume <id>` | "Resuming session..." |
| `/rename <name>` | "Renaming session to: {name}" |
| `/server <name>` | "Switching to server: {name}" |
| `/model <name>` | "Switching to model: {name}" |
| `/mcp restart` | "Restarting MCP connections..." |
| `/acp restart` | "Restarting ACP connection..." |
| `/files` | "Listing files..." |
| `/save <path>` | "Saving to: {path}" |
| `/validate` | "Validating files..." |
| `/discard <path>` | "Discarding changes to: {path}" |
| `/diff` | "Generating diff..." |
| `/chain <name>` | "Running chain: {name}" |

### 2. Confirmation Dialogs for Destructive Commands

`/factory-reset` and `/factory-reset-project` route through `Screen::Confirm`:

```rust
// In dispatch: return ConfirmAction instead of BridgeRequest
CommandOutput::ConfirmAction {
    message: "Are you sure? This will delete ALL global SimSE configuration.".into(),
    action: BridgeAction::FactoryReset,
}
```

The Confirm screen shows the message with [Yes] / [Cancel] options. Enter confirms → executes the BridgeAction. Escape cancels → returns to Chat.

### 3. Factory-Reset → Onboarding Restart

After `FactoryReset` completes in event_loop:
1. Delete `data_dir` (existing behavior)
2. Send `AppMessage::BridgeResult` back to app
3. App handler for FactoryReset result:
   - Push success message to output
   - Reset `app.onboarding = OnboardingState { needs_setup: true, welcome_shown: false }`
   - Set `app.screen = Screen::Chat`
   - Clear config-dependent state

### 4. Status Bar with Server/Model Info

Expand `render_status_line()` to show:

```
ask (shift+tab) │ server: claude-code │ model: claude-sonnet │ idle
```

During operations:
```
ask (shift+tab) │ server: claude-code │ searching library...
```

Add fields to `App`:
- `server_name: String` (from config)
- `model_name: String` (from config)
- `status_message: Option<String>` (set by command feedback, cleared on completion)

### 5. Error Display Improvements

**Missing required args → show usage:**
```rust
// /add with no args:
"Usage: /add <topic> <text>\n\nAdd a volume to the library with the given topic and text."
```

**Unknown command → suggest similar:**
```rust
// /sarch →
"Unknown command: /sarch. Did you mean /search?"
```

Use Levenshtein distance (≤2) against the command registry to find suggestions.

**Connection errors → actionable hint:**
```
"ACP connection failed: connection refused. Run /acp restart or /setup to configure."
```

### 6. Command Result Formatting

Bridge action results get formatted before display:

| Action | Raw Result | Formatted |
|--------|-----------|-----------|
| FactoryReset | "Factory reset complete..." | "Factory reset complete.\nRemoved all configuration from {path}.\nRestarting setup..." |
| InitConfig | "Project initialized." | "Project initialized.\nCreated .simse/ in {work_dir}" |
| LibrarySearch | JSON array | Formatted table: "Found N results:\n  1. title - topic (score)" |
| SwitchServer | "Switched to: name" | "Switched ACP server to: {name}\nReconnecting..." |

### 7. Empty State Messages

Improve empty-state feedback:

| Command | Current | Improved |
|---------|---------|----------|
| `/sessions` | "No saved sessions." | "No saved sessions. Start chatting to create one." |
| `/tools` | "No tools registered." | "No tools registered. Connect to an ACP server with /setup to get started." |
| `/agents` | "No agents configured." | "No agents configured. Add agent files to .simse/agents/ to define custom agents." |
| `/skills` | "No skills configured." | "No skills configured. Add skills to .simse/skills/ to extend functionality." |
| `/prompts` | "No prompt templates configured." | "No prompt templates configured. Add prompts to .simse/prompts.json." |
| `/config` | "No configuration loaded." | "No configuration loaded. Run /init to create project configuration, or /setup for first-time setup." |

## E2E Test Plan

### New Test Files

#### `onboarding.rs` (6 tests)
1. `fresh_app_shows_welcome` — needs_setup=true shows welcome
2. `welcome_contains_setup_instructions` — has actionable steps
3. `setup_command_from_onboarding` — /setup opens wizard
4. `preset_selection_satisfies_onboarding` — selecting preset clears needs_setup
5. `factory_reset_triggers_onboarding` — reset → welcome reappears
6. `factory_reset_project_does_not_trigger_onboarding` — project reset ≠ global

#### `config_settings.rs` (6 tests, RuntimeTestHarness)
1. `config_shows_loaded_values` — /config displays real config
2. `settings_overlay_shows_file_contents` — field values match files
3. `factory_reset_deletes_global_config` — data_dir removed
4. `factory_reset_project_deletes_project_config` — .simse/ removed
5. `init_creates_project_directory` — /init creates .simse/
6. `global_vs_project_precedence` — project overrides global

#### `real_acp.rs` (6 tests, RuntimeTestHarness, always run)
1. `real_claude_code_connects` — connects to Claude Code ACP
2. `real_claude_code_prompt_roundtrip` — send prompt, get response
3. `real_ollama_connects` — connects to Ollama ACP
4. `real_ollama_prompt_roundtrip` — send prompt, get response
5. `real_server_switch` — switch between servers
6. `real_acp_restart` — restart connection

#### `command_feedback.rs` (8 tests)
1. `factory_reset_shows_confirmation` — /factory-reset shows confirm dialog
2. `factory_reset_confirm_executes` — confirming runs the reset
3. `factory_reset_cancel_aborts` — canceling returns to chat
4. `commands_show_progress_message` — bridge commands show "Working..." feedback
5. `unknown_command_suggests_similar` — typos get suggestions
6. `missing_args_shows_usage` — /add with no args shows usage
7. `status_bar_shows_server_info` — server/model visible in status
8. `empty_state_messages_are_helpful` — empty lists show guidance

### Existing Test Updates

All existing e2e tests that only check `pending_bridge_action` will be reviewed and enhanced to also verify:
- User-visible feedback messages appear in output
- Screen text contains the feedback
- Confirmation flows work for destructive commands

## Test Matrix Summary

| File | Tests | Type |
|------|-------|------|
| `onboarding.rs` | 6 | SimseTestHarness |
| `config_settings.rs` | 6 | RuntimeTestHarness |
| `real_acp.rs` | 6 | RuntimeTestHarness |
| `command_feedback.rs` | 8 | SimseTestHarness |
| Existing test updates | ~20 | SimseTestHarness |
| **New total** | **~46** | |

Combined with existing 91 e2e tests → **~117+ e2e tests**.
