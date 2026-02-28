use regex::Regex;

/// How to match query text against file lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchMode {
	Substring,
	Regex,
}

/// Configuration for a search operation.
#[derive(Debug, Clone)]
pub struct SearchOptions {
	pub max_results: usize,
	pub mode: SearchMode,
	pub context_before: usize,
	pub context_after: usize,
	pub count_only: bool,
}

/// A single search match within a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
	/// The file path where the match was found.
	pub path: String,
	/// 1-indexed line number.
	pub line: usize,
	/// 1-indexed column number.
	pub column: usize,
	/// The matched text.
	pub match_text: String,
	/// Lines before the match (None when empty).
	pub context_before: Option<Vec<String>>,
	/// Lines after the match (None when empty).
	pub context_after: Option<Vec<String>>,
}

/// Result of a search operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchResult {
	Matches(Vec<SearchMatch>),
	Count(usize),
}

/// Search a single file's text content. Accumulates into `results` or increments `count`.
///
/// Returns `true` if `max_results` limit was hit.
///
/// - In `count_only` mode, matches increment `count` without building result objects,
///   and `max_results` is not enforced.
/// - Lines and columns are 1-indexed.
/// - Context arrays are `None` when they would be empty.
pub fn search_text(
	path: &str,
	text: &str,
	query: &str,
	options: &SearchOptions,
	compiled_regex: Option<&Regex>,
	count: &mut usize,
	results: &mut Vec<SearchMatch>,
) -> bool {
	let lines: Vec<&str> = text.lines().collect();

	for (line_idx, line) in lines.iter().enumerate() {
		// Find a match on this line (position + matched text).
		let found = match options.mode {
			SearchMode::Substring => line.find(query).map(|pos| (pos, query.to_string())),
			SearchMode::Regex => {
				if let Some(re) = compiled_regex {
					re.find(line).map(|m| (m.start(), m.as_str().to_string()))
				} else {
					None
				}
			}
		};

		let Some((col_0, match_text)) = found else {
			continue;
		};

		if options.count_only {
			*count += 1;
			continue;
		}

		// Build context before.
		let ctx_start = line_idx.saturating_sub(options.context_before);
		let before: Vec<String> = lines[ctx_start..line_idx]
			.iter()
			.map(|s| s.to_string())
			.collect();
		let context_before = if before.is_empty() { None } else { Some(before) };

		// Build context after.
		let after_start = line_idx + 1;
		let after_end = (line_idx + 1 + options.context_after).min(lines.len());
		let after: Vec<String> = if after_start <= after_end {
			lines[after_start..after_end]
				.iter()
				.map(|s| s.to_string())
				.collect()
		} else {
			Vec::new()
		};
		let context_after = if after.is_empty() { None } else { Some(after) };

		results.push(SearchMatch {
			path: path.to_string(),
			line: line_idx + 1,
			column: col_0 + 1,
			match_text,
			context_before,
			context_after,
		});

		if results.len() >= options.max_results {
			return true;
		}
	}

	false
}

#[cfg(test)]
mod tests {
	use super::*;

	fn default_options() -> SearchOptions {
		SearchOptions {
			max_results: 100,
			mode: SearchMode::Substring,
			context_before: 0,
			context_after: 0,
			count_only: false,
		}
	}

	#[test]
	fn substring_search_finds_match() {
		let text = "hello world\nfoo bar\nhello again";
		let mut results = Vec::new();
		let mut count = 0;
		let opts = default_options();

		let hit_limit = search_text("/test.txt", text, "hello", &opts, None, &mut count, &mut results);

		assert!(!hit_limit);
		assert_eq!(results.len(), 2);

		assert_eq!(results[0].path, "/test.txt");
		assert_eq!(results[0].line, 1);
		assert_eq!(results[0].column, 1);
		assert_eq!(results[0].match_text, "hello");

		assert_eq!(results[1].line, 3);
		assert_eq!(results[1].column, 1);
		assert_eq!(results[1].match_text, "hello");
	}

