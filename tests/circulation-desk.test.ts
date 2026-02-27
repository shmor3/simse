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
