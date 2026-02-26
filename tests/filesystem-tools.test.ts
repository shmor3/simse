import { afterEach, beforeEach, describe, expect, it } from 'bun:test';
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { registerFilesystemTools } from '../src/ai/tools/host/filesystem.js';
import { createToolRegistry } from '../src/ai/tools/tool-registry.js';
import type { ToolRegistry } from '../src/ai/tools/types.js';
import { createSilentLogger } from './utils/mocks.js';

// ---------------------------------------------------------------------------
// Test setup
// ---------------------------------------------------------------------------

let tempDir: string;
let registry: ToolRegistry;

beforeEach(async () => {
	tempDir = await mkdtemp(join(tmpdir(), 'simse-fs-test-'));
	registry = createToolRegistry({ logger: createSilentLogger() });
	registerFilesystemTools(registry, { workingDirectory: tempDir });
});

afterEach(async () => {
	await rm(tempDir, { recursive: true, force: true });
});

// ---------------------------------------------------------------------------
// fs_read
// ---------------------------------------------------------------------------

describe('fs_read', () => {
	it('reads a file and returns line-numbered content', async () => {
		await writeFile(join(tempDir, 'hello.txt'), 'line1\nline2\nline3', 'utf-8');

		const result = await registry.execute({
			id: 'r1',
			name: 'fs_read',
			arguments: { path: 'hello.txt' },
		});

		expect(result.isError).toBe(false);
		expect(result.output).toContain('line1');
		expect(result.output).toContain('line2');
		expect(result.output).toContain('line3');
	});

	it('supports offset and limit parameters', async () => {
		await writeFile(join(tempDir, 'lines.txt'), 'a\nb\nc\nd\ne', 'utf-8');

		const result = await registry.execute({
			id: 'r2',
			name: 'fs_read',
			arguments: { path: 'lines.txt', offset: 2, limit: 2 },
		});

		expect(result.isError).toBe(false);
		expect(result.output).toContain('b');
		expect(result.output).toContain('c');
		expect(result.output).not.toContain('\ta\n');
		expect(result.output).not.toContain('\td\n');
	});

	it('rejects path escape attempts', async () => {
		const result = await registry.execute({
			id: 'r3',
			name: 'fs_read',
			arguments: { path: '../../etc/passwd' },
		});

		expect(result.isError).toBe(true);
		expect(result.output).toContain('escapes');
	});
});

// ---------------------------------------------------------------------------
// fs_write
// ---------------------------------------------------------------------------

describe('fs_write', () => {
	it('creates a file and reports bytes written', async () => {
		const result = await registry.execute({
			id: 'w1',
			name: 'fs_write',
			arguments: { path: 'output.txt', content: 'hello world' },
		});

		expect(result.isError).toBe(false);
		expect(result.output).toContain('11 bytes');

		// Verify file was actually created
		const readResult = await registry.execute({
			id: 'r1',
			name: 'fs_read',
			arguments: { path: 'output.txt' },
		});
		expect(readResult.output).toContain('hello world');
	});

	it('auto-creates parent directories', async () => {
		const result = await registry.execute({
			id: 'w2',
			name: 'fs_write',
			arguments: { path: 'deep/nested/dir/file.txt', content: 'nested' },
		});

		expect(result.isError).toBe(false);

		const readResult = await registry.execute({
			id: 'r2',
			name: 'fs_read',
			arguments: { path: 'deep/nested/dir/file.txt' },
		});
		expect(readResult.output).toContain('nested');
	});
});

// ---------------------------------------------------------------------------
// fs_edit
// ---------------------------------------------------------------------------

describe('fs_edit', () => {
	it('replaces text in a file', async () => {
		await writeFile(
			join(tempDir, 'edit-me.txt'),
			'const x = 1;\nconst y = 2;\n',
			'utf-8',
		);

		const result = await registry.execute({
			id: 'e1',
			name: 'fs_edit',
			arguments: {
				path: 'edit-me.txt',
				old_string: 'const x = 1;',
				new_string: 'const x = 42;',
			},
		});

		expect(result.isError).toBe(false);
		expect(result.output).toContain('exact');

		const readResult = await registry.execute({
			id: 'r1',
			name: 'fs_read',
			arguments: { path: 'edit-me.txt' },
		});
		expect(readResult.output).toContain('const x = 42;');
	});

	it('returns error when no match is found', async () => {
		await writeFile(join(tempDir, 'no-match.txt'), 'hello\n', 'utf-8');

		const result = await registry.execute({
			id: 'e2',
			name: 'fs_edit',
			arguments: {
				path: 'no-match.txt',
				old_string: 'this does not exist in the file at all',
				new_string: 'replacement',
			},
		});

		expect(result.isError).toBe(true);
		expect(result.output).toContain('No match');
	});
});

