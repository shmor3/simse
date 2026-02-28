// ---------------------------------------------------------------------------
// Text Search Utilities
// ---------------------------------------------------------------------------
//
// Pure-function helpers for fuzzy matching, token overlap, and other
// text-search primitives used by the VectorStore advanced search.
//
// Ported from the TypeScript implementation in src/text-search.ts.
// ---------------------------------------------------------------------------

use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;

use regex::Regex;

use crate::types::MetadataFilter;

// ---------------------------------------------------------------------------
// Regex cache
// ---------------------------------------------------------------------------

thread_local! {
	static REGEX_CACHE: RefCell<HashMap<String, Option<Regex>>> = RefCell::new(HashMap::new());
}
const REGEX_CACHE_MAX: usize = 64;

fn get_cached_regex(pattern: &str) -> Option<Regex> {
	REGEX_CACHE.with(|cache| {
		let mut cache = cache.borrow_mut();
		if let Some(cached) = cache.get(pattern) {
			return cached.clone();
		}
		let compiled = Regex::new(pattern).ok();
		if cache.len() >= REGEX_CACHE_MAX {
			if let Some(first_key) = cache.keys().next().cloned() {
				cache.remove(&first_key);
			}
		}
		cache.insert(pattern.to_string(), compiled.clone());
		compiled
	})
}

// ---------------------------------------------------------------------------
// Levenshtein distance / similarity
// ---------------------------------------------------------------------------

/// Compute the Levenshtein edit-distance between two strings.
///
/// Uses the classic Wagner-Fischer dynamic-programming algorithm with
/// O(min(a, b)) space.
pub fn levenshtein_distance(a: &str, b: &str) -> usize {
	// Ensure `a` is the shorter string so we only need one row of storage.
	let (a, b) = if a.len() > b.len() { (b, a) } else { (a, b) };

	let a_chars: Vec<char> = a.chars().collect();
	let b_chars: Vec<char> = b.chars().collect();
	let a_len = a_chars.len();
	let b_len = b_chars.len();

	if a_len == 0 {
		return b_len;
	}

	// Previous row of distances (indices 0..a_len).
	let mut prev: Vec<usize> = (0..=a_len).collect();
	let mut curr: Vec<usize> = vec![0; a_len + 1];

	for j in 1..=b_len {
		curr[0] = j;

		for i in 1..=a_len {
			let cost = if a_chars[i - 1] == b_chars[j - 1] {
				0
			} else {
				1
			};
			curr[i] = (curr[i - 1] + 1)
				.min(prev[i] + 1)
				.min(prev[i - 1] + cost);
		}

		std::mem::swap(&mut prev, &mut curr);
	}

	prev[a_len]
}

/// Return a normalised similarity score (0-1) derived from the Levenshtein
/// distance between two strings. 1 means identical, 0 means completely
/// different.
pub fn levenshtein_similarity(a: &str, b: &str) -> f64 {
	let max_len = a.chars().count().max(b.chars().count());
	if max_len == 0 {
		return 1.0; // two empty strings are identical
	}
	1.0 - levenshtein_distance(a, b) as f64 / max_len as f64
}

// ---------------------------------------------------------------------------
// N-gram similarity
// ---------------------------------------------------------------------------

/// Extract character-level n-grams from a string.
///
/// Returns a `HashMap` of n-gram to count. Text is lowercased before
/// extraction. If text is shorter than `n`, the whole text is a single gram.
pub fn ngrams(text: &str, n: usize) -> HashMap<String, usize> {
	let mut result = HashMap::new();
	let lower = text.to_lowercase();
	let chars: Vec<char> = lower.chars().collect();

	if chars.len() < n {
		// The whole string is a single (short) gram.
		*result.entry(lower).or_insert(0) += 1;
		return result;
	}

	for i in 0..=chars.len() - n {
		let gram: String = chars[i..i + n].iter().collect();
		*result.entry(gram).or_insert(0) += 1;
	}

	result
}

