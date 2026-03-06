# Settings & Config Interactive UI Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers-extended-cc:executing-plans to implement this plan task-by-task.

**Goal:** Make all 8 config files editable through an interactive settings overlay with schema-driven forms, backed by a `ConfigStorage` trait in simse-ui-core for platform portability.

**Architecture:** Trait abstraction in simse-ui-core (ConfigStorage + SettingsFormState) with form UX from Approach A. TUI implements the trait with file I/O. State machine returns actions; host executes them.

**Tech Stack:** Rust, serde_json, async_trait, tokio::fs (TUI implementation)

---

### Task 1: Add `async-trait` dependency to simse-ui-core

**Files:**
- Modify: `simse-ui-core/Cargo.toml`

**Step 1: Add the dependency**

Add `async-trait` to `simse-ui-core/Cargo.toml` under `[dependencies]`:

```toml
[dependencies]
simse-core = { path = "../simse-core" }
serde = { workspace = true }
serde_json = { workspace = true }
regex = { workspace = true }
async-trait = "0.1"
```

**Step 2: Verify it compiles**

Run: `cd simse-ui-core && cargo check`
Expected: Compiles successfully.

**Step 3: Commit**

```bash
git add simse-ui-core/Cargo.toml
git commit -m "chore(simse-ui-core): add async-trait dependency for ConfigStorage"
```

---

### Task 2: Create ConfigStorage trait and helpers

**Files:**
- Create: `simse-ui-core/src/config/storage.rs`
- Modify: `simse-ui-core/src/config/mod.rs`
- Test: `simse-ui-core/src/config/storage.rs` (inline tests)

**Step 1: Write failing tests for ConfigStorage helpers**

Create `simse-ui-core/src/config/storage.rs` with:

```rust
//! Platform-agnostic config storage trait and helpers.

use async_trait::async_trait;
use serde_json::Value;
use std::fmt;

// ── Error types ────────────────────────────────────────────

/// Scope of a config file: global (~/.config/simse) or project (.simse/).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigScope {
    Global,
    Project,
}

impl fmt::Display for ConfigScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigScope::Global => write!(f, "global"),
            ConfigScope::Project => write!(f, "project"),
        }
    }
}

/// Errors that can occur during config operations.
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigError {
    NotFound { filename: String },
    IoError(String),
    ParseError { filename: String, detail: String },
    ValidationError { field: String, detail: String },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::NotFound { filename } => write!(f, "Config file not found: {filename}"),
            ConfigError::IoError(msg) => write!(f, "I/O error: {msg}"),
            ConfigError::ParseError { filename, detail } => {
                write!(f, "Parse error in {filename}: {detail}")
            }
            ConfigError::ValidationError { field, detail } => {
                write!(f, "Validation error for {field}: {detail}")
            }
        }
    }
}

pub type ConfigResult<T> = Result<T, ConfigError>;

// ── Trait ──────────────────────────────────────────────────

/// Platform-agnostic config storage.
///
/// Implementations handle the actual file I/O (or HTTP, or whatever the
/// platform uses). The state machine in `SettingsFormState` never calls
/// this directly — it returns `SettingsAction` values that the host
/// dispatches through a `ConfigStorage` implementation.
#[async_trait]
pub trait ConfigStorage: Send + Sync {
    /// Load a config file as raw JSON.
    async fn load_file(&self, filename: &str, scope: ConfigScope) -> ConfigResult<Value>;

    /// Save raw JSON to a config file.
    async fn save_file(&self, filename: &str, scope: ConfigScope, data: &Value) -> ConfigResult<()>;

    /// Check if a config file exists.
    async fn file_exists(&self, filename: &str, scope: ConfigScope) -> bool;

    /// Delete a config file.
    async fn delete_file(&self, filename: &str, scope: ConfigScope) -> ConfigResult<()>;

    /// Delete all config files in the given scope.
    async fn delete_all(&self, scope: ConfigScope) -> ConfigResult<()>;

    /// Ensure the config directory for the given scope exists.
    async fn ensure_dir(&self, scope: ConfigScope) -> ConfigResult<()>;
}

// ── Helper functions ──────────────────────────────────────

/// Read a single field from a JSON object.
pub fn get_field(data: &Value, key: &str) -> Option<Value> {
    data.as_object().and_then(|obj| obj.get(key).cloned())
}

/// Set a single field in a JSON object (read-modify-write).
///
/// If `data` is not an object, it is replaced with a new object containing
/// only the given key.
pub fn set_field(data: &mut Value, key: &str, value: Value) {
    if !data.is_object() {
        *data = Value::Object(serde_json::Map::new());
    }
    if let Some(obj) = data.as_object_mut() {
        obj.insert(key.to_string(), value);
    }
}

/// Append an entry to an array field. Creates the array if it doesn't exist.
pub fn add_array_entry(data: &mut Value, array_key: &str, entry: Value) {
    if !data.is_object() {
        *data = Value::Object(serde_json::Map::new());
    }
    if let Some(obj) = data.as_object_mut() {
        let arr = obj
            .entry(array_key.to_string())
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Some(vec) = arr.as_array_mut() {
            vec.push(entry);
        }
    }
}

/// Remove an entry from an array field by index.
///
/// Returns `Err` if the index is out of bounds or the field is not an array.
pub fn remove_array_entry(
    data: &mut Value,
    array_key: &str,
    index: usize,
) -> ConfigResult<Value> {
    let obj = data
        .as_object_mut()
        .ok_or_else(|| ConfigError::ValidationError {
            field: array_key.to_string(),
            detail: "data is not an object".to_string(),
        })?;

    let arr = obj
        .get_mut(array_key)
        .and_then(|v| v.as_array_mut())
        .ok_or_else(|| ConfigError::ValidationError {
            field: array_key.to_string(),
            detail: "field is not an array".to_string(),
        })?;

    if index >= arr.len() {
        return Err(ConfigError::ValidationError {
            field: array_key.to_string(),
            detail: format!("index {index} out of bounds (len {})", arr.len()),
        });
    }

    Ok(arr.remove(index))
}

// ── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── ConfigScope Display ──────────────────────
    #[test]
    fn config_scope_display() {
        assert_eq!(ConfigScope::Global.to_string(), "global");
        assert_eq!(ConfigScope::Project.to_string(), "project");
    }

    // ── ConfigError Display ──────────────────────
    #[test]
    fn config_error_display() {
        let e = ConfigError::NotFound { filename: "x.json".into() };
        assert!(e.to_string().contains("x.json"));
        let e = ConfigError::IoError("disk full".into());
        assert!(e.to_string().contains("disk full"));
        let e = ConfigError::ParseError {
            filename: "y.json".into(),
            detail: "bad json".into(),
        };
        assert!(e.to_string().contains("y.json"));
        let e = ConfigError::ValidationError {
            field: "port".into(),
            detail: "must be positive".into(),
        };
        assert!(e.to_string().contains("port"));
    }

    // ── get_field ────────────────────────────────
    #[test]
    fn get_field_existing() {
        let data = json!({"host": "localhost", "port": 8080});
        assert_eq!(get_field(&data, "host"), Some(json!("localhost")));
        assert_eq!(get_field(&data, "port"), Some(json!(8080)));
    }

    #[test]
    fn get_field_missing() {
        let data = json!({"host": "localhost"});
        assert_eq!(get_field(&data, "missing"), None);
    }

    #[test]
    fn get_field_non_object() {
        let data = json!("string");
        assert_eq!(get_field(&data, "key"), None);
    }

    #[test]
    fn get_field_null() {
        assert_eq!(get_field(&Value::Null, "key"), None);
    }

    // ── set_field ────────────────────────────────
    #[test]
    fn set_field_existing_object() {
        let mut data = json!({"host": "localhost"});
        set_field(&mut data, "port", json!(9090));
        assert_eq!(data, json!({"host": "localhost", "port": 9090}));
    }

    #[test]
    fn set_field_overwrites() {
        let mut data = json!({"host": "old"});
        set_field(&mut data, "host", json!("new"));
        assert_eq!(data["host"], json!("new"));
    }

    #[test]
    fn set_field_on_non_object_creates_object() {
        let mut data = json!("not an object");
        set_field(&mut data, "key", json!("value"));
        assert_eq!(data, json!({"key": "value"}));
    }

    #[test]
    fn set_field_on_null() {
        let mut data = Value::Null;
        set_field(&mut data, "key", json!(42));
        assert_eq!(data, json!({"key": 42}));
    }

    // ── add_array_entry ──────────────────────────
    #[test]
    fn add_array_entry_existing() {
        let mut data = json!({"servers": [{"name": "a"}]});
        add_array_entry(&mut data, "servers", json!({"name": "b"}));
        let arr = data["servers"].as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[1], json!({"name": "b"}));
    }

    #[test]
    fn add_array_entry_creates_array() {
        let mut data = json!({});
        add_array_entry(&mut data, "servers", json!({"name": "first"}));
        assert_eq!(data["servers"], json!([{"name": "first"}]));
    }

    #[test]
    fn add_array_entry_on_non_object() {
        let mut data = Value::Null;
        add_array_entry(&mut data, "items", json!("x"));
        assert_eq!(data, json!({"items": ["x"]}));
    }

    // ── remove_array_entry ───────────────────────
    #[test]
    fn remove_array_entry_valid() {
        let mut data = json!({"servers": ["a", "b", "c"]});
        let removed = remove_array_entry(&mut data, "servers", 1).unwrap();
        assert_eq!(removed, json!("b"));
        assert_eq!(data["servers"], json!(["a", "c"]));
    }

    #[test]
    fn remove_array_entry_out_of_bounds() {
        let mut data = json!({"servers": ["a"]});
        let err = remove_array_entry(&mut data, "servers", 5);
        assert!(err.is_err());
    }

    #[test]
    fn remove_array_entry_not_array() {
        let mut data = json!({"servers": "not-array"});
        let err = remove_array_entry(&mut data, "servers", 0);
        assert!(err.is_err());
    }

    #[test]
    fn remove_array_entry_not_object() {
        let mut data = json!("string");
        let err = remove_array_entry(&mut data, "key", 0);
        assert!(err.is_err());
    }
}
```

