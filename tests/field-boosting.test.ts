import { describe, expect, it } from 'bun:test';
import {
	createMagnitudeCache,
	createMetadataIndex,
} from '../src/ai/memory/indexing.js';
import { createInvertedIndex } from '../src/ai/memory/inverted-index.js';
import type { VectorEntry } from '../src/ai/memory/types.js';
import {
	advancedVectorSearch,
	type VectorSearchConfig,
} from '../src/ai/memory/vector-search.js';

function makeEntry(
	id: string,
	text: string,
	embedding: number[],
	metadata: Record<string, string> = {},
): VectorEntry {
	return { id, text, embedding, metadata, timestamp: Date.now() };
}

const searchConfig: VectorSearchConfig = {
	maxRegexPatternLength: 256,
	warn: () => {},
};

describe('field boosting', () => {
	it('text boost increases text score influence', () => {
		const magCache = createMagnitudeCache();
		const metaIdx = createMetadataIndex();
		const invIdx = createInvertedIndex();

		const e1 = makeEntry('1', 'rust programming', [1, 0, 0]);
		const e2 = makeEntry('2', 'python scripting', [0.9, 0.1, 0]);
		const entries = [e1, e2];
		magCache.set('1', e1.embedding);
		magCache.set('2', e2.embedding);
		metaIdx.addEntry(e1.id, e1.metadata);
		metaIdx.addEntry(e2.id, e2.metadata);
		invIdx.addEntry(e1);
		invIdx.addEntry(e2);

		// Without boost
		const normalResults = advancedVectorSearch(
			entries,
			{
				queryEmbedding: [1, 0, 0],
				text: { query: 'rust', mode: 'bm25' },
				rankBy: 'average',
			},
			searchConfig,
			magCache,
			metaIdx,
			invIdx,
		);

		// With high text boost
		const boostedResults = advancedVectorSearch(
			entries,
			{
				queryEmbedding: [1, 0, 0],
				text: { query: 'rust', mode: 'bm25' },
				rankBy: 'average',
				fieldBoosts: { text: 3.0 },
			},
			searchConfig,
			magCache,
			metaIdx,
			invIdx,
		);

		// Entry 1 matches 'rust' text — boosted results should show higher score for entry 1
		const normal1 = normalResults.find((r) => r.entry.id === '1');
		const boosted1 = boostedResults.find((r) => r.entry.id === '1');
		expect(normal1).toBeDefined();
		expect(boosted1).toBeDefined();
		expect(boosted1!.score).toBeGreaterThan(normal1!.score);
	});

	it('metadata boost increases score for metadata-matching entries', () => {
		const magCache = createMagnitudeCache();
		const metaIdx = createMetadataIndex();
		const invIdx = createInvertedIndex();

		const e1 = makeEntry('1', 'some text', [1, 0, 0], { lang: 'rust' });
		const e2 = makeEntry('2', 'other text', [0.95, 0.05, 0], {
			lang: 'python',
		});
		const entries = [e1, e2];
		magCache.set('1', e1.embedding);
		magCache.set('2', e2.embedding);
		metaIdx.addEntry(e1.id, e1.metadata);
		metaIdx.addEntry(e2.id, e2.metadata);
		invIdx.addEntry(e1);
		invIdx.addEntry(e2);

		// Without metadata boost
		const normalResults = advancedVectorSearch(
			entries,
			{
				queryEmbedding: [1, 0, 0],
				metadata: [{ key: 'lang', value: 'rust' }],
				rankBy: 'vector',
			},
			searchConfig,
			magCache,
			metaIdx,
			invIdx,
		);

		// With metadata boost
		const boostedResults = advancedVectorSearch(
			entries,
			{
				queryEmbedding: [1, 0, 0],
				metadata: [{ key: 'lang', value: 'rust' }],
				rankBy: 'vector',
				fieldBoosts: { metadata: 2.0 },
			},
			searchConfig,
			magCache,
			metaIdx,
			invIdx,
		);

		// Both filter to only e1, but boosted should have a higher score
		expect(normalResults.length).toBe(1);
		expect(boostedResults.length).toBe(1);
		expect(boostedResults[0].score).toBeGreaterThan(normalResults[0].score);
	});

	it('topic boost increases score for topic-matching entries', () => {
		const magCache = createMagnitudeCache();
		const metaIdx = createMetadataIndex();
		const invIdx = createInvertedIndex();

		const e1 = makeEntry('1', 'some text', [1, 0, 0], {
			topic: 'programming',
		});
		const e2 = makeEntry('2', 'other text', [0.95, 0.05, 0], {
			topic: 'cooking',
		});
		const entries = [e1, e2];
		magCache.set('1', e1.embedding);
		magCache.set('2', e2.embedding);
		metaIdx.addEntry(e1.id, e1.metadata);
		metaIdx.addEntry(e2.id, e2.metadata);
		invIdx.addEntry(e1);
		invIdx.addEntry(e2);

		// Without topic boost
		const normalResults = advancedVectorSearch(
			entries,
			{
				queryEmbedding: [1, 0, 0],
				rankBy: 'vector',
			},
			searchConfig,
			magCache,
			metaIdx,
			invIdx,
		);

		// With topic boost — entry with 'programming' topic gets boosted
		const boostedResults = advancedVectorSearch(
			entries,
			{
				queryEmbedding: [1, 0, 0],
				rankBy: 'vector',
				fieldBoosts: { topic: 2.0 },
				topicFilter: ['programming'],
			},
			searchConfig,
			magCache,
			metaIdx,
			invIdx,
		);

		const normal1 = normalResults.find((r) => r.entry.id === '1');
		const boosted1 = boostedResults.find((r) => r.entry.id === '1');
		expect(normal1).toBeDefined();
		expect(boosted1).toBeDefined();
		expect(boosted1!.score).toBeGreaterThan(normal1!.score);
	});
});

