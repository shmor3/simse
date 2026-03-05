// ---------------------------------------------------------------------------
// Query DSL — parse a human-friendly query string into SearchOptions
// ---------------------------------------------------------------------------
//
// Supports:
//   topic:path           — filter by topic
//   metadata:key=value   — metadata equals filter
//   "quoted text"        — exact phrase search
//   fuzzy~term           — fuzzy text search
//   score>N              — minimum score threshold
//   plain text           — BM25 search (default)
//
// Ported from the TypeScript implementation in src/query-dsl.ts.
// ---------------------------------------------------------------------------

use serde::{Deserialize, Serialize};

use crate::types::MetadataFilter;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParsedQuery {
	#[serde(rename = "textSearch")]
	pub text_search: Option<TextSearchParsed>,
	#[serde(rename = "topicFilter")]
	pub topic_filter: Option<Vec<String>>,
	#[serde(rename = "metadataFilters")]
	pub metadata_filters: Option<Vec<MetadataFilter>>,
	#[serde(rename = "minScore")]
	pub min_score: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TextSearchParsed {
	pub query: String,
	pub mode: String,
}

// ---------------------------------------------------------------------------
// Tokenizer
// ---------------------------------------------------------------------------

/// Split input on whitespace while preserving quoted strings as single tokens.
/// Unterminated quotes consume the rest of the string.
fn tokenize(input: &str) -> Vec<String> {
	let mut tokens = Vec::new();
	let chars: Vec<char> = input.chars().collect();
	let len = chars.len();
	let mut i = 0;

	while i < len {
		// Skip whitespace
		while i < len && chars[i] == ' ' {
			i += 1;
		}
		if i >= len {
			break;
		}

		if chars[i] == '"' {
			// Quoted token — find closing quote
			let start = i;
			i += 1; // skip opening quote
			let mut closing_idx = None;
			for j in i..len {
				if chars[j] == '"' {
					closing_idx = Some(j);
					break;
				}
			}
			match closing_idx {
				None => {
					// Unterminated quote — take rest of string including opening quote
					let token: String = chars[start..].iter().collect();
					tokens.push(token);
					break;
				}
				Some(close) => {
					let token: String = chars[start..=close].iter().collect();
					tokens.push(token);
					i = close + 1;
				}
			}
		} else {
			// Unquoted token — read until whitespace
			let start = i;
			while i < len && chars[i] != ' ' {
				i += 1;
			}
			let token: String = chars[start..i].iter().collect();
			tokens.push(token);
		}
	}

	tokens
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/// Parse a DSL query string into a structured [`ParsedQuery`].
///
/// Supported syntax:
/// - `topic:path` — filter by topic
/// - `metadata:key=value` — metadata equals filter
/// - `"quoted text"` — exact phrase search
/// - `fuzzy~term` — fuzzy text search
/// - `score>N` — minimum score threshold
/// - Plain text — BM25 search (default)
pub fn parse_query(dsl: &str) -> ParsedQuery {
	let tokens = tokenize(dsl);

	let mut topics: Vec<String> = Vec::new();
	let mut metadata_filters: Vec<MetadataFilter> = Vec::new();
	let mut plain_parts: Vec<String> = Vec::new();

	let mut quoted_text: Option<String> = None;
	let mut fuzzy_text: Option<String> = None;
	let mut min_score: Option<f64> = None;

	for token in &tokens {
		if let Some(value) = token.strip_prefix("topic:") {
			if !value.is_empty() {
				topics.push(value.to_string());
			}
		} else if let Some(rest) = token.strip_prefix("metadata:") {
			if let Some(eq_idx) = rest.find('=') {
				if eq_idx > 0 {
					let key = rest[..eq_idx].to_string();
					let value = rest[eq_idx + 1..].to_string();
					metadata_filters.push(MetadataFilter {
						key,
						value: Some(serde_json::Value::String(value)),
						mode: Some("eq".to_string()),
					});
				}
			}
		} else if token.starts_with('"') {
			// Quoted string — strip surrounding quotes if present
			if token.ends_with('"') && token.len() > 1 {
				quoted_text = Some(token[1..token.len() - 1].to_string());
			} else {
				// Unterminated quote — strip opening quote only
				quoted_text = Some(token[1..].to_string());
			}
		} else if let Some(value) = token.strip_prefix("fuzzy~") {
			if !value.is_empty() {
				fuzzy_text = Some(value.to_string());
			}
		} else if let Some(value) = token.strip_prefix("score>") {
			if let Ok(v) = value.parse::<f64>() {
				min_score = Some(v);
			}
		} else {
			plain_parts.push(token.clone());
		}
	}

	// Determine text_search
	let text_search = if let Some(quoted) = quoted_text {
		Some(TextSearchParsed {
			query: quoted,
			mode: "exact".to_string(),
		})
	} else if let Some(fuzzy) = fuzzy_text {
		Some(TextSearchParsed {
			query: fuzzy,
			mode: "fuzzy".to_string(),
		})
	} else {
		let joined = plain_parts.join(" ");
		Some(TextSearchParsed {
			query: joined,
			mode: "bm25".to_string(),
		})
	};

	ParsedQuery {
		text_search,
		topic_filter: if topics.is_empty() {
			None
		} else {
			Some(topics)
		},
		metadata_filters: if metadata_filters.is_empty() {
			None
		} else {
			Some(metadata_filters)
		},
		min_score,
	}
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parse_plain_text() {
		let q = parse_query("hello world");
		assert_eq!(
			q.text_search,
			Some(TextSearchParsed {
				query: "hello world".to_string(),
				mode: "bm25".to_string(),
			})
		);
		assert!(q.topic_filter.is_none());
		assert!(q.metadata_filters.is_none());
		assert!(q.min_score.is_none());
	}

	#[test]
	fn parse_topic_filter() {
		let q = parse_query("topic:rust/async some query");
		assert_eq!(q.topic_filter, Some(vec!["rust/async".to_string()]));
		assert_eq!(
			q.text_search,
			Some(TextSearchParsed {
				query: "some query".to_string(),
				mode: "bm25".to_string(),
			})
		);
	}

	#[test]
	fn parse_multiple_topics() {
		let q = parse_query("topic:rust topic:go");
		assert_eq!(
			q.topic_filter,
			Some(vec!["rust".to_string(), "go".to_string()])
		);
	}

	#[test]
	fn parse_metadata_filter() {
		let q = parse_query("metadata:author=Alice search terms");
		let filters = q.metadata_filters.unwrap();
		assert_eq!(filters.len(), 1);
		assert_eq!(filters[0].key, "author");
		assert_eq!(
			filters[0].value,
			Some(serde_json::Value::String("Alice".to_string()))
		);
		assert_eq!(filters[0].mode, Some("eq".to_string()));
	}

	#[test]
	fn parse_quoted_exact_search() {
		let q = parse_query("\"exact phrase\"");
		assert_eq!(
			q.text_search,
			Some(TextSearchParsed {
				query: "exact phrase".to_string(),
				mode: "exact".to_string(),
			})
		);
	}

	#[test]
	fn parse_fuzzy_search() {
		let q = parse_query("fuzzy~rustlang");
		assert_eq!(
			q.text_search,
			Some(TextSearchParsed {
				query: "rustlang".to_string(),
				mode: "fuzzy".to_string(),
			})
		);
	}

	#[test]
	fn parse_score_threshold() {
		let q = parse_query("score>0.75 search terms");
		assert_eq!(q.min_score, Some(0.75));
		assert_eq!(
			q.text_search,
			Some(TextSearchParsed {
				query: "search terms".to_string(),
				mode: "bm25".to_string(),
			})
		);
	}

	#[test]
	fn parse_combined_query() {
		let q = parse_query("topic:rust metadata:lang=en score>0.5 hello world");
		assert_eq!(q.topic_filter, Some(vec!["rust".to_string()]));
		assert!(q.metadata_filters.is_some());
		assert_eq!(q.min_score, Some(0.5));
		assert_eq!(
			q.text_search,
			Some(TextSearchParsed {
				query: "hello world".to_string(),
				mode: "bm25".to_string(),
			})
		);
	}

	#[test]
	fn parse_empty_query() {
		let q = parse_query("");
		assert_eq!(
			q.text_search,
			Some(TextSearchParsed {
				query: "".to_string(),
				mode: "bm25".to_string(),
			})
		);
	}

	#[test]
	fn parse_unterminated_quote() {
		let q = parse_query("\"unterminated");
		assert_eq!(
			q.text_search,
			Some(TextSearchParsed {
				query: "unterminated".to_string(),
				mode: "exact".to_string(),
			})
		);
	}

	#[test]
	fn parse_topic_empty_value_ignored() {
		let q = parse_query("topic: other");
		assert!(q.topic_filter.is_none());
		assert_eq!(
			q.text_search,
			Some(TextSearchParsed {
				query: "other".to_string(),
				mode: "bm25".to_string(),
			})
		);
	}

	#[test]
	fn parse_score_invalid_value_ignored() {
		let q = parse_query("score>abc hello");
		assert!(q.min_score.is_none());
		assert_eq!(
			q.text_search,
			Some(TextSearchParsed {
				query: "hello".to_string(),
				mode: "bm25".to_string(),
			})
		);
	}

	#[test]
	fn tokenize_preserves_quoted_strings() {
		let tokens = tokenize("hello \"world of rust\" bye");
		assert_eq!(tokens, vec!["hello", "\"world of rust\"", "bye"]);
	}

	#[test]
	fn quoted_takes_precedence_over_plain() {
		let q = parse_query("plain \"quoted text\" more");
		assert_eq!(
			q.text_search,
			Some(TextSearchParsed {
				query: "quoted text".to_string(),
				mode: "exact".to_string(),
			})
		);
	}
}
