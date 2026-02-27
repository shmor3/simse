import { describe, expect, test } from 'bun:test';
import type { DiffLine } from '../simse-code/diff-display.js';
import {
	computeInlineDiff,
	convertVFSDiff,
	pairDiffLines,
} from '../simse-code/diff-display.js';

describe('convertVFSDiff', () => {
	test('converts VFS equal lines to context lines', () => {
		const vfsDiff = {
			oldPath: '/a.txt',
			newPath: '/a.txt',
			hunks: [
				{
					oldStart: 1,
					oldCount: 3,
					newStart: 1,
					newCount: 3,
					lines: [
						{ type: 'equal' as const, text: 'hello', oldLine: 1, newLine: 1 },
						{ type: 'remove' as const, text: 'old', oldLine: 2 },
						{ type: 'add' as const, text: 'new', newLine: 2 },
						{ type: 'equal' as const, text: 'world', oldLine: 3, newLine: 3 },
					],
				},
			],
			additions: 1,
			deletions: 1,
		};

		const result = convertVFSDiff(vfsDiff);

		expect(result.oldPath).toBe('/a.txt');
		expect(result.hunks[0].lines[0]).toEqual({
			type: 'context',
			content: 'hello',
			oldLineNumber: 1,
			newLineNumber: 1,
		});
		expect(result.hunks[0].lines[1]).toEqual({
			type: 'remove',
			content: 'old',
			oldLineNumber: 2,
			newLineNumber: undefined,
		});
		expect(result.hunks[0].lines[2]).toEqual({
			type: 'add',
			content: 'new',
			oldLineNumber: undefined,
			newLineNumber: 2,
		});
	});

	test('preserves additions and deletions counts', () => {
		const vfsDiff = {
			oldPath: '/x.ts',
			newPath: '/x.ts',
			hunks: [],
			additions: 5,
			deletions: 3,
		};
		const result = convertVFSDiff(vfsDiff);
		expect(result.additions).toBe(5);
		expect(result.deletions).toBe(3);
	});
});

describe('computeInlineDiff', () => {
	test('identifies changed segments between two lines', () => {
		const result = computeInlineDiff(
			'const name = "hello";',
			'const name = "world";',
		);
		expect(result.old[0]).toEqual({ text: 'const name = "', changed: false });
		expect(result.old[1]).toEqual({ text: 'hello', changed: true });
		expect(result.old[2]).toEqual({ text: '";', changed: false });
		expect(result.new[1]).toEqual({ text: 'world', changed: true });
	});

	test('handles completely different lines', () => {
		const result = computeInlineDiff('aaa', 'bbb');
		expect(result.old).toEqual([{ text: 'aaa', changed: true }]);
		expect(result.new).toEqual([{ text: 'bbb', changed: true }]);
	});

	test('handles identical lines', () => {
		const result = computeInlineDiff('same', 'same');
		expect(result.old).toEqual([{ text: 'same', changed: false }]);
		expect(result.new).toEqual([{ text: 'same', changed: false }]);
	});

	test('handles empty to non-empty', () => {
		const result = computeInlineDiff('', 'added');
		expect(result.new).toEqual([{ text: 'added', changed: true }]);
		expect(result.old).toEqual([]);
	});

	test('handles non-empty to empty', () => {
		const result = computeInlineDiff('removed', '');
		expect(result.old).toEqual([{ text: 'removed', changed: true }]);
		expect(result.new).toEqual([]);
	});
});

describe('pairDiffLines', () => {
	test('pairs contiguous remove/add blocks', () => {
		const lines: DiffLine[] = [
			{
				type: 'context',
				content: 'before',
				oldLineNumber: 1,
				newLineNumber: 1,
			},
			{ type: 'remove', content: 'old1', oldLineNumber: 2 },
			{ type: 'remove', content: 'old2', oldLineNumber: 3 },
			{ type: 'add', content: 'new1', newLineNumber: 2 },
			{ type: 'add', content: 'new2', newLineNumber: 3 },
			{
				type: 'context',
				content: 'after',
				oldLineNumber: 4,
				newLineNumber: 4,
			},
		];

		const paired = pairDiffLines(lines);
		expect(paired).toHaveLength(6);
		expect(paired[1].pair?.content).toBe('new1');
		expect(paired[2].pair?.content).toBe('new2');
		expect(paired[3].isPaired).toBe(true);
		expect(paired[4].isPaired).toBe(true);
	});

	test('unpaired removes have no pair', () => {
		const lines: DiffLine[] = [
			{ type: 'remove', content: 'deleted', oldLineNumber: 1 },
			{ type: 'context', content: 'gap', oldLineNumber: 2, newLineNumber: 1 },
			{ type: 'add', content: 'added', newLineNumber: 2 },
		];

		const paired = pairDiffLines(lines);
		expect(paired[0].pair).toBeUndefined();
		expect(paired[2].isPaired).toBeUndefined();
	});

	test('handles more removes than adds', () => {
		const lines: DiffLine[] = [
			{ type: 'remove', content: 'a', oldLineNumber: 1 },
			{ type: 'remove', content: 'b', oldLineNumber: 2 },
			{ type: 'remove', content: 'c', oldLineNumber: 3 },
			{ type: 'add', content: 'x', newLineNumber: 1 },
		];

		const paired = pairDiffLines(lines);
		expect(paired[0].pair?.content).toBe('x');
		expect(paired[1].pair).toBeUndefined();
		expect(paired[2].pair).toBeUndefined();
		expect(paired[3].isPaired).toBe(true);
	});

	test('handles more adds than removes', () => {
		const lines: DiffLine[] = [
			{ type: 'remove', content: 'a', oldLineNumber: 1 },
			{ type: 'add', content: 'x', newLineNumber: 1 },
			{ type: 'add', content: 'y', newLineNumber: 2 },
			{ type: 'add', content: 'z', newLineNumber: 3 },
		];

		const paired = pairDiffLines(lines);
		expect(paired[0].pair?.content).toBe('x');
		expect(paired[1].isPaired).toBe(true);
		expect(paired[2].isPaired).toBeUndefined();
		expect(paired[3].isPaired).toBeUndefined();
	});
});
