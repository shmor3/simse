import { describe, expect, it } from 'bun:test';
import { matchesMetadataFilter } from '../src/ai/memory/text-search.js';
import type { MetadataFilter } from '../src/ai/memory/types.js';

describe('new metadata operators', () => {
	it('gt: matches when value is greater', () => {
		const filter: MetadataFilter = { key: 'score', value: '5', mode: 'gt' };
		expect(matchesMetadataFilter({ score: '10' }, filter)).toBe(true);
		expect(matchesMetadataFilter({ score: '3' }, filter)).toBe(false);
		expect(matchesMetadataFilter({ score: '5' }, filter)).toBe(false);
	});

	it('gte: matches when value is greater or equal', () => {
		const filter: MetadataFilter = { key: 'score', value: '5', mode: 'gte' };
		expect(matchesMetadataFilter({ score: '5' }, filter)).toBe(true);
		expect(matchesMetadataFilter({ score: '10' }, filter)).toBe(true);
		expect(matchesMetadataFilter({ score: '3' }, filter)).toBe(false);
	});

	it('lt: matches when value is less', () => {
		const filter: MetadataFilter = { key: 'score', value: '5', mode: 'lt' };
		expect(matchesMetadataFilter({ score: '3' }, filter)).toBe(true);
		expect(matchesMetadataFilter({ score: '10' }, filter)).toBe(false);
		expect(matchesMetadataFilter({ score: '5' }, filter)).toBe(false);
	});

	it('lte: matches when value is less or equal', () => {
		const filter: MetadataFilter = { key: 'score', value: '5', mode: 'lte' };
		expect(matchesMetadataFilter({ score: '5' }, filter)).toBe(true);
		expect(matchesMetadataFilter({ score: '3' }, filter)).toBe(true);
		expect(matchesMetadataFilter({ score: '10' }, filter)).toBe(false);
	});

	it('in: matches when value is in array', () => {
		const filter: MetadataFilter = {
			key: 'status',
			value: ['active', 'pending'],
			mode: 'in',
		};
		expect(matchesMetadataFilter({ status: 'active' }, filter)).toBe(true);
		expect(matchesMetadataFilter({ status: 'closed' }, filter)).toBe(false);
	});

	it('notIn: matches when value is not in array', () => {
		const filter: MetadataFilter = {
			key: 'status',
			value: ['blocked', 'closed'],
			mode: 'notIn',
		};
		expect(matchesMetadataFilter({ status: 'active' }, filter)).toBe(true);
		expect(matchesMetadataFilter({ status: 'closed' }, filter)).toBe(false);
	});

	it('between: matches when value is in range', () => {
		const filter: MetadataFilter = {
			key: 'score',
			value: ['3', '7'],
			mode: 'between',
		};
		expect(matchesMetadataFilter({ score: '5' }, filter)).toBe(true);
		expect(matchesMetadataFilter({ score: '3' }, filter)).toBe(true);
		expect(matchesMetadataFilter({ score: '7' }, filter)).toBe(true);
		expect(matchesMetadataFilter({ score: '1' }, filter)).toBe(false);
		expect(matchesMetadataFilter({ score: '10' }, filter)).toBe(false);
	});

	it('gt: returns false for non-numeric values', () => {
		const filter: MetadataFilter = { key: 'score', value: '5', mode: 'gt' };
		expect(matchesMetadataFilter({ score: 'abc' }, filter)).toBe(false);
	});

	it('between: returns false for non-array value', () => {
		const filter: MetadataFilter = {
			key: 'score',
			value: '5',
			mode: 'between',
		};
		expect(matchesMetadataFilter({ score: '5' }, filter)).toBe(false);
	});

	it('in: returns false for non-array value', () => {
		const filter: MetadataFilter = {
			key: 'status',
			value: 'active',
			mode: 'in',
		};
		expect(matchesMetadataFilter({ status: 'active' }, filter)).toBe(false);
	});

	it('gt: returns false for missing key', () => {
		const filter: MetadataFilter = { key: 'score', value: '5', mode: 'gt' };
		expect(matchesMetadataFilter({}, filter)).toBe(false);
	});
});
