# Visual Diffs Improvement Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add word-level inline diffs, polish the diff gutter/display, and wire diffs to appear inline on writes and via the /diff command.

**Architecture:** Enhance the existing `diff-display.ts` module with a character-level LCS for word-level highlighting and a polished two-column gutter. Add a `convertVFSDiff()` adapter to bridge VFS types (`'equal'`, `text`, `oldLine`) to display types (`'context'`, `content`, `oldLineNumber`). Wire `renderUnifiedDiff` into two places in `cli.ts`: the `onToolCallEnd` callback (for inline diffs on writes) and the `/diff` command handler (for detailed file diffs).

**Tech Stack:** TypeScript, ANSI escape codes, Bun test runner

---

### Task 1: Add VFS-to-DiffDisplay type converter

**Files:**
- Modify: `simse-code/diff-display.ts:1-52` (types section)

The VFS produces `VFSDiffLine` with `type: 'equal'`, field `text`, and fields `oldLine`/`newLine`. The display module uses `DiffLine` with `type: 'context'`, field `content`, and fields `oldLineNumber`/`newLineNumber`. We need a converter.

**Step 1: Write the failing test**

Create `tests/diff-display.test.ts`:

```typescript
import { describe, expect, test } from 'bun:test';
import { convertVFSDiff } from '../simse-code/diff-display.js';

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
});
```

**Step 2: Run test to verify it fails**

Run: `bun test tests/diff-display.test.ts`
Expected: FAIL — `convertVFSDiff` not exported

**Step 3: Write the implementation**

Add to `simse-code/diff-display.ts` after the existing type definitions (after line 52):

```typescript
// ---------------------------------------------------------------------------
// VFS type adapter
// ---------------------------------------------------------------------------

interface VFSDiffInput {
	readonly oldPath: string;
	readonly newPath: string;
	readonly hunks: readonly {
		readonly oldStart: number;
		readonly oldCount: number;
		readonly newStart: number;
		readonly newCount: number;
		readonly lines: readonly {
			readonly type: 'add' | 'remove' | 'equal';
			readonly text: string;
			readonly oldLine?: number;
			readonly newLine?: number;
		}[];
	}[];
	readonly additions: number;
	readonly deletions: number;
}

export function convertVFSDiff(vfs: VFSDiffInput): DiffResult {
	return {
		oldPath: vfs.oldPath,
		newPath: vfs.newPath,
		additions: vfs.additions,
		deletions: vfs.deletions,
		hunks: vfs.hunks.map((hunk) => ({
			oldStart: hunk.oldStart,
			oldCount: hunk.oldCount,
			newStart: hunk.newStart,
			newCount: hunk.newCount,
			lines: hunk.lines.map((line) => ({
				type: line.type === 'equal' ? ('context' as const) : line.type,
				content: line.text,
				oldLineNumber: line.oldLine,
				newLineNumber: line.newLine,
			})),
		})),
	};
}
```

**Step 4: Run test to verify it passes**

Run: `bun test tests/diff-display.test.ts`
Expected: PASS

**Step 5: Commit**

```bash
git add tests/diff-display.test.ts simse-code/diff-display.ts
git commit -m "feat: add VFS-to-display diff type converter"
```

---

### Task 2: Add word-level character diff

**Files:**
- Modify: `simse-code/diff-display.ts` (add `computeInlineDiff` function)
- Test: `tests/diff-display.test.ts`

**Step 1: Write the failing test**

Add to `tests/diff-display.test.ts`:

```typescript
import { computeInlineDiff } from '../simse-code/diff-display.js';

describe('computeInlineDiff', () => {
	test('identifies changed segments between two lines', () => {
		const result = computeInlineDiff('const name = "hello";', 'const name = "world";');
		// Result is an array of segments: { text, changed }
		expect(result.old.some((s) => s.changed && s.text === '"hello"')).toBe(true);
		expect(result.new.some((s) => s.changed && s.text === '"world"')).toBe(true);
		// Common prefix/suffix are unchanged
		expect(result.old[0].changed).toBe(false);
		expect(result.old[0].text).toBe('const name = ');
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
});
```

**Step 2: Run test to verify it fails**

Run: `bun test tests/diff-display.test.ts`
Expected: FAIL — `computeInlineDiff` not exported

**Step 3: Write the implementation**

Add to `simse-code/diff-display.ts`:

