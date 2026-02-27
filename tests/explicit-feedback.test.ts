import { describe, expect, it } from 'bun:test';
import { createLearningEngine } from '../src/ai/library/patron-learning.js';

describe('explicit relevance feedback', () => {
	it('positive feedback boosts relevance score', () => {
		const engine = createLearningEngine({ enabled: true });
		engine.recordQuery([1, 0, 0], ['entry-1']);

		const before = engine.getRelevanceFeedback('entry-1');
		engine.recordFeedback('entry-1', true);
		const after = engine.getRelevanceFeedback('entry-1');

		expect(after!.relevanceScore).toBeGreaterThan(before!.relevanceScore);
	});

	it('negative feedback reduces relevance score', () => {
		const engine = createLearningEngine({ enabled: true });
		engine.recordQuery([1, 0, 0], ['entry-1']);
		engine.recordFeedback('entry-1', true);
		engine.recordFeedback('entry-1', true);

		const before = engine.getRelevanceFeedback('entry-1');
		engine.recordFeedback('entry-1', false);
		const after = engine.getRelevanceFeedback('entry-1');

		expect(after!.relevanceScore).toBeLessThan(before!.relevanceScore);
	});

	it('feedback persists through serialize/restore', () => {
		const engine = createLearningEngine({ enabled: true });
		engine.recordQuery([1, 0, 0], ['entry-1']);
		engine.recordFeedback('entry-1', true);

		const state = engine.serialize();
		const engine2 = createLearningEngine({ enabled: true });
		engine2.restore(state);

		const feedback = engine2.getRelevanceFeedback('entry-1');
		expect(feedback).toBeDefined();
		expect(feedback!.relevanceScore).toBeGreaterThan(0);
	});

	it('explicit feedback weighted stronger than implicit', () => {
		const engine1 = createLearningEngine({ enabled: true });
		// Implicit only: 5 retrievals
		for (let i = 0; i < 5; i++) {
			engine1.recordQuery([1, 0, 0], ['entry-1']);
		}
		const implicitScore =
			engine1.getRelevanceFeedback('entry-1')!.relevanceScore;

		const engine2 = createLearningEngine({ enabled: true });
		engine2.recordQuery([1, 0, 0], ['entry-1']);
		engine2.recordFeedback('entry-1', true);
		const explicitScore =
			engine2.getRelevanceFeedback('entry-1')!.relevanceScore;

		expect(explicitScore).toBeGreaterThan(implicitScore);
	});

	it('feedback for unknown entry is silently ignored or creates entry', () => {
		const engine = createLearningEngine({ enabled: true });
		// No recordQuery before feedback — should not throw
		expect(() => engine.recordFeedback('unknown-entry', true)).not.toThrow();
	});

	it('pruneEntries removes feedback for deleted entries', () => {
		const engine = createLearningEngine({ enabled: true });
		engine.recordQuery([1, 0, 0], ['entry-1', 'entry-2']);
		engine.recordFeedback('entry-1', true);
		engine.recordFeedback('entry-2', true);

		engine.pruneEntries(new Set(['entry-1']));

		expect(engine.getRelevanceFeedback('entry-1')).toBeDefined();
		expect(engine.getRelevanceFeedback('entry-2')).toBeUndefined();
	});

	it('clear removes all feedback', () => {
		const engine = createLearningEngine({ enabled: true });
		engine.recordQuery([1, 0, 0], ['entry-1']);
		engine.recordFeedback('entry-1', true);
		engine.clear();

		expect(engine.getRelevanceFeedback('entry-1')).toBeUndefined();
	});
});
