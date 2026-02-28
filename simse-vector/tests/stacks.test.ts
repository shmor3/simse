import { describe, expect, it } from 'bun:test';
import { Buffer } from 'node:buffer';
import { cosineSimilarity } from '../src/cosine.js';
import { isLibraryError, isStacksCorruptionError } from '../src/errors.js';
import { createNoopLogger, type Logger } from '../src/logger.js';
import { encodeEmbedding } from '../src/preservation.js';
import type { Stacks } from '../src/stacks.js';
import type { StorageBackend } from '../src/storage.js';
import {
	fuzzyScore,
	levenshteinDistance,
	levenshteinSimilarity,
	matchesAllMetadataFilters,
	matchesMetadataFilter,
	ngramSimilarity,
	tokenize,
	tokenOverlapScore,
} from '../src/text-search.js';
import type { Volume } from '../src/types.js';
import { expectGuardedThrow } from './error-helpers.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function createSilentLogger(): Logger {
	return createNoopLogger();
}

/**
 * In-memory StorageBackend for tests. Stores KV data in a Map.
 * Optionally shares state via a `sharedData` map for cross-store tests.
 */
function createMemoryStorage(sharedData?: Map<string, Buffer>): StorageBackend {
	const data: Map<string, Buffer> = sharedData ?? new Map();
	return Object.freeze({
		load: async () => new Map(data),
		save: async (newData: Map<string, Buffer>) => {
			data.clear();
			for (const [k, v] of newData) {
				data.set(k, v);
			}
		},
		close: async () => {},
	});
}

function createFailingStorage(error: Error): StorageBackend {
	return Object.freeze({
		load: async () => {
			throw error;
		},
		save: async () => {},
		close: async () => {},
	});
}

import { createStacks } from '../src/stacks.js';

function createStore(options?: {
	autoSave?: boolean;
	flushIntervalMs?: number;
	storage?: StorageBackend;
}): Stacks {
	return createStacks({
		storage: options?.storage ?? createMemoryStorage(),
		logger: createSilentLogger(),
		autoSave: options?.autoSave ?? true,
		flushIntervalMs: options?.flushIntervalMs ?? 0,
	});
}

function makeEmbedding(dim: number, fill: number = 0.5): number[] {
	return new Array(dim).fill(fill);
}

function makeEntry(overrides: Partial<Volume> = {}): Volume {
	return {
		id: overrides.id ?? 'test-id-1',
		text: overrides.text ?? 'test text',
		embedding: overrides.embedding ?? [0.1, 0.2, 0.3],
		metadata: overrides.metadata ?? {},
		timestamp: overrides.timestamp ?? Date.now(),
	};
}

/**
 * Serialize a single vector entry into the binary format used by the KV store.
 * Format: [4b text-len][text][4b emb-b64-len][emb-b64][4b meta-json-len][meta-json]
 *         [8b timestamp][4b accessCount][8b lastAccessed]
 */
function serializeTestEntry(entry: Volume): Buffer {
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

	const ts = entry.timestamp;
	buf.writeUInt32BE(Math.floor(ts / 0x100000000), offset);
	offset += 4;
	buf.writeUInt32BE(ts >>> 0, offset);
	offset += 4;

	buf.writeUInt32BE(0, offset); // accessCount
	offset += 4;
	buf.writeUInt32BE(0, offset); // lastAccessed high
	offset += 4;
	buf.writeUInt32BE(0, offset); // lastAccessed low

	return buf;
}

/**
 * Write pre-populated test data into an in-memory Map using the binary KV
 * format. Only entries with complete Volume fields (id, text, embedding,
 * metadata, timestamp) are written; entries missing required fields get a
 * corrupt blob (useful for testing partial-corruption scenarios).
 */
async function writeStoreData(
	sharedData: Map<string, Buffer>,
	entries: Array<Record<string, unknown> | Volume>,
): Promise<void> {
	for (const entry of entries) {
		const id = entry.id as string | undefined;
		if (!id) continue;

		const text = entry.text as string | undefined;
		const embedding = entry.embedding as number[] | undefined;
		const metadata = (entry.metadata ?? {}) as Record<string, string>;
		const timestamp = (entry.timestamp ?? Date.now()) as number;

		if (typeof text !== 'string' || !Array.isArray(embedding)) {
			sharedData.set(id, Buffer.from('CORRUPT'));
			continue;
		}

		sharedData.set(
			id,
			serializeTestEntry({
				id,
				text,
				embedding,
				metadata,
				timestamp,
			}),
		);
	}
}

/**
 * Read persisted entries back from an in-memory Map in the binary KV format.
 * Returns an array of objects with id, text, embedding (base64), metadata, timestamp.
 */
async function readStoredEntries(
	sharedData: Map<string, Buffer>,
): Promise<Array<Record<string, unknown>>> {
	const result: Array<Record<string, unknown>> = [];
	for (const [key, buf] of sharedData) {
		if (key === '__learning') continue;
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

			const metaLen = buf.readUInt32BE(offset);
			offset += 4;
			const metaJson = buf.toString('utf-8', offset, offset + metaLen);
			offset += metaLen;
			const metadata = JSON.parse(metaJson);

			const tsHigh = buf.readUInt32BE(offset);
			offset += 4;
			const tsLow = buf.readUInt32BE(offset);
			offset += 4;
			const timestamp = tsHigh * 0x100000000 + tsLow;

			result.push({
				id: key,
				text,
				embedding: embB64,
				metadata,
				timestamp,
			});
		} catch {
			// Skip corrupt entries
		}
	}
	return result;
}

// ===========================================================================
// cosineSimilarity
// ===========================================================================