**Step 2: Export from mod.rs**

Update `simse-ui-core/src/config/mod.rs` to:

```rust
//! Configuration schema and settings.

pub mod settings_schema;
pub mod storage;
```

**Step 3: Run tests to verify they pass**

Run: `cd simse-ui-core && cargo test config::storage`
Expected: All tests pass.

**Step 4: Commit**

```bash
git add simse-ui-core/src/config/storage.rs simse-ui-core/src/config/mod.rs
git commit -m "feat(simse-ui-core): add ConfigStorage trait and JSON helpers"
```

---

### Task 3: Create SettingsFormState state machine

**Files:**
- Create: `simse-ui-core/src/config/settings_state.rs`
- Modify: `simse-ui-core/src/config/mod.rs`

**Step 1: Create the state machine**

Create `simse-ui-core/src/config/settings_state.rs`. This is a pure state machine with no I/O — it returns `SettingsAction` values for the host to execute.

```rust
//! Pure settings form state machine.
//!
//! Four navigation levels:
//! - Level 0: FileList — 8 config files
//! - Level 1: FieldList — fields/entries for selected file
//! - Level 2: Editing — single field value editing
//! - Level 3: ArrayEntry — fields within an array entry (servers, prompts)

use std::time::Instant;

use crate::config::settings_schema::{
    all_config_schemas, get_config_schema, ConfigFileSchema, FieldType,
};
use crate::config::storage::ConfigScope;

// ── Constants ───────────────────────────────────────────────

/// Config files available in the settings form, with display labels and scopes.
pub const CONFIG_FILES: &[(&str, &str, ConfigScope)] = &[
    ("config.json", "General", ConfigScope::Global),
    ("acp.json", "ACP Servers", ConfigScope::Global),
    ("mcp.json", "MCP Servers", ConfigScope::Global),
    ("embed.json", "Embedding", ConfigScope::Global),
    ("memory.json", "Memory", ConfigScope::Global),
    ("summarize.json", "Summarization", ConfigScope::Global),
    ("settings.json", "Settings", ConfigScope::Project),
    ("prompts.json", "System Prompts", ConfigScope::Project),
];

/// Duration (in seconds) for which the "Saved" indicator is visible.
const SAVED_INDICATOR_DURATION_SECS: f64 = 1.5;

// ── Navigation level ────────────────────────────────────────

/// Which navigation level the settings form is at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsLevel {
    /// Choosing a config file.
    FileList,
    /// Browsing fields of the selected file.
    FieldList,
    /// Editing a specific field value.
    Editing,
    /// Editing fields within an array entry (servers, prompts).
    ArrayEntry,
}

// ── Actions returned by the state machine ───────────────────

/// Actions the host must execute after a state machine method call.
#[derive(Debug, Clone, PartialEq)]
pub enum SettingsAction {
    /// No action needed.
    None,
    /// Load a config file from storage.
    LoadFile {
        filename: String,
        scope: ConfigScope,
    },
    /// Save a single field value.
    SaveField {
        filename: String,
        scope: ConfigScope,
        key: String,
        value: serde_json::Value,
    },
    /// Add an entry to an array field.
    AddEntry {
        filename: String,
        scope: ConfigScope,
        array_key: String,
        entry: serde_json::Value,
    },
    /// Remove an entry from an array field.
    RemoveEntry {
        filename: String,
        scope: ConfigScope,
        array_key: String,
        index: usize,
    },
    /// Dismiss the settings overlay.
    Dismiss,
}

// ── State ───────────────────────────────────────────────────

/// Pure state machine for the interactive settings form.
///
/// All methods are synchronous and return `SettingsAction` values.
/// The host is responsible for executing those actions and feeding
/// results back via `file_loaded()`, `field_saved()`, etc.
#[derive(Debug, Clone)]
pub struct SettingsFormState {
    /// Current navigation level.
    pub level: SettingsLevel,
    /// Index of the selected config file.
    pub selected_file: usize,
    /// Index of the selected field within the current file.
    pub selected_field: usize,
    /// The current edit buffer (for text/number editing).
    pub edit_value: String,
    /// Cursor position within the edit buffer.
    pub cursor: usize,
    /// Loaded config data for the current file.
    pub config_data: serde_json::Value,
    /// Timestamp of the last save action.
    pub saved_indicator: Option<Instant>,
    /// Select field option index (for cycling through options).
    pub select_index: usize,
    /// Error message to display (cleared on next action).
    pub error: Option<String>,
}

impl SettingsFormState {
    /// Create a new settings form state at the FileList level.
    pub fn new() -> Self {
        Self {
            level: SettingsLevel::FileList,
            selected_file: 0,
            selected_field: 0,
            edit_value: String::new(),
            cursor: 0,
            config_data: serde_json::Value::Null,
            saved_indicator: None,
            select_index: 0,
            error: None,
        }
    }

    // ── Navigation ──────────────────────────────

    /// Move selection up within the current level.
    pub fn move_up(&mut self) {
        match self.level {
            SettingsLevel::FileList => {
                if self.selected_file > 0 {
                    self.selected_file -= 1;
                }
            }
            SettingsLevel::FieldList | SettingsLevel::Editing | SettingsLevel::ArrayEntry => {
                if self.selected_field > 0 {
                    self.selected_field -= 1;
                }
            }
        }
    }

    /// Move selection down within the current level.
    pub fn move_down(&mut self) {
        let count = self.current_item_count();
        if count == 0 {
            return;
        }
        match self.level {
            SettingsLevel::FileList => {
                if self.selected_file + 1 < count {
                    self.selected_file += 1;
                }
            }
            SettingsLevel::FieldList | SettingsLevel::Editing | SettingsLevel::ArrayEntry => {
                if self.selected_field + 1 < count {
                    self.selected_field += 1;
                }
            }
        }
    }

    /// Returns the number of items in the current level.
    pub fn current_item_count(&self) -> usize {
        match self.level {
            SettingsLevel::FileList => CONFIG_FILES.len(),
            SettingsLevel::FieldList => self.field_count(),
            SettingsLevel::Editing | SettingsLevel::ArrayEntry => 0,
        }
    }

    /// Enter: go deeper or confirm edit.
    pub fn enter(&mut self) -> SettingsAction {
        self.error = None;
        match self.level {
            SettingsLevel::FileList => {
                let (filename, _, scope) = CONFIG_FILES[self.selected_file];
                self.level = SettingsLevel::FieldList;
                self.selected_field = 0;
                SettingsAction::LoadFile {
                    filename: filename.to_string(),
                    scope,
                }
            }
            SettingsLevel::FieldList => {
                // Populate the edit buffer with the current value.
                let current = self.current_field_value();
                self.edit_value = current;
                self.cursor = self.edit_value.len();
                self.select_index = self.current_select_index();
                self.level = SettingsLevel::Editing;
                SettingsAction::None
            }
            SettingsLevel::Editing => {
                // Confirm edit → save.
                self.save_current_field()
            }
            SettingsLevel::ArrayEntry => {
                // TODO: array entry editing
                SettingsAction::None
            }
        }
    }

    /// Back: go up one level or dismiss.
    pub fn back(&mut self) -> SettingsAction {
        self.error = None;
        match self.level {
            SettingsLevel::ArrayEntry => {
                self.level = SettingsLevel::FieldList;
                self.edit_value.clear();
                SettingsAction::None
            }
            SettingsLevel::Editing => {
                self.level = SettingsLevel::FieldList;
                self.edit_value.clear();
                SettingsAction::None
            }
            SettingsLevel::FieldList => {
                self.level = SettingsLevel::FileList;
                self.config_data = serde_json::Value::Null;
                SettingsAction::None
            }
            SettingsLevel::FileList => SettingsAction::Dismiss,
        }
    }

    /// Toggle a boolean value (at Editing level).
    pub fn toggle(&mut self) -> SettingsAction {
        if self.level != SettingsLevel::Editing {
            return SettingsAction::None;
        }
        match self.edit_value.as_str() {
            "true" => {
                self.edit_value = "false".to_string();
                self.save_current_field()
            }
            "false" => {
                self.edit_value = "true".to_string();
                self.save_current_field()
            }
            _ => SettingsAction::None,
        }
    }

    /// Cycle a Select field to the next option.
    pub fn cycle_select(&mut self) -> SettingsAction {
        if self.level != SettingsLevel::Editing {
            return SettingsAction::None;
        }
        if let Some(schema) = self.current_file_schema() {
            if let Some(field_schema) = schema.fields.get(self.selected_field) {
                if let FieldType::Select { options } = &field_schema.field_type {
                    if !options.is_empty() {
                        self.select_index = (self.select_index + 1) % options.len();
                        self.edit_value = options[self.select_index].clone();
                        return self.save_current_field();
                    }
                }
            }
        }
        SettingsAction::None
    }

    /// Type a character into the edit buffer.
    pub fn type_char(&mut self, c: char) {
        if self.level == SettingsLevel::Editing {
            self.edit_value.insert(self.cursor, c);
            self.cursor += c.len_utf8();
        }
    }

    /// Delete the character before the cursor.
    pub fn backspace(&mut self) {
        if self.level == SettingsLevel::Editing && self.cursor > 0 {
            // Find the previous char boundary.
            let prev = self.edit_value[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.edit_value.remove(prev);
            self.cursor = prev;
        }
    }

    /// Delete the character after the cursor.
    pub fn delete(&mut self) {
        if self.level == SettingsLevel::Editing && self.cursor < self.edit_value.len() {
            self.edit_value.remove(self.cursor);
        }
    }

    /// Move cursor left.
    pub fn cursor_left(&mut self) {
        if self.cursor > 0 {
            let prev = self.edit_value[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.cursor = prev;
        }
    }

    /// Move cursor right.
    pub fn cursor_right(&mut self) {
        if self.cursor < self.edit_value.len() {
            let next = self.edit_value[self.cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor + i)
                .unwrap_or(self.edit_value.len());
            self.cursor = next;
        }
    }

    /// Move cursor to start.
    pub fn cursor_home(&mut self) {
        self.cursor = 0;
    }

    /// Move cursor to end.
    pub fn cursor_end(&mut self) {
        self.cursor = self.edit_value.len();
    }

    // ── Callbacks from host ────────────────────

    /// Called by the host after a file is loaded.
    pub fn file_loaded(&mut self, data: serde_json::Value) {
        self.config_data = data;
        self.selected_field = 0;
    }

    /// Called by the host after a field is saved.
    pub fn field_saved(&mut self, key: &str, value: &serde_json::Value) {
        // Update in-memory config data.
        if let Some(obj) = self.config_data.as_object_mut() {
            obj.insert(key.to_string(), value.clone());
        }
        self.saved_indicator = Some(Instant::now());
        // Go back to field list after saving.
        self.level = SettingsLevel::FieldList;
        self.edit_value.clear();
    }

    /// Called by the host when an error occurs.
    pub fn set_error(&mut self, message: String) {
        self.error = Some(message);
    }

    // ── Query methods ──────────────────────────

    /// Returns `true` if the saved indicator should be visible.
    pub fn is_saved_visible(&self) -> bool {
        match self.saved_indicator {
            Some(instant) => instant.elapsed().as_secs_f64() < SAVED_INDICATOR_DURATION_SECS,
            None => false,
        }
    }

    /// Returns the currently selected config file name.
    pub fn selected_file_name(&self) -> &str {
        CONFIG_FILES
            .get(self.selected_file)
            .map(|(name, _, _)| *name)
            .unwrap_or("config.json")
    }

    /// Returns the currently selected config file label.
    pub fn selected_file_label(&self) -> &str {
        CONFIG_FILES
            .get(self.selected_file)
            .map(|(_, label, _)| *label)
            .unwrap_or("General")
    }

    /// Returns the scope of the currently selected config file.
    pub fn selected_file_scope(&self) -> ConfigScope {
        CONFIG_FILES
            .get(self.selected_file)
            .map(|(_, _, scope)| *scope)
            .unwrap_or(ConfigScope::Global)
    }

    /// Returns the schema for the currently selected file.
    pub fn current_file_schema(&self) -> Option<ConfigFileSchema> {
        get_config_schema(self.selected_file_name())
    }

    // ── Internal helpers ───────────────────────

    /// Number of fields in the current file (from loaded data or schema).
    fn field_count(&self) -> usize {
        // Use loaded data keys if available, fall back to schema.
        if let Some(obj) = self.config_data.as_object() {
            if !obj.is_empty() {
                return obj.len();
            }
        }
        self.current_file_schema()
            .map(|s| s.fields.len())
            .unwrap_or(0)
    }

    /// Get the current field value as a string for the edit buffer.
    fn current_field_value(&self) -> String {
        if let Some(schema) = self.current_file_schema() {
            if let Some(field_schema) = schema.fields.get(self.selected_field) {
                // Try loaded data first, then default.
                let value = self
                    .config_data
                    .as_object()
                    .and_then(|obj| obj.get(&field_schema.key))
                    .unwrap_or(&field_schema.default_value);
                return value_to_edit_string(value);
            }
        }
        // Fallback: get from raw data by index.
        if let Some(obj) = self.config_data.as_object() {
            let keys: Vec<_> = obj.keys().collect();
            if let Some(key) = keys.get(self.selected_field) {
                if let Some(val) = obj.get(*key) {
                    return value_to_edit_string(val);
                }
            }
        }
        String::new()
    }

    /// Get the current select index for a Select field.
    fn current_select_index(&self) -> usize {
        if let Some(schema) = self.current_file_schema() {
            if let Some(field_schema) = schema.fields.get(self.selected_field) {
                if let FieldType::Select { options } = &field_schema.field_type {
                    let current = self.current_field_value();
                    return options
                        .iter()
                        .position(|o| o == &current)
                        .unwrap_or(0);
                }
            }
        }
        0
    }

    /// Build a SaveField action from the current edit state.
    fn save_current_field(&self) -> SettingsAction {
        let (filename, _, scope) = CONFIG_FILES[self.selected_file];

        // Determine the key and typed value.
        if let Some(schema) = self.current_file_schema() {
            if let Some(field_schema) = schema.fields.get(self.selected_field) {
                let value = parse_edit_value(&self.edit_value, &field_schema.field_type);
                return SettingsAction::SaveField {
                    filename: filename.to_string(),
                    scope,
                    key: field_schema.key.clone(),
                    value,
                };
            }
        }

        // Fallback: raw key from data.
        if let Some(obj) = self.config_data.as_object() {
            let keys: Vec<_> = obj.keys().collect();
            if let Some(key) = keys.get(self.selected_field) {
                return SettingsAction::SaveField {
                    filename: filename.to_string(),
                    scope,
                    key: key.to_string(),
                    value: serde_json::Value::String(self.edit_value.clone()),
                };
            }
        }

        SettingsAction::None
    }
}

impl Default for SettingsFormState {
    fn default() -> Self {
        Self::new()
    }
}

// ── Value conversion ────────────────────────────────────────

/// Convert a JSON value to an editable string.
fn value_to_edit_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => String::new(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

/// Parse an edit string back to a typed JSON value based on field type.
fn parse_edit_value(input: &str, field_type: &FieldType) -> serde_json::Value {
    match field_type {
        FieldType::Boolean => {
            serde_json::Value::Bool(input == "true")
        }
        FieldType::Number => {
            if let Ok(n) = input.parse::<f64>() {
                serde_json::json!(n)
            } else if let Ok(n) = input.parse::<i64>() {
                serde_json::json!(n)
            } else {
                serde_json::Value::String(input.to_string())
            }
        }
        FieldType::Text | FieldType::FilePath => {
            if input.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::Value::String(input.to_string())
            }
        }
        FieldType::Select { .. } => {
            serde_json::Value::String(input.to_string())
        }
    }
}

// ── Tests ───────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Construction ─────────────────────────────
    #[test]
    fn new_defaults() {
        let s = SettingsFormState::new();
        assert_eq!(s.level, SettingsLevel::FileList);
        assert_eq!(s.selected_file, 0);
        assert_eq!(s.selected_field, 0);
        assert!(s.edit_value.is_empty());
        assert_eq!(s.cursor, 0);
        assert_eq!(s.config_data, serde_json::Value::Null);
        assert!(s.saved_indicator.is_none());
        assert!(s.error.is_none());
    }

    #[test]
    fn default_equals_new() {
        let a = SettingsFormState::new();
        let b = SettingsFormState::default();
        assert_eq!(a.level, b.level);
        assert_eq!(a.selected_file, b.selected_file);
    }

    // ── CONFIG_FILES ─────────────────────────────
    #[test]
    fn config_files_has_8_entries() {
        assert_eq!(CONFIG_FILES.len(), 8);
    }

    #[test]
    fn config_files_scopes_correct() {
        assert_eq!(CONFIG_FILES[0].2, ConfigScope::Global); // config.json
        assert_eq!(CONFIG_FILES[6].2, ConfigScope::Project); // settings.json
        assert_eq!(CONFIG_FILES[7].2, ConfigScope::Project); // prompts.json
    }

    // ── Navigation ──────────────────────────────
    #[test]
    fn move_up_clamps_at_zero() {
        let mut s = SettingsFormState::new();
        s.move_up();
        assert_eq!(s.selected_file, 0);
    }

    #[test]
    fn move_down_increments() {
        let mut s = SettingsFormState::new();
        s.move_down();
        assert_eq!(s.selected_file, 1);
    }

    #[test]
    fn move_down_clamps_at_max() {
        let mut s = SettingsFormState::new();
        for _ in 0..20 {
            s.move_down();
        }
        assert_eq!(s.selected_file, CONFIG_FILES.len() - 1);
    }

    #[test]
    fn move_up_in_field_list() {
        let mut s = SettingsFormState::new();
        s.level = SettingsLevel::FieldList;
        s.selected_field = 3;
        s.move_up();
        assert_eq!(s.selected_field, 2);
    }

    // ── Enter/Back ──────────────────────────────
    #[test]
    fn enter_at_file_list_returns_load_file() {
        let mut s = SettingsFormState::new();
        s.selected_file = 0; // config.json
        let action = s.enter();
        assert_eq!(s.level, SettingsLevel::FieldList);
        assert!(matches!(action, SettingsAction::LoadFile { filename, scope }
            if filename == "config.json" && scope == ConfigScope::Global));
    }

    #[test]
    fn enter_at_field_list_goes_to_editing() {
        let mut s = SettingsFormState::new();
        s.level = SettingsLevel::FieldList;
        s.config_data = serde_json::json!({"logLevel": "warn"});
        let action = s.enter();
        assert_eq!(s.level, SettingsLevel::Editing);
        assert_eq!(action, SettingsAction::None);
    }

    #[test]
    fn enter_at_editing_returns_save_field() {
        let mut s = SettingsFormState::new();
        s.selected_file = 0; // config.json
        s.level = SettingsLevel::Editing;
        s.selected_field = 0; // logLevel
        s.edit_value = "debug".to_string();
        let action = s.enter();
        assert!(matches!(action, SettingsAction::SaveField { key, .. } if key == "logLevel"));
    }

    #[test]
    fn back_from_editing_goes_to_field_list() {
        let mut s = SettingsFormState::new();
        s.level = SettingsLevel::Editing;
        s.edit_value = "some value".to_string();
        let action = s.back();
        assert_eq!(s.level, SettingsLevel::FieldList);
        assert!(s.edit_value.is_empty());
        assert_eq!(action, SettingsAction::None);
    }

    #[test]
    fn back_from_field_list_goes_to_file_list() {
        let mut s = SettingsFormState::new();
        s.level = SettingsLevel::FieldList;
        let action = s.back();
        assert_eq!(s.level, SettingsLevel::FileList);
        assert_eq!(action, SettingsAction::None);
    }

    #[test]
    fn back_from_field_list_clears_config_data() {
        let mut s = SettingsFormState::new();
        s.level = SettingsLevel::FieldList;
        s.config_data = serde_json::json!({"key": "value"});
        s.back();
        assert_eq!(s.config_data, serde_json::Value::Null);
    }

    #[test]
    fn back_from_file_list_returns_dismiss() {
        let mut s = SettingsFormState::new();
        let action = s.back();
        assert_eq!(action, SettingsAction::Dismiss);
    }

    // ── Edit buffer ─────────────────────────────
    #[test]
    fn type_char_and_backspace() {
        let mut s = SettingsFormState::new();
        s.level = SettingsLevel::Editing;
        s.type_char('h');
        s.type_char('i');
        assert_eq!(s.edit_value, "hi");
        assert_eq!(s.cursor, 2);
        s.backspace();
        assert_eq!(s.edit_value, "h");
        assert_eq!(s.cursor, 1);
    }

    #[test]
    fn type_char_ignored_outside_editing() {
        let mut s = SettingsFormState::new();
        s.type_char('x');
        assert!(s.edit_value.is_empty());
    }

    #[test]
    fn cursor_movement() {
        let mut s = SettingsFormState::new();
        s.level = SettingsLevel::Editing;
        s.edit_value = "hello".to_string();
        s.cursor = 5;
        s.cursor_left();
        assert_eq!(s.cursor, 4);
        s.cursor_home();
        assert_eq!(s.cursor, 0);
        s.cursor_right();
        assert_eq!(s.cursor, 1);
        s.cursor_end();
        assert_eq!(s.cursor, 5);
    }

    #[test]
    fn delete_char() {
        let mut s = SettingsFormState::new();
        s.level = SettingsLevel::Editing;
        s.edit_value = "abc".to_string();
        s.cursor = 1;
        s.delete();
        assert_eq!(s.edit_value, "ac");
        assert_eq!(s.cursor, 1);
    }

    // ── Toggle ──────────────────────────────────
    #[test]
    fn toggle_boolean_true_to_false() {
        let mut s = SettingsFormState::new();
        s.selected_file = 4; // memory.json
        s.level = SettingsLevel::Editing;
        s.selected_field = 0; // enabled (Boolean)
        s.edit_value = "true".to_string();
        let action = s.toggle();
        assert_eq!(s.edit_value, "false");
        assert!(matches!(action, SettingsAction::SaveField { .. }));
    }

    #[test]
    fn toggle_non_boolean_is_noop() {
        let mut s = SettingsFormState::new();
        s.level = SettingsLevel::Editing;
        s.edit_value = "hello".to_string();
        let action = s.toggle();
        assert_eq!(action, SettingsAction::None);
    }

    // ── Callbacks ───────────────────────────────
    #[test]
    fn file_loaded_sets_data() {
        let mut s = SettingsFormState::new();
        s.level = SettingsLevel::FieldList;
        let data = serde_json::json!({"host": "localhost"});
        s.file_loaded(data.clone());
        assert_eq!(s.config_data, data);
        assert_eq!(s.selected_field, 0);
    }

    #[test]
    fn field_saved_updates_data() {
        let mut s = SettingsFormState::new();
        s.level = SettingsLevel::Editing;
        s.config_data = serde_json::json!({"host": "old"});
        s.edit_value = "new".to_string();
        s.field_saved("host", &serde_json::json!("new"));
        assert_eq!(s.config_data["host"], serde_json::json!("new"));
        assert_eq!(s.level, SettingsLevel::FieldList);
        assert!(s.edit_value.is_empty());
        assert!(s.is_saved_visible());
    }

    // ── Query methods ───────────────────────────
    #[test]
    fn selected_file_name_and_label() {
        let mut s = SettingsFormState::new();
        assert_eq!(s.selected_file_name(), "config.json");
        assert_eq!(s.selected_file_label(), "General");
        s.selected_file = 3;
        assert_eq!(s.selected_file_name(), "embed.json");
        assert_eq!(s.selected_file_label(), "Embedding");
    }

    #[test]
    fn selected_file_scope() {
        let mut s = SettingsFormState::new();
        assert_eq!(s.selected_file_scope(), ConfigScope::Global);
        s.selected_file = 6; // settings.json
        assert_eq!(s.selected_file_scope(), ConfigScope::Project);
    }

    // ── Value conversion ────────────────────────
    #[test]
    fn value_to_edit_string_types() {
        assert_eq!(value_to_edit_string(&serde_json::json!("hello")), "hello");
        assert_eq!(value_to_edit_string(&serde_json::json!(42)), "42");
        assert_eq!(value_to_edit_string(&serde_json::json!(true)), "true");
        assert_eq!(value_to_edit_string(&serde_json::Value::Null), "");
    }

    #[test]
    fn parse_edit_value_types() {
        assert_eq!(
            parse_edit_value("true", &FieldType::Boolean),
            serde_json::json!(true)
        );
        assert_eq!(
            parse_edit_value("42", &FieldType::Number),
            serde_json::json!(42.0)
        );
        assert_eq!(
            parse_edit_value("hello", &FieldType::Text),
            serde_json::json!("hello")
        );
        assert_eq!(
            parse_edit_value("", &FieldType::Text),
            serde_json::Value::Null
        );
    }

    // ── Full workflow ───────────────────────────
    #[test]
    fn full_workflow() {
        let mut s = SettingsFormState::new();

        // Start at FileList.
        assert_eq!(s.level, SettingsLevel::FileList);

        // Navigate to memory.json (index 4).
        for _ in 0..4 {
            s.move_down();
        }
        assert_eq!(s.selected_file_name(), "memory.json");

        // Enter → LoadFile action.
        let action = s.enter();
        assert!(matches!(action, SettingsAction::LoadFile { filename, .. } if filename == "memory.json"));
        assert_eq!(s.level, SettingsLevel::FieldList);

        // Simulate host loading file.
        s.file_loaded(serde_json::json!({
            "enabled": true,
            "similarityThreshold": 0.7,
            "maxResults": 10
        }));

        // Enter field → Editing.
        let action = s.enter();
        assert_eq!(s.level, SettingsLevel::Editing);
        assert_eq!(action, SettingsAction::None);

        // Toggle boolean.
        assert_eq!(s.edit_value, "true");
        let action = s.toggle();
        assert_eq!(s.edit_value, "false");
        assert!(matches!(action, SettingsAction::SaveField { .. }));

        // Simulate host saving.
        s.field_saved("enabled", &serde_json::json!(false));
        assert_eq!(s.level, SettingsLevel::FieldList);
        assert!(s.is_saved_visible());

        // Back to file list.
        s.back();
        assert_eq!(s.level, SettingsLevel::FileList);

        // Back again = dismiss.
        let action = s.back();
        assert_eq!(action, SettingsAction::Dismiss);
    }
}
```

