// ---------------------------------------------------------------------------
// Inverted Text Index -- term-level inverted index with BM25 scoring
// ---------------------------------------------------------------------------
//
// Builds an in-memory inverted index mapping terms to document IDs and
// supports Okapi BM25 ranking for full-text search queries.
//
// Ported from the TypeScript implementation in src/inverted-index.ts.
// ---------------------------------------------------------------------------

use std::collections::{HashMap, HashSet};

use crate::text_search::tokenize;

// ---------------------------------------------------------------------------
// Result type
// ---------------------------------------------------------------------------

/// A single BM25 search result: document ID and relevance score.
#[derive(Debug, Clone)]
pub struct BM25Result {
	pub id: String,
	pub score: f64,
}

// ---------------------------------------------------------------------------
// InvertedIndex
// ---------------------------------------------------------------------------

/// An in-memory inverted index that maps terms to the set of document IDs
/// containing them, together with per-document term frequencies and lengths
/// needed for BM25 scoring.
pub struct InvertedIndex {
	/// term -> set of entry IDs
	index: HashMap<String, HashSet<String>>,
	/// entry ID -> token count (document length)
	doc_lengths: HashMap<String, usize>,
	/// term -> (entry ID -> frequency)
	term_freqs: HashMap<String, HashMap<String, usize>>,
	/// sum of all document lengths
	total_doc_length: usize,
}

impl InvertedIndex {
	/// Create an empty inverted index.
	pub fn new() -> Self {
		Self {
			index: HashMap::new(),
			doc_lengths: HashMap::new(),
			term_freqs: HashMap::new(),
			total_doc_length: 0,
		}
	}

	/// Add a single entry to the index.
	///
	/// Tokenizes `text`, updates postings lists, document lengths, and
	/// per-term frequencies.
	pub fn add_entry(&mut self, id: &str, text: &str) {
		let tokens = tokenize(text);
		self.doc_lengths.insert(id.to_string(), tokens.len());
		self.total_doc_length += tokens.len();

		for token in &tokens {
			// Update postings list
			self.index
				.entry(token.clone())
				.or_default()
				.insert(id.to_string());

			// Update term frequency
			let freq = self
				.term_freqs
				.entry(token.clone())
				.or_default()
				.entry(id.to_string())
				.or_insert(0);
			*freq += 1;
		}
	}

	/// Remove an entry from the index by ID and its original text.
	///
	/// Deduplicate tokens before cleaning so each term is processed once.
	pub fn remove_entry(&mut self, id: &str, text: &str) {
		let tokens = tokenize(text);

		// Subtract document length
		if let Some(dl) = self.doc_lengths.remove(id) {
			self.total_doc_length -= dl;
		}

		// Deduplicate tokens so we only clean each term once
		let unique_tokens: HashSet<&String> = tokens.iter().collect();

		for token in unique_tokens {
			// Clean postings list
			if let Some(postings) = self.index.get_mut(token) {
				postings.remove(id);
				if postings.is_empty() {
					self.index.remove(token);
				}
			}

			// Clean term frequencies
			if let Some(freqs) = self.term_freqs.get_mut(token) {
				freqs.remove(id);
				if freqs.is_empty() {
					self.term_freqs.remove(token);
				}
			}
		}
	}

	/// Get all entry IDs that contain the given term (lowercased).
	///
	/// Returns an empty `Vec` if the term is not in the index.
	pub fn get_entries(&self, term: &str) -> Vec<String> {
		let lower = term.to_lowercase();
		match self.index.get(&lower) {
			Some(postings) => postings.iter().cloned().collect(),
			None => Vec::new(),
		}
	}

	/// Search the index with BM25 scoring.
	///
	/// - `k1`: term frequency saturation parameter (typical default: 1.2)
	/// - `b`: document length normalization parameter, 0-1 (typical default: 0.75)
	///
	/// Returns results sorted descending by score. Returns empty if the
	/// query produces no tokens or the index is empty.
	pub fn bm25_search(&self, query: &str, k1: f64, b: f64) -> Vec<BM25Result> {
		let query_tokens = tokenize(query);
		if query_tokens.is_empty() {
			return Vec::new();
		}

		let n = self.doc_lengths.len();
		if n == 0 {
			return Vec::new();
		}

		let avgdl = self.total_doc_length as f64 / n as f64;
		let mut scores: HashMap<String, f64> = HashMap::new();

		for token in &query_tokens {
			let postings = match self.index.get(token) {
				Some(p) => p,
				None => continue,
			};

			let df = postings.len() as f64;
			let idf = ((n as f64 - df + 0.5) / (df + 0.5) + 1.0).ln();

			let freqs = match self.term_freqs.get(token) {
				Some(f) => f,
				None => continue,
			};

			for doc_id in postings {
				let tf = *freqs.get(doc_id).unwrap_or(&0) as f64;
				let dl = *self.doc_lengths.get(doc_id).unwrap_or(&0) as f64;
				let tf_norm = (tf * (k1 + 1.0)) / (tf + k1 * (1.0 - b + b * dl / avgdl));
				let contribution = idf * tf_norm;

				*scores.entry(doc_id.clone()).or_insert(0.0) += contribution;
			}
		}

		let mut results: Vec<BM25Result> = scores
			.into_iter()
			.map(|(id, score)| BM25Result { id, score })
			.collect();

		results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
		results
	}

