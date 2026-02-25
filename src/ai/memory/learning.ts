// ---------------------------------------------------------------------------
// Adaptive Learning Engine
// ---------------------------------------------------------------------------
//
// Observes search patterns and adapts the memory system in real time:
//
// 1. Relevance feedback — tracks which entries are retrieved by diverse
//    queries, boosting consistently-relevant entries.
// 2. Adaptive weight profiles — shifts vector/recency/frequency weights
//    based on which signals best predict useful results.
// 3. Interest embedding — maintains a decayed average of recent query
//    embeddings representing the user's evolving interests.
//
// All state is serializable for persistence via the vector store's
// compressed JSON format.
// ---------------------------------------------------------------------------

import { decodeEmbedding, encodeEmbedding } from './compression.js';
import { cosineSimilarity } from './cosine.js';
import type {
	LearningOptions,
	LearningProfile,
	QueryRecord,
	RelevanceFeedback,
	WeightProfile,
} from './types.js';
import type {
	ExplicitFeedbackEntry,
	FeedbackEntry,
	LearningState,
	SerializedQueryRecord,
} from './vector-persistence.js';

// ---------------------------------------------------------------------------
// LearningEngine interface
// ---------------------------------------------------------------------------

export interface LearningEngine {
	/** Record a completed query and its result set for learning. */
	readonly recordQuery: (
		queryEmbedding: readonly number[],
		resultIds: readonly string[],
	) => void;
	/** Record explicit user feedback on whether an entry was relevant. */
	readonly recordFeedback: (entryId: string, relevant: boolean) => void;
	/** Get accumulated relevance feedback for an entry. */
	readonly getRelevanceFeedback: (id: string) => RelevanceFeedback | undefined;
	/** Get the current adapted weight profile. */
	readonly getAdaptedWeights: () => Readonly<Required<WeightProfile>>;
	/** Get the current interest embedding (decayed average of queries). */
	readonly getInterestEmbedding: () => readonly number[] | undefined;
	/** Compute a boost multiplier for an entry based on learning state. */
	readonly computeBoost: (
		entryId: string,
		entryEmbedding: readonly number[],
	) => number;
	/** Serialize all learning state for persistence. */
	readonly serialize: () => LearningState;
	/** Restore learning state from a previously serialized snapshot. */
	readonly restore: (state: LearningState) => void;
	/** Clear all learning state. */
	readonly clear: () => void;
	/** Remove feedback for entries that no longer exist. */
	readonly pruneEntries: (validIds: ReadonlySet<string>) => void;
	/** Get the full learning profile snapshot. */
	readonly getProfile: () => LearningProfile;
	/** Total number of queries recorded. */
	readonly totalQueries: number;
	/** Whether any learning state exists. */
	readonly hasData: boolean;
}

// ---------------------------------------------------------------------------
// Defaults
// ---------------------------------------------------------------------------

