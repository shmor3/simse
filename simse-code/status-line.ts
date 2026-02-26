/**
 * SimSE Code — Status Line
 *
 * Bottom-of-screen status bar showing model, context usage,
 * file changes, permission mode, and other indicators.
 * No external deps — raw ANSI escape codes only.
 */

import type {
	PermissionMode,
	StatusLine,
	StatusLineData,
} from './app-context.js';
import type { TermColors } from './ui.js';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type { StatusLine, StatusLineData };

export interface StatusLineOptions {
	readonly colors: TermColors;
	readonly stream?: NodeJS.WriteStream;
	readonly enabled?: boolean;
}

// ---------------------------------------------------------------------------
// Mode labels (compact for status line)
// ---------------------------------------------------------------------------

const MODE_SHORT: Readonly<Record<PermissionMode, string>> = {
	default: '',
	acceptEdits: 'AUTO-EDIT',
	plan: 'PLAN',
	dontAsk: 'YOLO',
};

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createStatusLine(options: StatusLineOptions): StatusLine {
	const { colors } = options;
	const stream = options.stream ?? process.stderr;
	const enabled = (options.enabled ?? true) && stream.isTTY === true;

	let currentData: StatusLineData = {
		model: '',
		contextPercent: 0,
		costEstimate: '',
		additions: 0,
		deletions: 0,
		permissionMode: 'default',
		planMode: false,
		bgTaskCount: 0,
		todoCount: 0,
		todoDone: 0,
	};

	const render = (): void => {
		if (!enabled) return;

		const rows = stream.rows ?? 24;
		const cols = stream.columns ?? 80;

		// Build segments
		const segments: string[] = [];

		// Model
		if (currentData.model) {
			segments.push(currentData.model);
		}

		// Context usage
		if (currentData.contextPercent > 0) {
			const pct = Math.round(currentData.contextPercent * 100);
			const contextColor =
				pct > 90 ? colors.red : pct > 70 ? colors.yellow : colors.green;
			segments.push(contextColor(`${pct}%`));
		}

		// File changes
		if (currentData.additions > 0 || currentData.deletions > 0) {
			const parts: string[] = [];
			if (currentData.additions > 0)
				parts.push(colors.green(`+${currentData.additions}`));
			if (currentData.deletions > 0)
				parts.push(colors.red(`-${currentData.deletions}`));
			segments.push(parts.join(' '));
		}

		// Permission mode
		const modeLabel = MODE_SHORT[currentData.permissionMode];
		if (modeLabel) {
			const modeColor =
				currentData.permissionMode === 'dontAsk'
					? colors.red
					: currentData.permissionMode === 'plan'
						? colors.yellow
						: colors.green;
			segments.push(modeColor(`[${modeLabel}]`));
		}

		// Plan mode indicator
		if (currentData.planMode) {
			segments.push(colors.yellow('[PLAN]'));
		}

		// Background tasks
		if (currentData.bgTaskCount > 0) {
			segments.push(colors.cyan(`(${currentData.bgTaskCount} bg)`));
		}

		// Todos
		if (currentData.todoCount > 0) {
			segments.push(
				colors.dim(`${currentData.todoDone}/${currentData.todoCount} todos`),
			);
		}

		// Cost estimate
		if (currentData.costEstimate) {
			segments.push(colors.dim(currentData.costEstimate));
		}

		const content = segments.join(colors.dim(' │ '));

		// Save cursor position, move to last row, render, restore cursor
		stream.write('\x1b7'); // save cursor
		stream.write(`\x1b[${rows};1H`); // move to last row
		stream.write('\x1b[2K'); // clear line
		stream.write(`\x1b[7m`); // reverse video (inverted bar)

		// Pad to full width
		const plainLen = stripAnsi(content).length;
		const padding = Math.max(0, cols - plainLen - 2);
		stream.write(` ${content}${' '.repeat(padding)} `);

		stream.write('\x1b[0m'); // reset
		stream.write('\x1b8'); // restore cursor
	};

	const update = (data: Partial<StatusLineData>): void => {
		currentData = { ...currentData, ...data };
		render();
	};

	return Object.freeze({ render, update });
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

// biome-ignore lint/suspicious/noControlCharactersInRegex: ANSI escape stripping requires matching ESC character
const ANSI_RE = /\x1b\[[0-9;]*m/g;

function stripAnsi(str: string): string {
	return str.replace(ANSI_RE, '');
}
