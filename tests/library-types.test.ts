import { describe, expect, it } from 'bun:test';
import type {
	Volume,
	Lookup,
	TextLookup,
	AdvancedLookup,
	DuplicateVolumes,
	CompendiumOptions,
	CompendiumResult,
	PatronProfile,
	LibraryConfig,
	Recommendation,
	DuplicateCheckResult,
} from '../src/ai/library/types.js';

describe('Library types', () => {
	it('Volume has the correct shape', () => {
		const vol: Volume = {
			id: 'v1',
			text: 'hello',
			embedding: [0.1, 0.2],
			metadata: { topic: 'test' },
			timestamp: Date.now(),
		};
		expect(vol.id).toBe('v1');
	});

	it('Lookup has volume + score', () => {
		const lookup: Lookup = {
			volume: {
				id: 'v1',
				text: 'hello',
				embedding: [0.1],
				metadata: {},
				timestamp: Date.now(),
			},
			score: 0.95,
		};
		expect(lookup.score).toBe(0.95);
	});

	it('TextLookup has volume + score', () => {
		const lookup: TextLookup = {
			volume: {
				id: 'v1',
				text: 'hello',
				embedding: [0.1],
				metadata: {},
				timestamp: Date.now(),
			},
			score: 0.8,
		};
		expect(lookup.volume.text).toBe('hello');
	});

	it('AdvancedLookup has volume + scores', () => {
		const lookup: AdvancedLookup = {
			volume: {
				id: 'v1',
				text: 'hello',
				embedding: [0.1],
				metadata: {},
				timestamp: Date.now(),
			},
			score: 0.9,
			scores: { vector: 0.9, text: 0.8 },
		};
		expect(lookup.scores.vector).toBe(0.9);
	});

	it('DuplicateVolumes has representative + duplicates', () => {
		const vol: Volume = {
			id: 'v1',
			text: 'hello',
			embedding: [0.1],
			metadata: {},
			timestamp: Date.now(),
		};
		const group: DuplicateVolumes = {
			representative: vol,
			duplicates: [vol],
			averageSimilarity: 0.99,
		};
		expect(group.averageSimilarity).toBe(0.99);
	});

	it('DuplicateCheckResult has existingVolume', () => {
		const result: DuplicateCheckResult = {
			isDuplicate: true,
			existingVolume: {
				id: 'v1',
				text: 'hello',
				embedding: [0.1],
				metadata: {},
				timestamp: Date.now(),
			},
			similarity: 0.98,
		};
		expect(result.existingVolume?.id).toBe('v1');
	});

	it('CompendiumOptions has required fields', () => {
		const opts: CompendiumOptions = {
			ids: ['a', 'b'],
		};
		expect(opts.ids.length).toBe(2);
	});

	it('CompendiumResult has compendiumId', () => {
		const result: CompendiumResult = {
			compendiumId: 'c1',
			text: 'summary',
			sourceIds: ['a', 'b'],
			deletedOriginals: false,
		};
		expect(result.compendiumId).toBe('c1');
	});

	it('PatronProfile has adaptedWeights', () => {
		const profile: PatronProfile = {
			queryHistory: [],
			adaptedWeights: { vector: 0.6, recency: 0.2, frequency: 0.2 },
			interestEmbedding: undefined,
			totalQueries: 0,
			lastUpdated: 0,
		};
		expect(profile.totalQueries).toBe(0);
	});

	it('LibraryConfig replaces MemoryConfig', () => {
		const config: LibraryConfig = {
			enabled: true,
			similarityThreshold: 0.7,
			maxResults: 10,
		};
		expect(config.enabled).toBe(true);
	});

	it('Recommendation has volume + scores', () => {
		const rec: Recommendation = {
			volume: {
				id: 'v1',
				text: 'hello',
				embedding: [0.1],
				metadata: {},
				timestamp: Date.now(),
			},
			score: 0.85,
			scores: { vector: 0.9, recency: 0.7, frequency: 0.5 },
		};
		expect(rec.volume.id).toBe('v1');
	});
});
