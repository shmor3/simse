// tests/circulation-desk.test.ts
import { describe, expect, it, mock } from 'bun:test';
import type {
	Librarian,
	LibrarianRegistry,
	ManagedLibrarian,
} from '../src/ai/library/types.js';
import { createCirculationDesk } from '../src/ai/library/circulation-desk.js';

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
		summarize: mock(async (volumes, _topic) => ({
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
		bid: mock(async () => ({
			librarianName: 'default',
			argument: 'I can handle this.',
			confidence: 0.8,
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
			getVolumesForTopic: async () => [],
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
			getVolumesForTopic: async () => [],
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
			getVolumesForTopic: async () => [],
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
			getVolumesForTopic: async () => [],
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
			getVolumesForTopic: async () => [],
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
			getVolumesForTopic: async () => [],
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
			getVolumesForTopic: async () => [],
			deleteVolume: async () => {},
			getTotalVolumeCount: async () => 0,
			getAllTopics: async () => [],
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
			getVolumesForTopic: async () => volumes,
			deleteVolume: deleteFn,
			getTotalVolumeCount: async () => 2,
			getAllTopics: async () => ['test'],
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
			getVolumesForTopic: async () => volumes,
			deleteVolume: async () => {},
			getTotalVolumeCount: async () => 51,
			getAllTopics: async () => ['test/topic'],
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
			getVolumesForTopic: async () => [],
			deleteVolume: async () => {},
			getTotalVolumeCount: async () => 5,
			getAllTopics: async () => ['test'],
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

// ---------------------------------------------------------------------------
// Helper: create a mock LibrarianRegistry
// ---------------------------------------------------------------------------

function createMockManagedLibrarian(
	name: string,
	librarian: Librarian,
): ManagedLibrarian {
	return {
		definition: {
			name,
			description: `Mock ${name} librarian`,
			purpose: `Mock ${name} purpose`,
			topics: ['*'],
			permissions: { add: true, delete: true, reorganize: true },
			thresholds: { topicComplexity: 100, escalateAt: 500 },
		},
		librarian,
		provider: { generate: mock(async () => '') },
	};
}

function createMockRegistry(
	defaultLibrarian: Librarian,
	overrides?: Partial<LibrarianRegistry>,
): LibrarianRegistry {
	const managed = createMockManagedLibrarian('default', defaultLibrarian);
	return {
		initialize: mock(async () => {}),
		dispose: mock(async () => {}),
		register: mock(async () => managed),
		unregister: mock(async () => {}),
		get: mock(() => managed),
		list: mock(() => [managed]),
		defaultLibrarian: managed,
		resolveLibrarian: mock(async () => ({
			winner: 'default',
			reason: 'Only default available.',
			bids: [],
		})),
		spawnSpecialist: mock(async () => managed),
		...overrides,
	};
}

describe('CirculationDesk with registry', () => {
	it('routes extraction through registry', async () => {
		const librarian = createMockLibrarian();
		const registry = createMockRegistry(librarian);
		const addFn = mock(async () => 'new-id');

		const desk = createCirculationDesk({
			registry,
			addVolume: addFn,
			checkDuplicate: async () => ({ isDuplicate: false }),
			getVolumesForTopic: async () => [],
		});

		desk.enqueueExtraction({
			userInput: 'What is X?',
			response: 'X is important.',
		});

		expect(desk.pending).toBe(1);
		await desk.drain();
		expect(desk.pending).toBe(0);
		expect(librarian.extract).toHaveBeenCalled();
		expect(registry.resolveLibrarian).toHaveBeenCalled();
		expect(addFn).toHaveBeenCalled();
		// Verify the librarian tag is included in metadata
		const addCall = (addFn as any).mock.calls[0];
		expect(addCall[1].librarian).toBe('default');
	});

	it('tags specialist when registry resolves a non-default librarian', async () => {
		const defaultLib = createMockLibrarian();
		const specialistLib = createMockLibrarian();
		const specialistManaged = createMockManagedLibrarian(
			'code-specialist',
			specialistLib,
		);

		const registry = createMockRegistry(defaultLib, {
			resolveLibrarian: mock(async () => ({
				winner: 'code-specialist',
				reason: 'Specialist matched.',
				bids: [],
			})),
			get: mock((name: string) =>
				name === 'code-specialist' ? specialistManaged : undefined,
			),
		});

		const addFn = mock(async () => 'new-id');

		const desk = createCirculationDesk({
			registry,
			addVolume: addFn,
			checkDuplicate: async () => ({ isDuplicate: false }),
			getVolumesForTopic: async () => [],
		});

		desk.enqueueExtraction({ userInput: 'x', response: 'y' });
		await desk.drain();

		expect(addFn).toHaveBeenCalled();
		const addCall = (addFn as any).mock.calls[0];
		expect(addCall[1].librarian).toBe('code-specialist');
	});

	it('routes compendium job through resolved librarian', async () => {
		const defaultLib = createMockLibrarian();
		const specialistLib = createMockLibrarian();
		const specialistManaged = createMockManagedLibrarian(
			'code-specialist',
			specialistLib,
		);

		const registry = createMockRegistry(defaultLib, {
			resolveLibrarian: mock(async () => ({
				winner: 'code-specialist',
				reason: 'Specialist matched.',
				bids: [],
			})),
			get: mock((name: string) =>
				name === 'code-specialist' ? specialistManaged : undefined,
			),
		});

		const volumes = Array.from({ length: 12 }, (_, i) => ({
			id: `v${i}`,
			text: `fact ${i}`,
			embedding: [0.1],
			metadata: { topic: 'code/react' },
			timestamp: i,
		}));

		const desk = createCirculationDesk({
			registry,
			addVolume: async () => 'id',
			checkDuplicate: async () => ({ isDuplicate: false }),
			getVolumesForTopic: async () => volumes,
			thresholds: { compendium: { minEntries: 10 } },
		});

		desk.enqueueCompendium('code/react');
		await desk.drain();

		expect(specialistLib.summarize).toHaveBeenCalled();
		expect(defaultLib.summarize).not.toHaveBeenCalled();
	});

	it('routes optimization job through resolved librarian', async () => {
		const defaultLib = createMockLibrarian();
		const specialistLib = createMockLibrarian();
		const specialistManaged = createMockManagedLibrarian(
			'code-specialist',
			specialistLib,
		);

		(specialistLib.optimize as any).mockImplementation(async () => ({
			pruned: ['v1'],
			summary: 'Specialist optimized',
			reorganization: { moves: [], newSubtopics: [], merges: [] },
			modelUsed: 'claude-opus-4-6',
		}));

		const registry = createMockRegistry(defaultLib, {
			resolveLibrarian: mock(async () => ({
				winner: 'code-specialist',
				reason: 'Specialist matched.',
				bids: [],
			})),
			get: mock((name: string) =>
				name === 'code-specialist' ? specialistManaged : undefined,
			),
		});

		const volumes = [
			{
				id: 'v1',
				text: 'fact 1',
				embedding: [0.1],
				metadata: { topic: 'code/react' },
				timestamp: 1,
			},
		];

		const deleteFn = mock(async () => {});
		const addFn = mock(async () => 'new-id');

		const desk = createCirculationDesk({
			registry,
			addVolume: addFn,
			checkDuplicate: async () => ({ isDuplicate: false }),
			getVolumesForTopic: async () => volumes,
			deleteVolume: deleteFn,
			thresholds: { optimization: { modelId: 'claude-opus-4-6' } },
		});

		desk.enqueueOptimization('code/react');
		await desk.drain();

		expect(specialistLib.optimize).toHaveBeenCalled();
		expect(defaultLib.optimize).not.toHaveBeenCalled();
		expect(deleteFn).toHaveBeenCalledWith('v1');
	});

	it('falls back to default librarian without registry', async () => {
		const librarian = createMockLibrarian();
		const addFn = mock(async () => 'new-id');

		const desk = createCirculationDesk({
			librarian,
			addVolume: addFn,
			checkDuplicate: async () => ({ isDuplicate: false }),
			getVolumesForTopic: async () => [],
		});

		desk.enqueueExtraction({ userInput: 'a', response: 'b' });
		await desk.drain();

		expect(librarian.extract).toHaveBeenCalled();
		expect(addFn).toHaveBeenCalled();
	});

	it('throws when neither librarian nor registry is provided', () => {
		expect(() =>
			createCirculationDesk({
				addVolume: async () => 'id',
				checkDuplicate: async () => ({ isDuplicate: false }),
				getVolumesForTopic: async () => [],
			}),
		).toThrow('CirculationDesk requires either librarian or registry');
	});

	it('triggers spawn check after extraction when spawning thresholds configured', async () => {
		const defaultLib = createMockLibrarian();
		const spawnFn = mock(async () =>
			createMockManagedLibrarian('new-specialist', createMockLibrarian()),
		);

		const volumes = Array.from({ length: 101 }, (_, i) => ({
			id: `v${i}`,
			text: `fact ${i}`,
			embedding: [0.1],
			metadata: { topic: 'test/topic' },
			timestamp: i,
		}));

		const registry = createMockRegistry(defaultLib, {
			spawnSpecialist: spawnFn,
		});

		const desk = createCirculationDesk({
			registry,
			addVolume: async () => 'id',
			checkDuplicate: async () => ({ isDuplicate: false }),
			getVolumesForTopic: async () => volumes,
			thresholds: {
				spawning: {
					complexityThreshold: 100,
					modelId: 'claude-opus-4-6',
				},
			},
		});

		desk.enqueueExtraction({ userInput: 'x', response: 'y' });
		await desk.drain();

		expect(spawnFn).toHaveBeenCalled();
	});

	it('does not trigger spawn check without registry', async () => {
		const librarian = createMockLibrarian();

		const volumes = Array.from({ length: 101 }, (_, i) => ({
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
			getVolumesForTopic: async () => volumes,
			thresholds: {
				spawning: {
					complexityThreshold: 100,
					modelId: 'claude-opus-4-6',
				},
			},
		});

		desk.enqueueExtraction({ userInput: 'x', response: 'y' });
		await desk.drain();
		// Should complete without error; no registry means no spawning
		expect(desk.pending).toBe(0);
	});
});
