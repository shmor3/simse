// ---------------------------------------------------------------------------
// Vector store serialization / deserialization
// ---------------------------------------------------------------------------
//
// Extracted from vector-store.ts to isolate the binary encoding logic.
// Entry binary format per key in the KV store:
// [4b text-len][text][4b emb-b64-len][emb-b64][4b meta-json-len][meta-json]
// [8b timestamp][4b accessCount][8b lastAccessed]
// ---------------------------------------------------------------------------

import { Buffer } from 'node:buffer';
import { decodeEmbedding, encodeEmbedding } from './preservation.js';
import type { VectorEntry } from './types.js';
import type { LearningState } from './stacks-persistence.js';
import { isValidLearningState } from './stacks-persistence.js';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface AccessStats {
	accessCount: number;
	lastAccessed: number;
}

export interface DeserializedData {
	readonly entries: VectorEntry[];
	readonly accessStats: Map<string, AccessStats>;
	readonly learningState?: LearningState;
	readonly skipped: number;
}

export interface SerializedData {
	readonly data: Map<string, Buffer>;
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

export const LEARNING_KEY = '__learning';

// ---------------------------------------------------------------------------
// Per-entry binary codec
// ---------------------------------------------------------------------------

export function serializeEntry(
	entry: VectorEntry,
	stats?: { readonly accessCount: number; readonly lastAccessed: number },
): Buffer {
	const textBuf = Buffer.from(entry.text, 'utf-8');
	const embBuf = Buffer.from(encodeEmbedding(entry.embedding), 'utf-8');
	const metaBuf = Buffer.from(JSON.stringify(entry.metadata), 'utf-8');

	const totalSize =
		4 + textBuf.length + 4 + embBuf.length + 4 + metaBuf.length + 8 + 4 + 8;

	const buf = Buffer.alloc(totalSize);
	let offset = 0;

	buf.writeUInt32BE(textBuf.length, offset);
	offset += 4;
	textBuf.copy(buf, offset);
	offset += textBuf.length;

	buf.writeUInt32BE(embBuf.length, offset);
	offset += 4;
	embBuf.copy(buf, offset);
	offset += embBuf.length;

	buf.writeUInt32BE(metaBuf.length, offset);
	offset += 4;
	metaBuf.copy(buf, offset);
	offset += metaBuf.length;

	// Timestamp as two 32-bit halves (JS numbers are 64-bit floats)
	const ts = entry.timestamp;
	buf.writeUInt32BE(Math.floor(ts / 0x100000000), offset);
	offset += 4;
	buf.writeUInt32BE(ts >>> 0, offset);
	offset += 4;

	buf.writeUInt32BE(stats?.accessCount ?? 0, offset);
	offset += 4;

	const la = stats?.lastAccessed ?? 0;
	buf.writeUInt32BE(Math.floor(la / 0x100000000), offset);
	offset += 4;
	buf.writeUInt32BE(la >>> 0, offset);

	return buf;
}

export function deserializeEntry(
	id: string,
	buf: Buffer,
): {
	entry: VectorEntry;
	accessCount: number;
	lastAccessed: number;
} | null {
	try {
		let offset = 0;

		const textLen = buf.readUInt32BE(offset);
		offset += 4;
		const text = buf.toString('utf-8', offset, offset + textLen);
		offset += textLen;

		const embLen = buf.readUInt32BE(offset);
		offset += 4;
		const embB64 = buf.toString('utf-8', offset, offset + embLen);
		offset += embLen;
		const embedding = decodeEmbedding(embB64);

		const metaLen = buf.readUInt32BE(offset);
		offset += 4;
		const metaJson = buf.toString('utf-8', offset, offset + metaLen);
		offset += metaLen;
		const metadata: Record<string, string> = JSON.parse(metaJson);

		const tsHigh = buf.readUInt32BE(offset);
		offset += 4;
		const tsLow = buf.readUInt32BE(offset);
		offset += 4;
		const timestamp = tsHigh * 0x100000000 + tsLow;

		const accessCount = buf.readUInt32BE(offset);
		offset += 4;

		const laHigh = buf.readUInt32BE(offset);
		offset += 4;
		const laLow = buf.readUInt32BE(offset);
		const lastAccessed = laHigh * 0x100000000 + laLow;

		return {
			entry: { id, text, embedding, metadata, timestamp },
			accessCount,
			lastAccessed,
		};
	} catch {
		return null;
	}
}

// ---------------------------------------------------------------------------
// Bulk deserialization — converts raw storage data into in-memory structures
// ---------------------------------------------------------------------------

export function deserializeFromStorage(
	rawData: Map<string, Buffer>,
	logger?: {
		warn: (msg: string, metadata?: Readonly<Record<string, unknown>>) => void;
	},
): DeserializedData {
	const entries: VectorEntry[] = [];
	const accessStats = new Map<string, AccessStats>();
	let skipped = 0;
	let learningState: LearningState | undefined;

	for (const [key, value] of rawData) {
		if (key === LEARNING_KEY) {
			// Parse learning state
			try {
				const learningJson = value.toString('utf-8');
				const learningParsed: unknown = JSON.parse(learningJson);
				if (isValidLearningState(learningParsed)) {
					learningState = learningParsed;
				} else {
					logger?.warn('Invalid learning state — starting fresh');
				}
			} catch {
				logger?.warn('Failed to parse learning state — starting fresh');
			}
			continue;
		}

		const result = deserializeEntry(key, value);
		if (result === null) {
			skipped++;
			logger?.warn(`Skipping corrupt entry: ${key}`);
			continue;
		}

		entries.push(result.entry);

		if (result.accessCount > 0 || result.lastAccessed > 0) {
			accessStats.set(result.entry.id, {
				accessCount: result.accessCount,
				lastAccessed: result.lastAccessed,
			});
		}
	}

	return { entries, accessStats, learningState, skipped };
}

// ---------------------------------------------------------------------------
// Bulk serialization — converts in-memory structures into raw storage data
// ---------------------------------------------------------------------------

export function serializeToStorage(
	entries: readonly VectorEntry[],
	accessStats: ReadonlyMap<
		string,
		{ readonly accessCount: number; readonly lastAccessed: number }
	>,
	learningState?: {
		readonly hasData: boolean;
		readonly serialize: () => LearningState;
	},
): SerializedData {
	const data = new Map<string, Buffer>();

	for (const entry of entries) {
		const stats = accessStats.get(entry.id);
		data.set(entry.id, serializeEntry(entry, stats));
	}

	// Persist learning state alongside entries
	if (learningState?.hasData) {
		const state = learningState.serialize();
		const learningJson = JSON.stringify(state);
		data.set(LEARNING_KEY, Buffer.from(learningJson, 'utf-8'));
	}

	return { data };
}
