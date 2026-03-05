# Full Stack Test Architecture Audit & Migration — Design

## Goal

Eliminate the disconnect between what tests verify and what production runs. Replace all custom test harnesses (`SimseTestHarness`, `RuntimeTestHarness`) with `terminal-testlib` PTY tests that spawn the real `simse-tui` binary. Add integration tests across `simse-bridge`, `simse-acp`, and `simse-ui-core` that use real production code paths — real ACP servers, real filesystem I/O, real subprocess lifecycle.

## Problem Statement

The factory-reset bug exposed a fundamental flaw: **all 180 non-unit tests test the wrong layer**. `SimseTestHarness` calls `update()` directly, bypassing `main.rs` entirely. Production runs through `main.rs` → `TuiRuntime` → `AcpClient` → `simse-acp-engine` → real ACP servers. The tests verified that `pending_bridge_action` was set, but nothing verified it was ever dispatched.

### Audit Findings by Crate

| Crate | Unit Tests | Non-Unit Tests | Critical Gaps |
|-------|-----------|----------------|---------------|
| simse-tui | 869 | 180 (all via SimseTestHarness) | `main.rs` event loop NEVER tested. Bridge dispatch, config loading, TuiRuntime — all bypassed |
| simse-bridge | 145 | 9 (stub-based) | `client.rs`, `json_io.rs`, `storage.rs` = 0 tests. AcpClient operations untested |
| simse-ui-core | 221 | 0 | Zero integration tests. Command dispatch + permission pipeline untested |
| simse-acp | 107 | 26 (protocol only) | AcpClient public methods untested. Real ACP server tests = 0 |

## Architecture

### Testing Library: terminal-testlib