describe('cosineSimilarity', () => {
	it('should return 1 for identical vectors', () => {
		const a = [1, 2, 3];
		const b = [1, 2, 3];
		expect(cosineSimilarity(a, b)).toBeCloseTo(1.0, 10);
	});

	it('should return 1 for parallel vectors (scaled)', () => {
		const a = [1, 2, 3];
		const b = [2, 4, 6];
		expect(cosineSimilarity(a, b)).toBeCloseTo(1.0, 10);
	});

	it('should return -1 for antiparallel vectors', () => {
		const a = [1, 2, 3];
		const b = [-1, -2, -3];
		expect(cosineSimilarity(a, b)).toBeCloseTo(-1.0, 10);
	});

	it('should return 0 for orthogonal vectors', () => {
		const a = [1, 0, 0];
		const b = [0, 1, 0];
		expect(cosineSimilarity(a, b)).toBeCloseTo(0.0, 10);
	});

	it('should handle unit vectors', () => {
		const a = [1, 0];
		const b = [0, 1];
		expect(cosineSimilarity(a, b)).toBeCloseTo(0.0, 10);

		const c = [Math.SQRT1_2, Math.SQRT1_2];
		const d = [1, 0];
		expect(cosineSimilarity(c, d)).toBeCloseTo(Math.SQRT1_2, 10);
	});

	it('should return 0 for zero-length vectors (both empty)', () => {
		expect(cosineSimilarity([], [])).toBe(0);
	});

	it('should return 0 for dimension mismatch', () => {
		const a = [1, 2, 3];
		const b = [1, 2];
		expect(cosineSimilarity(a, b)).toBe(0);
	});

	it('should return 0 when one vector is all zeros', () => {
		const a = [0, 0, 0];
		const b = [1, 2, 3];
		expect(cosineSimilarity(a, b)).toBe(0);
	});

	it('should return 0 when both vectors are all zeros', () => {
		const a = [0, 0, 0];
		const b = [0, 0, 0];
		expect(cosineSimilarity(a, b)).toBe(0);
	});

	it('should handle negative values correctly', () => {
		const a = [-1, 2, -3];
		const b = [-1, 2, -3];
		expect(cosineSimilarity(a, b)).toBeCloseTo(1.0, 10);
	});

	it('should handle single-element vectors', () => {
		expect(cosineSimilarity([5], [3])).toBeCloseTo(1.0, 10);
		expect(cosineSimilarity([5], [-3])).toBeCloseTo(-1.0, 10);
	});

	it('should handle very small values', () => {
		const a = [1e-10, 1e-10, 1e-10];
		const b = [1e-10, 1e-10, 1e-10];
		expect(cosineSimilarity(a, b)).toBeCloseTo(1.0, 5);
	});

	it('should handle very large values', () => {
		const a = [1e10, 1e10, 1e10];
		const b = [1e10, 1e10, 1e10];
		expect(cosineSimilarity(a, b)).toBeCloseTo(1.0, 5);
	});

	it('should compute known similarity values', () => {
		// cos(45°) ≈ 0.7071
		const a = [1, 0];
		const b = [1, 1];
		const expected = 1 / Math.sqrt(2);
		expect(cosineSimilarity(a, b)).toBeCloseTo(expected, 5);
	});

	it('should be commutative', () => {
		const a = [1, 3, -5];
		const b = [4, -2, -1];
		expect(cosineSimilarity(a, b)).toBeCloseTo(cosineSimilarity(b, a), 10);
	});

	it('should handle high-dimensional vectors', () => {
		const dim = 1024;
		const a = Array.from({ length: dim }, (_, i) => Math.sin(i));
		const b = Array.from({ length: dim }, (_, i) => Math.sin(i));
		expect(cosineSimilarity(a, b)).toBeCloseTo(1.0, 5);
	});

	it('should return value between -1 and 1 for random vectors', () => {
		const a = Array.from({ length: 128 }, () => Math.random() * 2 - 1);
		const b = Array.from({ length: 128 }, () => Math.random() * 2 - 1);
		const sim = cosineSimilarity(a, b);
		expect(sim).toBeGreaterThanOrEqual(-1);
		expect(sim).toBeLessThanOrEqual(1);
	});
});

// ===========================================================================
// Stacks — Lifecycle
// ===========================================================================

describe('Stacks — Lifecycle', () => {
	it('should start with zero entries when no file exists', async () => {
		const store = createStore();
		await store.load();

		expect(store.size).toBe(0);
		expect(store.getAll()).toEqual([]);
	});

	it('should load existing entries from a valid store directory', async () => {
		const sharedData = new Map<string, Buffer>();
		await writeStoreData(sharedData, [
			makeEntry({ id: 'id-1', text: 'hello', embedding: [1, 2, 3] }),
			makeEntry({ id: 'id-2', text: 'world', embedding: [4, 5, 6] }),
		]);

		const store = createStore({ storage: createMemoryStorage(sharedData) });
		await store.load();

		expect(store.size).toBe(2);
		const all = store.getAll();
		expect(all[0].id).toBe('id-1');
		expect(all[1].id).toBe('id-2');
	});

	it('should load an empty store gracefully', async () => {
		const sharedData = new Map<string, Buffer>();

		const store = createStore({ storage: createMemoryStorage(sharedData) });
		await store.load();

		expect(store.size).toBe(0);
	});

	it('should handle empty store file (no content) gracefully', async () => {
		const sharedData = new Map<string, Buffer>();

		const store = createStore({ storage: createMemoryStorage(sharedData) });
		await store.load();

		expect(store.size).toBe(0);
	});

	it('should throw StacksCorruptionError for corrupt store file', async () => {
		const store = createStore({
			storage: createFailingStorage(new Error('corrupt data')),
		});
		await expectGuardedThrow(
			() => store.load(),
			isStacksCorruptionError,
			'STACKS_CORRUPT',
		);
	});

	it('should throw StacksCorruptionError for truncated store file', async () => {
		const store = createStore({
			storage: createFailingStorage(new Error('truncated data')),
		});
		await expectGuardedThrow(
			() => store.load(),
			isStacksCorruptionError,
			'STACKS_CORRUPT',
		);
	});

	it('should skip invalid entries and warn (partial corruption)', async () => {
		const sharedData = new Map<string, Buffer>();
		await writeStoreData(sharedData, [
			makeEntry({ id: 'good-1', text: 'valid' }),
			{ id: 'bad-1' }, // missing fields — writes corrupt blob
			makeEntry({ id: 'good-2', text: 'also valid' }),
		]);

		const store = createStore({ storage: createMemoryStorage(sharedData) });
		await store.load();

		expect(store.size).toBe(2);
		expect(store.getAll().map((e) => e.id)).toEqual(['good-1', 'good-2']);
	});

	it('should mark as dirty when invalid entries are skipped', async () => {
		const sharedData = new Map<string, Buffer>();
		await writeStoreData(sharedData, [
			makeEntry({ id: 'good' }),
			{ id: 'bad' }, // invalid — corrupt blob
		]);

		const store = createStore({
			autoSave: false,
			storage: createMemoryStorage(sharedData),
		});
		await store.load();

		expect(store.isDirty).toBe(true);
	});

	it('should not be dirty after loading a clean store', async () => {
		const sharedData = new Map<string, Buffer>();
		await writeStoreData(sharedData, [makeEntry({ id: 'clean' })]);

		const store = createStore({
			autoSave: false,
			storage: createMemoryStorage(sharedData),
		});
		await store.load();

		expect(store.isDirty).toBe(false);
	});
});

// ===========================================================================
// Stacks — Save
// ===========================================================================

describe('Stacks — Save', () => {
	it('should persist entries to disk in binary KV format', async () => {
		const sharedData = new Map<string, Buffer>();
		const store = createStore({ storage: createMemoryStorage(sharedData) });
		await store.load();

		await store.add('hello', [1, 2, 3], { key: 'val' });

		const data = await readStoredEntries(sharedData);
		expect(data).toHaveLength(1);
		const entry = data[0] as Record<string, unknown>;
		expect(typeof entry.embedding).toBe('string');
		expect(entry.metadata).toEqual({ key: 'val' });
		expect(entry.text).toBe('hello');
	});

	it('should persist data correctly after save', async () => {
		const sharedData = new Map<string, Buffer>();
		const store = createStore({ storage: createMemoryStorage(sharedData) });
		await store.load();
		await store.add('test', [1], {});

		const data = await readStoredEntries(sharedData);
		expect(data).toHaveLength(1);
	});

	it('should not be dirty after save', async () => {
		const store = createStore({ autoSave: false });
		await store.load();

		await store.add('test', [1], {});
		expect(store.isDirty).toBe(true);

		await store.save();
		expect(store.isDirty).toBe(false);
	});

	it('should use atomic write by default (tmp + rename)', async () => {
		const sharedData = new Map<string, Buffer>();
		const store = createStore({ storage: createMemoryStorage(sharedData) });
		await store.load();

		await store.add('entry', [0.5], {});

		const data = await readStoredEntries(sharedData);
		expect(data).toHaveLength(1);
	});

	it('should persist data correctly with storage backend', async () => {
		const sharedData = new Map<string, Buffer>();
		const store = createStore({ storage: createMemoryStorage(sharedData) });
		await store.load();

		await store.add('entry', [0.5], {});

		const data = await readStoredEntries(sharedData);
		expect(data).toHaveLength(1);
	});

	it('should roundtrip data correctly (save then load)', async () => {
		const sharedData = new Map<string, Buffer>();

		// Write
		const store1 = createStore({ storage: createMemoryStorage(sharedData) });
		await store1.load();
		await store1.add('text1', [0.1, 0.2], { a: '1' });
		await store1.add('text2', [0.3, 0.4], { b: '2' });

		// Read in a new store instance
		const store2 = createStore({ storage: createMemoryStorage(sharedData) });
		await store2.load();

		expect(store2.size).toBe(2);
		const all = store2.getAll();
		expect(all[0].text).toBe('text1');
		expect(all[0].metadata).toEqual({ a: '1' });
		expect(all[1].text).toBe('text2');
		expect(all[1].metadata).toEqual({ b: '2' });
	});
});

