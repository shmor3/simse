// ---------------------------------------------------------------------------
// Diff display utilities — convert VFS diffs to a renderable format and
// produce coloured unified-diff output for the terminal.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface DiffLine {
	readonly type: 'context' | 'add' | 'remove';
	readonly content: string;
	readonly oldLineNumber?: number;
	readonly newLineNumber?: number;
	/** Paired add line for a remove (used for inline highlighting). */
	readonly pair?: DiffLine;
	/** Whether this add line is already paired with a remove. */
	readonly isPaired?: boolean;
}

export interface DiffHunk {
	readonly oldStart: number;
	readonly oldCount: number;
	readonly newStart: number;
	readonly newCount: number;
	readonly lines: readonly DiffLine[];
}

export interface DiffResult {
	readonly oldPath: string;
	readonly newPath: string;
	readonly hunks: readonly DiffHunk[];
	readonly additions: number;
	readonly deletions: number;
}

interface VFSDiffLineInput {
	readonly type: 'add' | 'remove' | 'equal';
	readonly text: string;
	readonly oldLine?: number;
	readonly newLine?: number;
}

interface VFSDiffHunkInput {
	readonly oldStart: number;
	readonly oldCount: number;
	readonly newStart: number;
	readonly newCount: number;
	readonly lines: readonly VFSDiffLineInput[];
}

interface VFSDiffInput {
	readonly oldPath: string;
	readonly newPath: string;
	readonly hunks: readonly VFSDiffHunkInput[];
	readonly additions: number;
	readonly deletions: number;
}

export interface InlineDiffSegment {
	readonly text: string;
	readonly changed: boolean;
}

export interface InlineDiffResult {
	readonly old: readonly InlineDiffSegment[];
	readonly new: readonly InlineDiffSegment[];
}

export interface ColorFunctions {
	readonly bold: (s: string) => string;
	readonly dim: (s: string) => string;
	readonly red: (s: string) => string;
	readonly green: (s: string) => string;
	readonly gray: (s: string) => string;
	readonly [key: string]: unknown;
}

export interface RenderOptions {
	readonly maxLines?: number;
}

// ---------------------------------------------------------------------------
// convertVFSDiff — translate VFS diff types into our renderable format
// ---------------------------------------------------------------------------

export function convertVFSDiff(vfs: VFSDiffInput): DiffResult {
	const hunks: DiffHunk[] = vfs.hunks.map((h) => ({
		oldStart: h.oldStart,
		oldCount: h.oldCount,
		newStart: h.newStart,
		newCount: h.newCount,
		lines: h.lines.map((l): DiffLine => {
			if (l.type === 'equal') {
				return {
					type: 'context',
					content: l.text,
					oldLineNumber: l.oldLine,
					newLineNumber: l.newLine,
				};
			}
			if (l.type === 'remove') {
				return {
					type: 'remove',
					content: l.text,
					oldLineNumber: l.oldLine,
					newLineNumber: undefined,
				};
			}
			return {
				type: 'add',
				content: l.text,
				oldLineNumber: undefined,
				newLineNumber: l.newLine,
			};
		}),
	}));

	return {
		oldPath: vfs.oldPath,
		newPath: vfs.newPath,
		hunks,
		additions: vfs.additions,
		deletions: vfs.deletions,
	};
}

// ---------------------------------------------------------------------------
// computeInlineDiff — find changed segments between two strings
// ---------------------------------------------------------------------------