[`terminal-testlib`](https://github.com/raibid-labs/ratatui-testlib) v0.6.0 provides PTY-based integration testing:

- Spawns the **real binary** in a pseudo-terminal
- Sends **real keystrokes** via `send_key()` / `send_keys()`
- Reads **real screen output** via `wait_for_text()` / `screen_contents()`
- Supports async via `async-tokio` feature
- CI-ready via `headless` feature

This means tests exercise the **exact same code path** as production:

```
terminal-testlib PTY
  └── spawns simse-tui binary
      └── main.rs (real event loop)
          ├── parse_cli_args() (real CLI parsing)
          ├── load_config() (real config loading)
          ├── TuiRuntime::new() (real runtime)
          ├── map_event() (real crossterm input)
          ├── update() (real state transitions)
          ├── pending_bridge_action.take() (real dispatch)
          │   └── dispatch_bridge_action() (real execution)
          │       └── AcpClient → simse-acp-engine → real ACP server
          └── view() (real rendering)
```

### Gating Policy

**No env var gating.** Real ACP servers (Claude Code, Ollama) must be available for every test run. If they're not, the test fails — that's a real problem.

### What Gets Deleted

- `simse-tui/tests/e2e/harness.rs` — `SimseTestHarness`
- `simse-tui/tests/e2e/runtime_harness.rs` — `RuntimeTestHarness`
- All tests that use `SimseTestHarness::new()` are rewritten

### What Gets Kept

- **Unit tests** (869 in simse-tui/src/) — pure function tests for `update()/view()`, parsing, autocomplete, etc. These are fast and valuable.
- Unit tests across all crates remain unchanged.

## Test Design

### Layer 4: simse-tui PTY Tests

**Dependency:**
```toml
[dev-dependencies]
terminal-testlib = { version = "0.6", features = ["async-tokio", "headless"] }
tempfile = "3"
```

**New file structure:**
```
tests/pty/
  mod.rs              — Shared helpers (spawn binary, type command, wait for text)
  startup.rs          — App launch, banner, onboarding (migrated from e2e/startup.rs)
  input.rs            — Text input, cursor, selection (migrated from e2e/input.rs)
  autocomplete.rs     — /command autocomplete (migrated from e2e/autocomplete.rs)
  overlays.rs         — Settings, librarian, setup overlays (migrated from e2e/overlays.rs)
  commands_config.rs  — /config, /settings, /init, /factory-reset (migrated + enhanced)
  commands_session.rs — /sessions, /resume, /rename (migrated + enhanced)
  commands_files.rs   — /files, /save, /validate (migrated + enhanced)
  commands_library.rs — /add, /search, /topics (migrated + enhanced)
  commands_meta.rs    — /help, /plan, /verbose (migrated from e2e/commands_meta.rs)
  commands_tools.rs   — /tools (migrated from e2e/commands_tools.rs)
  acp_flow.rs         — ACP connection, prompt, streaming (migrated from e2e/acp_integration.rs + real_acp.rs)
  error_states.rs     — Error display (migrated from e2e/error_states.rs)
  setup_wizard.rs     — Setup wizard flow (migrated from e2e/setup_wizard.rs)
  onboarding.rs       — Onboarding state (migrated from e2e/onboarding.rs)
  command_feedback.rs — Feedback messages (migrated from e2e/command_feedback.rs)
  bridge_actions.rs   — NEW: Bridge dispatch end-to-end tests
  config_settings.rs  — Config/settings real I/O (migrated from e2e/config_settings.rs)
```

**Shared helper (mod.rs):**
```rust
use terminal_testlib::{TuiTestHarness, KeyCode};
use tempfile::TempDir;
use std::path::Path;

pub fn spawn_simse(data_dir: &Path) -> TuiTestHarness {
    let mut harness = TuiTestHarness::new(120, 40).expect("Failed to create harness");
    let binary = env!("CARGO_BIN_EXE_simse-tui");
    let mut cmd = portable_pty::CommandBuilder::new(binary);
    cmd.arg("--data-dir").arg(data_dir.to_str().unwrap());
    harness.spawn(cmd).expect("Failed to spawn simse-tui");
    harness
}

pub fn type_command(harness: &mut TuiTestHarness, cmd: &str) {
    harness.send_keys(cmd).unwrap();
    harness.send_key(KeyCode::Enter).unwrap();
}
```

**Migration pattern — old vs new:**

Old (SimseTestHarness):
```rust
#[test]
fn factory_reset_confirm_creates_bridge_action() {
    let mut h = SimseTestHarness::new();
    h.submit("/factory-reset");           // Calls update() directly
    h.press_enter();                      // Calls update() directly
    let action = h.app.pending_bridge_action.as_ref();
    assert_eq!(*action.unwrap(), BridgeAction::FactoryReset);  // Checks state
    // NEVER dispatches the action, NEVER verifies it works
}
```

New (terminal-testlib PTY):
```rust
#[tokio::test]
async fn factory_reset_full_flow() {
    let tmp = TempDir::new().unwrap();
    let data_dir = tmp.path().join("data");
    std::fs::create_dir_all(&data_dir).unwrap();
    // Write minimal config so factory-reset has something to delete
    std::fs::write(data_dir.join("config.json"), "{}").unwrap();

    let mut h = spawn_simse(&data_dir);
    h.wait_for_text("SimSE").unwrap();         // App started
    type_command(&mut h, "/factory-reset");
    h.wait_for_text("Are you sure").unwrap();  // Confirm dialog
    h.send_key(KeyCode::Enter).unwrap();       // Confirm
    h.wait_for_text("Factory reset complete").unwrap();  // Action dispatched!
    // Verify onboarding restarts
    h.wait_for_text("Welcome").unwrap();
    // Verify data_dir was actually deleted
    assert!(!data_dir.exists());
}
```

**Test counts (migrated + new):**

| File | Migrated | New | Total |
|------|----------|-----|-------|
| startup.rs | 5 | 0 | 5 |
| input.rs | 8 | 0 | 8 |
| autocomplete.rs | 7 | 0 | 7 |
| overlays.rs | 8 | 0 | 8 |
| commands_config.rs | 7 | 3 | 10 |
| commands_session.rs | 7 | 0 | 7 |
| commands_files.rs | 5 | 0 | 5 |
| commands_library.rs | 7 | 0 | 7 |
| commands_meta.rs | 9 | 0 | 9 |
| commands_tools.rs | 5 | 0 | 5 |
| acp_flow.rs | 14 | 4 | 18 |
| error_states.rs | 4 | 0 | 4 |
| setup_wizard.rs | 10 | 0 | 10 |
| onboarding.rs | 6 | 0 | 6 |
| command_feedback.rs | 8 | 0 | 8 |
| bridge_actions.rs | 0 | 7 | 7 |
| config_settings.rs | 6 | 3 | 9 |
| **Subtotal** | **116** | **17** | **133** |

Plus integration.rs migration: 61 → PTY tests merged into above files.

### Layer 3: simse-bridge Integration Tests

**New file: `simse-bridge/tests/real_integration.rs`** (~16 tests)

Config loading (4):
1. `load_config_reads_real_files` — real config files in tmpdir
2. `load_config_merges_precedence` — workspace overrides global
3. `load_config_agents_and_skills` — discovers .md files
4. `load_config_simse_md` — reads SIMSE.md

AcpClient with real simse-acp-engine (5):
5. `acp_client_real_connection` — connects to real Claude Code
6. `acp_client_generates_response` — generate() returns text
7. `acp_client_streams` — generate_stream() yields deltas
8. `acp_client_session_lifecycle` — new → prompt → list
9. `acp_client_embed` — embed() returns vectors

Session store (3):
10. `session_store_full_lifecycle` — CRUD with real filesystem
11. `session_store_survives_corruption` — corrupt lines skipped
12. `session_store_cross_instance` — persists between instances

Untested modules (4):
13. `json_io_roundtrip` — write/read JSON files
14. `json_io_jsonl_roundtrip` — append/read JSONL
15. `storage_save_load` — FileStorageBackend roundtrip
16. `storage_gzip` — compression works

### Layer 2: simse-acp Integration Tests

**Enhanced `simse-acp/tests/integration.rs`** (+8 tests)

Real ACP server tests:
1. `initialize_with_real_server` — connect to Claude Code
2. `generate_real_response` — get text from real server
3. `stream_real_response` — receive streaming deltas
4. `session_lifecycle` — create, prompt, list, delete
5. `embed_real_vectors` — generate embeddings
6. `server_health` — health check returns healthy
7. `multi_server_init` — multiple servers simultaneously
8. `dispose_cleanup` — dispose then verify methods fail

### Layer 1: simse-ui-core Integration Tests

**New `simse-ui-core/tests/integration.rs`** (~9 tests)

Command dispatch pipeline:
1. `all_commands_dispatch_without_panic` — iterate registry, dispatch each
2. `bridge_commands_produce_bridge_request` — correct BridgeAction variants
3. `inline_commands_produce_immediate_output` — /help, /verbose
4. `unknown_command_produces_error` — invalid → Error

Permission integration:
5. `plan_mode_blocks_write_tools` — Plan mode denies writes
6. `accept_edits_allows_edit_tools` — AcceptEdits allows edits
7. `permission_rules_override_mode` — custom rules > mode

Registry validation:
8. `no_duplicate_command_names` — all names unique
9. `all_commands_have_descriptions` — no empty metadata

## Test Counts Summary

| Crate | Current Non-Unit | After Migration | Delta |
|-------|-----------------|-----------------|-------|
| simse-tui | 180 (SimseTestHarness) | 133 (PTY) | Refactored, +17 new |
| simse-bridge | 9 (stub) | 25 (real) | +16 new |
| simse-acp | 26 (protocol only) | 34 (real servers) | +8 new |
| simse-ui-core | 0 | 9 | +9 new |
| **Total** | **215** | **201** | **+50 new, all real production code** |

Note: Some existing tests that were redundant across e2e and integration.rs are consolidated.

## Success Criteria

After this migration:
1. **Zero test uses `SimseTestHarness` or `RuntimeTestHarness`** — deleted
2. **Every command test spawns the real binary** — through terminal-testlib PTY
3. **Bridge actions are dispatched and verified** — not just created
4. **ACP tests connect to real servers** — Claude Code, Ollama
5. **Config loading tested with real files** — not just tmpdir stubs
6. **Session persistence tested end-to-end** — create, save, load across instances
7. **If a test passes, the feature works in production** — no more false confidence
