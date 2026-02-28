// ---------------------------------------------------------------------------
// Text Search Utilities
// ---------------------------------------------------------------------------
//
// Pure-function helpers for fuzzy matching, token overlap, and other
// text-search primitives used by the VectorStore advanced search.
// ---------------------------------------------------------------------------

import type { MetadataFilter } from './types.js';

/**
 * Compute the Levenshtein edit-distance between two strings.
 *
 * Uses the classic Wagner–Fischer dynamic-programming algorithm with
 * O(min(a, b)) space.
 */
export function levenshteinDistance(a: string, b: string): number {
	// Ensure `a` is the shorter string so we only need one row of storage.
	if (a.length > b.length) {
		[a, b] = [b, a];
	}

	const aLen = a.length;
	const bLen = b.length;

	if (aLen === 0) return bLen;

	// Previous row of distances (indices 0..aLen).
	let prev = new Uint32Array(aLen + 1);
	let curr = new Uint32Array(aLen + 1);

	for (let i = 0; i <= aLen; i++) {
		prev[i] = i;
	}

	for (let j = 1; j <= bLen; j++) {
		curr[0] = j;

		for (let i = 1; i <= aLen; i++) {
			const cost = a[i - 1] === b[j - 1] ? 0 : 1;
			curr[i] = Math.min(
				curr[i - 1] + 1, // insertion
				prev[i] + 1, // deletion
				prev[i - 1] + cost, // substitution
			);
		}

		// Swap rows.
		[prev, curr] = [curr, prev];
	}

	return prev[aLen];
}

/**
 * Return a normalised similarity score (0–1) derived from the Levenshtein
 * distance between two strings.  1 means identical, 0 means completely
 * different.
 */
export function levenshteinSimilarity(a: string, b: string): number {
	const maxLen = Math.max(a.length, b.length);
	if (maxLen === 0) return 1; // two empty strings are identical
	return 1 - levenshteinDistance(a, b) / maxLen;
}

// ---------------------------------------------------------------------------
// N-gram similarity
// ---------------------------------------------------------------------------

/**
 * Extract character-level n-grams from a string.
 *
 * @param text  — Input string.
 * @param n     — Size of each gram (defaults to 2 for bigrams).
 * @returns A `Map` of n-gram → count.
 */
export function ngrams(text: string, n = 2): Map<string, number> {
	const result = new Map<string, number>();
	if (text.length < n) {
		// The whole string is a single (short) gram.
		result.set(text.toLowerCase(), 1);
		return result;
	}
	const lower = text.toLowerCase();
	for (let i = 0; i <= lower.length - n; i++) {
		const gram = lower.slice(i, i + n);
		result.set(gram, (result.get(gram) ?? 0) + 1);
	}
	return result;
}

/**
 * Compute the Sørensen–Dice coefficient between two strings using
 * character-level bigrams.  Returns a value in [0, 1] where 1 indicates
 * identical bigram sets.
 */
export function ngramSimilarity(a: string, b: string, n = 2): number {
	if (a.length === 0 && b.length === 0) return 1;
	if (a.length === 0 || b.length === 0) return 0;

	const gramsA = ngrams(a, n);
	const gramsB = ngrams(b, n);

	let intersection = 0;
	for (const [gram, countA] of gramsA) {
		const countB = gramsB.get(gram);
		if (countB !== undefined) {
			intersection += Math.min(countA, countB);
		}
	}

	let totalA = 0;
	for (const count of gramsA.values()) totalA += count;

	let totalB = 0;
	for (const count of gramsB.values()) totalB += count;

	return (2 * intersection) / (totalA + totalB);
}

// ---------------------------------------------------------------------------
// Tokenisation
// ---------------------------------------------------------------------------

/**
 * Split text into lowercased word tokens, stripping punctuation.
 *
 * This is intentionally simple — no stemming or stop-word removal — so it
 * stays deterministic and dependency-free.
 */
export function tokenize(text: string): string[] {
	return text
		.toLowerCase()
		.replace(/[^\p{L}\p{N}\s]/gu, ' ')
		.split(/\s+/)
		.filter((t) => t.length > 0);
}