const SEVEN_DAYS_MS = 7 * 24 * 60 * 60 * 1000;
const MIN_WEIGHT = 0.05;
const MAX_WEIGHT = 0.9;
const BOOST_MIN = 0.8;
const BOOST_MAX = 1.2;

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createLearningEngine(
	options?: LearningOptions,
): LearningEngine {
	const enabled = options?.enabled ?? true;
	const maxQueryHistory = options?.maxQueryHistory ?? 50;
	const queryDecayMs = options?.queryDecayMs ?? SEVEN_DAYS_MS;
	const weightAdaptationRate = options?.weightAdaptationRate ?? 0.05;
	const interestBoostWeight = options?.interestBoostWeight ?? 0.15;

	// -- Mutable state -------------------------------------------------------

	/** Per-entry feedback: id -> mutable feedback tracking. */
	const feedback = new Map<
		string,
		{
			queryCount: number;
			totalRetrievals: number;
			lastQueryTimestamp: number;
			queryEmbeddings: number[][]; // sample of distinct query embeddings that found this entry
		}
	>();

	/** Per-entry explicit relevance feedback: id -> positive/negative counts. */
	const explicitFeedback = new Map<
		string,
		{ positive: number; negative: number }
	>();

	/** Recent query history (capped at maxQueryHistory). */
	let queryHistory: QueryRecord[] = [];

	/** Current adapted weights. */
	let adaptedWeights: Required<WeightProfile> = {
		vector: 0.6,
		recency: 0.2,
		frequency: 0.2,
	};

	/** Computed interest embedding (unit vector). */
	let interestEmbedding: number[] | undefined;

	/** Total queries recorded ever. */
	let totalQueriesCount = 0;

	/** Last time learning state changed. */
	let lastUpdated = 0;

	// -- Helpers --------------------------------------------------------------

	const clampWeight = (w: number): number =>
		Math.min(MAX_WEIGHT, Math.max(MIN_WEIGHT, w));

	const normalizeWeightsInPlace = (): void => {
		const v = clampWeight(adaptedWeights.vector);
		const r = clampWeight(adaptedWeights.recency);
		const f = clampWeight(adaptedWeights.frequency);
		const total = v + r + f;
		adaptedWeights = {
			vector: v / total,
			recency: r / total,
			frequency: f / total,
		};
	};

	/**
	 * Compute the relevance score for an entry based on implicit retrieval
	 * counts and explicit user feedback.
	 *
	 * Formula: clamp((queryCount + positive * 5 - negative * 3) / maxScale, 0, 1)
	 * Explicit feedback is weighted much stronger than implicit retrievals.
	 */
	const computeRelevanceScore = (
		entryId: string,
		entry: typeof feedback extends Map<string, infer V> ? V : never,
	): number => {
		const explicit = explicitFeedback.get(entryId);
		const positiveFeedback = explicit?.positive ?? 0;
		const negativeFeedback = explicit?.negative ?? 0;

		const rawScore =
			entry.queryCount + positiveFeedback * 5 - negativeFeedback * 3;

		const maxScale = maxQueryHistory;
		return Math.min(1, Math.max(0, rawScore / maxScale));
	};

	/**
	 * Recompute the interest embedding from query history using
	 * exponential decay weighting.
	 */
	const recomputeInterestEmbedding = (): void => {
		if (queryHistory.length === 0) {
			interestEmbedding = undefined;
			return;
		}

		const now = Date.now();
		const lambda = Math.LN2 / queryDecayMs;
		const dim = queryHistory[0].embedding.length;
		if (dim === 0) {
			interestEmbedding = undefined;
			return;
		}

		const weighted = new Float64Array(dim);
		let totalWeight = 0;

		for (const record of queryHistory) {
			if (record.embedding.length !== dim) continue;
			const age = Math.max(0, now - record.timestamp);
			const w = Math.exp(-lambda * age);
			totalWeight += w;
			for (let i = 0; i < dim; i++) {
				weighted[i] += record.embedding[i] * w;
			}
		}

		if (totalWeight === 0) {
			interestEmbedding = undefined;
			return;
		}

		// Normalize to unit vector
		let mag = 0;
		for (let i = 0; i < dim; i++) {
			weighted[i] /= totalWeight;
			mag += weighted[i] * weighted[i];
		}
		mag = Math.sqrt(mag);

		if (mag === 0) {
			interestEmbedding = undefined;
			return;
		}

		const result = new Array<number>(dim);
		for (let i = 0; i < dim; i++) {
			result[i] = weighted[i] / mag;
		}
		interestEmbedding = result;
	};

	// -- Public API -----------------------------------------------------------

	const recordQuery = (
		queryEmbedding: readonly number[],
		resultIds: readonly string[],
	): void => {
		if (!enabled) return;
		if (queryEmbedding.length === 0 || resultIds.length === 0) return;

		const now = Date.now();
		totalQueriesCount++;
		lastUpdated = now;

		// Add to query history (FIFO capped)
		const record: QueryRecord = {
			embedding: [...queryEmbedding],
			timestamp: now,
			resultCount: resultIds.length,
		};
		queryHistory.push(record);
		if (queryHistory.length > maxQueryHistory) {
			queryHistory = queryHistory.slice(-maxQueryHistory);
		}

		// Update per-entry feedback
		for (const id of resultIds) {
			const existing = feedback.get(id);
			if (existing) {
				existing.totalRetrievals++;
				existing.lastQueryTimestamp = now;

				// Track diverse queries: only count if this query embedding
				// is sufficiently different from previously recorded ones.
				const isDiverse =
					existing.queryEmbeddings.length === 0 ||
					existing.queryEmbeddings.every(
						(prev) => cosineSimilarity(prev, [...queryEmbedding]) < 0.9,
					);
				if (isDiverse) {
					existing.queryCount++;
					// Keep a bounded sample of query embeddings for diversity tracking
					if (existing.queryEmbeddings.length < 20) {
						existing.queryEmbeddings.push([...queryEmbedding]);
					}
				}
			} else {
				feedback.set(id, {
					queryCount: 1,
					totalRetrievals: 1,
					lastQueryTimestamp: now,
					queryEmbeddings: [[...queryEmbedding]],
				});
			}
		}

		// Adapt weights based on whether recent results tended to be
		// recently-created or frequently-accessed entries.
		// We use a simple heuristic: if results were found, slightly
		// shift toward the weights that the system is using.
		// This creates a gentle positive feedback loop that stabilizes.
		if (resultIds.length > 0) {
			// Count how many results have high access (feedback) counts
			let highFeedbackCount = 0;
			for (const id of resultIds) {
				const fb = feedback.get(id);
				if (fb && fb.totalRetrievals > 3) {
					highFeedbackCount++;
				}
			}

			const ratio = highFeedbackCount / resultIds.length;
			if (ratio > 0.5) {
				// Results skew toward frequently-accessed entries → boost frequency weight
				adaptedWeights = {
					vector: adaptedWeights.vector,
					recency: adaptedWeights.recency,
					frequency: adaptedWeights.frequency + weightAdaptationRate * 0.5,
				};
			} else {
				// Results are fresh/diverse → boost vector weight (semantic relevance)
				adaptedWeights = {
					vector: adaptedWeights.vector + weightAdaptationRate * 0.5,
					recency: adaptedWeights.recency,
					frequency: adaptedWeights.frequency,
				};
			}
			normalizeWeightsInPlace();
		}

		// Recompute interest embedding
		recomputeInterestEmbedding();
	};

	const recordFeedback = (entryId: string, relevant: boolean): void => {
		if (!enabled) return;

		const existing = explicitFeedback.get(entryId);
		if (existing) {
			if (relevant) {
				existing.positive++;
			} else {
				existing.negative++;
			}
		} else {
			explicitFeedback.set(entryId, {
				positive: relevant ? 1 : 0,
				negative: relevant ? 0 : 1,
			});
		}

		lastUpdated = Date.now();
	};

	const getRelevanceFeedback = (id: string): RelevanceFeedback | undefined => {
		const entry = feedback.get(id);
		if (!entry) {
			// Check if there's explicit feedback without implicit tracking
			const explicit = explicitFeedback.get(id);
			if (!explicit) return undefined;

			// Return feedback based solely on explicit signals
			const rawScore = explicit.positive * 5 - explicit.negative * 3;
			const maxScale = maxQueryHistory;
			return {
				queryCount: 0,
				totalRetrievals: 0,
				lastQueryTimestamp: 0,
				relevanceScore: Math.min(1, Math.max(0, rawScore / maxScale)),
			};
		}

		return {
			queryCount: entry.queryCount,
			totalRetrievals: entry.totalRetrievals,
			lastQueryTimestamp: entry.lastQueryTimestamp,
			relevanceScore: computeRelevanceScore(id, entry),
		};
	};

	const getAdaptedWeights = (): Readonly<Required<WeightProfile>> => ({
		...adaptedWeights,
	});

	const getInterestEmbedding = (): readonly number[] | undefined =>
		interestEmbedding ? [...interestEmbedding] : undefined;

	const computeBoost = (
		entryId: string,
		entryEmbedding: readonly number[],
	): number => {
		if (!enabled) return 1.0;

		let boost = 1.0;

		// Relevance feedback component: entries retrieved by diverse queries get a boost
		const fb = feedback.get(entryId);
		if (fb) {
			const relevance = computeRelevanceScore(entryId, fb);
			// Scale from 0→0 boost to 1→+0.1 boost
			boost += relevance * 0.1;
		}

		// Interest alignment component: entries closer to interest profile get a small boost
		if (
			interestEmbedding &&
			entryEmbedding.length === interestEmbedding.length
		) {
			const similarity = cosineSimilarity(
				[...entryEmbedding],
				interestEmbedding,
			);
			// Scale: similarity of 1.0 → +interestBoostWeight, 0.0 → +0
			boost += Math.max(0, similarity) * interestBoostWeight;
		}

		return Math.min(BOOST_MAX, Math.max(BOOST_MIN, boost));
	};

	const serialize = (): LearningState => {
		const serializedFeedback: FeedbackEntry[] = [];
		for (const [id, entry] of feedback) {
			serializedFeedback.push({
				id,
				queryCount: entry.queryCount,
				totalRetrievals: entry.totalRetrievals,
				lastQueryTimestamp: entry.lastQueryTimestamp,
			});
		}

		const serializedHistory: SerializedQueryRecord[] = queryHistory.map(
			(r) => ({
				embedding: encodeEmbedding([...r.embedding]),
				timestamp: r.timestamp,
				resultCount: r.resultCount,
			}),
		);

		const serializedExplicitFeedback: ExplicitFeedbackEntry[] = [];
		for (const [entryId, counts] of explicitFeedback) {
			serializedExplicitFeedback.push({
				entryId,
				positiveCount: counts.positive,
				negativeCount: counts.negative,
			});
		}

		return {
			version: 1,
			feedback: serializedFeedback,
			queryHistory: serializedHistory,
			adaptedWeights: { ...adaptedWeights },
			interestEmbedding: interestEmbedding
				? encodeEmbedding(interestEmbedding)
				: undefined,
			totalQueries: totalQueriesCount,
			lastUpdated,
			explicitFeedback:
				serializedExplicitFeedback.length > 0
					? serializedExplicitFeedback
					: undefined,
		};
	};

	const restore = (state: LearningState): void => {
		// Restore feedback
		feedback.clear();
		for (const entry of state.feedback) {
			feedback.set(entry.id, {
				queryCount: entry.queryCount,
				totalRetrievals: entry.totalRetrievals,
				lastQueryTimestamp: entry.lastQueryTimestamp,
				queryEmbeddings: [], // not persisted — rebuilt from future queries
			});
		}

		// Restore query history
		queryHistory = [];
		for (const record of state.queryHistory) {
			try {
				const embedding = decodeEmbedding(record.embedding);
				queryHistory.push({
					embedding,
					timestamp: record.timestamp,
					resultCount: record.resultCount,
				});
			} catch {
				// Skip corrupt records
			}
		}

		// Restore weights
		adaptedWeights = {
			vector: state.adaptedWeights.vector,
			recency: state.adaptedWeights.recency,
			frequency: state.adaptedWeights.frequency,
		};
		normalizeWeightsInPlace();

		// Restore interest embedding
		if (state.interestEmbedding) {
			try {
				interestEmbedding = decodeEmbedding(state.interestEmbedding);
			} catch {
				interestEmbedding = undefined;
			}
		} else {
			interestEmbedding = undefined;
		}

		// Restore explicit feedback
		explicitFeedback.clear();
		if (state.explicitFeedback) {
			for (const entry of state.explicitFeedback) {
				explicitFeedback.set(entry.entryId, {
					positive: entry.positiveCount,
					negative: entry.negativeCount,
				});
			}
		}

		totalQueriesCount = state.totalQueries;
		lastUpdated = state.lastUpdated;
	};

	const clear = (): void => {
		feedback.clear();
		explicitFeedback.clear();
		queryHistory = [];
		adaptedWeights = { vector: 0.6, recency: 0.2, frequency: 0.2 };
		interestEmbedding = undefined;
		totalQueriesCount = 0;
		lastUpdated = 0;
	};

	const pruneEntries = (validIds: ReadonlySet<string>): void => {
		for (const id of feedback.keys()) {
			if (!validIds.has(id)) {
				feedback.delete(id);
			}
		}
		for (const id of explicitFeedback.keys()) {
			if (!validIds.has(id)) {
				explicitFeedback.delete(id);
			}
		}
	};

	const getProfile = (): LearningProfile => ({
		queryHistory: queryHistory.map((r) => ({
			embedding: [...r.embedding],
			timestamp: r.timestamp,
			resultCount: r.resultCount,
		})),
		adaptedWeights: { ...adaptedWeights },
		interestEmbedding: interestEmbedding ? [...interestEmbedding] : undefined,
		totalQueries: totalQueriesCount,
		lastUpdated,
	});

	return Object.freeze({
		recordQuery,
		recordFeedback,
		getRelevanceFeedback,
		getAdaptedWeights,
		getInterestEmbedding,
		computeBoost,
		serialize,
		restore,
		clear,
		pruneEntries,
		getProfile,
		get totalQueries() {
			return totalQueriesCount;
		},
		get hasData() {
			return totalQueriesCount > 0;
		},
	});
}
