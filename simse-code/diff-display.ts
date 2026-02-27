/**
 * SimSE Code — Diff Display
 *
 * Renders unified diffs with colored output for terminal display.
 * Uses VFS diff types. Works standalone (no VFS dependency needed).
 * No external deps.
 */

import type { TermColors } from './ui.js';

// ---------------------------------------------------------------------------
// Types (matching VFS diff output)
// ---------------------------------------------------------------------------

export interface DiffLine {
	readonly type: 'add' | 'remove' | 'context';
	readonly content: string;
	readonly oldLineNumber?: number;
	readonly newLineNumber?: number;
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

export interface DiffDisplayOptions {
	/** Maximum lines to show per file. Default: 50 */
	readonly maxLines?: number;
	/** Show line numbers. Default: true */
	readonly lineNumbers?: boolean;
	/** Context lines around changes. Default: 3 */
	readonly contextLines?: number;
	/** Theme diff colors (256-color codes). Overrides default red/green backgrounds. */
	readonly themeColors?: {
		readonly add: number;
		readonly remove: number;
		readonly context: number;
		readonly hunkHeader: number;
	};
}

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
 * Uses common prefix/suffix trimming for the changed region.
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
 * When N removes are immediately followed by M adds, pair 1:1 up to min(N, M).
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

			for (let j = 0; j < removes.length; j++) {
				result.push({
					...removes[j],
					pair: j < pairCount ? adds[j] : undefined,
				});
			}

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

// ---------------------------------------------------------------------------
// Render
// ---------------------------------------------------------------------------

// ANSI background colors — defaults (standard 16-color)
const DEFAULT_RED_BG = '\x1b[41m';
const DEFAULT_GREEN_BG = '\x1b[42m';
const BOLD = '\x1b[1m';
const RESET = '\x1b[0m';

/** Build ANSI 256-color background escape sequence. */
function bg256(code: number): string {
	return `\x1b[48;5;${code}m`;
}

/** Format gutter with line number and separator. */
function formatGutter(oldLine?: number, newLine?: number): string {
	const lineNum = oldLine ?? newLine;
	const numStr = lineNum !== undefined ? String(lineNum).padStart(4) : '    ';
	return `${numStr} │`;
}

/** Render inline segments with highlight for changed parts. */
function renderSegments(
	segments: readonly InlineSegment[],
	normalBg: string,
	highlightBg: string,
	reset: string,
): string {
	return segments
		.map((seg) =>
			seg.changed
				? `${reset}${highlightBg}${seg.text}${reset}${normalBg}`
				: seg.text,
		)
		.join('');
}

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
	const lines: string[] = [];

	// Use theme 256-color backgrounds if provided, else standard 16-color
	const addBg = theme ? bg256(theme.add) : DEFAULT_GREEN_BG;
	const removeBg = theme ? bg256(theme.remove) : DEFAULT_RED_BG;
	const addHighlight = theme
		? `${bg256(theme.add)}${BOLD}`
		: `${DEFAULT_GREEN_BG}${BOLD}`;
	const removeHighlight = theme
		? `${bg256(theme.remove)}${BOLD}`
		: `${DEFAULT_RED_BG}${BOLD}`;

	let lineCount = 0;
	let truncated = false;

