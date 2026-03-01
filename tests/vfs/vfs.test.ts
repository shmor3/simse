import { afterEach, describe, expect, it } from 'bun:test';
import { fileURLToPath } from 'node:url';
import type { Logger } from '../../src/ai/shared/logger.js';
import { createNoopLogger } from '../../src/ai/shared/logger.js';
import { isVFSError } from '../../src/ai/vfs/errors.js';
import {
	ancestorPaths,
	baseName,
	normalizePath,
	parentPath,
	pathDepth,
	toLocalPath,
	VFS_ROOT,
	VFS_SCHEME,
	validatePath,
} from '../../src/ai/vfs/path-utils.js';
import type { VFSSearchResult } from '../../src/ai/vfs/types.js';
import type { VirtualFS } from '../../src/ai/vfs/vfs.js';
import { createVirtualFS } from '../../src/ai/vfs/vfs.js';
import { expectGuardedThrow } from './utils/error-helpers';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const ENGINE_PATH = fileURLToPath(
	new URL('../../simse-vfs/target/debug/simse-vfs-engine.exe', import.meta.url),
);

function createSilentLogger(): Logger {
	return createNoopLogger();
}

const instances: VirtualFS[] = [];

async function createFS(
	overrides?: Parameters<typeof createVirtualFS>[0],
): Promise<VirtualFS> {
	const vfs = await createVirtualFS({
		logger: createSilentLogger(),
		enginePath: ENGINE_PATH,
		...overrides,
	});
	instances.push(vfs);
	return vfs;
}

afterEach(async () => {
	for (const vfs of instances) {
		await vfs.dispose();
	}
	instances.length = 0;
});

// ---------------------------------------------------------------------------
// path-utils
// ---------------------------------------------------------------------------

describe('path-utils', () => {
	describe('VFS_SCHEME', () => {
		it('equals vfs://', () => {
			expect(VFS_SCHEME).toBe('vfs://');
		});
	});

	describe('VFS_ROOT', () => {
		it('equals vfs:///', () => {
			expect(VFS_ROOT).toBe('vfs:///');
		});
	});

	describe('toLocalPath', () => {
		it('strips vfs:// prefix', () => {
			expect(toLocalPath('vfs:///foo/bar')).toBe('/foo/bar');
		});

		it('returns / for vfs://', () => {
			expect(toLocalPath('vfs://')).toBe('/');
		});

		it('returns / for vfs:///', () => {
			expect(toLocalPath('vfs:///')).toBe('/');
		});

		it('throws on bare path', () => {
			expect(() => toLocalPath('/foo/bar')).toThrow(
				'Path must start with vfs://',
			);
		});

		it('throws on empty string', () => {
			expect(() => toLocalPath('')).toThrow('Path must start with vfs://');
		});
	});

	describe('normalizePath', () => {
		it('returns vfs:/// for vfs:///', () => {
			expect(normalizePath('vfs:///')).toBe('vfs:///');
		});

		it('returns vfs:/// for vfs://', () => {
			expect(normalizePath('vfs://')).toBe('vfs:///');
		});

		it('preserves absolute path', () => {
			expect(normalizePath('vfs:///foo/bar')).toBe('vfs:///foo/bar');
		});

		it('resolves . segments', () => {
			expect(normalizePath('vfs:///foo/./bar')).toBe('vfs:///foo/bar');
		});

		it('resolves .. segments', () => {
			expect(normalizePath('vfs:///foo/bar/../baz')).toBe('vfs:///foo/baz');
		});

		it('does not go above root with ..', () => {
			expect(normalizePath('vfs:///../../foo')).toBe('vfs:///foo');
		});

		it('converts backslashes', () => {
			expect(normalizePath('vfs:///foo\\bar')).toBe('vfs:///foo/bar');
		});

		it('collapses multiple slashes', () => {
			expect(normalizePath('vfs:////foo///bar///')).toBe('vfs:///foo/bar');
		});

		it('throws on /foo/bar', () => {
			expect(() => normalizePath('/foo/bar')).toThrow(
				'Path must start with vfs://',
			);
		});

		it('throws on foo/bar', () => {
			expect(() => normalizePath('foo/bar')).toThrow(
				'Path must start with vfs://',
			);
		});

		it('throws on empty string', () => {
			expect(() => normalizePath('')).toThrow('Path must start with vfs://');
		});
	});

	describe('parentPath', () => {
		it('returns undefined for root', () => {
			expect(parentPath('vfs:///')).toBeUndefined();
		});

		it('returns vfs:/// for direct child of root', () => {
			expect(parentPath('vfs:///foo')).toBe('vfs:///');
		});

		it('returns parent for nested path', () => {
			expect(parentPath('vfs:///foo/bar/baz')).toBe('vfs:///foo/bar');
		});
	});

	describe('baseName', () => {
		it('returns empty string for root', () => {
			expect(baseName('vfs:///')).toBe('');
		});

		it('returns last segment', () => {
			expect(baseName('vfs:///foo/bar')).toBe('bar');
		});
	});

	describe('ancestorPaths', () => {
		it('returns [vfs:///] for root', () => {
			expect(ancestorPaths('vfs:///')).toEqual(['vfs:///']);
		});

		it('returns [vfs:///] for direct child of root', () => {
			expect(ancestorPaths('vfs:///foo')).toEqual(['vfs:///']);
		});

		it('returns all ancestors for nested path', () => {
			expect(ancestorPaths('vfs:///a/b/c')).toEqual([
				'vfs:///',
				'vfs:///a',
				'vfs:///a/b',
			]);
		});
	});

	describe('pathDepth', () => {
		it('returns 0 for root', () => {
			expect(pathDepth('vfs:///')).toBe(0);
		});

		it('returns 1 for vfs:///foo', () => {
			expect(pathDepth('vfs:///foo')).toBe(1);
		});

		it('returns 3 for vfs:///a/b/c', () => {
			expect(pathDepth('vfs:///a/b/c')).toBe(3);
		});
	});

	describe('validatePath', () => {
		const limits = {
			maxFiles: 100,
			maxTotalSize: 10_000_000,
			maxFileSize: 1_000_000,
			maxPathLength: 20,
			maxPathDepth: 3,
			maxNameLength: 10,
			maxNodeCount: 1000,
			maxDiffLines: 5000,
		};

		it('returns undefined for a valid path', () => {
			expect(validatePath('vfs:///foo/bar', limits)).toBeUndefined();
		});

		it('returns error when local part exceeds maxPathLength', () => {
			const longPath = `vfs:///${'a'.repeat(25)}`;
			expect(validatePath(longPath, limits)).toBe(
				'Path exceeds max length (20)',
			);
		});

		it('returns error when depth exceeds maxPathDepth', () => {
			expect(validatePath('vfs:///a/b/c/d', limits)).toBe(
				'Path exceeds max depth (3)',
			);
		});

		it('returns error for segment exceeding maxNameLength', () => {
			const longName = 'a'.repeat(11);
			expect(validatePath(`vfs:///${longName}`, limits)).toBe(
				'Path segment exceeds max name length (10)',
			);
		});

		it('returns error for segment with forbidden characters', () => {
			expect(validatePath('vfs:///foo\x01bar', limits)).toBe(
				'Path segment contains forbidden characters',
			);
		});
	});
});

// ---------------------------------------------------------------------------
// createVirtualFS
// ---------------------------------------------------------------------------