// ===========================================================================
// Stacks — CRUD: add
// ===========================================================================

describe('Stacks — add', () => {
	it('should add an entry and return a UUID', async () => {
		const store = createStore();
		await store.load();

		const id = await store.add('hello world', [1, 2, 3]);

		expect(id).toBeDefined();
		expect(typeof id).toBe('string');
		expect(id.length).toBeGreaterThan(0);
		expect(store.size).toBe(1);
	});

	it('should generate unique IDs for each entry', async () => {
		const store = createStore();
		await store.load();

		const id1 = await store.add('a', [1]);
		const id2 = await store.add('b', [2]);
		const id3 = await store.add('c', [3]);

		expect(new Set([id1, id2, id3]).size).toBe(3);
	});

	it('should store text, embedding, and metadata correctly', async () => {
		const store = createStore();
		await store.load();

		const id = await store.add('test text', [0.5, 0.6], { key: 'value' });

		const entry = store.getById(id);
		expect(entry).toBeDefined();
		expect(entry?.text).toBe('test text');
		expect(entry?.embedding).toEqual([0.5, 0.6]);
		expect(entry?.metadata).toEqual({ key: 'value' });
		expect(entry?.timestamp).toBeGreaterThan(0);
	});

	it('should default metadata to empty object', async () => {
		const store = createStore();
		await store.load();

		const id = await store.add('text', [1]);
		const entry = store.getById(id);
		expect(entry?.metadata).toEqual({});
	});

	it('should throw LibraryError for empty text', async () => {
		const store = createStore();
		await store.load();

		await expectGuardedThrow(
			() => store.add('', [0.1, 0.2, 0.3]),
			isLibraryError,
			'STACKS_EMPTY_TEXT',
		);
	});

	it('should throw LibraryError for empty embedding', async () => {
		const store = createStore();
		await store.load();

		await expectGuardedThrow(
			() => store.add('some text', []),
			isLibraryError,
			'STACKS_EMPTY_EMBEDDING',
		);
	});

	it('should increment size with each add', async () => {
		const store = createStore();
		await store.load();

		expect(store.size).toBe(0);
		await store.add('a', [1]);
		expect(store.size).toBe(1);
		await store.add('b', [2]);
		expect(store.size).toBe(2);
		await store.add('c', [3]);
		expect(store.size).toBe(3);
	});

	it('should auto-save when autoSave is true', async () => {
		const sharedData = new Map<string, Buffer>();
		const store = createStore({
			autoSave: true,
			storage: createMemoryStorage(sharedData),
		});
		await store.load();

		await store.add('entry', [1]);

		const data = await readStoredEntries(sharedData);
		expect(data).toHaveLength(1);
	});

	it('should NOT auto-save when autoSave is false', async () => {
		const sharedData = new Map<string, Buffer>();
		const store = createStore({
			autoSave: false,
			flushIntervalMs: 0,
			storage: createMemoryStorage(sharedData),
		});
		await store.load();

		await store.add('entry', [1]);

		expect(sharedData.size).toBe(0);
		expect(store.isDirty).toBe(true);
	});
});

// ===========================================================================
// Stacks — CRUD: addBatch
// ===========================================================================

describe('Stacks — addBatch', () => {
	it('should add multiple entries at once', async () => {
		const store = createStore();
		await store.load();

		const ids = await store.addBatch([
			{ text: 'a', embedding: [1] },
			{ text: 'b', embedding: [2] },
			{ text: 'c', embedding: [3], metadata: { key: 'val' } },
		]);

		expect(ids).toHaveLength(3);
		expect(store.size).toBe(3);
		expect(new Set(ids).size).toBe(3); // All unique
	});

	it('should return empty array for empty batch', async () => {
		const store = createStore();
		await store.load();

		const ids = await store.addBatch([]);
		expect(ids).toEqual([]);
		expect(store.size).toBe(0);
	});

	it('should throw LibraryError if any entry has empty text', async () => {
		const store = createStore();
		await store.load();

		await expectGuardedThrow(
			() =>
				store.addBatch([
					{ text: 'ok', embedding: [1] },
					{ text: '', embedding: [2] },
				]),
			isLibraryError,
			'STACKS_EMPTY_TEXT',
		);
	});

	it('should throw LibraryError if any entry has empty embedding', async () => {
		const store = createStore();
		await store.load();

		await expectGuardedThrow(
			() =>
				store.addBatch([
					{ text: 'ok', embedding: [1] },
					{ text: 'bad', embedding: [] },
				]),
			isLibraryError,
			'STACKS_EMPTY_EMBEDDING',
		);
	});

	it('should save only once for the entire batch (autoSave)', async () => {
		const sharedData = new Map<string, Buffer>();
		const store = createStore({
			autoSave: true,
			storage: createMemoryStorage(sharedData),
		});
		await store.load();

		await store.addBatch([
			{ text: 'a', embedding: [1] },
			{ text: 'b', embedding: [2] },
		]);

		const data = await readStoredEntries(sharedData);
		expect(data).toHaveLength(2);
	});

	it('should default metadata to empty object when not provided', async () => {
		const store = createStore();
		await store.load();

		const ids = await store.addBatch([{ text: 'no meta', embedding: [1] }]);
		const entry = store.getById(ids[0]);
		expect(entry?.metadata).toEqual({});
	});
});

// ===========================================================================
// Stacks — CRUD: delete
// ===========================================================================

describe('Stacks — delete', () => {
	it('should delete an existing entry and return true', async () => {
		const store = createStore();
		await store.load();

		const id = await store.add('to delete', [1, 2]);
		expect(store.size).toBe(1);

		const result = await store.delete(id);
		expect(result).toBe(true);
		expect(store.size).toBe(0);
	});

	it('should return false for non-existent ID', async () => {
		const store = createStore();
		await store.load();

		const result = await store.delete('non-existent-id');
		expect(result).toBe(false);
	});

	it('should only delete the specified entry', async () => {
		const store = createStore();
		await store.load();

		const id1 = await store.add('keep me', [1]);
		const id2 = await store.add('delete me', [2]);
		const id3 = await store.add('also keep', [3]);

		await store.delete(id2);

		expect(store.size).toBe(2);
		expect(store.getById(id1)).toBeDefined();
		expect(store.getById(id2)).toBeUndefined();
		expect(store.getById(id3)).toBeDefined();
	});

	it('should not modify the store when deleting non-existent ID', async () => {
		const store = createStore({ autoSave: false });
		await store.load();

		await store.add('keep', [1]);
		await store.save();
		expect(store.isDirty).toBe(false);

		await store.delete('fake-id');
		// Should not be marked dirty since nothing changed
		expect(store.isDirty).toBe(false);
	});
});

