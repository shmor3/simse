import { beforeEach, describe, expect, it } from 'bun:test';
import { mkdtemp } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { registerBashTool } from '../src/ai/tools/host/bash.js';
import { createToolRegistry } from '../src/ai/tools/tool-registry.js';
import type { ToolRegistry } from '../src/ai/tools/types.js';
import { createSilentLogger } from './utils/mocks.js';

describe('registerBashTool', () => {
	let registry: ToolRegistry;
	let tempDir: string;

	beforeEach(async () => {
		tempDir = await mkdtemp(join(tmpdir(), 'bash-tool-'));
		registry = createToolRegistry({ logger: createSilentLogger() });
		registerBashTool(registry, { workingDirectory: tempDir });
	});

	it('registers a bash tool', () => {
		expect(registry.toolNames).toContain('bash');
	});

	it('runs a simple echo command', async () => {
		const result = await registry.execute({
			id: 'call-1',
			name: 'bash',
			arguments: { command: 'echo hello world' },
		});
		expect(result.isError).toBe(false);
		expect(result.output.trim()).toBe('hello world');
	});

	it('returns exit code for failing command', async () => {
		const result = await registry.execute({
			id: 'call-2',
			name: 'bash',
			arguments: { command: 'false' },
		});
		expect(result.isError).toBe(false);
		expect(result.output).toContain('[exit code');
	});

	it('captures stderr', async () => {
		const result = await registry.execute({
			id: 'call-3',
			name: 'bash',
			arguments: { command: 'echo error-output >&2' },
		});
		expect(result.isError).toBe(false);
		expect(result.output).toContain('error-output');
	});

	it('respects timeout', async () => {
		const result = await registry.execute({
			id: 'call-4',
			name: 'bash',
			arguments: { command: 'sleep 60', timeout: 500 },
		});
		expect(result.isError).toBe(false);
		expect(result.output).toContain('[timeout after 500ms]');
	});

	it('truncates large output', async () => {
		const maxBytes = 200;
		const smallRegistry = createToolRegistry({
			logger: createSilentLogger(),
		});
		registerBashTool(smallRegistry, {
			workingDirectory: tempDir,
			maxOutputBytes: maxBytes,
		});

		// Generate output larger than maxBytes
		const result = await smallRegistry.execute({
			id: 'call-5',
			name: 'bash',
			arguments: {
				command: 'yes aaaaaaaaaa | head -n 100',
			},
		});
		expect(result.isError).toBe(false);
		expect(result.output).toContain('[truncated:');
	});

	it('uses custom cwd argument', async () => {
		const subDir = join(tempDir, 'subdir');
		await Bun.spawn(['mkdir', '-p', subDir]).exited;

		const result = await registry.execute({
			id: 'call-6',
			name: 'bash',
			arguments: { command: 'pwd', cwd: subDir },
		});
		expect(result.isError).toBe(false);
		expect(result.output.trim()).toContain('subdir');
	});
});
