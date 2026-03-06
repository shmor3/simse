# Settings & Config Interactive UI Design

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers-extended-cc:executing-plans to implement this plan task-by-task.

**Goal:** Make all 8 config files editable through an interactive settings overlay with schema-driven forms, backed by a `ConfigStorage` trait in simse-ui-core for platform portability.

**Architecture:** Trait abstraction in simse-ui-core (ConfigStorage + SettingsFormState) with form UX from Approach A. TUI implements the trait with file I/O. State machine returns actions; host executes them.

**Tech Stack:** Rust, serde_json, async_trait, tokio::fs (TUI implementation)

---

## 1. ConfigStorage Trait (simse-ui-core)

Lives in `simse-ui-core/src/config/storage.rs`.

```rust
pub enum ConfigScope { Global, Project }

pub enum ConfigError {
    NotFound { filename: String },
    IoError(String),
    ParseError { filename: String, detail: String },
    ValidationError { field: String, detail: String },
}

pub type ConfigResult<T> = Result<T, ConfigError>;

#[async_trait]
pub trait ConfigStorage: Send + Sync {
    async fn load_file(&self, filename: &str, scope: ConfigScope) -> ConfigResult<serde_json::Value>;
    async fn save_file(&self, filename: &str, scope: ConfigScope, data: &serde_json::Value) -> ConfigResult<()>;
    async fn file_exists(&self, filename: &str, scope: ConfigScope) -> bool;
    async fn delete_file(&self, filename: &str, scope: ConfigScope) -> ConfigResult<()>;
    async fn delete_all(&self, scope: ConfigScope) -> ConfigResult<()>;
    async fn ensure_dir(&self, scope: ConfigScope) -> ConfigResult<()>;
}
```

Helper functions (not trait methods):
- `get_field()` — read single field from config file
- `set_field()` — read-modify-write single field
- `add_array_entry()` — append to array field (servers)
- `remove_array_entry()` — remove from array by index

## 2. SettingsFormState (simse-ui-core)

Lives in `simse-ui-core/src/config/settings_state.rs`. Pure state machine, no I/O.

**4 navigation levels:**
- Level 0: FileList — 8 config files
- Level 1: FieldList — fields/entries for selected file
- Level 2: Editing — single field value editing
- Level 3: ArrayEntry — fields within an array entry (servers, prompts)

**Input methods return `SettingsAction`:**
- `enter()` → `LoadFile`, `SaveField`, `AddEntry`, `None`
- `back()` → go up one level or `Dismiss`
- `toggle()` → `SaveField` with flipped boolean
- `type_char()`, `backspace()` → edit buffer manipulation
- `move_up()`, `move_down()` → navigation

**SettingsAction enum:**
```rust
pub enum SettingsAction {
    None,
    LoadFile { filename: String, scope: ConfigScope },
    SaveField { filename: String, scope: ConfigScope, key: String, value: serde_json::Value },
    AddEntry { filename: String, scope: ConfigScope, array_key: String, entry: serde_json::Value },
    RemoveEntry { filename: String, scope: ConfigScope, array_key: String, index: usize },
    Dismiss,
}
```

## 3. TUI Integration

**FileConfigStorage** in `simse-tui/src/config.rs`:
- Implements `ConfigStorage` with `tokio::fs`
- Atomic writes via temp file + rename
- Paths from `global_dir` (data_dir) and `project_dir` (work_dir/.simse/)

**App changes:**
- Replace `SettingsExplorerState` with `SettingsFormState`
- Remove `settings_config_data` (now in `SettingsFormState.config_data`)
- New AppMessages: `SettingsFileLoaded`, `SettingsFieldSaved`, `SettingsError`
- Bridge actions (InitConfig, FactoryReset, SetupAcp) reimplemented via ConfigStorage

**Event flow:**
1. `/settings` → create `SettingsFormState` at FileList
2. Enter on file → `LoadFile` action → TUI loads via storage → `SettingsFileLoaded` message
3. Navigate to field, Enter → edit buffer populated
4. Edit + Enter → `SaveField` action → TUI saves via storage → `SettingsFieldSaved` message
5. "Saved ✓" feedback shown

**Rendering** stays in `simse-tui/src/overlays/settings.rs`, reads from `SettingsFormState`.

## 4. Missing Schemas

Add to `simse-ui-core/src/config/settings_schema.rs`:

**mcp.json** — array of MCP server entries:
- Fields per entry: name (Text), command (Text), args (Text)

**prompts.json** — HashMap of named prompts:
- Fields per entry: name (Text key), template (Text), description (Text)

## 5. Testing

**Layer 1 — Unit tests in simse-ui-core:**
- SettingsFormState navigation (all levels, wrap-around, actions)
- Edit buffer manipulation
- Boolean toggle, select cycling
- ConfigStorage helpers with MockConfigStorage
- Error cases (missing file, parse error)

**Layer 2 — PTY integration tests in simse-tui:**
- `/settings` → navigate → see actual field values from disk
- Edit field → Enter → verify file on disk changed
- `/init` → verify `.simse/` created
- `/factory-reset` → confirm → verify files deleted
- `/setup claude` → verify `acp.json` written
- Boolean toggle → verify on disk
- Add/remove server entries → verify on disk

## Files Changed

**New files:**
- `simse-ui-core/src/config/storage.rs` — ConfigStorage trait + helpers
- `simse-ui-core/src/config/settings_state.rs` — SettingsFormState state machine

**Modified files:**
- `simse-ui-core/src/config/mod.rs` — export new modules
- `simse-ui-core/src/config/settings_schema.rs` — add mcp.json + prompts.json schemas
- `simse-tui/src/config.rs` — add FileConfigStorage impl
- `simse-tui/src/app.rs` — replace SettingsExplorerState, new AppMessages
- `simse-tui/src/main.rs` — dispatch SettingsActions via storage
- `simse-tui/src/overlays/settings.rs` — render from SettingsFormState
- `simse-tui/src/event_loop.rs` — bridge actions use ConfigStorage
- `simse-tui/tests/pty/` — new PTY tests for settings persistence