/// Compute the Sorensen-Dice coefficient between two strings using
/// character-level n-grams. Returns a value in [0, 1] where 1 indicates
/// identical n-gram sets.
pub fn ngram_similarity(a: &str, b: &str, n: usize) -> f64 {
	if a.is_empty() && b.is_empty() {
		return 1.0;
	}
	if a.is_empty() || b.is_empty() {
		return 0.0;
	}

	let grams_a = ngrams(a, n);
	let grams_b = ngrams(b, n);

	let mut intersection: usize = 0;
	for (gram, count_a) in &grams_a {
		if let Some(count_b) = grams_b.get(gram) {
			intersection += (*count_a).min(*count_b);
		}
	}

	let total_a: usize = grams_a.values().sum();
	let total_b: usize = grams_b.values().sum();

	(2 * intersection) as f64 / (total_a + total_b) as f64
}

// ---------------------------------------------------------------------------
// Tokenisation
// ---------------------------------------------------------------------------

/// Split text into lowercased word tokens, stripping punctuation.
///
/// This is intentionally simple -- no stemming or stop-word removal -- so it
/// stays deterministic and dependency-free.
pub fn tokenize(text: &str) -> Vec<String> {
	text.to_lowercase()
		.chars()
		.map(|c| if c.is_alphanumeric() || c.is_whitespace() { c } else { ' ' })
		.collect::<String>()
		.split_whitespace()
		.filter(|t| !t.is_empty())
		.map(|t| t.to_string())
		.collect()
}

/// Compute a token-overlap similarity score (Jaccard index) between two
/// pieces of text. Returns a value in [0, 1].
pub fn token_overlap_score(a: &str, b: &str) -> f64 {
	let tokens_a: HashSet<String> = tokenize(a).into_iter().collect();
	let tokens_b: HashSet<String> = tokenize(b).into_iter().collect();

	if tokens_a.is_empty() && tokens_b.is_empty() {
		return 1.0;
	}
	if tokens_a.is_empty() || tokens_b.is_empty() {
		return 0.0;
	}

	let intersection = tokens_a.intersection(&tokens_b).count();
	let union = tokens_a.union(&tokens_b).count();

	intersection as f64 / union as f64
}

// ---------------------------------------------------------------------------
// Composite fuzzy score
// ---------------------------------------------------------------------------

/// Compute a combined fuzzy relevance score (0-1) between a query and a
/// candidate string.
///
/// The score blends three signals:
/// 1. Best-window Levenshtein -- slide a window the size of the query
///    over the candidate and take the best normalised edit-distance score.
/// 2. Bigram similarity -- structural character-level overlap.
/// 3. Token overlap -- semantic word-level overlap.
pub fn fuzzy_score(query: &str, candidate: &str) -> f64 {
	let q = query.to_lowercase();
	let c = candidate.to_lowercase();

	if q.is_empty() && c.is_empty() {
		return 1.0;
	}
	if q.is_empty() || c.is_empty() {
		return 0.0;
	}

	let q_chars: Vec<char> = q.chars().collect();
	let c_chars: Vec<char> = c.chars().collect();
	let q_len = q_chars.len();
	let c_len = c_chars.len();

	// Substring containment short-circuit (skip for very short queries).
	if q_len >= 3 && c.contains(&q) {
		return 1.0;
	}

	// Best-window Levenshtein.
	let best_lev: f64;
	if q_len >= c_len {
		best_lev = levenshtein_similarity(&q, &c);
	} else {
		// Slide a window of length q_len (+/- small margin) over c.
		let mut best: f64 = 0.0;
		let window_sizes = [
			q_len,
			(q_len + 1).min(c_len),
			if q_len > 1 { q_len - 1 } else { 1 },
		];
		'outer: for &ws in &window_sizes {
			if ws > c_len {
				continue;
			}
			for start in 0..=c_len - ws {
				let window: String = c_chars[start..start + ws].iter().collect();
				let sim = levenshtein_similarity(&q, &window);
				if sim > best {
					best = sim;
				}
				if best == 1.0 {
					break 'outer;
				}
			}
		}
		best_lev = best;
	}

	// Bigram similarity.
	let bigram_sim = ngram_similarity(&q, &c, 2);

	// Token overlap.
	let token_sim = token_overlap_score(query, candidate);

	// Weighted combination.
	0.4 * best_lev + 0.3 * bigram_sim + 0.3 * token_sim
}

// ---------------------------------------------------------------------------
// Metadata matching
// ---------------------------------------------------------------------------

