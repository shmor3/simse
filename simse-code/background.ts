/**
 * SimSE Code — Background Tasks
 *
 * Manages background AI generations that continue running
 * while the user interacts with the REPL.
 * No external deps.
 */

import { randomUUID } from 'node:crypto';
import type { BackgroundManager, BackgroundTask } from './app-context.js';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type { BackgroundManager, BackgroundTask };

export interface BackgroundManagerOptions {
	/** Called when a background task completes. */
	readonly onComplete?: (id: string, label: string) => void;
	/** Called when a background task fails. */
	readonly onError?: (id: string, label: string, error: Error) => void;
}

// ---------------------------------------------------------------------------
// Internal state
// ---------------------------------------------------------------------------

interface InternalTask {
	readonly id: string;
	readonly label: string;
	readonly startedAt: number;
	status: 'running' | 'completed' | 'failed';
	result: unknown;
	error: Error | undefined;
	readonly promise: Promise<unknown>;
	readonly abortController: AbortController;
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createBackgroundManager(
	options?: BackgroundManagerOptions,
): BackgroundManager {
	const tasks = new Map<string, InternalTask>();

	const background = (label: string, promise: Promise<unknown>): string => {
		const id = randomUUID().slice(0, 8);
		const abortController = new AbortController();

		const wrappedPromise = promise
			.then((result) => {
				const task = tasks.get(id);
				if (task) {
					task.status = 'completed';
					task.result = result;
				}
				options?.onComplete?.(id, label);
				return result;
			})
			.catch((err) => {
				const task = tasks.get(id);
				const error = err instanceof Error ? err : new Error(String(err));
				if (task) {
					task.status = 'failed';
					task.error = error;
				}
				options?.onError?.(id, label, error);
			});

		const internalTask: InternalTask = {
			id,
			label,
			startedAt: Date.now(),
			status: 'running',
			result: undefined,
			error: undefined,
			promise: wrappedPromise,
			abortController,
		};

		tasks.set(id, internalTask);
		return id;
	};

	const foreground = (id: string): Promise<unknown> | undefined => {
		const task = tasks.get(id);
		if (!task) return undefined;
		return task.promise;
	};

	const list = (): readonly BackgroundTask[] => {
		const result: BackgroundTask[] = [];
		for (const task of tasks.values()) {
			result.push(
				Object.freeze({
					id: task.id,
					label: task.label,
					startedAt: task.startedAt,
					status: task.status,
				}),
			);
		}
		return result;
	};

	const abort = (id: string): void => {
		const task = tasks.get(id);
		if (task && task.status === 'running') {
			task.abortController.abort();
			task.status = 'failed';
			task.error = new Error('Aborted by user');
		}
	};

	const activeCount = (): number => {
		let count = 0;
		for (const task of tasks.values()) {
			if (task.status === 'running') count++;
		}
		return count;
	};

	return Object.freeze({ background, foreground, list, abort, activeCount });
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/**
 * Render background tasks list for /tasks command.
 */
export function renderBackgroundTasks(
	taskList: readonly BackgroundTask[],
	colors: {
		dim: (s: string) => string;
		cyan: (s: string) => string;
		green: (s: string) => string;
		yellow: (s: string) => string;
		red: (s: string) => string;
	},
): string {
	if (taskList.length === 0) {
		return `  ${colors.dim('No background tasks.')}`;
	}

	const lines: string[] = [];
	for (const task of taskList) {
		const icon =
			task.status === 'running'
				? colors.yellow('◌')
				: task.status === 'completed'
					? colors.green('●')
					: colors.red('●');

		const elapsed = formatElapsed(Date.now() - task.startedAt);
		lines.push(
			`  ${icon} ${colors.cyan(task.id)} ${task.label} ${colors.dim(elapsed)}`,
		);
	}

	return lines.join('\n');
}

function formatElapsed(ms: number): string {
	if (ms < 1000) return `${ms}ms`;
	if (ms < 60_000) return `${(ms / 1000).toFixed(0)}s`;
	const mins = Math.floor(ms / 60_000);
	const secs = Math.round((ms % 60_000) / 1000);
	return `${mins}m${secs}s`;
}
