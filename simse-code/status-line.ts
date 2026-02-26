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
// Mode labels — Claude Code style descriptive text
// ---------------------------------------------------------------------------

const MODE_DESCRIPTIONS: Readonly<Record<PermissionMode, string>> = {
	default: 'permissions on',
	acceptEdits: 'auto-edit on',
	plan: 'plan mode',
	dontAsk: 'bypass permissions on',
};

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createStatusLine(options: StatusLineOptions): StatusLine {
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

		// Determine if bypass/yolo mode — use red background
		const isBypass = currentData.permissionMode === 'dontAsk';

		// Build left segments (permission info)
		const leftParts: string[] = [];

		// Permission mode with lock icon
		const modeDesc =
			MODE_DESCRIPTIONS[currentData.permissionMode] ?? 'permissions on';
		const lockIcon = isBypass ? '🔓' : '🔒';
		leftParts.push(`${lockIcon} ${modeDesc}`);
		leftParts.push('(shift+tab to cycle)');

		// Build right segments (status info)
		const rightParts: string[] = [];

		// Background tasks
		if (currentData.bgTaskCount > 0) {
			rightParts.push(`${currentData.bgTaskCount} bg`);
		}

		// Todos
		if (currentData.todoCount > 0) {
			rightParts.push(`${currentData.todoDone}/${currentData.todoCount} todos`);
		}

		rightParts.push('esc to interrupt');

		const left = leftParts.join(' ');
		const right = rightParts.join(' · ');

		// Save cursor, move to last row, render, restore cursor
		stream.write('\x1b7'); // save cursor
		stream.write(`\x1b[${rows};1H`); // move to last row
		stream.write('\x1b[2K'); // clear line

		// Background color: red for bypass, default reverse for normal
		if (isBypass) {
			stream.write('\x1b[41m\x1b[97m'); // red bg, bright white text
		} else {
			stream.write('\x1b[7m'); // reverse video
		}

		// Layout: left-aligned permission info, right-aligned status
		const totalContent = `${left}${right}`;
		const plainLen = stripAnsi(totalContent).length;
		const gap = Math.max(1, cols - plainLen - 4);
		stream.write(` ${left}${' '.repeat(gap)}${right}  `);

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