describe('weighted ranking mode', () => {
	it('uses custom weights for score combination', () => {
		const magCache = createMagnitudeCache();
		const metaIdx = createMetadataIndex();
		const invIdx = createInvertedIndex();

		const e1 = makeEntry('1', 'rust', [1, 0, 0]);
		const e2 = makeEntry('2', 'other', [0.5, 0.5, 0]);
		const entries = [e1, e2];
		magCache.set('1', e1.embedding);
		magCache.set('2', e2.embedding);
		metaIdx.addEntry(e1.id, e1.metadata);
		metaIdx.addEntry(e2.id, e2.metadata);
		invIdx.addEntry(e1);
		invIdx.addEntry(e2);

		const results = advancedVectorSearch(
			entries,
			{
				queryEmbedding: [1, 0, 0],
				text: { query: 'rust', mode: 'bm25' },
				rankBy: 'weighted',
				rankWeights: { vector: 0.1, text: 0.9 },
			},
			searchConfig,
			magCache,
			metaIdx,
			invIdx,
		);

		expect(results.length).toBeGreaterThan(0);
		// Entry 1 matches text 'rust' strongly and has perfect vector match
		expect(results[0].entry.id).toBe('1');
	});

	it('weighted ranking with zero vector weight ignores vector scores', () => {
		const magCache = createMagnitudeCache();
		const metaIdx = createMetadataIndex();
		const invIdx = createInvertedIndex();

		// Entry 1: bad vector match, good text match
		const e1 = makeEntry('1', 'machine learning', [0, 0, 1]);
		// Entry 2: good vector match, no text match
		const e2 = makeEntry('2', 'something else', [1, 0, 0]);
		const entries = [e1, e2];
		magCache.set('1', e1.embedding);
		magCache.set('2', e2.embedding);
		metaIdx.addEntry(e1.id, e1.metadata);
		metaIdx.addEntry(e2.id, e2.metadata);
		invIdx.addEntry(e1);
		invIdx.addEntry(e2);

		const results = advancedVectorSearch(
			entries,
			{
				queryEmbedding: [1, 0, 0],
				text: { query: 'machine learning', mode: 'bm25' },
				rankBy: 'weighted',
				rankWeights: { vector: 0, text: 1 },
			},
			searchConfig,
			magCache,
			metaIdx,
			invIdx,
		);

		// With zero vector weight, text-matching entry should rank first
		expect(results[0].entry.id).toBe('1');
	});

	it('weighted ranking includes recency component', () => {
		const magCache = createMagnitudeCache();
		const metaIdx = createMetadataIndex();
		const invIdx = createInvertedIndex();

		const now = Date.now();
		// Entry 1: recent, weaker vector match
		const e1: VectorEntry = {
			id: '1',
			text: 'recent entry',
			embedding: [0.7, 0.3, 0],
			metadata: {},
			timestamp: now,
		};
		// Entry 2: old, stronger vector match
		const e2: VectorEntry = {
			id: '2',
			text: 'old entry',
			embedding: [0.95, 0.05, 0],
			metadata: {},
			timestamp: now - 365 * 24 * 60 * 60 * 1000, // 1 year ago
		};
		const entries = [e1, e2];
		magCache.set('1', e1.embedding);
		magCache.set('2', e2.embedding);
		metaIdx.addEntry(e1.id, e1.metadata);
		metaIdx.addEntry(e2.id, e2.metadata);
		invIdx.addEntry(e1);
		invIdx.addEntry(e2);

		// Heavy recency weight should prefer the recent entry
		const results = advancedVectorSearch(
			entries,
			{
				queryEmbedding: [1, 0, 0],
				rankBy: 'weighted',
				rankWeights: { vector: 0.1, recency: 0.9 },
			},
			searchConfig,
			magCache,
			metaIdx,
			invIdx,
		);

		expect(results.length).toBe(2);
		expect(results[0].entry.id).toBe('1');
	});

	it('default rankWeights are used when not specified', () => {
		const magCache = createMagnitudeCache();
		const metaIdx = createMetadataIndex();
		const invIdx = createInvertedIndex();

		const e1 = makeEntry('1', 'test', [1, 0, 0]);
		const entries = [e1];
		magCache.set('1', e1.embedding);
		metaIdx.addEntry(e1.id, e1.metadata);
		invIdx.addEntry(e1);

		const results = advancedVectorSearch(
			entries,
			{
				queryEmbedding: [1, 0, 0],
				text: { query: 'test', mode: 'bm25' },
				rankBy: 'weighted',
			},
			searchConfig,
			magCache,
			metaIdx,
			invIdx,
		);

		expect(results.length).toBe(1);
		expect(results[0].score).toBeGreaterThan(0);
	});
});
