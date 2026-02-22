import { afterEach, beforeEach, describe, expect, it } from 'bun:test';
import * as fsSync from 'node:fs';
import * as fs from 'node:fs/promises';
import * as os from 'node:os';
import * as path from 'node:path';
import {
	compressText,
	decompressText,
	encodeEmbedding,
	isGzipped,
} from '../src/ai/memory/compression.js';
import { cosineSimilarity } from '../src/ai/memory/cosine.js';
import {
	fuzzyScore,
	levenshteinDistance,
	levenshteinSimilarity,
	matchesAllMetadataFilters,
	matchesMetadataFilter,
	ngramSimilarity,
	tokenize,
	tokenOverlapScore,
} from '../src/ai/memory/text-search.js';
import type { VectorEntry } from '../src/ai/memory/types.js';
import type { VectorStore } from '../src/ai/memory/vector-store.js';
import {
	isMemoryError,
	isVectorStoreCorruptionError,
} from '../src/errors/index.js';
import { createLogger, type Logger } from '../src/logger.js';
import { expectGuardedThrow } from './utils/error-helpers';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function createSilentLogger(): Logger {
	return createLogger({ context: 'test', level: 'none', transports: [] });
}

let tmpDir: string;

async function createTmpDir(): Promise<string> {
	const dir = await fs.mkdtemp(path.join(os.tmpdir(), 'simse-test-'));
	return dir;
}

async function cleanupTmpDir(dir: string): Promise<void> {
	try {
		await fs.rm(dir, { recursive: true, force: true });
	} catch {
		// Ignore cleanup failures
	}
}

import { createVectorStore } from '../src/ai/memory/vector-store.js';

function createStore(
	storePath: string,
	options?: {
		autoSave?: boolean;
		flushIntervalMs?: number;
		atomicWrite?: boolean;
	},
): VectorStore {
	return createVectorStore(storePath, {
		logger: createSilentLogger(),
		autoSave: options?.autoSave ?? true,
		flushIntervalMs: options?.flushIntervalMs ?? 0,
		atomicWrite: options?.atomicWrite ?? true,
	});
}

function makeEmbedding(dim: number, fill: number = 0.5): number[] {
	return new Array(dim).fill(fill);
}

function makeEntry(overrides: Partial<VectorEntry> = {}): VectorEntry {
	return {
		id: overrides.id ?? 'test-id-1',
		text: overrides.text ?? 'test text',
		embedding: overrides.embedding ?? [0.1, 0.2, 0.3],
		metadata: overrides.metadata ?? {},
		timestamp: overrides.timestamp ?? Date.now(),
	};
}

/**
 * Write the directory-based store format (index.json + entries/*.md) for
 * pre-populating test stores.
 *
 * `storePath` is the store **directory**.  The function creates:
 *   - `storePath/index.json`  — gzip-compressed v2 `{ version: 2, entries }` with base64 embeddings
 *   - `storePath/entries/{id}.md` — one gzip-compressed markdown file per entry containing the text
 *
 * When `entries` contains full `VectorEntry` objects the text is split out
 * and the embedding is encoded to base64 automatically.
 */
async function writeStoreData(
	storePath: string,
	entries: Array<Record<string, unknown> | VectorEntry>,
): Promise<void> {
	const entriesDir = path.join(storePath, 'entries');
	await fs.mkdir(entriesDir, { recursive: true });

	const indexEntries: Array<Record<string, unknown>> = [];

	for (const entry of entries) {
		const plain: Record<string, unknown> = { ...entry };
		const { text, ...rest } = plain;

		// Encode number[] embeddings to base64 strings for v2 format
		if (Array.isArray(rest.embedding)) {
			rest.embedding = encodeEmbedding(rest.embedding as number[]);
		}

		indexEntries.push(rest);

		// Write gzip-compressed .md file when text is present
		if (typeof text === 'string' && typeof rest.id === 'string') {
			const mdPath = path.join(entriesDir, `${rest.id}.md`);
			await fs.writeFile(mdPath, compressText(text));
		}
	}

	const indexFile = { version: 2, entries: indexEntries };
	const compressed = compressText(JSON.stringify(indexFile));
	await fs.writeFile(path.join(storePath, 'index.json'), compressed);
}

/**
 * Read the persisted index.json back from a store directory.
 * Handles gzip-compressed v2 format and returns the entries array.
 */
async function readIndex(
	storePath: string,
): Promise<Array<Record<string, unknown>>> {
	const buf = await fs.readFile(path.join(storePath, 'index.json'));
	const jsonStr = isGzipped(buf) ? decompressText(buf) : buf.toString('utf-8');
	const parsed = JSON.parse(jsonStr) as Record<string, unknown>;
	return parsed.entries as Array<Record<string, unknown>>;
}

/**
 * Read a single entry's markdown content from the entries directory.
 * Handles gzip-compressed .md files.
 */