**Step 2: Export from mod.rs**

Update `simse-ui-core/src/config/mod.rs` to:

```rust
//! Configuration schema and settings.

pub mod settings_schema;
pub mod settings_state;
pub mod storage;
```

**Step 3: Run tests**

Run: `cd simse-ui-core && cargo test config::settings_state`
Expected: All tests pass.

**Step 4: Commit**

```bash
git add simse-ui-core/src/config/settings_state.rs simse-ui-core/src/config/mod.rs
git commit -m "feat(simse-ui-core): add SettingsFormState state machine"
```

---

### Task 4: Add mcp.json and prompts.json schemas

**Files:**
- Modify: `simse-ui-core/src/config/settings_schema.rs`

**Step 1: Add mcp.json schema**

Add after `settings_json_schema()`:

```rust
fn mcp_json_schema() -> ConfigFileSchema {
    ConfigFileSchema {
        filename: "mcp.json".to_string(),
        description: "MCP server configuration".to_string(),
        fields: vec![
            field(
                "servers",
                "Servers",
                "MCP server entries (array)",
                FieldType::Text, // Array editing handled at ArrayEntry level
                serde_json::json!([]),
                "mcp",
            ),
        ],
    }
}

fn prompts_json_schema() -> ConfigFileSchema {
    ConfigFileSchema {
        filename: "prompts.json".to_string(),
        description: "Named prompt templates and chain definitions".to_string(),
        fields: vec![
            field(
                "prompts",
                "Prompts",
                "Named prompt entries (object)",
                FieldType::Text, // Object editing handled at ArrayEntry level
                serde_json::json!({}),
                "prompts",
            ),
        ],
    }
}
```