```typescript
// ---------------------------------------------------------------------------
// Word-level inline diff
// ---------------------------------------------------------------------------

export interface InlineSegment {
	readonly text: string;
	readonly changed: boolean;
}

export interface InlineDiffResult {
	readonly old: readonly InlineSegment[];
	readonly new: readonly InlineSegment[];
}

/**
 * Compute character-level diff between two lines.
 * Uses common prefix/suffix trimming + LCS for the middle.
 * Returns segments marked as changed or unchanged.
 */
export function computeInlineDiff(
	oldText: string,
	newText: string,
): InlineDiffResult {
	if (oldText === newText) {
		return {
			old: oldText.length > 0 ? [{ text: oldText, changed: false }] : [],
			new: newText.length > 0 ? [{ text: newText, changed: false }] : [],
		};
	}

	// Find common prefix
	let prefixLen = 0;
	const minLen = Math.min(oldText.length, newText.length);
	while (prefixLen < minLen && oldText[prefixLen] === newText[prefixLen]) {
		prefixLen++;
	}

	// Find common suffix (don't overlap with prefix)
	let suffixLen = 0;
	while (
		suffixLen < minLen - prefixLen &&
		oldText[oldText.length - 1 - suffixLen] ===
			newText[newText.length - 1 - suffixLen]
	) {
		suffixLen++;
	}

	const prefix = oldText.slice(0, prefixLen);
	const oldMiddle = oldText.slice(prefixLen, oldText.length - suffixLen);
	const newMiddle = newText.slice(prefixLen, newText.length - suffixLen);
	const suffix = oldText.slice(oldText.length - suffixLen);

	const oldSegments: InlineSegment[] = [];
	const newSegments: InlineSegment[] = [];

	if (prefix.length > 0) {
		oldSegments.push({ text: prefix, changed: false });
		newSegments.push({ text: prefix, changed: false });
	}
	if (oldMiddle.length > 0) {
		oldSegments.push({ text: oldMiddle, changed: true });
	}
	if (newMiddle.length > 0) {
		newSegments.push({ text: newMiddle, changed: true });
	}
	if (suffix.length > 0) {
		oldSegments.push({ text: suffix, changed: false });
		newSegments.push({ text: suffix, changed: false });
	}

	return { old: oldSegments, new: newSegments };
}
```

**Step 4: Run test to verify it passes**

Run: `bun test tests/diff-display.test.ts`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-code/diff-display.ts tests/diff-display.test.ts
git commit -m "feat: add character-level inline diff for word-level highlighting"
```

---

### Task 3: Add line pairing utility for hunks

**Files:**
- Modify: `simse-code/diff-display.ts`
- Test: `tests/diff-display.test.ts`

This pairs contiguous remove+add blocks within a hunk for word-level diffing.

**Step 1: Write the failing test**

Add to `tests/diff-display.test.ts`:

```typescript
import { pairDiffLines } from '../simse-code/diff-display.js';
import type { DiffLine } from '../simse-code/diff-display.js';