	for (let hunkIdx = 0; hunkIdx < diff.hunks.length; hunkIdx++) {
		const hunk = diff.hunks[hunkIdx];

		// Blank line between hunks (except before the first)
		if (hunkIdx > 0) {
			lines.push('');
		}

		// Always show hunk header
		const header = `@@ -${hunk.oldStart},${hunk.oldCount} +${hunk.newStart},${hunk.newCount} @@`;
		if (theme) {
			lines.push(`     │ \x1b[38;5;${theme.hunkHeader}m${header}${RESET}`);
		} else {
			lines.push(`     │ ${colors.cyan(header)}`);
		}
		lineCount++;

		// Pre-compute inline diffs for paired lines
		const paired = pairDiffLines([...hunk.lines]);
		const inlineDiffs = new Map<number, InlineDiffResult>();
		for (let idx = 0; idx < paired.length; idx++) {
			const line = paired[idx];
			if (line.type === 'remove' && line.pair) {
				// Find the paired add's index
				for (let addIdx = idx + 1; addIdx < paired.length; addIdx++) {
					if (
						paired[addIdx].type === 'add' &&
						paired[addIdx].isPaired &&
						paired[addIdx].content === line.pair.content
					) {
						const diffResult = computeInlineDiff(
							line.content,
							line.pair.content,
						);
						inlineDiffs.set(idx, diffResult);
						inlineDiffs.set(addIdx, diffResult);
						break;
					}
				}
			}
		}

		for (let idx = 0; idx < paired.length; idx++) {
			if (lineCount >= maxLines) {
				const remaining = diff.additions + diff.deletions - lineCount;
				if (remaining > 0) {
					lines.push(colors.dim(`     │ ... ${remaining} more changes`));
				}
				truncated = true;
				break;
			}

			const line = paired[idx];
			const gutter = formatGutter(
				line.type === 'remove' ? line.oldLineNumber : undefined,
				line.type !== 'remove' ? line.newLineNumber : undefined,
			);

			switch (line.type) {
				case 'remove': {
					const inlineDiff = inlineDiffs.get(idx);
					if (colors.enabled && inlineDiff) {
						const rendered = renderSegments(
							inlineDiff.old,
							removeBg,
							removeHighlight,
							RESET,
						);
						lines.push(`${gutter} ${removeBg}-${rendered}${RESET}`);
					} else if (colors.enabled) {
						lines.push(`${gutter} ${removeBg}-${line.content}${RESET}`);
					} else {
						lines.push(`${gutter} -${line.content}`);
					}
					break;
				}
				case 'add': {
					const inlineDiff = inlineDiffs.get(idx);
					if (colors.enabled && inlineDiff) {
						const rendered = renderSegments(
							inlineDiff.new,
							addBg,
							addHighlight,
							RESET,
						);
						lines.push(`${gutter} ${addBg}+${rendered}${RESET}`);
					} else if (colors.enabled) {
						lines.push(`${gutter} ${addBg}+${line.content}${RESET}`);
					} else {
						lines.push(`${gutter} +${line.content}`);
					}
					break;
				}
				case 'context':
					if (theme) {
						lines.push(
							`${gutter} \x1b[38;5;${theme.context}m ${line.content}${RESET}`,
						);
					} else {
						lines.push(`${gutter} ${colors.dim(` ${line.content}`)}`);
					}
					break;
			}
			lineCount++;
		}

		if (truncated) break;
	}

	return lines.join('\n');
}

/**
 * Render a compact diff summary (e.g. for status line or /files).
 */
export function renderDiffSummary(
	diffs: readonly DiffResult[],
	colors: TermColors,
): string {
	const lines: string[] = [];
	let totalAdd = 0;
	let totalDel = 0;

	for (const diff of diffs) {
		totalAdd += diff.additions;
		totalDel += diff.deletions;

		const addStr = diff.additions > 0 ? colors.green(`+${diff.additions}`) : '';
		const delStr = diff.deletions > 0 ? colors.red(`-${diff.deletions}`) : '';
		const sep = addStr && delStr ? ' ' : '';
		const path = diff.newPath || diff.oldPath;

		lines.push(`  ${path}  ${addStr}${sep}${delStr}`);
	}

	if (diffs.length > 1) {
		lines.push('');
		lines.push(
			`  ${colors.bold('Total:')} ${colors.green(`+${totalAdd}`)} ${colors.red(`-${totalDel}`)} in ${diffs.length} files`,
		);
	}

	return lines.join('\n');
}

/**
 * Render a compact single-line change summary: "+42 -17"
 */
export function renderChangeCount(
	additions: number,
	deletions: number,
	colors: TermColors,
): string {
	const parts: string[] = [];
	if (additions > 0) parts.push(colors.green(`+${additions}`));
	if (deletions > 0) parts.push(colors.red(`-${deletions}`));
	return parts.join(' ');
}