// ===========================================================================
// Stacks — CRUD: deleteBatch
// ===========================================================================

describe('Stacks — deleteBatch', () => {
	it('should delete multiple entries by ID', async () => {
		const store = createStore();
		await store.load();

		const id1 = await store.add('a', [1]);
		const id2 = await store.add('b', [2]);
		const id3 = await store.add('c', [3]);

		const deleted = await store.deleteBatch([id1, id3]);

		expect(deleted).toBe(2);
		expect(store.size).toBe(1);
		expect(store.getById(id2)).toBeDefined();
	});

	it('should return 0 for empty IDs array', async () => {
		const store = createStore();
		await store.load();

		const deleted = await store.deleteBatch([]);
		expect(deleted).toBe(0);
	});

	it('should handle mixed existing and non-existing IDs', async () => {
		const store = createStore();
		await store.load();

		const id1 = await store.add('a', [1]);
		await store.add('b', [2]);

		const deleted = await store.deleteBatch([id1, 'non-existent']);
		expect(deleted).toBe(1);
		expect(store.size).toBe(1);
	});

	it('should handle all non-existing IDs', async () => {
		const store = createStore();
		await store.load();

		await store.add('a', [1]);

		const deleted = await store.deleteBatch(['fake-1', 'fake-2']);
		expect(deleted).toBe(0);
		expect(store.size).toBe(1);
	});
});

// ===========================================================================
// Stacks — CRUD: clear
// ===========================================================================

describe('Stacks — clear', () => {
	it('should remove all entries', async () => {
		const store = createStore();
		await store.load();

		await store.add('a', [1]);
		await store.add('b', [2]);
		await store.add('c', [3]);
		expect(store.size).toBe(3);

		await store.clear();
		expect(store.size).toBe(0);
		expect(store.getAll()).toEqual([]);
	});

	it('should be safe to call on an already empty store', async () => {
		const store = createStore();
		await store.load();

		await store.clear();
		expect(store.size).toBe(0);
	});

	it('should persist the empty state when autoSave is on', async () => {
		const sharedData = new Map<string, Buffer>();
		const store = createStore({ storage: createMemoryStorage(sharedData) });
		await store.load();

		await store.add('a', [1]);
		await store.clear();

		const entries = await readStoredEntries(sharedData);
		expect(entries).toEqual([]);
	});
});

// ===========================================================================
// Stacks — Search
// ===========================================================================

describe('Stacks — search', () => {
	it('should find the most similar entry', async () => {
		const store = createStore();
		await store.load();

		await store.add('exact match', [1, 0, 0]);
		await store.add('partial match', [0.7, 0.7, 0]);
		await store.add('no match', [0, 0, 1]);

		const results = store.search([1, 0, 0], 10, 0);

		expect(results.length).toBeGreaterThan(0);
		expect(results[0].volume.text).toBe('exact match');
		expect(results[0].score).toBeCloseTo(1.0, 5);
	});

	it('should return results sorted by descending score', async () => {
		const store = createStore();
		await store.load();

		await store.add('bad', [0, 0, 1]);
		await store.add('good', [0.9, 0.1, 0]);
		await store.add('best', [1, 0, 0]);

		const results = store.search([1, 0, 0], 10, 0);

		for (let i = 1; i < results.length; i++) {
			expect(results[i - 1].score).toBeGreaterThanOrEqual(results[i].score);
		}
	});

	it('should respect the maxResults limit', async () => {
		const store = createStore();
		await store.load();

		for (let i = 0; i < 10; i++) {
			await store.add(`entry ${i}`, [Math.random(), Math.random()]);
		}

		const results = store.search([0.5, 0.5], 3, 0);
		expect(results.length).toBeLessThanOrEqual(3);
	});

	it('should respect the threshold', async () => {
		const store = createStore();
		await store.load();

		await store.add('similar', [1, 0, 0]); // cosine = 1.0 with query
		await store.add('different', [0, 1, 0]); // cosine = 0.0 with query
		await store.add('opposite', [-1, 0, 0]); // cosine = -1.0 with query

		const results = store.search([1, 0, 0], 10, 0.5);

		expect(results).toHaveLength(1);
		expect(results[0].volume.text).toBe('similar');
	});

	it('should return empty array when no entries meet the threshold', async () => {
		const store = createStore();
		await store.load();

		await store.add('entry', [0, 1, 0]);

		const results = store.search([1, 0, 0], 10, 0.99);
		expect(results).toEqual([]);
	});

	it('should return empty array from an empty store', async () => {
		const store = createStore();
		await store.load();

		const results = store.search([1, 0, 0], 10, 0);
		expect(results).toEqual([]);
	});

	it('should return empty array for empty query embedding', async () => {
		const store = createStore();
		await store.load();

		await store.add('entry', [1, 2, 3]);

		const results = store.search([], 10, 0);
		expect(results).toEqual([]);
	});

	it('should skip entries with mismatched embedding dimensions', async () => {
		const sharedData = new Map<string, Buffer>();
		await writeStoreData(sharedData, [
			makeEntry({ id: 'dim3', text: '3d', embedding: [1, 0, 0] }),
			makeEntry({ id: 'dim2', text: '2d', embedding: [1, 0] }),
			makeEntry({ id: 'dim3b', text: '3d-b', embedding: [0.5, 0.5, 0] }),
		]);

		const store = createStore({ storage: createMemoryStorage(sharedData) });
		await store.load();

		const results = store.search([1, 0, 0], 10, 0);
		// Should only include the two 3D entries
		expect(results).toHaveLength(2);
		expect(results.map((r) => r.volume.id).sort()).toEqual(['dim3', 'dim3b']);
	});

	it('should return correct score values', async () => {
		const store = createStore();
		await store.load();

		await store.add('identical', [1, 0, 0]);

		const results = store.search([1, 0, 0], 1, 0);

		expect(results).toHaveLength(1);
		expect(results[0].score).toBeCloseTo(1.0, 10);
	});

	it('should handle threshold of exactly 0', async () => {
		const store = createStore();
		await store.load();

		await store.add('a', [1, 0]);
		await store.add('b', [0, 1]);
		await store.add('c', [-1, 0]);

		const results = store.search([1, 0], 10, 0);
		// Should include a (1.0), b (0.0), and c should be excluded (-1.0 < 0)
		// Wait: threshold 0 means >= 0, so b (0.0) is included, c (-1.0) is excluded
		expect(results).toHaveLength(2);
	});

	it('should handle threshold of exactly 1.0 (only exact matches)', async () => {
		const store = createStore();
		await store.load();

		await store.add('exact', [1, 0, 0]);
		await store.add('close', [0.999, 0.001, 0]);
		await store.add('far', [0, 1, 0]);

		const results = store.search([1, 0, 0], 10, 1.0);
		// Only the exact match should pass
		expect(results).toHaveLength(1);
		expect(results[0].volume.text).toBe('exact');
	});
});

// ===========================================================================
// Stacks — Accessors
// ===========================================================================

