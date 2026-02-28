/**
 * SimSE Code — Session Persistence
 *
 * Crash-safe session storage using JSONL (append-only) format.
 * Each conversation message is appended as a single line.
 * On load, lines are replayed to reconstruct the conversation.
 */

import { existsSync, readFileSync, unlinkSync } from 'node:fs';
import { join } from 'node:path';
import type { ConversationMessage, ConversationRole } from './conversation.js';
import { appendJsonLine, readJsonFile, writeJsonFile } from './json-io.js';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface SessionMeta {
	readonly id: string;
	readonly title: string;
	readonly createdAt: string;
	readonly updatedAt: string;
	readonly messageCount: number;
	readonly workDir: string;
}

/** A single line in the JSONL session file. */
interface SessionEntry {
	readonly ts: string;
	readonly role: ConversationRole;
	readonly content: string;
	readonly toolCallId?: string;
	readonly toolName?: string;
}

export interface SessionStore {
	/** Create a new session. Returns the session ID. */
	readonly create: (workDir: string) => string;
	/** Append a message to a session. Crash-safe (sync write). */
	readonly append: (sessionId: string, message: ConversationMessage) => void;
	/** Load all messages from a session. */
	readonly load: (sessionId: string) => readonly ConversationMessage[];
	/** List all sessions, newest first. */
	readonly list: () => readonly SessionMeta[];
	/** Get metadata for a specific session. */
	readonly get: (sessionId: string) => SessionMeta | undefined;
	/** Update the session title. */
	readonly rename: (sessionId: string, title: string) => void;
	/** Delete a session. */
	readonly remove: (sessionId: string) => void;
	/** Get the most recent session ID for the current workDir, or undefined. */
	readonly latest: (workDir: string) => string | undefined;
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createSessionStore(dataDir: string): SessionStore {
	const sessionsDir = join(dataDir, 'sessions');
	const indexPath = join(sessionsDir, 'index.json');

	/** Load or create the index. */
	const loadIndex = (): SessionMeta[] => {
		return readJsonFile<SessionMeta[]>(indexPath) ?? [];
	};

	/** Save the index. */
	const saveIndex = (index: SessionMeta[]): void => {
		writeJsonFile(indexPath, index);
	};

	const sessionFilePath = (id: string): string =>
		join(sessionsDir, `${id}.jsonl`);

	const generateId = (): string => {
		// 8 random hex chars + timestamp suffix for uniqueness
		const hex = Math.random().toString(16).slice(2, 10);
		const ts = Date.now().toString(36);
		return `${hex}-${ts}`;
	};

	const create = (workDir: string): string => {
		const id = generateId();
		const now = new Date().toISOString();
		const index = loadIndex();
		index.unshift({
			id,
			title: `Session ${new Date().toLocaleString()}`,
			createdAt: now,
			updatedAt: now,
			messageCount: 0,
			workDir,
		});
		saveIndex(index);
		return id;
	};

	const append = (sessionId: string, message: ConversationMessage): void => {
		const entry: SessionEntry = {
			ts: new Date().toISOString(),
			role: message.role,
			content: message.content,
			...(message.toolCallId ? { toolCallId: message.toolCallId } : {}),
			...(message.toolName ? { toolName: message.toolName } : {}),
		};
		appendJsonLine(sessionFilePath(sessionId), entry);

		// Update index metadata
		const index = loadIndex();
		const meta = index.find((s) => s.id === sessionId);
		if (meta) {
			const idx = index.indexOf(meta);
			index[idx] = {
				...meta,
				updatedAt: new Date().toISOString(),
				messageCount: meta.messageCount + 1,
			};
			saveIndex(index);
		}
	};

	const load = (sessionId: string): readonly ConversationMessage[] => {
		const filePath = sessionFilePath(sessionId);
		if (!existsSync(filePath)) return [];

		try {
			const raw = readFileSync(filePath, 'utf-8');
			const lines = raw.split('\n').filter((line) => line.trim().length > 0);

			return lines
				.map((line) => {
					try {
						const entry = JSON.parse(line) as SessionEntry;
						const msg: ConversationMessage = {
							role: entry.role,
							content: entry.content,
							...(entry.toolCallId ? { toolCallId: entry.toolCallId } : {}),
							...(entry.toolName ? { toolName: entry.toolName } : {}),
						};
						return Object.freeze(msg);
					} catch {
						return null;
					}
				})
				.filter((m): m is ConversationMessage => m !== null);
		} catch {
			return [];
		}
	};

	const list = (): readonly SessionMeta[] => {
		return loadIndex();
	};

	const get = (sessionId: string): SessionMeta | undefined => {
		return loadIndex().find((s) => s.id === sessionId);
	};

	const rename = (sessionId: string, title: string): void => {
		const index = loadIndex();
		const meta = index.find((s) => s.id === sessionId);
		if (meta) {
			const idx = index.indexOf(meta);
			index[idx] = { ...meta, title };
			saveIndex(index);
		}
	};

	const remove = (sessionId: string): void => {
		const index = loadIndex().filter((s) => s.id !== sessionId);
		saveIndex(index);
		const filePath = sessionFilePath(sessionId);
		if (existsSync(filePath)) {
			unlinkSync(filePath);
		}
	};

	const latest = (workDir: string): string | undefined => {
		const index = loadIndex();
		const matching = index.filter((s) => s.workDir === workDir);
		return matching[0]?.id;
	};

	return Object.freeze({
		create,
		append,
		load,
		list,
		get,
		rename,
		remove,
		latest,
	});
}