**Step 2: Update `all_config_schemas` and `get_config_schema`**

Update `all_config_schemas()` to return 8 schemas (add `mcp_json_schema()` and `prompts_json_schema()`).

Update `get_config_schema()` to handle `"mcp.json"` and `"prompts.json"`.

**Step 3: Update tests**

Change `all_config_schemas_present` to assert `schemas.len() == 8` and add assertions for the new schemas.

**Step 4: Run tests**

Run: `cd simse-ui-core && cargo test config::settings_schema`
Expected: All tests pass.

**Step 5: Commit**

```bash
git add simse-ui-core/src/config/settings_schema.rs
git commit -m "feat(simse-ui-core): add mcp.json and prompts.json schemas"
```

---

### Task 5: Implement FileConfigStorage in simse-tui

**Files:**
- Modify: `simse-tui/src/config.rs` (add `FileConfigStorage` struct at the end)
- Modify: `simse-tui/Cargo.toml` (if `async-trait` not already present)

**Step 1: Add `FileConfigStorage` implementation**

At the bottom of `simse-tui/src/config.rs` (before the `#[cfg(test)] mod tests` block), add:

```rust
use async_trait::async_trait;
use simse_ui_core::config::storage::{ConfigError, ConfigResult, ConfigScope, ConfigStorage};

/// File-based config storage for the TUI.
///
/// Reads/writes JSON config files from:
/// - Global scope: `data_dir/` (e.g. `~/.config/simse/`)
/// - Project scope: `work_dir/.simse/`
pub struct FileConfigStorage {
    pub data_dir: PathBuf,
    pub work_dir: PathBuf,
}

impl FileConfigStorage {
    pub fn new(data_dir: PathBuf, work_dir: PathBuf) -> Self {
        Self { data_dir, work_dir }
    }

    fn dir_for_scope(&self, scope: ConfigScope) -> PathBuf {
        match scope {
            ConfigScope::Global => self.data_dir.clone(),
            ConfigScope::Project => self.work_dir.join(".simse"),
        }
    }
}

#[async_trait]
impl ConfigStorage for FileConfigStorage {
    async fn load_file(&self, filename: &str, scope: ConfigScope) -> ConfigResult<serde_json::Value> {
        let path = self.dir_for_scope(scope).join(filename);
        let content = tokio::fs::read_to_string(&path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ConfigError::NotFound {
                    filename: filename.to_string(),
                }
            } else {
                ConfigError::IoError(e.to_string())
            }
        })?;
        serde_json::from_str(&content).map_err(|e| ConfigError::ParseError {
            filename: filename.to_string(),
            detail: e.to_string(),
        })
    }

    async fn save_file(
        &self,
        filename: &str,
        scope: ConfigScope,
        data: &serde_json::Value,
    ) -> ConfigResult<()> {
        let dir = self.dir_for_scope(scope);
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|e| ConfigError::IoError(e.to_string()))?;

        let path = dir.join(filename);
        let content = serde_json::to_string_pretty(data)
            .map_err(|e| ConfigError::IoError(e.to_string()))?;

        // Atomic write: write to temp file then rename.
        let tmp_path = path.with_extension("tmp");
        tokio::fs::write(&tmp_path, content.as_bytes())
            .await
            .map_err(|e| ConfigError::IoError(e.to_string()))?;
        tokio::fs::rename(&tmp_path, &path)
            .await
            .map_err(|e| ConfigError::IoError(e.to_string()))?;

        Ok(())
    }

    async fn file_exists(&self, filename: &str, scope: ConfigScope) -> bool {
        let path = self.dir_for_scope(scope).join(filename);
        tokio::fs::metadata(&path).await.is_ok()
    }

    async fn delete_file(&self, filename: &str, scope: ConfigScope) -> ConfigResult<()> {
        let path = self.dir_for_scope(scope).join(filename);
        tokio::fs::remove_file(&path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ConfigError::NotFound {
                    filename: filename.to_string(),
                }
            } else {
                ConfigError::IoError(e.to_string())
            }
        })
    }

    async fn delete_all(&self, scope: ConfigScope) -> ConfigResult<()> {
        let dir = self.dir_for_scope(scope);
        if tokio::fs::metadata(&dir).await.is_ok() {
            tokio::fs::remove_dir_all(&dir)
                .await
                .map_err(|e| ConfigError::IoError(e.to_string()))?;
        }
        Ok(())
    }

    async fn ensure_dir(&self, scope: ConfigScope) -> ConfigResult<()> {
        let dir = self.dir_for_scope(scope);
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|e| ConfigError::IoError(e.to_string()))?;
        Ok(())
    }
}
```

