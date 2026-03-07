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

    #[test]
    fn config_scope_display() {
        assert_eq!(ConfigScope::Global.to_string(), "global");
        assert_eq!(ConfigScope::Project.to_string(), "project");
    }

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
