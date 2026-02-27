// ---------------------------------------------------------------------------
// Recommendation — pure recommendation computation extracted from VectorStore
// ---------------------------------------------------------------------------
//
// Computes weighted recommendation scores combining vector similarity,
// recency, and access frequency. Supports pre-filtering by topic, metadata,
// and date range, plus adaptive learning engine integration.
//
// This module is pure — no side effects, no access tracking, no dirty flags.
// ---------------------------------------------------------------------------

import type { MagnitudeCache, MetadataIndex, TopicIndex } from './cataloging.js';
import { computeMagnitude } from './cataloging.js';
import type { LearningEngine } from './patron-learning.js';
import {
	computeRecommendationScore,
	frequencyScore,
	normalizeWeights,
	type RecencyOptions,
	recencyScore,
} from './recommendation.js';
import { matchesAllMetadataFilters } from './text-search.js';
import type {
	MetadataFilter,
	RecommendationResult,
	RecommendOptions,
	VectorEntry,
} from './types.js';

// ---------------------------------------------------------------------------
// Fast cosine similarity using pre-computed magnitudes
// ---------------------------------------------------------------------------

/**
 * Compute cosine similarity using a pre-computed query magnitude and cached
 * entry magnitudes. Returns `undefined` when vectors are incompatible or
 * zero-magnitude.
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
	return Number.isFinite(raw) ? Math.min(1, Math.max(-1, raw)) : undefined;
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Compute recommendations from a set of vector entries.
 *
 * This is a pure function — it does NOT track access or mutate any state.
 * The caller is responsible for any side-effect bookkeeping.
 *
 * @param entries       - All vector entries to consider.
 * @param accessStats   - Per-entry access statistics for frequency scoring.
 * @param options       - Recommendation query options (filters, weights, limits).
 * @param magnitudeCache - Pre-computed magnitude cache for fast cosine.
 * @param topicIndex    - Topic index for topic-based pre-filtering.
 * @param metadataIndex - Metadata index (unused directly but reserved for future use).
 * @param learningEngine - Optional adaptive learning engine for weight adaptation and boosting.
 * @param recencyOptions - Optional recency decay configuration.
 * @returns Sorted recommendation results (highest score first).
 */
export function computeRecommendations(
	entries: readonly VectorEntry[],
	accessStats: ReadonlyMap<
		string,
		{ readonly accessCount: number; readonly lastAccessed: number }
	>,
	options: RecommendOptions,
	magnitudeCache: MagnitudeCache,
	topicIndex: TopicIndex,
	_metadataIndex: MetadataIndex,
	learningEngine?: LearningEngine,
	recencyOptions?: RecencyOptions,
): RecommendationResult[] {
	// Use adapted weights from learning engine if available, falling back to user-provided or defaults
	const baseWeights =
		learningEngine && !options.weights
			? learningEngine.getAdaptedWeights()
			: normalizeWeights(options.weights);
	const weights = baseWeights;
	const maxResults = options.maxResults ?? 10;
	const minScore = options.minScore ?? 0;

	// Pre-filter candidates
	let candidates: readonly VectorEntry[] = entries;

	// Topic filter
	if (options.topics && options.topics.length > 0) {
		const matchingIds = new Set<string>();
		for (const topic of options.topics) {
			for (const id of topicIndex.getEntries(topic)) {
				matchingIds.add(id);
			}
		}
		candidates = candidates.filter((e) => matchingIds.has(e.id));
	}

	// Metadata filter
	if (options.metadata && options.metadata.length > 0) {
		candidates = candidates.filter((e) =>
			matchesAllMetadataFilters(
				e.metadata,
				options.metadata as MetadataFilter[],
			),
		);
	}

	// Date range filter
	if (options.dateRange) {
		const { after, before } = options.dateRange;
		candidates = candidates.filter((e) => {
			if (after !== undefined && e.timestamp < after) return false;
			if (before !== undefined && e.timestamp > before) return false;
			return true;
		});
	}

	if (candidates.length === 0) return [];

	// Find max access count for frequency normalization
	let maxAccessCount = 0;
	for (const entry of candidates) {
		const stats = accessStats.get(entry.id);
		if (stats && stats.accessCount > maxAccessCount) {
			maxAccessCount = stats.accessCount;
		}
	}

	// Pre-compute query magnitude for vector scoring
	const queryEmbedding = options.queryEmbedding;
	const queryMag =
		queryEmbedding && queryEmbedding.length > 0
			? computeMagnitude(queryEmbedding)
			: 0;

	const results: RecommendationResult[] = [];

	for (const entry of candidates) {
		// Vector similarity score
		let vectorScoreVal: number | undefined;
		if (queryEmbedding && queryEmbedding.length > 0 && queryMag > 0) {
			vectorScoreVal = fastCosine(
				queryEmbedding,
				queryMag,
				entry,
				magnitudeCache,
			);
		}

		// Recency score
		const recencyVal = recencyScore(entry.timestamp, recencyOptions);

		// Frequency score
		const stats = accessStats.get(entry.id);
		const freqVal = frequencyScore(stats?.accessCount ?? 0, maxAccessCount);

		const recommendation = computeRecommendationScore(
			{
				vectorScore: vectorScoreVal,
				recencyScore: recencyVal,
				frequencyScore: freqVal,
			},
			weights,
		);

		// Apply learning boost if available
		const boost = learningEngine
			? learningEngine.computeBoost(entry.id, entry.embedding)
			: 1.0;
		const boostedScore = recommendation.score * boost;

		if (boostedScore >= minScore) {
			results.push({
				entry,
				score: boostedScore,
				scores: recommendation.scores,
			});
		}
	}

	results.sort((a, b) => b.score - a.score);

	return results.slice(0, maxResults);
}