	/// Remove all entries and reset internal state.
	pub fn clear(&mut self) {
		self.index.clear();
		self.doc_lengths.clear();
		self.term_freqs.clear();
		self.total_doc_length = 0;
	}

	/// Number of indexed documents.
	pub fn document_count(&self) -> usize {
		self.doc_lengths.len()
	}

	/// Average document length in tokens. Returns `0.0` if the index is empty.
	pub fn average_document_length(&self) -> f64 {
		if self.doc_lengths.is_empty() {
			0.0
		} else {
			self.total_doc_length as f64 / self.doc_lengths.len() as f64
		}
	}
}

impl Default for InvertedIndex {
	fn default() -> Self {
		Self::new()
	}
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn add_and_get_entries() {
		let mut idx = InvertedIndex::new();
		idx.add_entry("doc1", "hello world");
		idx.add_entry("doc2", "hello rust");
		let entries = idx.get_entries("hello");
		assert_eq!(entries.len(), 2);
		assert!(entries.contains(&"doc1".to_string()));
		assert!(entries.contains(&"doc2".to_string()));
	}

	#[test]
	fn bm25_basic_ranking() {
		let mut idx = InvertedIndex::new();
		idx.add_entry("doc1", "the quick brown fox");
		idx.add_entry("doc2", "the quick brown fox jumps over the lazy dog");
		idx.add_entry("doc3", "hello world");
		let results = idx.bm25_search("quick brown fox", 1.2, 0.75);
		assert!(results.len() >= 2);
		// doc1 and doc2 should score, doc3 should not
		let ids: Vec<&str> = results.iter().map(|r| r.id.as_str()).collect();
		assert!(ids.contains(&"doc1"));
		assert!(ids.contains(&"doc2"));
		assert!(!ids.contains(&"doc3"));
	}

	#[test]
	fn remove_entry() {
		let mut idx = InvertedIndex::new();
		idx.add_entry("doc1", "hello world");
		idx.add_entry("doc2", "hello rust");
		assert_eq!(idx.document_count(), 2);
		idx.remove_entry("doc1", "hello world");
		assert_eq!(idx.document_count(), 1);
		let entries = idx.get_entries("world");
		assert!(entries.is_empty());
	}

	#[test]
	fn clear_resets() {
		let mut idx = InvertedIndex::new();
		idx.add_entry("doc1", "hello");
		idx.clear();
		assert_eq!(idx.document_count(), 0);
		assert_eq!(idx.average_document_length(), 0.0);
	}

	#[test]
	fn empty_query_returns_empty() {
		let mut idx = InvertedIndex::new();
		idx.add_entry("doc1", "hello");
		let results = idx.bm25_search("", 1.2, 0.75);
		assert!(results.is_empty());
	}

	#[test]
	fn empty_index_returns_empty() {
		let idx = InvertedIndex::new();
		let results = idx.bm25_search("hello", 1.2, 0.75);
		assert!(results.is_empty());
	}

	#[test]
	fn document_count_and_avg_length() {
		let mut idx = InvertedIndex::new();
		idx.add_entry("d1", "one two three");
		idx.add_entry("d2", "four five");
		assert_eq!(idx.document_count(), 2);
		assert!((idx.average_document_length() - 2.5).abs() < 0.01);
	}

	#[test]
	fn bm25_results_sorted_descending() {
		let mut idx = InvertedIndex::new();
		idx.add_entry("doc1", "rust rust rust");
		idx.add_entry("doc2", "rust programming");
		idx.add_entry("doc3", "hello world");
		let results = idx.bm25_search("rust", 1.2, 0.75);
		for i in 1..results.len() {
			assert!(results[i - 1].score >= results[i].score);
		}
	}

	#[test]
	fn get_entries_case_insensitive() {
		let mut idx = InvertedIndex::new();
		idx.add_entry("doc1", "Hello World");
		let entries = idx.get_entries("HELLO");
		assert_eq!(entries.len(), 1);
	}

	#[test]
	fn remove_entry_cleans_term_freqs() {
		let mut idx = InvertedIndex::new();
		idx.add_entry("doc1", "rust rust rust");
		idx.remove_entry("doc1", "rust rust rust");
		assert_eq!(idx.document_count(), 0);
		assert!(idx.get_entries("rust").is_empty());
		// Internal term_freqs should also be cleaned
		assert!(idx.term_freqs.is_empty());
	}

	#[test]
	fn bm25_higher_tf_scores_higher() {
		let mut idx = InvertedIndex::new();
		// doc1 has "rust" three times, doc2 has it once, both same length padding
		idx.add_entry("doc1", "rust rust rust foo");
		idx.add_entry("doc2", "rust foo bar baz");
		let results = idx.bm25_search("rust", 1.2, 0.75);
		assert_eq!(results.len(), 2);
		// doc1 should score higher due to more occurrences of "rust"
		assert_eq!(results[0].id, "doc1");
		assert_eq!(results[1].id, "doc2");
		assert!(results[0].score > results[1].score);
	}

	#[test]
	fn default_trait() {
		let idx = InvertedIndex::default();
		assert_eq!(idx.document_count(), 0);
		assert_eq!(idx.average_document_length(), 0.0);
	}
}