export function computeInlineDiff(
	oldStr: string,
	newStr: string,
): InlineDiffResult {
	if (oldStr === newStr) {
		return {
			old: oldStr ? [{ text: oldStr, changed: false }] : [],
			new: newStr ? [{ text: newStr, changed: false }] : [],
		};
	}

	if (!oldStr) {
		return {
			old: [],
			new: [{ text: newStr, changed: true }],
		};
	}
	if (!newStr) {
		return {
			old: [{ text: oldStr, changed: true }],
			new: [],
		};
	}

	// Find common prefix
	let prefixLen = 0;
	while (
		prefixLen < oldStr.length &&
		prefixLen < newStr.length &&
		oldStr[prefixLen] === newStr[prefixLen]
	) {
		prefixLen++;
	}

	// Find common suffix (from the end, not overlapping prefix)
	let suffixLen = 0;
	while (
		suffixLen < oldStr.length - prefixLen &&
		suffixLen < newStr.length - prefixLen &&
		oldStr[oldStr.length - 1 - suffixLen] ===
			newStr[newStr.length - 1 - suffixLen]
	) {
		suffixLen++;
	}

	const prefix = oldStr.slice(0, prefixLen);
	const oldMiddle = oldStr.slice(prefixLen, oldStr.length - suffixLen);
	const newMiddle = newStr.slice(prefixLen, newStr.length - suffixLen);
	const suffix = oldStr.slice(oldStr.length - suffixLen);

	const buildSegments = (middle: string): InlineDiffSegment[] => {
		const segs: InlineDiffSegment[] = [];
		if (prefix) segs.push({ text: prefix, changed: false });
		if (middle) segs.push({ text: middle, changed: true });
		if (suffix) segs.push({ text: suffix, changed: false });
		// If nothing changed (shouldn't happen — handled above), return full text
		if (segs.length === 0) segs.push({ text: '', changed: false });
		return segs;
	};

	return {
		old: buildSegments(oldMiddle),
		new: buildSegments(newMiddle),
	};
}

// ---------------------------------------------------------------------------
// pairDiffLines — match contiguous remove/add blocks for inline diffing
// ---------------------------------------------------------------------------

export function pairDiffLines(
	lines: readonly DiffLine[],
): (DiffLine & { pair?: DiffLine; isPaired?: boolean })[] {
	const result: (DiffLine & { pair?: DiffLine; isPaired?: boolean })[] =
		lines.map((l) => ({ ...l }));

	let i = 0;
	while (i < result.length) {
		// Find a contiguous block of removes
		const removeStart = i;
		while (i < result.length && result[i]?.type === 'remove') i++;
		const removeEnd = i;

		// Find a contiguous block of adds immediately following
		const addStart = i;
		while (i < result.length && result[i]?.type === 'add') i++;
		const addEnd = i;

		const removeCount = removeEnd - removeStart;
		const addCount = addEnd - addStart;

		if (removeCount > 0 && addCount > 0) {
			// Pair removes with adds (min of the two counts)
			const pairCount = Math.min(removeCount, addCount);
			for (let j = 0; j < pairCount; j++) {
				const removeEntry = result[removeStart + j];
				const addEntry = result[addStart + j];
				if (removeEntry && addEntry) {
					removeEntry.pair = addEntry;
					addEntry.isPaired = true;
				}
			}
		}

		// If we didn't advance, move forward to avoid infinite loop
		if (i === removeStart) i++;
	}

	return result;
}

// ---------------------------------------------------------------------------
// renderUnifiedDiff — produce terminal-ready unified diff output
// ---------------------------------------------------------------------------

export function renderUnifiedDiff(
	diff: DiffResult,
	colors: ColorFunctions,
	options?: RenderOptions,
): string {
	const maxLines = options?.maxLines ?? 50;
	const lines: string[] = [];
	let lineCount = 0;
	let truncated = false;

	for (const hunk of diff.hunks) {
		// Hunk header
		const header = `@@ -${hunk.oldStart},${hunk.oldCount} +${hunk.newStart},${hunk.newCount} @@`;
		lines.push(colors.dim(header));

		for (const line of hunk.lines) {
			if (lineCount >= maxLines) {
				truncated = true;
				break;
			}

			const oldNum =
				line.oldLineNumber !== undefined
					? String(line.oldLineNumber).padStart(4)
					: '    ';
			const newNum =
				line.newLineNumber !== undefined
					? String(line.newLineNumber).padStart(4)
					: '    ';
			const gutter = `${oldNum} \u2502 ${newNum}`;

			switch (line.type) {
				case 'remove':
					lines.push(
						`${colors.gray(gutter)} ${colors.red(`-${line.content}`)}`,
					);
					break;
				case 'add':
					lines.push(
						`${colors.gray(gutter)} ${colors.green(`+${line.content}`)}`,
					);
					break;
				case 'context':
					lines.push(`${colors.gray(gutter)}  ${line.content}`);
					break;
			}
			lineCount++;
		}

		if (truncated) break;
	}

	if (truncated) {
		const remaining = diff.additions + diff.deletions - lineCount;
		lines.push(
			colors.dim(
				`... ${remaining} more changes (${diff.additions}+ ${diff.deletions}-)`,
			),
		);
	}

	return lines.join('\n');
}
