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
// 4. Per-topic profiles — tracks weights, interest embeddings, and query
//    counts independently per topic, falling back to global state when
//    a topic has insufficient data (< 10 queries).
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
	TopicProfileEntry,
} from './vector-persistence.js';

// ---------------------------------------------------------------------------
// LearningEngine interface
// ---------------------------------------------------------------------------

export interface LearningEngine {
	/** Record a completed query and its result set for learning. */
	readonly recordQuery: (
		queryEmbedding: readonly number[],
		resultIds: readonly string[],
		options?: { readonly topic?: string },
	) => void;
	/** Record explicit user feedback on whether an entry was relevant. */
	readonly recordFeedback: (entryId: string, relevant: boolean) => void;
	/** Get accumulated relevance feedback for an entry. */
	readonly getRelevanceFeedback: (id: string) => RelevanceFeedback | undefined;
	/** Get the current adapted weight profile, optionally per-topic. */
	readonly getAdaptedWeights: (
		topic?: string,
	) => Readonly<Required<WeightProfile>>;
	/** Get the current interest embedding, optionally per-topic. */
	readonly getInterestEmbedding: (
		topic?: string,
	) => readonly number[] | undefined;
	/** Compute a boost multiplier for an entry based on learning state. */
	readonly computeBoost: (
		entryId: string,
		entryEmbedding: readonly number[],
		topic?: string,
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
const TOPIC_QUERY_THRESHOLD = 10;

// ---------------------------------------------------------------------------
// Per-topic mutable state
// ---------------------------------------------------------------------------

interface TopicState {
	weights: Required<WeightProfile>;
	interestEmbedding: number[] | undefined;
	queryCount: number;
	queryHistory: QueryRecord[];
}

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

	/** Per-topic learning state. */
	const topicStates = new Map<string, TopicState>();

	// -- Helpers --------------------------------------------------------------

	const clampWeight = (w: number): number =>
		Math.min(MAX_WEIGHT, Math.max(MIN_WEIGHT, w));

	const normalizeWeights = (
		weights: Required<WeightProfile>,
	): Required<WeightProfile> => {
		const v = clampWeight(weights.vector);
		const r = clampWeight(weights.recency);
		const f = clampWeight(weights.frequency);
		const total = v + r + f;
		return {
			vector: v / total,
			recency: r / total,
			frequency: f / total,
		};
	};

	const normalizeWeightsInPlace = (): void => {
		adaptedWeights = normalizeWeights(adaptedWeights);
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
	 * Recompute an interest embedding from a set of query records using
	 * exponential decay weighting.
	 */
	const computeInterestEmbeddingFromHistory = (
		history: readonly QueryRecord[],
	): number[] | undefined => {
		if (history.length === 0) return undefined;

		const now = Date.now();
		const lambda = Math.LN2 / queryDecayMs;
		const dim = history[0].embedding.length;
		if (dim === 0) return undefined;

		const weighted = new Float64Array(dim);
		let totalWeight = 0;

		for (const record of history) {
			if (record.embedding.length !== dim) continue;
			const age = Math.max(0, now - record.timestamp);
			const w = Math.exp(-lambda * age);
			totalWeight += w;
			for (let i = 0; i < dim; i++) {
				weighted[i] += record.embedding[i] * w;
			}
		}

		if (totalWeight === 0) return undefined;

		// Normalize to unit vector
		let mag = 0;
		for (let i = 0; i < dim; i++) {
			weighted[i] /= totalWeight;
			mag += weighted[i] * weighted[i];
		}
		mag = Math.sqrt(mag);

		if (mag === 0) return undefined;

		const result = new Array<number>(dim);
		for (let i = 0; i < dim; i++) {
			result[i] = weighted[i] / mag;
		}
		return result;
	};

	/**
	 * Recompute the global interest embedding from query history.
	 */
	const recomputeInterestEmbedding = (): void => {
		interestEmbedding = computeInterestEmbeddingFromHistory(queryHistory);
	};

	/**
	 * Get or create the mutable topic state for a given topic.
	 */
	const getOrCreateTopicState = (topic: string): TopicState => {
		let state = topicStates.get(topic);
		if (!state) {
			state = {
				weights: { vector: 0.6, recency: 0.2, frequency: 0.2 },
				interestEmbedding: undefined,
				queryCount: 0,
				queryHistory: [],
			};
			topicStates.set(topic, state);
		}
		return state;
	};

	/**
	 * Adapt weights based on whether recent results tended to be
	 * frequently-accessed entries. Returns the new weights.
	 */
	const adaptWeightsForResults = (
		currentWeights: Required<WeightProfile>,
		resultIds: readonly string[],
	): Required<WeightProfile> => {
		if (resultIds.length === 0) return currentWeights;

		let highFeedbackCount = 0;
		for (const id of resultIds) {
			const fb = feedback.get(id);
			if (fb && fb.totalRetrievals > 3) {
				highFeedbackCount++;
			}
		}

		const ratio = highFeedbackCount / resultIds.length;
		let newWeights: Required<WeightProfile>;
		if (ratio > 0.5) {
			newWeights = {
				vector: currentWeights.vector,
				recency: currentWeights.recency,
				frequency: currentWeights.frequency + weightAdaptationRate * 0.5,
			};
		} else {
			newWeights = {
				vector: currentWeights.vector + weightAdaptationRate * 0.5,
				recency: currentWeights.recency,
				frequency: currentWeights.frequency,
			};
		}
		return normalizeWeights(newWeights);
	};

	// -- Public API -----------------------------------------------------------

	const recordQuery = (
		queryEmbedding: readonly number[],
		resultIds: readonly string[],
		options?: { readonly topic?: string },
	): void => {
		if (!enabled) return;
		if (queryEmbedding.length === 0 || resultIds.length === 0) return;

		const now = Date.now();
		totalQueriesCount++;
		lastUpdated = now;

		// Add to global query history (FIFO capped)
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

		// Adapt global weights
		adaptedWeights = adaptWeightsForResults(adaptedWeights, resultIds);

		// Recompute global interest embedding
		recomputeInterestEmbedding();

		// Update per-topic state if topic provided
		const topic = options?.topic;
		if (topic) {
			const topicState = getOrCreateTopicState(topic);
			topicState.queryCount++;

			// Add to topic query history (FIFO capped)
			topicState.queryHistory.push(record);
			if (topicState.queryHistory.length > maxQueryHistory) {
				topicState.queryHistory = topicState.queryHistory.slice(
					-maxQueryHistory,
				);
			}

			// Adapt topic weights
			topicState.weights = adaptWeightsForResults(
				topicState.weights,
				resultIds,
			);

			// Recompute topic interest embedding
			topicState.interestEmbedding = computeInterestEmbeddingFromHistory(
				topicState.queryHistory,
			);
		}
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

	const getAdaptedWeights = (
		topic?: string,
	): Readonly<Required<WeightProfile>> => {
		if (topic) {
			const topicState = topicStates.get(topic);
			if (topicState && topicState.queryCount >= TOPIC_QUERY_THRESHOLD) {
				return { ...topicState.weights };
			}
		}
		return { ...adaptedWeights };
	};

	const getInterestEmbedding = (
		topic?: string,
	): readonly number[] | undefined => {
		if (topic) {
			const topicState = topicStates.get(topic);
			if (topicState?.interestEmbedding) {
				return [...topicState.interestEmbedding];
			}
			return undefined;
		}
		return interestEmbedding ? [...interestEmbedding] : undefined;
	};

	const computeBoost = (
		entryId: string,
		entryEmbedding: readonly number[],
		topic?: string,
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

		// Interest alignment component: use topic-specific interest if available,
		// otherwise fall back to global interest embedding
		let effectiveInterest: number[] | undefined;
		if (topic) {
			const topicState = topicStates.get(topic);
			effectiveInterest = topicState?.interestEmbedding;
		}
		if (!effectiveInterest) {
			effectiveInterest = interestEmbedding;
		}

		if (
			effectiveInterest &&
			entryEmbedding.length === effectiveInterest.length
		) {
			const similarity = cosineSimilarity(
				[...entryEmbedding],
				effectiveInterest,
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

		// Serialize per-topic state
		const serializedTopicProfiles: TopicProfileEntry[] = [];
		for (const [topic, state] of topicStates) {
			serializedTopicProfiles.push({
				topic,
				weights: { ...state.weights },
				interestEmbedding: state.interestEmbedding
					? encodeEmbedding(state.interestEmbedding)
					: undefined,
				queryCount: state.queryCount,
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
			topicProfiles:
				serializedTopicProfiles.length > 0
					? serializedTopicProfiles
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

		// Restore per-topic state
		topicStates.clear();
		if (state.topicProfiles) {
			for (const profile of state.topicProfiles) {
				const topicState: TopicState = {
					weights: normalizeWeights({
						vector: profile.weights.vector,
						recency: profile.weights.recency,
						frequency: profile.weights.frequency,
					}),
					interestEmbedding: undefined,
					queryCount: profile.queryCount,
					queryHistory: [], // not persisted — rebuilt from future queries
				};

				if (profile.interestEmbedding) {
					try {
						topicState.interestEmbedding = decodeEmbedding(
							profile.interestEmbedding,
						);
					} catch {
						// Skip corrupt embeddings
					}
				}

				topicStates.set(profile.topic, topicState);
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
		topicStates.clear();
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
