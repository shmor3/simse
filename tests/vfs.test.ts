import { describe, expect, it } from 'bun:test';
import {
	VFS_ROOT,
	VFS_SCHEME,
	ancestorPaths,
	baseName,
	normalizePath,
	parentPath,
	pathDepth,
	toLocalPath,
	validatePath,
} from '../src/ai/vfs/path-utils.js';
import type { VirtualFS } from '../src/ai/vfs/vfs.js';
import { createVirtualFS } from '../src/ai/vfs/vfs.js';
import { isVFSError } from '../src/errors/vfs.js';
import { createLogger, type Logger } from '../src/logger.js';
import { expectGuardedThrow } from './utils/error-helpers';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function createSilentLogger(): Logger {
	return createLogger({ context: 'test', level: 'none', transports: [] });
}

function createFS(
	overrides?: Parameters<typeof createVirtualFS>[0],
): VirtualFS {
	return createVirtualFS({
		logger: createSilentLogger(),
		...overrides,
	});
}

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
			expect(() => normalizePath('')).toThrow(
				'Path must start with vfs://',
			);
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
	it('returns a frozen object', () => {
		const vfs = createFS();
		expect(Object.isFrozen(vfs)).toBe(true);
	});

	it('starts with root directory only', () => {
		const vfs = createFS();
		expect(vfs.exists('/')).toBe(true);
		expect(vfs.nodeCount).toBe(1);
		expect(vfs.directoryCount).toBe(1);
		expect(vfs.fileCount).toBe(0);
		expect(vfs.totalSize).toBe(0);
	});

	// -- writeFile / readFile ---------------------------------------------

	describe('writeFile / readFile', () => {
		it('writes and reads a text file', () => {
			const vfs = createFS();
			vfs.writeFile('/hello.txt', 'Hello, world!');
			const result = vfs.readFile('/hello.txt');
			expect(result.contentType).toBe('text');
			expect(result.text).toBe('Hello, world!');
			expect(result.data).toBeUndefined();
			expect(result.size).toBeGreaterThan(0);
		});

		it('writes and reads a binary file', () => {
			const vfs = createFS();
			const data = new Uint8Array([1, 2, 3, 4, 5]);
			vfs.writeFile('/bin.dat', data);
			const result = vfs.readFile('/bin.dat');
			expect(result.contentType).toBe('binary');
			expect(result.data).toEqual(data);
			expect(result.text).toBeUndefined();
			expect(result.size).toBe(5);
		});

		it('returns a defensive copy for binary data', () => {
			const vfs = createFS();
			const data = new Uint8Array([1, 2, 3]);
			vfs.writeFile('/bin.dat', data);
			const result = vfs.readFile('/bin.dat');
			result.data![0] = 99;
			const result2 = vfs.readFile('/bin.dat');
			expect(result2.data![0]).toBe(1);
		});

		it('overwrites existing file', () => {
			const vfs = createFS();
			vfs.writeFile('/f.txt', 'first');
			vfs.writeFile('/f.txt', 'second');
			expect(vfs.readFile('/f.txt').text).toBe('second');
			expect(vfs.fileCount).toBe(1);
		});

		it('preserves createdAt on overwrite', () => {
			const vfs = createFS();
			vfs.writeFile('/f.txt', 'first');
			const created = vfs.stat('/f.txt').createdAt;
			vfs.writeFile('/f.txt', 'second');
			expect(vfs.stat('/f.txt').createdAt).toBe(created);
		});

		it('writes with createParents', () => {
			const vfs = createFS();
			vfs.writeFile('/a/b/c/file.txt', 'deep', { createParents: true });
			expect(vfs.exists('/a')).toBe(true);
			expect(vfs.exists('/a/b')).toBe(true);
			expect(vfs.exists('/a/b/c')).toBe(true);
			expect(vfs.readFile('/a/b/c/file.txt').text).toBe('deep');
		});

		it('throws VFS_NOT_FOUND without parent directory', () => {
			const vfs = createFS();
			expectGuardedThrow(
				() => vfs.writeFile('/missing/file.txt', 'data'),
				isVFSError,
				'VFS_NOT_FOUND',
			);
		});

		it('throws VFS_LIMIT_EXCEEDED for maxFileSize', () => {
			const vfs = createFS({ limits: { maxFileSize: 5 } });
			expectGuardedThrow(
				() => vfs.writeFile('/big.txt', 'more than five bytes'),
				isVFSError,
				'VFS_LIMIT_EXCEEDED',
			);
		});

		it('throws VFS_LIMIT_EXCEEDED for maxTotalSize', () => {
			const vfs = createFS({ limits: { maxTotalSize: 10 } });
			vfs.writeFile('/a.txt', '12345');
			expectGuardedThrow(
				() => vfs.writeFile('/b.txt', '123456'),
				isVFSError,
				'VFS_LIMIT_EXCEEDED',
			);
		});

		it('throws VFS_INVALID_OPERATION when writing to root', () => {
			const vfs = createFS();
			expectGuardedThrow(
				() => vfs.writeFile('/', 'data'),
				isVFSError,
				'VFS_INVALID_OPERATION',
			);
		});

		it('throws VFS_NOT_FOUND when reading non-existent file', () => {
			const vfs = createFS();
			expectGuardedThrow(
				() => vfs.readFile('/missing.txt'),
				isVFSError,
				'VFS_NOT_FOUND',
			);
		});

		it('throws VFS_NOT_FILE when reading a directory', () => {
			const vfs = createFS();
			vfs.mkdir('/dir');
			expectGuardedThrow(
				() => vfs.readFile('/dir'),
				isVFSError,
				'VFS_NOT_FILE',
			);
		});

		it('throws VFS_NOT_FILE when overwriting directory with file', () => {
			const vfs = createFS();
			vfs.mkdir('/dir');
			expectGuardedThrow(
				() => vfs.writeFile('/dir', 'data'),
				isVFSError,
				'VFS_NOT_FILE',
			);
		});

		it('tracks totalSize correctly through writes', () => {
			const vfs = createFS();
			vfs.writeFile('/a.txt', 'abc');
			const s1 = vfs.totalSize;
			expect(s1).toBe(3);
			vfs.writeFile('/a.txt', 'ab');
			expect(vfs.totalSize).toBe(2);
		});
	});

	// -- appendFile -------------------------------------------------------

	describe('appendFile', () => {
		it('appends to a text file', () => {
			const vfs = createFS();
			vfs.writeFile('/f.txt', 'hello');
			vfs.appendFile('/f.txt', ' world');
			expect(vfs.readFile('/f.txt').text).toBe('hello world');
		});

		it('throws VFS_INVALID_OPERATION on binary file', () => {
			const vfs = createFS();
			vfs.writeFile('/bin.dat', new Uint8Array([1]));
			expectGuardedThrow(
				() => vfs.appendFile('/bin.dat', 'text'),
				isVFSError,
				'VFS_INVALID_OPERATION',
			);
		});

		it('throws VFS_NOT_FOUND on non-existent file', () => {
			const vfs = createFS();
			expectGuardedThrow(
				() => vfs.appendFile('/missing.txt', 'text'),
				isVFSError,
				'VFS_NOT_FOUND',
			);
		});

		it('updates totalSize after append', () => {
			const vfs = createFS();
			vfs.writeFile('/f.txt', 'abc');
			vfs.appendFile('/f.txt', 'de');
			expect(vfs.totalSize).toBe(5);
		});
	});

	// -- deleteFile -------------------------------------------------------

	describe('deleteFile', () => {
		it('deletes an existing file', () => {
			const vfs = createFS();
			vfs.writeFile('/f.txt', 'data');
			expect(vfs.deleteFile('/f.txt')).toBe(true);
			expect(vfs.exists('/f.txt')).toBe(false);
			expect(vfs.fileCount).toBe(0);
		});

		it('returns false for non-existent file', () => {
			const vfs = createFS();
			expect(vfs.deleteFile('/missing.txt')).toBe(false);
		});

		it('throws VFS_NOT_FILE when deleting a directory', () => {
			const vfs = createFS();
			vfs.mkdir('/dir');
			expectGuardedThrow(
				() => vfs.deleteFile('/dir'),
				isVFSError,
				'VFS_NOT_FILE',
			);
		});

		it('updates totalSize after delete', () => {
			const vfs = createFS();
			vfs.writeFile('/f.txt', 'abc');
			expect(vfs.totalSize).toBe(3);
			vfs.deleteFile('/f.txt');
			expect(vfs.totalSize).toBe(0);
		});
	});

	// -- mkdir ------------------------------------------------------------

	describe('mkdir', () => {
		it('creates a directory', () => {
			const vfs = createFS();
			vfs.mkdir('/dir');
			expect(vfs.exists('/dir')).toBe(true);
			expect(vfs.stat('/dir').type).toBe('directory');
		});

		it('creates nested directories with recursive', () => {
			const vfs = createFS();
			vfs.mkdir('/a/b/c', { recursive: true });
			expect(vfs.exists('/a')).toBe(true);
			expect(vfs.exists('/a/b')).toBe(true);
			expect(vfs.exists('/a/b/c')).toBe(true);
		});

		it('throws VFS_NOT_FOUND without parent directory', () => {
			const vfs = createFS();
			expectGuardedThrow(
				() => vfs.mkdir('/missing/dir'),
				isVFSError,
				'VFS_NOT_FOUND',
			);
		});

		it('is idempotent for existing directory', () => {
			const vfs = createFS();
			vfs.mkdir('/dir');
			vfs.mkdir('/dir');
			expect(vfs.directoryCount).toBe(2);
		});

		it('throws VFS_NOT_DIRECTORY when path is a file', () => {
			const vfs = createFS();
			vfs.writeFile('/f.txt', 'data');
			expectGuardedThrow(
				() => vfs.mkdir('/f.txt'),
				isVFSError,
				'VFS_NOT_DIRECTORY',
			);
		});

		it('is a no-op for root', () => {
			const vfs = createFS();
			vfs.mkdir('/');
			expect(vfs.directoryCount).toBe(1);
		});
	});

	// -- readdir ----------------------------------------------------------

	describe('readdir', () => {
		it('lists empty directory', () => {
			const vfs = createFS();
			vfs.mkdir('/dir');
			expect(vfs.readdir('/dir')).toEqual([]);
		});

		it('lists files and subdirectories', () => {
			const vfs = createFS();
			vfs.mkdir('/dir');
			vfs.writeFile('/dir/a.txt', 'a');
			vfs.mkdir('/dir/sub');
			const entries = vfs.readdir('/dir');
			expect(entries.length).toBe(2);
			const names = entries.map((e) => e.name).sort();
			expect(names).toEqual(['a.txt', 'sub']);
		});

		it('lists recursively', () => {
			const vfs = createFS();
			vfs.mkdir('/dir/sub', { recursive: true });
			vfs.writeFile('/dir/a.txt', 'a');
			vfs.writeFile('/dir/sub/b.txt', 'b');
			const entries = vfs.readdir('/dir', { recursive: true });
			const names = entries.map((e) => e.name).sort();
			expect(names).toEqual(['a.txt', 'sub', 'sub/b.txt']);
		});

		it('throws VFS_NOT_DIRECTORY on a file', () => {
			const vfs = createFS();
			vfs.writeFile('/f.txt', 'data');
			expectGuardedThrow(
				() => vfs.readdir('/f.txt'),
				isVFSError,
				'VFS_NOT_DIRECTORY',
			);
		});

		it('throws VFS_NOT_FOUND on non-existent path', () => {
			const vfs = createFS();
			expectGuardedThrow(
				() => vfs.readdir('/missing'),
				isVFSError,
				'VFS_NOT_FOUND',
			);
		});

		it('returns frozen array', () => {
			const vfs = createFS();
			const entries = vfs.readdir('/');
			expect(Object.isFrozen(entries)).toBe(true);
		});
	});

	// -- rmdir ------------------------------------------------------------

	describe('rmdir', () => {
		it('removes empty directory', () => {
			const vfs = createFS();
			vfs.mkdir('/dir');
			expect(vfs.rmdir('/dir')).toBe(true);
			expect(vfs.exists('/dir')).toBe(false);
		});

		it('returns false for non-existent directory', () => {
			const vfs = createFS();
			expect(vfs.rmdir('/missing')).toBe(false);
		});

		it('throws VFS_NOT_EMPTY without recursive', () => {
			const vfs = createFS();
			vfs.mkdir('/dir');
			vfs.writeFile('/dir/f.txt', 'data');
			expectGuardedThrow(() => vfs.rmdir('/dir'), isVFSError, 'VFS_NOT_EMPTY');
		});

		it('removes non-empty directory with recursive', () => {
			const vfs = createFS();
			vfs.mkdir('/dir/sub', { recursive: true });
			vfs.writeFile('/dir/a.txt', 'a');
			vfs.writeFile('/dir/sub/b.txt', 'b');
			expect(vfs.rmdir('/dir', { recursive: true })).toBe(true);
			expect(vfs.exists('/dir')).toBe(false);
			expect(vfs.exists('/dir/sub')).toBe(false);
			expect(vfs.exists('/dir/a.txt')).toBe(false);
		});

		it('updates totalSize after recursive delete', () => {
			const vfs = createFS();
			vfs.mkdir('/dir');
			vfs.writeFile('/dir/a.txt', 'abc');
			vfs.writeFile('/dir/b.txt', 'de');
			expect(vfs.totalSize).toBe(5);
			vfs.rmdir('/dir', { recursive: true });
			expect(vfs.totalSize).toBe(0);
		});

		it('throws VFS_INVALID_OPERATION when deleting root', () => {
			const vfs = createFS();
			expectGuardedThrow(
				() => vfs.rmdir('/'),
				isVFSError,
				'VFS_INVALID_OPERATION',
			);
		});

		it('throws VFS_NOT_DIRECTORY when path is a file', () => {
			const vfs = createFS();
			vfs.writeFile('/f.txt', 'data');
			expectGuardedThrow(
				() => vfs.rmdir('/f.txt'),
				isVFSError,
				'VFS_NOT_DIRECTORY',
			);
		});
	});

	// -- stat -------------------------------------------------------------

	describe('stat', () => {
		it('returns stat for a file', () => {
			const vfs = createFS();
			vfs.writeFile('/f.txt', 'hello');
			const s = vfs.stat('/f.txt');
			expect(s.type).toBe('file');
			expect(s.path).toBe('/f.txt');
			expect(s.size).toBe(5);
			expect(s.createdAt).toBeGreaterThan(0);
			expect(s.modifiedAt).toBeGreaterThanOrEqual(s.createdAt);
		});

		it('returns stat for a directory', () => {
			const vfs = createFS();
			vfs.mkdir('/dir');
			const s = vfs.stat('/dir');
			expect(s.type).toBe('directory');
			expect(s.size).toBe(0);
		});

		it('throws VFS_NOT_FOUND for non-existent path', () => {
			const vfs = createFS();
			expectGuardedThrow(
				() => vfs.stat('/missing'),
				isVFSError,
				'VFS_NOT_FOUND',
			);
		});

		it('returns frozen object', () => {
			const vfs = createFS();
			vfs.writeFile('/f.txt', 'data');
			expect(Object.isFrozen(vfs.stat('/f.txt'))).toBe(true);
		});
	});

	// -- exists -----------------------------------------------------------

	describe('exists', () => {
		it('returns true for existing file', () => {
			const vfs = createFS();
			vfs.writeFile('/f.txt', 'data');
			expect(vfs.exists('/f.txt')).toBe(true);
		});

		it('returns true for existing directory', () => {
			const vfs = createFS();
			vfs.mkdir('/dir');
			expect(vfs.exists('/dir')).toBe(true);
		});

		it('returns false for non-existent path', () => {
			const vfs = createFS();
			expect(vfs.exists('/nope')).toBe(false);
		});

		it('returns true for root', () => {
			const vfs = createFS();
			expect(vfs.exists('/')).toBe(true);
		});
	});

	// -- rename -----------------------------------------------------------

	describe('rename', () => {
		it('renames a file', () => {
			const vfs = createFS();
			vfs.writeFile('/a.txt', 'data');
			vfs.rename('/a.txt', '/b.txt');
			expect(vfs.exists('/a.txt')).toBe(false);
			expect(vfs.readFile('/b.txt').text).toBe('data');
		});

		it('renames a directory and moves descendants', () => {
			const vfs = createFS();
			vfs.mkdir('/old/sub', { recursive: true });
			vfs.writeFile('/old/sub/f.txt', 'content');
			vfs.rename('/old', '/new');
			expect(vfs.exists('/old')).toBe(false);
			expect(vfs.exists('/old/sub')).toBe(false);
			expect(vfs.exists('/new')).toBe(true);
			expect(vfs.exists('/new/sub')).toBe(true);
			expect(vfs.readFile('/new/sub/f.txt').text).toBe('content');
		});

		it('throws VFS_ALREADY_EXISTS when destination exists', () => {
			const vfs = createFS();
			vfs.writeFile('/a.txt', 'a');
			vfs.writeFile('/b.txt', 'b');
			expectGuardedThrow(
				() => vfs.rename('/a.txt', '/b.txt'),
				isVFSError,
				'VFS_ALREADY_EXISTS',
			);
		});

		it('throws VFS_INVALID_OPERATION when renaming root', () => {
			const vfs = createFS();
			expectGuardedThrow(
				() => vfs.rename('/', '/new'),
				isVFSError,
				'VFS_INVALID_OPERATION',
			);
		});

		it('throws VFS_NOT_FOUND when source does not exist', () => {
			const vfs = createFS();
			expectGuardedThrow(
				() => vfs.rename('/missing', '/new'),
				isVFSError,
				'VFS_NOT_FOUND',
			);
		});

		it('throws VFS_NOT_FOUND when dest parent does not exist', () => {
			const vfs = createFS();
			vfs.writeFile('/a.txt', 'data');
			expectGuardedThrow(
				() => vfs.rename('/a.txt', '/missing/b.txt'),
				isVFSError,
				'VFS_NOT_FOUND',
			);
		});
	});

	// -- copy -------------------------------------------------------------

	describe('copy', () => {
		it('copies a file', () => {
			const vfs = createFS();
			vfs.writeFile('/a.txt', 'data');
			vfs.copy('/a.txt', '/b.txt');
			expect(vfs.readFile('/a.txt').text).toBe('data');
			expect(vfs.readFile('/b.txt').text).toBe('data');
			expect(vfs.fileCount).toBe(2);
		});

		it('copies a file with overwrite', () => {
			const vfs = createFS();
			vfs.writeFile('/a.txt', 'first');
			vfs.writeFile('/b.txt', 'second');
			vfs.copy('/a.txt', '/b.txt', { overwrite: true });
			expect(vfs.readFile('/b.txt').text).toBe('first');
		});

		it('throws VFS_ALREADY_EXISTS without overwrite', () => {
			const vfs = createFS();
			vfs.writeFile('/a.txt', 'a');
			vfs.writeFile('/b.txt', 'b');
			expectGuardedThrow(
				() => vfs.copy('/a.txt', '/b.txt'),
				isVFSError,
				'VFS_ALREADY_EXISTS',
			);
		});

		it('copies a directory recursively', () => {
			const vfs = createFS();
			vfs.mkdir('/src/sub', { recursive: true });
			vfs.writeFile('/src/a.txt', 'a');
			vfs.writeFile('/src/sub/b.txt', 'b');
			vfs.copy('/src', '/dst', { recursive: true });
			expect(vfs.exists('/dst')).toBe(true);
			expect(vfs.exists('/dst/sub')).toBe(true);
			expect(vfs.readFile('/dst/a.txt').text).toBe('a');
			expect(vfs.readFile('/dst/sub/b.txt').text).toBe('b');
			// Source still exists
			expect(vfs.readFile('/src/a.txt').text).toBe('a');
		});

		it('throws VFS_INVALID_OPERATION when copying dir without recursive', () => {
			const vfs = createFS();
			vfs.mkdir('/src');
			expectGuardedThrow(
				() => vfs.copy('/src', '/dst'),
				isVFSError,
				'VFS_INVALID_OPERATION',
			);
		});

		it('copies binary files', () => {
			const vfs = createFS();
			const data = new Uint8Array([10, 20, 30]);
			vfs.writeFile('/bin.dat', data);
			vfs.copy('/bin.dat', '/copy.dat');
			expect(vfs.readFile('/copy.dat').data).toEqual(data);
		});
	});

	// -- snapshot / restore -----------------------------------------------

	describe('snapshot / restore', () => {
		it('round-trips text files', () => {
			const vfs = createFS();
			vfs.mkdir('/dir');
			vfs.writeFile('/dir/f.txt', 'hello');
			const snap = vfs.snapshot();

			const vfs2 = createFS();
			vfs2.restore(snap);
			expect(vfs2.readFile('/dir/f.txt').text).toBe('hello');
			expect(vfs2.exists('/dir')).toBe(true);
		});

		it('round-trips binary files', () => {
			const vfs = createFS();
			const data = new Uint8Array([255, 0, 128]);
			vfs.writeFile('/bin.dat', data);
			const snap = vfs.snapshot();

			const vfs2 = createFS();
			vfs2.restore(snap);
			expect(vfs2.readFile('/bin.dat').data).toEqual(data);
			expect(vfs2.readFile('/bin.dat').contentType).toBe('binary');
		});

		it('preserves timestamps', () => {
			const vfs = createFS();
			vfs.writeFile('/f.txt', 'data');
			const original = vfs.stat('/f.txt');
			const snap = vfs.snapshot();

			const vfs2 = createFS();
			vfs2.restore(snap);
			const restored = vfs2.stat('/f.txt');
			expect(restored.createdAt).toBe(original.createdAt);
			expect(restored.modifiedAt).toBe(original.modifiedAt);
		});

		it('clear resets to empty root', () => {
			const vfs = createFS();
			vfs.mkdir('/dir');
			vfs.writeFile('/dir/f.txt', 'data');
			vfs.clear();
			expect(vfs.exists('/')).toBe(true);
			expect(vfs.nodeCount).toBe(1);
			expect(vfs.totalSize).toBe(0);
		});

		it('snapshot is frozen', () => {
			const vfs = createFS();
			const snap = vfs.snapshot();
			expect(Object.isFrozen(snap)).toBe(true);
			expect(Object.isFrozen(snap.files)).toBe(true);
			expect(Object.isFrozen(snap.directories)).toBe(true);
		});

		it('snapshot does not include root directory', () => {
			const vfs = createFS();
			const snap = vfs.snapshot();
			expect(snap.directories.length).toBe(0);
			expect(snap.files.length).toBe(0);
		});
	});

	// -- limits -----------------------------------------------------------

	describe('limits', () => {
		it('enforces maxNodeCount', () => {
			const vfs = createFS({ limits: { maxNodeCount: 3 } });
			// Root is 1, dir is 2, file is 3
			vfs.mkdir('/dir');
			vfs.writeFile('/dir/f.txt', 'data');
			expectGuardedThrow(
				() => vfs.writeFile('/dir/g.txt', 'more'),
				isVFSError,
				'VFS_LIMIT_EXCEEDED',
			);
		});

		it('enforces maxPathDepth', () => {
			const vfs = createFS({ limits: { maxPathDepth: 2 } });
			vfs.mkdir('/a/b', { recursive: true });
			expectGuardedThrow(
				() => vfs.mkdir('/a/b/c'),
				isVFSError,
				'VFS_INVALID_PATH',
			);
		});

		it('enforces maxNameLength', () => {
			const vfs = createFS({ limits: { maxNameLength: 5 } });
			expectGuardedThrow(
				() => vfs.mkdir('/toolong'),
				isVFSError,
				'VFS_INVALID_PATH',
			);
			vfs.mkdir('/short');
			expect(vfs.exists('/short')).toBe(true);
		});

		it('enforces maxPathLength', () => {
			const vfs = createFS({ limits: { maxPathLength: 10 } });
			expectGuardedThrow(
				() => vfs.mkdir('/this-is-way-too-long'),
				isVFSError,
				'VFS_INVALID_PATH',
			);
		});

		it('enforces maxFileSize', () => {
			const vfs = createFS({ limits: { maxFileSize: 3 } });
			vfs.writeFile('/ok.txt', 'abc');
			expectGuardedThrow(
				() => vfs.writeFile('/big.txt', 'abcd'),
				isVFSError,
				'VFS_LIMIT_EXCEEDED',
			);
		});

		it('enforces maxTotalSize across multiple files', () => {
			const vfs = createFS({ limits: { maxTotalSize: 10 } });
			vfs.writeFile('/a.txt', '12345');
			vfs.writeFile('/b.txt', '12345');
			expectGuardedThrow(
				() => vfs.writeFile('/c.txt', '1'),
				isVFSError,
				'VFS_LIMIT_EXCEEDED',
			);
		});

		it('allows overwrite that reduces size within limits', () => {
			const vfs = createFS({ limits: { maxTotalSize: 10 } });
			vfs.writeFile('/a.txt', '1234567890');
			// Overwrite with smaller content should be fine
			vfs.writeFile('/a.txt', 'small');
			expect(vfs.totalSize).toBe(5);
		});
	});

	// -- metrics ----------------------------------------------------------

	describe('metrics', () => {
		it('tracks nodeCount accurately', () => {
			const vfs = createFS();
			expect(vfs.nodeCount).toBe(1); // root
			vfs.mkdir('/dir');
			expect(vfs.nodeCount).toBe(2);
			vfs.writeFile('/dir/f.txt', 'data');
			expect(vfs.nodeCount).toBe(3);
			vfs.deleteFile('/dir/f.txt');
			expect(vfs.nodeCount).toBe(2);
		});

		it('tracks fileCount and directoryCount', () => {
			const vfs = createFS();
			vfs.mkdir('/a');
			vfs.mkdir('/b');
			vfs.writeFile('/a/f1.txt', '1');
			vfs.writeFile('/a/f2.txt', '2');
			expect(vfs.fileCount).toBe(2);
			expect(vfs.directoryCount).toBe(3); // root + a + b
		});

		it('tracks totalSize through operations', () => {
			const vfs = createFS();
			expect(vfs.totalSize).toBe(0);
			vfs.writeFile('/a.txt', 'abc');
			expect(vfs.totalSize).toBe(3);
			vfs.appendFile('/a.txt', 'de');
			expect(vfs.totalSize).toBe(5);
			vfs.writeFile('/a.txt', 'x');
			expect(vfs.totalSize).toBe(1);
			vfs.deleteFile('/a.txt');
			expect(vfs.totalSize).toBe(0);
		});
	});

	// -- path normalization in API ----------------------------------------

	describe('path normalization', () => {
		it('normalizes paths on write and read', () => {
			const vfs = createFS();
			vfs.mkdir('/bar');
			vfs.writeFile('/foo/../bar/./file.txt', 'data');
			expect(vfs.readFile('/bar/file.txt').text).toBe('data');
		});

		it('handles paths without leading slash', () => {
			const vfs = createFS();
			vfs.writeFile('file.txt', 'data');
			expect(vfs.readFile('/file.txt').text).toBe('data');
		});
	});

	// -- rename: self-descendant guard ------------------------------------

	describe('rename into own descendant', () => {
		it('throws VFS_INVALID_OPERATION when moving dir into its own child', () => {
			const vfs = createFS();
			vfs.mkdir('/a/b', { recursive: true });
			expectGuardedThrow(
				() => vfs.rename('/a', '/a/b/c'),
				isVFSError,
				'VFS_INVALID_OPERATION',
			);
		});

		it('allows renaming to a sibling path that shares a prefix', () => {
			const vfs = createFS();
			vfs.mkdir('/abc');
			vfs.rename('/abc', '/abcdef');
			expect(vfs.exists('/abcdef')).toBe(true);
			expect(vfs.exists('/abc')).toBe(false);
		});

		it('is a no-op when old and new paths are the same', () => {
			const vfs = createFS();
			vfs.writeFile('/f.txt', 'data');
			vfs.rename('/f.txt', '/f.txt');
			expect(vfs.readFile('/f.txt').text).toBe('data');
		});
	});

	// -- copy overwrite on directories ------------------------------------

	describe('copy overwrite on directories', () => {
		it('replaces dest directory contents with overwrite + recursive', () => {
			const vfs = createFS();
			vfs.mkdir('/src');
			vfs.writeFile('/src/a.txt', 'source-a');

			vfs.mkdir('/dst');
			vfs.writeFile('/dst/stale.txt', 'should-be-removed');

			vfs.copy('/src', '/dst', { overwrite: true, recursive: true });

			expect(vfs.readFile('/dst/a.txt').text).toBe('source-a');
			expect(vfs.exists('/dst/stale.txt')).toBe(false);
		});

		it('works when dest does not exist', () => {
			const vfs = createFS();
			vfs.mkdir('/src');
			vfs.writeFile('/src/a.txt', 'data');
			vfs.copy('/src', '/dst', { overwrite: true, recursive: true });
			expect(vfs.readFile('/dst/a.txt').text).toBe('data');
		});
	});

	// -- restore with limits ----------------------------------------------

	describe('restore with limits', () => {
		it('throws VFS_LIMIT_EXCEEDED when snapshot exceeds maxNodeCount', () => {
			const vfs = createFS({ limits: { maxNodeCount: 2 } });
			const snap: Parameters<typeof vfs.restore>[0] = {
				files: [
					{
						path: '/a.txt',
						contentType: 'text',
						text: 'a',
						createdAt: 0,
						modifiedAt: 0,
					},
					{
						path: '/b.txt',
						contentType: 'text',
						text: 'b',
						createdAt: 0,
						modifiedAt: 0,
					},
				],
				directories: [],
			};
			expectGuardedThrow(
				() => vfs.restore(snap),
				isVFSError,
				'VFS_LIMIT_EXCEEDED',
			);
		});

		it('throws VFS_LIMIT_EXCEEDED when snapshot exceeds maxFileSize', () => {
			const vfs = createFS({ limits: { maxFileSize: 3 } });
			const snap: Parameters<typeof vfs.restore>[0] = {
				files: [
					{
						path: '/big.txt',
						contentType: 'text',
						text: 'toolong',
						createdAt: 0,
						modifiedAt: 0,
					},
				],
				directories: [],
			};
			expectGuardedThrow(
				() => vfs.restore(snap),
				isVFSError,
				'VFS_LIMIT_EXCEEDED',
			);
		});

		it('throws VFS_LIMIT_EXCEEDED when snapshot exceeds maxTotalSize', () => {
			const vfs = createFS({ limits: { maxTotalSize: 5 } });
			const snap: Parameters<typeof vfs.restore>[0] = {
				files: [
					{
						path: '/a.txt',
						contentType: 'text',
						text: 'aaa',
						createdAt: 0,
						modifiedAt: 0,
					},
					{
						path: '/b.txt',
						contentType: 'text',
						text: 'bbb',
						createdAt: 0,
						modifiedAt: 0,
					},
				],
				directories: [],
			};
			expectGuardedThrow(
				() => vfs.restore(snap),
				isVFSError,
				'VFS_LIMIT_EXCEEDED',
			);
		});

		it('throws VFS_INVALID_PATH when snapshot path exceeds limits', () => {
			const vfs = createFS({ limits: { maxPathDepth: 1 } });
			const snap: Parameters<typeof vfs.restore>[0] = {
				files: [],
				directories: [{ path: '/a/b/c', createdAt: 0, modifiedAt: 0 }],
			};
			expectGuardedThrow(
				() => vfs.restore(snap),
				isVFSError,
				'VFS_INVALID_PATH',
			);
		});

		it('does not partially commit on validation failure', () => {
			const vfs = createFS({ limits: { maxNodeCount: 2 } });
			vfs.writeFile('/existing.txt', 'keep');

			const snap: Parameters<typeof vfs.restore>[0] = {
				files: [
					{
						path: '/a.txt',
						contentType: 'text',
						text: 'a',
						createdAt: 0,
						modifiedAt: 0,
					},
					{
						path: '/b.txt',
						contentType: 'text',
						text: 'b',
						createdAt: 0,
						modifiedAt: 0,
					},
				],
				directories: [],
			};
			try {
				vfs.restore(snap);
			} catch {
				/* expected */
			}

			// Original state should be intact since we validated before committing
			expect(vfs.exists('/existing.txt')).toBe(true);
		});
	});

	// -- discriminated union VFSReadResult --------------------------------

	describe('VFSReadResult discriminated union', () => {
		it('text files have text: string (not undefined)', () => {
			const vfs = createFS();
			vfs.writeFile('/f.txt', 'hello');
			const result = vfs.readFile('/f.txt');
			if (result.contentType === 'text') {
				const text: string = result.text;
				expect(text).toBe('hello');
			}
		});

		it('binary files have data: Uint8Array (not undefined)', () => {
			const vfs = createFS();
			vfs.writeFile('/bin.dat', new Uint8Array([1, 2]));
			const result = vfs.readFile('/bin.dat');
			if (result.contentType === 'binary') {
				const data: Uint8Array = result.data;
				expect(data).toEqual(new Uint8Array([1, 2]));
			}
		});
	});

	// -- tracked counters -------------------------------------------------

	describe('tracked counters', () => {
		it('fileCount and directoryCount stay accurate through operations', () => {
			const vfs = createFS();
			expect(vfs.fileCount).toBe(0);
			expect(vfs.directoryCount).toBe(1); // root

			vfs.mkdir('/a');
			expect(vfs.directoryCount).toBe(2);

			vfs.writeFile('/a/f1.txt', '1');
			vfs.writeFile('/a/f2.txt', '2');
			expect(vfs.fileCount).toBe(2);

			vfs.deleteFile('/a/f1.txt');
			expect(vfs.fileCount).toBe(1);

			vfs.rmdir('/a', { recursive: true });
			expect(vfs.fileCount).toBe(0);
			expect(vfs.directoryCount).toBe(1); // root only
		});

		it('counters reset on clear', () => {
			const vfs = createFS();
			vfs.mkdir('/a');
			vfs.writeFile('/a/f.txt', 'data');
			vfs.clear();
			expect(vfs.fileCount).toBe(0);
			expect(vfs.directoryCount).toBe(1);
		});

		it('counters are correct after restore', () => {
			const vfs = createFS();
			vfs.mkdir('/dir');
			vfs.writeFile('/dir/f.txt', 'data');
			const snap = vfs.snapshot();

			const vfs2 = createFS();
			vfs2.restore(snap);
			expect(vfs2.fileCount).toBe(1);
			expect(vfs2.directoryCount).toBe(2);
		});
	});

	// -- glob -------------------------------------------------------------

	describe('glob', () => {
		it('matches files by extension', () => {
			const vfs = createFS();
			vfs.mkdir('/src');
			vfs.writeFile('/src/a.ts', 'a');
			vfs.writeFile('/src/b.ts', 'b');
			vfs.writeFile('/src/c.js', 'c');

			const results = vfs.glob('/**/*.ts');
			expect(results).toEqual(['/src/a.ts', '/src/b.ts']);
		});

		it('matches with ** for any depth', () => {
			const vfs = createFS();
			vfs.mkdir('/a/b/c', { recursive: true });
			vfs.writeFile('/a/f.txt', '1');
			vfs.writeFile('/a/b/f.txt', '2');
			vfs.writeFile('/a/b/c/f.txt', '3');

			const results = vfs.glob('/**/f.txt');
			expect(results).toEqual(['/a/b/c/f.txt', '/a/b/f.txt', '/a/f.txt']);
		});

		it('matches with ? for single character', () => {
			const vfs = createFS();
			vfs.writeFile('/a.txt', 'a');
			vfs.writeFile('/ab.txt', 'ab');

			const results = vfs.glob('/?.txt');
			expect(results).toEqual(['/a.txt']);
		});

		it('returns empty array when no matches', () => {
			const vfs = createFS();
			vfs.writeFile('/a.txt', 'a');
			expect(vfs.glob('/*.js')).toEqual([]);
		});

		it('returns sorted results', () => {
			const vfs = createFS();
			vfs.writeFile('/c.txt', 'c');
			vfs.writeFile('/a.txt', 'a');
			vfs.writeFile('/b.txt', 'b');
			expect(vfs.glob('/*.txt')).toEqual(['/a.txt', '/b.txt', '/c.txt']);
		});

		it('only matches files, not directories', () => {
			const vfs = createFS();
			vfs.mkdir('/src');
			vfs.writeFile('/src/file.ts', 'data');
			const results = vfs.glob('/**/*');
			expect(results).toEqual(['/src/file.ts']);
		});
	});

	// -- tree -------------------------------------------------------------

	describe('tree', () => {
		it('renders a tree from root', () => {
			const vfs = createFS();
			vfs.mkdir('/src');
			vfs.writeFile('/src/index.ts', 'code');
			vfs.writeFile('/README.md', 'readme');

			const output = vfs.tree();
			expect(output).toContain('/');
			expect(output).toContain('src/');
			expect(output).toContain('index.ts');
			expect(output).toContain('README.md');
			expect(output).toContain('bytes');
		});

		it('renders a subtree', () => {
			const vfs = createFS();
			vfs.mkdir('/a/b', { recursive: true });
			vfs.writeFile('/a/b/f.txt', 'hi');

			const output = vfs.tree('/a');
			expect(output).toContain('a');
			expect(output).toContain('b/');
			expect(output).toContain('f.txt');
		});

		it('uses tree characters', () => {
			const vfs = createFS();
			vfs.writeFile('/a.txt', 'a');
			vfs.writeFile('/b.txt', 'b');

			const output = vfs.tree();
			expect(output).toContain('├──');
			expect(output).toContain('└──');
		});

		it('throws VFS_NOT_FOUND for non-existent path', () => {
			const vfs = createFS();
			expectGuardedThrow(
				() => vfs.tree('/missing'),
				isVFSError,
				'VFS_NOT_FOUND',
			);
		});

		it('throws VFS_NOT_DIRECTORY for a file', () => {
			const vfs = createFS();
			vfs.writeFile('/f.txt', 'data');
			expectGuardedThrow(
				() => vfs.tree('/f.txt'),
				isVFSError,
				'VFS_NOT_DIRECTORY',
			);
		});
	});

	// -- du ---------------------------------------------------------------

	describe('du', () => {
		it('returns file size for a single file', () => {
			const vfs = createFS();
			vfs.writeFile('/f.txt', 'hello');
			expect(vfs.du('/f.txt')).toBe(5);
		});

		it('returns total size of a directory subtree', () => {
			const vfs = createFS();
			vfs.mkdir('/dir');
			vfs.writeFile('/dir/a.txt', 'abc');
			vfs.writeFile('/dir/b.txt', 'de');
			expect(vfs.du('/dir')).toBe(5);
		});

		it('includes nested subdirectory files', () => {
			const vfs = createFS();
			vfs.mkdir('/a/b', { recursive: true });
			vfs.writeFile('/a/f1.txt', 'xx');
			vfs.writeFile('/a/b/f2.txt', 'yyy');
			expect(vfs.du('/a')).toBe(5);
		});

		it('returns 0 for an empty directory', () => {
			const vfs = createFS();
			vfs.mkdir('/empty');
			expect(vfs.du('/empty')).toBe(0);
		});

		it('throws VFS_NOT_FOUND for non-existent path', () => {
			const vfs = createFS();
			expectGuardedThrow(() => vfs.du('/missing'), isVFSError, 'VFS_NOT_FOUND');
		});
	});

	// -- history ----------------------------------------------------------

	describe('history', () => {
		it('returns empty array for a file with no edits', () => {
			const vfs = createFS();
			vfs.writeFile('/f.txt', 'initial');
			expect(vfs.history('/f.txt')).toEqual([]);
		});

		it('records previous version on writeFile overwrite', () => {
			const vfs = createFS();
			vfs.writeFile('/f.txt', 'v1');
			vfs.writeFile('/f.txt', 'v2');

			const hist = vfs.history('/f.txt');
			expect(hist.length).toBe(1);
			expect(hist[0].version).toBe(1);
			expect(hist[0].text).toBe('v1');
			expect(hist[0].contentType).toBe('text');
		});

		it('records multiple versions', () => {
			const vfs = createFS();
			vfs.writeFile('/f.txt', 'v1');
			vfs.writeFile('/f.txt', 'v2');
			vfs.writeFile('/f.txt', 'v3');

			const hist = vfs.history('/f.txt');
			expect(hist.length).toBe(2);
			expect(hist[0].text).toBe('v1');
			expect(hist[1].text).toBe('v2');
		});

		it('records history on appendFile', () => {
			const vfs = createFS();
			vfs.writeFile('/f.txt', 'line1');
			vfs.appendFile('/f.txt', '\nline2');

			const hist = vfs.history('/f.txt');
			expect(hist.length).toBe(1);
			expect(hist[0].text).toBe('line1');
		});

		it('tracks binary file history', () => {
			const vfs = createFS();
			vfs.writeFile('/bin.dat', new Uint8Array([1, 2, 3]));
			vfs.writeFile('/bin.dat', new Uint8Array([4, 5, 6]));

			const hist = vfs.history('/bin.dat');
			expect(hist.length).toBe(1);
			expect(hist[0].contentType).toBe('binary');
			expect(hist[0].base64).toBeDefined();
		});

		it('respects maxEntriesPerFile limit', () => {
			const vfs = createFS({ history: { maxEntriesPerFile: 3 } });
			vfs.writeFile('/f.txt', 'v1');
			vfs.writeFile('/f.txt', 'v2');
			vfs.writeFile('/f.txt', 'v3');
			vfs.writeFile('/f.txt', 'v4');
			vfs.writeFile('/f.txt', 'v5');

			const hist = vfs.history('/f.txt');
			expect(hist.length).toBe(3);
			// Oldest entries should be trimmed
			expect(hist[0].text).toBe('v2');
		});

		it('clears history when file is deleted', () => {
			const vfs = createFS();
			vfs.writeFile('/f.txt', 'v1');
			vfs.writeFile('/f.txt', 'v2');
			vfs.deleteFile('/f.txt');

			// Re-create file
			vfs.writeFile('/f.txt', 'fresh');
			expect(vfs.history('/f.txt')).toEqual([]);
		});

		it('clears history on rmdir recursive', () => {
			const vfs = createFS();
			vfs.mkdir('/dir');
			vfs.writeFile('/dir/f.txt', 'v1');
			vfs.writeFile('/dir/f.txt', 'v2');
			vfs.rmdir('/dir', { recursive: true });

			vfs.mkdir('/dir');
			vfs.writeFile('/dir/f.txt', 'fresh');
			expect(vfs.history('/dir/f.txt')).toEqual([]);
		});

		it('transfers history on rename', () => {
			const vfs = createFS();
			vfs.writeFile('/a.txt', 'v1');
			vfs.writeFile('/a.txt', 'v2');
			vfs.rename('/a.txt', '/b.txt');

			const hist = vfs.history('/b.txt');
			expect(hist.length).toBe(1);
			expect(hist[0].text).toBe('v1');
		});

		it('clears history on clear', () => {
			const vfs = createFS();
			vfs.writeFile('/f.txt', 'v1');
			vfs.writeFile('/f.txt', 'v2');
			vfs.clear();

			vfs.writeFile('/f.txt', 'fresh');
			expect(vfs.history('/f.txt')).toEqual([]);
		});

		it('returns frozen array', () => {
			const vfs = createFS();
			vfs.writeFile('/f.txt', 'v1');
			vfs.writeFile('/f.txt', 'v2');
			expect(Object.isFrozen(vfs.history('/f.txt'))).toBe(true);
		});

		it('throws VFS_NOT_FOUND for non-existent file', () => {
			const vfs = createFS();
			expectGuardedThrow(
				() => vfs.history('/missing.txt'),
				isVFSError,
				'VFS_NOT_FOUND',
			);
		});

		it('throws VFS_NOT_FILE for a directory', () => {
			const vfs = createFS();
			vfs.mkdir('/dir');
			expectGuardedThrow(() => vfs.history('/dir'), isVFSError, 'VFS_NOT_FILE');
		});
	});

	// -- diff -------------------------------------------------------------

	describe('diff', () => {
		it('diffs two identical files with no hunks', () => {
			const vfs = createFS();
			vfs.writeFile('/a.txt', 'hello\nworld');
			vfs.writeFile('/b.txt', 'hello\nworld');

			const result = vfs.diff('/a.txt', '/b.txt');
			expect(result.additions).toBe(0);
			expect(result.deletions).toBe(0);
			expect(result.hunks.length).toBe(0);
		});

		it('detects additions', () => {
			const vfs = createFS();
			vfs.writeFile('/a.txt', 'line1\nline2');
			vfs.writeFile('/b.txt', 'line1\nline2\nline3');

			const result = vfs.diff('/a.txt', '/b.txt');
			expect(result.additions).toBe(1);
			expect(result.deletions).toBe(0);
			expect(result.oldPath).toBe('/a.txt');
			expect(result.newPath).toBe('/b.txt');
		});

		it('detects deletions', () => {
			const vfs = createFS();
			vfs.writeFile('/a.txt', 'line1\nline2\nline3');
			vfs.writeFile('/b.txt', 'line1\nline3');

			const result = vfs.diff('/a.txt', '/b.txt');
			expect(result.deletions).toBe(1);
			expect(result.additions).toBe(0);
		});

		it('detects both additions and deletions', () => {
			const vfs = createFS();
			vfs.writeFile('/a.txt', 'aaa\nbbb\nccc');
			vfs.writeFile('/b.txt', 'aaa\nxxx\nccc');

			const result = vfs.diff('/a.txt', '/b.txt');
			expect(result.additions).toBeGreaterThanOrEqual(1);
			expect(result.deletions).toBeGreaterThanOrEqual(1);
		});

		it('handles empty file comparisons', () => {
			const vfs = createFS();
			vfs.writeFile('/a.txt', '');
			vfs.writeFile('/b.txt', 'hello');

			const result = vfs.diff('/a.txt', '/b.txt');
			expect(result.additions).toBe(1);
			expect(result.deletions).toBe(1);
		});

		it('respects context option', () => {
			const vfs = createFS();
			const lines = Array.from({ length: 20 }, (_, i) => `line${i + 1}`);
			vfs.writeFile('/a.txt', lines.join('\n'));

			const modified = [...lines];
			modified[10] = 'CHANGED';
			vfs.writeFile('/b.txt', modified.join('\n'));

			const result1 = vfs.diff('/a.txt', '/b.txt', { context: 1 });
			const result2 = vfs.diff('/a.txt', '/b.txt', { context: 5 });

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

		it('returns frozen result', () => {
			const vfs = createFS();
			vfs.writeFile('/a.txt', 'hello');
			vfs.writeFile('/b.txt', 'world');
			const result = vfs.diff('/a.txt', '/b.txt');
			expect(Object.isFrozen(result)).toBe(true);
			expect(Object.isFrozen(result.hunks)).toBe(true);
		});

		it('throws VFS_NOT_FOUND for non-existent file', () => {
			const vfs = createFS();
			vfs.writeFile('/a.txt', 'hello');
			expectGuardedThrow(
				() => vfs.diff('/a.txt', '/missing.txt'),
				isVFSError,
				'VFS_NOT_FOUND',
			);
		});

		it('throws VFS_NOT_FILE for a directory', () => {
			const vfs = createFS();
			vfs.writeFile('/a.txt', 'hello');
			vfs.mkdir('/dir');
			expectGuardedThrow(
				() => vfs.diff('/a.txt', '/dir'),
				isVFSError,
				'VFS_NOT_FILE',
			);
		});

		it('hunk line numbers are correct', () => {
			const vfs = createFS();
			vfs.writeFile('/a.txt', 'aaa\nbbb\nccc');
			vfs.writeFile('/b.txt', 'aaa\nbbb\nccc\nddd');

			const result = vfs.diff('/a.txt', '/b.txt');
			expect(result.hunks.length).toBeGreaterThan(0);
			const hunk = result.hunks[0];
			expect(hunk.oldStart).toBeGreaterThan(0);
			expect(hunk.newStart).toBeGreaterThan(0);
		});
	});

	// -- diffVersions -----------------------------------------------------

	describe('diffVersions', () => {
		it('diffs two history versions', () => {
			const vfs = createFS();
			vfs.writeFile('/f.txt', 'line1\nline2');
			vfs.writeFile('/f.txt', 'line1\nline2\nline3');
			vfs.writeFile('/f.txt', 'line1\nline3');

			// v1 = 'line1\nline2', v2 = 'line1\nline2\nline3', current (v3) = 'line1\nline3'
			const result = vfs.diffVersions('/f.txt', 1, 2);
			expect(result.additions).toBe(1);
			expect(result.oldPath).toContain('@v1');
			expect(result.newPath).toContain('@v2');
		});

		it('diffs a history version against current', () => {
			const vfs = createFS();
			vfs.writeFile('/f.txt', 'old');
			vfs.writeFile('/f.txt', 'new');

			const result = vfs.diffVersions('/f.txt', 1);
			expect(result.oldPath).toContain('@v1');
			expect(result.newPath).toContain('@v2');
		});

		it('throws VFS_NOT_FOUND for invalid version', () => {
			const vfs = createFS();
			vfs.writeFile('/f.txt', 'hello');

			expectGuardedThrow(
				() => vfs.diffVersions('/f.txt', 5),
				isVFSError,
				'VFS_NOT_FOUND',
			);
		});

		it('returns no changes when diffing same version', () => {
			const vfs = createFS();
			vfs.writeFile('/f.txt', 'hello');
			vfs.writeFile('/f.txt', 'world');

			const result = vfs.diffVersions('/f.txt', 1, 1);
			expect(result.additions).toBe(0);
			expect(result.deletions).toBe(0);
		});

		it('throws VFS_NOT_FOUND for non-existent file', () => {
			const vfs = createFS();
			expectGuardedThrow(
				() => vfs.diffVersions('/missing.txt', 1),
				isVFSError,
				'VFS_NOT_FOUND',
			);
		});
	});

	// -- checkout ---------------------------------------------------------

	describe('checkout', () => {
		it('restores a previous version of a file', () => {
			const vfs = createFS();
			vfs.writeFile('/f.txt', 'version1');
			vfs.writeFile('/f.txt', 'version2');
			vfs.writeFile('/f.txt', 'version3');

			// History: v1='version1', v2='version2', current(v3)='version3'
			vfs.checkout('/f.txt', 1);
			expect(vfs.readFile('/f.txt').text).toBe('version1');
		});

		it('records current state as history before checkout', () => {
			const vfs = createFS();
			vfs.writeFile('/f.txt', 'v1');
			vfs.writeFile('/f.txt', 'v2');

			// History: [v1='v1'], current(v2)='v2'
			vfs.checkout('/f.txt', 1);

			// After checkout, 'v2' should be in history
			const hist = vfs.history('/f.txt');
			expect(hist.some((h) => h.text === 'v2')).toBe(true);
		});

		it('is a no-op when checking out current version', () => {
			const vfs = createFS();
			vfs.writeFile('/f.txt', 'v1');
			vfs.writeFile('/f.txt', 'v2');

			// Current version is 2 (history has 1 entry)
			vfs.checkout('/f.txt', 2);
			expect(vfs.readFile('/f.txt').text).toBe('v2');
		});

		it('updates totalSize after checkout', () => {
			const vfs = createFS();
			vfs.writeFile('/f.txt', 'short');
			vfs.writeFile('/f.txt', 'this is a much longer string');

			const sizeBefore = vfs.totalSize;
			vfs.checkout('/f.txt', 1);
			expect(vfs.totalSize).toBeLessThan(sizeBefore);
		});

		it('handles binary file checkout', () => {
			const vfs = createFS();
			vfs.writeFile('/bin.dat', new Uint8Array([1, 2, 3]));
			vfs.writeFile('/bin.dat', new Uint8Array([4, 5, 6, 7]));

			vfs.checkout('/bin.dat', 1);
			const result = vfs.readFile('/bin.dat');
			expect(result.contentType).toBe('binary');
			expect(result.data).toEqual(new Uint8Array([1, 2, 3]));
		});

		it('throws VFS_NOT_FOUND for invalid version', () => {
			const vfs = createFS();
			vfs.writeFile('/f.txt', 'hello');

			expectGuardedThrow(
				() => vfs.checkout('/f.txt', 99),
				isVFSError,
				'VFS_NOT_FOUND',
			);
		});

		it('throws VFS_NOT_FOUND for non-existent file', () => {
			const vfs = createFS();
			expectGuardedThrow(
				() => vfs.checkout('/missing.txt', 1),
				isVFSError,
				'VFS_NOT_FOUND',
			);
		});

		it('throws VFS_NOT_FILE for a directory', () => {
			const vfs = createFS();
			vfs.mkdir('/dir');
			expectGuardedThrow(
				() => vfs.checkout('/dir', 1),
				isVFSError,
				'VFS_NOT_FILE',
			);
		});
	});

	// -- search -----------------------------------------------------------

	describe('search', () => {
		it('finds substring matches in text files', () => {
			const vfs = createFS();
			vfs.writeFile('/f.txt', 'hello world\nfoo bar\nhello again');

			const results = vfs.search('hello');
			expect(results.length).toBe(2);
			expect(results[0].path).toBe('/f.txt');
			expect(results[0].line).toBe(1);
			expect(results[0].column).toBe(1);
			expect(results[1].line).toBe(3);
		});

		it('returns line and column accurately', () => {
			const vfs = createFS();
			vfs.writeFile('/f.txt', 'abcXYZdef');
			const results = vfs.search('XYZ');
			expect(results.length).toBe(1);
			expect(results[0].line).toBe(1);
			expect(results[0].column).toBe(4);
			expect(results[0].match).toBe('abcXYZdef');
		});

		it('respects maxResults', () => {
			const vfs = createFS();
			vfs.writeFile('/f.txt', 'aaa\naaa\naaa\naaa\naaa');
			const results = vfs.search('aaa', { maxResults: 2 });
			expect(results.length).toBe(2);
		});

		it('filters by glob pattern', () => {
			const vfs = createFS();
			vfs.mkdir('/src');
			vfs.writeFile('/src/a.ts', 'const x = 1;');
			vfs.writeFile('/src/b.js', 'const x = 2;');

			const results = vfs.search('const', { glob: '/**/*.ts' });
			expect(results.length).toBe(1);
			expect(results[0].path).toBe('/src/a.ts');
		});

		it('skips binary files', () => {
			const vfs = createFS();
			vfs.writeFile('/bin.dat', new Uint8Array([104, 101, 108, 108, 111]));
			vfs.writeFile('/f.txt', 'hello');
			const results = vfs.search('hello');
			expect(results.length).toBe(1);
			expect(results[0].path).toBe('/f.txt');
		});

		it('returns empty array when no matches', () => {
			const vfs = createFS();
			vfs.writeFile('/f.txt', 'hello');
			expect(vfs.search('xyz')).toEqual([]);
		});

		it('returns frozen results', () => {
			const vfs = createFS();
			vfs.writeFile('/f.txt', 'hello');
			const results = vfs.search('hello');
			expect(Object.isFrozen(results)).toBe(true);
		});
	});

	// -----------------------------------------------------------------------
	// onFileWrite callback
	// -----------------------------------------------------------------------

	describe('onFileWrite', () => {
		it('fires on writeFile for new file', () => {
			const events: any[] = [];
			const vfs = createFS({ onFileWrite: (e) => events.push(e) });

			vfs.writeFile('/f.txt', 'hello');

			expect(events.length).toBe(1);
			expect(events[0].path).toBe('/f.txt');
			expect(events[0].contentType).toBe('text');
			expect(events[0].size).toBe(5);
			expect(events[0].isNew).toBe(true);
		});

		it('fires on writeFile for overwrite', () => {
			const events: any[] = [];
			const vfs = createFS({ onFileWrite: (e) => events.push(e) });

			vfs.writeFile('/f.txt', 'old');
			vfs.writeFile('/f.txt', 'new content');

			expect(events.length).toBe(2);
			expect(events[1].isNew).toBe(false);
			expect(events[1].size).toBe(11);
		});

		it('fires on writeFile for binary', () => {
			const events: any[] = [];
			const vfs = createFS({ onFileWrite: (e) => events.push(e) });

			vfs.writeFile('/bin.dat', new Uint8Array([1, 2, 3]));

			expect(events[0].contentType).toBe('binary');
			expect(events[0].size).toBe(3);
			expect(events[0].isNew).toBe(true);
		});

		it('fires on appendFile', () => {
			const events: any[] = [];
			const vfs = createFS({ onFileWrite: (e) => events.push(e) });

			vfs.writeFile('/f.txt', 'hello');
			vfs.appendFile('/f.txt', ' world');

			expect(events.length).toBe(2);
			expect(events[1].path).toBe('/f.txt');
			expect(events[1].isNew).toBe(false);
			expect(events[1].size).toBe(11);
		});

		it('does not fire when no callback provided', () => {
			const vfs = createFS();
			// Should not throw
			vfs.writeFile('/f.txt', 'hello');
			vfs.appendFile('/f.txt', ' world');
		});
	});
});