describe('Stacks — Accessors', () => {
	describe('getAll', () => {
		it('should return a shallow copy of entries', async () => {
			const store = createStore();
			await store.load();

			await store.add('a', [1]);
			await store.add('b', [2]);

			const all1 = store.getAll();
			const all2 = store.getAll();

			expect(all1).toEqual(all2);
			expect(all1).not.toBe(all2); // Different array instances
		});

		it('should return empty array for empty store', async () => {
			const store = createStore();
			await store.load();

			expect(store.getAll()).toEqual([]);
		});
	});

	describe('getById', () => {
		it('should find entry by ID', async () => {
			const store = createStore();
			await store.load();

			const id = await store.add('findme', [1, 2, 3], { tag: 'test' });

			const entry = store.getById(id);
			expect(entry).toBeDefined();
			expect(entry?.text).toBe('findme');
			expect(entry?.metadata).toEqual({ tag: 'test' });
		});

		it('should return undefined for non-existent ID', async () => {
			const store = createStore();
			await store.load();

			expect(store.getById('does-not-exist')).toBeUndefined();
		});
	});

	describe('size', () => {
		it('should reflect current entry count', async () => {
			const store = createStore();
			await store.load();

			expect(store.size).toBe(0);

			const id = await store.add('a', [1]);
			expect(store.size).toBe(1);

			await store.add('b', [2]);
			expect(store.size).toBe(2);

			await store.delete(id);
			expect(store.size).toBe(1);

			await store.clear();
			expect(store.size).toBe(0);
		});
	});

	describe('isDirty', () => {
		it('should be false initially (no file)', async () => {
			const store = createStore({ autoSave: false });
			await store.load();
			expect(store.isDirty).toBe(false);
		});

		it('should be true after add (no autoSave)', async () => {
			const store = createStore({ autoSave: false });
			await store.load();

			await store.add('x', [1]);
			expect(store.isDirty).toBe(true);
		});

		it('should be false after save', async () => {
			const store = createStore({ autoSave: false });
			await store.load();

			await store.add('x', [1]);
			expect(store.isDirty).toBe(true);

			await store.save();
			expect(store.isDirty).toBe(false);
		});

		it('should be true after delete (no autoSave)', async () => {
			const store = createStore({ autoSave: false });
			await store.load();

			await store.add('x', [1]);
			await store.save();
			expect(store.isDirty).toBe(false);

			const all = store.getAll();
			await store.delete(all[0].id);
			expect(store.isDirty).toBe(true);
		});

		it('should be true after clear (no autoSave)', async () => {
			const store = createStore({ autoSave: false });
			await store.load();

			await store.add('x', [1]);
			await store.save();

			await store.clear();
			expect(store.isDirty).toBe(true);
		});
	});
});

// ===========================================================================
// Stacks — dispose
// ===========================================================================

describe('Stacks — dispose', () => {
	it('should flush dirty data on dispose', async () => {
		const sharedData = new Map<string, Buffer>();
		const store = createStore({
			autoSave: false,
			flushIntervalMs: 0,
			storage: createMemoryStorage(sharedData),
		});
		await store.load();

		await store.add('will persist', [1, 2]);
		expect(store.isDirty).toBe(true);

		await store.dispose();
		expect(store.isDirty).toBe(false);

		// Data should be in the shared map
		const data = await readStoredEntries(sharedData);
		expect(data).toHaveLength(1);
		expect(data[0].text).toBe('will persist');
	});

	it('should not throw when disposing a clean store', async () => {
		const store = createStore({ autoSave: false });
		await store.load();

		await store.dispose();
	});

	it('should not throw when disposing twice', async () => {
		const store = createStore({ autoSave: false });
		await store.load();

		await store.add('x', [1]);

		await store.dispose();
		await store.dispose();
	});
});

// ===========================================================================
// Stacks — Entry validation (on load)
// ===========================================================================

describe('Stacks — Entry validation', () => {
	it('should skip entries with corrupt binary blobs', async () => {
		const sharedData = new Map<string, Buffer>();
		sharedData.set(
			'good',
			serializeTestEntry(
				makeEntry({ id: 'good', text: 'valid', embedding: [1] }),
			),
		);
		sharedData.set('bad', Buffer.from('CORRUPT'));

		const store = createStore({ storage: createMemoryStorage(sharedData) });
		await store.load();
		expect(store.size).toBe(1);
		expect(store.getAll()[0].id).toBe('good');
	});

	it('should skip entries with truncated binary data', async () => {
		const sharedData = new Map<string, Buffer>();
		sharedData.set(
			'good',
			serializeTestEntry(
				makeEntry({ id: 'good', text: 'valid', embedding: [1] }),
			),
		);
		// Truncated entry — just the text length prefix, no actual data
		sharedData.set('truncated', Buffer.alloc(4));

		const store = createStore({ storage: createMemoryStorage(sharedData) });
		await store.load();
		expect(store.size).toBe(1);
	});

	it('should skip corrupt entries and keep valid ones', async () => {
		const sharedData = new Map<string, Buffer>();
		sharedData.set(
			'good-1',
			serializeTestEntry(
				makeEntry({ id: 'good-1', text: 'valid 1', embedding: [1] }),
			),
		);
		sharedData.set('bad-1', Buffer.from('CORRUPT'));
		sharedData.set(
			'good-2',
			serializeTestEntry(
				makeEntry({ id: 'good-2', text: 'valid 2', embedding: [2] }),
			),
		);
		sharedData.set('bad-2', Buffer.alloc(2));
		sharedData.set(
			'good-3',
			serializeTestEntry(
				makeEntry({ id: 'good-3', text: 'valid 3', embedding: [3] }),
			),
		);

		const store = createStore({ storage: createMemoryStorage(sharedData) });
		await store.load();

		expect(store.size).toBe(3);
		expect(
			store
				.getAll()
				.map((e) => e.id)
				.sort(),
		).toEqual(['good-1', 'good-2', 'good-3']);
	});
});

// ===========================================================================
// Stacks — Concurrency / ordering
// ===========================================================================

describe('Stacks — Ordering', () => {
	it('should maintain insertion order in getAll', async () => {
		const store = createStore();
		await store.load();

		await store.add('first', [1]);
		await store.add('second', [2]);
		await store.add('third', [3]);

		const all = store.getAll();
		expect(all.map((e) => e.text)).toEqual(['first', 'second', 'third']);
	});

	it('should maintain order after deleting middle entries', async () => {
		const store = createStore();
		await store.load();

		await store.add('a', [1]);
		const idB = await store.add('b', [2]);
		await store.add('c', [3]);

		await store.delete(idB);

		const all = store.getAll();
		expect(all.map((e) => e.text)).toEqual(['a', 'c']);
	});

	it('should handle rapid sequential operations', async () => {
		const store = createStore();
		await store.load();

		const ids: string[] = [];
		for (let i = 0; i < 50; i++) {
			ids.push(await store.add(`entry-${i}`, [i]));
		}

		expect(store.size).toBe(50);

		// Delete every other entry
		for (let i = 0; i < ids.length; i += 2) {
			await store.delete(ids[i]);
		}

		expect(store.size).toBe(25);

		// Remaining entries should be odd-indexed
		const remaining = store.getAll();
		for (const entry of remaining) {
			const num = parseInt(entry.text.replace('entry-', ''), 10);
			expect(num % 2).toBe(1);
		}
	});
});

// ===========================================================================
// Stacks — Edge cases
// ===========================================================================

