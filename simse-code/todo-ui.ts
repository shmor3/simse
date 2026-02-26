/**
 * SimSE Code — Todo UI
 *
 * Renders task list items for the /todos command.
 * Uses the existing TaskList from the library.
 * No external deps.
 */

import type { TermColors } from './ui.js';

// ---------------------------------------------------------------------------
// Types (matching the library's task types)
// ---------------------------------------------------------------------------

export interface TodoItem {
	readonly id: string;
	readonly subject: string;
	readonly status: 'pending' | 'in_progress' | 'completed';
	readonly description?: string;
	readonly blockedBy?: readonly string[];
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/**
 * Render a list of todos for terminal display.
 */
export function renderTodoList(
	items: readonly TodoItem[],
	colors: TermColors,
): string {
	if (items.length === 0) {
		return `  ${colors.dim('No tasks.')}`;
	}

	const lines: string[] = [];

	for (const item of items) {
		const icon = getStatusIcon(item.status, colors);
		const idStr = colors.dim(`#${item.id}`);
		const subject =
			item.status === 'completed' ? colors.dim(item.subject) : item.subject;
		const blocked =
			item.blockedBy && item.blockedBy.length > 0
				? colors.dim(` (blocked by: ${item.blockedBy.join(', ')})`)
				: '';

		lines.push(`  ${icon} ${idStr} ${subject}${blocked}`);
	}

	// Summary line
	const done = items.filter((i) => i.status === 'completed').length;
	const inProgress = items.filter((i) => i.status === 'in_progress').length;
	const pending = items.filter((i) => i.status === 'pending').length;

	lines.push('');
	const parts: string[] = [];
	if (done > 0) parts.push(colors.green(`${done} done`));
	if (inProgress > 0) parts.push(colors.yellow(`${inProgress} active`));
	if (pending > 0) parts.push(colors.dim(`${pending} pending`));
	lines.push(`  ${parts.join(' · ')}`);

	return lines.join('\n');
}

/**
 * Render a compact todo summary for status line: "3/5 todos"
 */
export function renderTodoSummary(
	total: number,
	done: number,
	colors: TermColors,
): string {
	if (total === 0) return '';
	return colors.dim(`${done}/${total} todos`);
}

/**
 * Parse a /todo subcommand.
 * Returns { action, args } or undefined if invalid.
 */
export function parseTodoCommand(
	input: string,
): { action: string; args: string } | undefined {
	const parts = input.trim().split(/\s+/);
	const action = parts[0]?.toLowerCase();

	if (!action) return undefined;

	const validActions = ['add', 'done', 'rm', 'list'];
	if (!validActions.includes(action)) return undefined;

	return { action, args: parts.slice(1).join(' ') };
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function getStatusIcon(
	status: 'pending' | 'in_progress' | 'completed',
	colors: TermColors,
): string {
	switch (status) {
		case 'pending':
			return colors.dim('[ ]');
		case 'in_progress':
			return colors.yellow('[>]');
		case 'completed':
			return colors.green('[x]');
	}
}
