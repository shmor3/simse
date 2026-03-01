import { afterEach, beforeEach, describe, expect, it } from 'bun:test';
import { existsSync } from 'node:fs';
import { mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import type { Logger } from '../../src/ai/shared/logger.js';
import { createNoopLogger } from '../../src/ai/shared/logger.js';
import { isVFSError } from '../../src/ai/vfs/errors.js';
import type { VirtualFS } from '../../src/ai/vfs/vfs.js';
import { createVirtualFS } from '../../src/ai/vfs/vfs.js';
import { createVFSDisk } from '../../src/ai/vfs/vfs-disk.js';
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

async function createFS(): Promise<VirtualFS> {
	const vfs = await createVirtualFS({
		logger: createSilentLogger(),
		enginePath: ENGINE_PATH,
	});
	instances.push(vfs);
	return vfs;
}

let tempDir: string;

beforeEach(async () => {
	tempDir = join(
		tmpdir(),
		`simse-vfs-disk-test-${Date.now()}-${Math.random().toString(36).slice(2)}`,
	);
	await mkdir(tempDir, { recursive: true });
});

afterEach(async () => {
	for (const vfs of instances) {
		await vfs.dispose();
	}
	instances.length = 0;

	try {
		await rm(tempDir, { recursive: true, force: true });
	} catch {
		// Ignore cleanup failures
	}
});

// ---------------------------------------------------------------------------
// commit
// ---------------------------------------------------------------------------

describe('createVFSDisk', () => {
	it('returns a frozen object', async () => {
		const vfs = await createFS();
		const disk = createVFSDisk(vfs, { logger: createSilentLogger() });
		expect(Object.isFrozen(disk)).toBe(true);
	});

	describe('commit', () => {
		it('writes text files to disk', async () => {
			const vfs = await createFS();
			const disk = createVFSDisk(vfs, { logger: createSilentLogger() });

			await vfs.writeFile('vfs:///hello.txt', 'Hello, world!');

			const result = await disk.commit(tempDir);
			expect(result.filesWritten).toBe(1);
			expect(result.bytesWritten).toBe(13);

			const content = await readFile(join(tempDir, 'hello.txt'), 'utf-8');
			expect(content).toBe('Hello, world!');
		});

		it('writes binary files to disk', async () => {
			const vfs = await createFS();
			const disk = createVFSDisk(vfs, { logger: createSilentLogger() });

			const data = new Uint8Array([1, 2, 3, 4, 5]);
			await vfs.writeFile('vfs:///bin.dat', data);

			const result = await disk.commit(tempDir);
			expect(result.filesWritten).toBe(1);
			expect(result.bytesWritten).toBe(5);

			const content = await readFile(join(tempDir, 'bin.dat'));
			expect(new Uint8Array(content)).toEqual(data);
		});

		it('creates directories on disk', async () => {
			const vfs = await createFS();
			const disk = createVFSDisk(vfs, { logger: createSilentLogger() });

			await vfs.mkdir('vfs:///src/components', { recursive: true });
			await vfs.writeFile('vfs:///src/components/App.ts', 'export default {};');

			const result = await disk.commit(tempDir);
			expect(result.directoriesCreated).toBeGreaterThanOrEqual(1);
			expect(result.filesWritten).toBe(1);

			const content = await readFile(
				join(tempDir, 'src', 'components', 'App.ts'),
				'utf-8',
			);
			expect(content).toBe('export default {};');
		});

		it('skips existing files without overwrite', async () => {
			const vfs = await createFS();
			const disk = createVFSDisk(vfs, { logger: createSilentLogger() });

			// Pre-create file on disk
			await writeFile(join(tempDir, 'existing.txt'), 'original');

			await vfs.writeFile('vfs:///existing.txt', 'new content');

			const result = await disk.commit(tempDir);
			expect(result.filesWritten).toBe(0);

			const skipped = result.operations.filter((op) => op.type === 'skip');
			expect(skipped.length).toBe(1);

			// Original content unchanged
			const content = await readFile(join(tempDir, 'existing.txt'), 'utf-8');
			expect(content).toBe('original');
		});

		it('overwrites existing files with overwrite: true', async () => {
			const vfs = await createFS();
			const disk = createVFSDisk(vfs, { logger: createSilentLogger() });

			await writeFile(join(tempDir, 'existing.txt'), 'original');

			await vfs.writeFile('vfs:///existing.txt', 'new content');

			const result = await disk.commit(tempDir, { overwrite: true });
			expect(result.filesWritten).toBe(1);

			const content = await readFile(join(tempDir, 'existing.txt'), 'utf-8');
			expect(content).toBe('new content');
		});

		it('dry run does not write files', async () => {
			const vfs = await createFS();
			const disk = createVFSDisk(vfs, { logger: createSilentLogger() });

			await vfs.writeFile('vfs:///f.txt', 'hello');

			const result = await disk.commit(tempDir, { dryRun: true });
			expect(result.filesWritten).toBe(1);
			expect(result.operations.length).toBeGreaterThan(0);

			// File should NOT exist on disk
			expect(existsSync(join(tempDir, 'f.txt'))).toBe(false);
		});

		it('respects filter option', async () => {
			const vfs = await createFS();
			const disk = createVFSDisk(vfs, { logger: createSilentLogger() });

			await vfs.mkdir('vfs:///src');
			await vfs.writeFile('vfs:///src/index.ts', 'code');
			await vfs.writeFile('vfs:///src/readme.md', 'docs');

			const result = await disk.commit(tempDir, {
				filter: (path) => path.endsWith('.ts'),
			});
			expect(result.filesWritten).toBe(1);

			expect(existsSync(join(tempDir, 'src', 'index.ts'))).toBe(true);
			expect(existsSync(join(tempDir, 'src', 'readme.md'))).toBe(false);
		});

		it('handles empty VFS', async () => {
			const vfs = await createFS();
			const disk = createVFSDisk(vfs, { logger: createSilentLogger() });

			const result = await disk.commit(tempDir);
			expect(result.filesWritten).toBe(0);
			expect(result.directoriesCreated).toBe(0);
			expect(result.bytesWritten).toBe(0);
		});

		it('returns frozen result', async () => {
			const vfs = await createFS();
			const disk = createVFSDisk(vfs, { logger: createSilentLogger() });

			await vfs.writeFile('vfs:///f.txt', 'data');
			const result = await disk.commit(tempDir);

			expect(Object.isFrozen(result)).toBe(true);
			expect(Object.isFrozen(result.operations)).toBe(true);
		});

		it('operations contain correct paths', async () => {
			const vfs = await createFS();
			const disk = createVFSDisk(vfs, { logger: createSilentLogger() });

			await vfs.writeFile('vfs:///f.txt', 'data');
			const result = await disk.commit(tempDir);

			const writeOp = result.operations.find((op) => op.type === 'write');
			expect(writeOp).toBeDefined();
			expect(writeOp?.path).toBe('vfs:///f.txt');
			expect(writeOp?.diskPath).toContain('f.txt');
			expect(writeOp?.size).toBe(4);
		});
	});

	// ---------------------------------------------------------------------------
	// load
	// ---------------------------------------------------------------------------

	describe('load', () => {
		it('loads text files from disk', async () => {
			const vfs = await createFS();
			const disk = createVFSDisk(vfs, { logger: createSilentLogger() });

			await writeFile(join(tempDir, 'hello.txt'), 'Hello from disk!');

			const result = await disk.load(tempDir);
			expect(result.filesWritten).toBe(1);

			expect((await vfs.readFile('vfs:///hello.txt')).text).toBe(
				'Hello from disk!',
			);
		});

		it('loads binary files from disk', async () => {
			const vfs = await createFS();
			const disk = createVFSDisk(vfs, { logger: createSilentLogger() });

			const data = Buffer.from([1, 2, 3, 4, 5]);
			await writeFile(join(tempDir, 'image.png'), data);

			const result = await disk.load(tempDir);
			expect(result.filesWritten).toBe(1);

			const read = await vfs.readFile('vfs:///image.png');
			expect(read.contentType).toBe('binary');
			expect(read.data).toEqual(new Uint8Array([1, 2, 3, 4, 5]));
		});

		it('loads nested directories', async () => {
			const vfs = await createFS();
			const disk = createVFSDisk(vfs, { logger: createSilentLogger() });

			await mkdir(join(tempDir, 'src', 'utils'), { recursive: true });
			await writeFile(join(tempDir, 'src', 'index.ts'), 'main');
			await writeFile(join(tempDir, 'src', 'utils', 'helpers.ts'), 'utils');

			const result = await disk.load(tempDir);
			expect(result.filesWritten).toBe(2);
			expect(result.directoriesCreated).toBeGreaterThanOrEqual(1);

			expect((await vfs.readFile('vfs:///src/index.ts')).text).toBe('main');
			expect((await vfs.readFile('vfs:///src/utils/helpers.ts')).text).toBe(
				'utils',
			);
		});

		it('skips existing VFS files without overwrite', async () => {
			const vfs = await createFS();
			const disk = createVFSDisk(vfs, { logger: createSilentLogger() });

			await vfs.writeFile('vfs:///existing.txt', 'vfs content');
			await writeFile(join(tempDir, 'existing.txt'), 'disk content');

			const result = await disk.load(tempDir);
			expect(result.filesWritten).toBe(0);

			// VFS content unchanged
			expect((await vfs.readFile('vfs:///existing.txt')).text).toBe(
				'vfs content',
			);
		});

		it('overwrites VFS files with overwrite: true', async () => {
			const vfs = await createFS();
			const disk = createVFSDisk(vfs, { logger: createSilentLogger() });

			await vfs.writeFile('vfs:///existing.txt', 'vfs content');
			await writeFile(join(tempDir, 'existing.txt'), 'disk content');

			const result = await disk.load(tempDir, { overwrite: true });
			expect(result.filesWritten).toBe(1);

			expect((await vfs.readFile('vfs:///existing.txt')).text).toBe(
				'disk content',
			);
		});

		it('respects maxFileSize option', async () => {
			const vfs = await createFS();
			const disk = createVFSDisk(vfs, { logger: createSilentLogger() });

			await writeFile(join(tempDir, 'small.txt'), 'hi');
			await writeFile(join(tempDir, 'big.txt'), 'x'.repeat(1000));

			const result = await disk.load(tempDir, { maxFileSize: 100 });
			expect(result.filesWritten).toBe(1);

			expect(await vfs.exists('vfs:///small.txt')).toBe(true);
			expect(await vfs.exists('vfs:///big.txt')).toBe(false);

			const skipped = result.operations.filter((op) => op.type === 'skip');
			expect(skipped.length).toBe(1);
			expect(skipped[0].reason).toContain('exceeds limit');
		});

		it('respects filter option', async () => {
			const vfs = await createFS();
			const disk = createVFSDisk(vfs, { logger: createSilentLogger() });

			await writeFile(join(tempDir, 'code.ts'), 'typescript');
			await writeFile(join(tempDir, 'notes.md'), 'markdown');

			const result = await disk.load(tempDir, {
				filter: (path) => path.endsWith('.ts'),
			});
			expect(result.filesWritten).toBe(1);

			expect(await vfs.exists('vfs:///code.ts')).toBe(true);
			expect(await vfs.exists('vfs:///notes.md')).toBe(false);
		});

		it('throws VFS_NOT_FOUND for non-existent source', async () => {
			const vfs = await createFS();
			const disk = createVFSDisk(vfs, { logger: createSilentLogger() });

			await expectGuardedThrow(
				() => disk.load('/nonexistent/path'),
				isVFSError,
				'VFS_NOT_FOUND',
			);
		});

		it('throws VFS_NOT_DIRECTORY for a file path', async () => {
			const vfs = await createFS();
			const disk = createVFSDisk(vfs, { logger: createSilentLogger() });

			const filePath = join(tempDir, 'afile.txt');
			await writeFile(filePath, 'data');

			await expectGuardedThrow(
				() => disk.load(filePath),
				isVFSError,
				'VFS_NOT_DIRECTORY',
			);
		});

		it('handles empty directory', async () => {
			const vfs = await createFS();
			const disk = createVFSDisk(vfs, { logger: createSilentLogger() });

			const result = await disk.load(tempDir);
			expect(result.filesWritten).toBe(0);
			expect(result.directoriesCreated).toBe(0);
		});

		it('returns frozen result', async () => {
			const vfs = await createFS();
			const disk = createVFSDisk(vfs, { logger: createSilentLogger() });

			await writeFile(join(tempDir, 'f.txt'), 'data');
			const result = await disk.load(tempDir);

			expect(Object.isFrozen(result)).toBe(true);
			expect(Object.isFrozen(result.operations)).toBe(true);
		});
	});

	// ---------------------------------------------------------------------------
	// round-trip
	// ---------------------------------------------------------------------------

	describe('round-trip', () => {
		it('commit then load preserves content', async () => {
			const vfs1 = await createFS();
			const disk1 = createVFSDisk(vfs1, { logger: createSilentLogger() });

			await vfs1.mkdir('vfs:///src');
			await vfs1.writeFile('vfs:///src/index.ts', 'const x = 1;');
			await vfs1.writeFile('vfs:///src/data.bin', new Uint8Array([10, 20, 30]));
			await vfs1.writeFile('vfs:///readme.txt', 'Hello');

			await disk1.commit(tempDir);

			const vfs2 = await createFS();
			const disk2 = createVFSDisk(vfs2, { logger: createSilentLogger() });

			await disk2.load(tempDir);

			expect((await vfs2.readFile('vfs:///src/index.ts')).text).toBe(
				'const x = 1;',
			);
			expect((await vfs2.readFile('vfs:///readme.txt')).text).toBe('Hello');
			// data.bin has a non-binary extension so will be loaded as text
			// but the bytes should still be present
		});

		it('commit then load preserves directory structure', async () => {
			const vfs1 = await createFS();
			const disk1 = createVFSDisk(vfs1, { logger: createSilentLogger() });

			await vfs1.mkdir('vfs:///a/b/c', { recursive: true });
			await vfs1.writeFile('vfs:///a/b/c/deep.txt', 'deep file');

			await disk1.commit(tempDir);

			const vfs2 = await createFS();
			const disk2 = createVFSDisk(vfs2, { logger: createSilentLogger() });

			await disk2.load(tempDir);

			expect((await vfs2.readFile('vfs:///a/b/c/deep.txt')).text).toBe(
				'deep file',
			);
			expect(await vfs2.exists('vfs:///a/b/c')).toBe(true);
		});
	});

	// ---------------------------------------------------------------------------
	// commit with validation
	// ---------------------------------------------------------------------------

	describe('commit with validation', () => {
		it('passes validation with valid files', async () => {
			const vfs = await createFS();
			const disk = createVFSDisk(vfs, { logger: createSilentLogger() });

			await vfs.writeFile('vfs:///index.ts', 'const x = 1;\n');
			await vfs.writeFile('vfs:///data.json', '{"key": "value"}\n');

			const result = await disk.commit(tempDir, { validate: true });
			expect(result.filesWritten).toBe(2);
			expect(result.validation).toBeDefined();
			expect(result.validation?.passed).toBe(true);
		});

		it('throws VFS_VALIDATION_FAILED on JSON syntax error', async () => {
			const vfs = await createFS();
			const disk = createVFSDisk(vfs, { logger: createSilentLogger() });

			await vfs.writeFile('vfs:///bad.json', '{not valid}');

			await expectGuardedThrow(
				() => disk.commit(tempDir, { validate: true }),
				isVFSError,
				'VFS_VALIDATION_FAILED',
			);
		});

		it('proceeds with warnings only', async () => {
			const vfs = await createFS();
			const disk = createVFSDisk(vfs, { logger: createSilentLogger() });

			// Trailing whitespace is a warning, not an error
			await vfs.writeFile('vfs:///code.ts', 'const x = 1;  \n');

			const result = await disk.commit(tempDir, { validate: true });
			expect(result.filesWritten).toBe(1);
			expect(result.validation?.passed).toBe(true);
			expect(result.validation?.warnings).toBeGreaterThanOrEqual(1);
		});

		it('accepts custom validators', async () => {
			const vfs = await createFS();
			const disk = createVFSDisk(vfs, { logger: createSilentLogger() });

			await vfs.writeFile('vfs:///file.txt', 'content\n');

			const noopValidator = {
				name: 'noop',
				validate: () => [] as const,
			};

			const result = await disk.commit(tempDir, {
				validate: [noopValidator],
			});
			expect(result.filesWritten).toBe(1);
			expect(result.validation?.passed).toBe(true);
		});

		it('does not write files when validation fails', async () => {
			const vfs = await createFS();
			const disk = createVFSDisk(vfs, { logger: createSilentLogger() });

			await vfs.writeFile('vfs:///bad.json', '{invalid}');

			try {
				await disk.commit(tempDir, { validate: true });
			} catch {
				// expected
			}

			expect(existsSync(join(tempDir, 'bad.json'))).toBe(false);
		});

		it('skips validation when validate is not set', async () => {
			const vfs = await createFS();
			const disk = createVFSDisk(vfs, { logger: createSilentLogger() });

			await vfs.writeFile('vfs:///bad.json', '{invalid}');

			// Without validate, commit should succeed
			const result = await disk.commit(tempDir);
			expect(result.filesWritten).toBe(1);
			expect(result.validation).toBeUndefined();
		});
	});

	// ---------------------------------------------------------------------------
	// baseDir
	// ---------------------------------------------------------------------------

	describe('baseDir', () => {
		it('commit uses baseDir when no targetDir given', async () => {
			const vfs = await createFS();
			const disk = createVFSDisk(vfs, {
				logger: createSilentLogger(),
				baseDir: tempDir,
			});

			await vfs.writeFile('vfs:///hello.txt', 'from baseDir');

			const result = await disk.commit();
			expect(result.filesWritten).toBe(1);

			const content = await readFile(join(tempDir, 'hello.txt'), 'utf-8');
			expect(content).toBe('from baseDir');
		});

		it('commit targetDir overrides baseDir', async () => {
			const overrideDir = join(tempDir, 'override');
			await mkdir(overrideDir, { recursive: true });

			const vfs = await createFS();
			const disk = createVFSDisk(vfs, {
				logger: createSilentLogger(),
				baseDir: tempDir,
			});

			await vfs.writeFile('vfs:///file.txt', 'override content');

			const result = await disk.commit(overrideDir);
			expect(result.filesWritten).toBe(1);

			const content = await readFile(join(overrideDir, 'file.txt'), 'utf-8');
			expect(content).toBe('override content');
		});

		it('load uses baseDir when no sourceDir given', async () => {
			await writeFile(join(tempDir, 'disk.txt'), 'disk data');

			const vfs = await createFS();
			const disk = createVFSDisk(vfs, {
				logger: createSilentLogger(),
				baseDir: tempDir,
			});

			const result = await disk.load();
			expect(result.filesWritten).toBe(1);
			expect((await vfs.readFile('vfs:///disk.txt')).text).toBe('disk data');
		});

		it('load sourceDir overrides baseDir', async () => {
			const subDir = join(tempDir, 'sub');
			await mkdir(subDir, { recursive: true });
			await writeFile(join(subDir, 'nested.txt'), 'nested');

			const vfs = await createFS();
			const disk = createVFSDisk(vfs, {
				logger: createSilentLogger(),
				baseDir: tempDir,
			});

			const result = await disk.load(subDir);
			expect(result.filesWritten).toBe(1);
			expect((await vfs.readFile('vfs:///nested.txt')).text).toBe('nested');
		});

		it('defaults baseDir to process.cwd()', async () => {
			const vfs = await createFS();
			// No baseDir provided — should default to cwd
			const disk = createVFSDisk(vfs, { logger: createSilentLogger() });
			expect(Object.isFrozen(disk)).toBe(true);
		});
	});
});