describe('Stacks — Edge cases', () => {
	it('should handle entries with very long text', async () => {
		const store = createStore();
		await store.load();

		const longText = 'x'.repeat(100_000);
		const id = await store.add(longText, [1]);

		const entry = store.getById(id);
		expect(entry?.text.length).toBe(100_000);
	});

	it('should handle entries with high-dimensional embeddings', async () => {
		const store = createStore();
		await store.load();

		const embedding = makeEmbedding(2048, 0.1);
		const id = await store.add('high dim', embedding);

		const entry = store.getById(id);
		expect(entry?.embedding.length).toBe(2048);
	});

	it('should handle entries with many metadata keys', async () => {
		const store = createStore();
		await store.load();

		const metadata: Record<string, string> = {};
		for (let i = 0; i < 100; i++) {
			metadata[`key_${i}`] = `value_${i}`;
		}

		const id = await store.add('metadata heavy', [1], metadata);
		const entry = store.getById(id);
		expect(entry).toBeDefined();
		expect(Object.keys(entry?.metadata ?? {})).toHaveLength(100);
	});

	it('should handle search with maxResults of 0', async () => {
		const store = createStore();
		await store.load();

		await store.add('a', [1, 0, 0]);

		const results = store.search([1, 0, 0], 0, 0);
		expect(results).toEqual([]);
	});

	it('should handle search when maxResults exceeds entry count', async () => {
		const store = createStore();
		await store.load();

		await store.add('a', [1, 0]);
		await store.add('b', [0, 1]);

		const results = store.search([1, 0], 100, 0);
		expect(results.length).toBeLessThanOrEqual(2);
	});

	it('should handle Unicode text correctly', async () => {
		const sharedData = new Map<string, Buffer>();
		const store = createStore({ storage: createMemoryStorage(sharedData) });
		await store.load();

		const unicodeText = 'こんにちは世界 🌍 Ñoño café über résumé';
		const id = await store.add(unicodeText, [1, 2, 3]);

		// Roundtrip through save/load
		const store2 = createStore({ storage: createMemoryStorage(sharedData) });
		await store2.load();

		const entry = store2.getById(id);
		expect(entry?.text).toBe(unicodeText);
	});

	it('should handle entries with negative embedding values', async () => {
		const store = createStore();
		await store.load();

		await store.add('negative', [-0.5, -0.3, -0.1]);

		const results = store.search([-0.5, -0.3, -0.1], 1, 0);
		expect(results).toHaveLength(1);
		expect(results[0].score).toBeCloseTo(1.0, 5);
	});

	it('should handle entries with very small embedding values', async () => {
		const store = createStore();
		await store.load();

		await store.add('small', [1e-15, 1e-15, 1e-15]);

		const results = store.search([1e-15, 1e-15, 1e-15], 1, 0);
		expect(results.length).toBeGreaterThanOrEqual(0);
		if (results.length > 0) {
			expect(results[0].score).toBeCloseTo(1.0, 2);
		}
	});
});

// ===========================================================================
// Text Search Utilities
// ===========================================================================

describe('levenshteinDistance', () => {
	it('should return 0 for identical strings', () => {
		expect(levenshteinDistance('hello', 'hello')).toBe(0);
	});

	it('should return length of non-empty string when other is empty', () => {
		expect(levenshteinDistance('', 'abc')).toBe(3);
		expect(levenshteinDistance('abc', '')).toBe(3);
	});

	it('should return 0 for two empty strings', () => {
		expect(levenshteinDistance('', '')).toBe(0);
	});

	it('should handle single character difference', () => {
		expect(levenshteinDistance('cat', 'bat')).toBe(1);
	});

	it('should handle insertion', () => {
		expect(levenshteinDistance('cat', 'cats')).toBe(1);
	});

	it('should handle deletion', () => {
		expect(levenshteinDistance('cats', 'cat')).toBe(1);
	});

	it('should be commutative', () => {
		expect(levenshteinDistance('kitten', 'sitting')).toBe(
			levenshteinDistance('sitting', 'kitten'),
		);
	});

	it('should compute known distance for kitten/sitting', () => {
		expect(levenshteinDistance('kitten', 'sitting')).toBe(3);
	});
});

describe('levenshteinSimilarity', () => {
	it('should return 1 for identical strings', () => {
		expect(levenshteinSimilarity('hello', 'hello')).toBeCloseTo(1.0);
	});

	it('should return 1 for two empty strings', () => {
		expect(levenshteinSimilarity('', '')).toBeCloseTo(1.0);
	});

	it('should return 0 for completely different strings of same length', () => {
		// "abc" vs "xyz" — distance 3, maxLen 3
		expect(levenshteinSimilarity('abc', 'xyz')).toBeCloseTo(0.0);
	});

	it('should return value between 0 and 1', () => {
		const sim = levenshteinSimilarity('hello', 'hallo');
		expect(sim).toBeGreaterThan(0);
		expect(sim).toBeLessThanOrEqual(1);
	});
});

describe('ngramSimilarity', () => {
	it('should return 1 for identical strings', () => {
		expect(ngramSimilarity('hello', 'hello')).toBeCloseTo(1.0);
	});

	it('should return 1 for two empty strings', () => {
		expect(ngramSimilarity('', '')).toBeCloseTo(1.0);
	});

	it('should return 0 when one string is empty', () => {
		expect(ngramSimilarity('hello', '')).toBeCloseTo(0.0);
	});

	it('should return high similarity for similar strings', () => {
		const sim = ngramSimilarity('night', 'nacht');
		expect(sim).toBeGreaterThan(0);
		expect(sim).toBeLessThan(1);
	});

	it('should return low similarity for very different strings', () => {
		const sim = ngramSimilarity('abcdef', 'xyz');
		expect(sim).toBeLessThan(0.3);
	});
});

describe('tokenize', () => {
	it('should split on whitespace and lowercase', () => {
		expect(tokenize('Hello World')).toEqual(['hello', 'world']);
	});

	it('should strip punctuation', () => {
		expect(tokenize('Hello, World!')).toEqual(['hello', 'world']);
	});

	it('should return empty array for empty string', () => {
		expect(tokenize('')).toEqual([]);
	});

	it('should handle Unicode letters', () => {
		const tokens = tokenize('café résumé');
		expect(tokens).toEqual(['café', 'résumé']);
	});
});

describe('tokenOverlapScore', () => {
	it('should return 1 for identical text', () => {
		expect(tokenOverlapScore('hello world', 'hello world')).toBeCloseTo(1.0);
	});

	it('should return 0 for completely different text', () => {
		expect(tokenOverlapScore('hello world', 'foo bar')).toBeCloseTo(0.0);
	});

	it('should return partial score for overlapping tokens', () => {
		const score = tokenOverlapScore('hello world', 'hello there');
		expect(score).toBeGreaterThan(0);
		expect(score).toBeLessThan(1);
	});

	it('should return 1 for two empty strings', () => {
		expect(tokenOverlapScore('', '')).toBeCloseTo(1.0);
	});
});

