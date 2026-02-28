import { describe, expect, it } from 'bun:test';
import { parseQuery } from 'simse-vector';

describe('query DSL parser', () => {
	it('parses plain text as BM25 query', () => {
		const opts = parseQuery('hello world');
		expect(opts.textSearch?.query).toBe('hello world');
		expect(opts.textSearch?.mode).toBe('bm25');
	});

	it('parses topic: prefix', () => {
		const opts = parseQuery('topic:programming/rust some query');
		expect(opts.topicFilter).toContain('programming/rust');
		expect(opts.textSearch?.query).toBe('some query');
	});

	it('parses metadata: prefix', () => {
		const opts = parseQuery('metadata:lang=rust hello');
		expect(opts.metadataFilters).toContainEqual(
			expect.objectContaining({ key: 'lang', value: 'rust', mode: 'eq' }),
		);
		expect(opts.textSearch?.query).toBe('hello');
	});

	it('parses quoted exact phrases', () => {
		const opts = parseQuery('"exact match"');
		expect(opts.textSearch?.query).toBe('exact match');
		expect(opts.textSearch?.mode).toBe('exact');
	});

	it('parses fuzzy~ prefix', () => {
		const opts = parseQuery('fuzzy~approx');
		expect(opts.textSearch?.query).toBe('approx');
		expect(opts.textSearch?.mode).toBe('fuzzy');
	});

	it('parses score> numeric filter', () => {
		const opts = parseQuery('score>0.5 some text');
		expect(opts.minScore).toBe(0.5);
		expect(opts.textSearch?.query).toBe('some text');
	});

	it('combines multiple DSL elements', () => {
		const opts = parseQuery(
			'topic:lang/rust metadata:type=tutorial "async programming"',
		);
		expect(opts.topicFilter).toContain('lang/rust');
		expect(opts.metadataFilters).toHaveLength(1);
		expect(opts.textSearch?.query).toBe('async programming');
		expect(opts.textSearch?.mode).toBe('exact');
	});

	it('handles empty query', () => {
		const opts = parseQuery('');
		expect(opts.textSearch?.query).toBe('');
	});

	it('handles multiple topics', () => {
		const opts = parseQuery('topic:rust topic:python');
		expect(opts.topicFilter).toContain('rust');
		expect(opts.topicFilter).toContain('python');
	});

	it('handles multiple metadata filters', () => {
		const opts = parseQuery('metadata:lang=rust metadata:type=lib');
		expect(opts.metadataFilters).toHaveLength(2);
	});

	it('preserves spaces in quoted strings', () => {
		const opts = parseQuery('"hello   world   test"');
		expect(opts.textSearch?.query).toBe('hello   world   test');
	});

	it('unterminated quote treats rest as token', () => {
		const opts = parseQuery('"unterminated string');
		expect(opts.textSearch?.query).toBe('unterminated string');
	});
});
