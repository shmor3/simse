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
	tempDir = await mkdtemp(join(tmpdir(), 'simse-fs-extra-test-'));
	registry = createToolRegistry({ logger: createSilentLogger() });
	registerFilesystemTools(registry, { workingDirectory: tempDir });
});

afterEach(async () => {
	await rm(tempDir, { recursive: true, force: true });
});

// ---------------------------------------------------------------------------
// fs_stat
// ---------------------------------------------------------------------------

describe('fs_stat', () => {
	it('returns file metadata as JSON', async () => {
		await writeFile(join(tempDir, 'test.txt'), 'hello world', 'utf-8');

		const result = await registry.execute({
			id: 's1',
			name: 'fs_stat',
			arguments: { path: 'test.txt' },
		});

		expect(result.isError).toBe(false);
		const parsed = JSON.parse(result.output);
		expect(parsed.type).toBe('file');
		expect(parsed.size).toBe(11); // "hello world"
		expect(parsed.modified).toBeDefined();
		expect(parsed.permissions).toBeDefined();
	});

	it('returns directory metadata', async () => {
		await mkdir(join(tempDir, 'subdir'));

		const result = await registry.execute({
			id: 's2',
			name: 'fs_stat',
			arguments: { path: 'subdir' },
		});

		expect(result.isError).toBe(false);
		const parsed = JSON.parse(result.output);
		expect(parsed.type).toBe('directory');
	});

	it('errors on non-existent path', async () => {
		const result = await registry.execute({
			id: 's3',
			name: 'fs_stat',
			arguments: { path: 'nonexistent.txt' },
		});

		expect(result.isError).toBe(true);
	});

	it('rejects path escape attempts', async () => {
		const result = await registry.execute({
			id: 's4',
			name: 'fs_stat',
			arguments: { path: '../../etc/passwd' },
		});

		expect(result.isError).toBe(true);
		expect(result.output).toContain('escapes');
	});
});

// ---------------------------------------------------------------------------
// fs_delete
// ---------------------------------------------------------------------------

describe('fs_delete', () => {
	it('deletes a file', async () => {
		await writeFile(join(tempDir, 'to-delete.txt'), 'bye', 'utf-8');

		const result = await registry.execute({
			id: 'd1',
			name: 'fs_delete',
			arguments: { path: 'to-delete.txt' },
		});

		expect(result.isError).toBe(false);
		expect(result.output).toContain('Deleted');

		// Verify file is gone
		const statResult = await registry.execute({
			id: 'd1b',
			name: 'fs_stat',
			arguments: { path: 'to-delete.txt' },
		});
		expect(statResult.isError).toBe(true);
	});

	it('deletes an empty directory', async () => {
		await mkdir(join(tempDir, 'empty-dir'));

		const result = await registry.execute({
			id: 'd2',
			name: 'fs_delete',
			arguments: { path: 'empty-dir' },
		});

		expect(result.isError).toBe(false);
	});

	it('fails on non-empty directory without recursive flag', async () => {
		await mkdir(join(tempDir, 'nonempty'));
		await writeFile(join(tempDir, 'nonempty', 'file.txt'), 'data', 'utf-8');

		const result = await registry.execute({
			id: 'd3',
			name: 'fs_delete',
			arguments: { path: 'nonempty' },
		});

		expect(result.isError).toBe(true);
	});

	it('recursively deletes non-empty directory with recursive=true', async () => {
		await mkdir(join(tempDir, 'deep/nested'), { recursive: true });
		await writeFile(join(tempDir, 'deep/nested/file.txt'), 'content', 'utf-8');

		const result = await registry.execute({
			id: 'd4',
			name: 'fs_delete',
			arguments: { path: 'deep', recursive: true },
		});

		expect(result.isError).toBe(false);
		expect(result.output).toContain('recursive');
	});

	it('rejects path escape attempts', async () => {
		const result = await registry.execute({
			id: 'd5',
			name: 'fs_delete',
			arguments: { path: '../../etc/passwd' },
		});

		expect(result.isError).toBe(true);
		expect(result.output).toContain('escapes');
	});
});

// ---------------------------------------------------------------------------
// fs_move
// ---------------------------------------------------------------------------

describe('fs_move', () => {
	it('moves a file', async () => {
		await writeFile(join(tempDir, 'src.txt'), 'content', 'utf-8');

		const result = await registry.execute({
			id: 'm1',
			name: 'fs_move',
			arguments: { source: 'src.txt', destination: 'dst.txt' },
		});

		expect(result.isError).toBe(false);
		expect(result.output).toContain('Moved');

		// Verify src is gone and dst exists
		const srcStat = await registry.execute({
			id: 'm1b',
			name: 'fs_stat',
			arguments: { path: 'src.txt' },
		});
		expect(srcStat.isError).toBe(true);

		const dstRead = await registry.execute({
			id: 'm1c',
			name: 'fs_read',
			arguments: { path: 'dst.txt' },
		});
		expect(dstRead.isError).toBe(false);
		expect(dstRead.output).toContain('content');
	});

	it('creates parent directories for destination', async () => {
		await writeFile(join(tempDir, 'file.txt'), 'data', 'utf-8');

		const result = await registry.execute({
			id: 'm2',
			name: 'fs_move',
			arguments: { source: 'file.txt', destination: 'new/dir/file.txt' },
		});

		expect(result.isError).toBe(false);

		const readResult = await registry.execute({
			id: 'm2b',
			name: 'fs_read',
			arguments: { path: 'new/dir/file.txt' },
		});
		expect(readResult.isError).toBe(false);
	});

	it('rejects source path escape', async () => {
		const result = await registry.execute({
			id: 'm3',
			name: 'fs_move',
			arguments: { source: '../../etc/passwd', destination: 'stolen.txt' },
		});

		expect(result.isError).toBe(true);
		expect(result.output).toContain('escapes');
	});

	it('rejects destination path escape', async () => {
		await writeFile(join(tempDir, 'file.txt'), 'data', 'utf-8');

		const result = await registry.execute({
			id: 'm4',
			name: 'fs_move',
			arguments: { source: 'file.txt', destination: '../../tmp/evil.txt' },
		});

		expect(result.isError).toBe(true);
		expect(result.output).toContain('escapes');
	});

	it('errors on non-existent source', async () => {
		const result = await registry.execute({
			id: 'm5',
			name: 'fs_move',
			arguments: { source: 'nonexistent.txt', destination: 'dst.txt' },
		});

		expect(result.isError).toBe(true);
	});
});
