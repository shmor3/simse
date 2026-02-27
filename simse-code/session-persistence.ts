/**
 * SimSE Code — Session Persistence
 *
 * Save and restore conversation sessions to gzipped JSON files.
 * Supports auto-save, resume, crash-safe atomic writes, and session listing.
 * No external deps.
 */

import { randomUUID } from 'node:crypto';
import {
	existsSync,
	mkdirSync,
	readdirSync,
	readFileSync,
	renameSync,
	rmSync,
	writeFileSync,
} from 'node:fs';
import { join } from 'node:path';
import { gunzipSync, gzipSync } from 'node:zlib';
import type {
	SessionRecord,
	SessionStore,
	SessionSummary,
} from './app-context.js';

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
// Internal helpers — crash-safe gzip I/O
// ---------------------------------------------------------------------------

function writeGzipAtomic(path: string, data: unknown): void {
	const dir = join(path, '..');
	mkdirSync(dir, { recursive: true });

	const json = JSON.stringify(data, null, '\t');
	const compressed = gzipSync(Buffer.from(json, 'utf-8'));

	// Write to a temp file first, then atomically rename.
	// If simse-code crashes mid-write, only the temp file is corrupt —
	// the previous session file is untouched.
	const tmpPath = `${path}.tmp`;
	writeFileSync(tmpPath, compressed);
	renameSync(tmpPath, path);
}

function readGzipFile<T>(path: string): T | undefined {
	try {
		if (!existsSync(path)) return undefined;
		const raw = readFileSync(path);
		const decompressed = gunzipSync(raw);
		return JSON.parse(decompressed.toString('utf-8')) as T;
	} catch {
		return undefined;
	}
}

function readJsonFileFallback<T>(path: string): T | undefined {
	try {
		if (!existsSync(path)) return undefined;
		const raw = readFileSync(path, 'utf-8');
		return JSON.parse(raw) as T;
	} catch {
		return undefined;
	}
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createSessionStore(options: SessionStoreOptions): SessionStore {
	const sessionsDir = join(options.dataDir, 'sessions');
	const maxSessions = options.maxSessions ?? 50;

	mkdirSync(sessionsDir, { recursive: true });

	const gzPath = (id: string): string => join(sessionsDir, `${id}.json.gz`);
	const legacyPath = (id: string): string => join(sessionsDir, `${id}.json`);

	const save = (session: SessionRecord): void => {
		writeGzipAtomic(gzPath(session.id), session);

		// Clean up legacy .json file if it exists
		const legacy = legacyPath(session.id);
		if (existsSync(legacy)) {
			try {
				rmSync(legacy);
			} catch {
				// Best-effort cleanup
			}
		}

		pruneOldSessions();
	};

	const load = (id: string): SessionRecord | undefined => {
		// Try gzip first, fall back to legacy JSON
		const gz = gzPath(id);
		if (existsSync(gz)) {
			return readGzipFile<SessionRecord>(gz);
		}
		return readJsonFileFallback<SessionRecord>(legacyPath(id));
	};

	const list = (): readonly SessionSummary[] => {
		if (!existsSync(sessionsDir)) return [];

		const files = readdirSync(sessionsDir)
			.filter((f) => f.endsWith('.json.gz') || f.endsWith('.json'))
			.sort((a, b) => {
				// Sort .json.gz before .json so first-seen-wins picks the newer format
				if (a.endsWith('.json.gz') && b.endsWith('.json')) return -1;
				if (a.endsWith('.json') && b.endsWith('.json.gz')) return 1;
				return a.localeCompare(b);
			});

		// Deduplicate: prefer .json.gz over .json for the same ID
		const seen = new Set<string>();
		const summaries: SessionSummary[] = [];

		for (const file of files) {
			const id = file.replace(/\.json(\.gz)?$/, '');
			if (seen.has(id)) continue;
			seen.add(id);

			const record = file.endsWith('.json.gz')
				? readGzipFile<SessionRecord>(join(sessionsDir, file))
				: readJsonFileFallback<SessionRecord>(join(sessionsDir, file));
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
		for (const path of [gzPath(id), legacyPath(id)]) {
			if (existsSync(path)) {
				rmSync(path);
			}
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
 * Generate a new session ID (truncated UUID).
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