**Step 2: Add unit tests for FileConfigStorage**

Add `#[cfg(test)]` tests at the bottom of the file:

```rust
#[cfg(test)]
mod file_config_storage_tests {
    use super::*;
    use simse_ui_core::config::storage::{ConfigScope, ConfigStorage};
    use tempfile::TempDir;

    fn make_storage() -> (FileConfigStorage, TempDir) {
        let tmp = TempDir::new().unwrap();
        let data_dir = tmp.path().join("global");
        let work_dir = tmp.path().join("work");
        std::fs::create_dir_all(&data_dir).unwrap();
        std::fs::create_dir_all(&work_dir).unwrap();
        (FileConfigStorage::new(data_dir, work_dir), tmp)
    }

    #[tokio::test]
    async fn save_and_load() {
        let (storage, _tmp) = make_storage();
        let data = serde_json::json!({"key": "value"});
        storage.save_file("test.json", ConfigScope::Global, &data).await.unwrap();
        let loaded = storage.load_file("test.json", ConfigScope::Global).await.unwrap();
        assert_eq!(loaded, data);
    }

    #[tokio::test]
    async fn load_missing_returns_not_found() {
        let (storage, _tmp) = make_storage();
        let err = storage.load_file("missing.json", ConfigScope::Global).await.unwrap_err();
        assert!(matches!(err, ConfigError::NotFound { .. }));
    }

    #[tokio::test]
    async fn file_exists_check() {
        let (storage, _tmp) = make_storage();
        assert!(!storage.file_exists("test.json", ConfigScope::Global).await);
        storage.save_file("test.json", ConfigScope::Global, &serde_json::json!({})).await.unwrap();
        assert!(storage.file_exists("test.json", ConfigScope::Global).await);
    }

    #[tokio::test]
    async fn delete_file_works() {
        let (storage, _tmp) = make_storage();
        storage.save_file("test.json", ConfigScope::Global, &serde_json::json!({})).await.unwrap();
        storage.delete_file("test.json", ConfigScope::Global).await.unwrap();
        assert!(!storage.file_exists("test.json", ConfigScope::Global).await);
    }

    #[tokio::test]
    async fn delete_all_works() {
        let (storage, _tmp) = make_storage();
        storage.save_file("a.json", ConfigScope::Global, &serde_json::json!({})).await.unwrap();
        storage.save_file("b.json", ConfigScope::Global, &serde_json::json!({})).await.unwrap();
        storage.delete_all(ConfigScope::Global).await.unwrap();
        assert!(!storage.file_exists("a.json", ConfigScope::Global).await);
    }

    #[tokio::test]
    async fn ensure_dir_creates_directory() {
        let (storage, _tmp) = make_storage();
        storage.ensure_dir(ConfigScope::Project).await.unwrap();
        let dir = storage.dir_for_scope(ConfigScope::Project);
        assert!(dir.exists());
    }

    #[tokio::test]
    async fn project_scope_uses_simse_subdir() {
        let (storage, _tmp) = make_storage();
        storage.save_file("settings.json", ConfigScope::Project, &serde_json::json!({"key": "val"})).await.unwrap();
        let path = storage.work_dir.join(".simse").join("settings.json");
        assert!(path.exists());
    }
}
```

