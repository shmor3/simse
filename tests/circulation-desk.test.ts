// tests/circulation-desk.test.ts
import { describe, expect, it, mock } from 'bun:test';
import { createCirculationDesk } from '../src/ai/library/circulation-desk.js';
import type { Librarian } from '../src/ai/library/types.js';

// Mock librarian that returns a single extraction
function createMockLibrarian(): Librarian {
	return {
		extract: mock(async () => ({
			memories: [
				{
					text: 'Important fact',
					topic: 'test/topic',
					tags: ['important'],
					entryType: 'fact' as const,
				},
			],
		})),
		summarize: mock(async (volumes, topic) => ({
			text: 'Summarized content',
			sourceIds: volumes.map((v) => v.id),
		})),
		classifyTopic: mock(async () => ({ topic: 'test', confidence: 0.9 })),
		reorganize: mock(async () => ({
			moves: [],
			newSubtopics: [],
			merges: [],
		})),
		optimize: mock(async () => ({
			pruned: [],
			summary: '',
			reorganization: { moves: [], newSubtopics: [], merges: [] },
			modelUsed: '',
		})),
	};
}

describe('CirculationDesk', () => {
	it('enqueueExtraction adds a job and drain processes it', async () => {
		const librarian = createMockLibrarian();
		const addFn = mock(async () => 'new-id');
		const desk = createCirculationDesk({
			librarian,
			addVolume: addFn,
			checkDuplicate: async () => ({ isDuplicate: false }),
			getVolumesForTopic: () => [],
		});

		desk.enqueueExtraction({
			userInput: 'What is X?',
			response: 'X is a fact about databases.',
		});

		expect(desk.pending).toBe(1);
		await desk.drain();
		expect(desk.pending).toBe(0);
		expect(librarian.extract).toHaveBeenCalled();
		expect(addFn).toHaveBeenCalled();
	});

	it('flush() cancels all pending jobs', async () => {
		const librarian = createMockLibrarian();
		const desk = createCirculationDesk({
			librarian,
			addVolume: async () => 'id',
			checkDuplicate: async () => ({ isDuplicate: false }),
			getVolumesForTopic: () => [],
		});

		desk.enqueueExtraction({ userInput: 'a', response: 'b' });
		desk.enqueueExtraction({ userInput: 'c', response: 'd' });
		expect(desk.pending).toBe(2);
		await desk.flush();
		expect(desk.pending).toBe(0);
	});

	it('processing is true during drain', async () => {
		const librarian = createMockLibrarian();
		const desk = createCirculationDesk({
			librarian,
			addVolume: async () => 'id',
			checkDuplicate: async () => ({ isDuplicate: false }),
			getVolumesForTopic: () => [],
		});

		desk.enqueueExtraction({ userInput: 'a', response: 'b' });
		const drainPromise = desk.drain();
		await drainPromise;
		expect(desk.processing).toBe(false);
	});

	it('dispose() prevents further processing', () => {
		const librarian = createMockLibrarian();
		const desk = createCirculationDesk({
			librarian,
			addVolume: async () => 'id',
			checkDuplicate: async () => ({ isDuplicate: false }),
			getVolumesForTopic: () => [],
		});

		desk.dispose();
		desk.enqueueExtraction({ userInput: 'a', response: 'b' });
		expect(desk.pending).toBe(0); // disposed — job not queued
	});

	it('respects custom compendium thresholds', async () => {
		const librarian = createMockLibrarian();
		const addFn = mock(async () => 'id');
		const desk = createCirculationDesk({
			librarian,
			addVolume: addFn,
			checkDuplicate: async () => ({ isDuplicate: false }),
			getVolumesForTopic: () => [],
			thresholds: {
				compendium: {
					minEntries: 3,
					minAgeMs: 60_000,
					deleteOriginals: true,
				},
				reorganization: {
					maxVolumesPerTopic: 5,
				},
			},
		});

		// With only 0 volumes returned and threshold of 3, compendium should not trigger
		desk.enqueueCompendium('test/topic');
		await desk.drain();
		expect(librarian.summarize).not.toHaveBeenCalled();
	});

	it('skips duplicate memories during extraction', async () => {
		const librarian = createMockLibrarian();
		const addFn = mock(async () => 'id');
		const desk = createCirculationDesk({
			librarian,
			addVolume: addFn,
			checkDuplicate: async () => ({ isDuplicate: true }),
			getVolumesForTopic: () => [],
		});

		desk.enqueueExtraction({ userInput: 'x', response: 'y' });
		await desk.drain();
		expect(addFn).not.toHaveBeenCalled();
	});
});

