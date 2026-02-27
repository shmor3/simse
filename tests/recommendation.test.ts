import { describe, expect, it } from 'bun:test';
import {
	computeRecommendationScore,
	frequencyScore,
	normalizeWeights,
	recencyScore,
} from '../src/ai/library/recommendation.js';

// ---------------------------------------------------------------------------
// normalizeWeights
// ---------------------------------------------------------------------------

describe('normalizeWeights', () => {
	it('returns defaults when called with no arguments', () => {
		const w = normalizeWeights();
		expect(w.vector).toBeCloseTo(0.6, 5);
		expect(w.recency).toBeCloseTo(0.2, 5);
		expect(w.frequency).toBeCloseTo(0.2, 5);
		expect(w.vector + w.recency + w.frequency).toBeCloseTo(1.0, 5);
	});

	it('returns defaults when called with undefined', () => {
		const w = normalizeWeights(undefined);
		expect(w.vector + w.recency + w.frequency).toBeCloseTo(1.0, 5);
	});

	it('normalizes partial overrides to sum to 1', () => {
		const w = normalizeWeights({ vector: 1.0 });
		// vector=1.0, recency=0.2, frequency=0.2 → total=1.4
		expect(w.vector + w.recency + w.frequency).toBeCloseTo(1.0, 5);
		// vector should be the dominant weight
		expect(w.vector).toBeGreaterThan(w.recency);
		expect(w.vector).toBeGreaterThan(w.frequency);
	});

	it('normalizes all-custom weights to sum to 1', () => {
		const w = normalizeWeights({ vector: 2, recency: 1, frequency: 1 });
		expect(w.vector + w.recency + w.frequency).toBeCloseTo(1.0, 5);
		expect(w.vector).toBeCloseTo(0.5, 5);
		expect(w.recency).toBeCloseTo(0.25, 5);
		expect(w.frequency).toBeCloseTo(0.25, 5);
	});

	it('returns defaults when all weights are zero', () => {
		const w = normalizeWeights({ vector: 0, recency: 0, frequency: 0 });
		expect(w.vector).toBeCloseTo(0.6, 5);
		expect(w.recency).toBeCloseTo(0.2, 5);
		expect(w.frequency).toBeCloseTo(0.2, 5);
	});

	it('handles equal weights', () => {
		const w = normalizeWeights({ vector: 1, recency: 1, frequency: 1 });
		expect(w.vector).toBeCloseTo(1 / 3, 5);
		expect(w.recency).toBeCloseTo(1 / 3, 5);
		expect(w.frequency).toBeCloseTo(1 / 3, 5);
	});
});

// ---------------------------------------------------------------------------
// recencyScore
// ---------------------------------------------------------------------------

describe('recencyScore', () => {
	it('returns 1.0 for an entry at current time', () => {
		const now = Date.now();
		const score = recencyScore(now, { now });
		expect(score).toBeCloseTo(1.0, 5);
	});

	it('returns ~0.5 at the half-life boundary', () => {
		const now = Date.now();
		const halfLifeMs = 10_000;
		const timestamp = now - halfLifeMs;
		const score = recencyScore(timestamp, { halfLifeMs, now });
		expect(score).toBeCloseTo(0.5, 2);
	});

	it('returns ~0.25 at double the half-life', () => {
		const now = Date.now();
		const halfLifeMs = 10_000;
		const timestamp = now - halfLifeMs * 2;
		const score = recencyScore(timestamp, { halfLifeMs, now });
		expect(score).toBeCloseTo(0.25, 2);
	});

	it('returns a value between 0 and 1 for old entries', () => {
		const now = Date.now();
		const score = recencyScore(now - 365 * 24 * 60 * 60 * 1000, { now });
		expect(score).toBeGreaterThan(0);
		expect(score).toBeLessThan(1);
	});

	it('treats future timestamps as age 0 (score = 1)', () => {
		const now = Date.now();
		const score = recencyScore(now + 1000, { now });
		expect(score).toBeCloseTo(1.0, 5);
	});

	it('uses defaults when options omitted', () => {
		const score = recencyScore(Date.now());
		expect(score).toBeCloseTo(1.0, 1);
	});
});

