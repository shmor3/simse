import { describe, expect, it } from 'bun:test';
import { createLearningEngine } from '../src/ai/memory/learning.js';

describe('per-topic weight profiles', () => {
	it('adapts weights per topic independently', () => {
		const engine = createLearningEngine({ enabled: true });

		// Simulate queries with access-heavy results for 'news' topic
		for (let i = 0; i < 15; i++) {
			engine.recordQuery([1, 0, 0], ['news-entry'], { topic: 'news' });
		}

		// Simulate queries for 'code' topic
		for (let i = 0; i < 15; i++) {
			engine.recordQuery([0, 1, 0], ['code-entry'], { topic: 'code' });
		}

		const newsWeights = engine.getAdaptedWeights('news');
		const codeWeights = engine.getAdaptedWeights('code');

		// Both should have had enough queries (15 > 10 threshold)
		// They should have weights (may or may not differ based on heuristic)
		expect(
			newsWeights.vector + newsWeights.recency + newsWeights.frequency,
		).toBeCloseTo(1.0, 5);
		expect(
			codeWeights.vector + codeWeights.recency + codeWeights.frequency,
		).toBeCloseTo(1.0, 5);
	});

	it('falls back to global weights for topics with < 10 queries', () => {
		const engine = createLearningEngine({ enabled: true });
		engine.recordQuery([1, 0, 0], ['entry-1'], { topic: 'rare' });

		const rareWeights = engine.getAdaptedWeights('rare');
		const globalWeights = engine.getAdaptedWeights();

		expect(rareWeights).toEqual(globalWeights);
	});

	it('per-topic interest embeddings are separate', () => {
		const engine = createLearningEngine({ enabled: true });

		for (let i = 0; i < 5; i++) {
			engine.recordQuery([1, 0, 0], ['a'], { topic: 'physics' });
			engine.recordQuery([0, 1, 0], ['b'], { topic: 'cooking' });
		}

		const physicsInterest = engine.getInterestEmbedding('physics');
		const cookingInterest = engine.getInterestEmbedding('cooking');

		expect(physicsInterest).toBeDefined();
		expect(cookingInterest).toBeDefined();
		// They should point in different directions
		expect(physicsInterest![0]).not.toBeCloseTo(cookingInterest![0], 1);
	});

	it('serializes and restores per-topic state', () => {
		const engine = createLearningEngine({ enabled: true });
		for (let i = 0; i < 15; i++) {
			engine.recordQuery([1, 0, 0], ['a'], { topic: 'math' });
		}

		const state = engine.serialize();
		const engine2 = createLearningEngine({ enabled: true });
		engine2.restore(state);

		const weights = engine2.getAdaptedWeights('math');
		expect(weights).toEqual(engine.getAdaptedWeights('math'));
	});

	it('recordQuery without topic still works (backward compat)', () => {
		const engine = createLearningEngine({ enabled: true });
		engine.recordQuery([1, 0, 0], ['entry-1']);

		const feedback = engine.getRelevanceFeedback('entry-1');
		expect(feedback).toBeDefined();
	});

	it('computeBoost uses topic-specific interest', () => {
		const engine = createLearningEngine({ enabled: true });

		// Build strong interest in physics direction
		for (let i = 0; i < 10; i++) {
			engine.recordQuery([1, 0, 0], ['a'], { topic: 'physics' });
		}

		// Entry aligned with physics interest
		const boostAligned = engine.computeBoost('a', [1, 0, 0], 'physics');
		// Entry NOT aligned
		const boostMisaligned = engine.computeBoost('a', [0, 0, 1], 'physics');

		expect(boostAligned).toBeGreaterThanOrEqual(boostMisaligned);
	});

	it('clear removes per-topic state', () => {
		const engine = createLearningEngine({ enabled: true });
		for (let i = 0; i < 15; i++) {
			engine.recordQuery([1, 0, 0], ['a'], { topic: 'test' });
		}
		engine.clear();

		// After clear, should fall back to global (default) weights
		const weights = engine.getAdaptedWeights('test');
		const global = engine.getAdaptedWeights();
		expect(weights).toEqual(global);
	});

	it('topic queries also update global state', () => {
		const engine = createLearningEngine({ enabled: true });

		engine.recordQuery([1, 0, 0], ['a'], { topic: 'science' });

		// Global should also have recorded this query
		expect(engine.totalQueries).toBe(1);
		expect(engine.getInterestEmbedding()).toBeDefined();
	});

	it('serialized topicProfiles round-trip correctly', () => {
		const engine = createLearningEngine({ enabled: true });

		for (let i = 0; i < 12; i++) {
			engine.recordQuery([1, 0, 0], ['a'], { topic: 'alpha' });
		}
		for (let i = 0; i < 3; i++) {
			engine.recordQuery([0, 1, 0], ['b'], { topic: 'beta' });
		}

		const state = engine.serialize();
		expect(state.topicProfiles).toBeDefined();
		expect(state.topicProfiles!.length).toBe(2);

		const alpha = state.topicProfiles!.find((p) => p.topic === 'alpha');
		expect(alpha).toBeDefined();
		expect(alpha!.queryCount).toBe(12);
		expect(alpha!.interestEmbedding).toBeDefined();

		const beta = state.topicProfiles!.find((p) => p.topic === 'beta');
		expect(beta).toBeDefined();
		expect(beta!.queryCount).toBe(3);
	});

	it('getInterestEmbedding returns undefined for unknown topic', () => {
		const engine = createLearningEngine({ enabled: true });
		engine.recordQuery([1, 0, 0], ['a'], { topic: 'known' });

		expect(engine.getInterestEmbedding('unknown')).toBeUndefined();
	});

	it('restore clears old per-topic state before loading', () => {
		const engine = createLearningEngine({ enabled: true });
		for (let i = 0; i < 15; i++) {
			engine.recordQuery([1, 0, 0], ['a'], { topic: 'old-topic' });
		}

		// Serialize a fresh engine (no per-topic state)
		const freshEngine = createLearningEngine({ enabled: true });
		freshEngine.recordQuery([0, 1, 0], ['b']);
		const freshState = freshEngine.serialize();

		// Restore should clear old-topic
		engine.restore(freshState);
		const weights = engine.getAdaptedWeights('old-topic');
		const global = engine.getAdaptedWeights();
		expect(weights).toEqual(global);
	});
});