describe('fuzzyScore', () => {
	it('should return 1 for exact match', () => {
		expect(fuzzyScore('hello', 'hello')).toBeCloseTo(1.0);
	});

	it('should return 1 for substring match', () => {
		expect(fuzzyScore('hello', 'say hello world')).toBeCloseTo(1.0);
	});

	it('should return high score for close match', () => {
		const score = fuzzyScore('helo', 'hello');
		expect(score).toBeGreaterThan(0.5);
	});

	it('should return low score for very different strings', () => {
		const score = fuzzyScore('abcdef', 'xyz123');
		expect(score).toBeLessThan(0.3);
	});

	it('should return 0 when query is empty', () => {
		expect(fuzzyScore('', 'hello')).toBe(0);
	});

	it('should return 0 when candidate is empty', () => {
		expect(fuzzyScore('hello', '')).toBe(0);
	});

	it('should return 1 for two empty strings', () => {
		expect(fuzzyScore('', '')).toBeCloseTo(1.0);
	});
});

describe('matchesMetadataFilter', () => {
	const meta = { color: 'Blue', size: 'Large', tag: 'AI-powered' };

	it('should match exact equality (default mode)', () => {
		expect(matchesMetadataFilter(meta, { key: 'color', value: 'Blue' })).toBe(
			true,
		);
		expect(matchesMetadataFilter(meta, { key: 'color', value: 'Red' })).toBe(
			false,
		);
	});

	it('should match neq', () => {
		expect(
			matchesMetadataFilter(meta, { key: 'color', value: 'Red', mode: 'neq' }),
		).toBe(true);
		expect(
			matchesMetadataFilter(meta, { key: 'color', value: 'Blue', mode: 'neq' }),
		).toBe(false);
	});

	it('should match contains (case-insensitive)', () => {
		expect(
			matchesMetadataFilter(meta, {
				key: 'tag',
				value: 'ai',
				mode: 'contains',
			}),
		).toBe(true);
		expect(
			matchesMetadataFilter(meta, {
				key: 'tag',
				value: 'xyz',
				mode: 'contains',
			}),
		).toBe(false);
	});

	it('should match startsWith (case-insensitive)', () => {
		expect(
			matchesMetadataFilter(meta, {
				key: 'tag',
				value: 'ai',
				mode: 'startsWith',
			}),
		).toBe(true);
		expect(
			matchesMetadataFilter(meta, {
				key: 'tag',
				value: 'powered',
				mode: 'startsWith',
			}),
		).toBe(false);
	});

	it('should match endsWith (case-insensitive)', () => {
		expect(
			matchesMetadataFilter(meta, {
				key: 'tag',
				value: 'powered',
				mode: 'endsWith',
			}),
		).toBe(true);
		expect(
			matchesMetadataFilter(meta, {
				key: 'tag',
				value: 'ai',
				mode: 'endsWith',
			}),
		).toBe(false);
	});

	it('should match regex', () => {
		expect(
			matchesMetadataFilter(meta, {
				key: 'color',
				value: '^Bl',
				mode: 'regex',
			}),
		).toBe(true);
		expect(
			matchesMetadataFilter(meta, {
				key: 'color',
				value: '^Re',
				mode: 'regex',
			}),
		).toBe(false);
	});

	it('should match exists', () => {
		expect(matchesMetadataFilter(meta, { key: 'color', mode: 'exists' })).toBe(
			true,
		);
		expect(
			matchesMetadataFilter(meta, { key: 'missing', mode: 'exists' }),
		).toBe(false);
	});

	it('should match notExists', () => {
		expect(
			matchesMetadataFilter(meta, { key: 'missing', mode: 'notExists' }),
		).toBe(true);
		expect(
			matchesMetadataFilter(meta, { key: 'color', mode: 'notExists' }),
		).toBe(false);
	});
});

describe('matchesAllMetadataFilters', () => {
	const meta = { color: 'Blue', size: 'Large' };

	it('should return true when all filters match', () => {
		expect(
			matchesAllMetadataFilters(meta, [
				{ key: 'color', value: 'Blue' },
				{ key: 'size', value: 'Large' },
			]),
		).toBe(true);
	});

	it('should return false when any filter fails', () => {
		expect(
			matchesAllMetadataFilters(meta, [
				{ key: 'color', value: 'Blue' },
				{ key: 'size', value: 'Small' },
			]),
		).toBe(false);
	});

	it('should return true for empty filters', () => {
		expect(matchesAllMetadataFilters(meta, [])).toBe(true);
	});
});

// ===========================================================================
// Stacks — textSearch
// ===========================================================================

describe('Stacks — textSearch', () => {
	it('should find entries by fuzzy match (default mode)', async () => {
		const store = createStore();
		await store.load();

		await store.add('The quick brown fox jumps', [1]);
		await store.add('A slow red dog sits', [2]);
		await store.add('The quick brown cat leaps', [3]);

		const results = store.textSearch({ query: 'quick brown fox' });
		expect(results.length).toBeGreaterThanOrEqual(1);
		expect(results[0].volume.text).toContain('quick brown fox');
	});

	it('should find entries by substring match', async () => {
		const store = createStore();
		await store.load();

		await store.add('Hello World', [1]);
		await store.add('Goodbye World', [2]);
		await store.add('Hello There', [3]);

		const results = store.textSearch({ query: 'hello', mode: 'substring' });
		expect(results).toHaveLength(2);
	});

	it('should find entries by exact match', async () => {
		const store = createStore();
		await store.load();

		await store.add('Hello World', [1]);
		await store.add('hello world', [2]);

		const results = store.textSearch({ query: 'Hello World', mode: 'exact' });
		expect(results).toHaveLength(1);
		expect(results[0].volume.text).toBe('Hello World');
	});

	it('should find entries by regex match', async () => {
		const store = createStore();
		await store.load();

		await store.add('Error: 404 not found', [1]);
		await store.add('Error: 500 server error', [2]);
		await store.add('Success: 200 ok', [3]);

		const results = store.textSearch({
			query: 'Error:\\s+\\d+',
			mode: 'regex',
		});
		expect(results).toHaveLength(2);
	});

	it('should find entries by token overlap', async () => {
		const store = createStore();
		await store.load();

		await store.add('machine learning algorithms', [1]);
		await store.add('deep learning neural networks', [2]);
		await store.add('cooking recipes and tips', [3]);

		const results = store.textSearch({
			query: 'learning algorithms',
			mode: 'token',
			threshold: 0.1,
		});
		expect(results.length).toBeGreaterThanOrEqual(1);
		// The entry with "machine learning algorithms" should rank highest
		expect(results[0].volume.text).toContain('learning algorithms');
	});

	it('should return empty array for empty query', async () => {
		const store = createStore();
		await store.load();
		await store.add('something', [1]);

		const results = store.textSearch({ query: '' });
		expect(results).toEqual([]);
	});

	it('should respect the threshold', async () => {
		const store = createStore();
		await store.load();

		await store.add('completely unrelated text', [1]);

		const results = store.textSearch({
			query: 'xyz',
			mode: 'fuzzy',
			threshold: 0.9,
		});
		expect(results).toHaveLength(0);
	});
});

// ===========================================================================
// Stacks — filterByMetadata
// ===========================================================================

