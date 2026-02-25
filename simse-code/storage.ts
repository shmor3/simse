/**
 * SimSE — File Storage Backend
 *
 * A file-based StorageBackend implementation that stores all data in a
 * single gzipped binary file with atomic writes. This is an example
 * implementation — consumers can swap in any backend (SQLite, S3, etc.).
 *
 * Uses only node:* built-ins — no external dependencies.
 */

import { Buffer } from 'node:buffer';
import { existsSync } from 'node:fs';
import { mkdir, readFile, rename, rm, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { gunzipSync, gzipSync } from 'node:zlib';
import type { StorageBackend } from 'simse';

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

export interface FileStorageOptions {
	/** Absolute or relative path to the storage file. */
	readonly path: string;
	/**
	 * If `true`, writes are performed atomically by writing to a temporary
	 * file first and then renaming. Defaults to `true`.
	 */
	readonly atomicWrite?: boolean;
	/** Gzip compression level (1-9). */
	readonly compressionLevel?: number;
}

// ---------------------------------------------------------------------------
// Binary format constants
// ---------------------------------------------------------------------------

const MAGIC = Buffer.from('SIMK');
const FORMAT_VERSION = 1;

// ---------------------------------------------------------------------------
// Serialization helpers
// ---------------------------------------------------------------------------

function serialize(data: Map<string, Buffer>): Buffer {
	let totalSize = MAGIC.length + 2 + 4; // magic + version(u16) + count(u32)
	for (const [key, value] of data) {
		const keyBytes = Buffer.byteLength(key, 'utf-8');
		totalSize += 4 + keyBytes + 4 + value.length;
	}

	const buf = Buffer.alloc(totalSize);
	let offset = 0;

	MAGIC.copy(buf, offset);
	offset += MAGIC.length;
	buf.writeUInt16BE(FORMAT_VERSION, offset);
	offset += 2;
	buf.writeUInt32BE(data.size, offset);
	offset += 4;

	for (const [key, value] of data) {
		const keyBuf = Buffer.from(key, 'utf-8');
		buf.writeUInt32BE(keyBuf.length, offset);
		offset += 4;
		keyBuf.copy(buf, offset);
		offset += keyBuf.length;
		buf.writeUInt32BE(value.length, offset);
		offset += 4;
		value.copy(buf, offset);
		offset += value.length;
	}

	return buf;
}

function deserialize(buf: Buffer): Map<string, Buffer> {
	const result = new Map<string, Buffer>();

	if (buf.length < MAGIC.length + 2 + 4) {
		throw new Error('Storage file too small — corrupt or truncated');
	}

	if (!buf.subarray(0, MAGIC.length).equals(MAGIC)) {
		throw new Error('Invalid storage file — missing SIMK magic header');
	}

	let offset = MAGIC.length;

	const version = buf.readUInt16BE(offset);
	offset += 2;
	if (version !== FORMAT_VERSION) {
		throw new Error(
			`Unsupported storage format version: ${version} (expected ${FORMAT_VERSION})`,
		);
	}

	const count = buf.readUInt32BE(offset);
	offset += 4;

	for (let i = 0; i < count; i++) {
		if (offset + 4 > buf.length) {
			throw new Error(`Corrupt storage file at entry ${i} — unexpected EOF`);
		}
		const keyLen = buf.readUInt32BE(offset);
		offset += 4;

		if (offset + keyLen > buf.length) {
			throw new Error(`Corrupt storage file at entry ${i} — key truncated`);
		}
		const key = buf.toString('utf-8', offset, offset + keyLen);
		offset += keyLen;

		if (offset + 4 > buf.length) {
			throw new Error(
				`Corrupt storage file at entry ${i} — missing value length`,
			);
		}
		const valLen = buf.readUInt32BE(offset);
		offset += 4;

		if (offset + valLen > buf.length) {
			throw new Error(`Corrupt storage file at entry ${i} — value truncated`);
		}
		const value = Buffer.from(buf.subarray(offset, offset + valLen));
		offset += valLen;

		result.set(key, value);
	}

	return result;
}

// ---------------------------------------------------------------------------
// File storage backend
// ---------------------------------------------------------------------------

export function createFileStorageBackend(
	options: FileStorageOptions,
): StorageBackend {
	const filePath = resolve(process.cwd(), options.path);
	const atomicWrite = options.atomicWrite ?? true;
	const compressionLevel = options.compressionLevel ?? 6;

	const load = async (): Promise<Map<string, Buffer>> => {
		if (!existsSync(filePath)) {
			return new Map();
		}

		let raw: Buffer;
		try {
			raw = await readFile(filePath);
		} catch (error) {
			throw new Error(
				`Failed to read storage file: ${error instanceof Error ? error.message : error}`,
			);
		}

		if (raw.length === 0) {
			return new Map();
		}

		// Detect gzip (magic bytes 0x1f 0x8b)
		if (raw.length >= 2 && raw[0] === 0x1f && raw[1] === 0x8b) {
			try {
				raw = gunzipSync(raw) as Buffer;
			} catch (error) {
				throw new Error(
					`Failed to decompress storage file: ${error instanceof Error ? error.message : error}`,
				);
			}
		}

		return deserialize(raw);
	};

	const save = async (data: Map<string, Buffer>): Promise<void> => {
		const dir = dirname(filePath);
		await mkdir(dir, { recursive: true });

		const raw = serialize(data);
		const compressed = gzipSync(raw, { level: compressionLevel });

		if (atomicWrite) {
			const tmpPath = `${filePath}.tmp`;
			await writeFile(tmpPath, compressed);
			await rename(tmpPath, filePath);
		} else {
			await writeFile(filePath, compressed);
		}
	};

	const close = async (): Promise<void> => {
		const tmpPath = `${filePath}.tmp`;
		if (existsSync(tmpPath)) {
			await rm(tmpPath, { force: true });
		}
	};

	return Object.freeze({ load, save, close });
}