// ---------------------------------------------------------------------------
// fs_glob
// ---------------------------------------------------------------------------

describe('fs_glob', () => {
	it('finds files by pattern', async () => {
		await mkdir(join(tempDir, 'src'), { recursive: true });
		await writeFile(join(tempDir, 'src', 'a.ts'), 'a', 'utf-8');
		await writeFile(join(tempDir, 'src', 'b.ts'), 'b', 'utf-8');
		await writeFile(join(tempDir, 'src', 'c.json'), 'c', 'utf-8');

		const result = await registry.execute({
			id: 'g1',
			name: 'fs_glob',
			arguments: { pattern: '**/*.ts' },
		});

		expect(result.isError).toBe(false);
		expect(result.output).toContain('a.ts');
		expect(result.output).toContain('b.ts');
		expect(result.output).not.toContain('c.json');
	});

	it('returns empty message when no files match', async () => {
		const result = await registry.execute({
			id: 'g2',
			name: 'fs_glob',
			arguments: { pattern: '**/*.xyz' },
		});

		expect(result.isError).toBe(false);
		expect(result.output).toContain('No files found');
	});
});

// ---------------------------------------------------------------------------
// fs_grep
// ---------------------------------------------------------------------------

describe('fs_grep', () => {
	it('searches file contents with regex', async () => {
		await mkdir(join(tempDir, 'src'), { recursive: true });
		await writeFile(
			join(tempDir, 'src', 'app.ts'),
			'function hello() {\n  return "world";\n}\n',
			'utf-8',
		);
		await writeFile(
			join(tempDir, 'src', 'util.ts'),
			'export const PI = 3.14;\n',
			'utf-8',
		);

		const result = await registry.execute({
			id: 'gr1',
			name: 'fs_grep',
			arguments: { pattern: 'function' },
		});

		expect(result.isError).toBe(false);
		expect(result.output).toContain('app.ts');
		expect(result.output).toContain('function hello');
	});

	it('returns no-match message when nothing found', async () => {
		await writeFile(join(tempDir, 'empty.txt'), 'nothing here\n', 'utf-8');

		const result = await registry.execute({
			id: 'gr2',
			name: 'fs_grep',
			arguments: { pattern: 'nonexistent_pattern_xyz' },
		});

		expect(result.isError).toBe(false);
		expect(result.output).toContain('No matches');
	});
});

// ---------------------------------------------------------------------------
// fs_list
// ---------------------------------------------------------------------------

describe('fs_list', () => {
	it('lists directory contents', async () => {
		await writeFile(join(tempDir, 'file1.txt'), 'a', 'utf-8');
		await writeFile(join(tempDir, 'file2.txt'), 'b', 'utf-8');
		await mkdir(join(tempDir, 'subdir'), { recursive: true });

		const result = await registry.execute({
			id: 'l1',
			name: 'fs_list',
			arguments: {},
		});

		expect(result.isError).toBe(false);
		expect(result.output).toContain('file1.txt');
		expect(result.output).toContain('file2.txt');
		expect(result.output).toContain('subdir');
	});

	it('shows file/directory type indicators', async () => {
		await writeFile(join(tempDir, 'a.txt'), 'a', 'utf-8');
		await mkdir(join(tempDir, 'dir'), { recursive: true });

		const result = await registry.execute({
			id: 'l2',
			name: 'fs_list',
			arguments: {},
		});

		expect(result.isError).toBe(false);
		expect(result.output).toContain('f a.txt');
		expect(result.output).toContain('d dir');
	});

	it('returns empty message for empty directory', async () => {
		await mkdir(join(tempDir, 'empty'), { recursive: true });

		const result = await registry.execute({
			id: 'l3',
			name: 'fs_list',
			arguments: { path: 'empty' },
		});

		expect(result.isError).toBe(false);
		expect(result.output).toContain('empty');
	});
});