**Step 3: Run tests**

Run: `cd simse-tui && cargo test file_config_storage`
Expected: All tests pass.

**Step 4: Commit**

```bash
git add simse-tui/src/config.rs
git commit -m "feat(simse-tui): implement FileConfigStorage"
```

---

### Task 6: Wire SettingsFormState into App model

**Files:**
- Modify: `simse-tui/src/app.rs` — Replace `SettingsExplorerState` with `SettingsFormState`, add new messages
- Modify: `simse-tui/src/overlays/settings.rs` — Update rendering to work with `SettingsFormState`

**Step 1: Update App struct**

In `app.rs`, replace imports and fields:

```rust
// Replace:
use crate::overlays::settings::{render_settings_explorer, SettingsExplorerState};
// With:
use simse_ui_core::config::settings_state::{SettingsFormState, SettingsAction, SettingsLevel, CONFIG_FILES};
use crate::overlays::settings::render_settings_form;
```

Replace the fields in `App`:

```rust
// Replace:
pub settings_state: SettingsExplorerState,
pub settings_config_data: serde_json::Value,
// With:
pub settings_state: SettingsFormState,
```

Add new `AppMessage` variants:

```rust
/// Settings file loaded from storage.
SettingsFileLoaded(serde_json::Value),
/// Settings field saved to storage.
SettingsFieldSaved { key: String, value: serde_json::Value },
/// Settings error.
SettingsError(String),
```