/**
 * Compute a token-overlap similarity score (Jaccard index) between two
 * pieces of text.  Returns a value in [0, 1].
 */
export function tokenOverlapScore(a: string, b: string): number {
	const tokensA = new Set(tokenize(a));
	const tokensB = new Set(tokenize(b));

	if (tokensA.size === 0 && tokensB.size === 0) return 1;
	if (tokensA.size === 0 || tokensB.size === 0) return 0;

	let intersection = 0;
	for (const t of tokensA) {
		if (tokensB.has(t)) intersection++;
	}

	const union = new Set([...tokensA, ...tokensB]).size;
	return intersection / union;
}

// ---------------------------------------------------------------------------
// Fuzzy scoring configuration
// ---------------------------------------------------------------------------

/**
 * Configurable weights for the composite fuzzy scoring algorithm.
 * All three values should sum to 1.0 for normalised results.
 */
export interface FuzzyScoreWeights {
	/** Weight for best-window Levenshtein similarity. */
	readonly levenshtein?: number;
	/** Weight for character-level bigram (Sørensen–Dice) similarity. */
	readonly bigram?: number;
	/** Weight for word-level token overlap (Jaccard) similarity. */
	readonly token?: number;
}

export interface FuzzyScoreOptions {
	/** Algorithm weights. Defaults to `{ levenshtein: 0.4, bigram: 0.3, token: 0.3 }`. */
	readonly weights?: FuzzyScoreWeights;
	/**
	 * Minimum query length for the substring containment short-circuit.
	 * Queries shorter than this skip the fast-path exact-substring check.
	 * Defaults to `3`.
	 */
	readonly substringMinLength?: number;
}

// ---------------------------------------------------------------------------
// Composite fuzzy score
// ---------------------------------------------------------------------------

/**
 * Compute a combined fuzzy relevance score (0–1) between a query and a
 * candidate string.
 *
 * The score blends three signals:
 * 1. **Best-window Levenshtein** — slide a window the size of the query
 *    over the candidate and take the best normalised edit-distance score.
 *    This lets short queries match inside long documents.
 * 2. **Bigram similarity** — structural character-level overlap.
 * 3. **Token overlap** — semantic word-level overlap.
 */
export function fuzzyScore(
	query: string,
	candidate: string,
	options?: FuzzyScoreOptions,
): number {
	const q = query.toLowerCase();
	const c = candidate.toLowerCase();

	if (q.length === 0 && c.length === 0) return 1;
	if (q.length === 0 || c.length === 0) return 0;

	const substringMinLength = options?.substringMinLength ?? 3;

	// --- Substring containment short-circuit (skip for very short queries) ---
	if (q.length >= substringMinLength && c.includes(q)) return 1;

	// --- Best-window Levenshtein ---
	let bestLev: number;
	if (q.length >= c.length) {
		bestLev = levenshteinSimilarity(q, c);
	} else {
		// Slide a window of length q.length (± small margin) over c.
		let best = 0;
		const windowSizes = [
			q.length,
			Math.min(q.length + 1, c.length),
			Math.max(q.length - 1, 1),
		];
		for (const ws of windowSizes) {
			for (let start = 0; start <= c.length - ws; start++) {
				const window = c.slice(start, start + ws);
				const sim = levenshteinSimilarity(q, window);
				if (sim > best) best = sim;
				if (best === 1) break;
			}
			if (best === 1) break;
		}
		bestLev = best;
	}

	// --- Bigram similarity ---
	const bigramSim = ngramSimilarity(q, c, 2);

	// --- Token overlap ---
	const tokenSim = tokenOverlapScore(query, candidate);

	const wLev = options?.weights?.levenshtein ?? 0.4;
	const wBigram = options?.weights?.bigram ?? 0.3;
	const wToken = options?.weights?.token ?? 0.3;

	return wLev * bestLev + wBigram * bigramSim + wToken * tokenSim;
}

// ---------------------------------------------------------------------------
// Metadata matching
// ---------------------------------------------------------------------------