describe('pairDiffLines', () => {
	test('pairs contiguous remove/add blocks', () => {
		const lines: DiffLine[] = [
			{ type: 'context', content: 'before', oldLineNumber: 1, newLineNumber: 1 },
			{ type: 'remove', content: 'old1', oldLineNumber: 2 },
			{ type: 'remove', content: 'old2', oldLineNumber: 3 },
			{ type: 'add', content: 'new1', newLineNumber: 2 },
			{ type: 'add', content: 'new2', newLineNumber: 3 },
			{ type: 'context', content: 'after', oldLineNumber: 4, newLineNumber: 4 },
		];

		const paired = pairDiffLines(lines);
		expect(paired).toHaveLength(6);
		expect(paired[1].pair).toBeDefined();
		expect(paired[1].pair!.content).toBe('new1');
		expect(paired[2].pair).toBeDefined();
		expect(paired[2].pair!.content).toBe('new2');
		// The add lines that are paired should be marked as paired
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
});
```

**Step 2: Run test to verify it fails**

Run: `bun test tests/diff-display.test.ts`
Expected: FAIL — `pairDiffLines` not exported

**Step 3: Write the implementation**

Add to `simse-code/diff-display.ts`:

```typescript
// ---------------------------------------------------------------------------
// Line pairing for word-level diff
// ---------------------------------------------------------------------------

export interface PairedDiffLine {
	readonly type: 'add' | 'remove' | 'context';
	readonly content: string;
	readonly oldLineNumber?: number;
	readonly newLineNumber?: number;
	/** For remove lines: the paired add line (for word-level diff). */
	readonly pair?: DiffLine;
	/** For add lines: true if already consumed as a pair. */
	readonly isPaired?: boolean;
}

/**
 * Walk hunk lines and pair contiguous remove+add blocks for word-level diff.
 * When a block of N removes is immediately followed by M adds,
 * pair them 1:1 up to min(N, M). Excess lines remain unpaired.
 */
export function pairDiffLines(lines: readonly DiffLine[]): PairedDiffLine[] {
	const result: PairedDiffLine[] = [];
	let i = 0;

	while (i < lines.length) {
		// Collect contiguous removes
		const removeStart = i;
		while (i < lines.length && lines[i].type === 'remove') i++;
		const removes = lines.slice(removeStart, i);

		// Collect contiguous adds immediately after
		const addStart = i;
		while (i < lines.length && lines[i].type === 'add') i++;
		const adds = lines.slice(addStart, i);

		if (removes.length > 0 || adds.length > 0) {
			const pairCount = Math.min(removes.length, adds.length);

			// Emit paired removes
			for (let j = 0; j < removes.length; j++) {
				result.push({
					...removes[j],
					pair: j < pairCount ? adds[j] : undefined,
				});
			}

			// Emit adds — mark paired ones
			for (let j = 0; j < adds.length; j++) {
				result.push({
					...adds[j],
					isPaired: j < pairCount ? true : undefined,
				});
			}
		} else {
			// Context line — pass through
			if (i < lines.length) {
				result.push({ ...lines[i] });
				i++;
			}
		}
	}

	return result;
}
```

**Step 4: Run test to verify it passes**

Run: `bun test tests/diff-display.test.ts`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-code/diff-display.ts tests/diff-display.test.ts
git commit -m "feat: add line pairing for word-level diff in hunks"
```

---

### Task 4: Rewrite renderUnifiedDiff with gutter + word-level highlights

**Files:**
- Modify: `simse-code/diff-display.ts:73-145` (replace `renderUnifiedDiff`)
- Test: `tests/diff-display.test.ts`

**Step 1: Write the failing test**

Add to `tests/diff-display.test.ts`:

```typescript
import { renderUnifiedDiff, createColors } from '../simse-code/diff-display.js';

// Note: createColors is from ui.ts, imported indirectly
// For testing we use a no-color version
const noColors = {
	bold: (s: string) => s,
	dim: (s: string) => s,
	italic: (s: string) => s,
	underline: (s: string) => s,
	red: (s: string) => s,
	green: (s: string) => s,
	yellow: (s: string) => s,
	blue: (s: string) => s,
	magenta: (s: string) => s,
	cyan: (s: string) => s,
	gray: (s: string) => s,
	white: (s: string) => s,
	enabled: false,
} as const;

describe('renderUnifiedDiff', () => {
	test('renders gutter with line numbers and separator', () => {
		const diff = {
			oldPath: '/a.txt',
			newPath: '/a.txt',
			hunks: [
				{
					oldStart: 1,
					oldCount: 2,
					newStart: 1,
					newCount: 2,
					lines: [
						{ type: 'remove' as const, content: 'old', oldLineNumber: 1 },
						{ type: 'add' as const, content: 'new', newLineNumber: 1 },
						{ type: 'context' as const, content: 'same', oldLineNumber: 2, newLineNumber: 2 },
					],
				},
			],
			additions: 1,
			deletions: 1,
		};

		const output = renderUnifiedDiff(diff, noColors);
		// Should contain line numbers and │ separator
		expect(output).toContain('│');
		expect(output).toContain('-old');
		expect(output).toContain('+new');
		expect(output).toContain(' same');
	});

	test('truncates with helpful message', () => {
		const manyLines = Array.from({ length: 60 }, (_, i) => ({
			type: 'add' as const,
			content: `line ${i}`,
			newLineNumber: i + 1,
		}));
		const diff = {
			oldPath: '/big.txt',
			newPath: '/big.txt',
			hunks: [{ oldStart: 1, oldCount: 0, newStart: 1, newCount: 60, lines: manyLines }],
			additions: 60,
			deletions: 0,
		};

		const output = renderUnifiedDiff(diff, noColors, { maxLines: 10 });
		expect(output).toContain('more changes');
	});
});
```

**Step 2: Run test to verify current rendering behavior**

Run: `bun test tests/diff-display.test.ts`
Check: Tests may pass partially with current rendering. The key is that the new gutter format uses `│` separator which the current code doesn't have.

**Step 3: Rewrite renderUnifiedDiff**

Replace the `renderUnifiedDiff` function in `simse-code/diff-display.ts` (lines 73-145) with:

```typescript
/**
 * Render a unified diff with ANSI colors, two-column gutter, and word-level highlights.
 */
export function renderUnifiedDiff(
	diff: DiffResult,
	colors: TermColors,
	options?: DiffDisplayOptions,
): string {
	const maxLines = options?.maxLines ?? 50;
	const theme = options?.themeColors;
	const output: string[] = [];

	const addBg = theme ? bg256(theme.add) : DEFAULT_GREEN_BG;
	const removeBg = theme ? bg256(theme.remove) : DEFAULT_RED_BG;
	const BOLD = '\x1b[1m';

	let lineCount = 0;
	let truncated = false;

	for (let h = 0; h < diff.hunks.length; h++) {
		const hunk = diff.hunks[h];

		// Hunk separator
		if (h > 0) output.push('');

		// Hunk header
		const header = `@@ -${hunk.oldStart},${hunk.oldCount} +${hunk.newStart},${hunk.newCount} @@`;
		if (theme) {
			output.push(`    ${colors.dim('     │')} \x1b[38;5;${theme.hunkHeader}m${header}${RESET}`);
		} else {
			output.push(`    ${colors.dim('     │')} ${colors.cyan(header)}`);
		}
		lineCount++;

		// Pair lines for word-level diff
		const paired = pairDiffLines(hunk.lines);

		for (const line of paired) {
			if (lineCount >= maxLines) {
				const remaining = diff.additions + diff.deletions - lineCount;
				output.push(
					colors.dim(`    ${' '.repeat(5)}│ ... ${remaining} more changes`),
				);
				truncated = true;
				break;
			}

			// Skip add lines already consumed as pairs (rendered with their remove)
			if (line.type === 'add' && line.isPaired) {
				// Render the paired add line with word-level highlights
				const gutter = formatGutter(undefined, line.newLineNumber);
				// Find the remove line that paired with us — get inline diff from parent
				// We handle this in the remove case below; this add was already emitted
				// Actually we need to render the paired add here
				const pairedRemove = findPairedRemove(paired, line);
				if (pairedRemove) {
					const inline = computeInlineDiff(pairedRemove.content, line.content);
					if (colors.enabled) {
						const rendered = renderSegments(inline.new, `${addBg}`, `${addBg}${BOLD}`, RESET);
						output.push(`    ${colors.dim(gutter)} ${addBg}+${rendered}${RESET}`);
					} else {
						output.push(`    ${gutter} +${line.content}`);
					}
				} else {
					if (colors.enabled) {
						output.push(`    ${colors.dim(gutter)} ${addBg}+${line.content}${RESET}`);
					} else {
						output.push(`    ${gutter} +${line.content}`);
					}
				}
				lineCount++;
				continue;
			}

			const gutter = formatGutter(
				line.type === 'remove' || line.type === 'context'
					? line.oldLineNumber
					: undefined,
				line.type === 'add' || line.type === 'context'
					? line.newLineNumber
					: undefined,
			);

			switch (line.type) {
				case 'remove': {
					if (line.pair && colors.enabled) {
						// Word-level diff: highlight changed segments
						const inline = computeInlineDiff(line.content, line.pair.content);
						const rendered = renderSegments(inline.old, `${removeBg}`, `${removeBg}${BOLD}`, RESET);
						output.push(`    ${colors.dim(gutter)} ${removeBg}-${rendered}${RESET}`);
					} else if (colors.enabled) {
						output.push(`    ${colors.dim(gutter)} ${removeBg}-${line.content}${RESET}`);
					} else {
						output.push(`    ${gutter} -${line.content}`);
					}
					break;
				}
				case 'add': {
					if (colors.enabled) {
						output.push(`    ${colors.dim(gutter)} ${addBg}+${line.content}${RESET}`);
					} else {
						output.push(`    ${gutter} +${line.content}`);
					}
					break;
				}
				case 'context': {
					if (theme) {
						output.push(
							`    ${colors.dim(gutter)} \x1b[38;5;${theme.context}m ${line.content}${RESET}`,
						);
					} else {
						output.push(`    ${colors.dim(gutter)} ${colors.dim(` ${line.content}`)}`);
					}
					break;
				}
			}
			lineCount++;
		}

		if (truncated) break;
	}

	return output.join('\n');
}

/** Format the gutter: "  42 │" or "     │" */
function formatGutter(oldLine?: number, newLine?: number): string {
	const lineNum = oldLine ?? newLine;
	const numStr = lineNum !== undefined ? String(lineNum).padStart(4) : '    ';
	return `${numStr} │`;
}

/** Render inline segments with normal and highlighted backgrounds. */
function renderSegments(
	segments: readonly InlineSegment[],
	normalBg: string,
	highlightBg: string,
	reset: string,
): string {
	return segments
		.map((seg) => (seg.changed ? `${reset}${highlightBg}${seg.text}${reset}${normalBg}` : seg.text))
		.join('');
}

/** Find the remove line that was paired with a given add line. */
function findPairedRemove(
	paired: readonly PairedDiffLine[],
	addLine: PairedDiffLine,
): PairedDiffLine | undefined {
	for (const line of paired) {
		if (line.type === 'remove' && line.pair === addLine) return line;
		// Match by content since the pair reference points add→remove
		if (line.type === 'remove' && line.pair?.content === addLine.content &&
			line.pair?.newLineNumber === addLine.newLineNumber) return line;
	}
	return undefined;
}
```

**Step 4: Run tests to verify**

Run: `bun test tests/diff-display.test.ts`
Expected: PASS

**Step 5: Run typecheck and lint**

Run: `bun run typecheck && bun run lint`
Expected: PASS

**Step 6: Commit**

```bash
git add simse-code/diff-display.ts tests/diff-display.test.ts
git commit -m "feat: rewrite renderUnifiedDiff with gutter and word-level highlights"
```

---

### Task 5: Wire inline diffs to onToolCallEnd for VFS writes

**Files:**
- Modify: `simse-code/cli.ts:36` (import `convertVFSDiff` and `renderUnifiedDiff`)
- Modify: `simse-code/cli.ts:540-573` (first `onToolCallEnd` handler)
- Modify: `simse-code/cli.ts:770-803` (second `onToolCallEnd` handler — skill loop)

**Step 1: Update imports in cli.ts**

Change line 36 from:
```typescript
import { renderChangeCount } from './diff-display.js';
```
to:
```typescript
import { convertVFSDiff, renderChangeCount, renderUnifiedDiff } from './diff-display.js';
```

**Step 2: Add inline diff helper function**

Add after `deriveToolSummary` (after line 122) in `cli.ts`:

```typescript
/**
 * Try to compute and render an inline diff for a VFS write operation.
 * Returns the rendered diff string, or undefined if no diff is available.
 */
function tryRenderInlineDiff(
	toolName: string,
	argsStr: string,
	vfs: VirtualFS,
	colors: TermColors,
	themeColors?: DiffDisplayOptions['themeColors'],
): string | undefined {
	// Only for write/edit operations
	if (
		!toolName.includes('write') &&
		!toolName.includes('edit') &&
		!toolName.includes('create')
	) {
		return undefined;
	}

	// Extract path from tool args
	let path: string | undefined;
	try {
		const parsed = JSON.parse(argsStr) as Record<string, unknown>;
		path =
			(parsed.path as string) ??
			(parsed.file_path as string) ??
			(parsed.filePath as string);
	} catch {
		return undefined;
	}

	if (!path) return undefined;

	try {
		const history = vfs.history(path);
		if (history.length < 1) return undefined; // New file, no previous version

		// Diff previous version against current
		const vfsDiff = vfs.diffVersions(path, history.length);
		if (vfsDiff.additions === 0 && vfsDiff.deletions === 0) return undefined;

		const displayDiff = convertVFSDiff(vfsDiff);
		return renderUnifiedDiff(displayDiff, colors, {
			maxLines: 50,
			themeColors: themeColors,
		});
	} catch {
		return undefined;
	}
}
```

**Step 3: Wire into onToolCallEnd handlers**

In the first `onToolCallEnd` handler (line 540-573), after `renderToolResultCollapsed` (after line 571), add:

```typescript
// Inline diff for write operations
const inlineDiff = tryRenderInlineDiff(
	toolResult.name,
	argsStr,
	ctx.vfs,
	colors,
	ctx.themeManager?.getActive().diff,
);
if (inlineDiff) {
	console.log(inlineDiff);
}
```

Apply the same change to the second `onToolCallEnd` handler (lines 770-803), after `renderToolResultCollapsed`.

**Step 4: Add VirtualFS import if not present**

Check that `VirtualFS` type is available in scope. It should be via the `AppContext` type.

Also add `DiffDisplayOptions` to the import from `diff-display.js`:

```typescript
import { convertVFSDiff, renderChangeCount, renderUnifiedDiff } from './diff-display.js';
import type { DiffDisplayOptions } from './diff-display.js';
```

**Step 5: Run typecheck**

Run: `bun run typecheck`
Expected: PASS

**Step 6: Commit**

```bash
git add simse-code/cli.ts
git commit -m "feat: show inline diffs on VFS write tool completions"
```

---

### Task 6: Enhance /diff command to show actual unified diffs

**Files:**
- Modify: `simse-code/cli.ts:3169-3199` (diffCommand handler)

**Step 1: Rewrite the /diff command handler**

Replace the handler in the `diffCommand` object (lines 3169-3199) with:

```typescript
const diffCommand: Command = {
	name: 'diff',
	aliases: ['d'],
	usage: '/diff [path]',
	description: 'Show unified diffs of changed files',
	category: 'session',
	handler: (ctx, args) => {
		const { colors, vfs } = ctx;
		if (vfs.fileCount === 0) return colors.dim('No files in sandbox.');

		const themeColors = ctx.themeManager?.getActive().diff;
		const targetPath = args?.trim();

		if (targetPath) {
			// Single file diff
			try {
				const history = vfs.history(targetPath);
				if (history.length < 1) {
					return `  ${colors.green('+')} ${targetPath} ${colors.dim('(new file)')}`;
				}
				const vfsDiff = vfs.diffVersions(targetPath, history.length);
				const displayDiff = convertVFSDiff(vfsDiff);
				const header = `  ${colors.bold(targetPath)}  ${renderChangeCount(vfsDiff.additions, vfsDiff.deletions, colors)}`;
				const diffOutput = renderUnifiedDiff(displayDiff, colors, {
					maxLines: 200,
					themeColors: themeColors,
				});
				return `${header}\n${diffOutput}`;
			} catch {
				return colors.red(`  File not found: ${targetPath}`);
			}
		}

		// All files
		const files = vfs.listAll();
		const sections: string[] = [];
		let totalAdd = 0;
		let totalDel = 0;

		for (const entry of files) {
			if (entry.type !== 'file') continue;

			try {
				const history = vfs.history(entry.path);
				if (history.length < 1) {
					sections.push(`  ${colors.green('+')} ${entry.path} ${colors.dim('(new file)')}`);
					const content = vfs.readFile(entry.path);
					totalAdd += content.text.split('\n').length;
					continue;
				}

				const vfsDiff = vfs.diffVersions(entry.path, history.length);
				if (vfsDiff.additions === 0 && vfsDiff.deletions === 0) continue;

				totalAdd += vfsDiff.additions;
				totalDel += vfsDiff.deletions;

				const displayDiff = convertVFSDiff(vfsDiff);
				const header = `  ${colors.bold(entry.path)}  ${renderChangeCount(vfsDiff.additions, vfsDiff.deletions, colors)}`;
				const diffOutput = renderUnifiedDiff(displayDiff, colors, {
					maxLines: 200,
					themeColors: themeColors,
				});
				sections.push(`${header}\n${diffOutput}`);
			} catch {
				sections.push(`  ${entry.path} ${colors.dim('(unable to diff)')}`);
			}
		}

		if (sections.length === 0) return colors.dim('No file changes to show.');

		const lines: string[] = [colors.bold(colors.cyan('File changes:')), ''];
		lines.push(sections.join('\n\n'));

		if (totalAdd > 0 || totalDel > 0) {
			lines.push('');
			lines.push(`  ${colors.bold('Total:')} ${renderChangeCount(totalAdd, totalDel, colors)}`);
		}

		return lines.join('\n');
	},
};
```

**Step 2: Run typecheck**

Run: `bun run typecheck`
Expected: PASS

**Step 3: Commit**

```bash
git add simse-code/cli.ts
git commit -m "feat: enhance /diff command to show actual unified diffs"
```

---

### Task 7: Wire file tracker to VFS writes

**Files:**
- Modify: `simse-code/cli.ts:3625-3632` (onFileWrite handler)

Currently the `onFileWrite` handler only logs. It should also update the file tracker with diff stats.

**Step 1: Update the onFileWrite handler**

Replace lines 3625-3632 with:

```typescript
onFileWrite: (event: VFSWriteEvent) => {
	const label = event.isNew
		? colors.green('created')
		: colors.yellow('updated');
	console.log(
		`  ${colors.dim('[vfs]')} ${label} ${event.path} ${colors.dim(`(${event.size} bytes)`)}`,
	);
	// Update file tracker with diff stats
	if (fileTracker) {
		try {
			const history = vfs.history(event.path);
			if (history.length >= 1 && !event.isNew) {
				const vfsDiff = vfs.diffVersions(event.path, history.length);
				fileTracker.track(
					event.path,
					vfsDiff.additions,
					vfsDiff.deletions,
					false,
				);
			} else {
				// New file — count all lines as additions
				const content = vfs.readFile(event.path);
				const lineCount = content.text.split('\n').length;
				fileTracker.track(event.path, lineCount, 0, true);
			}
		} catch {
			// Silently skip tracking on error
		}
	}
},
```

**Note:** The `vfs` variable is created on line 3623 and the `onFileWrite` callback is part of its options, so `vfs` is not yet available inside the callback at construction time. This is a circular reference problem.

**Fix:** Move the file tracker wiring outside the VFS constructor. Instead, register a separate listener after VFS creation. Check if VFS supports event subscriptions — if not, the tracking must happen differently.

**Alternative approach:** Since we already compute diffs in `tryRenderInlineDiff` during `onToolCallEnd`, we can track there instead:

After the inline diff rendering in each `onToolCallEnd` handler, add:

```typescript
// Track file changes
if (ctx.fileTracker && inlineDiff) {
	try {
		const parsed = JSON.parse(argsStr) as Record<string, unknown>;
		const filePath = (parsed.path as string) ?? (parsed.file_path as string);
		if (filePath) {
			const history = ctx.vfs.history(filePath);
			if (history.length >= 1) {
				const vfsDiff = ctx.vfs.diffVersions(filePath, history.length);
				ctx.fileTracker.track(filePath, vfsDiff.additions, vfsDiff.deletions, false);
			}
		}
	} catch {
		// Skip
	}
}
```

**Step 2: Run typecheck**

Run: `bun run typecheck`
Expected: PASS

**Step 3: Commit**

```bash
git add simse-code/cli.ts
git commit -m "feat: wire file tracker to VFS write stats"
```

---

### Task 8: Add themeManager to AppContext (if not present)

**Files:**
- Check: `simse-code/app-context.ts` for `themeManager` field
- Modify if needed

**Step 1: Check if themeManager exists on AppContext**

Check `simse-code/app-context.ts` for `themeManager`. The code in Task 5 references `ctx.themeManager?.getActive().diff` — this must exist on `AppContext`.

If not present, add:
```typescript
readonly themeManager?: ThemeManager;
```

And wire it during AppContext construction in cli.ts.

**Step 2: Run typecheck**

Run: `bun run typecheck`
Expected: PASS

**Step 3: Commit (if changes needed)**

```bash
git add simse-code/app-context.ts simse-code/cli.ts
git commit -m "feat: expose themeManager on AppContext for diff theming"
```

---

### Task 9: Final integration test and cleanup

**Files:**
- Test: `tests/diff-display.test.ts`
- Check: `simse-code/diff-display.ts`

**Step 1: Run full test suite**

Run: `bun test`
Expected: All tests PASS

**Step 2: Run typecheck**

Run: `bun run typecheck`
Expected: PASS

**Step 3: Run lint**

Run: `bun run lint`
Expected: PASS (or fix any issues)

**Step 4: Remove old formatLineNumber function**

The old `formatLineNumber` function (lines 198-207 of original diff-display.ts) is replaced by `formatGutter`. Remove it if unused.

**Step 5: Final commit**

```bash
git add -A
git commit -m "chore: cleanup unused helpers in diff-display"
```