/// Test whether a metadata record satisfies a single `MetadataFilter`.
pub fn matches_metadata_filter(
	metadata: &HashMap<String, String>,
	filter: &MetadataFilter,
) -> bool {
	let mode = filter.mode.as_deref().unwrap_or("eq");
	let actual = metadata.get(&filter.key);

	match mode {
		"exists" => metadata.contains_key(&filter.key),
		"notExists" => !metadata.contains_key(&filter.key),
		"eq" => {
			if let (Some(actual_val), Some(filter_val)) = (actual, &filter.value) {
				if let Some(s) = filter_val.as_str() {
					actual_val == s
				} else {
					false
				}
			} else {
				false
			}
		}
		"neq" => {
			if let Some(actual_val) = actual {
				if let Some(filter_val) = &filter.value {
					if let Some(s) = filter_val.as_str() {
						actual_val != s
					} else {
						true // filter value isn't a string, so they're not equal
					}
				} else {
					true // no filter value provided, actual exists so neq
				}
			} else {
				false
			}
		}
		"contains" => {
			if let (Some(actual_val), Some(filter_val)) = (actual, &filter.value) {
				if let Some(s) = filter_val.as_str() {
					actual_val.to_lowercase().contains(&s.to_lowercase())
				} else {
					false
				}
			} else {
				false
			}
		}
		"startsWith" => {
			if let (Some(actual_val), Some(filter_val)) = (actual, &filter.value) {
				if let Some(s) = filter_val.as_str() {
					actual_val.to_lowercase().starts_with(&s.to_lowercase())
				} else {
					false
				}
			} else {
				false
			}
		}
		"endsWith" => {
			if let (Some(actual_val), Some(filter_val)) = (actual, &filter.value) {
				if let Some(s) = filter_val.as_str() {
					actual_val.to_lowercase().ends_with(&s.to_lowercase())
				} else {
					false
				}
			} else {
				false
			}
		}
		"regex" => {
			if let (Some(actual_val), Some(filter_val)) = (actual, &filter.value) {
				if let Some(pattern) = filter_val.as_str() {
					if let Some(re) = get_cached_regex(pattern) {
						re.is_match(actual_val)
					} else {
						false
					}
				} else {
					false
				}
			} else {
				false
			}
		}
		"gt" => numeric_compare(actual, &filter.value, |a, b| a > b),
		"gte" => numeric_compare(actual, &filter.value, |a, b| a >= b),
		"lt" => numeric_compare(actual, &filter.value, |a, b| a < b),
		"lte" => numeric_compare(actual, &filter.value, |a, b| a <= b),
		"in" => {
			if let (Some(actual_val), Some(filter_val)) = (actual, &filter.value) {
				if let Some(arr) = filter_val.as_array() {
					arr.iter().any(|v| v.as_str() == Some(actual_val.as_str()))
				} else {
					false
				}
			} else {
				false
			}
		}
		"notIn" => {
			if let (Some(actual_val), Some(filter_val)) = (actual, &filter.value) {
				if let Some(arr) = filter_val.as_array() {
					!arr.iter().any(|v| v.as_str() == Some(actual_val.as_str()))
				} else {
					false
				}
			} else {
				false
			}
		}
		"between" => {
			if let (Some(actual_val), Some(filter_val)) = (actual, &filter.value) {
				if let Some(arr) = filter_val.as_array() {
					if arr.len() != 2 {
						return false;
					}
					let val = match actual_val.parse::<f64>() {
						Ok(v) => v,
						Err(_) => return false,
					};
					let min = match arr[0].as_str().and_then(|s| s.parse::<f64>().ok()) {
						Some(v) => v,
						None => match arr[0].as_f64() {
							Some(v) => v,
							None => return false,
						},
					};
					let max = match arr[1].as_str().and_then(|s| s.parse::<f64>().ok()) {
						Some(v) => v,
						None => match arr[1].as_f64() {
							Some(v) => v,
							None => return false,
						},
					};
					val >= min && val <= max
				} else {
					false
				}
			} else {
				false
			}
		}
		_ => false,
	}
}

