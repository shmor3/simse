/**
 * SimSE Code — File Change Tracking
 *
 * Tracks file additions and deletions for status line display.
 * Hooks into VFS onFileWrite callback.
 * No external deps.
 */

import type { FileChange, FileTracker } from './app-context.js';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type { FileChange, FileTracker };

export interface FileTrackerOptions {
	/** Initial changes to seed the tracker with (for session restore). */
	readonly initial?: readonly FileChange[];
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createFileTracker(options?: FileTrackerOptions): FileTracker {
	const changes = new Map<
		string,
		{ additions: number; deletions: number; isNew: boolean }
	>();

	// Seed with initial changes if provided
	if (options?.initial) {
		for (const change of options.initial) {
			changes.set(change.path, {
				additions: change.additions,
				deletions: change.deletions,
				isNew: change.isNew,
			});
		}
	}

	const track = (
		path: string,
		additions: number,
		deletions: number,
		isNew: boolean,
	): void => {
		const existing = changes.get(path);
		if (existing) {
			changes.set(path, {
				additions: existing.additions + additions,
				deletions: existing.deletions + deletions,
				isNew: existing.isNew || isNew,
			});
		} else {
			changes.set(path, { additions, deletions, isNew });
		}
	};

	const getChanges = (): readonly FileChange[] => {
		const result: FileChange[] = [];
		for (const [path, data] of changes) {
			result.push({
				path,
				additions: data.additions,
				deletions: data.deletions,
				isNew: data.isNew,
			});
		}
		return result.sort((a, b) => a.path.localeCompare(b.path));
	};

	const getTotals = (): { additions: number; deletions: number } => {
		let additions = 0;
		let deletions = 0;
		for (const data of changes.values()) {
			additions += data.additions;
			deletions += data.deletions;
		}
		return { additions, deletions };
	};

	const clear = (): void => {
		changes.clear();
	};

	return Object.freeze({ track, getChanges, getTotals, clear });
}
