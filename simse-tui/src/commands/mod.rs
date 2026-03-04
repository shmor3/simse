//! Command handlers for the SimSE TUI.
//!
//! Each submodule exposes handler functions that parse command arguments and
//! return `Vec<CommandOutput>`.  Bridge-dependent operations return placeholder
//! `Info` items describing what *would* happen once the bridge is wired.

pub mod ai;
pub mod config;
pub mod files;
pub mod library;
pub mod meta;
pub mod session;
pub mod tools;

/// The result type returned by every command handler.
#[derive(Debug, Clone, PartialEq)]
pub enum CommandOutput {
	/// A successful result message.
	Success(String),
	/// An error message.
	Error(String),
	/// An informational message (dim gray in the UI).
	Info(String),
	/// Tabular data.
	Table {
		headers: Vec<String>,
		rows: Vec<Vec<String>>,
	},
	/// Request the UI to open an overlay.
	OpenOverlay(OverlayAction),
}

/// Overlay actions that a command can request.
#[derive(Debug, Clone, PartialEq)]
pub enum OverlayAction {
	/// Open the settings explorer overlay.
	Settings,
	/// Open the librarian explorer overlay.
	Librarians,
	/// Open the setup wizard, optionally jumping to a preset.
	Setup(Option<String>),
	/// Open the keyboard shortcuts overlay.
	Shortcuts,
}

/// Format a `CommandOutput::Table` as a fixed-width plain-text table.
pub fn format_table(headers: &[String], rows: &[Vec<String>]) -> String {
	if headers.is_empty() {
		return String::new();
	}

	// Determine column widths.
	let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
	for row in rows {
		for (i, cell) in row.iter().enumerate() {
			if i < widths.len() {
				widths[i] = widths[i].max(cell.len());
			}
		}
	}

	let mut out = String::new();

	// Header row.
	for (i, h) in headers.iter().enumerate() {
		if i > 0 {
			out.push_str("  ");
		}
		out.push_str(&format!("{:<width$}", h, width = widths[i]));
	}
	out.push('\n');

	// Separator.
	for (i, w) in widths.iter().enumerate() {
		if i > 0 {
			out.push_str("  ");
		}
		out.push_str(&"-".repeat(*w));
	}
	out.push('\n');

	// Data rows.
	for row in rows {
		for (i, cell) in row.iter().enumerate() {
			if i > 0 {
				out.push_str("  ");
			}
			let w = widths.get(i).copied().unwrap_or(0);
			out.push_str(&format!("{:<width$}", cell, width = w));
		}
		out.push('\n');
	}

	out
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn format_table_empty_headers() {
		assert_eq!(format_table(&[], &[]), "");
	}

	#[test]
	fn format_table_basic() {
		let headers = vec!["Name".into(), "Value".into()];
		let rows = vec![
			vec!["foo".into(), "1".into()],
			vec!["barbaz".into(), "2".into()],
		];
		let table = format_table(&headers, &rows);
		assert!(table.contains("Name"));
		assert!(table.contains("barbaz"));
		assert!(table.contains("---"));
	}

	#[test]
	fn overlay_action_variants() {
		let a = OverlayAction::Settings;
		let b = OverlayAction::Librarians;
		let c = OverlayAction::Setup(Some("ollama".into()));
		let d = OverlayAction::Shortcuts;
		// Ensure Debug works and equality checks pass.
		assert_ne!(a, b);
		assert_ne!(c, d);
		assert_eq!(a, OverlayAction::Settings);
	}

	#[test]
	fn command_output_variants() {
		let s = CommandOutput::Success("ok".into());
		let e = CommandOutput::Error("fail".into());
		let i = CommandOutput::Info("note".into());
		assert_ne!(s, e);
		assert_ne!(e, i);
	}
}
