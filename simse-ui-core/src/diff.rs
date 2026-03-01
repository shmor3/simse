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

/// A segment of an inline diff (changed or unchanged text).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineDiffSegment {
	pub text: String,
	pub changed: bool,
}

/// Result of computing an inline diff between two strings.
#[derive(Debug, Clone)]
pub struct InlineDiffResult {
	pub old_segments: Vec<InlineDiffSegment>,
	pub new_segments: Vec<InlineDiffSegment>,
}

/// Compute inline diff between two strings by finding common prefix and suffix.
pub fn compute_inline_diff(old: &str, new: &str) -> InlineDiffResult {
	if old == new {
		return InlineDiffResult {
			old_segments: vec![InlineDiffSegment {
				text: old.to_string(),
				changed: false,
			}],
			new_segments: vec![InlineDiffSegment {
				text: new.to_string(),
				changed: false,
			}],
		};
	}
	if old.is_empty() {
		return InlineDiffResult {
			old_segments: vec![],
			new_segments: vec![InlineDiffSegment {
				text: new.to_string(),
				changed: true,
			}],
		};
	}
	if new.is_empty() {
		return InlineDiffResult {
			old_segments: vec![InlineDiffSegment {
				text: old.to_string(),
				changed: true,
			}],
			new_segments: vec![],
		};
	}

	let old_bytes = old.as_bytes();
	let new_bytes = new.as_bytes();

	// Find common prefix length
	let prefix_len = old_bytes
		.iter()
		.zip(new_bytes.iter())
		.take_while(|(a, b)| a == b)
		.count();

	// Find common suffix length (not overlapping with prefix)
	let old_remaining = old_bytes.len() - prefix_len;
	let new_remaining = new_bytes.len() - prefix_len;
	let suffix_len = old_bytes[prefix_len..]
		.iter()
		.rev()
		.zip(new_bytes[prefix_len..].iter().rev())
		.take_while(|(a, b)| a == b)
		.count()
		.min(old_remaining)
		.min(new_remaining);

	let mut old_segments = Vec::new();
	let mut new_segments = Vec::new();

	if prefix_len > 0 {
		let prefix = &old[..prefix_len];
		old_segments.push(InlineDiffSegment {
			text: prefix.to_string(),
			changed: false,
		});
		new_segments.push(InlineDiffSegment {
			text: prefix.to_string(),
			changed: false,
		});
	}

	let old_mid = &old[prefix_len..old.len() - suffix_len];
	let new_mid = &new[prefix_len..new.len() - suffix_len];

	if !old_mid.is_empty() {
		old_segments.push(InlineDiffSegment {
			text: old_mid.to_string(),
			changed: true,
		});
	}
	if !new_mid.is_empty() {
		new_segments.push(InlineDiffSegment {
			text: new_mid.to_string(),
			changed: true,
		});
	}

	if suffix_len > 0 {
		let suffix = &old[old.len() - suffix_len..];
		old_segments.push(InlineDiffSegment {
			text: suffix.to_string(),
			changed: false,
		});
		new_segments.push(InlineDiffSegment {
			text: suffix.to_string(),
			changed: false,
		});
	}

	InlineDiffResult {
		old_segments,
		new_segments,
	}
}

/// Pair contiguous remove/add blocks for inline highlighting.
/// Returns pairs of (removed_text, added_text).
pub fn pair_diff_lines(lines: &[DiffLine]) -> Vec<(String, String)> {
	let mut pairs = Vec::new();
	let mut i = 0;
	while i < lines.len() {
		// Collect contiguous removes
		let mut removes = Vec::new();
		while i < lines.len() {
			if let DiffLine::Removed(ref s) = lines[i] {
				removes.push(s.clone());
				i += 1;
			} else {
				break;
			}
		}

		// Collect contiguous adds
		let mut adds = Vec::new();
		while i < lines.len() {
			if let DiffLine::Added(ref s) = lines[i] {
				adds.push(s.clone());
				i += 1;
			} else {
				break;
			}
		}

		// Pair min(removes, adds)
		let pair_count = removes.len().min(adds.len());
		for j in 0..pair_count {
			pairs.push((removes[j].clone(), adds[j].clone()));
		}

		// Skip context lines
		if removes.is_empty() && adds.is_empty() {
			i += 1;
		}
	}
	pairs
}

