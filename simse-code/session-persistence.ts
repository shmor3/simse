/**
 * SimSE Code — Session Persistence
 *
 * Save and restore conversation sessions to JSON files.
 * Supports auto-save, resume, and session listing.
 * No external deps.
 */

import { randomUUID } from 'node:crypto';
import { existsSync, mkdirSync, readdirSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import type {
	SessionRecord,
	SessionStore,
	SessionSummary,
} from './app-context.js';
import { readJsonFile, writeJsonFile } from './json-io.js';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type { SessionRecord, SessionStore, SessionSummary };

export interface SessionStoreOptions {
	readonly dataDir: string;
	/** Debounce interval for auto-save (ms). Default: 2000 */
	readonly autoSaveDebounceMs?: number;
	/** Max number of sessions to keep. Default: 50 */
	readonly maxSessions?: number;
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createSessionStore(options: SessionStoreOptions): SessionStore {
	const sessionsDir = join(options.dataDir, 'sessions');
	const maxSessions = options.maxSessions ?? 50;

	mkdirSync(sessionsDir, { recursive: true });

	const sessionPath = (id: string): string => join(sessionsDir, `${id}.json`);

	const save = (session: SessionRecord): void => {
		writeJsonFile(sessionPath(session.id), session);
		pruneOldSessions();
	};

	const load = (id: string): SessionRecord | undefined => {
		const path = sessionPath(id);
		if (!existsSync(path)) return undefined;
		return readJsonFile<SessionRecord>(path);
	};

	const list = (): readonly SessionSummary[] => {
		if (!existsSync(sessionsDir)) return [];

		const files = readdirSync(sessionsDir).filter((f) => f.endsWith('.json'));
		const summaries: SessionSummary[] = [];

		for (const file of files) {
			const record = readJsonFile<SessionRecord>(join(sessionsDir, file));
			if (!record) continue;
			summaries.push({
				id: record.id,
				createdAt: record.createdAt,
				updatedAt: record.updatedAt,
				model: record.model,
				directory: record.directory,
				messageCount: record.messages.length,
			});
		}

		// Sort newest first
		summaries.sort((a, b) => b.updatedAt - a.updatedAt);
		return summaries;
	};

	const remove = (id: string): void => {
		const path = sessionPath(id);
		if (existsSync(path)) {
			rmSync(path);
		}
	};

	const pruneOldSessions = (): void => {
		const all = list();
		if (all.length > maxSessions) {
			const toRemove = all.slice(maxSessions);
			for (const session of toRemove) {
				remove(session.id);
			}
		}
	};

	return Object.freeze({ save, load, list, remove });
}

// ---------------------------------------------------------------------------
// Auto-save helper
// ---------------------------------------------------------------------------

export interface AutoSaver {
	readonly schedule: () => void;
	readonly saveNow: () => void;
	readonly dispose: () => void;
}

export interface AutoSaveOptions {
	readonly store: SessionStore;
	readonly getRecord: () => SessionRecord;
	readonly debounceMs?: number;
}

export function createAutoSaver(options: AutoSaveOptions): AutoSaver {
	const debounceMs = options.debounceMs ?? 2000;
	let timer: ReturnType<typeof setTimeout> | undefined;

	const saveNow = (): void => {
		if (timer) {
			clearTimeout(timer);
			timer = undefined;
		}
		options.store.save(options.getRecord());
	};

	const schedule = (): void => {
		if (timer) clearTimeout(timer);
		timer = setTimeout(saveNow, debounceMs);
	};

	const dispose = (): void => {
		if (timer) {
			clearTimeout(timer);
			timer = undefined;
		}
	};

	return Object.freeze({ schedule, saveNow, dispose });
}

/**
 * Generate a new session ID.
 */
export function newSessionId(): string {
	return randomUUID().slice(0, 8);
}

/**
 * Format a session summary for display.
 */
export function formatSessionSummary(
	summary: SessionSummary,
	colors: { dim: (s: string) => string; cyan: (s: string) => string },
): string {
	const date = new Date(summary.updatedAt);
	const relative = formatRelativeTime(date);
	const dir = summary.directory.replace(/\\/g, '/');
	const shortDir = dir.length > 40 ? `...${dir.slice(-37)}` : dir;
	return `${colors.cyan(summary.id)} ${colors.dim(relative)} ${shortDir} ${colors.dim(`(${summary.messageCount} msgs)`)}`;
}

function formatRelativeTime(date: Date): string {
	const now = Date.now();
	const diff = now - date.getTime();
	const mins = Math.floor(diff / 60_000);
	if (mins < 1) return 'just now';
	if (mins < 60) return `${mins}m ago`;
	const hours = Math.floor(mins / 60);
	if (hours < 24) return `${hours}h ago`;
	const days = Math.floor(hours / 24);
	if (days < 7) return `${days}d ago`;
	return date.toLocaleDateString();
}