**Step 2: Update the `update()` function**

In the `Screen::Settings` match arms, update to use `SettingsFormState` methods:

- `CharInput(c)` → call `settings_state.type_char(c)` (check field type for toggle/cycle)
- `Submit` → call `settings_state.enter()`, handle returned `SettingsAction`
- `Backspace` → call `settings_state.backspace()`
- `HistoryUp` → call `settings_state.move_up()`
- `HistoryDown` → call `settings_state.move_down()`
- `Escape` → call `settings_state.back()`, handle `SettingsAction::Dismiss`
- `CursorLeft` → call `settings_state.cursor_left()`
- `CursorRight` → call `settings_state.cursor_right()`
- `Home` → call `settings_state.cursor_home()`
- `End` → call `settings_state.cursor_end()`
- `Delete` → call `settings_state.delete()`

For `SettingsAction` return values:
- `SettingsAction::LoadFile { .. }` → set `pending_bridge_action` to a new `BridgeAction::LoadConfigFile`
- `SettingsAction::SaveField { .. }` → set `pending_bridge_action` to a new `BridgeAction::SaveConfigField`
- `SettingsAction::Dismiss` → switch screen to `Screen::Chat`
- `SettingsAction::None` → no-op

Handle the new `AppMessage` variants:
- `SettingsFileLoaded(data)` → call `settings_state.file_loaded(data)`
- `SettingsFieldSaved { key, value }` → call `settings_state.field_saved(&key, &value)`
- `SettingsError(msg)` → call `settings_state.set_error(msg)`

**Step 3: Add new BridgeAction variants**

In `simse-tui/src/commands/mod.rs`, add:

```rust
/// Load a config file for the settings UI.
LoadConfigFile { filename: String, scope: ConfigScope },
/// Save a field in a config file from the settings UI.
SaveConfigField { filename: String, scope: ConfigScope, key: String, value: serde_json::Value },
```

**Step 4: Update the view**

In the `view()` function, replace:

```rust
// Replace:
Screen::Settings => {
    render_settings_explorer(frame, area, &app.settings_state, &app.settings_config_data);
}
// With:
Screen::Settings => {
    render_settings_form(frame, area, &app.settings_state);
}
```

**Step 5: Update `OverlayAction::Settings` handler**

```rust
OverlayAction::Settings => {
    app.settings_state = SettingsFormState::new();
    app.screen = Screen::Settings;
}
```

**Step 6: Remove `get_settings_current_value()` helper**

This function is no longer needed — `SettingsFormState.enter()` handles populating the edit buffer internally.

**Step 7: Run tests**

Run: `cd simse-tui && cargo test`
Expected: All tests pass (some existing settings tests may need updates to use new types).

**Step 8: Commit**

```bash
git add simse-tui/src/app.rs simse-tui/src/commands/mod.rs
git commit -m "feat(simse-tui): wire SettingsFormState into App model"
```

---

### Task 7: Update settings overlay rendering

**Files:**
- Modify: `simse-tui/src/overlays/settings.rs` — Update rendering functions

**Step 1: Update rendering for SettingsFormState**

Replace the import of `SettingsExplorerState` with `SettingsFormState` from `simse_ui_core`. The rendering functions should read from `SettingsFormState` instead of receiving separate `config_data` parameter.