// ---------------------------------------------------------------------------
// frequencyScore
// ---------------------------------------------------------------------------

describe('frequencyScore', () => {
	it('returns 0 when maxAccessCount is 0', () => {
		expect(frequencyScore(5, 0)).toBe(0);
	});

	it('returns 0 when maxAccessCount is negative', () => {
		expect(frequencyScore(5, -1)).toBe(0);
	});

	it('returns 1 when accessCount equals maxAccessCount', () => {
		expect(frequencyScore(10, 10)).toBeCloseTo(1.0, 5);
	});

	it('returns 0 when accessCount is 0', () => {
		expect(frequencyScore(0, 10)).toBe(0);
	});

	it('returns a value between 0 and 1 for intermediate counts', () => {
		const score = frequencyScore(5, 10);
		expect(score).toBeGreaterThan(0);
		expect(score).toBeLessThan(1);
	});

	it('uses logarithmic scaling (diminishing returns)', () => {
		// Going from 1 to 5 should have more impact than 5 to 10
		const score1 = frequencyScore(1, 100);
		const score5 = frequencyScore(5, 100);
		const score10 = frequencyScore(10, 100);

		const delta1to5 = score5 - score1;
		const delta5to10 = score10 - score5;
		expect(delta1to5).toBeGreaterThan(delta5to10);
	});
});

// ---------------------------------------------------------------------------
// computeRecommendationScore
// ---------------------------------------------------------------------------

describe('computeRecommendationScore', () => {
	it('returns 0 when all inputs are undefined', () => {
		const result = computeRecommendationScore(
			{},
			{ vector: 0.6, recency: 0.2, frequency: 0.2 },
		);
		expect(result.score).toBe(0);
		expect(result.scores.vector).toBeUndefined();
		expect(result.scores.recency).toBeUndefined();
		expect(result.scores.frequency).toBeUndefined();
	});

	it('computes weighted sum of all components', () => {
		const weights = { vector: 0.5, recency: 0.3, frequency: 0.2 };
		const result = computeRecommendationScore(
			{ vectorScore: 0.8, recencyScore: 0.6, frequencyScore: 0.4 },
			weights,
		);

		const expected = 0.8 * 0.5 + 0.6 * 0.3 + 0.4 * 0.2;
		expect(result.score).toBeCloseTo(expected, 5);
		expect(result.scores.vector).toBe(0.8);
		expect(result.scores.recency).toBe(0.6);
		expect(result.scores.frequency).toBe(0.4);
	});

	it('handles only vector score', () => {
		const weights = { vector: 0.6, recency: 0.2, frequency: 0.2 };
		const result = computeRecommendationScore({ vectorScore: 1.0 }, weights);
		expect(result.score).toBeCloseTo(0.6, 5);
	});

	it('handles only recency score', () => {
		const weights = { vector: 0.6, recency: 0.2, frequency: 0.2 };
		const result = computeRecommendationScore({ recencyScore: 1.0 }, weights);
		expect(result.score).toBeCloseTo(0.2, 5);
	});

	it('handles only frequency score', () => {
		const weights = { vector: 0.6, recency: 0.2, frequency: 0.2 };
		const result = computeRecommendationScore({ frequencyScore: 1.0 }, weights);
		expect(result.score).toBeCloseTo(0.2, 5);
	});

	it('preserves individual score components in result', () => {
		const weights = { vector: 0.5, recency: 0.3, frequency: 0.2 };
		const result = computeRecommendationScore(
			{ vectorScore: 0.9, frequencyScore: 0.7 },
			weights,
		);
		expect(result.scores.vector).toBe(0.9);
		expect(result.scores.recency).toBeUndefined();
		expect(result.scores.frequency).toBe(0.7);
	});
});
