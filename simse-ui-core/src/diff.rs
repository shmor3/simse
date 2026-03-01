//! Diff parsing and formatting.

use serde::{Deserialize, Serialize};

/// A single diff hunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffHunk {
	pub old_start: usize,
	pub old_count: usize,
	pub new_start: usize,
	pub new_count: usize,
	pub lines: Vec<DiffLine>,
}

/// A line in a diff hunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiffLine {
	Context(String),
	Added(String),
	Removed(String),
}

/// Parse a unified diff string into hunks.
pub fn parse_unified_diff(diff_text: &str) -> Vec<DiffHunk> {
	let mut hunks = Vec::new();
	let mut current_hunk: Option<DiffHunk> = None;

	for line in diff_text.lines() {
		if line.starts_with("@@") {
			if let Some(hunk) = current_hunk.take() {
				hunks.push(hunk);
			}
			if let Some(hunk) = parse_hunk_header(line) {
				current_hunk = Some(hunk);
			}
		} else if let Some(ref mut hunk) = current_hunk {
			if let Some(stripped) = line.strip_prefix('+') {
				hunk.lines.push(DiffLine::Added(stripped.to_string()));
			} else if let Some(stripped) = line.strip_prefix('-') {
				hunk.lines.push(DiffLine::Removed(stripped.to_string()));
			} else if let Some(stripped) = line.strip_prefix(' ') {
				hunk.lines.push(DiffLine::Context(stripped.to_string()));
			} else {
				hunk.lines.push(DiffLine::Context(line.to_string()));
			}
		}
	}

	if let Some(hunk) = current_hunk {
		hunks.push(hunk);
	}

	hunks
}

fn parse_hunk_header(line: &str) -> Option<DiffHunk> {
	// Parse "@@ -old_start,old_count +new_start,new_count @@"
	let line = line.trim_start_matches("@@ ");
	let parts: Vec<&str> = line.split("@@").next()?.trim().split(' ').collect();
	if parts.len() < 2 {
		return None;
	}

	let old = parse_range(parts[0].trim_start_matches('-'))?;
	let new = parse_range(parts[1].trim_start_matches('+'))?;

	Some(DiffHunk {
		old_start: old.0,
		old_count: old.1,
		new_start: new.0,
		new_count: new.1,
		lines: Vec::new(),
	})
}

fn parse_range(s: &str) -> Option<(usize, usize)> {
	let parts: Vec<&str> = s.split(',').collect();
	let start = parts.first()?.parse().ok()?;
	let count = parts.get(1).and_then(|c| c.parse().ok()).unwrap_or(1);
	Some((start, count))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parse_simple_diff() {
		let diff = "\
@@ -1,3 +1,4 @@
 line1
+added
 line2
 line3";
		let hunks = parse_unified_diff(diff);
		assert_eq!(hunks.len(), 1);
		assert_eq!(hunks[0].lines.len(), 4);
		assert!(matches!(hunks[0].lines[1], DiffLine::Added(ref s) if s == "added"));
	}

	#[test]
	fn parse_empty_diff() {
		let hunks = parse_unified_diff("");
		assert!(hunks.is_empty());
	}
}
