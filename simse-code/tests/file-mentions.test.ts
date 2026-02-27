import { describe, expect, test } from 'bun:test';
import {
	completeAtMention,
	formatMentionsAsContext,
	resolveFileMentions,
} from '../file-mentions.js';
import type { FileMention } from '../file-mentions.js';

describe('resolveFileMentions', () => {
	test('resolves VFS mentions when resolver is provided', () => {
		const result = resolveFileMentions('check @vfs://test.py please', {
			resolveVFS: (path) => {
				if (path === 'test.py') {
					return { content: 'print("hello")', size: 15 };
				}
				return undefined;
			},
		});

		expect(result.mentions).toHaveLength(1);
		expect(result.mentions[0]!.kind).toBe('vfs');
		expect(result.mentions[0]!.path).toBe('vfs://test.py');
		expect(result.mentions[0]!.content).toBe('print("hello")');
		expect(result.cleanInput).toBe('check  please');
	});

	test('resolves note ID mentions when resolver is provided', () => {
		const result = resolveFileMentions('explain @a1b2c3d4', {
			resolveNote: (idPrefix) => {
				if (idPrefix === 'a1b2c3d4') {
					return { id: 'a1b2c3d4-full', text: 'note content', topic: 'dev' };
				}
				return undefined;
			},
		});

		expect(result.mentions).toHaveLength(1);
		expect(result.mentions[0]!.kind).toBe('note');
		expect(result.mentions[0]!.path).toBe('a1b2c3d4');
		expect(result.mentions[0]!.content).toBe('note content');
		expect(result.mentions[0]!.topic).toBe('dev');
		expect(result.cleanInput).toBe('explain');
	});

	test('ignores VFS mentions when no resolver provided', () => {
		const result = resolveFileMentions('check @vfs://test.py');
		expect(result.mentions).toHaveLength(0);
	});

	test('ignores note IDs when no resolver provided', () => {
		const result = resolveFileMentions('explain @a1b2c3d4');
		expect(result.mentions).toHaveLength(0);
	});

	test('does not treat paths with / or . as note IDs', () => {
		const result = resolveFileMentions('look at @src/main.ts', {
			resolveNote: () => ({
				id: 'fake',
				text: 'fake',
				topic: 'fake',
			}),
		});
		// src/main.ts contains / and .ts, so it should NOT match as a note
		// It may match as a file (filesystem), but not as a note
		for (const m of result.mentions) {
			expect(m.kind).not.toBe('note');
		}
	});

	test('deduplicates VFS mentions', () => {
		const result = resolveFileMentions(
			'@vfs://a.py and @vfs://a.py again',
			{
				resolveVFS: (path) => {
					if (path === 'a.py') {
						return { content: 'code', size: 4 };
					}
					return undefined;
				},
			},
		);
		expect(result.mentions).toHaveLength(1);
	});

	test('handles multiple mention types in one input', () => {
		const result = resolveFileMentions(
			'compare @vfs://draft.py with note @abcd1234',
			{
				resolveVFS: (path) => {
					if (path === 'draft.py') {
						return { content: 'vfs content', size: 11 };
					}
					return undefined;
				},
				resolveNote: (id) => {
					if (id === 'abcd1234') {
						return { id: 'abcd1234-full', text: 'note text', topic: 'misc' };
					}
					return undefined;
				},
			},
		);

		expect(result.mentions).toHaveLength(2);
		const kinds = result.mentions.map((m) => m.kind);
		expect(kinds).toContain('vfs');
		expect(kinds).toContain('note');
	});
});

describe('formatMentionsAsContext', () => {
	test('formats file mentions as XML tags', () => {
		const mentions: FileMention[] = [
			{ path: 'src/main.ts', content: 'code here', size: 9, kind: 'file' },
		];
		const ctx = formatMentionsAsContext(mentions);
		expect(ctx).toContain('<file path="src/main.ts">');
		expect(ctx).toContain('code here');
		expect(ctx).toContain('</file>');
	});

	test('formats note mentions with topic', () => {
		const mentions: FileMention[] = [
			{
				path: 'a1b2c3d4',
				content: 'note body',
				size: 9,
				kind: 'note',
				topic: 'design',
			},
		];
		const ctx = formatMentionsAsContext(mentions);
		expect(ctx).toContain('<note id="a1b2c3d4" topic="design">');
		expect(ctx).toContain('note body');
		expect(ctx).toContain('</note>');
	});

	test('formats VFS mentions as file tags', () => {
		const mentions: FileMention[] = [
			{ path: 'vfs://test.py', content: 'vfs code', size: 8, kind: 'vfs' },
		];
		const ctx = formatMentionsAsContext(mentions);
		expect(ctx).toContain('<file path="vfs://test.py">');
	});

	test('returns empty string for no mentions', () => {
		expect(formatMentionsAsContext([])).toBe('');
	});
});

describe('completeAtMention', () => {
	test('dispatches vfs:// prefix to completeVFS callback', () => {
		const result = completeAtMention('vfs://te', {
			completeVFS: (partial) => {
				if (partial === 'te') return ['test.py', 'temp.txt'];
				return [];
			},
		});
		expect(result).toEqual(['test.py', 'temp.txt']);
	});

	test('dispatches hex prefix to completeNote callback', () => {
		const result = completeAtMention('a1b2', {
			completeNote: (partial) => {
				if (partial === 'a1b2') return ['a1b2c3d4'];
				return [];
			},
		});
		expect(result).toContain('a1b2c3d4');
	});

	test('falls back to filesystem for non-hex, non-vfs partial', () => {
		// Just verify it returns an array (filesystem results depend on cwd)
		const result = completeAtMention('nonexistent_path_xyz');
		expect(Array.isArray(result)).toBe(true);
	});

	test('returns empty for vfs:// with no callback', () => {
		const result = completeAtMention('vfs://test');
		expect(result).toEqual([]);
	});
});