Key changes:
- `render_settings_explorer` → `render_settings_form` (takes `&SettingsFormState` instead of `&SettingsExplorerState` + `&serde_json::Value`)
- `SettingsLevel` comes from `simse_ui_core::config::settings_state::SettingsLevel`
- `CONFIG_FILES` comes from `simse_ui_core::config::settings_state::CONFIG_FILES` (now has 3-tuple with scope)
- Config data is in `state.config_data` instead of separate parameter
- Show Select fields with cycling indicator
- Show cursor position in text edit fields
- Show error message if `state.error.is_some()`
- Remove the old `SettingsExplorerState` struct and its impl (replaced by ui-core version)

**Step 2: Update existing rendering tests**

Update all test functions that use `SettingsExplorerState` to use `SettingsFormState`, and remove tests that duplicate ui-core tests. Keep render smoke tests.

**Step 3: Run tests**

Run: `cd simse-tui && cargo test`
Expected: All tests pass.

**Step 4: Commit**

```bash
git add simse-tui/src/overlays/settings.rs
git commit -m "feat(simse-tui): update settings rendering for SettingsFormState"
```

---

### Task 8: Add SettingsAction dispatching via ConfigStorage

**Files:**
- Modify: `simse-tui/src/event_loop.rs` — Add `FileConfigStorage` field, dispatch new BridgeActions
- Modify: `simse-tui/src/main.rs` — Create `FileConfigStorage` and pass to runtime

**Step 1: Add FileConfigStorage to TuiRuntime**

In `event_loop.rs`, add a `config_storage` field to `TuiRuntime`:

```rust
use crate::config::FileConfigStorage;
use simse_ui_core::config::storage::{ConfigScope, ConfigStorage};

// In TuiRuntime:
pub config_storage: FileConfigStorage,
```

Initialize it in `TuiRuntime::new()` using the loaded config's `data_dir` and `work_dir`.

**Step 2: Handle new BridgeAction variants**

In `dispatch_bridge_action()`, add handlers:

```rust
BridgeAction::LoadConfigFile { filename, scope } => {
    match self.config_storage.load_file(&filename, scope).await {
        Ok(data) => return AppMessage::SettingsFileLoaded(data),
        Err(e) => {
            // File doesn't exist yet — return empty object so schema defaults show.
            if matches!(e, ConfigError::NotFound { .. }) {
                return AppMessage::SettingsFileLoaded(serde_json::json!({}));
            }
            return AppMessage::SettingsError(e.to_string());
        }
    }
}
BridgeAction::SaveConfigField { filename, scope, key, value } => {
    // Read-modify-write.
    let mut data = self.config_storage.load_file(&filename, scope).await
        .unwrap_or(serde_json::json!({}));
    simse_ui_core::config::storage::set_field(&mut data, &key, value.clone());
    match self.config_storage.save_file(&filename, scope, &data).await {
        Ok(()) => return AppMessage::SettingsFieldSaved { key, value },
        Err(e) => return AppMessage::SettingsError(e.to_string()),
    }
}
```

**Step 3: Reimplement existing bridge actions via ConfigStorage**

Replace the direct `std::fs` calls in `InitConfig`, `FactoryReset`, `FactoryResetProject` with `config_storage` calls:

```rust
BridgeAction::InitConfig { force } => {
    let exists = self.config_storage.file_exists("settings.json", ConfigScope::Project).await;
    if exists && !force {
        return Ok("Project already initialized. Use --force to overwrite.".into());
    }
    self.config_storage.ensure_dir(ConfigScope::Project).await
        .map_err(|e| RuntimeError::Acp(e.to_string()))?;
    Ok(format!("Initialized project config at {}", self.config_storage.dir_for_scope(ConfigScope::Project).display()))
}

BridgeAction::FactoryReset => {
    self.config_storage.delete_all(ConfigScope::Global).await
        .map_err(|e| RuntimeError::Acp(e.to_string()))?;
    Ok("Factory reset complete. Global configuration removed.".into())
}

BridgeAction::FactoryResetProject => {
    self.config_storage.delete_all(ConfigScope::Project).await
        .map_err(|e| RuntimeError::Acp(e.to_string()))?;
    Ok("Project configuration reset.".into())
}
```

**Step 4: Run tests**

Run: `cd simse-tui && cargo test`
Expected: All tests pass.

**Step 5: Commit**

```bash
git add simse-tui/src/event_loop.rs simse-tui/src/main.rs simse-tui/src/commands/mod.rs
git commit -m "feat(simse-tui): dispatch SettingsActions via FileConfigStorage"
```

---

### Task 9: PTY integration tests for settings persistence

**Files:**
- Create or modify: `simse-tui/tests/pty/config_settings.rs` — PTY tests for end-to-end settings flow
- Modify: `simse-tui/tests/pty/commands_config.rs` — Add new tests for /factory-reset verification

**Step 1: Add PTY test for /settings field editing**

In `config_settings.rs`, add a test that:
1. Spawns simse with a preconfigured data_dir containing `config.json`
2. Opens `/settings` → navigates to `config.json` → navigates to a field
3. Edits the field value → presses Enter
4. Verifies "Saved" indicator appears on screen
5. Reads `config.json` from disk and verifies the value changed

**Step 2: Add PTY test for boolean toggle**

Similar to above but navigates to `memory.json` → `enabled` field → toggles → verifies disk.

**Step 3: Add PTY test for /init verification**

Test that `/init` creates `.simse/` directory and subsequent `/settings` can navigate to project-scoped files.

**Step 4: Add PTY test for /factory-reset verification**

Test that `/factory-reset` → confirm → verifies global config directory is removed from disk.

**Step 5: Run all PTY tests**

Run: `cd simse-tui && cargo test --test pty`
Expected: All tests pass.

**Step 6: Commit**

```bash
git add simse-tui/tests/pty/config_settings.rs simse-tui/tests/pty/commands_config.rs
git commit -m "test(simse-tui): add PTY integration tests for settings persistence"
```

---

### Task 10: Run full test suite and push

**Step 1: Run simse-ui-core tests**

Run: `cd simse-ui-core && cargo test`
Expected: All tests pass.

**Step 2: Run simse-tui tests**

Run: `cd simse-tui && cargo test`
Expected: All tests pass.

**Step 3: Push**

```bash
git push
```

## Files Changed Summary

**New files:**
- `simse-ui-core/src/config/storage.rs` — ConfigStorage trait + helpers
- `simse-ui-core/src/config/settings_state.rs` — SettingsFormState state machine

**Modified files:**
- `simse-ui-core/Cargo.toml` — add async-trait
- `simse-ui-core/src/config/mod.rs` — export new modules
- `simse-ui-core/src/config/settings_schema.rs` — add mcp.json + prompts.json schemas
- `simse-tui/src/config.rs` — add FileConfigStorage impl
- `simse-tui/src/app.rs` — replace SettingsExplorerState with SettingsFormState, new messages
- `simse-tui/src/commands/mod.rs` — new BridgeAction variants
- `simse-tui/src/event_loop.rs` — dispatch settings actions via ConfigStorage
- `simse-tui/src/main.rs` — create FileConfigStorage
- `simse-tui/src/overlays/settings.rs` — render from SettingsFormState
- `simse-tui/tests/pty/config_settings.rs` — new PTY tests
- `simse-tui/tests/pty/commands_config.rs` — enhanced config command tests