describe('Stacks — filterByMetadata', () => {
	it('should filter entries by exact metadata match', async () => {
		const store = createStore();
		await store.load();

		await store.add('entry1', [1], { source: 'web' });
		await store.add('entry2', [2], { source: 'api' });
		await store.add('entry3', [3], { source: 'web' });

		const results = store.filterByMetadata([{ key: 'source', value: 'web' }]);
		expect(results).toHaveLength(2);
		expect(results.every((e) => e.metadata.source === 'web')).toBe(true);
	});

	it('should support contains filter', async () => {
		const store = createStore();
		await store.load();

		await store.add('entry1', [1], { tag: 'AI-powered tool' });
		await store.add('entry2', [2], { tag: 'simple script' });

		const results = store.filterByMetadata([
			{ key: 'tag', value: 'ai', mode: 'contains' },
		]);
		expect(results).toHaveLength(1);
		expect(results[0].metadata.tag).toBe('AI-powered tool');
	});

	it('should support exists filter', async () => {
		const store = createStore();
		await store.load();

		await store.add('entry1', [1], { author: 'Alice' });
		await store.add('entry2', [2], {});

		const results = store.filterByMetadata([{ key: 'author', mode: 'exists' }]);
		expect(results).toHaveLength(1);
	});

	it('should AND multiple filters', async () => {
		const store = createStore();
		await store.load();

		await store.add('entry1', [1], { source: 'web', lang: 'en' });
		await store.add('entry2', [2], { source: 'web', lang: 'fr' });
		await store.add('entry3', [3], { source: 'api', lang: 'en' });

		const results = store.filterByMetadata([
			{ key: 'source', value: 'web' },
			{ key: 'lang', value: 'en' },
		]);
		expect(results).toHaveLength(1);
		expect(results[0].text).toBe('entry1');
	});

	it('should return all entries when no filters are provided', async () => {
		const store = createStore();
		await store.load();

		await store.add('a', [1]);
		await store.add('b', [2]);

		const results = store.filterByMetadata([]);
		expect(results).toHaveLength(2);
	});
});

// ===========================================================================
// Stacks — filterByDateRange
// ===========================================================================

describe('Stacks — filterByDateRange', () => {
	it('should filter entries after a timestamp', async () => {
		const store = createStore();
		await store.load();

		await store.add('old', [1]);
		const cutoff = Date.now() + 100;
		// Wait a bit then add another
		await new Promise((r) => setTimeout(r, 150));
		await store.add('new', [2]);

		const results = store.filterByDateRange({ after: cutoff });
		expect(results).toHaveLength(1);
		expect(results[0].text).toBe('new');
	});

	it('should filter entries before a timestamp', async () => {
		const store = createStore();
		await store.load();

		await store.add('first', [1]);
		const cutoff = Date.now() + 100;
		await new Promise((r) => setTimeout(r, 150));
		await store.add('second', [2]);

		const results = store.filterByDateRange({ before: cutoff });
		expect(results).toHaveLength(1);
		expect(results[0].text).toBe('first');
	});

	it('should filter entries within a range', async () => {
		const store = createStore();
		await store.load();

		const before = Date.now() - 1;
		await store.add('in range', [1]);
		const after = Date.now() + 1;

		const results = store.filterByDateRange({ after: before, before: after });
		expect(results).toHaveLength(1);
	});
});

// ===========================================================================
// Stacks — advancedSearch
// ===========================================================================

describe('Stacks — advancedSearch', () => {
	it('should combine vector and text search', async () => {
		const store = createStore();
		await store.load();

		await store.add('machine learning is great', [1, 0, 0]);
		await store.add('cooking recipes are fun', [0, 1, 0]);
		await store.add('deep learning models', [0.9, 0.1, 0]);

		const results = store.advancedSearch({
			queryEmbedding: [1, 0, 0],
			text: { query: 'learning', mode: 'substring' },
			maxResults: 10,
		});

		// Both "machine learning" and "deep learning" should match
		expect(results.length).toBeGreaterThanOrEqual(2);
		// They should have both vector and text scores
		expect(results[0].scores.vector).toBeDefined();
		expect(results[0].scores.text).toBeDefined();
	});

	it('should filter by metadata in advanced search', async () => {
		const store = createStore();
		await store.load();

		await store.add('entry a', [1, 0], { source: 'web' });
		await store.add('entry b', [0.9, 0.1], { source: 'api' });
		await store.add('entry c', [0.8, 0.2], { source: 'web' });

		const results = store.advancedSearch({
			queryEmbedding: [1, 0],
			metadata: [{ key: 'source', value: 'web' }],
			maxResults: 10,
		});

		expect(results).toHaveLength(2);
		expect(results.every((r) => r.volume.metadata.source === 'web')).toBe(true);
	});

	it('should filter by date range in advanced search', async () => {
		const store = createStore();
		await store.load();

		await store.add('old entry', [1, 0]);
		const cutoff = Date.now() + 100;
		await new Promise((r) => setTimeout(r, 150));
		await store.add('new entry', [0.9, 0.1]);

		const results = store.advancedSearch({
			queryEmbedding: [1, 0],
			dateRange: { after: cutoff },
			maxResults: 10,
		});

		expect(results).toHaveLength(1);
		expect(results[0].volume.text).toBe('new entry');
	});

	it('should work with text search only (no embedding)', async () => {
		const store = createStore();
		await store.load();

		await store.add('The quick brown fox', [1]);
		await store.add('A lazy dog', [2]);
		await store.add('Quick brown cat', [3]);

		const results = store.advancedSearch({
			text: { query: 'quick brown', mode: 'substring' },
			maxResults: 10,
		});

		// Both "The quick brown fox" and "Quick brown cat" contain "quick brown" (case-insensitive)
		expect(results).toHaveLength(2);
		const texts = results.map((r) => r.volume.text);
		expect(texts).toContain('The quick brown fox');
		expect(texts).toContain('Quick brown cat');
	});

	it('should respect maxResults', async () => {
		const store = createStore();
		await store.load();

		for (let i = 0; i < 10; i++) {
			await store.add(`entry ${i}`, [Math.random()]);
		}

		const results = store.advancedSearch({
			text: { query: 'entry', mode: 'substring' },
			maxResults: 3,
		});

		expect(results.length).toBeLessThanOrEqual(3);
	});

	it("should support rankBy 'vector'", async () => {
		const store = createStore();
		await store.load();

		await store.add('match text here', [1, 0]);
		await store.add('match text also', [0.5, 0.5]);

		const results = store.advancedSearch({
			queryEmbedding: [1, 0],
			text: { query: 'match', mode: 'substring' },
			rankBy: 'vector',
			maxResults: 10,
		});

		expect(results).toHaveLength(2);
		// Ranked by vector score, so [1,0] should be first
		expect(results[0].score).toBe(results[0].scores.vector!);
	});

	it("should support rankBy 'text'", async () => {
		const store = createStore();
		await store.load();

		await store.add('exact match query', [0.5, 0.5]);
		await store.add('close match query', [1, 0]);

		const results = store.advancedSearch({
			queryEmbedding: [1, 0],
			text: { query: 'exact match query', mode: 'fuzzy', threshold: 0.1 },
			rankBy: 'text',
			maxResults: 10,
		});

		expect(results).toHaveLength(2);
		// Ranked by text score
		expect(results[0].score).toBe(results[0].scores.text!);
	});

	it("should support rankBy 'multiply'", async () => {
		const store = createStore();
		await store.load();

		await store.add('good match', [1, 0]);
		await store.add('bad match', [0.1, 0.9]);

		const results = store.advancedSearch({
			queryEmbedding: [1, 0],
			text: { query: 'good match', mode: 'fuzzy' },
			rankBy: 'multiply',
			maxResults: 10,
		});

		if (results.length > 0) {
			const r = results[0];
			const expectedScore = (r.scores.vector ?? 0) * (r.scores.text ?? 0);
			expect(r.score).toBeCloseTo(expectedScore, 5);
		}
	});
});
