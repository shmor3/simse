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
	/** Maximum lines to show per file. Default: 100 */
	readonly maxLines?: number;
	/** Show line numbers. Default: true */
	readonly lineNumbers?: boolean;
	/** Context lines around changes. Default: 3 */
	readonly contextLines?: number;
}

// ---------------------------------------------------------------------------
// Render
// ---------------------------------------------------------------------------

// ANSI background colors for diff highlighting
const RED_BG = '\x1b[41m';
const GREEN_BG = '\x1b[42m';
const RESET = '\x1b[0m';

/**
 * Render a unified diff with ANSI colors — Claude Code style.
 * Uses background colors for full-line highlighting.
 */
export function renderUnifiedDiff(
	diff: DiffResult,
	colors: TermColors,
	options?: DiffDisplayOptions,
): string {
	const maxLines = options?.maxLines ?? 100;
	const showLineNumbers = options?.lineNumbers ?? true;
	const lines: string[] = [];

	let lineCount = 0;

	for (const hunk of diff.hunks) {
		// Hunk header (only if multiple hunks or non-contiguous)
		if (diff.hunks.length > 1) {
			const header = `@@ -${hunk.oldStart},${hunk.oldCount} +${hunk.newStart},${hunk.newCount} @@`;
			lines.push(`    ${colors.cyan(header)}`);
			lineCount++;
		}

		for (const line of hunk.lines) {
			if (lineCount >= maxLines) {
				lines.push(
					colors.dim(
						`    ... (${diff.additions + diff.deletions - lineCount} more changes)`,
					),
				);
				break;
			}

			const lineNum = showLineNumbers ? formatLineNumber(line) : '';

			switch (line.type) {
				case 'add':
					if (colors.enabled) {
						lines.push(`    ${lineNum}${GREEN_BG}+${line.content}${RESET}`);
					} else {
						lines.push(`    ${lineNum}+${line.content}`);
					}
					break;
				case 'remove':
					if (colors.enabled) {
						lines.push(`    ${lineNum}${RED_BG}-${line.content}${RESET}`);
					} else {
						lines.push(`    ${lineNum}-${line.content}`);
					}
					break;
				case 'context':
					lines.push(`    ${lineNum}${colors.dim(` ${line.content}`)}`);
					break;
			}
			lineCount++;
		}

		if (lineCount >= maxLines) break;
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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatLineNumber(line: DiffLine): string {
	switch (line.type) {
		case 'add':
			return `${String(line.newLineNumber ?? '').padStart(4)} `;
		case 'remove':
			return `${String(line.oldLineNumber ?? '').padStart(4)} `;
		case 'context':
			return `${String(line.newLineNumber ?? '').padStart(4)} `;
	}
}