	#[test]
	fn substring_search_correct_column() {
		let text = "abc hello xyz";
		let mut results = Vec::new();
		let mut count = 0;
		let opts = default_options();

		search_text("/f.txt", text, "hello", &opts, None, &mut count, &mut results);

		assert_eq!(results.len(), 1);
		// "hello" starts at byte index 4, so column = 5 (1-indexed).
		assert_eq!(results[0].column, 5);
	}

	#[test]
	fn regex_search_finds_match() {
		let text = "error 404\nwarning 200\nerror 500";
		let re = Regex::new(r"error \d+").unwrap();
		let mut results = Vec::new();
		let mut count = 0;
		let opts = SearchOptions {
			mode: SearchMode::Regex,
			..default_options()
		};

		let hit_limit = search_text("/log.txt", text, "", &opts, Some(&re), &mut count, &mut results);

		assert!(!hit_limit);
		assert_eq!(results.len(), 2);

		assert_eq!(results[0].line, 1);
		assert_eq!(results[0].match_text, "error 404");
		assert_eq!(results[0].column, 1);

		assert_eq!(results[1].line, 3);
		assert_eq!(results[1].match_text, "error 500");
	}

	#[test]
	fn context_lines_are_captured() {
		let text = "line1\nline2\nline3\nline4\nline5";
		let mut results = Vec::new();
		let mut count = 0;
		let opts = SearchOptions {
			context_before: 2,
			context_after: 1,
			..default_options()
		};

		search_text("/ctx.txt", text, "line3", &opts, None, &mut count, &mut results);

		assert_eq!(results.len(), 1);
		let m = &results[0];
		assert_eq!(m.line, 3);
		assert_eq!(
			m.context_before,
			Some(vec!["line1".to_string(), "line2".to_string()])
		);
		assert_eq!(m.context_after, Some(vec!["line4".to_string()]));
	}

	#[test]
	fn context_none_when_empty() {
		let text = "only line";
		let mut results = Vec::new();
		let mut count = 0;
		let opts = SearchOptions {
			context_before: 3,
			context_after: 3,
			..default_options()
		};

		search_text("/single.txt", text, "only", &opts, None, &mut count, &mut results);

		assert_eq!(results.len(), 1);
		assert_eq!(results[0].context_before, None);
		assert_eq!(results[0].context_after, None);
	}

	#[test]
	fn count_only_returns_count() {
		let text = "aaa\nbbb\naaa\nccc\naaa";
		let mut results = Vec::new();
		let mut count = 0;
		let opts = SearchOptions {
			count_only: true,
			..default_options()
		};

		let hit_limit = search_text("/cnt.txt", text, "aaa", &opts, None, &mut count, &mut results);

		assert!(!hit_limit);
		assert_eq!(count, 3);
		assert!(results.is_empty(), "count_only should not build results");
	}

	#[test]
	fn count_only_ignores_max_results() {
		let text = "x\nx\nx\nx\nx";
		let mut results = Vec::new();
		let mut count = 0;
		let opts = SearchOptions {
			max_results: 2,
			count_only: true,
			..default_options()
		};

		let hit_limit = search_text("/cnt2.txt", text, "x", &opts, None, &mut count, &mut results);

		// count_only does not enforce max_results.
		assert!(!hit_limit);
		assert_eq!(count, 5);
	}

	#[test]
	fn max_results_limit_is_respected() {
		let text = "match\nmatch\nmatch\nmatch\nmatch";
		let mut results = Vec::new();
		let mut count = 0;
		let opts = SearchOptions {
			max_results: 3,
			..default_options()
		};

		let hit_limit = search_text("/limit.txt", text, "match", &opts, None, &mut count, &mut results);

		assert!(hit_limit);
		assert_eq!(results.len(), 3);
	}

	#[test]
	fn context_clamped_at_boundaries() {
		let text = "first\nsecond\nthird";
		let mut results = Vec::new();
		let mut count = 0;
		let opts = SearchOptions {
			context_before: 10,
			context_after: 10,
			..default_options()
		};

		search_text("/edge.txt", text, "second", &opts, None, &mut count, &mut results);

		assert_eq!(results.len(), 1);
		let m = &results[0];
		assert_eq!(m.context_before, Some(vec!["first".to_string()]));
		assert_eq!(m.context_after, Some(vec!["third".to_string()]));
	}
}