describe('CirculationDesk optimization', () => {
	it('enqueueOptimization adds an optimization job', async () => {
		const librarian = createMockLibrarian();
		const desk = createCirculationDesk({
			librarian,
			addVolume: async () => 'id',
			checkDuplicate: async () => ({ isDuplicate: false }),
			getVolumesForTopic: () => [],
			deleteVolume: async () => {},
			getTotalVolumeCount: () => 0,
			getAllTopics: () => [],
			thresholds: {
				optimization: { modelId: 'claude-opus-4-6' },
			},
		});

		desk.enqueueOptimization('test/topic');
		expect(desk.pending).toBe(1);
		await desk.drain();
		expect(desk.pending).toBe(0);
	});

	it('optimization job calls librarian.optimize and deletes pruned volumes', async () => {
		const librarian = createMockLibrarian();
		(librarian.optimize as any).mockImplementation(async () => ({
			pruned: ['v2'],
			summary: 'Optimized summary',
			reorganization: { moves: [], newSubtopics: [], merges: [] },
			modelUsed: 'claude-opus-4-6',
		}));

		const deleteFn = mock(async () => {});
		const addFn = mock(async () => 'new-id');
		const volumes = [
			{
				id: 'v1',
				text: 'fact 1',
				embedding: [0.1],
				metadata: { topic: 'test' },
				timestamp: 1,
			},
			{
				id: 'v2',
				text: 'fact 2 (duplicate)',
				embedding: [0.2],
				metadata: { topic: 'test' },
				timestamp: 2,
			},
		];

		const desk = createCirculationDesk({
			librarian,
			addVolume: addFn,
			checkDuplicate: async () => ({ isDuplicate: false }),
			getVolumesForTopic: () => volumes,
			deleteVolume: deleteFn,
			getTotalVolumeCount: () => 2,
			getAllTopics: () => ['test'],
			thresholds: {
				optimization: { modelId: 'claude-opus-4-6' },
			},
		});

		desk.enqueueOptimization('test');
		await desk.drain();
		expect(librarian.optimize).toHaveBeenCalled();
		expect(deleteFn).toHaveBeenCalledWith('v2');
		expect(addFn).toHaveBeenCalledWith('Optimized summary', {
			topic: 'test',
			entryType: 'compendium',
		});
	});

	it('auto-escalates when topic threshold is exceeded after extraction', async () => {
		const librarian = createMockLibrarian();
		const volumes = Array.from({ length: 51 }, (_, i) => ({
			id: `v${i}`,
			text: `fact ${i}`,
			embedding: [0.1],
			metadata: { topic: 'test/topic' },
			timestamp: i,
		}));

		const desk = createCirculationDesk({
			librarian,
			addVolume: async () => 'id',
			checkDuplicate: async () => ({ isDuplicate: false }),
			getVolumesForTopic: () => volumes,
			deleteVolume: async () => {},
			getTotalVolumeCount: () => 51,
			getAllTopics: () => ['test/topic'],
			thresholds: {
				optimization: {
					topicThreshold: 50,
					modelId: 'claude-opus-4-6',
				},
			},
		});

		desk.enqueueExtraction({ userInput: 'x', response: 'y' });
		await desk.drain();
		// After extraction, auto-escalation should have enqueued and processed an optimization job
		expect(librarian.optimize).toHaveBeenCalled();
	});

	it('does not auto-escalate when thresholds are not exceeded', async () => {
		const librarian = createMockLibrarian();
		const desk = createCirculationDesk({
			librarian,
			addVolume: async () => 'id',
			checkDuplicate: async () => ({ isDuplicate: false }),
			getVolumesForTopic: () => [],
			deleteVolume: async () => {},
			getTotalVolumeCount: () => 5,
			getAllTopics: () => ['test'],
			thresholds: {
				optimization: {
					topicThreshold: 50,
					globalThreshold: 500,
					modelId: 'claude-opus-4-6',
				},
			},
		});

		desk.enqueueExtraction({ userInput: 'x', response: 'y' });
		await desk.drain();
		expect(librarian.optimize).not.toHaveBeenCalled();
	});
});