/// Helper for numeric comparison modes (gt, gte, lt, lte).
fn numeric_compare(
	actual: Option<&String>,
	filter_value: &Option<serde_json::Value>,
	cmp: fn(f64, f64) -> bool,
) -> bool {
	if let (Some(actual_val), Some(fv)) = (actual, filter_value) {
		let a = match actual_val.parse::<f64>() {
			Ok(v) => v,
			Err(_) => return false,
		};
		let b = if let Some(s) = fv.as_str() {
			match s.parse::<f64>() {
				Ok(v) => v,
				Err(_) => return false,
			}
		} else if let Some(n) = fv.as_f64() {
			n
		} else {
			return false;
		};
		cmp(a, b)
	} else {
		false
	}
}

/// Test whether a metadata record satisfies **all** filters (logical AND).
pub fn matches_all_metadata_filters(
	metadata: &HashMap<String, String>,
	filters: &[MetadataFilter],
) -> bool {
	filters.iter().all(|f| matches_metadata_filter(metadata, f))
}

// ---------------------------------------------------------------------------
// Text scoring dispatch
// ---------------------------------------------------------------------------

/// Score a query against a text value using the specified mode.
///
/// Returns `Some(score)` if the score meets or exceeds the threshold,
/// otherwise `None`.
///
/// Supported modes:
/// - `"fuzzy"` (default) -- composite fuzzy score
/// - `"exact"` -- exact equality
/// - `"substring"` -- case-insensitive substring containment
/// - `"regex"` -- regex match
/// - `"token"` -- token overlap (Jaccard)
pub fn score_text(query: &str, text: &str, mode: &str, threshold: f64) -> Option<f64> {
	match mode {
		"exact" => {
			if query == text {
				Some(1.0)
			} else {
				None
			}
		}
		"substring" => {
			if text.to_lowercase().contains(&query.to_lowercase()) {
				Some(1.0)
			} else {
				None
			}
		}
		"regex" => {
			if let Some(re) = get_cached_regex(query) {
				if re.is_match(text) {
					Some(1.0)
				} else {
					None
				}
			} else {
				None
			}
		}
		"token" => {
			let score = token_overlap_score(query, text);
			if score >= threshold {
				Some(score)
			} else {
				None
			}
		}
		// "fuzzy" or any unknown mode defaults to fuzzy
		_ => {
			let score = fuzzy_score(query, text);
			if score >= threshold {
				Some(score)
			} else {
				None
			}
		}
	}
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;

	// -- Levenshtein distance tests --

	#[test]
	fn levenshtein_kitten_sitting() {
		assert_eq!(levenshtein_distance("kitten", "sitting"), 3);
	}

	#[test]
	fn levenshtein_empty() {
		assert_eq!(levenshtein_distance("", "hello"), 5);
		assert_eq!(levenshtein_distance("hello", ""), 5);
		assert_eq!(levenshtein_distance("", ""), 0);
	}

	#[test]
	fn levenshtein_identical() {
		assert_eq!(levenshtein_distance("test", "test"), 0);
	}

	#[test]
	fn levenshtein_similarity_empty_strings() {
		assert!((levenshtein_similarity("", "") - 1.0).abs() < f64::EPSILON);
	}

	#[test]
	fn levenshtein_similarity_known_values() {
		let sim = levenshtein_similarity("kitten", "sitting");
		// distance=3, max_len=7 => 1 - 3/7 ~= 0.5714
		assert!((sim - (1.0 - 3.0 / 7.0)).abs() < 1e-10);
	}

	// -- N-gram tests --

	#[test]
	fn ngram_basic() {
		let grams = ngrams("hello", 2);
		assert_eq!(grams.get("he"), Some(&1));
		assert_eq!(grams.get("el"), Some(&1));
		assert_eq!(grams.get("ll"), Some(&1));
		assert_eq!(grams.get("lo"), Some(&1));
		assert_eq!(grams.len(), 4);
	}

	#[test]
	fn ngram_short_text() {
		let grams = ngrams("a", 2);
		assert_eq!(grams.get("a"), Some(&1));
		assert_eq!(grams.len(), 1);
	}

	#[test]
	fn ngram_similarity_identical() {
		let sim = ngram_similarity("hello", "hello", 2);
		assert!((sim - 1.0).abs() < f64::EPSILON);
	}

	#[test]
	fn ngram_similarity_different() {
		let sim = ngram_similarity("abcdef", "zyxwvu", 2);
		assert!(sim < 0.1);
	}

	#[test]
	fn ngram_similarity_empty() {
		assert!((ngram_similarity("", "", 2) - 1.0).abs() < f64::EPSILON);
		assert!((ngram_similarity("hello", "", 2)).abs() < f64::EPSILON);
		assert!((ngram_similarity("", "hello", 2)).abs() < f64::EPSILON);
	}

	// -- Tokenize tests --

	#[test]
	fn tokenize_basic() {
		let tokens = tokenize("Hello, World!");
		assert_eq!(tokens, vec!["hello", "world"]);
	}

	#[test]
	fn tokenize_special_chars() {
		let tokens = tokenize("foo-bar_baz@qux.com");
		assert_eq!(tokens, vec!["foo", "bar", "baz", "qux", "com"]);
	}

	#[test]
	fn tokenize_empty() {
		let tokens = tokenize("");
		assert!(tokens.is_empty());
	}

	// -- Token overlap tests --

	#[test]
	fn token_overlap_identical() {
		let score = token_overlap_score("hello world", "hello world");
		assert!((score - 1.0).abs() < f64::EPSILON);
	}

	#[test]
	fn token_overlap_disjoint() {
		let score = token_overlap_score("hello world", "foo bar");
		assert!((score).abs() < f64::EPSILON);
	}

	#[test]
	fn token_overlap_partial() {
		let score = token_overlap_score("hello world", "hello foo");
		// intersection = {hello}, union = {hello, world, foo} => 1/3
		assert!((score - 1.0 / 3.0).abs() < 1e-10);
	}

	#[test]
	fn token_overlap_empty() {
		assert!((token_overlap_score("", "") - 1.0).abs() < f64::EPSILON);
		assert!((token_overlap_score("hello", "")).abs() < f64::EPSILON);
	}

	// -- Fuzzy score tests --

	#[test]
	fn fuzzy_substring_shortcircuit() {
		let score = fuzzy_score("hello", "say hello there");
		assert!((score - 1.0).abs() < f64::EPSILON);
	}

	#[test]
	fn fuzzy_exact_match() {
		let score = fuzzy_score("test string", "test string");
		assert!((score - 1.0).abs() < f64::EPSILON);
	}

	#[test]
	fn fuzzy_different_strings() {
		let score = fuzzy_score("hello world", "zyxwvu qrstuv");
		assert!(score < 0.3);
	}

	#[test]
	fn fuzzy_empty_strings() {
		assert!((fuzzy_score("", "") - 1.0).abs() < f64::EPSILON);
		assert!((fuzzy_score("hello", "")).abs() < f64::EPSILON);
		assert!((fuzzy_score("", "hello")).abs() < f64::EPSILON);
	}

	#[test]
	fn fuzzy_short_query_no_shortcircuit() {
		// Query "ab" is < 3 chars, so no substring short-circuit
		let score = fuzzy_score("ab", "ab");
		// Should still score high via levenshtein + ngram
		assert!(score > 0.5);
	}

	// -- Metadata filter tests --

	fn make_metadata() -> HashMap<String, String> {
		let mut m = HashMap::new();
		m.insert("name".to_string(), "Alice".to_string());
		m.insert("age".to_string(), "30".to_string());
		m.insert("city".to_string(), "New York".to_string());
		m.insert("score".to_string(), "85.5".to_string());
		m
	}

	#[test]
	fn metadata_eq() {
		let m = make_metadata();
		let filter = MetadataFilter {
			key: "name".to_string(),
			value: Some(json!("Alice")),
			mode: Some("eq".to_string()),
		};
		assert!(matches_metadata_filter(&m, &filter));

		let filter2 = MetadataFilter {
			key: "name".to_string(),
			value: Some(json!("Bob")),
			mode: Some("eq".to_string()),
		};
		assert!(!matches_metadata_filter(&m, &filter2));
	}

	#[test]
	fn metadata_eq_default_mode() {
		let m = make_metadata();
		let filter = MetadataFilter {
			key: "name".to_string(),
			value: Some(json!("Alice")),
			mode: None, // defaults to "eq"
		};
		assert!(matches_metadata_filter(&m, &filter));
	}

	#[test]
	fn metadata_neq() {
		let m = make_metadata();
		let filter = MetadataFilter {
			key: "name".to_string(),
			value: Some(json!("Bob")),
			mode: Some("neq".to_string()),
		};
		assert!(matches_metadata_filter(&m, &filter));
	}

	#[test]
	fn metadata_contains() {
		let m = make_metadata();
		let filter = MetadataFilter {
			key: "city".to_string(),
			value: Some(json!("new")),
			mode: Some("contains".to_string()),
		};
		assert!(matches_metadata_filter(&m, &filter));

		let filter2 = MetadataFilter {
			key: "city".to_string(),
			value: Some(json!("YORK")),
			mode: Some("contains".to_string()),
		};
		assert!(matches_metadata_filter(&m, &filter2));
	}

	#[test]
	fn metadata_starts_with() {
		let m = make_metadata();
		let filter = MetadataFilter {
			key: "city".to_string(),
			value: Some(json!("new")),
			mode: Some("startsWith".to_string()),
		};
		assert!(matches_metadata_filter(&m, &filter));
	}

	#[test]
	fn metadata_ends_with() {
		let m = make_metadata();
		let filter = MetadataFilter {
			key: "city".to_string(),
			value: Some(json!("york")),
			mode: Some("endsWith".to_string()),
		};
		assert!(matches_metadata_filter(&m, &filter));
	}

	#[test]
	fn metadata_regex() {
		let m = make_metadata();
		let filter = MetadataFilter {
			key: "name".to_string(),
			value: Some(json!("^Ali")),
			mode: Some("regex".to_string()),
		};
		assert!(matches_metadata_filter(&m, &filter));

		let filter2 = MetadataFilter {
			key: "name".to_string(),
			value: Some(json!("^Bob")),
			mode: Some("regex".to_string()),
		};
		assert!(!matches_metadata_filter(&m, &filter2));
	}

	#[test]
	fn metadata_numeric_gt() {
		let m = make_metadata();
		let filter = MetadataFilter {
			key: "age".to_string(),
			value: Some(json!("25")),
			mode: Some("gt".to_string()),
		};
		assert!(matches_metadata_filter(&m, &filter));

		let filter2 = MetadataFilter {
			key: "age".to_string(),
			value: Some(json!("35")),
			mode: Some("gt".to_string()),
		};
		assert!(!matches_metadata_filter(&m, &filter2));
	}

	#[test]
	fn metadata_numeric_gte() {
		let m = make_metadata();
		let filter = MetadataFilter {
			key: "age".to_string(),
			value: Some(json!("30")),
			mode: Some("gte".to_string()),
		};
		assert!(matches_metadata_filter(&m, &filter));
	}

	#[test]
	fn metadata_numeric_lt() {
		let m = make_metadata();
		let filter = MetadataFilter {
			key: "age".to_string(),
			value: Some(json!("35")),
			mode: Some("lt".to_string()),
		};
		assert!(matches_metadata_filter(&m, &filter));
	}

	#[test]
	fn metadata_numeric_lte() {
		let m = make_metadata();
		let filter = MetadataFilter {
			key: "age".to_string(),
			value: Some(json!("30")),
			mode: Some("lte".to_string()),
		};
		assert!(matches_metadata_filter(&m, &filter));
	}

	#[test]
	fn metadata_in() {
		let m = make_metadata();
		let filter = MetadataFilter {
			key: "name".to_string(),
			value: Some(json!(["Alice", "Bob", "Charlie"])),
			mode: Some("in".to_string()),
		};
		assert!(matches_metadata_filter(&m, &filter));

		let filter2 = MetadataFilter {
			key: "name".to_string(),
			value: Some(json!(["Bob", "Charlie"])),
			mode: Some("in".to_string()),
		};
		assert!(!matches_metadata_filter(&m, &filter2));
	}

	#[test]
	fn metadata_not_in() {
		let m = make_metadata();
		let filter = MetadataFilter {
			key: "name".to_string(),
			value: Some(json!(["Bob", "Charlie"])),
			mode: Some("notIn".to_string()),
		};
		assert!(matches_metadata_filter(&m, &filter));
	}

	#[test]
	fn metadata_between() {
		let m = make_metadata();
		let filter = MetadataFilter {
			key: "score".to_string(),
			value: Some(json!(["80", "90"])),
			mode: Some("between".to_string()),
		};
		assert!(matches_metadata_filter(&m, &filter));

		let filter2 = MetadataFilter {
			key: "score".to_string(),
			value: Some(json!(["90", "100"])),
			mode: Some("between".to_string()),
		};
		assert!(!matches_metadata_filter(&m, &filter2));
	}

	#[test]
	fn metadata_exists_not_exists() {
		let m = make_metadata();
		let filter = MetadataFilter {
			key: "name".to_string(),
			value: None,
			mode: Some("exists".to_string()),
		};
		assert!(matches_metadata_filter(&m, &filter));

		let filter2 = MetadataFilter {
			key: "missing".to_string(),
			value: None,
			mode: Some("exists".to_string()),
		};
		assert!(!matches_metadata_filter(&m, &filter2));

		let filter3 = MetadataFilter {
			key: "missing".to_string(),
			value: None,
			mode: Some("notExists".to_string()),
		};
		assert!(matches_metadata_filter(&m, &filter3));

		let filter4 = MetadataFilter {
			key: "name".to_string(),
			value: None,
			mode: Some("notExists".to_string()),
		};
		assert!(!matches_metadata_filter(&m, &filter4));
	}

	#[test]
	fn metadata_all_filters() {
		let m = make_metadata();
		let filters = vec![
			MetadataFilter {
				key: "name".to_string(),
				value: Some(json!("Alice")),
				mode: Some("eq".to_string()),
			},
			MetadataFilter {
				key: "age".to_string(),
				value: Some(json!("25")),
				mode: Some("gt".to_string()),
			},
		];
		assert!(matches_all_metadata_filters(&m, &filters));

		let filters2 = vec![
			MetadataFilter {
				key: "name".to_string(),
				value: Some(json!("Alice")),
				mode: Some("eq".to_string()),
			},
			MetadataFilter {
				key: "age".to_string(),
				value: Some(json!("35")),
				mode: Some("gt".to_string()),
			},
		];
		assert!(!matches_all_metadata_filters(&m, &filters2));
	}

	#[test]
	fn metadata_missing_key() {
		let m = make_metadata();
		let filter = MetadataFilter {
			key: "nonexistent".to_string(),
			value: Some(json!("value")),
			mode: Some("eq".to_string()),
		};
		assert!(!matches_metadata_filter(&m, &filter));
	}

	// -- score_text tests --

	#[test]
	fn score_text_fuzzy_mode() {
		let result = score_text("hello world", "hello world", "fuzzy", 0.5);
		assert!(result.is_some());
		assert!((result.unwrap() - 1.0).abs() < f64::EPSILON);
	}

	#[test]
	fn score_text_exact_mode() {
		assert!(score_text("hello", "hello", "exact", 0.0).is_some());
		assert!(score_text("hello", "Hello", "exact", 0.0).is_none());
	}

	#[test]
	fn score_text_substring_mode() {
		assert!(score_text("hello", "say hello there", "substring", 0.0).is_some());
		assert!(score_text("HELLO", "say hello there", "substring", 0.0).is_some());
		assert!(score_text("xyz", "say hello there", "substring", 0.0).is_none());
	}

	#[test]
	fn score_text_regex_mode() {
		assert!(score_text("^hel", "hello world", "regex", 0.0).is_some());
		assert!(score_text("^world", "hello world", "regex", 0.0).is_none());
	}

	#[test]
	fn score_text_token_mode() {
		let result = score_text("hello world", "hello world", "token", 0.5);
		assert!(result.is_some());
		assert!((result.unwrap() - 1.0).abs() < f64::EPSILON);
	}

	#[test]
	fn score_text_default_mode() {
		// Unknown mode defaults to fuzzy
		let result = score_text("hello world", "hello world", "unknown_mode", 0.5);
		assert!(result.is_some());
	}

	#[test]
	fn score_text_below_threshold() {
		let result = score_text("hello", "zzzzz", "fuzzy", 0.9);
		assert!(result.is_none());
	}

	// -- Regex cache tests --

	#[test]
	fn regex_cache_valid_pattern() {
		let re = get_cached_regex("^test");
		assert!(re.is_some());
		assert!(re.unwrap().is_match("testing"));
	}

	#[test]
	fn regex_cache_invalid_pattern() {
		let re = get_cached_regex("[invalid");
		assert!(re.is_none());
	}

	#[test]
	fn regex_cache_reuse() {
		let re1 = get_cached_regex("^hello");
		let re2 = get_cached_regex("^hello");
		assert!(re1.is_some());
		assert!(re2.is_some());
	}
}
