// ---------------------------------------------------------------------------
// Recommendation Engine — scoring functions for memory recommendations
// ---------------------------------------------------------------------------
//
// Pure functions for computing recommendation scores combining vector
// similarity, recency, and access frequency. No side effects.
// ---------------------------------------------------------------------------

import type { WeightProfile } from './types.js';

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

export interface RecencyOptions {
	/** Half-life in milliseconds for the exponential decay. Defaults to 30 days. */
	readonly halfLifeMs?: number;
}

const DEFAULT_HALF_LIFE_MS = 30 * 24 * 60 * 60 * 1000; // 30 days

// ---------------------------------------------------------------------------
// Weight normalization
// ---------------------------------------------------------------------------

const DEFAULT_WEIGHTS: Readonly<Required<WeightProfile>> = {
	vector: 0.6,
	recency: 0.2,
	frequency: 0.2,
};

/**
 * Normalize a partial weight profile so all components sum to 1.
 * Missing weights use defaults, then the whole profile is scaled.
 */
export function normalizeWeights(
	weights?: WeightProfile,
): Required<WeightProfile> {
	const raw = {
		vector: weights?.vector ?? DEFAULT_WEIGHTS.vector,
		recency: weights?.recency ?? DEFAULT_WEIGHTS.recency,
		frequency: weights?.frequency ?? DEFAULT_WEIGHTS.frequency,
	};

	const total = raw.vector + raw.recency + raw.frequency;
	if (total === 0) return { ...DEFAULT_WEIGHTS };

	return {
		vector: raw.vector / total,
		recency: raw.recency / total,
		frequency: raw.frequency / total,
	};
}

// ---------------------------------------------------------------------------
// Individual scoring functions
// ---------------------------------------------------------------------------

/**
 * Compute a recency score using exponential decay.
 *
 * Returns a value between 0 and 1. Entries at `now` get 1.0, entries
 * at `halfLifeMs` ago get ~0.5, older entries approach 0.
 */
export function recencyScore(
	entryTimestamp: number,
	options?: RecencyOptions & { readonly now?: number },
): number {
	const now = options?.now ?? Date.now();
	const halfLife = options?.halfLifeMs ?? DEFAULT_HALF_LIFE_MS;
	const ageMs = Math.max(0, now - entryTimestamp);
	const lambda = Math.LN2 / halfLife;
	return Math.exp(-lambda * ageMs);
}

/**
 * Compute a frequency score using logarithmic scaling.
 *
 * Returns a value between 0 and 1. `log(1 + count) / log(1 + max)`
 * ensures diminishing returns for very high access counts.
 */
export function frequencyScore(
	accessCount: number,
	maxAccessCount: number,
): number {
	if (maxAccessCount <= 0) return 0;
	return Math.log(1 + accessCount) / Math.log(1 + maxAccessCount);
}

// ---------------------------------------------------------------------------
// Combined recommendation score
// ---------------------------------------------------------------------------

export interface RecommendationScoreInput {
	readonly vectorScore?: number;
	readonly recencyScore?: number;
	readonly frequencyScore?: number;
}

export interface RecommendationScoreResult {
	readonly score: number;
	readonly scores: {
		readonly vector?: number;
		readonly recency?: number;
		readonly frequency?: number;
	};
}

/**
 * Compute a weighted recommendation score combining multiple signals.
 */
export function computeRecommendationScore(
	input: RecommendationScoreInput,
	weights: Required<WeightProfile>,
): RecommendationScoreResult {
	let score = 0;

	if (input.vectorScore !== undefined) {
		score += input.vectorScore * weights.vector;
	}
	if (input.recencyScore !== undefined) {
		score += input.recencyScore * weights.recency;
	}
	if (input.frequencyScore !== undefined) {
		score += input.frequencyScore * weights.frequency;
	}

	return {
		score,
		scores: {
			vector: input.vectorScore,
			recency: input.recencyScore,
			frequency: input.frequencyScore,
		},
	};
}
