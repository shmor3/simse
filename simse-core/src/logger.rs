//! Structured logger with child loggers and shared log level.
//!
//! Thin wrapper around `tracing` that provides:
//! - `LogLevel` enum with ordering (Debug < Info < Warn < Error < None)
//! - `Logger` struct with context string and shared level via `Arc<Mutex<LogLevel>>`
//! - `child()` creates a new Logger with "parent:child" context sharing the same level
//! - `create_noop_logger()` returns a Logger with level=None (filters everything)

use std::sync::{Arc, Mutex};

/// Log level with ordering support.
///
/// Levels are ordered: Debug < Info < Warn < Error < None.
/// Setting level to `Warn` means only `Warn` and `Error` messages pass through.
/// `None` filters all messages (used by the noop logger).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum LogLevel {
    Debug,
    #[default]
    Info,
    Warn,
    Error,
    None,
}

/// Structured logger with hierarchical context and shared log level.
///
/// Parent and child loggers share the same `LogLevel` via `Arc<Mutex<LogLevel>>`,
/// so changing the level on the parent propagates to all children.
#[derive(Clone)]
pub struct Logger {
    context: String,
    level: Arc<Mutex<LogLevel>>,
}

impl Logger {
    /// Create a new root logger with the given context name.
    ///
    /// The default log level is `Info`.
    pub fn new(context: &str) -> Self {
        Self {
            context: context.to_string(),
            level: Arc::new(Mutex::new(LogLevel::default())),
        }
    }

    /// Create a child logger that shares the parent's log level.
    ///
    /// The child's context is formatted as "parent:child".
    pub fn child(&self, name: &str) -> Self {
        Self {
            context: format!("{}:{}", self.context, name),
            level: Arc::clone(&self.level),
        }
    }

    /// Set the shared log level.
    ///
    /// This affects the parent and all children sharing the same level.
    pub fn set_level(&self, new_level: LogLevel) {
        let mut level = self.level.lock().unwrap_or_else(|e| e.into_inner());
        *level = new_level;
    }

    /// Get the current log level.
    pub fn get_level(&self) -> LogLevel {
        let level = self.level.lock().unwrap_or_else(|e| e.into_inner());
        *level
    }

    /// Return the context string for this logger.
    pub fn context(&self) -> &str {
        &self.context
    }

    /// Log a debug message.
    pub fn debug(&self, msg: &str) {
        if self.is_enabled(LogLevel::Debug) {
            tracing::debug!(context = %self.context, "{}", msg);
        }
    }

    /// Log an info message.
    pub fn info(&self, msg: &str) {
        if self.is_enabled(LogLevel::Info) {
            tracing::info!(context = %self.context, "{}", msg);
        }
    }

    /// Log a warning message.
    pub fn warn(&self, msg: &str) {
        if self.is_enabled(LogLevel::Warn) {
            tracing::warn!(context = %self.context, "{}", msg);
        }
    }

    /// Log an error message.
    pub fn error(&self, msg: &str) {
        if self.is_enabled(LogLevel::Error) {
            tracing::error!(context = %self.context, "{}", msg);
        }
    }

    /// Check if a given log level is enabled (i.e., passes the current filter).
    fn is_enabled(&self, msg_level: LogLevel) -> bool {
        let current = self.get_level();
        msg_level >= current
    }
}

/// Create a no-op logger that filters all messages.
///
/// Equivalent to a logger with level set to `None`.
pub fn create_noop_logger() -> Logger {
    let logger = Logger::new("noop");
    logger.set_level(LogLevel::None);
    logger
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_level_is_info() {
        let logger = Logger::new("test");
        assert_eq!(logger.get_level(), LogLevel::Info);
    }

    #[test]
    fn test_child_context_format() {
        let parent = Logger::new("parent");
        let child = parent.child("child");
        assert_eq!(child.context(), "parent:child");
    }

    #[test]
    fn test_nested_children() {
        let root = Logger::new("root");
        let mid = root.child("mid");
        let leaf = mid.child("leaf");
        assert_eq!(leaf.context(), "root:mid:leaf");
    }

    #[test]
    fn test_noop_logger_level() {
        let logger = create_noop_logger();
        assert_eq!(logger.get_level(), LogLevel::None);
    }
}
