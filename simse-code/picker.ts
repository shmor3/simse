/**
 * SimSE Code — Interactive Picker
 *
 * Numbered-list selection prompt for terminal UIs.
 * No external deps — raw readline only.
 */

import type { Interface as ReadlineInterface } from 'node:readline';
import type { TermColors } from './ui.js';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface PickerItem {
	readonly label: string;
	readonly detail?: string;
}

export interface PickerOptions {
	readonly title?: string;
	readonly colors: TermColors;
}

// ---------------------------------------------------------------------------
// Picker
// ---------------------------------------------------------------------------

/**
 * Show a numbered-list picker and return the selected index.
 * Returns -1 if the user cancels (empty input or invalid selection).
 *
 * @example
 * ```ts
 * const idx = await showPicker(
 *   [{ label: 'Option A', detail: 'does thing A' }, { label: 'Option B' }],
 *   rl,
 *   { title: 'Choose one:', colors },
 * );
 * ```
 */
export async function showPicker(
	items: readonly PickerItem[],
	rl: ReadlineInterface,
	options: PickerOptions,
): Promise<number> {
	const { colors, title } = options;

	if (items.length === 0) return -1;

	// Print title
	if (title) {
		console.log(`\n  ${colors.bold(title)}`);
	}

	// Print numbered list
	for (let i = 0; i < items.length; i++) {
		const num = colors.cyan(`${i + 1}`);
		const label = items[i].label;
		const detail = items[i].detail ? colors.dim(` — ${items[i].detail}`) : '';
		console.log(`  ${num}. ${label}${detail}`);
	}

	// Prompt
	const answer = await new Promise<string>((resolve) => {
		rl.question(`\n  ${colors.dim(`Select [1-${items.length}]:`)} `, resolve);
	});

	const trimmed = answer.trim();
	if (trimmed === '') return -1;

	const num = Number.parseInt(trimmed, 10);
	if (Number.isNaN(num) || num < 1 || num > items.length) return -1;

	return num - 1;
}

/**
 * Show a yes/no confirmation prompt. Returns true for yes.
 */
export async function showConfirm(
	message: string,
	rl: ReadlineInterface,
	colors: TermColors,
): Promise<boolean> {
	const answer = await new Promise<string>((resolve) => {
		rl.question(`  ${message} ${colors.dim('[y/N]')} `, resolve);
	});
	const trimmed = answer.trim().toLowerCase();
	return trimmed === 'y' || trimmed === 'yes';
}
