import { describe, expect, it } from 'bun:test';
import {
	createInvertedIndex,
	tokenizeForIndex,
} from '../src/ai/memory/inverted-index.js';
import type { VectorEntry } from '../src/ai/memory/types.js';

function makeEntry(id: string, text: string): VectorEntry {
	return {
		id,
		text,
		embedding: [0.1, 0.2, 0.3],
		metadata: {},
		timestamp: Date.now(),
	};
}

describe('tokenizeForIndex', () => {
	it('lowercases and splits on whitespace', () => {
		expect(tokenizeForIndex('Hello World')).toEqual(['hello', 'world']);
	});

	it('strips punctuation', () => {
		expect(tokenizeForIndex('hello, world!')).toEqual(['hello', 'world']);
	});

	it('filters empty tokens', () => {
		expect(tokenizeForIndex('  hello   world  ')).toEqual(['hello', 'world']);
	});
});

describe('InvertedIndex', () => {
	it('indexes terms and retrieves entry IDs', () => {
		const idx = createInvertedIndex();
		idx.addEntry(makeEntry('1', 'the quick brown fox'));
		idx.addEntry(makeEntry('2', 'the lazy brown dog'));

		expect(idx.getEntries('brown')).toContain('1');
		expect(idx.getEntries('brown')).toContain('2');
		expect(idx.getEntries('fox')).toContain('1');
		expect(idx.getEntries('fox')).not.toContain('2');
	});

	it('removes entry from index', () => {
		const idx = createInvertedIndex();
		idx.addEntry(makeEntry('1', 'hello world'));
		idx.removeEntry('1', 'hello world');
		expect(idx.getEntries('hello')).toHaveLength(0);
		expect(idx.documentCount).toBe(0);
	});

	it('computes BM25 scores', () => {
		const idx = createInvertedIndex();
		idx.addEntry(makeEntry('1', 'rust programming language systems'));
		idx.addEntry(makeEntry('2', 'python programming language scripting'));
		idx.addEntry(makeEntry('3', 'rust rust rust systems low level'));

		const results = idx.bm25Search('rust programming');
		expect(results.length).toBeGreaterThan(0);
		const ids = results.map((r) => r.id);
		expect(ids).toContain('1');
		expect(ids).toContain('3');
	});

	it('returns empty for unknown terms', () => {
		const idx = createInvertedIndex();
		idx.addEntry(makeEntry('1', 'hello'));
		expect(idx.bm25Search('nonexistent')).toHaveLength(0);
	});

	it('clear removes all entries', () => {
		const idx = createInvertedIndex();
		idx.addEntry(makeEntry('1', 'hello world'));
		idx.clear();
		expect(idx.getEntries('hello')).toHaveLength(0);
		expect(idx.documentCount).toBe(0);
	});

	it('tracks document count and average length', () => {
		const idx = createInvertedIndex();
		idx.addEntry(makeEntry('1', 'one two three'));
		idx.addEntry(makeEntry('2', 'four five'));
		expect(idx.documentCount).toBe(2);
		expect(idx.averageDocumentLength).toBe(2.5);
	});

	it('addEntries batch adds multiple', () => {
		const idx = createInvertedIndex();
		idx.addEntries([
			makeEntry('1', 'hello world'),
			makeEntry('2', 'hello there'),
		]);
		expect(idx.getEntries('hello')).toHaveLength(2);
		expect(idx.documentCount).toBe(2);
	});

	it('BM25 ranks documents with more matching terms higher', () => {
		const idx = createInvertedIndex();
		idx.addEntry(makeEntry('1', 'apple'));
		idx.addEntry(makeEntry('2', 'apple banana'));
		idx.addEntry(makeEntry('3', 'apple banana cherry'));

		const results = idx.bm25Search('apple banana');
		// Entry 2 and 3 have both terms, entry 1 has only one
		const scoreMap = new Map(results.map((r) => [r.id, r.score]));
		expect(scoreMap.get('2')!).toBeGreaterThan(scoreMap.get('1')!);
	});

	it('returns empty for empty query', () => {
		const idx = createInvertedIndex();
		idx.addEntry(makeEntry('1', 'hello'));
		expect(idx.bm25Search('')).toHaveLength(0);
	});
});
