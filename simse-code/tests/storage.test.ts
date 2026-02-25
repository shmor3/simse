import { afterAll, describe, expect, it } from 'bun:test';
import { existsSync, mkdirSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { createFileStorageBackend } from '../storage.js';

const TEST_DIR = join(import.meta.dirname, '.test-storage-tmp');
const TEST_FILE = join(TEST_DIR, 'test.simk');

// Clean up before and after
if (existsSync(TEST_DIR)) rmSync(TEST_DIR, { recursive: true, force: true });
mkdirSync(TEST_DIR, { recursive: true });

afterAll(() => {
	if (existsSync(TEST_DIR)) rmSync(TEST_DIR, { recursive: true, force: true });
});

describe('createFileStorageBackend', () => {
	it('should return a frozen object', () => {
		const backend = createFileStorageBackend({ path: TEST_FILE });
		expect(Object.isFrozen(backend)).toBe(true);
	});

	it('should load an empty Map when file does not exist', async () => {
		const path = join(TEST_DIR, 'nonexistent.simk');
		const backend = createFileStorageBackend({ path });
		const data = await backend.load();
		expect(data.size).toBe(0);
	});

	it('should save and load data round-trip', async () => {
		const path = join(TEST_DIR, 'roundtrip.simk');
		const backend = createFileStorageBackend({ path });

		const data = new Map<string, Buffer>();
		data.set('key1', Buffer.from('value1'));
		data.set('key2', Buffer.from('value2'));
		data.set('key3', Buffer.from([0x00, 0x01, 0x02, 0xff]));

		await backend.save(data);
		const loaded = await backend.load();

		expect(loaded.size).toBe(3);
		expect(loaded.get('key1')?.toString()).toBe('value1');
		expect(loaded.get('key2')?.toString()).toBe('value2');
		expect(loaded.get('key3')).toEqual(Buffer.from([0x00, 0x01, 0x02, 0xff]));
	});

	it('should handle empty data', async () => {
		const path = join(TEST_DIR, 'empty.simk');
		const backend = createFileStorageBackend({ path });

		await backend.save(new Map());
		const loaded = await backend.load();
		expect(loaded.size).toBe(0);
	});

	it('should overwrite previous data on save', async () => {
		const path = join(TEST_DIR, 'overwrite.simk');
		const backend = createFileStorageBackend({ path });

		const data1 = new Map<string, Buffer>();
		data1.set('a', Buffer.from('first'));
		await backend.save(data1);

		const data2 = new Map<string, Buffer>();
		data2.set('b', Buffer.from('second'));
		await backend.save(data2);

		const loaded = await backend.load();
		expect(loaded.size).toBe(1);
		expect(loaded.has('a')).toBe(false);
		expect(loaded.get('b')?.toString()).toBe('second');
	});

	it('should handle large values', async () => {
		const path = join(TEST_DIR, 'large.simk');
		const backend = createFileStorageBackend({ path });

		const largeValue = Buffer.alloc(100_000, 0x42);
		const data = new Map<string, Buffer>();
		data.set('large', largeValue);

		await backend.save(data);
		const loaded = await backend.load();

		expect(loaded.get('large')?.length).toBe(100_000);
		expect(loaded.get('large')?.every((b) => b === 0x42)).toBe(true);
	});

	it('should clean up temp files on close', async () => {
		const path = join(TEST_DIR, 'cleanup.simk');
		const backend = createFileStorageBackend({ path });

		const data = new Map<string, Buffer>();
		data.set('test', Buffer.from('data'));
		await backend.save(data);

		await backend.close();
		expect(existsSync(`${path}.tmp`)).toBe(false);
	});
});
