// ---------------------------------------------------------------------------
// Inverted Text Index — term-level inverted index with BM25 scoring
// ---------------------------------------------------------------------------
//
// Builds an in-memory inverted index mapping terms to document IDs and
// supports Okapi BM25 ranking for full-text search queries.
// No external dependencies.
// ---------------------------------------------------------------------------

import type { VectorEntry } from './types.js';

// ---------------------------------------------------------------------------
// Result / Option types
// ---------------------------------------------------------------------------

export interface BM25Result {
	readonly id: string;
	readonly score: number;
}

export interface BM25Options {
	/** Term frequency saturation parameter. Defaults to `1.2`. */
	readonly k1?: number;
	/** Document length normalization parameter (0–1). Defaults to `0.75`. */
	readonly b?: number;
}

// ---------------------------------------------------------------------------
// Index interface
// ---------------------------------------------------------------------------

export interface InvertedIndex {
	/** Add a single entry to the index. */
	readonly addEntry: (entry: VectorEntry) => void;
	/** Batch-add multiple entries. */
	readonly addEntries: (entries: readonly VectorEntry[]) => void;
	/** Remove an entry from the index by ID and its original text. */
	readonly removeEntry: (id: string, text: string) => void;
	/** Get all entry IDs that contain the given term. */
	readonly getEntries: (term: string) => readonly string[];
	/** Search the index with BM25 scoring. */
	readonly bm25Search: (
		query: string,
		options?: BM25Options,
	) => readonly BM25Result[];
	/** Remove all entries and reset internal state. */
	readonly clear: () => void;
	/** Number of indexed documents. */
	readonly documentCount: number;
	/** Average document length in tokens (0 if empty). */
	readonly averageDocumentLength: number;
}

// ---------------------------------------------------------------------------
// Tokenization
// ---------------------------------------------------------------------------

/**
 * Tokenize text for indexing: lowercase, strip punctuation, split on
 * whitespace, and filter empty tokens.
 */
export function tokenizeForIndex(text: string): string[] {
	return text
		.toLowerCase()
		.replace(/[^\w\s]/g, ' ')
		.split(/\s+/)
		.filter((t) => t.length > 0);
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

/**
 * Create an inverted text index with BM25 search support.
 *
 * Maintains an in-memory mapping from terms to the set of document IDs
 * that contain them, along with per-document term frequencies and lengths
 * needed for BM25 scoring.
 */
export function createInvertedIndex(): InvertedIndex {
	// term -> set of entry IDs
	const index = new Map<string, Set<string>>();
	// entry ID -> token count
	const docLengths = new Map<string, number>();
	// term -> (entry ID -> frequency)
	const termFreqs = new Map<string, Map<string, number>>();
	// sum of all document lengths
	let totalDocLength = 0;

	// -----------------------------------------------------------------------
	// Internal helpers
	// -----------------------------------------------------------------------

	function addSingleEntry(entry: VectorEntry): void {
		const tokens = tokenizeForIndex(entry.text);
		docLengths.set(entry.id, tokens.length);
		totalDocLength += tokens.length;

		for (const token of tokens) {
			// Update postings list
			let postings = index.get(token);
			if (!postings) {
				postings = new Set<string>();
				index.set(token, postings);
			}
			postings.add(entry.id);

			// Update term frequency
			let freqs = termFreqs.get(token);
			if (!freqs) {
				freqs = new Map<string, number>();
				termFreqs.set(token, freqs);
			}
			freqs.set(entry.id, (freqs.get(entry.id) ?? 0) + 1);
		}
	}

	// -----------------------------------------------------------------------
	// Public API
	// -----------------------------------------------------------------------

	function addEntry(entry: VectorEntry): void {
		addSingleEntry(entry);
	}

	function addEntries(entries: readonly VectorEntry[]): void {
		for (const entry of entries) {
			addSingleEntry(entry);
		}
	}

	function removeEntry(id: string, text: string): void {
		const tokens = tokenizeForIndex(text);
		const dl = docLengths.get(id);
		if (dl !== undefined) {
			totalDocLength -= dl;
			docLengths.delete(id);
		}

		// Deduplicate tokens so we only clean each term once
		const uniqueTokens = new Set(tokens);
		for (const token of uniqueTokens) {
			const postings = index.get(token);
			if (postings) {
				postings.delete(id);
				if (postings.size === 0) {
					index.delete(token);
				}
			}

			const freqs = termFreqs.get(token);
			if (freqs) {
				freqs.delete(id);
				if (freqs.size === 0) {
					termFreqs.delete(token);
				}
			}
		}
	}

	function getEntries(term: string): readonly string[] {
		const postings = index.get(term.toLowerCase());
		if (!postings) return [];
		return [...postings];
	}

	function bm25Search(
		query: string,
		options?: BM25Options,
	): readonly BM25Result[] {
		const queryTokens = tokenizeForIndex(query);
		if (queryTokens.length === 0) return [];

		const k1 = options?.k1 ?? 1.2;
		const b = options?.b ?? 0.75;
		const N = docLengths.size;
		if (N === 0) return [];

		const avgdl = totalDocLength / N;
		const scores = new Map<string, number>();

		for (const token of queryTokens) {
			const postings = index.get(token);
			if (!postings) continue;

			const df = postings.size;
			const idf = Math.log((N - df + 0.5) / (df + 0.5) + 1);

			const freqs = termFreqs.get(token);
			if (!freqs) continue;

			for (const docId of postings) {
				const tf = freqs.get(docId) ?? 0;
				const dl = docLengths.get(docId) ?? 0;
				const tfNorm = (tf * (k1 + 1)) / (tf + k1 * (1 - b + (b * dl) / avgdl));
				const contribution = idf * tfNorm;

				scores.set(docId, (scores.get(docId) ?? 0) + contribution);
			}
		}

		const results: BM25Result[] = [];
		for (const [id, score] of scores) {
			results.push({ id, score });
		}

		results.sort((a, b) => b.score - a.score);
		return results;
	}

	function clear(): void {
		index.clear();
		docLengths.clear();
		termFreqs.clear();
		totalDocLength = 0;
	}

	return Object.freeze({
		addEntry,
		addEntries,
		removeEntry,
		getEntries,
		bm25Search,
		clear,
		get documentCount() {
			return docLengths.size;
		},
		get averageDocumentLength() {
			return docLengths.size === 0 ? 0 : totalDocLength / docLengths.size;
		},
	});
}
