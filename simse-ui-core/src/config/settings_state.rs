//! Pure settings form state machine.
//!
//! Four navigation levels:
//! - Level 0: FileList — 8 config files
//! - Level 1: FieldList — fields/entries for selected file
//! - Level 2: Editing — single field value editing
//! - Level 3: ArrayEntry — fields within an array entry (servers, prompts)

use std::time::Instant;

use crate::config::settings_schema::{get_config_schema, ConfigFileSchema, FieldType};
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