async function readEntryMd(storePath: string, id: string): Promise<string> {
	const buf = await fs.readFile(path.join(storePath, 'entries', `${id}.md`));
	return isGzipped(buf) ? decompressText(buf) : buf.toString('utf-8');
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
// VectorStore — Lifecycle
// ===========================================================================

describe('VectorStore — Lifecycle', () => {
	beforeEach(async () => {
		tmpDir = await createTmpDir();
	});

	afterEach(async () => {
		await cleanupTmpDir(tmpDir);
	});

	it('should start with zero entries when no file exists', async () => {
		const store = createStore(path.join(tmpDir, 'nonexistent'));
		await store.load();

		expect(store.size).toBe(0);
		expect(store.getAll()).toEqual([]);
	});

	it('should load existing entries from a valid store directory', async () => {
		const storePath = path.join(tmpDir, 'store');
		await writeStoreData(storePath, [
			makeEntry({ id: 'id-1', text: 'hello', embedding: [1, 2, 3] }),
			makeEntry({ id: 'id-2', text: 'world', embedding: [4, 5, 6] }),
		]);

		const store = createStore(storePath);
		await store.load();

		expect(store.size).toBe(2);
		const all = store.getAll();
		expect(all[0].id).toBe('id-1');
		expect(all[1].id).toBe('id-2');
	});

	it('should load an empty entries array from v2 index gracefully', async () => {
		const storePath = path.join(tmpDir, 'empty-array');
		await fs.mkdir(storePath, { recursive: true });
		const emptyIndex = JSON.stringify({ version: 2, entries: [] });
		await fs.writeFile(
			path.join(storePath, 'index.json'),
			compressText(emptyIndex),
		);

		const store = createStore(storePath);
		await store.load();

		expect(store.size).toBe(0);
	});

	it('should handle empty index file (no content) gracefully', async () => {
		const storePath = path.join(tmpDir, 'empty-file');
		await fs.mkdir(storePath, { recursive: true });
		await fs.writeFile(path.join(storePath, 'index.json'), '', 'utf-8');

		const store = createStore(storePath);
		await store.load();

		expect(store.size).toBe(0);
	});

	it('should handle whitespace-only index file gracefully', async () => {
		const storePath = path.join(tmpDir, 'whitespace');
		await fs.mkdir(storePath, { recursive: true });
		await fs.writeFile(
			path.join(storePath, 'index.json'),
			'   \n  \t  ',
			'utf-8',
		);

		const store = createStore(storePath);
		await store.load();

		expect(store.size).toBe(0);
	});

	it('should throw VectorStoreCorruptionError for malformed JSON', async () => {
		const storePath = path.join(tmpDir, 'corrupt');
		await fs.mkdir(storePath, { recursive: true });
		await fs.writeFile(
			path.join(storePath, 'index.json'),
			'{ not valid json }',
			'utf-8',
		);

		const store = createStore(storePath);
		await expectGuardedThrow(
			() => store.load(),
			isVectorStoreCorruptionError,
			'VECTOR_STORE_CORRUPT',
		);
	});

	it('should throw VectorStoreCorruptionError when root is not a valid IndexFile', async () => {
		const storePath = path.join(tmpDir, 'object');
		await fs.mkdir(storePath, { recursive: true });
		await fs.writeFile(
			path.join(storePath, 'index.json'),
			'{"key": "value"}',
			'utf-8',
		);

		const store = createStore(storePath);
		await expectGuardedThrow(
			() => store.load(),
			isVectorStoreCorruptionError,
			'VECTOR_STORE_CORRUPT',
		);
	});

	it('should skip invalid entries and warn (partial corruption)', async () => {
		const storePath = path.join(tmpDir, 'partial');
		// Write good entries with text, bad entries without valid structure
		await writeStoreData(storePath, [
			makeEntry({ id: 'good-1', text: 'valid' }),
			{ id: 'bad-1' }, // missing fields — no embedding/metadata/timestamp
			makeEntry({ id: 'good-2', text: 'also valid' }),
		]);
		// Also add invalid primitives directly to the compressed index
		const indexPath = path.join(storePath, 'index.json');
		const buf = await fs.readFile(indexPath);
		const jsonStr = isGzipped(buf)
			? decompressText(buf)
			: buf.toString('utf-8');
		const indexFile = JSON.parse(jsonStr) as {
			version: number;
			entries: unknown[];
		};
		indexFile.entries.push('not an object', null);
		await fs.writeFile(indexPath, compressText(JSON.stringify(indexFile)));

		const store = createStore(storePath);
		await store.load();

		expect(store.size).toBe(2);
		expect(store.getAll().map((e) => e.id)).toEqual(['good-1', 'good-2']);
	});

	it('should mark as dirty when invalid entries are skipped', async () => {
		const storePath = path.join(tmpDir, 'partial-dirty');
		await writeStoreData(storePath, [
			makeEntry({ id: 'good' }),
			{ id: 'bad' }, // invalid — missing required fields
		]);

		const store = createStore(storePath, { autoSave: false });
		await store.load();

		expect(store.isDirty).toBe(true);
	});

	it('should not be dirty after loading a clean store', async () => {
		const storePath = path.join(tmpDir, 'clean');
		await writeStoreData(storePath, [makeEntry({ id: 'clean' })]);

		const store = createStore(storePath, { autoSave: false });
		await store.load();

		expect(store.isDirty).toBe(false);
	});
});

// ===========================================================================
// VectorStore — Save
// ===========================================================================

describe('VectorStore — Save', () => {
	beforeEach(async () => {
		tmpDir = await createTmpDir();
	});

	afterEach(async () => {
		await cleanupTmpDir(tmpDir);
	});

	it('should persist entries to disk in compressed v2 format', async () => {
		const storePath = path.join(tmpDir, 'persist');
		const store = createStore(storePath);
		await store.load();

		await store.add('hello', [1, 2, 3], { key: 'val' });

		const data = await readIndex(storePath);
		expect(data).toHaveLength(1);
		const entry = data[0] as Record<string, unknown>;
		// Embedding is now a base64-encoded Float32Array string
		expect(typeof entry.embedding).toBe('string');
		expect(entry.metadata).toEqual({ key: 'val' });
		// text lives in the .md file, not the index
		expect(entry.text).toBeUndefined();
		const text = await readEntryMd(storePath, entry.id as string);
		expect(text).toBe('hello');
	});

	it("should create parent directories if they don't exist", async () => {
		const storePath = path.join(tmpDir, 'nested', 'deep', 'store');
		const store = createStore(storePath);
		await store.load();
		await store.add('test', [1], {});

		expect(fsSync.existsSync(path.join(storePath, 'index.json'))).toBe(true);
	});

	it('should not be dirty after save', async () => {
		const storePath = path.join(tmpDir, 'dirty-test');
		const store = createStore(storePath, { autoSave: false });
		await store.load();

		await store.add('test', [1], {});
		expect(store.isDirty).toBe(true);

		await store.save();
		expect(store.isDirty).toBe(false);
	});

	it('should use atomic write by default (tmp + rename)', async () => {
		const storePath = path.join(tmpDir, 'atomic');
		const store = createStore(storePath, { atomicWrite: true });
		await store.load();

		await store.add('entry', [0.5], {});

		const indexPath = path.join(storePath, 'index.json');
		// The final file should exist and the tmp file should not
		expect(fsSync.existsSync(indexPath)).toBe(true);
		expect(fsSync.existsSync(`${indexPath}.tmp`)).toBe(false);
	});

	it('should write directly when atomicWrite is false', async () => {
		const storePath = path.join(tmpDir, 'non-atomic');
		const store = createStore(storePath, { atomicWrite: false });
		await store.load();

		await store.add('entry', [0.5], {});

		expect(fsSync.existsSync(path.join(storePath, 'index.json'))).toBe(true);
		const data = await readIndex(storePath);
		expect(data).toHaveLength(1);
	});

	it('should roundtrip data correctly (save then load)', async () => {
		const storePath = path.join(tmpDir, 'roundtrip');

		// Write
		const store1 = createStore(storePath);
		await store1.load();
		await store1.add('text1', [0.1, 0.2], { a: '1' });
		await store1.add('text2', [0.3, 0.4], { b: '2' });

		// Read in a new store instance
		const store2 = createStore(storePath);
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
// VectorStore — CRUD: add
// ===========================================================================

describe('VectorStore — add', () => {
	beforeEach(async () => {
		tmpDir = await createTmpDir();
	});

	afterEach(async () => {
		await cleanupTmpDir(tmpDir);
	});

	it('should add an entry and return a UUID', async () => {
		const store = createStore(path.join(tmpDir, 'add'));
		await store.load();

		const id = await store.add('hello world', [1, 2, 3]);

		expect(id).toBeDefined();
		expect(typeof id).toBe('string');
		expect(id.length).toBeGreaterThan(0);
		expect(store.size).toBe(1);
	});

	it('should generate unique IDs for each entry', async () => {
		const store = createStore(path.join(tmpDir, 'unique-ids'));
		await store.load();

		const id1 = await store.add('a', [1]);
		const id2 = await store.add('b', [2]);
		const id3 = await store.add('c', [3]);

		expect(new Set([id1, id2, id3]).size).toBe(3);
	});

	it('should store text, embedding, and metadata correctly', async () => {
		const store = createStore(path.join(tmpDir, 'store-fields'));
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
		const store = createStore(path.join(tmpDir, 'default-meta'));
		await store.load();

		const id = await store.add('text', [1]);
		const entry = store.getById(id);
		expect(entry?.metadata).toEqual({});
	});

	it('should throw MemoryError for empty text', async () => {
		const store = createStore(path.join(tmpDir, 'empty-text'));
		await store.load();

		await expectGuardedThrow(
			() => store.add('', [0.1, 0.2, 0.3]),
			isMemoryError,
			'VECTOR_STORE_EMPTY_TEXT',
		);
	});

	it('should throw MemoryError for empty embedding', async () => {
		const store = createStore(path.join(tmpDir, 'empty-emb'));
		await store.load();

		await expectGuardedThrow(
			() => store.add('some text', []),
			isMemoryError,
			'VECTOR_STORE_EMPTY_EMBEDDING',
		);
	});

	it('should increment size with each add', async () => {
		const store = createStore(path.join(tmpDir, 'size'));
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
		const storePath = path.join(tmpDir, 'auto-save');
		const store = createStore(storePath, { autoSave: true });
		await store.load();

		await store.add('entry', [1]);

		expect(fsSync.existsSync(path.join(storePath, 'index.json'))).toBe(true);
		const data = await readIndex(storePath);
		expect(data).toHaveLength(1);
	});

	it('should NOT auto-save when autoSave is false', async () => {
		const storePath = path.join(tmpDir, 'no-auto');
		const store = createStore(storePath, {
			autoSave: false,
			flushIntervalMs: 0,
		});
		await store.load();

		await store.add('entry', [1]);

		expect(fsSync.existsSync(path.join(storePath, 'index.json'))).toBe(false);
		expect(store.isDirty).toBe(true);
	});
});

// ===========================================================================
// VectorStore — CRUD: addBatch
// ===========================================================================

describe('VectorStore — addBatch', () => {
	beforeEach(async () => {
		tmpDir = await createTmpDir();
	});

	afterEach(async () => {
		await cleanupTmpDir(tmpDir);
	});

	it('should add multiple entries at once', async () => {
		const store = createStore(path.join(tmpDir, 'batch'));
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
		const store = createStore(path.join(tmpDir, 'empty-batch'));
		await store.load();

		const ids = await store.addBatch([]);
		expect(ids).toEqual([]);
		expect(store.size).toBe(0);
	});

	it('should throw MemoryError if any entry has empty text', async () => {
		const store = createStore(path.join(tmpDir, 'bad-batch'));
		await store.load();

		await expectGuardedThrow(
			() =>
				store.addBatch([
					{ text: 'ok', embedding: [1] },
					{ text: '', embedding: [2] },
				]),
			isMemoryError,
			'VECTOR_STORE_EMPTY_TEXT',
		);
	});

	it('should throw MemoryError if any entry has empty embedding', async () => {
		const store = createStore(path.join(tmpDir, 'bad-emb-batch'));
		await store.load();

		await expectGuardedThrow(
			() =>
				store.addBatch([
					{ text: 'ok', embedding: [1] },
					{ text: 'bad', embedding: [] },
				]),
			isMemoryError,
			'VECTOR_STORE_EMPTY_EMBEDDING',
		);
	});

	it('should save only once for the entire batch (autoSave)', async () => {
		const storePath = path.join(tmpDir, 'batch-save');
		const store = createStore(storePath, { autoSave: true });
		await store.load();

		await store.addBatch([
			{ text: 'a', embedding: [1] },
			{ text: 'b', embedding: [2] },
		]);

		const data = await readIndex(storePath);
		expect(data).toHaveLength(2);
	});

	it('should default metadata to empty object when not provided', async () => {
		const store = createStore(path.join(tmpDir, 'batch-meta'));
		await store.load();

		const ids = await store.addBatch([{ text: 'no meta', embedding: [1] }]);
		const entry = store.getById(ids[0]);
		expect(entry?.metadata).toEqual({});
	});
});

// ===========================================================================
// VectorStore — CRUD: delete
// ===========================================================================

describe('VectorStore — delete', () => {
	beforeEach(async () => {
		tmpDir = await createTmpDir();
	});

	afterEach(async () => {
		await cleanupTmpDir(tmpDir);
	});

	it('should delete an existing entry and return true', async () => {
		const store = createStore(path.join(tmpDir, 'delete'));
		await store.load();

		const id = await store.add('to delete', [1, 2]);
		expect(store.size).toBe(1);

		const result = await store.delete(id);
		expect(result).toBe(true);
		expect(store.size).toBe(0);
	});

	it('should return false for non-existent ID', async () => {
		const store = createStore(path.join(tmpDir, 'no-delete'));
		await store.load();

		const result = await store.delete('non-existent-id');
		expect(result).toBe(false);
	});

	it('should only delete the specified entry', async () => {
		const store = createStore(path.join(tmpDir, 'selective-delete'));
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
		const store = createStore(path.join(tmpDir, 'no-mod'), {
			autoSave: false,
		});
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
// VectorStore — CRUD: deleteBatch
// ===========================================================================

describe('VectorStore — deleteBatch', () => {
	beforeEach(async () => {
		tmpDir = await createTmpDir();
	});

	afterEach(async () => {
		await cleanupTmpDir(tmpDir);
	});

	it('should delete multiple entries by ID', async () => {
		const store = createStore(path.join(tmpDir, 'batch-del'));
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
		const store = createStore(path.join(tmpDir, 'empty-del'));
		await store.load();

		const deleted = await store.deleteBatch([]);
		expect(deleted).toBe(0);
	});

	it('should handle mixed existing and non-existing IDs', async () => {
		const store = createStore(path.join(tmpDir, 'mixed-del'));
		await store.load();

		const id1 = await store.add('a', [1]);
		await store.add('b', [2]);

		const deleted = await store.deleteBatch([id1, 'non-existent']);
		expect(deleted).toBe(1);
		expect(store.size).toBe(1);
	});

	it('should handle all non-existing IDs', async () => {
		const store = createStore(path.join(tmpDir, 'none-del'));
		await store.load();

		await store.add('a', [1]);

		const deleted = await store.deleteBatch(['fake-1', 'fake-2']);
		expect(deleted).toBe(0);
		expect(store.size).toBe(1);
	});
});

// ===========================================================================
// VectorStore — CRUD: clear
// ===========================================================================

describe('VectorStore — clear', () => {
	beforeEach(async () => {
		tmpDir = await createTmpDir();
	});

	afterEach(async () => {
		await cleanupTmpDir(tmpDir);
	});

	it('should remove all entries', async () => {
		const store = createStore(path.join(tmpDir, 'clear'));
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
		const store = createStore(path.join(tmpDir, 'clear-empty'));
		await store.load();

		await store.clear();
		expect(store.size).toBe(0);
	});

	it('should persist the empty state when autoSave is on', async () => {
		const storePath = path.join(tmpDir, 'clear-persist');
		const store = createStore(storePath);
		await store.load();

		await store.add('a', [1]);
		await store.clear();

		const entries = await readIndex(storePath);
		expect(entries).toEqual([]);
	});
});

// ===========================================================================
// VectorStore — Search
// ===========================================================================

describe('VectorStore — search', () => {
	beforeEach(async () => {
		tmpDir = await createTmpDir();
	});

	afterEach(async () => {
		await cleanupTmpDir(tmpDir);
	});

	it('should find the most similar entry', async () => {
		const store = createStore(path.join(tmpDir, 'search'));
		await store.load();

		await store.add('exact match', [1, 0, 0]);
		await store.add('partial match', [0.7, 0.7, 0]);
		await store.add('no match', [0, 0, 1]);

		const results = store.search([1, 0, 0], 10, 0);

		expect(results.length).toBeGreaterThan(0);
		expect(results[0].entry.text).toBe('exact match');
		expect(results[0].score).toBeCloseTo(1.0, 5);
	});

	it('should return results sorted by descending score', async () => {
		const store = createStore(path.join(tmpDir, 'sorted'));
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
		const store = createStore(path.join(tmpDir, 'max'));
		await store.load();

		for (let i = 0; i < 10; i++) {
			await store.add(`entry ${i}`, [Math.random(), Math.random()]);
		}

		const results = store.search([0.5, 0.5], 3, 0);
		expect(results.length).toBeLessThanOrEqual(3);
	});

	it('should respect the threshold', async () => {
		const store = createStore(path.join(tmpDir, 'threshold'));
		await store.load();

		await store.add('similar', [1, 0, 0]); // cosine = 1.0 with query
		await store.add('different', [0, 1, 0]); // cosine = 0.0 with query
		await store.add('opposite', [-1, 0, 0]); // cosine = -1.0 with query

		const results = store.search([1, 0, 0], 10, 0.5);

		expect(results).toHaveLength(1);
		expect(results[0].entry.text).toBe('similar');
	});

	it('should return empty array when no entries meet the threshold', async () => {
		const store = createStore(path.join(tmpDir, 'no-match'));
		await store.load();

		await store.add('entry', [0, 1, 0]);

		const results = store.search([1, 0, 0], 10, 0.99);
		expect(results).toEqual([]);
	});

	it('should return empty array from an empty store', async () => {
		const store = createStore(path.join(tmpDir, 'empty-search'));
		await store.load();

		const results = store.search([1, 0, 0], 10, 0);
		expect(results).toEqual([]);
	});

	it('should return empty array for empty query embedding', async () => {
		const store = createStore(path.join(tmpDir, 'empty-query'));
		await store.load();

		await store.add('entry', [1, 2, 3]);

		const results = store.search([], 10, 0);
		expect(results).toEqual([]);
	});

	it('should skip entries with mismatched embedding dimensions', async () => {
		const storePath = path.join(tmpDir, 'mismatch');

		// Write entries with different dimensions directly
		await writeStoreData(storePath, [
			makeEntry({ id: 'dim3', text: '3d', embedding: [1, 0, 0] }),
			makeEntry({ id: 'dim2', text: '2d', embedding: [1, 0] }),
			makeEntry({ id: 'dim3b', text: '3d-b', embedding: [0.5, 0.5, 0] }),
		]);

		const store = createStore(storePath);
		await store.load();

		const results = store.search([1, 0, 0], 10, 0);
		// Should only include the two 3D entries
		expect(results).toHaveLength(2);
		expect(results.map((r) => r.entry.id).sort()).toEqual(['dim3', 'dim3b']);
	});

	it('should return correct score values', async () => {
		const store = createStore(path.join(tmpDir, 'scores'));
		await store.load();

		await store.add('identical', [1, 0, 0]);

		const results = store.search([1, 0, 0], 1, 0);

		expect(results).toHaveLength(1);
		expect(results[0].score).toBeCloseTo(1.0, 10);
	});

	it('should handle threshold of exactly 0', async () => {
		const store = createStore(path.join(tmpDir, 'zero-threshold'));
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
		const store = createStore(path.join(tmpDir, 'exact-threshold'));
		await store.load();

		await store.add('exact', [1, 0, 0]);
		await store.add('close', [0.999, 0.001, 0]);
		await store.add('far', [0, 1, 0]);

		const results = store.search([1, 0, 0], 10, 1.0);
		// Only the exact match should pass
		expect(results).toHaveLength(1);
		expect(results[0].entry.text).toBe('exact');
	});
});

// ===========================================================================
// VectorStore — Accessors
// ===========================================================================

describe('VectorStore — Accessors', () => {
	beforeEach(async () => {
		tmpDir = await createTmpDir();
	});

	afterEach(async () => {
		await cleanupTmpDir(tmpDir);
	});

	describe('getAll', () => {
		it('should return a shallow copy of entries', async () => {
			const store = createStore(path.join(tmpDir, 'getall'));
			await store.load();

			await store.add('a', [1]);
			await store.add('b', [2]);

			const all1 = store.getAll();
			const all2 = store.getAll();

			expect(all1).toEqual(all2);
			expect(all1).not.toBe(all2); // Different array instances
		});

		it('should return empty array for empty store', async () => {
			const store = createStore(path.join(tmpDir, 'empty-getall'));
			await store.load();

			expect(store.getAll()).toEqual([]);
		});
	});

	describe('getById', () => {
		it('should find entry by ID', async () => {
			const store = createStore(path.join(tmpDir, 'getbyid'));
			await store.load();

			const id = await store.add('findme', [1, 2, 3], { tag: 'test' });

			const entry = store.getById(id);
			expect(entry).toBeDefined();
			expect(entry?.text).toBe('findme');
			expect(entry?.metadata).toEqual({ tag: 'test' });
		});

		it('should return undefined for non-existent ID', async () => {
			const store = createStore(path.join(tmpDir, 'nobyid'));
			await store.load();

			expect(store.getById('does-not-exist')).toBeUndefined();
		});
	});

	describe('size', () => {
		it('should reflect current entry count', async () => {
			const store = createStore(path.join(tmpDir, 'size-acc'));
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
			const store = createStore(path.join(tmpDir, 'dirty1'), {
				autoSave: false,
			});
			await store.load();
			expect(store.isDirty).toBe(false);
		});

		it('should be true after add (no autoSave)', async () => {
			const store = createStore(path.join(tmpDir, 'dirty2'), {
				autoSave: false,
			});
			await store.load();

			await store.add('x', [1]);
			expect(store.isDirty).toBe(true);
		});

		it('should be false after save', async () => {
			const store = createStore(path.join(tmpDir, 'dirty3'), {
				autoSave: false,
			});
			await store.load();

			await store.add('x', [1]);
			expect(store.isDirty).toBe(true);

			await store.save();
			expect(store.isDirty).toBe(false);
		});

		it('should be true after delete (no autoSave)', async () => {
			const store = createStore(path.join(tmpDir, 'dirty4'), {
				autoSave: false,
			});
			await store.load();

			await store.add('x', [1]);
			await store.save();
			expect(store.isDirty).toBe(false);

			const all = store.getAll();
			await store.delete(all[0].id);
			expect(store.isDirty).toBe(true);
		});

		it('should be true after clear (no autoSave)', async () => {
			const store = createStore(path.join(tmpDir, 'dirty5'), {
				autoSave: false,
			});
			await store.load();

			await store.add('x', [1]);
			await store.save();

			await store.clear();
			expect(store.isDirty).toBe(true);
		});
	});
});

// ===========================================================================
// VectorStore — dispose
// ===========================================================================

describe('VectorStore — dispose', () => {
	beforeEach(async () => {
		tmpDir = await createTmpDir();
	});

	afterEach(async () => {
		await cleanupTmpDir(tmpDir);
	});

	it('should flush dirty data on dispose', async () => {
		const storePath = path.join(tmpDir, 'dispose');
		const store = createStore(storePath, {
			autoSave: false,
			flushIntervalMs: 0,
		});
		await store.load();

		await store.add('will persist', [1, 2]);
		expect(store.isDirty).toBe(true);

		await store.dispose();
		expect(store.isDirty).toBe(false);

		// Data should be on disk — index has the entry, text is in .md
		const data = await readIndex(storePath);
		expect(data).toHaveLength(1);
		const entry = data[0] as Record<string, unknown>;
		const text = await readEntryMd(storePath, entry.id as string);
		expect(text).toBe('will persist');
	});

	it('should not throw when disposing a clean store', async () => {
		const store = createStore(path.join(tmpDir, 'clean-dispose'), {
			autoSave: false,
		});
		await store.load();

		await store.dispose();
	});

	it('should not throw when disposing twice', async () => {
		const store = createStore(path.join(tmpDir, 'double-dispose'), {
			autoSave: false,
		});
		await store.load();

		await store.add('x', [1]);

		await store.dispose();
		await store.dispose();
	});
});

// ===========================================================================
// VectorStore — Entry validation (on load)
// ===========================================================================

describe('VectorStore — Entry validation', () => {
	beforeEach(async () => {
		tmpDir = await createTmpDir();
	});

	afterEach(async () => {
		await cleanupTmpDir(tmpDir);
	});

	it('should reject entries with missing id', async () => {
		const storePath = path.join(tmpDir, 'no-id');
		await writeStoreData(storePath, [
			{ embedding: [1], metadata: {}, timestamp: 1 },
		]);

		const store = createStore(storePath);
		await store.load();
		expect(store.size).toBe(0);
	});

	it('should reject entries with empty id', async () => {
		const storePath = path.join(tmpDir, 'empty-id');
		await writeStoreData(storePath, [
			{
				id: '',
				text: 'empty id',
				embedding: [1],
				metadata: {},
				timestamp: 1,
			},
		]);

		const store = createStore(storePath);
		await store.load();
		expect(store.size).toBe(0);
	});

	it('should reject entries with empty embedding string', async () => {
		const storePath = path.join(tmpDir, 'empty-emb-str');
		// Write a v2 index directly with an empty embedding string
		const entriesDir = path.join(storePath, 'entries');
		await fs.mkdir(entriesDir, { recursive: true });
		await fs.writeFile(path.join(entriesDir, 'x.md'), compressText('t'));
		const indexFile = {
			version: 2,
			entries: [{ id: 'x', embedding: '', metadata: {}, timestamp: 1 }],
		};
		await fs.writeFile(
			path.join(storePath, 'index.json'),
			compressText(JSON.stringify(indexFile)),
		);

		const store = createStore(storePath);
		await store.load();
		expect(store.size).toBe(0);
	});

	it('should reject entries with non-string embedding (number)', async () => {
		const storePath = path.join(tmpDir, 'num-emb');
		// Write a v2 index directly with a numeric embedding (invalid for v2)
		const entriesDir = path.join(storePath, 'entries');
		await fs.mkdir(entriesDir, { recursive: true });
		await fs.writeFile(path.join(entriesDir, 'x.md'), compressText('t'));
		const indexFile = {
			version: 2,
			entries: [{ id: 'x', embedding: 42, metadata: {}, timestamp: 1 }],
		};
		await fs.writeFile(
			path.join(storePath, 'index.json'),
			compressText(JSON.stringify(indexFile)),
		);

		const store = createStore(storePath);
		await store.load();
		expect(store.size).toBe(0);
	});

	it('should reject entries with non-object metadata', async () => {
		const storePath = path.join(tmpDir, 'non-obj-meta');
		await writeStoreData(storePath, [
			{
				id: 'x',
				text: 't',
				embedding: [1],
				metadata: 'string',
				timestamp: 1,
			},
		]);

		const store = createStore(storePath);
		await store.load();
		expect(store.size).toBe(0);
	});

	it('should reject entries with null metadata', async () => {
		const storePath = path.join(tmpDir, 'null-meta');
		await writeStoreData(storePath, [
			{ id: 'x', text: 't', embedding: [1], metadata: null, timestamp: 1 },
		]);

		const store = createStore(storePath);
		await store.load();
		expect(store.size).toBe(0);
	});

	it('should reject entries with non-number timestamp', async () => {
		const storePath = path.join(tmpDir, 'non-num-ts');
		await writeStoreData(storePath, [
			{
				id: 'x',
				text: 't',
				embedding: [1],
				metadata: {},
				timestamp: '2024-01-01',
			},
		]);

		const store = createStore(storePath);
		await store.load();
		expect(store.size).toBe(0);
	});

	it('should reject null entries in the array', async () => {
		const storePath = path.join(tmpDir, 'null-entry');
		await writeStoreData(storePath, [makeEntry({ id: 'valid' })]);
		// Manually inject a null into the compressed index
		const indexPath = path.join(storePath, 'index.json');
		const buf = await fs.readFile(indexPath);
		const jsonStr = isGzipped(buf)
			? decompressText(buf)
			: buf.toString('utf-8');
		const indexFile = JSON.parse(jsonStr) as {
			version: number;
			entries: unknown[];
		};
		indexFile.entries.unshift(null);
		await fs.writeFile(indexPath, compressText(JSON.stringify(indexFile)));

		const store = createStore(storePath);
		await store.load();
		expect(store.size).toBe(1);
	});

	it('should reject primitive entries in the array', async () => {
		const storePath = path.join(tmpDir, 'prim-entry');
		await writeStoreData(storePath, [makeEntry({ id: 'valid' })]);
		// Manually inject primitives into the compressed index
		const indexPath = path.join(storePath, 'index.json');
		const buf = await fs.readFile(indexPath);
		const jsonStr = isGzipped(buf)
			? decompressText(buf)
			: buf.toString('utf-8');
		const indexFile = JSON.parse(jsonStr) as {
			version: number;
			entries: unknown[];
		};
		indexFile.entries.unshift(42, 'string', true);
		await fs.writeFile(indexPath, compressText(JSON.stringify(indexFile)));

		const store = createStore(storePath);
		await store.load();
		expect(store.size).toBe(1);
	});

	it('should accept valid entries alongside invalid ones', async () => {
		const storePath = path.join(tmpDir, 'mixed-validity');
		await writeStoreData(storePath, [
			makeEntry({ id: 'good-1', text: 'valid 1' }),
			makeEntry({ id: 'good-2', text: 'valid 2' }),
			makeEntry({ id: 'good-3', text: 'valid 3' }),
		]);
		// Inject invalid entries into the compressed index
		const indexPath = path.join(storePath, 'index.json');
		const buf = await fs.readFile(indexPath);
		const jsonStr = isGzipped(buf)
			? decompressText(buf)
			: buf.toString('utf-8');
		const indexFile = JSON.parse(jsonStr) as {
			version: number;
			entries: unknown[];
		};
		indexFile.entries.splice(1, 0, { id: 'bad-1' }); // missing fields
		indexFile.entries.splice(3, 0, {}); // empty object
		await fs.writeFile(indexPath, compressText(JSON.stringify(indexFile)));

		const store = createStore(storePath);
		await store.load();

		expect(store.size).toBe(3);
		expect(store.getAll().map((e) => e.id)).toEqual([
			'good-1',
			'good-2',
			'good-3',
		]);
	});
});

// ===========================================================================
// VectorStore — Concurrency / ordering
// ===========================================================================

describe('VectorStore — Ordering', () => {
	beforeEach(async () => {
		tmpDir = await createTmpDir();
	});

	afterEach(async () => {
		await cleanupTmpDir(tmpDir);
	});

	it('should maintain insertion order in getAll', async () => {
		const store = createStore(path.join(tmpDir, 'order'));
		await store.load();

		await store.add('first', [1]);
		await store.add('second', [2]);
		await store.add('third', [3]);

		const all = store.getAll();
		expect(all.map((e) => e.text)).toEqual(['first', 'second', 'third']);
	});

	it('should maintain order after deleting middle entries', async () => {
		const store = createStore(path.join(tmpDir, 'order-del'));
		await store.load();

		await store.add('a', [1]);
		const idB = await store.add('b', [2]);
		await store.add('c', [3]);

		await store.delete(idB);

		const all = store.getAll();
		expect(all.map((e) => e.text)).toEqual(['a', 'c']);
	});

	it('should handle rapid sequential operations', async () => {
		const store = createStore(path.join(tmpDir, 'rapid'));
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
// VectorStore — Edge cases
// ===========================================================================

describe('VectorStore — Edge cases', () => {
	beforeEach(async () => {
		tmpDir = await createTmpDir();
	});

	afterEach(async () => {
		await cleanupTmpDir(tmpDir);
	});

	it('should handle entries with very long text', async () => {
		const store = createStore(path.join(tmpDir, 'long-text'));
		await store.load();

		const longText = 'x'.repeat(100_000);
		const id = await store.add(longText, [1]);

		const entry = store.getById(id);
		expect(entry?.text.length).toBe(100_000);
	});

	it('should handle entries with high-dimensional embeddings', async () => {
		const store = createStore(path.join(tmpDir, 'high-dim'));
		await store.load();

		const embedding = makeEmbedding(2048, 0.1);
		const id = await store.add('high dim', embedding);

		const entry = store.getById(id);
		expect(entry?.embedding.length).toBe(2048);
	});

	it('should handle entries with many metadata keys', async () => {
		const store = createStore(path.join(tmpDir, 'many-meta'));
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
		const store = createStore(path.join(tmpDir, 'max0'));
		await store.load();

		await store.add('a', [1, 0, 0]);

		const results = store.search([1, 0, 0], 0, 0);
		expect(results).toEqual([]);
	});

	it('should handle search when maxResults exceeds entry count', async () => {
		const store = createStore(path.join(tmpDir, 'max-exceed'));
		await store.load();

		await store.add('a', [1, 0]);
		await store.add('b', [0, 1]);

		const results = store.search([1, 0], 100, 0);
		expect(results.length).toBeLessThanOrEqual(2);
	});

	it('should handle Unicode text correctly', async () => {
		const store = createStore(path.join(tmpDir, 'unicode'));
		await store.load();

		const unicodeText = 'こんにちは世界 🌍 Ñoño café über résumé';
		const id = await store.add(unicodeText, [1, 2, 3]);

		// Roundtrip through save/load
		const store2 = createStore(path.join(tmpDir, 'unicode'));
		await store2.load();

		const entry = store2.getById(id);
		expect(entry?.text).toBe(unicodeText);
	});

	it('should handle entries with negative embedding values', async () => {
		const store = createStore(path.join(tmpDir, 'negative'));
		await store.load();

		await store.add('negative', [-0.5, -0.3, -0.1]);

		const results = store.search([-0.5, -0.3, -0.1], 1, 0);
		expect(results).toHaveLength(1);
		expect(results[0].score).toBeCloseTo(1.0, 5);
	});

	it('should handle entries with very small embedding values', async () => {
		const store = createStore(path.join(tmpDir, 'small'));
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
// VectorStore — textSearch
// ===========================================================================

describe('VectorStore — textSearch', () => {
	beforeEach(async () => {
		tmpDir = await createTmpDir();
	});

	afterEach(async () => {
		await cleanupTmpDir(tmpDir);
	});

	it('should find entries by fuzzy match (default mode)', async () => {
		const store = createStore(path.join(tmpDir, 'fuzzy'));
		await store.load();

		await store.add('The quick brown fox jumps', [1]);
		await store.add('A slow red dog sits', [2]);
		await store.add('The quick brown cat leaps', [3]);

		const results = store.textSearch({ query: 'quick brown fox' });
		expect(results.length).toBeGreaterThanOrEqual(1);
		expect(results[0].entry.text).toContain('quick brown fox');
	});

	it('should find entries by substring match', async () => {
		const store = createStore(path.join(tmpDir, 'substr'));
		await store.load();

		await store.add('Hello World', [1]);
		await store.add('Goodbye World', [2]);
		await store.add('Hello There', [3]);

		const results = store.textSearch({ query: 'hello', mode: 'substring' });
		expect(results).toHaveLength(2);
	});

	it('should find entries by exact match', async () => {
		const store = createStore(path.join(tmpDir, 'exact'));
		await store.load();

		await store.add('Hello World', [1]);
		await store.add('hello world', [2]);

		const results = store.textSearch({ query: 'Hello World', mode: 'exact' });
		expect(results).toHaveLength(1);
		expect(results[0].entry.text).toBe('Hello World');
	});

	it('should find entries by regex match', async () => {
		const store = createStore(path.join(tmpDir, 'regex'));
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
		const store = createStore(path.join(tmpDir, 'token'));
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
		expect(results[0].entry.text).toContain('learning algorithms');
	});

	it('should return empty array for empty query', async () => {
		const store = createStore(path.join(tmpDir, 'empty-q'));
		await store.load();
		await store.add('something', [1]);

		const results = store.textSearch({ query: '' });
		expect(results).toEqual([]);
	});

	it('should respect the threshold', async () => {
		const store = createStore(path.join(tmpDir, 'thresh'));
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
// VectorStore — filterByMetadata
// ===========================================================================

describe('VectorStore — filterByMetadata', () => {
	beforeEach(async () => {
		tmpDir = await createTmpDir();
	});

	afterEach(async () => {
		await cleanupTmpDir(tmpDir);
	});

	it('should filter entries by exact metadata match', async () => {
		const store = createStore(path.join(tmpDir, 'meta-eq'));
		await store.load();

		await store.add('entry1', [1], { source: 'web' });
		await store.add('entry2', [2], { source: 'api' });
		await store.add('entry3', [3], { source: 'web' });

		const results = store.filterByMetadata([{ key: 'source', value: 'web' }]);
		expect(results).toHaveLength(2);
		expect(results.every((e) => e.metadata.source === 'web')).toBe(true);
	});

	it('should support contains filter', async () => {
		const store = createStore(path.join(tmpDir, 'meta-contains'));
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
		const store = createStore(path.join(tmpDir, 'meta-exists'));
		await store.load();

		await store.add('entry1', [1], { author: 'Alice' });
		await store.add('entry2', [2], {});

		const results = store.filterByMetadata([{ key: 'author', mode: 'exists' }]);
		expect(results).toHaveLength(1);
	});

	it('should AND multiple filters', async () => {
		const store = createStore(path.join(tmpDir, 'meta-and'));
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
		const store = createStore(path.join(tmpDir, 'meta-none'));
		await store.load();

		await store.add('a', [1]);
		await store.add('b', [2]);

		const results = store.filterByMetadata([]);
		expect(results).toHaveLength(2);
	});
});

// ===========================================================================
// VectorStore — filterByDateRange
// ===========================================================================

describe('VectorStore — filterByDateRange', () => {
	beforeEach(async () => {
		tmpDir = await createTmpDir();
	});

	afterEach(async () => {
		await cleanupTmpDir(tmpDir);
	});

	it('should filter entries after a timestamp', async () => {
		const store = createStore(path.join(tmpDir, 'date-after'));
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
		const store = createStore(path.join(tmpDir, 'date-before'));
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
		const store = createStore(path.join(tmpDir, 'date-range'));
		await store.load();

		const before = Date.now() - 1;
		await store.add('in range', [1]);
		const after = Date.now() + 1;

		const results = store.filterByDateRange({ after: before, before: after });
		expect(results).toHaveLength(1);
	});
});

// ===========================================================================
// VectorStore — advancedSearch
// ===========================================================================

describe('VectorStore — advancedSearch', () => {
	beforeEach(async () => {
		tmpDir = await createTmpDir();
	});

	afterEach(async () => {
		await cleanupTmpDir(tmpDir);
	});

	it('should combine vector and text search', async () => {
		const store = createStore(path.join(tmpDir, 'adv-combo'));
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
		const store = createStore(path.join(tmpDir, 'adv-meta'));
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
		expect(results.every((r) => r.entry.metadata.source === 'web')).toBe(true);
	});

	it('should filter by date range in advanced search', async () => {
		const store = createStore(path.join(tmpDir, 'adv-date'));
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
		expect(results[0].entry.text).toBe('new entry');
	});

	it('should work with text search only (no embedding)', async () => {
		const store = createStore(path.join(tmpDir, 'adv-text-only'));
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
		const texts = results.map((r) => r.entry.text);
		expect(texts).toContain('The quick brown fox');
		expect(texts).toContain('Quick brown cat');
	});

	it('should respect maxResults', async () => {
		const store = createStore(path.join(tmpDir, 'adv-max'));
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
		const store = createStore(path.join(tmpDir, 'rank-vector'));
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
		const store = createStore(path.join(tmpDir, 'rank-text'));
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
		const store = createStore(path.join(tmpDir, 'rank-mult'));
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
