// ---------------------------------------------------------------------------
// Vector Search — pure search functions extracted from vector-store.ts
// ---------------------------------------------------------------------------
//
// Standalone, side-effect-free search functions that operate on readonly
// collections of VectorEntry data.  The vector store delegates to these
// functions and handles access tracking / learning engine recording itself.
// ---------------------------------------------------------------------------

import type { MagnitudeCache, MetadataIndex } from './cataloging.js';
import { computeMagnitude } from './cataloging.js';
import type { InvertedIndex } from './inverted-index.js';
import { recencyScore } from './recommendation.js';
import {
	fuzzyScore,
	matchesAllMetadataFilters,
	tokenOverlapScore,
} from './text-search.js';
import type {
	AdvancedSearchResult,
	DateRange,
	MetadataFilter,
	SearchOptions,
	SearchResult,
	TextSearchOptions,
	TextSearchResult,
	VectorEntry,
} from './types.js';

// ---------------------------------------------------------------------------
// Configuration passed from the store
// ---------------------------------------------------------------------------

export interface VectorSearchConfig {
	/** Maximum regex pattern length before rejection. */
	readonly maxRegexPatternLength: number;
	/** Logger warn function for diagnostics. */
	readonly warn: (msg: string) => void;
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/**
 * Fast cosine similarity using pre-computed magnitudes.
 * Returns undefined if vectors are incompatible or zero-magnitude.
 */
function fastCosine(
	queryEmbedding: readonly number[],
	queryMag: number,
	entry: VectorEntry,
	magnitudeCache: MagnitudeCache,
): number | undefined {
	if (entry.embedding.length !== queryEmbedding.length) return undefined;
	const entryMag =
		magnitudeCache.get(entry.id) ?? computeMagnitude(entry.embedding);
	if (entryMag === 0) return undefined;
	let dot = 0;
	for (let i = 0; i < queryEmbedding.length; i++) {
		dot += queryEmbedding[i] * entry.embedding[i];
	}
	const raw = dot / (queryMag * entryMag);
	// Clamp to [-1, 1] to guard against floating-point rounding
	return Number.isFinite(raw) ? Math.min(1, Math.max(-1, raw)) : undefined;
}

function scoreText(
	candidate: string,
	query: string,
	mode: string,
	compiledRegex: RegExp | undefined,
	config: VectorSearchConfig,
): number {
	switch (mode) {
		case 'fuzzy':
			return fuzzyScore(query, candidate);

		case 'substring':
			return candidate.toLowerCase().includes(query.toLowerCase()) ? 1 : 0;

		case 'exact':
			return candidate === query ? 1 : 0;

		case 'regex': {
			if (compiledRegex) {
				return compiledRegex.test(candidate) ? 1 : 0;
			}
			// Fallback: compile once (should not normally reach here)
			if (query.length > config.maxRegexPatternLength) {
				config.warn(
					`Regex pattern exceeds ${config.maxRegexPatternLength} chars, skipping`,
				);
				return 0;
			}
			try {
				return new RegExp(query).test(candidate) ? 1 : 0;
			} catch {
				config.warn(`Invalid regex pattern: ${query}`);
				return 0;
			}
		}

		case 'token':
			return tokenOverlapScore(query, candidate);

		default:
			return 0;
	}
}

function combineScores(
	vectorScore: number | undefined,
	textScore: number | undefined,
	rankBy: string,
): number {
	const v = vectorScore ?? 0;
	const t = textScore ?? 0;
	const hasVector = vectorScore !== undefined;
	const hasText = textScore !== undefined;

	if (!hasVector && !hasText) return 0;
	if (!hasVector) return t;
	if (!hasText) return v;

	switch (rankBy) {
		case 'vector':
			return v;
		case 'text':
			return t;
		case 'multiply':
			return v * t;
		default:
			return (v + t) / 2;
	}
}

// ---------------------------------------------------------------------------
// Public search functions
// ---------------------------------------------------------------------------

/**
 * Pure vector similarity search using cosine distance with magnitude cache.
 *
 * Returns results sorted by descending score, limited to `maxResults`.
 * Does NOT track access or record queries — the caller is responsible
 * for those side effects.
 */
export function vectorSearch(
	entries: readonly VectorEntry[],
	queryEmbedding: readonly number[],
	maxResults: number,
	threshold: number,
	magnitudeCache: MagnitudeCache,
): SearchResult[] {
	if (queryEmbedding.length === 0) return [];

	// Pre-compute query magnitude once
	const queryMag = computeMagnitude(queryEmbedding);
	if (queryMag === 0) return [];

	const scored: SearchResult[] = [];

	for (const entry of entries) {
		const score = fastCosine(queryEmbedding, queryMag, entry, magnitudeCache);
		if (score === undefined) continue;
		if (score >= threshold) {
			scored.push({ entry, score });
		}
	}

	scored.sort((a, b) => b.score - a.score);
	return scored.slice(0, maxResults);
}

/**
 * Text-based search supporting fuzzy, substring, exact, regex, token, and
 * BM25 modes.
 *
 * When `mode === 'bm25'` and an `invertedIndex` is provided, the function
 * uses BM25 term-frequency scoring via the inverted index for O(terms)
 * lookup instead of a linear scan.  For all other modes the existing
 * linear-scan behaviour is preserved.
 */
export function textSearchEntries(
	entries: readonly VectorEntry[],
	options: TextSearchOptions,
	config: VectorSearchConfig,
	invertedIndex?: InvertedIndex,
): TextSearchResult[] {
	const { query, mode = 'fuzzy', threshold = 0.3 } = options;

	if (query.length === 0) return [];

	// ---- BM25 fast-path via inverted index ----
	if (mode === 'bm25' && invertedIndex) {
		const bm25Results = invertedIndex.bm25Search(query);
		if (bm25Results.length === 0) return [];

		// Build a lookup map from entries for O(1) access
		const entryMap = new Map<string, VectorEntry>();
		for (const entry of entries) {
			entryMap.set(entry.id, entry);
		}

		// Normalize BM25 scores to [0, 1] range using the max score
		const maxScore = bm25Results[0].score; // already sorted desc
		const results: TextSearchResult[] = [];

		for (const bm25 of bm25Results) {
			const entry = entryMap.get(bm25.id);
			if (!entry) continue;
			const normalizedScore = maxScore > 0 ? bm25.score / maxScore : 0;
			if (normalizedScore >= threshold) {
				results.push({ entry, score: normalizedScore });
			}
		}

		return results;
	}

	// ---- Standard linear-scan modes ----

	// Compile regex once before the loop
	let compiledRegex: RegExp | undefined;
	if (mode === 'regex') {
		if (query.length > config.maxRegexPatternLength) {
			config.warn(
				`Regex pattern exceeds ${config.maxRegexPatternLength} chars, skipping`,
			);
			return [];
		}
		try {
			compiledRegex = new RegExp(query);
		} catch {
			config.warn(`Invalid regex pattern: ${query}`);
			return [];
		}
	}

	const results: TextSearchResult[] = [];

	for (const entry of entries) {
		const score = scoreText(entry.text, query, mode, compiledRegex, config);
		if (score >= threshold) {
			results.push({ entry, score });
		}
	}

	results.sort((a, b) => b.score - a.score);
	return results;
}

/**
 * Filter entries by metadata using indexed lookups (for simple `eq` filters)
 * or a linear scan (for complex filter modes).
 */
export function filterEntriesByMetadata(
	entries: readonly VectorEntry[],
	filters: readonly MetadataFilter[],
	metadataIndex: MetadataIndex,
): VectorEntry[] {
	if (filters.length === 0) return [...entries];

	// Optimization: if all filters are simple "eq" mode, use the metadata index
	const allEq = filters.every(
		(f) => (f.mode ?? 'eq') === 'eq' && f.value !== undefined,
	);
	if (allEq) {
		// Intersect sets from the metadata index
		let candidateIds: Set<string> | undefined;
		for (const f of filters) {
			const ids = metadataIndex.getEntries(f.key, f.value as string);
			if (candidateIds === undefined) {
				candidateIds = new Set(ids);
			} else {
				for (const id of candidateIds) {
					if (!ids.has(id)) candidateIds.delete(id);
				}
			}
			if (candidateIds.size === 0) return [];
		}
		if (!candidateIds) return [];
		return entries.filter((e) => candidateIds.has(e.id));
	}

	// Fallback: linear scan for complex filter modes
	return entries.filter((e) => matchesAllMetadataFilters(e.metadata, filters));
}

/**
 * Filter entries whose timestamp falls within the given date range.
 */
export function filterEntriesByDateRange(
	entries: readonly VectorEntry[],
	range: DateRange,
): VectorEntry[] {
	return entries.filter((e) => {
		if (range.after !== undefined && e.timestamp < range.after) return false;
		if (range.before !== undefined && e.timestamp > range.before) return false;
		return true;
	});
}

/**
 * Combined search across vector similarity, text matching, metadata filters,
 * and date ranges.
 *
 * When `options.text.mode === 'bm25'` and an `invertedIndex` is provided,
 * BM25 scoring is used for the text component instead of a linear scan.
 *
 * Returns results sorted by descending combined score, limited to
 * `maxResults`.  Does NOT track access or record queries.
 */
export function advancedVectorSearch(
	entries: readonly VectorEntry[],
	options: SearchOptions,
	config: VectorSearchConfig,
	magnitudeCache: MagnitudeCache,
	_metadataIndex: MetadataIndex,
	invertedIndex?: InvertedIndex,
): AdvancedSearchResult[] {
	const {
		queryEmbedding,
		similarityThreshold = 0,
		text,
		metadata,
		dateRange,
		maxResults = 10,
		rankBy = 'average',
		fieldBoosts,
		rankWeights,
		topicFilter,
	} = options;

	// ---- BM25 fast-path for text component ----
	// When using BM25 mode we pre-compute text scores via the inverted index
	// so we can look them up per entry in O(1) instead of re-scanning.
	let bm25ScoreMap: Map<string, number> | undefined;
	if (text && (text.mode ?? 'fuzzy') === 'bm25' && invertedIndex) {
		const bm25Results = invertedIndex.bm25Search(text.query);
		if (bm25Results.length > 0) {
			const maxBm25 = bm25Results[0].score;
			bm25ScoreMap = new Map<string, number>();
			for (const r of bm25Results) {
				const normalized = maxBm25 > 0 ? r.score / maxBm25 : 0;
				bm25ScoreMap.set(r.id, normalized);
			}
		}
	}

	// ---- Pre-compute topic filter set for O(1) lookup ----
	const topicSet =
		topicFilter && topicFilter.length > 0
			? new Set<string>(topicFilter)
			: undefined;

	const results: AdvancedSearchResult[] = [];

	// Pre-compute query magnitude for fast cosine
	const queryMag =
		queryEmbedding && queryEmbedding.length > 0
			? computeMagnitude(queryEmbedding)
			: 0;

	// Pre-compile regex for text search if needed
	let compiledRegex: RegExp | undefined;
	if (text && (text.mode ?? 'fuzzy') === 'regex') {
		if (text.query.length > config.maxRegexPatternLength) {
			config.warn(
				`Regex pattern exceeds ${config.maxRegexPatternLength} chars, skipping text filter`,
			);
		} else {
			try {
				compiledRegex = new RegExp(text.query);
			} catch {
				config.warn(`Invalid regex pattern: ${text.query}`);
			}
		}
	}

	for (const entry of entries) {
		if (dateRange) {
			if (dateRange.after !== undefined && entry.timestamp < dateRange.after)
				continue;
			if (dateRange.before !== undefined && entry.timestamp > dateRange.before)
				continue;
		}

		// Metadata filtering — entries that don't match are excluded
		const passedMetadata =
			!metadata ||
			metadata.length === 0 ||
			matchesAllMetadataFilters(entry.metadata, metadata as MetadataFilter[]);
		if (!passedMetadata) continue;

		let vectorScore: number | undefined;
		if (queryEmbedding && queryEmbedding.length > 0 && queryMag > 0) {
			vectorScore = fastCosine(queryEmbedding, queryMag, entry, magnitudeCache);
			if (vectorScore === undefined) continue;
			if (vectorScore < similarityThreshold) continue;
		}

		let textScoreVal: number | undefined;
		if (text) {
			const mode = text.mode ?? 'fuzzy';
			const textThreshold = text.threshold ?? 0.3;

			if (mode === 'bm25' && bm25ScoreMap) {
				// Use pre-computed BM25 scores
				textScoreVal = bm25ScoreMap.get(entry.id);
				if (textScoreVal === undefined) textScoreVal = 0;
				if (textScoreVal < textThreshold) continue;
			} else if (mode === 'bm25') {
				// BM25 requested but no inverted index — skip text scoring
				textScoreVal = undefined;
			} else {
				textScoreVal = scoreText(
					entry.text,
					text.query,
					mode,
					compiledRegex,
					config,
				);
				if (textScoreVal < textThreshold) continue;
			}
		}

		// ---- Field boosting ----
		// Apply text boost multiplier
		let boostedTextScore = textScoreVal;
		if (boostedTextScore !== undefined && fieldBoosts?.text !== undefined) {
			boostedTextScore *= fieldBoosts.text;
		}

		// Metadata boost: bonus for entries that passed metadata filters
		let metadataBoost = 0;
		if (
			fieldBoosts?.metadata !== undefined &&
			metadata &&
			metadata.length > 0
		) {
			// Entry passed metadata filters — apply the metadata boost
			metadataBoost = fieldBoosts.metadata;
		}

		// Topic boost: bonus for entries whose topic matches the topic filter
		let topicBoost = 0;
		if (fieldBoosts?.topic !== undefined && topicSet) {
			const entryTopic = entry.metadata.topic;
			if (entryTopic && topicSet.has(entryTopic)) {
				topicBoost = fieldBoosts.topic;
			}
		}

		// ---- Score combination ----
		let finalScore: number;
		if (rankBy === 'weighted') {
			// Weighted ranking mode — combine all components with user-specified weights
			const wVector = rankWeights?.vector ?? 0.5;
			const wText = rankWeights?.text ?? 0.3;
			const wMetadata = rankWeights?.metadata ?? 0.1;
			const wRecency = rankWeights?.recency ?? 0.1;

			const recencyVal = recencyScore(entry.timestamp);

			finalScore =
				(vectorScore ?? 0) * wVector +
				(boostedTextScore ?? 0) * wText +
				metadataBoost * wMetadata +
				recencyVal * wRecency;
		} else {
			// Standard ranking modes — apply boosts as additive bonuses
			const baseScore = combineScores(vectorScore, boostedTextScore, rankBy);
			finalScore = baseScore + metadataBoost + topicBoost;
		}

		results.push({
			entry,
			score: finalScore,
			scores: {
				vector: vectorScore,
				text: textScoreVal,
			},
		});
	}

	results.sort((a, b) => b.score - a.score);
	return results.slice(0, maxResults);
}