describe('createVirtualFS', () => {
	it('returns a frozen object', async () => {
		const vfs = await createFS();
		expect(Object.isFrozen(vfs)).toBe(true);
	});

	it('starts with root directory only', async () => {
		const vfs = await createFS();
		expect(await vfs.exists('vfs:///')).toBe(true);
		const m = await vfs.metrics();
		expect(m.nodeCount).toBe(1);
		expect(m.directoryCount).toBe(1);
		expect(m.fileCount).toBe(0);
		expect(m.totalSize).toBe(0);
	});

	// -- writeFile / readFile ---------------------------------------------

	describe('writeFile / readFile', () => {
		it('writes and reads a text file', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///hello.txt', 'Hello, world!');
			const result = await vfs.readFile('vfs:///hello.txt');
			expect(result.contentType).toBe('text');
			expect(result.text).toBe('Hello, world!');
			expect(result.data).toBeUndefined();
			expect(result.size).toBeGreaterThan(0);
		});

		it('writes and reads a binary file', async () => {
			const vfs = await createFS();
			const data = new Uint8Array([1, 2, 3, 4, 5]);
			await vfs.writeFile('vfs:///bin.dat', data);
			const result = await vfs.readFile('vfs:///bin.dat');
			expect(result.contentType).toBe('binary');
			expect(result.data).toEqual(data);
			expect(result.text).toBeUndefined();
			expect(result.size).toBe(5);
		});

		it('returns a defensive copy for binary data', async () => {
			const vfs = await createFS();
			const data = new Uint8Array([1, 2, 3]);
			await vfs.writeFile('vfs:///bin.dat', data);
			const result = await vfs.readFile('vfs:///bin.dat');
			result.data![0] = 99;
			const result2 = await vfs.readFile('vfs:///bin.dat');
			expect(result2.data![0]).toBe(1);
		});

		it('overwrites existing file', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///f.txt', 'first');
			await vfs.writeFile('vfs:///f.txt', 'second');
			expect((await vfs.readFile('vfs:///f.txt')).text).toBe('second');
			expect((await vfs.metrics()).fileCount).toBe(1);
		});

		it('preserves createdAt on overwrite', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///f.txt', 'first');
			const created = (await vfs.stat('vfs:///f.txt')).createdAt;
			await vfs.writeFile('vfs:///f.txt', 'second');
			expect((await vfs.stat('vfs:///f.txt')).createdAt).toBe(created);
		});

		it('writes with createParents', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///a/b/c/file.txt', 'deep', {
				createParents: true,
			});
			expect(await vfs.exists('vfs:///a')).toBe(true);
			expect(await vfs.exists('vfs:///a/b')).toBe(true);
			expect(await vfs.exists('vfs:///a/b/c')).toBe(true);
			expect((await vfs.readFile('vfs:///a/b/c/file.txt')).text).toBe('deep');
		});

		it('throws VFS_NOT_FOUND without parent directory', async () => {
			const vfs = await createFS();
			await expectGuardedThrow(
				() => vfs.writeFile('vfs:///missing/file.txt', 'data'),
				isVFSError,
				'VFS_NOT_FOUND',
			);
		});

		it('throws VFS_LIMIT_EXCEEDED for maxFileSize', async () => {
			const vfs = await createFS({ limits: { maxFileSize: 5 } });
			await expectGuardedThrow(
				() => vfs.writeFile('vfs:///big.txt', 'more than five bytes'),
				isVFSError,
				'VFS_LIMIT_EXCEEDED',
			);
		});

		it('throws VFS_LIMIT_EXCEEDED for maxTotalSize', async () => {
			const vfs = await createFS({ limits: { maxTotalSize: 10 } });
			await vfs.writeFile('vfs:///a.txt', '12345');
			await expectGuardedThrow(
				() => vfs.writeFile('vfs:///b.txt', '123456'),
				isVFSError,
				'VFS_LIMIT_EXCEEDED',
			);
		});

		it('throws VFS_INVALID_OPERATION when writing to root', async () => {
			const vfs = await createFS();
			await expectGuardedThrow(
				() => vfs.writeFile('vfs:///', 'data'),
				isVFSError,
				'VFS_INVALID_OPERATION',
			);
		});

		it('throws VFS_NOT_FOUND when reading non-existent file', async () => {
			const vfs = await createFS();
			await expectGuardedThrow(
				() => vfs.readFile('vfs:///missing.txt'),
				isVFSError,
				'VFS_NOT_FOUND',
			);
		});

		it('throws VFS_NOT_FILE when reading a directory', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///dir');
			await expectGuardedThrow(
				() => vfs.readFile('vfs:///dir'),
				isVFSError,
				'VFS_NOT_FILE',
			);
		});

		it('throws VFS_NOT_FILE when overwriting directory with file', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///dir');
			await expectGuardedThrow(
				() => vfs.writeFile('vfs:///dir', 'data'),
				isVFSError,
				'VFS_NOT_FILE',
			);
		});

		it('tracks totalSize correctly through writes', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///a.txt', 'abc');
			const s1 = (await vfs.metrics()).totalSize;
			expect(s1).toBe(3);
			await vfs.writeFile('vfs:///a.txt', 'ab');
			expect((await vfs.metrics()).totalSize).toBe(2);
		});
	});

	// -- appendFile -------------------------------------------------------

	describe('appendFile', () => {
		it('appends to a text file', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///f.txt', 'hello');
			await vfs.appendFile('vfs:///f.txt', ' world');
			expect((await vfs.readFile('vfs:///f.txt')).text).toBe('hello world');
		});

		it('throws VFS_INVALID_OPERATION on binary file', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///bin.dat', new Uint8Array([1]));
			await expectGuardedThrow(
				() => vfs.appendFile('vfs:///bin.dat', 'text'),
				isVFSError,
				'VFS_INVALID_OPERATION',
			);
		});

		it('throws VFS_NOT_FOUND on non-existent file', async () => {
			const vfs = await createFS();
			await expectGuardedThrow(
				() => vfs.appendFile('vfs:///missing.txt', 'text'),
				isVFSError,
				'VFS_NOT_FOUND',
			);
		});

		it('updates totalSize after append', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///f.txt', 'abc');
			await vfs.appendFile('vfs:///f.txt', 'de');
			expect((await vfs.metrics()).totalSize).toBe(5);
		});
	});

	// -- deleteFile -------------------------------------------------------

	describe('deleteFile', () => {
		it('deletes an existing file', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///f.txt', 'data');
			expect(await vfs.deleteFile('vfs:///f.txt')).toBe(true);
			expect(await vfs.exists('vfs:///f.txt')).toBe(false);
			expect((await vfs.metrics()).fileCount).toBe(0);
		});

		it('returns false for non-existent file', async () => {
			const vfs = await createFS();
			expect(await vfs.deleteFile('vfs:///missing.txt')).toBe(false);
		});

		it('throws VFS_NOT_FILE when deleting a directory', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///dir');
			await expectGuardedThrow(
				() => vfs.deleteFile('vfs:///dir'),
				isVFSError,
				'VFS_NOT_FILE',
			);
		});

		it('updates totalSize after delete', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///f.txt', 'abc');
			expect((await vfs.metrics()).totalSize).toBe(3);
			await vfs.deleteFile('vfs:///f.txt');
			expect((await vfs.metrics()).totalSize).toBe(0);
		});
	});

	// -- mkdir ------------------------------------------------------------

	describe('mkdir', () => {
		it('creates a directory', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///dir');
			expect(await vfs.exists('vfs:///dir')).toBe(true);
			expect((await vfs.stat('vfs:///dir')).type).toBe('directory');
		});

		it('creates nested directories with recursive', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///a/b/c', { recursive: true });
			expect(await vfs.exists('vfs:///a')).toBe(true);
			expect(await vfs.exists('vfs:///a/b')).toBe(true);
			expect(await vfs.exists('vfs:///a/b/c')).toBe(true);
		});

		it('throws VFS_NOT_FOUND without parent directory', async () => {
			const vfs = await createFS();
			await expectGuardedThrow(
				() => vfs.mkdir('vfs:///missing/dir'),
				isVFSError,
				'VFS_NOT_FOUND',
			);
		});

		it('is idempotent for existing directory', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///dir');
			await vfs.mkdir('vfs:///dir');
			expect((await vfs.metrics()).directoryCount).toBe(2);
		});

		it('throws VFS_NOT_DIRECTORY when path is a file', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///f.txt', 'data');
			await expectGuardedThrow(
				() => vfs.mkdir('vfs:///f.txt'),
				isVFSError,
				'VFS_NOT_DIRECTORY',
			);
		});

		it('is a no-op for root', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///');
			expect((await vfs.metrics()).directoryCount).toBe(1);
		});
	});

	// -- readdir ----------------------------------------------------------

	describe('readdir', () => {
		it('lists empty directory', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///dir');
			expect(await vfs.readdir('vfs:///dir')).toEqual([]);
		});

		it('lists files and subdirectories', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///dir');
			await vfs.writeFile('vfs:///dir/a.txt', 'a');
			await vfs.mkdir('vfs:///dir/sub');
			const entries = await vfs.readdir('vfs:///dir');
			expect(entries.length).toBe(2);
			const names = entries.map((e) => e.name).sort();
			expect(names).toEqual(['a.txt', 'sub']);
		});

		it('lists recursively', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///dir/sub', { recursive: true });
			await vfs.writeFile('vfs:///dir/a.txt', 'a');
			await vfs.writeFile('vfs:///dir/sub/b.txt', 'b');
			const entries = await vfs.readdir('vfs:///dir', { recursive: true });
			const names = entries.map((e) => e.name).sort();
			expect(names).toEqual(['a.txt', 'sub', 'sub/b.txt']);
		});

		it('throws VFS_NOT_DIRECTORY on a file', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///f.txt', 'data');
			await expectGuardedThrow(
				() => vfs.readdir('vfs:///f.txt'),
				isVFSError,
				'VFS_NOT_DIRECTORY',
			);
		});

		it('throws VFS_NOT_FOUND on non-existent path', async () => {
			const vfs = await createFS();
			await expectGuardedThrow(
				() => vfs.readdir('vfs:///missing'),
				isVFSError,
				'VFS_NOT_FOUND',
			);
		});

		it('returns frozen array', async () => {
			const vfs = await createFS();
			const entries = await vfs.readdir('vfs:///');
			expect(Object.isFrozen(entries)).toBe(true);
		});
	});

	// -- rmdir ------------------------------------------------------------

	describe('rmdir', () => {
		it('removes empty directory', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///dir');
			expect(await vfs.rmdir('vfs:///dir')).toBe(true);
			expect(await vfs.exists('vfs:///dir')).toBe(false);
		});

		it('returns false for non-existent directory', async () => {
			const vfs = await createFS();
			expect(await vfs.rmdir('vfs:///missing')).toBe(false);
		});

		it('throws VFS_NOT_EMPTY without recursive', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///dir');
			await vfs.writeFile('vfs:///dir/f.txt', 'data');
			await expectGuardedThrow(
				() => vfs.rmdir('vfs:///dir'),
				isVFSError,
				'VFS_NOT_EMPTY',
			);
		});

		it('removes non-empty directory with recursive', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///dir/sub', { recursive: true });
			await vfs.writeFile('vfs:///dir/a.txt', 'a');
			await vfs.writeFile('vfs:///dir/sub/b.txt', 'b');
			expect(await vfs.rmdir('vfs:///dir', { recursive: true })).toBe(true);
			expect(await vfs.exists('vfs:///dir')).toBe(false);
			expect(await vfs.exists('vfs:///dir/sub')).toBe(false);
			expect(await vfs.exists('vfs:///dir/a.txt')).toBe(false);
		});

		it('updates totalSize after recursive delete', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///dir');
			await vfs.writeFile('vfs:///dir/a.txt', 'abc');
			await vfs.writeFile('vfs:///dir/b.txt', 'de');
			expect((await vfs.metrics()).totalSize).toBe(5);
			await vfs.rmdir('vfs:///dir', { recursive: true });
			expect((await vfs.metrics()).totalSize).toBe(0);
		});

		it('throws VFS_INVALID_OPERATION when deleting root', async () => {
			const vfs = await createFS();
			await expectGuardedThrow(
				() => vfs.rmdir('vfs:///'),
				isVFSError,
				'VFS_INVALID_OPERATION',
			);
		});

		it('throws VFS_NOT_DIRECTORY when path is a file', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///f.txt', 'data');
			await expectGuardedThrow(
				() => vfs.rmdir('vfs:///f.txt'),
				isVFSError,
				'VFS_NOT_DIRECTORY',
			);
		});
	});

	// -- stat -------------------------------------------------------------

	describe('stat', () => {
		it('returns stat for a file', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///f.txt', 'hello');
			const s = await vfs.stat('vfs:///f.txt');
			expect(s.type).toBe('file');
			expect(s.path).toBe('vfs:///f.txt');
			expect(s.size).toBe(5);
			expect(s.createdAt).toBeGreaterThan(0);
			expect(s.modifiedAt).toBeGreaterThanOrEqual(s.createdAt);
		});

		it('returns stat for a directory', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///dir');
			const s = await vfs.stat('vfs:///dir');
			expect(s.type).toBe('directory');
			expect(s.size).toBe(0);
		});

		it('throws VFS_NOT_FOUND for non-existent path', async () => {
			const vfs = await createFS();
			await expectGuardedThrow(
				() => vfs.stat('vfs:///missing'),
				isVFSError,
				'VFS_NOT_FOUND',
			);
		});

		it('returns frozen object', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///f.txt', 'data');
			expect(Object.isFrozen(await vfs.stat('vfs:///f.txt'))).toBe(true);
		});
	});

	// -- exists -----------------------------------------------------------

	describe('exists', () => {
		it('returns true for existing file', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///f.txt', 'data');
			expect(await vfs.exists('vfs:///f.txt')).toBe(true);
		});

		it('returns true for existing directory', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///dir');
			expect(await vfs.exists('vfs:///dir')).toBe(true);
		});

		it('returns false for non-existent path', async () => {
			const vfs = await createFS();
			expect(await vfs.exists('vfs:///nope')).toBe(false);
		});

		it('returns true for root', async () => {
			const vfs = await createFS();
			expect(await vfs.exists('vfs:///')).toBe(true);
		});
	});

	// -- rename -----------------------------------------------------------

	describe('rename', () => {
		it('renames a file', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///a.txt', 'data');
			await vfs.rename('vfs:///a.txt', 'vfs:///b.txt');
			expect(await vfs.exists('vfs:///a.txt')).toBe(false);
			expect((await vfs.readFile('vfs:///b.txt')).text).toBe('data');
		});

		it('renames a directory and moves descendants', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///old/sub', { recursive: true });
			await vfs.writeFile('vfs:///old/sub/f.txt', 'content');
			await vfs.rename('vfs:///old', 'vfs:///new');
			expect(await vfs.exists('vfs:///old')).toBe(false);
			expect(await vfs.exists('vfs:///old/sub')).toBe(false);
			expect(await vfs.exists('vfs:///new')).toBe(true);
			expect(await vfs.exists('vfs:///new/sub')).toBe(true);
			expect((await vfs.readFile('vfs:///new/sub/f.txt')).text).toBe('content');
		});

		it('throws VFS_ALREADY_EXISTS when destination exists', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///a.txt', 'a');
			await vfs.writeFile('vfs:///b.txt', 'b');
			await expectGuardedThrow(
				() => vfs.rename('vfs:///a.txt', 'vfs:///b.txt'),
				isVFSError,
				'VFS_ALREADY_EXISTS',
			);
		});

		it('throws VFS_INVALID_OPERATION when renaming root', async () => {
			const vfs = await createFS();
			await expectGuardedThrow(
				() => vfs.rename('vfs:///', 'vfs:///new'),
				isVFSError,
				'VFS_INVALID_OPERATION',
			);
		});

		it('throws VFS_NOT_FOUND when source does not exist', async () => {
			const vfs = await createFS();
			await expectGuardedThrow(
				() => vfs.rename('vfs:///missing', 'vfs:///new'),
				isVFSError,
				'VFS_NOT_FOUND',
			);
		});

		it('throws VFS_NOT_FOUND when dest parent does not exist', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///a.txt', 'data');
			await expectGuardedThrow(
				() => vfs.rename('vfs:///a.txt', 'vfs:///missing/b.txt'),
				isVFSError,
				'VFS_NOT_FOUND',
			);
		});
	});

	// -- copy -------------------------------------------------------------

	describe('copy', () => {
		it('copies a file', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///a.txt', 'data');
			await vfs.copy('vfs:///a.txt', 'vfs:///b.txt');
			expect((await vfs.readFile('vfs:///a.txt')).text).toBe('data');
			expect((await vfs.readFile('vfs:///b.txt')).text).toBe('data');
			expect((await vfs.metrics()).fileCount).toBe(2);
		});

		it('copies a file with overwrite', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///a.txt', 'first');
			await vfs.writeFile('vfs:///b.txt', 'second');
			await vfs.copy('vfs:///a.txt', 'vfs:///b.txt', { overwrite: true });
			expect((await vfs.readFile('vfs:///b.txt')).text).toBe('first');
		});

		it('throws VFS_ALREADY_EXISTS without overwrite', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///a.txt', 'a');
			await vfs.writeFile('vfs:///b.txt', 'b');
			await expectGuardedThrow(
				() => vfs.copy('vfs:///a.txt', 'vfs:///b.txt'),
				isVFSError,
				'VFS_ALREADY_EXISTS',
			);
		});

		it('copies a directory recursively', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///src/sub', { recursive: true });
			await vfs.writeFile('vfs:///src/a.txt', 'a');
			await vfs.writeFile('vfs:///src/sub/b.txt', 'b');
			await vfs.copy('vfs:///src', 'vfs:///dst', { recursive: true });
			expect(await vfs.exists('vfs:///dst')).toBe(true);
			expect(await vfs.exists('vfs:///dst/sub')).toBe(true);
			expect((await vfs.readFile('vfs:///dst/a.txt')).text).toBe('a');
			expect((await vfs.readFile('vfs:///dst/sub/b.txt')).text).toBe('b');
			// Source still exists
			expect((await vfs.readFile('vfs:///src/a.txt')).text).toBe('a');
		});

		it('throws VFS_INVALID_OPERATION when copying dir without recursive', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///src');
			await expectGuardedThrow(
				() => vfs.copy('vfs:///src', 'vfs:///dst'),
				isVFSError,
				'VFS_INVALID_OPERATION',
			);
		});

		it('copies binary files', async () => {
			const vfs = await createFS();
			const data = new Uint8Array([10, 20, 30]);
			await vfs.writeFile('vfs:///bin.dat', data);
			await vfs.copy('vfs:///bin.dat', 'vfs:///copy.dat');
			expect((await vfs.readFile('vfs:///copy.dat')).data).toEqual(data);
		});
	});

	// -- snapshot / restore -----------------------------------------------

	describe('snapshot / restore', () => {
		it('round-trips text files', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///dir');
			await vfs.writeFile('vfs:///dir/f.txt', 'hello');
			const snap = await vfs.snapshot();

			const vfs2 = await createFS();
			await vfs2.restore(snap);
			expect((await vfs2.readFile('vfs:///dir/f.txt')).text).toBe('hello');
			expect(await vfs2.exists('vfs:///dir')).toBe(true);
		});

		it('round-trips binary files', async () => {
			const vfs = await createFS();
			const data = new Uint8Array([255, 0, 128]);
			await vfs.writeFile('vfs:///bin.dat', data);
			const snap = await vfs.snapshot();

			const vfs2 = await createFS();
			await vfs2.restore(snap);
			expect((await vfs2.readFile('vfs:///bin.dat')).data).toEqual(data);
			expect((await vfs2.readFile('vfs:///bin.dat')).contentType).toBe(
				'binary',
			);
		});

		it('preserves timestamps', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///f.txt', 'data');
			const original = await vfs.stat('vfs:///f.txt');
			const snap = await vfs.snapshot();

			const vfs2 = await createFS();
			await vfs2.restore(snap);
			const restored = await vfs2.stat('vfs:///f.txt');
			expect(restored.createdAt).toBe(original.createdAt);
			expect(restored.modifiedAt).toBe(original.modifiedAt);
		});

		it('clear resets to empty root', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///dir');
			await vfs.writeFile('vfs:///dir/f.txt', 'data');
			await vfs.clear();
			expect(await vfs.exists('vfs:///')).toBe(true);
			const m = await vfs.metrics();
			expect(m.nodeCount).toBe(1);
			expect(m.totalSize).toBe(0);
		});

		it('snapshot is frozen', async () => {
			const vfs = await createFS();
			const snap = await vfs.snapshot();
			expect(Object.isFrozen(snap)).toBe(true);
			expect(Object.isFrozen(snap.files)).toBe(true);
			expect(Object.isFrozen(snap.directories)).toBe(true);
		});

		it('snapshot does not include root directory', async () => {
			const vfs = await createFS();
			const snap = await vfs.snapshot();
			expect(snap.directories.length).toBe(0);
			expect(snap.files.length).toBe(0);
		});
	});

	// -- limits -----------------------------------------------------------

	describe('limits', () => {
		it('enforces maxNodeCount', async () => {
			const vfs = await createFS({ limits: { maxNodeCount: 3 } });
			// Root is 1, dir is 2, file is 3
			await vfs.mkdir('vfs:///dir');
			await vfs.writeFile('vfs:///dir/f.txt', 'data');
			await expectGuardedThrow(
				() => vfs.writeFile('vfs:///dir/g.txt', 'more'),
				isVFSError,
				'VFS_LIMIT_EXCEEDED',
			);
		});

		it('enforces maxPathDepth', async () => {
			const vfs = await createFS({ limits: { maxPathDepth: 2 } });
			await vfs.mkdir('vfs:///a/b', { recursive: true });
			await expectGuardedThrow(
				() => vfs.mkdir('vfs:///a/b/c'),
				isVFSError,
				'VFS_INVALID_PATH',
			);
		});

		it('enforces maxNameLength', async () => {
			const vfs = await createFS({ limits: { maxNameLength: 5 } });
			await expectGuardedThrow(
				() => vfs.mkdir('vfs:///toolong'),
				isVFSError,
				'VFS_INVALID_PATH',
			);
			await vfs.mkdir('vfs:///short');
			expect(await vfs.exists('vfs:///short')).toBe(true);
		});

		it('enforces maxPathLength', async () => {
			const vfs = await createFS({ limits: { maxPathLength: 10 } });
			await expectGuardedThrow(
				() => vfs.mkdir('vfs:///this-is-way-too-long'),
				isVFSError,
				'VFS_INVALID_PATH',
			);
		});

		it('enforces maxFileSize', async () => {
			const vfs = await createFS({ limits: { maxFileSize: 3 } });
			await vfs.writeFile('vfs:///ok.txt', 'abc');
			await expectGuardedThrow(
				() => vfs.writeFile('vfs:///big.txt', 'abcd'),
				isVFSError,
				'VFS_LIMIT_EXCEEDED',
			);
		});

		it('enforces maxTotalSize across multiple files', async () => {
			const vfs = await createFS({ limits: { maxTotalSize: 10 } });
			await vfs.writeFile('vfs:///a.txt', '12345');
			await vfs.writeFile('vfs:///b.txt', '12345');
			await expectGuardedThrow(
				() => vfs.writeFile('vfs:///c.txt', '1'),
				isVFSError,
				'VFS_LIMIT_EXCEEDED',
			);
		});

		it('allows overwrite that reduces size within limits', async () => {
			const vfs = await createFS({ limits: { maxTotalSize: 10 } });
			await vfs.writeFile('vfs:///a.txt', '1234567890');
			// Overwrite with smaller content should be fine
			await vfs.writeFile('vfs:///a.txt', 'small');
			expect((await vfs.metrics()).totalSize).toBe(5);
		});
	});

	// -- metrics ----------------------------------------------------------

	describe('metrics', () => {
		it('tracks nodeCount accurately', async () => {
			const vfs = await createFS();
			expect((await vfs.metrics()).nodeCount).toBe(1); // root
			await vfs.mkdir('vfs:///dir');
			expect((await vfs.metrics()).nodeCount).toBe(2);
			await vfs.writeFile('vfs:///dir/f.txt', 'data');
			expect((await vfs.metrics()).nodeCount).toBe(3);
			await vfs.deleteFile('vfs:///dir/f.txt');
			expect((await vfs.metrics()).nodeCount).toBe(2);
		});

		it('tracks fileCount and directoryCount', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///a');
			await vfs.mkdir('vfs:///b');
			await vfs.writeFile('vfs:///a/f1.txt', '1');
			await vfs.writeFile('vfs:///a/f2.txt', '2');
			const m = await vfs.metrics();
			expect(m.fileCount).toBe(2);
			expect(m.directoryCount).toBe(3); // root + a + b
		});

		it('tracks totalSize through operations', async () => {
			const vfs = await createFS();
			expect((await vfs.metrics()).totalSize).toBe(0);
			await vfs.writeFile('vfs:///a.txt', 'abc');
			expect((await vfs.metrics()).totalSize).toBe(3);
			await vfs.appendFile('vfs:///a.txt', 'de');
			expect((await vfs.metrics()).totalSize).toBe(5);
			await vfs.writeFile('vfs:///a.txt', 'x');
			expect((await vfs.metrics()).totalSize).toBe(1);
			await vfs.deleteFile('vfs:///a.txt');
			expect((await vfs.metrics()).totalSize).toBe(0);
		});
	});

	// -- path normalization in API ----------------------------------------

	describe('path normalization', () => {
		it('normalizes paths on write and read', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///bar');
			await vfs.writeFile('vfs:///foo/../bar/./file.txt', 'data');
			expect((await vfs.readFile('vfs:///bar/file.txt')).text).toBe('data');
		});

		it('handles vfs:// paths without leading slash after scheme', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs://file.txt', 'data');
			expect((await vfs.readFile('vfs:///file.txt')).text).toBe('data');
		});
	});

	// -- rename: self-descendant guard ------------------------------------

	describe('rename into own descendant', () => {
		it('throws VFS_INVALID_OPERATION when moving dir into its own child', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///a/b', { recursive: true });
			await expectGuardedThrow(
				() => vfs.rename('vfs:///a', 'vfs:///a/b/c'),
				isVFSError,
				'VFS_INVALID_OPERATION',
			);
		});

		it('allows renaming to a sibling path that shares a prefix', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///abc');
			await vfs.rename('vfs:///abc', 'vfs:///abcdef');
			expect(await vfs.exists('vfs:///abcdef')).toBe(true);
			expect(await vfs.exists('vfs:///abc')).toBe(false);
		});

		it('is a no-op when old and new paths are the same', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///f.txt', 'data');
			await vfs.rename('vfs:///f.txt', 'vfs:///f.txt');
			expect((await vfs.readFile('vfs:///f.txt')).text).toBe('data');
		});
	});

	// -- copy overwrite on directories ------------------------------------

	describe('copy overwrite on directories', () => {
		it('replaces dest directory contents with overwrite + recursive', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///src');
			await vfs.writeFile('vfs:///src/a.txt', 'source-a');

			await vfs.mkdir('vfs:///dst');
			await vfs.writeFile('vfs:///dst/stale.txt', 'should-be-removed');

			await vfs.copy('vfs:///src', 'vfs:///dst', {
				overwrite: true,
				recursive: true,
			});

			expect((await vfs.readFile('vfs:///dst/a.txt')).text).toBe('source-a');
			expect(await vfs.exists('vfs:///dst/stale.txt')).toBe(false);
		});

		it('works when dest does not exist', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///src');
			await vfs.writeFile('vfs:///src/a.txt', 'data');
			await vfs.copy('vfs:///src', 'vfs:///dst', {
				overwrite: true,
				recursive: true,
			});
			expect((await vfs.readFile('vfs:///dst/a.txt')).text).toBe('data');
		});
	});

	// -- restore with limits ----------------------------------------------

	describe('restore with limits', () => {
		it('throws VFS_LIMIT_EXCEEDED when snapshot exceeds maxNodeCount', async () => {
			const vfs = await createFS({ limits: { maxNodeCount: 2 } });
			const snap: Parameters<typeof vfs.restore>[0] = {
				files: [
					{
						path: 'vfs:///a.txt',
						contentType: 'text',
						text: 'a',
						createdAt: 0,
						modifiedAt: 0,
					},
					{
						path: 'vfs:///b.txt',
						contentType: 'text',
						text: 'b',
						createdAt: 0,
						modifiedAt: 0,
					},
				],
				directories: [],
			};
			await expectGuardedThrow(
				() => vfs.restore(snap),
				isVFSError,
				'VFS_LIMIT_EXCEEDED',
			);
		});

		it('throws VFS_LIMIT_EXCEEDED when snapshot exceeds maxFileSize', async () => {
			const vfs = await createFS({ limits: { maxFileSize: 3 } });
			const snap: Parameters<typeof vfs.restore>[0] = {
				files: [
					{
						path: 'vfs:///big.txt',
						contentType: 'text',
						text: 'toolong',
						createdAt: 0,
						modifiedAt: 0,
					},
				],
				directories: [],
			};
			await expectGuardedThrow(
				() => vfs.restore(snap),
				isVFSError,
				'VFS_LIMIT_EXCEEDED',
			);
		});

		it('throws VFS_LIMIT_EXCEEDED when snapshot exceeds maxTotalSize', async () => {
			const vfs = await createFS({ limits: { maxTotalSize: 5 } });
			const snap: Parameters<typeof vfs.restore>[0] = {
				files: [
					{
						path: 'vfs:///a.txt',
						contentType: 'text',
						text: 'aaa',
						createdAt: 0,
						modifiedAt: 0,
					},
					{
						path: 'vfs:///b.txt',
						contentType: 'text',
						text: 'bbb',
						createdAt: 0,
						modifiedAt: 0,
					},
				],
				directories: [],
			};
			await expectGuardedThrow(
				() => vfs.restore(snap),
				isVFSError,
				'VFS_LIMIT_EXCEEDED',
			);
		});

		it('throws VFS_INVALID_PATH when snapshot path exceeds limits', async () => {
			const vfs = await createFS({ limits: { maxPathDepth: 1 } });
			const snap: Parameters<typeof vfs.restore>[0] = {
				files: [],
				directories: [{ path: 'vfs:///a/b/c', createdAt: 0, modifiedAt: 0 }],
			};
			await expectGuardedThrow(
				() => vfs.restore(snap),
				isVFSError,
				'VFS_INVALID_PATH',
			);
		});

		it('does not partially commit on validation failure', async () => {
			const vfs = await createFS({ limits: { maxNodeCount: 2 } });
			await vfs.writeFile('vfs:///existing.txt', 'keep');

			const snap: Parameters<typeof vfs.restore>[0] = {
				files: [
					{
						path: 'vfs:///a.txt',
						contentType: 'text',
						text: 'a',
						createdAt: 0,
						modifiedAt: 0,
					},
					{
						path: 'vfs:///b.txt',
						contentType: 'text',
						text: 'b',
						createdAt: 0,
						modifiedAt: 0,
					},
				],
				directories: [],
			};
			try {
				await vfs.restore(snap);
			} catch {
				/* expected */
			}

			// Original state should be intact since we validated before committing
			expect(await vfs.exists('vfs:///existing.txt')).toBe(true);
		});
	});

	// -- discriminated union VFSReadResult --------------------------------

	describe('VFSReadResult discriminated union', () => {
		it('text files have text: string (not undefined)', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///f.txt', 'hello');
			const result = await vfs.readFile('vfs:///f.txt');
			if (result.contentType === 'text') {
				const text: string = result.text;
				expect(text).toBe('hello');
			}
		});

		it('binary files have data: Uint8Array (not undefined)', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///bin.dat', new Uint8Array([1, 2]));
			const result = await vfs.readFile('vfs:///bin.dat');
			if (result.contentType === 'binary') {
				const data: Uint8Array = result.data;
				expect(data).toEqual(new Uint8Array([1, 2]));
			}
		});
	});

	// -- tracked counters -------------------------------------------------

	describe('tracked counters', () => {
		it('fileCount and directoryCount stay accurate through operations', async () => {
			const vfs = await createFS();
			let m = await vfs.metrics();
			expect(m.fileCount).toBe(0);
			expect(m.directoryCount).toBe(1); // root

			await vfs.mkdir('vfs:///a');
			expect((await vfs.metrics()).directoryCount).toBe(2);

			await vfs.writeFile('vfs:///a/f1.txt', '1');
			await vfs.writeFile('vfs:///a/f2.txt', '2');
			expect((await vfs.metrics()).fileCount).toBe(2);

			await vfs.deleteFile('vfs:///a/f1.txt');
			expect((await vfs.metrics()).fileCount).toBe(1);

			await vfs.rmdir('vfs:///a', { recursive: true });
			m = await vfs.metrics();
			expect(m.fileCount).toBe(0);
			expect(m.directoryCount).toBe(1); // root only
		});

		it('counters reset on clear', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///a');
			await vfs.writeFile('vfs:///a/f.txt', 'data');
			await vfs.clear();
			const m = await vfs.metrics();
			expect(m.fileCount).toBe(0);
			expect(m.directoryCount).toBe(1);
		});

		it('counters are correct after restore', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///dir');
			await vfs.writeFile('vfs:///dir/f.txt', 'data');
			const snap = await vfs.snapshot();

			const vfs2 = await createFS();
			await vfs2.restore(snap);
			const m = await vfs2.metrics();
			expect(m.fileCount).toBe(1);
			expect(m.directoryCount).toBe(2);
		});
	});

	// -- glob -------------------------------------------------------------

	describe('glob', () => {
		it('matches files by extension', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///src');
			await vfs.writeFile('vfs:///src/a.ts', 'a');
			await vfs.writeFile('vfs:///src/b.ts', 'b');
			await vfs.writeFile('vfs:///src/c.js', 'c');

			const results = await vfs.glob('vfs:///**/*.ts');
			expect(results).toEqual(['vfs:///src/a.ts', 'vfs:///src/b.ts']);
		});

		it('matches with ** for any depth', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///a/b/c', { recursive: true });
			await vfs.writeFile('vfs:///a/f.txt', '1');
			await vfs.writeFile('vfs:///a/b/f.txt', '2');
			await vfs.writeFile('vfs:///a/b/c/f.txt', '3');

			const results = await vfs.glob('vfs:///**/f.txt');
			expect(results).toEqual([
				'vfs:///a/b/c/f.txt',
				'vfs:///a/b/f.txt',
				'vfs:///a/f.txt',
			]);
		});

		it('matches with ? for single character', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///a.txt', 'a');
			await vfs.writeFile('vfs:///ab.txt', 'ab');

			const results = await vfs.glob('vfs:///?.txt');
			expect(results).toEqual(['vfs:///a.txt']);
		});

		it('returns empty array when no matches', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///a.txt', 'a');
			expect(await vfs.glob('vfs:///*.js')).toEqual([]);
		});

		it('returns sorted results', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///c.txt', 'c');
			await vfs.writeFile('vfs:///a.txt', 'a');
			await vfs.writeFile('vfs:///b.txt', 'b');
			expect(await vfs.glob('vfs:///*.txt')).toEqual([
				'vfs:///a.txt',
				'vfs:///b.txt',
				'vfs:///c.txt',
			]);
		});

		it('only matches files, not directories', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///src');
			await vfs.writeFile('vfs:///src/file.ts', 'data');
			const results = await vfs.glob('vfs:///**/*');
			expect(results).toEqual(['vfs:///src/file.ts']);
		});
	});

	// -- tree -------------------------------------------------------------

	describe('tree', () => {
		it('renders a tree from root', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///src');
			await vfs.writeFile('vfs:///src/index.ts', 'code');
			await vfs.writeFile('vfs:///README.md', 'readme');

			const output = await vfs.tree();
			expect(output).toContain('vfs:///');
			expect(output).toContain('src/');
			expect(output).toContain('index.ts');
			expect(output).toContain('README.md');
			expect(output).toContain('bytes');
		});

		it('renders a subtree', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///a/b', { recursive: true });
			await vfs.writeFile('vfs:///a/b/f.txt', 'hi');

			const output = await vfs.tree('vfs:///a');
			expect(output).toContain('a');
			expect(output).toContain('b/');
			expect(output).toContain('f.txt');
		});

		it('uses tree characters', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///a.txt', 'a');
			await vfs.writeFile('vfs:///b.txt', 'b');

			const output = await vfs.tree();
			expect(output).toContain('├──');
			expect(output).toContain('└──');
		});

		it('throws VFS_NOT_FOUND for non-existent path', async () => {
			const vfs = await createFS();
			await expectGuardedThrow(
				() => vfs.tree('vfs:///missing'),
				isVFSError,
				'VFS_NOT_FOUND',
			);
		});

		it('throws VFS_NOT_DIRECTORY for a file', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///f.txt', 'data');
			await expectGuardedThrow(
				() => vfs.tree('vfs:///f.txt'),
				isVFSError,
				'VFS_NOT_DIRECTORY',
			);
		});
	});

	// -- du ---------------------------------------------------------------

	describe('du', () => {
		it('returns file size for a single file', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///f.txt', 'hello');
			expect(await vfs.du('vfs:///f.txt')).toBe(5);
		});

		it('returns total size of a directory subtree', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///dir');
			await vfs.writeFile('vfs:///dir/a.txt', 'abc');
			await vfs.writeFile('vfs:///dir/b.txt', 'de');
			expect(await vfs.du('vfs:///dir')).toBe(5);
		});

		it('includes nested subdirectory files', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///a/b', { recursive: true });
			await vfs.writeFile('vfs:///a/f1.txt', 'xx');
			await vfs.writeFile('vfs:///a/b/f2.txt', 'yyy');
			expect(await vfs.du('vfs:///a')).toBe(5);
		});

		it('returns 0 for an empty directory', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///empty');
			expect(await vfs.du('vfs:///empty')).toBe(0);
		});

		it('throws VFS_NOT_FOUND for non-existent path', async () => {
			const vfs = await createFS();
			await expectGuardedThrow(
				() => vfs.du('vfs:///missing'),
				isVFSError,
				'VFS_NOT_FOUND',
			);
		});
	});

	// -- history ----------------------------------------------------------

	describe('history', () => {
		it('returns empty array for a file with no edits', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///f.txt', 'initial');
			expect(await vfs.history('vfs:///f.txt')).toEqual([]);
		});

		it('records previous version on writeFile overwrite', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///f.txt', 'v1');
			await vfs.writeFile('vfs:///f.txt', 'v2');

			const hist = await vfs.history('vfs:///f.txt');
			expect(hist.length).toBe(1);
			expect(hist[0].version).toBe(1);
			expect(hist[0].text).toBe('v1');
			expect(hist[0].contentType).toBe('text');
		});

		it('records multiple versions', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///f.txt', 'v1');
			await vfs.writeFile('vfs:///f.txt', 'v2');
			await vfs.writeFile('vfs:///f.txt', 'v3');

			const hist = await vfs.history('vfs:///f.txt');
			expect(hist.length).toBe(2);
			expect(hist[0].text).toBe('v1');
			expect(hist[1].text).toBe('v2');
		});

		it('records history on appendFile', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///f.txt', 'line1');
			await vfs.appendFile('vfs:///f.txt', '\nline2');

			const hist = await vfs.history('vfs:///f.txt');
			expect(hist.length).toBe(1);
			expect(hist[0].text).toBe('line1');
		});

		it('tracks binary file history', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///bin.dat', new Uint8Array([1, 2, 3]));
			await vfs.writeFile('vfs:///bin.dat', new Uint8Array([4, 5, 6]));

			const hist = await vfs.history('vfs:///bin.dat');
			expect(hist.length).toBe(1);
			expect(hist[0].contentType).toBe('binary');
			expect(hist[0].base64).toBeDefined();
		});

		it('respects maxEntriesPerFile limit', async () => {
			const vfs = await createFS({ history: { maxEntriesPerFile: 3 } });
			await vfs.writeFile('vfs:///f.txt', 'v1');
			await vfs.writeFile('vfs:///f.txt', 'v2');
			await vfs.writeFile('vfs:///f.txt', 'v3');
			await vfs.writeFile('vfs:///f.txt', 'v4');
			await vfs.writeFile('vfs:///f.txt', 'v5');

			const hist = await vfs.history('vfs:///f.txt');
			expect(hist.length).toBe(3);
			// Oldest entries should be trimmed
			expect(hist[0].text).toBe('v2');
		});

		it('clears history when file is deleted', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///f.txt', 'v1');
			await vfs.writeFile('vfs:///f.txt', 'v2');
			await vfs.deleteFile('vfs:///f.txt');

			// Re-create file
			await vfs.writeFile('vfs:///f.txt', 'fresh');
			expect(await vfs.history('vfs:///f.txt')).toEqual([]);
		});

		it('clears history on rmdir recursive', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///dir');
			await vfs.writeFile('vfs:///dir/f.txt', 'v1');
			await vfs.writeFile('vfs:///dir/f.txt', 'v2');
			await vfs.rmdir('vfs:///dir', { recursive: true });

			await vfs.mkdir('vfs:///dir');
			await vfs.writeFile('vfs:///dir/f.txt', 'fresh');
			expect(await vfs.history('vfs:///dir/f.txt')).toEqual([]);
		});

		it('transfers history on rename', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///a.txt', 'v1');
			await vfs.writeFile('vfs:///a.txt', 'v2');
			await vfs.rename('vfs:///a.txt', 'vfs:///b.txt');

			const hist = await vfs.history('vfs:///b.txt');
			expect(hist.length).toBe(1);
			expect(hist[0].text).toBe('v1');
		});

		it('clears history on clear', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///f.txt', 'v1');
			await vfs.writeFile('vfs:///f.txt', 'v2');
			await vfs.clear();

			await vfs.writeFile('vfs:///f.txt', 'fresh');
			expect(await vfs.history('vfs:///f.txt')).toEqual([]);
		});

		it('returns frozen array', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///f.txt', 'v1');
			await vfs.writeFile('vfs:///f.txt', 'v2');
			expect(Object.isFrozen(await vfs.history('vfs:///f.txt'))).toBe(true);
		});

		it('throws VFS_NOT_FOUND for non-existent file', async () => {
			const vfs = await createFS();
			await expectGuardedThrow(
				() => vfs.history('vfs:///missing.txt'),
				isVFSError,
				'VFS_NOT_FOUND',
			);
		});

		it('throws VFS_NOT_FILE for a directory', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///dir');
			await expectGuardedThrow(
				() => vfs.history('vfs:///dir'),
				isVFSError,
				'VFS_NOT_FILE',
			);
		});
	});

	// -- diff -------------------------------------------------------------

	describe('diff', () => {
		it('diffs two identical files with no hunks', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///a.txt', 'hello\nworld');
			await vfs.writeFile('vfs:///b.txt', 'hello\nworld');

			const result = await vfs.diff('vfs:///a.txt', 'vfs:///b.txt');
			expect(result.additions).toBe(0);
			expect(result.deletions).toBe(0);
			expect(result.hunks.length).toBe(0);
		});

		it('detects additions', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///a.txt', 'line1\nline2');
			await vfs.writeFile('vfs:///b.txt', 'line1\nline2\nline3');

			const result = await vfs.diff('vfs:///a.txt', 'vfs:///b.txt');
			expect(result.additions).toBe(1);
			expect(result.deletions).toBe(0);
			expect(result.oldPath).toBe('vfs:///a.txt');
			expect(result.newPath).toBe('vfs:///b.txt');
		});

		it('detects deletions', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///a.txt', 'line1\nline2\nline3');
			await vfs.writeFile('vfs:///b.txt', 'line1\nline3');

			const result = await vfs.diff('vfs:///a.txt', 'vfs:///b.txt');
			expect(result.deletions).toBe(1);
			expect(result.additions).toBe(0);
		});

		it('detects both additions and deletions', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///a.txt', 'aaa\nbbb\nccc');
			await vfs.writeFile('vfs:///b.txt', 'aaa\nxxx\nccc');

			const result = await vfs.diff('vfs:///a.txt', 'vfs:///b.txt');
			expect(result.additions).toBeGreaterThanOrEqual(1);
			expect(result.deletions).toBeGreaterThanOrEqual(1);
		});

		it('handles empty file comparisons', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///a.txt', '');
			await vfs.writeFile('vfs:///b.txt', 'hello');

			const result = await vfs.diff('vfs:///a.txt', 'vfs:///b.txt');
			expect(result.additions).toBe(1);
			expect(result.deletions).toBe(1);
		});

		it('respects context option', async () => {
			const vfs = await createFS();
			const lines = Array.from({ length: 20 }, (_, i) => `line${i + 1}`);
			await vfs.writeFile('vfs:///a.txt', lines.join('\n'));

			const modified = [...lines];
			modified[10] = 'CHANGED';
			await vfs.writeFile('vfs:///b.txt', modified.join('\n'));

			const result1 = await vfs.diff('vfs:///a.txt', 'vfs:///b.txt', {
				context: 1,
			});
			const result2 = await vfs.diff('vfs:///a.txt', 'vfs:///b.txt', {
				context: 5,
			});

			// More context = more lines in hunks
			const hunkLines1 = result1.hunks.reduce(
				(sum, h) => sum + h.lines.length,
				0,
			);
			const hunkLines2 = result2.hunks.reduce(
				(sum, h) => sum + h.lines.length,
				0,
			);
			expect(hunkLines2).toBeGreaterThanOrEqual(hunkLines1);
		});

		it('returns frozen result', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///a.txt', 'hello');
			await vfs.writeFile('vfs:///b.txt', 'world');
			const result = await vfs.diff('vfs:///a.txt', 'vfs:///b.txt');
			expect(Object.isFrozen(result)).toBe(true);
			expect(Object.isFrozen(result.hunks)).toBe(true);
		});

		it('throws VFS_NOT_FOUND for non-existent file', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///a.txt', 'hello');
			await expectGuardedThrow(
				() => vfs.diff('vfs:///a.txt', 'vfs:///missing.txt'),
				isVFSError,
				'VFS_NOT_FOUND',
			);
		});

		it('throws VFS_NOT_FILE for a directory', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///a.txt', 'hello');
			await vfs.mkdir('vfs:///dir');
			await expectGuardedThrow(
				() => vfs.diff('vfs:///a.txt', 'vfs:///dir'),
				isVFSError,
				'VFS_NOT_FILE',
			);
		});

		it('hunk line numbers are correct', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///a.txt', 'aaa\nbbb\nccc');
			await vfs.writeFile('vfs:///b.txt', 'aaa\nbbb\nccc\nddd');

			const result = await vfs.diff('vfs:///a.txt', 'vfs:///b.txt');
			expect(result.hunks.length).toBeGreaterThan(0);
			const hunk = result.hunks[0];
			expect(hunk.oldStart).toBeGreaterThan(0);
			expect(hunk.newStart).toBeGreaterThan(0);
		});
	});

	// -- diffVersions -----------------------------------------------------

	describe('diffVersions', () => {
		it('diffs two history versions', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///f.txt', 'line1\nline2');
			await vfs.writeFile('vfs:///f.txt', 'line1\nline2\nline3');
			await vfs.writeFile('vfs:///f.txt', 'line1\nline3');

			// v1 = 'line1\nline2', v2 = 'line1\nline2\nline3', current (v3) = 'line1\nline3'
			const result = await vfs.diffVersions('vfs:///f.txt', 1, 2);
			expect(result.additions).toBe(1);
			expect(result.oldPath).toContain('@v1');
			expect(result.newPath).toContain('@v2');
		});

		it('diffs a history version against current', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///f.txt', 'old');
			await vfs.writeFile('vfs:///f.txt', 'new');

			const result = await vfs.diffVersions('vfs:///f.txt', 1);
			expect(result.oldPath).toContain('@v1');
			expect(result.newPath).toContain('@v2');
		});

		it('throws VFS_NOT_FOUND for invalid version', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///f.txt', 'hello');

			await expectGuardedThrow(
				() => vfs.diffVersions('vfs:///f.txt', 5),
				isVFSError,
				'VFS_NOT_FOUND',
			);
		});

		it('returns no changes when diffing same version', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///f.txt', 'hello');
			await vfs.writeFile('vfs:///f.txt', 'world');

			const result = await vfs.diffVersions('vfs:///f.txt', 1, 1);
			expect(result.additions).toBe(0);
			expect(result.deletions).toBe(0);
		});

		it('throws VFS_NOT_FOUND for non-existent file', async () => {
			const vfs = await createFS();
			await expectGuardedThrow(
				() => vfs.diffVersions('vfs:///missing.txt', 1),
				isVFSError,
				'VFS_NOT_FOUND',
			);
		});
	});

	// -- checkout ---------------------------------------------------------

	describe('checkout', () => {
		it('restores a previous version of a file', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///f.txt', 'version1');
			await vfs.writeFile('vfs:///f.txt', 'version2');
			await vfs.writeFile('vfs:///f.txt', 'version3');

			// History: v1='version1', v2='version2', current(v3)='version3'
			await vfs.checkout('vfs:///f.txt', 1);
			expect((await vfs.readFile('vfs:///f.txt')).text).toBe('version1');
		});

		it('records current state as history before checkout', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///f.txt', 'v1');
			await vfs.writeFile('vfs:///f.txt', 'v2');

			// History: [v1='v1'], current(v2)='v2'
			await vfs.checkout('vfs:///f.txt', 1);

			// After checkout, 'v2' should be in history
			const hist = await vfs.history('vfs:///f.txt');
			expect(hist.some((h) => h.text === 'v2')).toBe(true);
		});

		it('is a no-op when checking out current version', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///f.txt', 'v1');
			await vfs.writeFile('vfs:///f.txt', 'v2');

			// Current version is 2 (history has 1 entry)
			await vfs.checkout('vfs:///f.txt', 2);
			expect((await vfs.readFile('vfs:///f.txt')).text).toBe('v2');
		});

		it('updates totalSize after checkout', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///f.txt', 'short');
			await vfs.writeFile('vfs:///f.txt', 'this is a much longer string');

			const sizeBefore = (await vfs.metrics()).totalSize;
			await vfs.checkout('vfs:///f.txt', 1);
			expect((await vfs.metrics()).totalSize).toBeLessThan(sizeBefore);
		});

		it('handles binary file checkout', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///bin.dat', new Uint8Array([1, 2, 3]));
			await vfs.writeFile('vfs:///bin.dat', new Uint8Array([4, 5, 6, 7]));

			await vfs.checkout('vfs:///bin.dat', 1);
			const result = await vfs.readFile('vfs:///bin.dat');
			expect(result.contentType).toBe('binary');
			expect(result.data).toEqual(new Uint8Array([1, 2, 3]));
		});

		it('throws VFS_NOT_FOUND for invalid version', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///f.txt', 'hello');

			await expectGuardedThrow(
				() => vfs.checkout('vfs:///f.txt', 99),
				isVFSError,
				'VFS_NOT_FOUND',
			);
		});

		it('throws VFS_NOT_FOUND for non-existent file', async () => {
			const vfs = await createFS();
			await expectGuardedThrow(
				() => vfs.checkout('vfs:///missing.txt', 1),
				isVFSError,
				'VFS_NOT_FOUND',
			);
		});

		it('throws VFS_NOT_FILE for a directory', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///dir');
			await expectGuardedThrow(
				() => vfs.checkout('vfs:///dir', 1),
				isVFSError,
				'VFS_NOT_FILE',
			);
		});
	});

	// -- search -----------------------------------------------------------

	describe('search', () => {
		it('finds substring matches in text files', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///f.txt', 'hello world\nfoo bar\nhello again');

			const results = (await vfs.search('hello')) as readonly VFSSearchResult[];
			expect(results.length).toBe(2);
			expect(results[0].path).toBe('vfs:///f.txt');
			expect(results[0].line).toBe(1);
			expect(results[0].column).toBe(1);
			expect(results[1].line).toBe(3);
		});

		it('returns line and column accurately', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///f.txt', 'abcXYZdef');
			const results = (await vfs.search('XYZ')) as readonly VFSSearchResult[];
			expect(results.length).toBe(1);
			expect(results[0].line).toBe(1);
			expect(results[0].column).toBe(4);
			expect(results[0].match).toBe('abcXYZdef');
		});

		it('respects maxResults', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///f.txt', 'aaa\naaa\naaa\naaa\naaa');
			const results = (await vfs.search('aaa', {
				maxResults: 2,
			})) as readonly VFSSearchResult[];
			expect(results.length).toBe(2);
		});

		it('filters by glob pattern', async () => {
			const vfs = await createFS();
			await vfs.mkdir('vfs:///src');
			await vfs.writeFile('vfs:///src/a.ts', 'const x = 1;');
			await vfs.writeFile('vfs:///src/b.js', 'const x = 2;');

			const results = (await vfs.search('const', {
				glob: 'vfs:///**/*.ts',
			})) as readonly VFSSearchResult[];
			expect(results.length).toBe(1);
			expect(results[0].path).toBe('vfs:///src/a.ts');
		});

		it('skips binary files', async () => {
			const vfs = await createFS();
			await vfs.writeFile(
				'vfs:///bin.dat',
				new Uint8Array([104, 101, 108, 108, 111]),
			);
			await vfs.writeFile('vfs:///f.txt', 'hello');
			const results = (await vfs.search('hello')) as readonly VFSSearchResult[];
			expect(results.length).toBe(1);
			expect(results[0].path).toBe('vfs:///f.txt');
		});

		it('returns empty array when no matches', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///f.txt', 'hello');
			expect(await vfs.search('xyz')).toEqual([]);
		});

		it('returns frozen results', async () => {
			const vfs = await createFS();
			await vfs.writeFile('vfs:///f.txt', 'hello');
			const results = await vfs.search('hello');
			expect(Object.isFrozen(results)).toBe(true);
		});
	});

	// -----------------------------------------------------------------------
	// onFileWrite callback
	// -----------------------------------------------------------------------

	describe('onFileWrite', () => {
		it('fires on writeFile for new file', async () => {
			const events: any[] = [];
			const vfs = await createFS({ onFileWrite: (e) => events.push(e) });

			await vfs.writeFile('vfs:///f.txt', 'hello');

			expect(events.length).toBe(1);
			expect(events[0].path).toBe('vfs:///f.txt');
			expect(events[0].contentType).toBe('text');
			expect(events[0].size).toBe(5);
			expect(events[0].isNew).toBe(true);
		});

		it('fires on writeFile for overwrite', async () => {
			const events: any[] = [];
			const vfs = await createFS({ onFileWrite: (e) => events.push(e) });

			await vfs.writeFile('vfs:///f.txt', 'old');
			await vfs.writeFile('vfs:///f.txt', 'new content');

			expect(events.length).toBe(2);
			expect(events[1].isNew).toBe(false);
			expect(events[1].size).toBe(11);
		});

		it('fires on writeFile for binary', async () => {
			const events: any[] = [];
			const vfs = await createFS({ onFileWrite: (e) => events.push(e) });

			await vfs.writeFile('vfs:///bin.dat', new Uint8Array([1, 2, 3]));

			expect(events[0].contentType).toBe('binary');
			expect(events[0].size).toBe(3);
			expect(events[0].isNew).toBe(true);
		});

		it('fires on appendFile', async () => {
			const events: any[] = [];
			const vfs = await createFS({ onFileWrite: (e) => events.push(e) });

			await vfs.writeFile('vfs:///f.txt', 'hello');
			await vfs.appendFile('vfs:///f.txt', ' world');

			expect(events.length).toBe(2);
			expect(events[1].path).toBe('vfs:///f.txt');
			expect(events[1].isNew).toBe(false);
			expect(events[1].size).toBe(11);
		});

		it('does not fire when no callback provided', async () => {
			const vfs = await createFS();
			// Should not throw
			await vfs.writeFile('vfs:///f.txt', 'hello');
			await vfs.appendFile('vfs:///f.txt', ' world');
		});
	});
});