/// Count additions and deletions across hunks.
pub fn count_diff_stats(hunks: &[DiffHunk]) -> (usize, usize) {
	let mut additions = 0;
	let mut deletions = 0;
	for hunk in hunks {
		for line in &hunk.lines {
			match line {
				DiffLine::Added(_) => additions += 1,
				DiffLine::Removed(_) => deletions += 1,
				DiffLine::Context(_) => {}
			}
		}
	}
	(additions, deletions)
}

/// Format a unified diff hunk header.
pub fn format_hunk_header(hunk: &DiffHunk) -> String {
	format!(
		"@@ -{},{} +{},{} @@",
		hunk.old_start, hunk.old_count, hunk.new_start, hunk.new_count
	)
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

	#[test]
	fn compute_inline_diff_changed_middle() {
		let result = compute_inline_diff("hello world", "hello rust!");
		// prefix: "hello ", old_mid: "world", new_mid: "rust!"
		assert!(result.old_segments.len() >= 2);
		assert!(!result.old_segments[0].changed); // "hello " unchanged
		assert!(result.old_segments.iter().any(|s| s.changed)); // middle changed
	}

	#[test]
	fn compute_inline_diff_identical() {
		let result = compute_inline_diff("same", "same");
		assert_eq!(result.old_segments.len(), 1);
		assert!(!result.old_segments[0].changed);
	}

	#[test]
	fn compute_inline_diff_empty_old() {
		let result = compute_inline_diff("", "new text");
		assert_eq!(result.new_segments.len(), 1);
		assert!(result.new_segments[0].changed);
		assert!(result.old_segments.is_empty());
	}

	#[test]
	fn compute_inline_diff_empty_new() {
		let result = compute_inline_diff("old text", "");
		assert_eq!(result.old_segments.len(), 1);
		assert!(result.old_segments[0].changed);
		assert!(result.new_segments.is_empty());
	}

	#[test]
	fn pair_diff_lines_basic() {
		let lines = vec![
			DiffLine::Removed("old1".into()),
			DiffLine::Removed("old2".into()),
			DiffLine::Added("new1".into()),
			DiffLine::Added("new2".into()),
		];
		let paired = pair_diff_lines(&lines);
		assert_eq!(paired.len(), 2);
		assert_eq!(paired[0], ("old1".into(), "new1".into()));
		assert_eq!(paired[1], ("old2".into(), "new2".into()));
	}

	#[test]
	fn pair_diff_lines_uneven() {
		let lines = vec![
			DiffLine::Removed("old1".into()),
			DiffLine::Added("new1".into()),
			DiffLine::Added("new2".into()),
		];
		let paired = pair_diff_lines(&lines);
		assert_eq!(paired.len(), 1);
		assert_eq!(paired[0], ("old1".into(), "new1".into()));
	}

	#[test]
	fn pair_diff_lines_with_context() {
		let lines = vec![
			DiffLine::Context("ctx".into()),
			DiffLine::Removed("old".into()),
			DiffLine::Added("new".into()),
			DiffLine::Context("ctx2".into()),
		];
		let paired = pair_diff_lines(&lines);
		assert_eq!(paired.len(), 1);
	}

	#[test]
	fn count_diff_stats_basic() {
		let hunks = vec![DiffHunk {
			old_start: 1,
			old_count: 2,
			new_start: 1,
			new_count: 3,
			lines: vec![
				DiffLine::Context("ctx".into()),
				DiffLine::Removed("old".into()),
				DiffLine::Added("new1".into()),
				DiffLine::Added("new2".into()),
			],
		}];
		let (adds, dels) = count_diff_stats(&hunks);
		assert_eq!(adds, 2);
		assert_eq!(dels, 1);
	}

	#[test]
	fn format_hunk_header_basic() {
		let hunk = DiffHunk {
			old_start: 10,
			old_count: 5,
			new_start: 10,
			new_count: 7,
			lines: vec![],
		};
		assert_eq!(format_hunk_header(&hunk), "@@ -10,5 +10,7 @@");
	}
}
