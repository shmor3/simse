//! Fuzzy Edit — 5-strategy matching engine for file edits.
//!
//! Ports `src/ai/tools/host/fuzzy-edit.ts` to Rust.
//!
//! Strategies tried in order of strictness:
//! 1. Exact match
//! 2. Line-trimmed (trim each line, compare)
//! 3. Whitespace-normalized (collapse internal whitespace)
//! 4. Indentation-flexible (strip common indent, re-indent replacement)
//! 5. Block-anchor + Levenshtein (match first/last line, 30% tolerance)

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Result of a successful fuzzy match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuzzyMatchResult {
	/// The content with the replacement applied.
	pub replaced: String,
	/// The strategy that succeeded.
	pub strategy: String,
}

// ---------------------------------------------------------------------------
// Levenshtein distance (two-row DP)
// ---------------------------------------------------------------------------

/// Compute the Levenshtein edit distance between two strings.
///
/// Uses a space-optimized two-row algorithm.
pub fn levenshtein(a: &str, b: &str) -> usize {
	let a_bytes = a.as_bytes();
	let b_bytes = b.as_bytes();
	let m = a_bytes.len();
	let n = b_bytes.len();

	if m == 0 {
		return n;
	}
	if n == 0 {
		return m;
	}

	let mut prev: Vec<usize> = (0..=n).collect();
	let mut curr: Vec<usize> = vec![0; n + 1];

	for i in 1..=m {
		curr[0] = i;
		for j in 1..=n {
			let cost = if a_bytes[i - 1] == b_bytes[j - 1] {
				0
			} else {
				1
			};
			curr[j] = (prev[j] + 1)
				.min(curr[j - 1] + 1)
				.min(prev[j - 1] + cost);
		}
		std::mem::swap(&mut prev, &mut curr);
	}

	prev[n]
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Split text into lines (by '\n').
fn split_lines(text: &str) -> Vec<&str> {
	text.split('\n').collect()
}

/// Get the common leading whitespace across non-empty lines.
fn get_common_indent(lines: &[&str]) -> String {
	let non_empty: Vec<&&str> = lines.iter().filter(|l| !l.trim().is_empty()).collect();
	if non_empty.is_empty() {
		return String::new();
	}

	// Compute the longest common prefix of leading whitespace across all non-empty lines
	let first = non_empty[0];
	let first_ws_len = first.len() - first.trim_start().len();
	let first_indent = &first[..first_ws_len];
	let mut prefix_len = first_indent.len();

	for line in &non_empty[1..] {
		let ws_len = line.len() - line.trim_start().len();
		let line_indent = &line[..ws_len];
		prefix_len = prefix_len.min(line_indent.len());
		// Compare byte-by-byte up to prefix_len
		for (i, (a, b)) in first_indent.bytes().zip(line_indent.bytes()).enumerate() {
			if i >= prefix_len {
				break;
			}
			if a != b {
				prefix_len = i;
				break;
			}
		}
	}

	first_indent[..prefix_len].to_string()
}

/// Strip the common indent from text, returning (stripped_text, indent_string).
fn strip_common_indent(text: &str) -> (String, String) {
	let lines = split_lines(text);
	let indent = get_common_indent(&lines);
	if indent.is_empty() {
		return (text.to_string(), String::new());
	}

	let stripped: Vec<&str> = lines
		.iter()
		.map(|line| {
			if line.starts_with(&indent) {
				&line[indent.len()..]
			} else {
				line
			}
		})
		.collect();

	(stripped.join("\n"), indent)
}

/// Re-indent text by prepending `indent` to every non-empty line.
fn re_indent(text: &str, indent: &str) -> String {
	if indent.is_empty() {
		return text.to_string();
	}
	split_lines(text)
		.iter()
		.map(|line| {
			if line.trim().is_empty() {
				(*line).to_string()
			} else {
				format!("{}{}", indent, line)
			}
		})
		.collect::<Vec<_>>()
		.join("\n")
}

// ---------------------------------------------------------------------------
// Strategy 1: Exact match
// ---------------------------------------------------------------------------

fn exact_match(content: &str, old_str: &str, new_str: &str) -> Option<FuzzyMatchResult> {
	let index = content.find(old_str)?;

	// Ensure unique match — check no second non-overlapping occurrence
	// Use index + old_str.len() to stay on UTF-8 boundary
	if content[index + old_str.len()..].contains(old_str) {
		return None;
	}

	let replaced =
		String::from(&content[..index]) + new_str + &content[index + old_str.len()..];

	Some(FuzzyMatchResult {
		replaced,
		strategy: "exact".to_string(),
	})
}

// ---------------------------------------------------------------------------
// Strategy 2: Line-trimmed match
// ---------------------------------------------------------------------------

fn line_trimmed_match(content: &str, old_str: &str, new_str: &str) -> Option<FuzzyMatchResult> {
	let content_lines = split_lines(content);
	let old_lines = split_lines(old_str);
	let trimmed_old: Vec<&str> = old_lines.iter().map(|l| l.trim()).collect();

	if old_lines.is_empty() {
		return None;
	}

	let mut match_start: Option<usize> = None;
	let mut match_count = 0;

	if content_lines.len() >= old_lines.len() {
		for i in 0..=(content_lines.len() - old_lines.len()) {
			let mut matches = true;
			for j in 0..old_lines.len() {
				if content_lines[i + j].trim() != trimmed_old[j] {
					matches = false;
					break;
				}
			}
			if matches {
				match_count += 1;
				if match_count > 1 {
					return None;
				}
				match_start = Some(i);
			}
		}
	}

	let start = match_start?;

	let mut result_parts: Vec<&str> = Vec::new();
	for line in &content_lines[..start] {
		result_parts.push(line);
	}
	result_parts.push(new_str);
	for line in &content_lines[start + old_lines.len()..] {
		result_parts.push(line);
	}

	Some(FuzzyMatchResult {
		replaced: result_parts.join("\n"),
		strategy: "line-trimmed".to_string(),
	})
}

// ---------------------------------------------------------------------------
// Strategy 3: Whitespace-normalized match
// ---------------------------------------------------------------------------

/// Collapse runs of whitespace into single spaces, then trim.
fn normalize_whitespace(s: &str) -> String {
	let trimmed = s.trim();
	let mut result = String::with_capacity(trimmed.len());
	let mut in_space = false;
	for ch in trimmed.chars() {
		if ch.is_whitespace() {
			if !in_space {
				result.push(' ');
				in_space = true;
			}
		} else {
			result.push(ch);
			in_space = false;
		}
	}
	result
}

fn whitespace_normalized_match(
	content: &str,
	old_str: &str,
	new_str: &str,
) -> Option<FuzzyMatchResult> {
	let content_lines = split_lines(content);
	let old_lines = split_lines(old_str);
	let normalized_old: Vec<String> = old_lines.iter().map(|l| normalize_whitespace(l)).collect();

	if old_lines.is_empty() {
		return None;
	}

	let mut match_start: Option<usize> = None;
	let mut match_count = 0;

	if content_lines.len() >= old_lines.len() {
		for i in 0..=(content_lines.len() - old_lines.len()) {
			let mut matches = true;
			for j in 0..old_lines.len() {
				if normalize_whitespace(content_lines[i + j]) != normalized_old[j] {
					matches = false;
					break;
				}
			}
			if matches {
				match_count += 1;
				if match_count > 1 {
					return None;
				}
				match_start = Some(i);
			}
		}
	}

	let start = match_start?;

	let mut result_parts: Vec<&str> = Vec::new();
	for line in &content_lines[..start] {
		result_parts.push(line);
	}
	result_parts.push(new_str);
	for line in &content_lines[start + old_lines.len()..] {
		result_parts.push(line);
	}

	Some(FuzzyMatchResult {
		replaced: result_parts.join("\n"),
		strategy: "whitespace-normalized".to_string(),
	})
}

// ---------------------------------------------------------------------------
// Strategy 4: Indentation-flexible match
// ---------------------------------------------------------------------------

fn indentation_flexible_match(
	content: &str,
	old_str: &str,
	new_str: &str,
) -> Option<FuzzyMatchResult> {
	let content_lines = split_lines(content);
	let (stripped_old, _) = strip_common_indent(old_str);
	let old_lines = split_lines(&stripped_old);

	if old_lines.is_empty() {
		return None;
	}

	let mut match_start: Option<usize> = None;
	let mut match_indent = String::new();
	let mut match_count = 0;

	if content_lines.len() >= old_lines.len() {
		for i in 0..=(content_lines.len() - old_lines.len()) {
			let block_lines: Vec<&str> = content_lines[i..i + old_lines.len()].to_vec();
			let block_text = block_lines.join("\n");
			let (stripped_block, indent) = strip_common_indent(&block_text);

			let stripped_block_lines = split_lines(&stripped_block);
			let old_lines_joined: Vec<&str> = old_lines.to_vec();

			if stripped_block_lines == old_lines_joined {
				match_count += 1;
				if match_count > 1 {
					return None;
				}
				match_start = Some(i);
				match_indent = indent;
			}
		}
	}

	let start = match_start?;

	let re_indented = re_indent(new_str, &match_indent);

	let before: Vec<&str> = content_lines[..start].to_vec();
	let after: Vec<&str> = content_lines[start + old_lines.len()..].to_vec();

	let mut result = before.join("\n");
	if !before.is_empty() {
		result.push('\n');
	}
	result.push_str(&re_indented);
	if !after.is_empty() {
		result.push('\n');
		result.push_str(&after.join("\n"));
	}

	Some(FuzzyMatchResult {
		replaced: result,
		strategy: "indentation-flexible".to_string(),
	})
}

// ---------------------------------------------------------------------------
// Strategy 5: Block-anchor + Levenshtein match
// ---------------------------------------------------------------------------

fn block_anchor_levenshtein_match(
	content: &str,
	old_str: &str,
	new_str: &str,
) -> Option<FuzzyMatchResult> {
	let content_lines = split_lines(content);
	let old_lines = split_lines(old_str);

	if old_lines.len() < 2 {
		return None;
	}

	let first_old_trimmed = old_lines[0].trim();
	let last_old_trimmed = old_lines[old_lines.len() - 1].trim();

	if first_old_trimmed.is_empty() || last_old_trimmed.is_empty() {
		return None;
	}

	let tolerance = 0.3_f64;
	let mut match_start: Option<usize> = None;
	let mut match_end: Option<usize> = None;
	let mut match_count = 0;

	for i in 0..content_lines.len() {
		if content_lines[i].trim() != first_old_trimmed {
			continue;
		}

		// Found first-line anchor; search for last-line anchor
		let max_end_exclusive = (i + old_lines.len()
			+ ((old_lines.len() as f64 * 0.5).ceil() as usize))
			.min(content_lines.len());

		let search_start = if !old_lines.is_empty() {
			i + old_lines.len() - 1
		} else {
			i
		};

		for j in search_start..max_end_exclusive {
			if content_lines[j].trim() != last_old_trimmed {
				continue;
			}

			// Check interior via Levenshtein
			let candidate_block = content_lines[i..=j].join("\n");
			let dist = levenshtein(old_str, &candidate_block);
			let max_len = old_str.len().max(candidate_block.len());

			if max_len > 0 && (dist as f64 / max_len as f64) <= tolerance {
				match_count += 1;
				if match_count > 1 {
					return None;
				}
				match_start = Some(i);
				match_end = Some(j);
			}
		}
	}

	let start = match_start?;
	let end = match_end?;

	let before: Vec<&str> = content_lines[..start].to_vec();
	let after: Vec<&str> = content_lines[end + 1..].to_vec();

	let mut result = before.join("\n");
	if !before.is_empty() {
		result.push('\n');
	}
	result.push_str(new_str);
	if !after.is_empty() {
		result.push('\n');
		result.push_str(&after.join("\n"));
	}

	Some(FuzzyMatchResult {
		replaced: result,
		strategy: "block-anchor-levenshtein".to_string(),
	})
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Try 5 matching strategies in order of strictness:
/// 1. Exact match — `content.contains(old_str)`, unique occurrence only
/// 2. Line-trimmed — trim each line, compare line-by-line, unique window only
/// 3. Whitespace-normalized — collapse internal whitespace, compare, unique only
/// 4. Indentation-flexible — strip common indent, match, re-indent replacement
/// 5. Block-anchor + Levenshtein — match first/last lines exactly (trimmed),
///    check interior via Levenshtein distance <= 30% tolerance
pub fn fuzzy_match(content: &str, old_str: &str, new_str: &str) -> Option<FuzzyMatchResult> {
	exact_match(content, old_str, new_str)
		.or_else(|| line_trimmed_match(content, old_str, new_str))
		.or_else(|| whitespace_normalized_match(content, old_str, new_str))
		.or_else(|| indentation_flexible_match(content, old_str, new_str))
		.or_else(|| block_anchor_levenshtein_match(content, old_str, new_str))
}