// Small LRU-ish cache for compiled regex patterns used by metadata filters.
// Avoids recompiling the same pattern on every entry during a filter scan.
const regexCache = new Map<string, RegExp | null>();
const REGEX_CACHE_MAX = 64;

function getCachedRegex(pattern: string): RegExp | null {
	let cached = regexCache.get(pattern);
	if (cached !== undefined) return cached;

	try {
		cached = new RegExp(pattern);
	} catch {
		cached = null;
	}

	if (regexCache.size >= REGEX_CACHE_MAX) {
		// Evict oldest entry (first key)
		const first = regexCache.keys().next().value;
		if (first !== undefined) regexCache.delete(first);
	}
	regexCache.set(pattern, cached);
	return cached;
}

/**
 * Test whether a metadata record satisfies a single `MetadataFilter`.
 */
export function matchesMetadataFilter(
	metadata: Record<string, string>,
	filter: MetadataFilter,
): boolean {
	const mode = filter.mode ?? 'eq';
	const actual = metadata[filter.key];

	switch (mode) {
		case 'exists':
			return filter.key in metadata;
		case 'notExists':
			return !(filter.key in metadata);
		case 'eq':
			return actual === filter.value;
		case 'neq':
			return actual !== undefined && actual !== filter.value;
		case 'contains':
			return (
				actual !== undefined &&
				typeof filter.value === 'string' &&
				actual.toLowerCase().includes(filter.value.toLowerCase())
			);
		case 'startsWith':
			return (
				actual !== undefined &&
				typeof filter.value === 'string' &&
				actual.toLowerCase().startsWith(filter.value.toLowerCase())
			);
		case 'endsWith':
			return (
				actual !== undefined &&
				typeof filter.value === 'string' &&
				actual.toLowerCase().endsWith(filter.value.toLowerCase())
			);
		case 'regex': {
			if (actual === undefined || typeof filter.value !== 'string')
				return false;
			const re = getCachedRegex(filter.value);
			return re?.test(actual) ?? false;
		}
		case 'gt': {
			if (actual === undefined || typeof filter.value !== 'string')
				return false;
			const a = Number(actual);
			const b = Number(filter.value);
			if (Number.isNaN(a) || Number.isNaN(b)) return false;
			return a > b;
		}
		case 'gte': {
			if (actual === undefined || typeof filter.value !== 'string')
				return false;
			const a = Number(actual);
			const b = Number(filter.value);
			if (Number.isNaN(a) || Number.isNaN(b)) return false;
			return a >= b;
		}
		case 'lt': {
			if (actual === undefined || typeof filter.value !== 'string')
				return false;
			const a = Number(actual);
			const b = Number(filter.value);
			if (Number.isNaN(a) || Number.isNaN(b)) return false;
			return a < b;
		}
		case 'lte': {
			if (actual === undefined || typeof filter.value !== 'string')
				return false;
			const a = Number(actual);
			const b = Number(filter.value);
			if (Number.isNaN(a) || Number.isNaN(b)) return false;
			return a <= b;
		}
		case 'in': {
			if (actual === undefined || !Array.isArray(filter.value)) return false;
			return filter.value.includes(actual);
		}
		case 'notIn': {
			if (actual === undefined || !Array.isArray(filter.value)) return false;
			return !filter.value.includes(actual);
		}
		case 'between': {
			if (actual === undefined || !Array.isArray(filter.value)) return false;
			if (filter.value.length !== 2) return false;
			const val = Number(actual);
			const min = Number(filter.value[0]);
			const max = Number(filter.value[1]);
			if (Number.isNaN(val) || Number.isNaN(min) || Number.isNaN(max))
				return false;
			return val >= min && val <= max;
		}
		default:
			return false;
	}
}

/**
 * Test whether a metadata record satisfies **all** filters (logical AND).
 */
export function matchesAllMetadataFilters(
	metadata: Readonly<Record<string, string>>,
	filters: readonly MetadataFilter[],
): boolean {
	return filters.every((f) => matchesMetadataFilter(metadata, f));
}
